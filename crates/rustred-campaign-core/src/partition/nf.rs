//! Normal form of case domains for the partition: per rule EQ literals (must vanish), GE literals (must be >= 0) and
//! BAD branches (AND-lists; the rule is excluded where every member of one branch vanishes). Literals are interned
//! once with their slot support. Pole loci become BAD branches (the AND of the d-coefficients of each exact linear
//! factor and of the cofactor: Z[n,d] has no zero divisors, so the zero set is the union over the factors).
//!
//! Every coefficient operation is checked (re-audit RA2-C1); an overflow is a typed error or an exclusion / weakening
//! recorded in the notes, never a wrapped literal.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use std::collections::{BTreeMap, HashMap};

use super::exact::{Affine, ExactError, MPoly, linear_factors, slot_names};
use super::{DomainSource, ElidedPolicy, PartitionError};
use crate::domain::{CaseDomain, TargetMap, TargetSlot};
use crate::key::NodeFrame;
use crate::poly::{D_VAR, ENGINE_PRIMES, Poly};

/// One interned literal.
#[derive(Clone, Debug)]
pub struct Lit {
    /// The literal as a polynomial in the slots (nvars = arity).
    pub poly: MPoly,
    /// The same as a core polynomial (exact evaluation with big-integer fallback).
    pub core: Poly,
    /// Slots that occur, ascending.
    pub support: Vec<usize>,
    /// GE literal (sign matters; normalised by positive content only) or zero-set literal (primitive, canonical sign).
    pub is_ge: bool,
    pub text: String,
}

/// A rule in normal form.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NfRule {
    pub eq: Vec<u32>,
    pub ge: Vec<u32>,
    pub bad: Vec<Vec<u32>>,
}

/// Counters of the normal form.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct NfStats {
    pub domains: usize,
    pub distinct: usize,
    pub dropped_out_of_chart: usize,
    pub dropped_elided: usize,
    pub dropped_dead_constant: usize,
    /// Always 0 since audit-part-B fix C2: non-canonical targets are modelled exactly (kept for the schema).
    pub excluded_unsupported_target: usize,
    /// Domains with a non-canonical target, modelled by composing every literal with the affine rule-point map.
    pub pattern_targets: usize,
    /// Domains EXCLUDED as a whole (a case literal could not be represented): they credit nothing.
    pub excluded_unrepresentable: usize,
    /// Domains represented by a SUBSET of their true domain (an exception member or pole coefficient that could not
    /// be represented is treated as vanishing everywhere).
    pub weakened: usize,
    pub pole_branches: usize,
    pub pole_linear_factors: usize,
    pub literals: usize,
    /// FailClosed / dispatch sub-problem: number of rules making up the complement of the blocking record's
    /// SELECTED set.
    pub blocked_variants: usize,
    /// Stop pieces evaluated before the equations (rule-point overflow, modular zero risk; audit-part3 M1 / C2).
    pub extra_stop_pieces: usize,
    /// Rule-point overflow pieces over-approximated by the whole fixed / constant set (literal not representable).
    pub stop_pieces_over_approximated: usize,
}

/// How one source domain entered the normal form, when not simply "represented exactly" (audit-part-A C2: every
/// exclusion is visible and none can produce a proof about the excluded domain).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum NfNote {
    /// PROVED never to apply, exactly (a fixed value or target constant outside the chart, a constant
    /// contradiction, an exception branch that always vanishes, a recenter / range bound that is never met).
    DroppedEmpty(String),
    /// Excluded as a whole: a case literal (equation or bound) could not be represented exactly. The rule credits
    /// nothing here (union coverage can only shrink); no statement about this domain is a proof.
    Excluded(String),
    /// Represented by a SUBSET of its true domain: an exception member or pole coefficient could not be represented
    /// and is treated as vanishing everywhere ("may vanish anywhere"); coverage can only shrink; no emptiness or
    /// containment statement about this domain is a proof.
    Weakened(String),
    /// Witness status not Complete (elided witness loci, or the record-level status Elided / NotCarried without
    /// attestation), removed from the credited rules (FailClosed / Absent / dispatch mode: it BLOCKS where selected).
    ElidedRemoved,
    /// Witness status not Complete, CREDITED as an ordinary domain (`ElidedPolicy::Ordinary`: a regression reference, never
    /// a proof: the engine never serves an elided record).
    ElidedCredited,
    /// The engine's MODULAR zero test may disagree with the exact one somewhere on the record's reachable rule points
    /// (an equation, exception member or pole coefficient can take a nonzero value divisible by both engine primes:
    /// [`modular_zero_risk`]; audit-part3 C2). Dispatch mode: the record STOPS dispatch (Undecided) on its whole
    /// fixed-slot / target-constant set, a stop piece that over-approximates the true one (sound for a no-gap
    /// proof). Union mode: a caveat (union coverage cannot model an undecided record).
    ModularZeroRisk(String),
}

impl NfNote {
    /// Does this note forbid every emptiness / containment PROOF about the domain itself?
    pub fn forbids_proof_about_domain(&self) -> bool {
        matches!(self, NfNote::Excluded(_) | NfNote::Weakened(_))
    }
}

/// One source note.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SourceNote {
    pub source: usize,
    pub note: NfNote,
}

/// Why a record STOPS first-match dispatch at keys where it does not apply, beyond its blocked set (the latter is
/// `selected minus applies`, see [`NfRules::selected`]). Both are evaluated by the dispatcher BEFORE the equations
/// (audit-part3 M1 / C2), so they also stop dispatch at keys where an equation fails, and for records the normal form
/// drops as never applying.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum StopWhy {
    /// `CaseDomain::rule_point`: the rule point leaves the `i32` range at target slot `slot` (`n_slot - offset`):
    /// `Verdict::Error(Overflow)`, Undecided. Exact.
    RulePointOverflow { slot: usize },
    /// The engine's modular zero test may read a nonzero literal value as zero ([`modular_zero_risk`]): the stop set
    /// is over-approximated by the record's fixed-slot / target-constant set.
    ModularZeroRisk(String),
}

/// One piece of a record's STOP set outside its selected-minus-applies set: the keys of `rule` (exact, or an
/// over-approximation for [`StopWhy::ModularZeroRisk`]) stop dispatch at the record whatever its own verdict there,
/// so the record's own APPLIES set is NOT subtracted from it (unlike the blocked set).
#[derive(Clone, Debug)]
pub struct StopPiece {
    pub rule: NfRule,
    pub why: StopWhy,
}

/// Which stop piece of a blocker a blocked sub-problem describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
pub enum StopRef {
    /// `selected(e) minus applies(e)` (pole loci, recenter / range bounds, elision, witness status).
    Selected,
    /// `extra_stops[e][j]` (rule-point overflow, modular zero risk).
    Extra(usize),
}

/// The rules of a partition run.
#[derive(Clone, Debug)]
pub struct NfRules {
    pub arity: usize,
    /// `lines[i]`: slot i is a line (floor 1, values >= 1), else a numerator (floor 0, values <= 0).
    pub lines: Vec<bool>,
    pub lits: Vec<Lit>,
    pub rules: Vec<NfRule>,
    /// Source indices merged into each rule.
    pub sources: Vec<Vec<usize>>,
    pub stats: NfStats,
    /// Per rule the fixed slots read from its one-slot unit EQ literals `n_s - v` (fast pre-check of the oracle).
    pub quick: Vec<Vec<(usize, i64)>>,
    /// FailClosed: source indices of the elided records (removed here; their blocked regions are separate).
    pub elided: Vec<usize>,
    /// Dispatch mode: source indices of every record whose SELECTED set may be larger than its APPLIES set (pole
    /// loci, recenter bounds, rule-point range bounds of a target pattern, elided witness loci): where such a record
    /// is selected but blocked it SHADOWS every later record without serving the key.
    pub blockers: Vec<usize>,
    /// Every source that was not represented exactly, with the reason.
    pub notes: Vec<SourceNote>,
    /// Per blocker source its SELECTED set (the engine's selection: fixed slots, target constants, equations,
    /// bounds, exceptions; without pole loci, recenter / range bounds, elision).
    pub selected: BTreeMap<usize, NfRule>,
    /// Per source the stop pieces evaluated BEFORE the equations (rule-point overflow, modular zero risk;
    /// audit-part3 M1 / C2); such a source is a blocker even when it never applies.
    pub extra_stops: BTreeMap<usize, Vec<StopPiece>>,
}

impl NfRules {
    /// The BLOCKED sub-problem of blocker source `e`, stop piece `which` (FailClosed / dispatch mode, audit-part-A M3,
    /// audit-part3 M1 / C2). `StopRef::Selected`: the rules of every source at or before `e` in dispatch order (`pos`:
    /// source -> position) plus the COMPLEMENT of `e`'s SELECTED set; its uncovered set is exactly
    /// `selected(e) \ applies(e) \ U_{s < e} applies(s)`. `StopRef::Extra(j)`: the rules of every source STRICTLY
    /// before `e` plus the complement of the stop piece `extra_stops[e][j]`; its uncovered set is
    /// `piece \ U_{s < e} applies(s)` (the record's own verdict there is Undecided: its applies set is not
    /// subtracted). The union of these sets over all blockers and pieces together with the union-uncovered set is the
    /// dispatch-uncovered set (bridge: interfaces/partition.md 14).
    pub fn blocked_subproblem(
        &self,
        e: usize,
        which: StopRef,
        pos: &HashMap<usize, usize>,
    ) -> Result<NfRules, PartitionError> {
        let pe = *pos
            .get(&e)
            .ok_or_else(|| PartitionError::Unsupported(format!("blocker {e} has no position")))?;
        let (sel, own) = match which {
            StopRef::Selected => (
                self.selected
                    .get(&e)
                    .ok_or_else(|| {
                        PartitionError::Unsupported(format!("source {e} has no selected set"))
                    })?
                    .clone(),
                true,
            ),
            StopRef::Extra(j) => (
                self.extra_stops
                    .get(&e)
                    .and_then(|v| v.get(j))
                    .ok_or_else(|| {
                        PartitionError::Unsupported(format!("source {e} has no stop piece {j}"))
                    })?
                    .rule
                    .clone(),
                false,
            ),
        };
        let mut b = Builder {
            arity: self.arity,
            lits: self.lits.clone(),
            index: self
                .lits
                .iter()
                .enumerate()
                .map(|(i, l)| Ok(((l.is_ge, l.poly.clone()), lit_id(i)?)))
                .collect::<Result<_, PartitionError>>()?,
        };
        let mut rules = vec![];
        let mut sources = vec![];
        for (k, r) in self.rules.iter().enumerate() {
            let before: Vec<usize> = self.sources[k]
                .iter()
                .copied()
                .filter(|s| pos.get(s).is_some_and(|&p| p < pe || (own && p == pe)))
                .collect();
            if !before.is_empty() {
                rules.push(r.clone());
                sources.push(before);
            }
        }
        let universal = NfRule {
            eq: vec![],
            ge: vec![],
            bad: vec![],
        };
        let (v, inexact) = subtract(&mut b, &universal, &sel)?;
        let mut stats = self.stats.clone();
        stats.blocked_variants = v.len();
        for r in v {
            rules.push(r);
            sources.push(vec![e]);
        }
        stats.distinct = rules.len();
        stats.literals = b.lits.len();
        let quick = rules
            .iter()
            .map(|r: &NfRule| {
                r.eq.iter()
                    .filter_map(|&l| unit_fixed(&b.lits[l as usize]))
                    .collect()
            })
            .collect();
        Ok(NfRules {
            arity: self.arity,
            lines: self.lines.clone(),
            lits: b.lits,
            rules,
            sources,
            stats,
            quick,
            elided: vec![],
            blockers: vec![],
            extra_stops: BTreeMap::new(),
            notes: if inexact {
                vec![SourceNote {
                    source: e,
                    note: NfNote::Weakened(
                        "the complement of a selection bound is not representable in i128: the blocked region is over-approximated"
                            .into(),
                    ),
                }]
            } else {
                vec![]
            },
            selected: BTreeMap::new(),
        })
    }

    /// The stop pieces of a blocker: its blocked set (if it has a selected set) and its extra pieces.
    pub fn stop_refs(&self, e: usize) -> Vec<StopRef> {
        let mut v = vec![];
        if self.selected.contains_key(&e) {
            v.push(StopRef::Selected);
        }
        if let Some(x) = self.extra_stops.get(&e) {
            v.extend((0..x.len()).map(StopRef::Extra));
        }
        v
    }

    /// The notes of one source.
    pub fn notes_of(&self, source: usize) -> impl Iterator<Item = &NfNote> {
        self.notes
            .iter()
            .filter(move |n| n.source == source)
            .map(|n| &n.note)
    }
}

/// Options of the normal form.
#[derive(Clone, Debug)]
pub struct NfOptions {
    pub elided: ElidedPolicy,
    pub dedup: bool,
    pub subset: Option<Vec<usize>>,
    /// Split product members of exception branches by their exact linear factors (default false = the checker's form).
    pub factor_exceptions: bool,
}

struct Builder {
    arity: usize,
    lits: Vec<Lit>,
    index: HashMap<(bool, MPoly), u32>,
}

enum Norm {
    Zero,
    NonZeroConst,
    Lit(u32),
}

fn lit_id(i: usize) -> Result<u32, PartitionError> {
    u32::try_from(i).map_err(|_| PartitionError::Unsupported("more than u32::MAX literals".into()))
}

impl Builder {
    fn intern(&mut self, p: &MPoly, is_ge: bool) -> Result<Norm, PartitionError> {
        if let Some(c) = p.as_constant() {
            return Ok(if is_ge {
                if c >= 0 {
                    Norm::Zero
                } else {
                    Norm::NonZeroConst
                }
            } else if c == 0 {
                Norm::Zero
            } else {
                Norm::NonZeroConst
            });
        }
        let q = if is_ge {
            p.positive_primitive()
        } else {
            p.primitive()
        }
        .map_err(PartitionError::Exact)?;
        let key = (is_ge, q.clone());
        if let Some(&i) = self.index.get(&key) {
            return Ok(Norm::Lit(i));
        }
        let id = lit_id(self.lits.len())?;
        let names = slot_names(self.arity, false);
        self.lits.push(Lit {
            core: q.to_poly(self.arity).map_err(PartitionError::Exact)?,
            support: q.support(),
            text: q.render(&names),
            poly: q,
            is_ge,
        });
        self.index.insert(key, id);
        Ok(Norm::Lit(id))
    }

    /// [`Builder::intern`] with an exact-arithmetic failure (a content of 2^127, ...) returned as `Ok(Err(reason))`:
    /// the caller excludes / weakens the domain (re-audit RA2-C1: an extreme literal never aborts the partition).
    fn try_intern(
        &mut self,
        p: &MPoly,
        is_ge: bool,
    ) -> Result<Result<Norm, String>, PartitionError> {
        match self.intern(p, is_ge) {
            Ok(n) => Ok(Ok(n)),
            Err(PartitionError::Exact(e)) => Ok(Err(format!("{e}"))),
            Err(e) => Err(e),
        }
    }

    /// AND-list of zero-set literals -> Some(branch) (possibly empty = always true), None = never true. A member
    /// that cannot be represented is DROPPED ("may vanish anywhere": the branch's zero set only grows); the bool
    /// says that happened.
    fn branch(&mut self, members: &[MPoly]) -> Result<(Option<Vec<u32>>, bool), PartitionError> {
        let mut b = vec![];
        let mut weak = false;
        for m in members {
            match self.try_intern(m, false)? {
                Ok(Norm::Zero) => {}
                Ok(Norm::NonZeroConst) => return Ok((None, weak)),
                Ok(Norm::Lit(i)) => b.push(i),
                Err(_) => weak = true,
            }
        }
        b.sort_unstable();
        b.dedup();
        Ok((Some(b), weak))
    }
}

/// Branches of a pole locus given by its d-coefficients (already in slot variables of the KEY): union over the exact
/// linear factors of the reconstructed polynomial `sum_k c_k d^k` and its cofactor of the AND of their
/// d-coefficients. Returns the AND-lists (d-free).
fn pole_branches(
    coeffs: &[(usize, MPoly)],
    arity: usize,
    nlin: &mut usize,
) -> Result<Vec<Vec<MPoly>>, PartitionError> {
    if coeffs.is_empty() {
        return Ok(vec![vec![]]); // identically zero: always forbidden
    }
    let nv = arity.saturating_add(1); // arity <= 64
    let mut p = MPoly::zero(nv);
    for (k, cm) in coeffs {
        let mut lifted = MPoly::zero(nv);
        let kk = u32::try_from(*k).map_err(|_| PartitionError::Exact(ExactError::Overflow))?;
        for (e, &x) in &cm.terms {
            let mut e2 = e.clone();
            e2.push(kk);
            lifted.terms.insert(e2, x);
        }
        p = p.add(&lifted).map_err(PartitionError::Exact)?;
    }
    let (fs, rest) = match linear_factors(&p) {
        Ok(x) => x,
        Err(ExactError::Overflow) | Err(ExactError::TooLarge(_)) => (vec![], p.clone()),
    };
    *nlin = nlin.saturating_add(fs.len());
    let mut out = vec![];
    for f in fs.iter().chain(std::iter::once(&rest)) {
        if f.as_constant().is_some_and(|c| c != 0) {
            continue;
        }
        // d-coefficients of f
        let cs = f.coeffs_in(arity);
        let mut and = vec![];
        let mut empty = false;
        for c in cs {
            if c.is_zero() {
                continue;
            }
            let mut d = MPoly::zero(arity);
            for (e, &x) in &c.terms {
                d.terms.insert(e[..arity].to_vec(), x);
            }
            if d.as_constant().is_some_and(|x| x != 0) {
                empty = true;
                break;
            }
            and.push(d);
        }
        if !empty {
            out.push(and);
        }
    }
    Ok(out)
}

fn split_products(members: &[MPoly]) -> Result<Vec<Vec<MPoly>>, PartitionError> {
    // AND(f g, h) = AND(f, h) OR AND(g, h)
    let mut acc: Vec<Vec<MPoly>> = vec![vec![]];
    for m in members {
        let (fs, rest) = linear_factors(m).unwrap_or((vec![], m.clone()));
        let mut parts: Vec<MPoly> = fs;
        if rest.as_constant().is_none() {
            parts.push(rest);
        }
        if parts.is_empty() {
            parts.push(m.clone());
        }
        let mut next = vec![];
        for a in &acc {
            for p in &parts {
                let mut b = a.clone();
                b.push(p.clone());
                next.push(b);
            }
        }
        acc = next;
        if acc.len() > 64 {
            return Ok(vec![members.to_vec()]);
        }
    }
    Ok(acc)
}

/// The affine rule-point map of a target pattern, in the KEY's slot variables (the engine's `find_rule`):
/// `pt[a] = n_j - o` for `n<a>+o` at slot j, the fixed value of slot a (else 0) for a parameter no slot maps. Also
/// returns the target constants (slot, value) and the rule-point RANGE bounds (target slot j, GE literal in the key
/// that holds where `n_j - o` fits `i32`): the dispatcher computes `pt[a]` in `i32` right after the fixed slots and
/// the constants, BEFORE the equations, and stops (Undecided) where `n_j - o` leaves it (audit-part3 M1).
#[allow(clippy::type_complexity)]
fn pattern_map(
    t: &[TargetSlot],
    fixed: &[Option<i32>],
    arity: usize,
) -> Result<(Vec<Affine>, Vec<(usize, i64)>, Vec<(usize, MPoly)>), String> {
    if t.len() != arity || fixed.len() != arity {
        return Err(format!(
            "target pattern of length {} for arity {arity}",
            t.len()
        ));
    }
    let mut map: Vec<Affine> = (0..arity)
        .map(|a| Affine::constant(arity, i128::from(fixed[a].unwrap_or(0))))
        .collect();
    let mut consts = vec![];
    let mut range = vec![];
    let mut used = vec![false; arity];
    for (j, s) in t.iter().enumerate() {
        match *s {
            TargetSlot::Const(c) => consts.push((j, c)),
            TargetSlot::Param { param, offset } => {
                if param >= arity || used[param] {
                    return Err(format!(
                        "target parameter n{param} out of range or repeated"
                    ));
                }
                used[param] = true;
                // i64 -> i128: the negation and the sums below never overflow i128
                let o = i128::from(offset);
                map[param] = Affine::unit(arity, j, 1, o.saturating_neg());
                let nj = MPoly::var(arity, j);
                let bound = if o > 0 {
                    // n_j - o >= i32::MIN
                    nj.sub(&MPoly::constant(
                        arity,
                        o.saturating_add(i128::from(i32::MIN)),
                    ))
                } else if o < 0 {
                    // i32::MAX - (n_j - o) >= 0
                    MPoly::constant(arity, i128::from(i32::MAX).saturating_add(o)).sub(&nj)
                } else {
                    continue;
                };
                range.push((j, bound.map_err(|e| format!("{e}"))?));
            }
        }
    }
    Ok((map, consts, range))
}

/// The product of the engine's two zero-test primes (`ENGINE_PRIMES`): a value is "zero" for the engine iff it vanishes
/// modulo both, so an exact value `v` with `0 < |v| < ENGINE_PRIME_PRODUCT` is never read as zero.
pub fn engine_prime_product() -> u128 {
    u128::from(ENGINE_PRIMES[0]).saturating_mul(u128::from(ENGINE_PRIMES[1]))
}

/// Largest `|f(pt)|` over rule points with `|pt[v]| <= m[v]`: `sum |c| prod m[v]^e`, checked (`None` on overflow).
fn max_abs_value(f: &Poly, m: &[u128]) -> Option<u128> {
    let mut acc: u128 = 0;
    for (mono, c) in f.terms() {
        let mut t = c.unsigned_abs();
        for &(v, e) in mono {
            if v == D_VAR {
                return None; // d-free literals only (pole coefficients are d-free)
            }
            let mv = *m.get(usize::from(v))?;
            t = t.checked_mul(mv.checked_pow(e)?)?;
        }
        acc = acc.checked_add(t)?;
    }
    Some(acc)
}

/// Can the ENGINE's modular zero test (a value is zero iff it vanishes modulo both `ENGINE_PRIMES`) disagree with the
/// exact one on a literal of this record at a rule point that the dispatcher or the engine evaluates (audit-part3 C2)?
/// The dispatcher stops there (`Verdict::Error(EngineZeroTest)`, Undecided) for equations and exception members; the
/// engine may read a pole coefficient as zero. Checked literals: equations, exception members, pole coefficients
/// (bounds and recenter bounds are sign tests). Rule points: a canonical target evaluates at the key (`|n| <= 2^31`
/// on the chart); a target pattern at `pt[a] = n_j - o` (the dispatcher only inside `i32`, the engine in `i64`:
/// `|pt[a]| <= 2^31 + |o|`; an offset with `|o| >= 2^62` may leave `i64`: risk), unmapped parameters at their fixed
/// value or 0. A literal whose bound `sum |c| prod |pt|^e` stays below the prime product can only be read as zero when
/// it IS zero. `Some(reason)` = the disagreement cannot be excluded (a dispatch stop is then modelled; never a guess).
pub fn modular_zero_risk(d: &CaseDomain) -> Option<String> {
    let n = d.arity;
    let base: u128 = 1u128 << 31;
    let m: Vec<u128> = match &d.target {
        TargetMap::Canonical => vec![base; n],
        TargetMap::Pattern(t) => {
            let mut m: Vec<u128> = (0..n)
                .map(|a| {
                    u128::from(
                        d.fixed
                            .get(a)
                            .copied()
                            .flatten()
                            .unwrap_or(0)
                            .unsigned_abs(),
                    )
                })
                .collect();
            for s in t {
                if let TargetSlot::Param { param, offset } = *s {
                    let o = u128::from(offset.unsigned_abs());
                    if o >= 1u128 << 62 {
                        return Some(format!(
                            "target offset {offset}: the engine's i64 rule point may overflow"
                        ));
                    }
                    if let Some(x) = m.get_mut(param) {
                        *x = base.saturating_add(o);
                    }
                }
            }
            m
        }
    };
    let p = engine_prime_product();
    let lits = d
        .equations
        .iter()
        .map(|f| ("equation", f))
        .chain(
            d.exceptions
                .iter()
                .flatten()
                .map(|f| ("exception member", f)),
        )
        .chain(
            d.poles
                .iter()
                .flat_map(|pl| pl.coeffs.iter())
                .map(|f| ("pole coefficient", f)),
        );
    for (what, f) in lits {
        match max_abs_value(f, &m) {
            Some(b) if b < p => {}
            Some(_) => {
                return Some(format!(
                    "{what} {f} can reach a nonzero multiple of the engine prime product"
                ));
            }
            None => return Some(format!("{what} {f}: value bound overflows u128")),
        }
    }
    None
}

/// Build the normal form of the selected domains of `src`.
pub fn build(
    src: &dyn DomainSource,
    frame: &NodeFrame,
    opts: &NfOptions,
) -> Result<NfRules, PartitionError> {
    let arity = frame.arity();
    if src.arity() != arity {
        return Err(PartitionError::Arity {
            expected: arity,
            got: src.arity(),
        });
    }
    let lines: Vec<bool> = (0..arity).map(|i| frame.is_line(i)).collect();
    let mut b = Builder {
        arity,
        lits: vec![],
        index: HashMap::new(),
    };
    let mut stats = NfStats::default();
    let mut rules: Vec<NfRule> = vec![];
    let mut sources: Vec<Vec<usize>> = vec![];
    let mut seen: HashMap<NfRule, usize> = HashMap::new();
    let mut elided: Vec<usize> = vec![];
    let mut blockers: Vec<usize> = vec![];
    let mut notes: Vec<SourceNote> = vec![];
    let mut selected: BTreeMap<usize, NfRule> = BTreeMap::new();
    let mut extra_stops: BTreeMap<usize, Vec<StopPiece>> = BTreeMap::new();
    type PoleKey = (Vec<Poly>, Option<Vec<Affine>>);
    let mut pole_cache: HashMap<PoleKey, (Vec<Vec<MPoly>>, bool)> = HashMap::new();
    let idx: Vec<usize> = match &opts.subset {
        Some(s) => s.clone(),
        None => (0..src.len()).collect(),
    };
    let in_chart = |s: usize, v: i64| (lines[s] && v >= 1) || (!lines[s] && v <= 0);
    'dom: for i in idx {
        stats.domains = stats.domains.saturating_add(1);
        let d: &CaseDomain = src.domain(i);
        if d.arity != arity {
            return Err(PartitionError::Arity {
                expected: arity,
                got: d.arity,
            });
        }
        if let Some(c) = &d.chart
            && c.as_slice() != lines.as_slice()
        {
            return Err(PartitionError::ChartNotFrame { source: i });
        }
        let note =
            |notes: &mut Vec<SourceNote>, n: NfNote| notes.push(SourceNote { source: i, note: n });
        // witness status not Complete: elided loci in the domain, or the dispatcher's record-level status (Elided /
        // NotCarried without attestation; corefix2): selected but never usable
        let witness_blocked = d.witness_elided || src.witness_block(i).is_some();
        let elided_here = witness_blocked && opts.elided != ElidedPolicy::Ordinary;
        if witness_blocked {
            if elided_here {
                stats.dropped_elided = stats.dropped_elided.saturating_add(1);
                note(&mut notes, NfNote::ElidedRemoved);
            } else {
                note(&mut notes, NfNote::ElidedCredited);
            }
        }
        if elided_here && opts.elided == ElidedPolicy::Absent {
            continue;
        }
        // the rule point as an affine map of the key (target pattern), its constants and range bounds
        let (map, consts, range) = match &d.target {
            TargetMap::Canonical => (None, vec![], vec![]),
            TargetMap::Pattern(t) => match pattern_map(t, &d.fixed, arity) {
                Ok((m, c, r)) => {
                    stats.pattern_targets = stats.pattern_targets.saturating_add(1);
                    (Some(m), c, r)
                }
                Err(e) => {
                    stats.excluded_unrepresentable =
                        stats.excluded_unrepresentable.saturating_add(1);
                    note(&mut notes, NfNote::Excluded(format!("target pattern: {e}")));
                    continue;
                }
            },
        };
        let lit_of = |p: &Poly| -> Result<MPoly, String> {
            let m = MPoly::from_poly(p, arity, false).map_err(|e| format!("{p}: {e}"))?;
            match &map {
                None => Ok(m),
                Some(mm) => m
                    .compose(mm, arity)
                    .map_err(|e| format!("{p} at the rule point: {e}")),
            }
        };
        let mut weakened: Vec<String> = vec![];
        // SELECTED set: fixed slots and target constants (on the key), equations, bounds, exceptions
        let mut sel = NfRule {
            eq: vec![],
            ge: vec![],
            bad: vec![],
        };
        let fixed_iter = d
            .fixed
            .iter()
            .enumerate()
            .filter_map(|(s, f)| f.map(|v| (s, i64::from(v))))
            .chain(consts.iter().copied());
        for (s, v) in fixed_iter {
            if !in_chart(s, v) {
                stats.dropped_out_of_chart = stats.dropped_out_of_chart.saturating_add(1);
                note(
                    &mut notes,
                    NfNote::DroppedEmpty(format!(
                        "fixed value / target constant n{s} = {v} outside the chart"
                    )),
                );
                continue 'dom;
            }
            let p = MPoly::var(arity, s)
                .sub(&MPoly::constant(arity, i128::from(v)))
                .map_err(PartitionError::Exact)?;
            match b.intern(&p, false)? {
                Norm::Lit(l) => sel.eq.push(l),
                _ => {
                    return Err(PartitionError::Unsupported(
                        "n_s - v interned as a constant".into(),
                    ));
                }
            }
        }
        // STOP pieces the dispatcher evaluates BEFORE the equations (audit-part3 M1 / C2), registered now: a record
        // dropped below as never applying still STOPS dispatch there. (A fixed value / target constant outside the
        // chart was handled above: such a record is never reached on a chart key.)
        sel.eq.sort_unstable();
        sel.eq.dedup();
        let fc = NfRule {
            eq: sel.eq.clone(),
            ge: vec![],
            bad: vec![],
        };
        let mut extra: Vec<StopPiece> = vec![];
        for (j, ok) in &range {
            // violated: -ok - 1 >= 0 (n_j - o outside i32)
            let viol = ok
                .scale(-1)
                .and_then(|p| p.sub(&MPoly::constant(arity, 1)));
            match viol.map_err(|e| format!("{e}")).map(|v| b.try_intern(&v, true)) {
                Ok(Ok(Ok(Norm::Lit(l)))) => {
                    let mut r = fc.clone();
                    r.ge.push(l);
                    extra.push(StopPiece {
                        rule: r,
                        why: StopWhy::RulePointOverflow { slot: *j },
                    });
                }
                Ok(Ok(Ok(Norm::NonZeroConst))) => {} // never violated
                Ok(Ok(Ok(Norm::Zero))) => extra.push(StopPiece {
                    rule: fc.clone(),
                    why: StopWhy::RulePointOverflow { slot: *j },
                }),
                Ok(Err(e)) => return Err(e),
                Ok(Ok(Err(_))) | Err(_) => {
                    // not representable: over-approximated by the whole fixed / constant set (a larger stop set is
                    // sound for a no-gap proof: it can only add gaps)
                    stats.stop_pieces_over_approximated =
                        stats.stop_pieces_over_approximated.saturating_add(1);
                    extra.push(StopPiece {
                        rule: fc.clone(),
                        why: StopWhy::RulePointOverflow { slot: *j },
                    });
                }
            }
        }
        if let Some(why) = modular_zero_risk(d) {
            note(&mut notes, NfNote::ModularZeroRisk(why.clone()));
            extra.push(StopPiece {
                rule: fc.clone(),
                why: StopWhy::ModularZeroRisk(why),
            });
        }
        let pre_blocker = !extra.is_empty();
        if pre_blocker {
            stats.extra_stop_pieces = stats.extra_stop_pieces.saturating_add(extra.len());
            extra_stops.insert(i, extra);
            blockers.push(i);
        }
        for e in &d.equations {
            let q = match lit_of(e) {
                Ok(q) => q,
                Err(why) => {
                    stats.excluded_unrepresentable =
                        stats.excluded_unrepresentable.saturating_add(1);
                    note(&mut notes, NfNote::Excluded(format!("equation {why}")));
                    continue 'dom;
                }
            };
            match b.try_intern(&q, false)? {
                Ok(Norm::Zero) => {}
                Ok(Norm::NonZeroConst) => {
                    stats.dropped_dead_constant = stats.dropped_dead_constant.saturating_add(1);
                    note(
                        &mut notes,
                        NfNote::DroppedEmpty(format!("equation {e} is a nonzero constant")),
                    );
                    continue 'dom;
                }
                Ok(Norm::Lit(l)) => sel.eq.push(l),
                Err(why) => {
                    stats.excluded_unrepresentable =
                        stats.excluded_unrepresentable.saturating_add(1);
                    note(&mut notes, NfNote::Excluded(format!("equation {e}: {why}")));
                    continue 'dom;
                }
            }
        }
        for g in &d.bounds {
            let q = match lit_of(g) {
                Ok(q) => q,
                Err(why) => {
                    stats.excluded_unrepresentable =
                        stats.excluded_unrepresentable.saturating_add(1);
                    note(&mut notes, NfNote::Excluded(format!("bound {why}")));
                    continue 'dom;
                }
            };
            match b.try_intern(&q, true)? {
                Ok(Norm::Zero) => {}
                Ok(Norm::NonZeroConst) => {
                    stats.dropped_dead_constant = stats.dropped_dead_constant.saturating_add(1);
                    note(
                        &mut notes,
                        NfNote::DroppedEmpty(format!("bound {g} is a negative constant")),
                    );
                    continue 'dom;
                }
                Ok(Norm::Lit(l)) => sel.ge.push(l),
                Err(why) => {
                    stats.excluded_unrepresentable =
                        stats.excluded_unrepresentable.saturating_add(1);
                    note(&mut notes, NfNote::Excluded(format!("bound {g}: {why}")));
                    continue 'dom;
                }
            }
        }
        let mut branches: Vec<Vec<MPoly>> = vec![];
        for br in &d.exceptions {
            let mut ms: Vec<MPoly> = vec![];
            for p in br {
                match lit_of(p) {
                    Ok(q) => ms.push(q),
                    // "may vanish anywhere": dropping a member ENLARGES the branch's zero set (conservative)
                    Err(why) => weakened.push(format!(
                        "exception member {why} treated as vanishing everywhere"
                    )),
                }
            }
            if opts.factor_exceptions {
                branches.extend(split_products(&ms)?);
            } else {
                branches.push(ms);
            }
        }
        for br in branches {
            let (got, weak) = b.branch(&br)?;
            if weak {
                weakened.push(
                    "an exception member treated as vanishing everywhere (not representable)"
                        .into(),
                );
            }
            match got {
                None => {}
                Some(v) if v.is_empty() => {
                    if weakened.is_empty() {
                        stats.dropped_dead_constant = stats.dropped_dead_constant.saturating_add(1);
                        note(
                            &mut notes,
                            NfNote::DroppedEmpty("an exception branch vanishes everywhere".into()),
                        );
                    } else {
                        stats.weakened = stats.weakened.saturating_add(1);
                        note(
                            &mut notes,
                            NfNote::Weakened(format!(
                                "{}; the domain then credits nothing",
                                weakened.join("; ")
                            )),
                        );
                    }
                    continue 'dom;
                }
                Some(v) => sel.bad.push(v),
            }
        }
        // BLOCKERS (selected but not usable): recenter bounds, rule-point range bounds, pole loci, elision
        let mut blk_ge: Vec<u32> = vec![];
        let mut blk_bad: Vec<Vec<u32>> = vec![];
        let mut never_applies: Option<String> = None;
        let mut ge_blockers: Vec<(String, MPoly)> = vec![];
        for g in &d.recenter_bounds {
            match lit_of(g) {
                Ok(q) => ge_blockers.push((format!("recenter bound {g}"), q)),
                Err(why) => {
                    // a blocker that cannot be represented: it may block anywhere (conservative for coverage)
                    weakened.push(format!(
                        "recenter bound {why} treated as violated everywhere"
                    ));
                    never_applies = Some("unrepresentable recenter bound".into());
                }
            }
        }
        for (_, q) in &range {
            ge_blockers.push(("rule-point range bound".into(), q.clone()));
        }
        for (what, q) in ge_blockers {
            match b.try_intern(&q, true)? {
                Ok(Norm::Zero) => {}
                Ok(Norm::NonZeroConst) => never_applies = Some(format!("{what} is never met")),
                Ok(Norm::Lit(l)) => blk_ge.push(l),
                Err(why) => {
                    // a blocker that cannot be represented may block anywhere (conservative for coverage)
                    weakened.push(format!(
                        "{what} not representable ({why}): treated as violated everywhere"
                    ));
                    never_applies = Some(format!("unrepresentable {what}"));
                }
            }
        }
        for pl in &d.poles {
            let key: PoleKey = (pl.coeffs.clone(), map.clone());
            let (pb, weak) = match pole_cache.get(&key) {
                Some(x) => x.clone(),
                None => {
                    let mut cs: Vec<(usize, MPoly)> = vec![];
                    let mut weak = false;
                    for (k, c) in pl.coeffs.iter().enumerate() {
                        match lit_of(c) {
                            Ok(q) => cs.push((k, q)),
                            // dropping a coefficient from the AND enlarges the pole's zero set (conservative)
                            Err(_) => weak = true,
                        }
                    }
                    let x = match pole_branches(&cs, arity, &mut stats.pole_linear_factors) {
                        Ok(pb) => (pb, weak),
                        // not representable: the pole may vanish anywhere (an always-vanishing branch: blocks)
                        Err(PartitionError::Exact(_)) => (vec![vec![]], true),
                        Err(e) => return Err(e),
                    };
                    pole_cache.insert(key, x.clone());
                    x
                }
            };
            if weak {
                weakened.push("a pole coefficient treated as vanishing everywhere".into());
            }
            stats.pole_branches = stats.pole_branches.saturating_add(pb.len());
            for br in pb {
                let (got, weak) = b.branch(&br)?;
                if weak {
                    weakened.push("a pole coefficient treated as vanishing everywhere".into());
                }
                match got {
                    None => {}
                    Some(v) if v.is_empty() => {
                        never_applies = Some("a pole locus vanishes everywhere".into());
                    }
                    Some(v) => blk_bad.push(v),
                }
            }
        }
        if elided_here {
            never_applies = Some(match src.witness_block(i) {
                Some(v) => format!("witness status not Complete ({v:?})"),
                None => "elided witness loci".into(),
            });
        }
        if !weakened.is_empty() {
            stats.weakened = stats.weakened.saturating_add(1);
            note(&mut notes, NfNote::Weakened(weakened.join("; ")));
        }
        for v in [&mut sel.eq, &mut sel.ge, &mut blk_ge] {
            v.sort_unstable();
            v.dedup();
        }
        sel.bad.sort();
        sel.bad.dedup();
        let has_blocker = never_applies.is_some() || !blk_ge.is_empty() || !blk_bad.is_empty();
        if has_blocker && !pre_blocker {
            blockers.push(i);
        }
        // APPLIES set = selected set minus the blockers
        let applies = if never_applies.is_some() {
            None
        } else {
            let mut r = sel.clone();
            r.ge.extend(blk_ge.iter().copied());
            r.ge.sort_unstable();
            r.ge.dedup();
            r.bad.extend(blk_bad.iter().cloned());
            r.bad.sort();
            r.bad.dedup();
            Some(r)
        };
        if has_blocker {
            selected.insert(i, sel.clone());
        }
        if elided_here {
            // FailClosed: the record credits nothing; its BLOCKED region is a separate partition
            elided.push(i);
            continue;
        }
        let Some(r) = applies else {
            // selected somewhere but never applicable: credits nothing (in dispatch mode it still BLOCKS)
            note(
                &mut notes,
                NfNote::DroppedEmpty(format!(
                    "never applies: {}",
                    never_applies.clone().unwrap_or_default()
                )),
            );
            stats.dropped_dead_constant = stats.dropped_dead_constant.saturating_add(1);
            continue;
        };
        if opts.dedup {
            if let Some(&k) = seen.get(&r) {
                sources[k].push(i);
                continue;
            }
            seen.insert(r.clone(), rules.len());
        }
        rules.push(r);
        sources.push(vec![i]);
    }
    stats.distinct = rules.len();
    stats.literals = b.lits.len();
    let quick = rules
        .iter()
        .map(|r| {
            r.eq.iter()
                .filter_map(|&l| unit_fixed(&b.lits[l as usize]))
                .collect()
        })
        .collect();
    Ok(NfRules {
        arity,
        lines,
        lits: b.lits,
        rules,
        sources,
        stats,
        quick,
        elided,
        blockers,
        notes,
        selected,
        extra_stops,
    })
}

/// `v \ h` as a union of normal-form rules: v with `l != 0` for each equation l of h (a BAD branch [l]), with
/// `g <= -1` for each bound g of h, and with every member of each exception branch of h as equations. Disjoint
/// fixed slots: v unchanged. The bool is true when a complement `-g - 1 >= 0` could not be represented (a
/// coefficient `i128::MIN`): that variant is left out, so the union is a SUBSET of `v \ h` (the caller then
/// over-approximates what it subtracts from; never silently).
fn subtract(
    b: &mut Builder,
    v: &NfRule,
    h: &NfRule,
) -> Result<(Vec<NfRule>, bool), PartitionError> {
    let fv: Vec<(usize, i64)> =
        v.eq.iter()
            .filter_map(|&l| unit_fixed(&b.lits[l as usize]))
            .collect();
    for &l in &h.eq {
        if let Some((s, x)) = unit_fixed(&b.lits[l as usize])
            && fv.iter().any(|&(t, y)| t == s && y != x)
        {
            return Ok((vec![v.clone()], false));
        }
    }
    let mut out = vec![];
    let mut inexact = false;
    for &l in &h.eq {
        if v.eq.contains(&l) {
            continue; // l = 0 on all of v: this variant is empty
        }
        let mut w = v.clone();
        w.bad.push(vec![l]);
        w.bad.sort();
        w.bad.dedup();
        out.push(w);
    }
    for &g in &h.ge {
        let neg = b.lits[g as usize]
            .poly
            .scale(-1)
            .and_then(|p| p.sub(&MPoly::constant(p.nvars, 1)));
        let Ok(neg) = neg else {
            inexact = true;
            continue;
        };
        match b.try_intern(&neg, true)? {
            Ok(Norm::Zero) => out.push(v.clone()),
            Ok(Norm::NonZeroConst) => {}
            Ok(Norm::Lit(n)) => {
                let mut w = v.clone();
                w.ge.push(n);
                w.ge.sort_unstable();
                w.ge.dedup();
                out.push(w);
            }
            Err(_) => inexact = true,
        }
    }
    for br in &h.bad {
        let mut w = v.clone();
        w.eq.extend(br.iter().copied());
        w.eq.sort_unstable();
        w.eq.dedup();
        out.push(w);
    }
    Ok((out, inexact))
}

/// `n_s - v` (or `v - n_s`) -> (s, v); `None` when v does not fit `i64` (re-audit RA2-m3: never an `as` cast; the
/// fast pre-check then simply does not apply, the literal itself is still evaluated exactly).
fn unit_fixed(l: &Lit) -> Option<(usize, i64)> {
    if l.support.len() != 1 {
        return None;
    }
    let (coef, c) = l.poly.linear_form()?;
    let s = l.support[0];
    match coef[s] {
        1 => i64::try_from(c.checked_neg()?).ok().map(|v| (s, v)),
        -1 => i64::try_from(c).ok().map(|v| (s, v)),
        _ => None,
    }
}

impl NfRules {
    /// Exact value of literal `l` at a key (big-integer fallback).
    pub fn lit_sign(&self, l: u32, key: &[i32]) -> Result<i32, PartitionError> {
        self.lits[l as usize]
            .core
            .eval(key)
            .map(|v| v.signum())
            .map_err(|e| PartitionError::Eval(format!("{e}")))
    }

    /// Union coverage of a chart key by rule `r` (exact).
    pub fn rule_covers(&self, r: usize, key: &[i32]) -> Result<bool, PartitionError> {
        for &(s, v) in &self.quick[r] {
            if i64::from(key[s]) != v {
                return Ok(false);
            }
        }
        self.nf_rule_covers(&self.rules[r], key)
    }

    /// Does a normal-form rule (whose literals are interned here, e.g. a blocker's SELECTED set) hold at the key?
    /// Exact (big-integer fallback).
    pub fn nf_rule_covers(&self, rule: &NfRule, key: &[i32]) -> Result<bool, PartitionError> {
        for &l in &rule.eq {
            if self.lit_sign(l, key)? != 0 {
                return Ok(false);
            }
        }
        for &l in &rule.ge {
            if self.lit_sign(l, key)? < 0 {
                return Ok(false);
            }
        }
        'b: for br in &rule.bad {
            for &l in br {
                if self.lit_sign(l, key)? != 0 {
                    continue 'b;
                }
            }
            return Ok(false);
        }
        Ok(true)
    }

    /// Is the chart key covered by some rule (union semantics)?
    pub fn covered(&self, key: &[i32]) -> Result<bool, PartitionError> {
        for r in 0..self.rules.len() {
            if self.rule_covers(r, key)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn floor(&self, slot: usize) -> i64 {
        if self.lines[slot] { 1 } else { 0 }
    }
}
