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
    /// (an equation or exception member can take a nonzero value divisible by both engine primes: [`modular_risk`];
    /// audit-part3 C2). Dispatch mode: the record STOPS dispatch (Undecided) at the exact rule points of
    /// one-variable linear literals, else on its whole fixed-slot / target-constant set (a stop piece that
    /// over-approximates the true one: sound for a no-gap proof). Union mode: a caveat.
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
    /// Per literal: first interned while building a stop piece. Such a literal is a state boundary only where a rule
    /// of the run refers to it (`classes::slot_states`): the main partition's states are those of its rules, the
    /// blocked sub-partitions of a piece refer to its literals through their complement rules.
    pub stop_lits: Vec<bool>,
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
        let nlits = b.lits.len();
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
            stop_lits: {
                let mut v = self.stop_lits.clone();
                v.resize(nlits, false);
                v
            },
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

/// Where the engine's modular zero test can disagree with the exact one on a record's literals ([`modular_risk`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModularRisk {
    /// Only at rule points whose variable `.0` equals one of the values `.1` (one-variable linear literals: the
    /// residue class is solved exactly); the record stops dispatch there (an exact superset of its undecided keys).
    Points(Vec<(usize, i128)>),
    /// Only at rule points with `|pt[v]| >= r` for some listed `(v, r)` (a value bound: below that radius in every
    /// variable the literal's primitive part stays below the prime modulus, so it vanishes modulo it only where it is
    /// 0); the record stops dispatch on `fixed & constants & (pt_v >= r or pt_v <= -r)` (an over-approximation confined
    /// to far keys).
    Far(Vec<(usize, i128)>),
    /// Possibly anywhere on the record's fixed-slot / target-constant set (an over-approximation), with the reason.
    Anywhere(String),
}

/// The risk of one literal ([`literal_modular_risk`]).
enum LitRisk {
    Points(Vec<(usize, i128)>),
    Far(Vec<(usize, i128)>),
}

/// `a * b mod m` for `a, b < m < 2^126` (double-and-add, every step checked; `None` never happens for such inputs).
fn mulmod(a: u128, b: u128, m: u128) -> Option<u128> {
    let mut a = a.checked_rem(m)?;
    let mut b = b;
    let mut r: u128 = 0;
    while b > 0 {
        if b & 1 == 1 {
            r = r.checked_add(a)?.checked_rem(m)?;
        }
        a = a.checked_add(a)?.checked_rem(m)?;
        b = b.checked_shr(1).unwrap_or(0);
    }
    Some(r)
}

/// `sum |c| prod m[v]^e` of a polynomial in the slot variables (`None` on overflow).
fn mpoly_bound(q: &MPoly, m: &[u128]) -> Option<u128> {
    let mut acc: u128 = 0;
    for (e, c) in &q.terms {
        let mut t = c.unsigned_abs();
        for (v, &x) in e.iter().enumerate() {
            if x > 0 {
                t = t.checked_mul(m.get(v)?.checked_pow(x)?)?;
            }
        }
        acc = acc.checked_add(t)?;
    }
    Some(acc)
}

/// The factor criterion of [`literal_modular_risk`]: can every prime of `pm` (each of `p1`, `p2` not dividing the
/// content) divide the primitive part of `f` at a reachable rule point without the value being 0? `true` when the
/// factorisation is unavailable (conservative).
fn factor_can_vanish(f: &Poly, m: &[u128], pm: u128, p1: u128, p2: u128) -> bool {
    let Ok(q) = MPoly::from_poly(f, m.len(), false).and_then(|x| x.primitive()) else {
        return true;
    };
    let (fs, rest) = linear_factors(&q).unwrap_or_else(|_| (vec![], q.clone()));
    let reach = |p: u128| -> bool {
        fs.iter()
            .chain(std::iter::once(&rest))
            .filter(|x| x.terms.len() >= 2)
            .any(|x| mpoly_bound(x, m).is_none_or(|b| b >= p))
    };
    [p1, p2]
        .into_iter()
        .filter(|&p| pm.checked_rem(p) == Some(0))
        .all(reach)
}

/// The risk of ONE literal `f` at rule points with `|pt[v]| <= m[v]` (every `m[v]` below both primes). With
/// `f = g q` (`g` the content), `f(pt)` is a nonzero multiple of both primes iff `q(pt) != 0` and `q(pt)` vanishes
/// modulo `pm` = the product of the primes NOT dividing `g`. So: `pm = 1` (both divide `g`): anywhere; `q` one
/// monomial: never (a monomial vanishes modulo a prime larger than every |pt| only where it is 0); `sum |c_q| prod
/// m^e < pm`: never; `q = a x + b` in one variable: exactly at the residue `x = -b / a (mod pm)` inside the box;
/// otherwise anywhere.
fn literal_modular_risk(f: &Poly, m: &[u128]) -> Result<LitRisk, String> {
    literal_modular_risk_with(f, m, true)
}

/// [`literal_modular_risk`] with the FACTOR criterion on (`factor = true`, the dispatcher's sharper test) or off (the
/// plain VALUE BOUND `sum |c_q| prod m^e < pm`, the criterion an independent verifier states; never smaller radii).
fn literal_modular_risk_with(f: &Poly, m: &[u128], factor: bool) -> Result<LitRisk, String> {
    let p1 = u128::from(ENGINE_PRIMES[0]);
    let p2 = u128::from(ENGINE_PRIMES[1]);
    // a coefficient beyond i128 (exact parser: big literal): the normal form cannot represent it either (the record
    // is excluded / weakened: a caveat); here: risk anywhere (conservative)
    let terms: &[(crate::poly::Mono, i128)] = f
        .small_terms()
        .ok_or_else(|| format!("literal {f}: a coefficient beyond i128"))?;
    if terms
        .iter()
        .any(|(mo, _)| mo.iter().any(|&(v, _)| v == D_VAR))
    {
        // a literal with d is not a valid case literal: the domain does not validate and the normal form excludes /
        // weakens it (a caveat in dispatch mode, a refusal of the minting preconditions)
        return Ok(LitRisk::Points(vec![]));
    }
    let mut g: u128 = 0;
    for (_, c) in terms {
        let mut a = c.unsigned_abs();
        let mut b = g;
        while b != 0 {
            let r = a.checked_rem(b).unwrap_or(0);
            a = b;
            b = r;
        }
        g = a;
    }
    if g == 0 {
        return Ok(LitRisk::Points(vec![])); // the zero polynomial: exactly zero everywhere
    }
    let mut pm: u128 = 1;
    for p in [p1, p2] {
        if g.checked_rem(p) != Some(0) {
            pm = pm.saturating_mul(p);
        }
    }
    if pm == 1 {
        return Err(format!(
            "literal {f}: its content is divisible by both engine primes"
        ));
    }
    let gi = i128::try_from(g).map_err(|_| format!("literal {f}: content 2^127"))?;
    let q: Vec<(&crate::poly::Mono, i128)> = terms
        .iter()
        .map(|(mo, c)| c.checked_div(gi).map(|x| (mo, x)))
        .collect::<Option<_>>()
        .ok_or_else(|| format!("literal {f}: content division"))?;
    if q.len() <= 1 {
        return Ok(LitRisk::Points(vec![]));
    }
    // sum |c_q| prod m^e (saturating: an overflow only means "at least the modulus")
    let mut bound: u128 = 0;
    for (mo, c) in &q {
        let mut t = c.unsigned_abs();
        for &(v, e) in mo.iter() {
            let mv = *m
                .get(usize::from(v))
                .ok_or_else(|| format!("literal {f}: variable beyond the arity"))?;
            t = mv
                .checked_pow(e)
                .and_then(|x| t.checked_mul(x))
                .unwrap_or(u128::MAX);
        }
        bound = bound.saturating_add(t);
    }
    if bound < pm {
        return Ok(LitRisk::Points(vec![]));
    }
    // FACTOR criterion (also necessary): with q = prod l_i * r (exact linear factors l_i, cofactor r), a prime p
    // divides q(pt) iff it divides some l_i(pt) or r(pt); a factor whose value bound stays below p (or a one-monomial
    // cofactor) is divisible by p only where it is 0, i.e. where q(pt) = 0. So each prime of pm must be reachable by a
    // "large" factor; products of small linear factors (high-degree exception members of real tables) never are.
    if factor && !factor_can_vanish(f, m, pm, p1, p2) {
        return Ok(LitRisk::Points(vec![]));
    }
    // one-variable linear primitive part a x + b: the residue is solved exactly; anything else: the far radius
    let mut var: Option<(usize, i128)> = None;
    let mut b: i128 = 0;
    let mut linear = true;
    for (mo, c) in &q {
        match mo.as_slice() {
            [] => b = *c,
            [(v, 1)] if var.is_none() => var = Some((usize::from(*v), *c)),
            _ => linear = false,
        }
    }
    if !linear {
        // smallest r with sum |c_q| r^deg >= pm: below r in every variable |q(pt)| < pm (no nonzero multiple)
        let at = |r: u128| -> u128 {
            let mut acc: u128 = 0;
            for (mo, c) in &q {
                let deg: u32 = mo.iter().map(|&(_, e)| e).sum();
                let t = r
                    .checked_pow(deg)
                    .and_then(|x| x.checked_mul(c.unsigned_abs()));
                acc = match t.and_then(|t| acc.checked_add(t)) {
                    Some(a) => a,
                    None => return u128::MAX,
                };
            }
            acc
        };
        let (mut lo, mut hi): (u128, u128) = (1, 1u128 << 62);
        if at(hi) < pm {
            return Ok(LitRisk::Points(vec![])); // out of every reachable range
        }
        while lo < hi {
            let mid = lo.saturating_add(hi.saturating_sub(lo).checked_div(2).unwrap_or(0));
            if at(mid) >= pm {
                hi = mid;
            } else {
                lo = mid.saturating_add(1);
            }
        }
        let r = i128::try_from(lo).map_err(|_| format!("literal {f}: radius"))?;
        let mut vars: Vec<usize> = q
            .iter()
            .flat_map(|(mo, _)| mo.iter().map(|&(v, _)| usize::from(v)))
            .collect();
        vars.sort_unstable();
        vars.dedup();
        return Ok(LitRisk::Far(vars.into_iter().map(|v| (v, r)).collect()));
    }
    let Some((v, a)) = var else {
        return Ok(LitRisk::Points(vec![]));
    };
    let pmi = i128::try_from(pm).map_err(|_| format!("literal {f}: modulus"))?;
    let am = a
        .checked_rem_euclid(pmi)
        .ok_or_else(|| format!("literal {f}: residue"))?;
    let (gg, inv, _) = super::exact::ext_gcd(am, pmi).map_err(|e| format!("literal {f}: {e}"))?;
    if gg != 1 {
        return Ok(LitRisk::Points(vec![])); // a prime of pm divides a but not b (gcd(a, b) = 1): never a multiple
    }
    let nb = b
        .checked_neg()
        .and_then(|x| x.checked_rem_euclid(pmi))
        .ok_or_else(|| format!("literal {f}: residue"))?;
    let invm = inv
        .checked_rem_euclid(pmi)
        .ok_or_else(|| format!("literal {f}: residue"))?;
    let r = mulmod(nb.unsigned_abs(), invm.unsigned_abs(), pm)
        .and_then(|x| i128::try_from(x).ok())
        .ok_or_else(|| format!("literal {f}: residue"))?;
    let mv = m.get(v).copied().unwrap_or(0);
    let mut out = vec![];
    for x in [Some(r), r.checked_sub(pmi)].into_iter().flatten() {
        if x.unsigned_abs() <= mv && a.checked_mul(x).and_then(|y| y.checked_add(b)) != Some(0) {
            out.push((v, x));
        }
    }
    Ok(LitRisk::Points(out))
}

/// Can the ENGINE's modular zero test (a value is zero iff it vanishes modulo both `ENGINE_PRIMES`) disagree with the
/// exact one on a literal the dispatcher evaluates with it (equations, exception members: `Verdict::Error(
/// EngineZeroTest)`, Undecided; audit-part3 C2)? Rule points: a canonical target evaluates at the key (`|n| <= 2^31`
/// on the chart); a target pattern at `pt[a] = n_j - o` (`|pt[a]| <= 2^31 + |o|`; an offset with `|o| >= 2^60`
/// could reach the primes: risk anywhere), unmapped parameters at their fixed value or 0. Per literal see
/// [`literal_modular_risk`]. `None` = no disagreement possible. (Pole coefficients are tested exactly by the
/// dispatcher and are not part of the engine's first match; the engine's modular REDUCTION at huge keys is a separate
/// matter, interfaces/partition.md 14.)
pub fn modular_risk(d: &CaseDomain) -> Option<ModularRisk> {
    modular_risk_with(d, true)
}

/// [`modular_risk`] WITHOUT the factor criterion: every literal whose plain value bound reaches the modulus counts
/// (the criterion of an independent radius check, VERIFIER_SPEC item 4); its far radii are never larger than those of
/// [`modular_risk`].
pub fn modular_risk_value_bound(d: &CaseDomain) -> Option<ModularRisk> {
    modular_risk_with(d, false)
}

fn modular_risk_with(d: &CaseDomain, factor: bool) -> Option<ModularRisk> {
    let m = match rule_point_bounds(d) {
        Ok(m) => m,
        Err(why) => return Some(ModularRisk::Anywhere(why)),
    };
    let mut points: Vec<(usize, i128)> = vec![];
    let mut far: Vec<(usize, i128)> = vec![];
    for (_, f) in reached_literals(d) {
        match literal_modular_risk_with(f, &m, factor) {
            Ok(LitRisk::Points(p)) => points.extend(p),
            Ok(LitRisk::Far(p)) => far.extend(p),
            Err(why) => return Some(ModularRisk::Anywhere(why)),
        }
    }
    if !far.is_empty() {
        // one radius per variable (the smallest), points absorbed by the far pieces only if beyond the radius
        far.sort_unstable();
        far.dedup_by(|a, b| a.0 == b.0);
        let pts: Vec<(usize, i128)> = points
            .into_iter()
            .filter(|(v, x)| {
                !far.iter()
                    .any(|(w, r)| w == v && x.unsigned_abs() >= r.unsigned_abs())
            })
            .collect();
        if !pts.is_empty() {
            return Some(ModularRisk::Anywhere(format!(
                "exact residue points {pts:?} together with far radii {far:?}"
            )));
        }
        return Some(ModularRisk::Far(far));
    }
    points.sort_unstable();
    points.dedup();
    (!points.is_empty()).then_some(ModularRisk::Points(points))
}

/// The largest `|pt[a]|` the dispatcher (i32) or the engine (i64: `key - offset`) evaluates a record's literals at:
/// `2^31` on the chart for a canonical target; `2^31 + |o|` for a mapped parameter of a target pattern, the fixed value
/// (else 0) for an unmapped one. `Err` for an offset `>= 2^60` (rule points could reach the engine primes).
fn rule_point_bounds(d: &CaseDomain) -> Result<Vec<u128>, String> {
    let n = d.arity;
    let base: u128 = 1u128 << 31;
    match &d.target {
        TargetMap::Canonical => Ok(vec![base; n]),
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
                    if o >= 1u128 << 60 {
                        return Err(format!(
                            "target offset {offset}: rule points may reach the engine primes"
                        ));
                    }
                    if let Some(x) = m.get_mut(param) {
                        *x = base.saturating_add(o);
                    }
                }
            }
            Ok(m)
        }
    }
}

/// Where a literal sits in the dispatcher's evaluation order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LitPos {
    Equation(usize),
    Member(usize, usize),
}

/// The literals the dispatcher can EVALUATE with its modular zero test, in its order (`at_rule_point`): the
/// equations up to the first nonzero constant one (it deselects everywhere: nothing after it is evaluated), then, if no
/// bound is a negative constant, the exception members branch by branch up to the first nonzero constant member of
/// each branch.
fn reached_literals(d: &CaseDomain) -> Vec<(LitPos, &Poly)> {
    let constant = |p: &Poly| -> Option<i128> {
        let t = p.small_terms()?;
        match t {
            [] => Some(0),
            [(mo, c)] if mo.is_empty() => Some(*c),
            _ => None,
        }
    };
    let mut out = vec![];
    for (j, e) in d.equations.iter().enumerate() {
        out.push((LitPos::Equation(j), e));
        if constant(e).is_some_and(|c| c != 0) {
            return out;
        }
    }
    if d.bounds.iter().any(|g| constant(g).is_some_and(|c| c < 0)) {
        return out;
    }
    for (bi, br) in d.exceptions.iter().enumerate() {
        for (mi, f) in br.iter().enumerate() {
            out.push((LitPos::Member(bi, mi), f));
            if constant(f).is_some_and(|c| c != 0) {
                break;
            }
        }
    }
    out
}

/// [`modular_risk`] as a reason (`Some` for any risk, points or anywhere).
pub fn modular_zero_risk(d: &CaseDomain) -> Option<String> {
    modular_risk(d).map(|r| match r {
        ModularRisk::Points(p) => format!(
            "the engine's modular zero test may differ at the rule points {p:?} (variable, value)"
        ),
        ModularRisk::Far(p) => format!(
            "the engine's modular zero test may differ only far out: some |pt[v]| >= r for (v, r) in {p:?}"
        ),
        ModularRisk::Anywhere(w) => w,
    })
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
    let mut stop_created: Vec<u32> = vec![];
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
        let lits_before = b.lits.len();
        for (j, ok) in &range {
            // violated: -ok - 1 >= 0 (n_j - o outside i32)
            let viol = ok.scale(-1).and_then(|p| p.sub(&MPoly::constant(arity, 1)));
            match viol
                .map_err(|e| format!("{e}"))
                .map(|v| b.try_intern(&v, true))
            {
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
        // modular zero test (audit-part3 C2), per reached literal in the dispatcher's order: the piece is `fc` AND the
        // literals evaluated before it hold (earlier equations zero; for a member: every equation zero, every bound
        // >= 0, the earlier members of its branch zero) AND the literal's risk region (exact residue points, far
        // radius, or everything); a condition that cannot be represented is left out (a larger piece: sound)
        let reached = reached_literals(d);
        let risks: Vec<(LitPos, Result<LitRisk, String>)> = match rule_point_bounds(d) {
            Ok(mb) => reached
                .iter()
                .map(|(pos, f)| (*pos, literal_modular_risk(f, &mb)))
                .collect(),
            Err(why) => reached
                .iter()
                .map(|(pos, _)| (*pos, Err(why.clone())))
                .collect(),
        };
        let mut risk_notes: Vec<String> = vec![];
        for (pos, risk) in risks {
            let region: Vec<(bool, MPoly, String)> = match &risk {
                Ok(LitRisk::Points(p)) if p.is_empty() => continue,
                Ok(LitRisk::Far(p)) if p.is_empty() => continue,
                Ok(LitRisk::Points(p)) => p
                    .iter()
                    .filter_map(|&(v, x)| {
                        MPoly::var(arity, v)
                            .sub(&MPoly::constant(arity, x))
                            .ok()
                            .map(|q| (false, q, format!("rule point n{v} = {x}")))
                    })
                    .collect(),
                Ok(LitRisk::Far(p)) => p
                    .iter()
                    .flat_map(|&(v, r)| {
                        [1i128, -1].into_iter().filter_map(move |sign| {
                            MPoly::var(arity, v)
                                .scale(sign)
                                .and_then(|q| q.sub(&MPoly::constant(arity, r)))
                                .ok()
                                .map(|q| (true, q, format!("|rule point n{v}| >= {r}")))
                        })
                    })
                    .collect(),
                Err(_) => vec![],
            };
            risk_notes.push(match &risk {
                Ok(LitRisk::Points(p)) => format!("{pos:?}: exact rule points {p:?}"),
                Ok(LitRisk::Far(p)) => format!("{pos:?}: far radii {p:?}"),
                Err(w) => format!("{pos:?}: anywhere ({w})"),
            });
            // the prefix conditions
            let mut base = fc.clone();
            let mut possible = true;
            let mut add_cond =
                |p: &Poly, is_ge: bool, base: &mut NfRule| -> Result<(), PartitionError> {
                    match lit_of(p).map(|q| b.try_intern(&q, is_ge)) {
                        Ok(Ok(Ok(Norm::Lit(l)))) => {
                            if is_ge {
                                base.ge.push(l);
                            } else {
                                base.eq.push(l);
                            }
                        }
                        Ok(Ok(Ok(Norm::NonZeroConst))) => possible = false,
                        Ok(Err(e)) => return Err(e),
                        _ => {} // always true, or not representable: left out (a larger piece)
                    }
                    Ok(())
                };
            match pos {
                LitPos::Equation(j) => {
                    for e in d.equations.iter().take(j) {
                        add_cond(e, false, &mut base)?;
                    }
                }
                LitPos::Member(bi, mi) => {
                    for e in &d.equations {
                        add_cond(e, false, &mut base)?;
                    }
                    for g in &d.bounds {
                        add_cond(g, true, &mut base)?;
                    }
                    for f in d.exceptions[bi].iter().take(mi) {
                        add_cond(f, false, &mut base)?;
                    }
                }
            }
            if !possible {
                continue; // a prefix condition never holds: the literal is never evaluated
            }
            let why = |w: &str| StopWhy::ModularZeroRisk(format!("{pos:?}: {w}"));
            if region.is_empty() {
                // anywhere: the prefix set
                extra.push(StopPiece {
                    rule: base.clone(),
                    why: why("anywhere"),
                });
                continue;
            }
            for (is_ge, q, label) in region {
                let lit = match &map {
                    None => Ok(q),
                    Some(mm) => q.compose(mm, arity),
                };
                match lit.map(|p| b.try_intern(&p, is_ge)) {
                    Ok(Ok(Ok(Norm::Lit(l)))) => {
                        let mut r = base.clone();
                        if is_ge {
                            r.ge.push(l);
                        } else {
                            r.eq.push(l);
                        }
                        extra.push(StopPiece {
                            rule: r,
                            why: why(&label),
                        });
                    }
                    Ok(Ok(Ok(Norm::NonZeroConst))) => {} // the region is never reached here
                    Ok(Err(e)) => return Err(e),
                    // always in the region, or not representable: the prefix set
                    _ => extra.push(StopPiece {
                        rule: base.clone(),
                        why: why(&label),
                    }),
                }
            }
        }
        if !risk_notes.is_empty() {
            note(&mut notes, NfNote::ModularZeroRisk(risk_notes.join("; ")));
        }
        for piece in &mut extra {
            piece.rule.eq.sort_unstable();
            piece.rule.eq.dedup();
            piece.rule.ge.sort_unstable();
            piece.rule.ge.dedup();
        }
        for id in lits_before..b.lits.len() {
            stop_created.push(lit_id(id)?);
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
    let mut stop_lits = vec![false; b.lits.len()];
    for id in stop_created {
        if let Some(x) = stop_lits.get_mut(id as usize) {
            *x = true;
        }
    }
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
        stop_lits,
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
