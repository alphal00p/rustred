//! Case domains of rules and their verdict at a key.
//!
//! A [`CaseDomain`] is the set of keys a parametric rule applies to:
//! - **chart** (optional): the orthant test (line slots >= 1, numerator slots <= 0), reported as
//!   [`Verdict::ChartMismatch`] and kept separate from the case. A record's `chart` must equal the frame mask
//!   ([`DomainError::ChartNotFrame`]; the engine refuses a chart other than the node mask);
//! - **target pattern** ([`TargetMap`]): as the engine's `find_rule`, the key is matched against the rule's `target`
//!   (`n<a>+o` sets the rule point `pt[a] = k[j] - o`, a constant `o` requires `k[j] == o`); equations, bounds,
//!   exceptions and pole loci are evaluated AT THE RULE POINT. A source that carries no target (case-domain
//!   exports, bank predicates) is loaded only with the caller's explicit statement
//!   [`TargetSource::CanonicalByConstruction`] (audit A M1);
//! - **case**: fixed slots (tested on the key, as the engine), equations (polynomials that must vanish), lower bounds
//!   (polynomials that must be >= 0: the T162 bank's written `domain_atoms`);
//! - **exceptions**: an OR-list of AND-lists of polynomials; the rule does NOT apply where every polynomial of some
//!   AND-list vanishes ([`Verdict::OnException`] names the branch);
//! - **recenter bounds** (opt-in for engine records): NOT tested by the engine's `find_rule`;
//! - **pole loci** (d-dependent conditions: written RHS denominators, header source conditions, witness loci): a
//!   polynomial `P(n,d) = sum_j c_j(n) d^j` forbids the key iff EVERY `c_j(n)` vanishes, i.e. `P` vanishes identically
//!   in `d` at the key ([`Verdict::OnPole`]);
//! - **elided witness loci**: a record whose `witness_loci` holds the literal `"elided"` is UNUSABLE wherever it would
//!   be selected: [`Verdict::Elided`] (ruling F1 of the coordinator; never a pass).
//!
//! # Selection versus failure after selection (audit A M3)
//!
//! The engine's first match (`mastermap.rs` `find_rule`) continues past a rule only for: a fixed slot that differs,
//! a target constant that differs, an equation that does not vanish, an exception branch that vanishes entirely
//! (and `dispatch` other than `Partition`, handled at load). Everything else is decided AFTER the rule is selected:
//! a coefficient pole surfaces in `reduce_mod` as an error, and the search does NOT move on to the next rule.
//! [`Verdict::stage`] encodes this: [`Stage::NotSelected`] (dispatch continues), [`Stage::Blocked`] (the rule is
//! selected but cannot be used at the key: pole, recenter bound, chart mismatch, elided witness loci) and
//! [`Stage::Undecided`] (evaluation error, wrong key length). Only [`Verdict::Applies`] credits the rule.
//! The T162 bank's written `GE` atoms are part of the bank's DEFINITION of the domain and count as selection.
//!
//! Domain literals (equations, bounds, exceptions) must be d-free: parsing asserts it ([`DomainError::Poly`] with
//! `PolyError::ContainsD`). They are read with the EXACT parser ([`Poly::parse_d_free_exact`]; pole loci with
//! [`Poly::parse_exact`]): a coefficient beyond `i128` is kept exactly (a big literal), never refused, never truncated
//! (lane import3: four T266 records carry exception literals with 39-46 digit coefficients). Evaluation is exact (i128
//! with an arbitrary-precision fallback). A literal that is
//! nonzero but vanishes modulo both of the engine's primes is reported as an evaluation error (the engine would read
//! it as zero; audit A N1).
//!
//! Differences from the engine's matcher are listed in the interface document
//! `plan/auto-campaign/interfaces/campaign-core-domain.md`.

use std::fmt;

use crate::key::{FrameError, NodeFrame};
use crate::poly::{EvalError, Poly, PolyError};
use crate::record::{BankPredicate, DomainBounds, RuleRecord};

/// Errors while building a domain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomainError {
    Poly {
        field: &'static str,
        text: String,
        err: PolyError,
    },
    Frame(FrameError),
    Malformed(String),
    /// The record's `dispatch` is neither absent nor `Partition`: the engine never selects it (`Table::load` sets
    /// `skip`). The table loader counts such records; they are never candidates (audit A M2).
    NotDispatched {
        dispatch: String,
    },
    /// The record carries no `target` and the caller did not state that targets are canonical by construction.
    MissingTarget,
    /// The `target` cannot be read, or uses a feature this crate refuses (repeated parameter, parameter out of range,
    /// a non-canonical target combined with recenter bounds).
    Target(String),
    /// The record's chart differs from the frame mask (the engine refuses a chart other than the node mask).
    ChartNotFrame {
        chart: String,
        frame: String,
    },
    /// Recenter bounds were requested ([`DomainOptions::recenter_bounds`]) but the record's `recenter` is `null` or
    /// absent (the engine writes `null` for a rule without a raw pivot): its bounds are unknown, so the record is
    /// refused instead of being loaded without bounds (re-audit RA minor N-m1).
    MissingRecenter,
    /// Inconsistent public fields (from [`CaseDomain::validate`]).
    Invalid(String),
    /// The record's `domain_bounds` is one the engine refuses or one whose engine behaviour is not modelled
    /// ([`bound_branches`]; lane partfix4).
    Bounds(String),
    /// The record carries top-level members outside [`crate::record::RULE_RECORD_MEMBERS`] (sorted): never loaded,
    /// since a member this crate does not know may change what the engine does (lane partfix4).
    UnknownMember(Vec<String>),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DomainError::Poly { field, text, err } => write!(f, "{field} literal {text:?}: {err}"),
            DomainError::Frame(e) => write!(f, "{e}"),
            DomainError::Malformed(s) => write!(f, "malformed record: {s}"),
            DomainError::NotDispatched { dispatch } => {
                write!(f, "dispatch {dispatch:?}: never selected by the engine")
            }
            DomainError::MissingTarget => write!(
                f,
                "record without target and no statement that targets are canonical by construction"
            ),
            DomainError::Target(s) => write!(f, "target: {s}"),
            DomainError::ChartNotFrame { chart, frame } => {
                write!(
                    f,
                    "record chart {chart} differs from the frame mask {frame}"
                )
            }
            DomainError::MissingRecenter => write!(
                f,
                "recenter bounds requested but the record's recenter is null or absent"
            ),
            DomainError::Invalid(s) => write!(f, "invalid domain: {s}"),
            DomainError::Bounds(s) => write!(f, "domain_bounds: {s}"),
            DomainError::UnknownMember(m) => write!(
                f,
                "rule record member(s) {m:?} unknown to the reader (refused, never ignored)"
            ),
        }
    }
}
impl std::error::Error for DomainError {}
impl From<FrameError> for DomainError {
    fn from(e: FrameError) -> Self {
        DomainError::Frame(e)
    }
}

/// Origin of a pole locus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PoleKind {
    /// A written RHS denominator.
    Denominator,
    /// A header source condition.
    SourceCondition,
    /// A witness locus of the rule.
    Witness,
    /// A cached exception-or-pole locus (bank field `cached_domain_exception_or_pole_AND_loci` is d-free and is
    /// stored as exceptions; this kind is kept for completeness).
    Cached,
}

/// A d-dependent condition: the key is forbidden iff every d-coefficient vanishes there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoleLocus {
    pub kind: PoleKind,
    /// The d-coefficients `c_j(n)` (d-free, nonzero polynomials). Empty = the polynomial is identically zero: the
    /// locus is everything.
    pub coeffs: Vec<Poly>,
}

impl PoleLocus {
    pub fn new(kind: PoleKind, p: &Poly) -> Self {
        PoleLocus {
            kind,
            coeffs: p.d_coefficients().into_iter().map(|(_, c)| c).collect(),
        }
    }
    /// True iff the polynomial vanishes identically in d at the key. A key shorter than a variable index used by ANY
    /// coefficient is an error, whatever the coefficients' values (audit RB m4: an early nonzero coefficient used to
    /// hide a too-short key).
    pub fn vanishes_at(&self, key: &[i32]) -> Result<bool, EvalError> {
        let needed = self.coeffs.iter().map(Poly::min_arity).max().unwrap_or(0);
        if key.len() < needed {
            return Err(EvalError::KeyTooShort {
                needed,
                len: key.len(),
            });
        }
        for c in &self.coeffs {
            if !c.is_zero_at(key)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Why a key is not in the case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotInCase {
    /// Key length differs from the domain arity.
    Arity { expected: usize, got: usize },
    /// A fixed slot differs.
    FixedSlot { slot: usize, fixed: i32, got: i32 },
    /// A constant slot of the target pattern differs.
    TargetConstant {
        slot: usize,
        constant: i64,
        got: i32,
    },
    /// Equation number `.0` does not vanish.
    Equation(usize),
    /// Bound number `.0` is negative.
    Bound(usize),
}

/// The verdict of a domain at a key. Only [`Verdict::Applies`] credits the rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Applies,
    /// The key is outside the chart orthant (first violating slot).
    ChartMismatch {
        slot: usize,
    },
    NotInCase(NotInCase),
    /// Every polynomial of exception branch `branch` vanishes.
    OnException {
        branch: usize,
    },
    /// Recenter bound number `.0` is violated (opt-in; not tested by the engine's matcher).
    RecenterBound(usize),
    /// Pole locus number `index` (of kind `kind`) vanishes identically in d.
    OnPole {
        kind: PoleKind,
        index: usize,
    },
    /// The rule is selected at the key, but its witness loci are elided: unusable (ruling F1), fail closed.
    Elided,
    /// The rule is selected at the key, but its source carries no witness loci (case-domain export) and no witness
    /// attestation covers it: unusable, fail closed (re-audit RA-MAJOR-1). Produced by the dispatcher
    /// ([`crate::cover::Dispatcher`]), never by [`CaseDomain::applies`], which does not know the source.
    WitnessNotCarried,
    /// Evaluation failed (rule point out of the `i32` range, or a value the engine's modular zero test would read
    /// differently); the selection itself is unknown.
    Error(EvalError),
}

/// Where a verdict stops first-match dispatch (see the module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// The rule is selected and applies: a credit.
    Applies,
    /// The engine's search continues past this rule.
    NotSelected,
    /// The rule is selected (case contains the key, no exception) but cannot be used here: the search STOPS and
    /// the key is open, naming this rule.
    Blocked,
    /// Whether the rule is selected is unknown: the search stops and the key is open.
    Undecided,
}

impl Verdict {
    pub fn applies(&self) -> bool {
        matches!(self, Verdict::Applies)
    }
    /// The stage of this verdict under the engine's first match.
    pub fn stage(&self) -> Stage {
        match self {
            Verdict::Applies => Stage::Applies,
            Verdict::NotInCase(NotInCase::Arity { .. }) | Verdict::Error(_) => Stage::Undecided,
            Verdict::NotInCase(_) | Verdict::OnException { .. } => Stage::NotSelected,
            Verdict::RecenterBound(_)
            | Verdict::OnPole { .. }
            | Verdict::Elided
            | Verdict::WitnessNotCarried
            | Verdict::ChartMismatch { .. } => Stage::Blocked,
        }
    }
}

/// One slot of a rule's target pattern (engine `Pat`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TargetSlot {
    /// `n<param> + offset`: the rule point gets `pt[param] = k[slot] - offset`.
    Param { param: usize, offset: i64 },
    /// A constant: the key must have exactly this value at the slot.
    Const(i64),
}

/// How a key is mapped to the rule point.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TargetMap {
    /// The rule point is the key itself (target `n<j>` on every free slot, the fixed value or `n<j>` on fixed slots).
    Canonical,
    /// Any other target: matched exactly as the engine's `find_rule` (see [`parse_target`]).
    Pattern(Vec<TargetSlot>),
}

/// Where the target of a record comes from (audit A M1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetSource {
    /// The record must carry `target`; it is parsed and matched (default).
    FromRecord,
    /// The caller states that this source's targets are canonical by construction (case-domain exports of engine
    /// tables, the T162 bank). A record that does carry a target still has it parsed and matched.
    CanonicalByConstruction { attestation: &'static str },
}

/// Parse a target pattern `I(a_0,..,a_{N-1})`, each argument `n<p>`, `n<p>+<o>`, `n<p>-<o>` or an integer (the
/// engine's `parse_pat` grammar, read strictly: no spaces inside an argument's sign, no leading `+` on constants).
/// Refused: a parameter `>= arity`, a parameter used twice (the engine would silently keep the last write).
pub fn parse_target(text: &str, arity: usize) -> Result<Vec<TargetSlot>, String> {
    let inner = text
        .trim()
        .strip_prefix("I(")
        .and_then(|t| t.strip_suffix(')'))
        .ok_or_else(|| format!("bad target {text:?}"))?;
    let strict_int = |t: &str| -> Option<i64> {
        let digits = t.strip_prefix('-').unwrap_or(t);
        (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
            .then(|| t.parse::<i64>().ok())
            .flatten()
    };
    let mut out = Vec::with_capacity(arity);
    let mut used = vec![false; arity];
    for a in inner.split(',') {
        let a = a.trim();
        if let Some(rest) = a.strip_prefix('n') {
            let end = rest.find(['+', '-']).unwrap_or(rest.len());
            let idx = &rest[..end];
            if idx.is_empty()
                || !idx.bytes().all(|b| b.is_ascii_digit())
                || (idx.len() > 1 && idx.starts_with('0'))
            {
                return Err(format!("bad target argument {a:?}"));
            }
            let param: usize = idx
                .parse()
                .map_err(|_| format!("bad target argument {a:?}"))?;
            let offset = if end == rest.len() {
                0
            } else {
                let o = &rest[end..];
                let v = match o.strip_prefix('+') {
                    Some(p) if !p.starts_with('-') => strict_int(p),
                    Some(_) => None,
                    None => strict_int(o),
                };
                v.ok_or_else(|| format!("bad target argument {a:?}"))?
            };
            if param >= arity {
                return Err(format!("target parameter n{param} >= arity {arity}"));
            }
            if used[param] {
                return Err(format!("target parameter n{param} used twice"));
            }
            used[param] = true;
            out.push(TargetSlot::Param { param, offset });
        } else {
            out.push(TargetSlot::Const(
                strict_int(a).ok_or_else(|| format!("bad target argument {a:?}"))?,
            ));
        }
    }
    if out.len() != arity {
        return Err(format!("target of arity {}, expected {arity}", out.len()));
    }
    Ok(out)
}

/// True iff a parsed target is the canonical pattern of its case (`n<j>` on every free slot, the fixed value or
/// `n<j>` on fixed slots): the rule point is then the key itself.
pub fn is_canonical_target(slots: &[TargetSlot], fixed: &[Option<i32>]) -> bool {
    slots.len() == fixed.len()
        && slots.iter().enumerate().all(|(j, t)| match (t, fixed[j]) {
            (TargetSlot::Param { param, offset: 0 }, _) if *param == j => true,
            (TargetSlot::Const(c), Some(v)) => *c == v as i64,
            _ => false,
        })
}

/// The fixed slots of a record as `i32`, checked against the arity.
pub(crate) fn record_fixed(r: &RuleRecord, n: usize) -> Result<Vec<Option<i32>>, DomainError> {
    if r.fixed.len() != n {
        return Err(DomainError::Malformed(format!(
            "fixed of length {} for arity {n}",
            r.fixed.len()
        )));
    }
    fixed_i32(&r.fixed)
}

fn target_map(slots: Vec<TargetSlot>, fixed: &[Option<i32>]) -> TargetMap {
    if is_canonical_target(&slots, fixed) {
        TargetMap::Canonical
    } else {
        TargetMap::Pattern(slots)
    }
}

/// The most chart values one `domain_bounds` entry may exclude on a FREE slot (rustred-core node/bounds.rs
/// `MAX_EXCLUDED_PER_SLOT`; the engine refuses a farther bound at load).
pub const ENGINE_MAX_BOUND_EXCLUSIONS: i64 = 64;

/// THE meaning of a rule record's `domain_bounds` (lane partfix4; audit-part4 B1 / B2 / B2b): the ENGINE's, ported
/// line by line from rustred-core node/bounds.rs `bound_exclusions` (frozen engine-line7a 6c2fb15c), which
/// mastermap.rs `Table::load` appends to the record's exceptions. The result is a list of extra EXCEPTION BRANCHES,
/// each the one literal `n_i - v`, which [`CaseDomain::from_rule_record`] appends AFTER the written exceptions, so the
/// dispatcher, the partition and the engine evaluate them by one code path: at the RULE POINT, a branch that vanishes
/// deselects the record (dispatch falls through). Per slot `i` (`chart[i]`: the record's chart, line = `1`):
/// - a `max` on a line slot or a `min` on a non-line slot is refused (fixed or not; the engine: it would cut an
///   infinite set);
/// - a FIXED slot (`fixed[i] = f`, tested on the KEY): a violated bound (`f < min` or `f > max`) adds `n_i - f`,
///   otherwise nothing, whatever the rule point's coordinate `i` is (B2b: with a target pattern the engine compares
///   the key's fixed value, not the rule point; reproduced, not "corrected");
/// - a free line slot with `min = m`: `n_i - v` for `v = 1 .. m-1` (refused when `m - 1 > 64`); a free non-line slot
///   with `max = M`: `n_i - v` for `v = M+1 ..= 0` (refused when `-M > 64`). With a canonical target the rule point is
///   the key and these are exactly `n_i >= m` / `n_i <= M` on the chart; with a target pattern the rule point may
///   leave the chart side, and only these finitely many values are excluded (B2; reproduced).
///
/// Refused where the engine's behaviour is not modelled: lengths other than the chart's; an `i64` edge value where the
/// engine's arithmetic would overflow (`min = i64::MIN`; `max = i64::MIN` makes the engine enumerate 2^63 values).
pub fn bound_branches(
    b: &DomainBounds,
    chart: &[bool],
    fixed: &[Option<i64>],
) -> Result<Vec<Vec<Poly>>, String> {
    let n = chart.len();
    if b.min.len() != n || b.max.len() != n || fixed.len() != n {
        return Err(format!(
            "min / max / fixed of lengths {} / {} / {} for the chart's arity {n}",
            b.min.len(),
            b.max.len(),
            fixed.len()
        ));
    }
    let mut out: Vec<Vec<Poly>> = Vec::new();
    for i in 0..n {
        let (min, max) = (b.min[i], b.max[i]);
        if chart[i] && max.is_some() {
            return Err(format!(
                "a max on line slot {i} would cut an infinite set (refused as by the engine)"
            ));
        }
        if !chart[i] && min.is_some() {
            return Err(format!(
                "a min on non-line slot {i} would cut an infinite set (refused as by the engine)"
            ));
        }
        if let Some(f) = fixed[i] {
            if min.is_some_and(|m| f < m) || max.is_some_and(|m| f > m) {
                out.push(vec![Poly::slot_minus(i, i128::from(f))]);
            }
            continue;
        }
        if let Some(m) = min {
            let far = m
                .checked_sub(1)
                .is_none_or(|x| x > ENGINE_MAX_BOUND_EXCLUSIONS);
            if far {
                return Err(format!(
                    "min {m} on free slot {i} excludes more than {ENGINE_MAX_BOUND_EXCLUSIONS} chart values (refused as by the engine)"
                ));
            }
            for v in 1..m {
                out.push(vec![Poly::slot_minus(i, i128::from(v))]);
            }
        }
        if let Some(m) = max {
            let far = m
                .checked_neg()
                .is_none_or(|x| x > ENGINE_MAX_BOUND_EXCLUSIONS);
            if far {
                return Err(format!(
                    "max {m} on free slot {i} excludes more than {ENGINE_MAX_BOUND_EXCLUSIONS} chart values (refused as by the engine)"
                ));
            }
            for v in (m + 1)..=0 {
                out.push(vec![Poly::slot_minus(i, i128::from(v))]);
            }
        }
    }
    Ok(out)
}

/// What to take from a raw rule record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DomainOptions {
    /// Where the target comes from (default: the record must carry it).
    pub targets: TargetSource,
    /// Check the chart orthant (default true).
    pub chart: bool,
    /// Evaluate `witness_loci` as pole loci and report `Elided` for elided ones (default true). The engine writes
    /// the factors of the witness poles into `exceptions` already, so this is a redundancy check.
    pub witness: bool,
    /// Add the recenter bounds (line slot n_i >= max(1, 1 - recenter_i), numerator n_j <= min(0, -recenter_j)) as
    /// post-selection bounds ([`Verdict::RecenterBound`], [`Stage::Blocked`]; default false: the engine's matcher
    /// does not test them). The record's `recenter` must have one entry per slot; a `null` or absent `recenter` is
    /// [`DomainError::MissingRecenter`].
    pub recenter_bounds: bool,
}

impl Default for DomainOptions {
    fn default() -> Self {
        DomainOptions {
            targets: TargetSource::FromRecord,
            chart: true,
            witness: true,
            recenter_bounds: false,
        }
    }
}

/// A rule's case domain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseDomain {
    pub arity: usize,
    /// Line mask for the chart orthant test; `None` = no chart test.
    pub chart: Option<Vec<bool>>,
    /// Tested on the KEY (as the engine).
    pub fixed: Vec<Option<i32>>,
    /// Key -> rule point.
    pub target: TargetMap,
    pub equations: Vec<Poly>,
    /// Must be >= 0 (selection: written domain atoms).
    pub bounds: Vec<Poly>,
    /// OR of AND-lists.
    pub exceptions: Vec<Vec<Poly>>,
    /// Must be >= 0 (after selection: opt-in recenter bounds).
    pub recenter_bounds: Vec<Poly>,
    pub poles: Vec<PoleLocus>,
    pub witness_elided: bool,
}

fn parse_list(v: &[String], arity: usize, field: &'static str) -> Result<Vec<Poly>, DomainError> {
    v.iter()
        .map(|s| {
            Poly::parse_d_free_exact(s, arity).map_err(|err| DomainError::Poly {
                field,
                text: s.clone(),
                err,
            })
        })
        .collect()
}

fn parse_poles(
    v: &[String],
    arity: usize,
    kind: PoleKind,
    out: &mut Vec<PoleLocus>,
) -> Result<(), DomainError> {
    for s in v {
        let p = Poly::parse_exact(s, arity, true).map_err(|err| DomainError::Poly {
            field: "pole",
            text: s.clone(),
            err,
        })?;
        out.push(PoleLocus::new(kind, &p));
    }
    Ok(())
}

fn fixed_i32(fixed: &[Option<i64>]) -> Result<Vec<Option<i32>>, DomainError> {
    fixed
        .iter()
        .map(|v| match v {
            None => Ok(None),
            Some(x) => i32::try_from(*x)
                .map(Some)
                .map_err(|_| DomainError::Malformed(format!("fixed value {x}"))),
        })
        .collect()
}

impl CaseDomain {
    /// A domain with free slots only (applies everywhere on the chart, if given).
    pub fn free(arity: usize, chart: Option<Vec<bool>>) -> Self {
        CaseDomain {
            arity,
            chart,
            fixed: vec![None; arity],
            target: TargetMap::Canonical,
            equations: vec![],
            bounds: vec![],
            exceptions: vec![],
            recenter_bounds: vec![],
            poles: vec![],
            witness_elided: false,
        }
    }

    /// Consistency of the public fields (lengths, polynomial supports, target parameters). The dispatcher calls it
    /// on every domain ([`crate::cover::Table::push`]).
    pub fn validate(&self) -> Result<(), DomainError> {
        let n = self.arity;
        let bad = |s: String| Err(DomainError::Invalid(s));
        if self.fixed.len() != n {
            return bad(format!(
                "fixed of length {} for arity {n}",
                self.fixed.len()
            ));
        }
        if let Some(c) = &self.chart
            && c.len() != n
        {
            return bad(format!("chart of length {} for arity {n}", c.len()));
        }
        if let TargetMap::Pattern(t) = &self.target {
            if t.len() != n {
                return bad(format!("target of length {} for arity {n}", t.len()));
            }
            let mut used = vec![false; n];
            for s in t {
                if let TargetSlot::Param { param, .. } = s {
                    if *param >= n || used[*param] {
                        return bad(format!("target parameter n{param}"));
                    }
                    used[*param] = true;
                }
            }
            if !self.recenter_bounds.is_empty() {
                return bad("recenter bounds with a non-canonical target".into());
            }
        }
        let polys = self
            .equations
            .iter()
            .chain(&self.bounds)
            .chain(self.exceptions.iter().flatten())
            .chain(&self.recenter_bounds)
            .chain(self.poles.iter().flat_map(|p| &p.coeffs));
        for p in polys {
            if p.min_arity() > n {
                return bad(format!("polynomial {p} beyond arity {n}"));
            }
        }
        for p in self
            .equations
            .iter()
            .chain(&self.bounds)
            .chain(self.exceptions.iter().flatten())
            .chain(&self.recenter_bounds)
        {
            if p.has_d() {
                return bad(format!("d in the d-free literal {p}"));
            }
        }
        Ok(())
    }

    /// From an rr_node `table_dump.v1` rule record or a case-domain export record.
    ///
    /// Refused (typed errors, never a silent reading): a non-zero power shift of the frame, `dispatch` other than
    /// absent/`Partition` ([`DomainError::NotDispatched`]), a record chart other than the frame mask, a missing
    /// target unless `opts.targets` states canonical-by-construction, an unreadable target, a `recenter` that is
    /// missing, `null` or of the wrong length (when recenter bounds are requested), a non-canonical target together
    /// with recenter bounds.
    pub fn from_rule_record(
        r: &RuleRecord,
        frame: &NodeFrame,
        opts: DomainOptions,
    ) -> Result<Self, DomainError> {
        frame.require_integer_only()?;
        let n = frame.arity();
        // `domain_bounds` first (lane partfix4): the engine converts them for EVERY rule record at load, dispatched or
        // not, and refuses the whole table on a bad one (mastermap.rs `Table::load`)
        let bound_exc = match &r.domain_bounds {
            None => vec![],
            Some(b) => {
                let chart: Vec<bool> = r
                    .chart
                    .as_deref()
                    .ok_or_else(|| DomainError::Bounds("rule without chart".into()))?
                    .chars()
                    .map(|c| c == '1')
                    .collect();
                bound_branches(b, &chart, &r.fixed).map_err(DomainError::Bounds)?
            }
        };
        if let Some(dsp) = &r.dispatch
            && dsp != "Partition"
        {
            return Err(DomainError::NotDispatched {
                dispatch: dsp.clone(),
            });
        }
        if !r.unknown_members.is_empty() {
            return Err(DomainError::UnknownMember(r.unknown_members.clone()));
        }
        if r.fixed.len() != n {
            return Err(DomainError::Malformed(format!(
                "fixed of length {} for arity {n}",
                r.fixed.len()
            )));
        }
        if let Some(c) = &r.chart
            && *c != frame.mask_string()
        {
            return Err(DomainError::ChartNotFrame {
                chart: c.clone(),
                frame: frame.mask_string(),
            });
        }
        let chart = opts.chart.then(|| frame.mask().to_vec());
        let fixed = fixed_i32(&r.fixed)?;
        let target = match (&r.target, opts.targets) {
            (Some(t), _) => target_map(parse_target(t, n).map_err(DomainError::Target)?, &fixed),
            (None, TargetSource::CanonicalByConstruction { .. }) => TargetMap::Canonical,
            (None, TargetSource::FromRecord) => return Err(DomainError::MissingTarget),
        };
        let mut d = CaseDomain {
            arity: n,
            chart,
            fixed,
            target,
            equations: parse_list(&r.equations, n, "equation")?,
            bounds: vec![],
            exceptions: r
                .exceptions
                .iter()
                .map(|b| parse_list(b, n, "exception"))
                .collect::<Result<_, _>>()?,
            recenter_bounds: vec![],
            poles: vec![],
            witness_elided: false,
        };
        // the engine's order: written exceptions, then the bound branches ([`bound_branches`])
        d.exceptions.extend(bound_exc);
        if opts.witness
            && let Some(w) = &r.witness_loci
        {
            let (elided, listed): (Vec<String>, Vec<String>) =
                w.iter().cloned().partition(|s| s.trim() == "elided");
            d.witness_elided = !elided.is_empty();
            parse_poles(&listed, n, PoleKind::Witness, &mut d.poles)?;
        }
        if opts.recenter_bounds {
            let rc = r.recenter.as_ref().ok_or(DomainError::MissingRecenter)?;
            if rc.len() != n {
                return Err(DomainError::Malformed(format!(
                    "recenter of length {} for arity {n}",
                    rc.len()
                )));
            }
            if d.target != TargetMap::Canonical {
                return Err(DomainError::Target(
                    "recenter bounds with a non-canonical target are not supported".into(),
                ));
            }
            for (i, &c) in rc.iter().enumerate() {
                let c = c as i128;
                d.recenter_bounds.push(if frame.is_line(i) {
                    Poly::slot_minus(i, (1i128).max(1 - c)) // n_i - max(1, 1-c) >= 0
                } else {
                    Poly::slot_minus(i, (0i128).min(-c))
                        .neg()
                        .map_err(|e| DomainError::Malformed(format!("recenter {c}: {e}")))? // min(0,-c) - n_i >= 0
                });
            }
        }
        d.validate()?;
        Ok(d)
    }

    /// From a T162 bank predicate (`RULE-PREDICATES2440.json`). The chart is given by the `GE` atoms, so no separate
    /// chart test is attached. Exceptions = written exceptions plus the cached loci; pole loci = written
    /// denominators, header source conditions and witness loci. The bank carries no target: `targets` must be
    /// [`TargetSource::CanonicalByConstruction`] (else [`DomainError::MissingTarget`]), and the predicate must say
    /// `no_recenter_or_permutation_applied: true`.
    pub fn from_bank_predicate(
        p: &BankPredicate,
        arity: usize,
        targets: TargetSource,
    ) -> Result<Self, DomainError> {
        if targets == TargetSource::FromRecord {
            return Err(DomainError::MissingTarget);
        }
        if p.no_recenter_or_permutation_applied != Some(true) {
            return Err(DomainError::Malformed(format!(
                "bank rule {}: no_recenter_or_permutation_applied is not true",
                p.global_ordinal0
            )));
        }
        if p.raw_fixed20.len() != arity {
            return Err(DomainError::Malformed(format!(
                "raw_fixed20 of length {}",
                p.raw_fixed20.len()
            )));
        }
        let fixed = fixed_i32(&p.raw_fixed20)?;
        let mut equations = parse_list(
            p.case_equations.as_deref().unwrap_or(&[]),
            arity,
            "equation",
        )?;
        let mut bounds = vec![];
        for (rel, t) in &p.domain_atoms {
            let q = Poly::parse_d_free_exact(t, arity).map_err(|err| DomainError::Poly {
                field: "atom",
                text: t.clone(),
                err,
            })?;
            match rel.as_str() {
                "EQ" => equations.push(q),
                "GE" => bounds.push(q),
                o => return Err(DomainError::Malformed(format!("atom relation {o}"))),
            }
        }
        let mut exceptions: Vec<Vec<Poly>> = vec![];
        for b in p
            .exceptions
            .iter()
            .flatten()
            .chain(p.cached_loci.iter().flatten())
        {
            exceptions.push(parse_list(b, arity, "exception")?);
        }
        let mut poles = vec![];
        parse_poles(
            p.denominators.as_deref().unwrap_or(&[]),
            arity,
            PoleKind::Denominator,
            &mut poles,
        )?;
        parse_poles(
            p.source_conditions.as_deref().unwrap_or(&[]),
            arity,
            PoleKind::SourceCondition,
            &mut poles,
        )?;
        // ruling F1: an elided witness locus makes the rule unusable wherever it is selected (as for raw records)
        let (elided, listed): (Vec<String>, Vec<String>) = p
            .witness_loci
            .iter()
            .flatten()
            .cloned()
            .partition(|s| s.trim() == "elided");
        parse_poles(&listed, arity, PoleKind::Witness, &mut poles)?;
        let d = CaseDomain {
            arity,
            chart: None,
            fixed,
            target: TargetMap::Canonical,
            equations,
            bounds,
            exceptions,
            recenter_bounds: vec![],
            poles,
            witness_elided: !elided.is_empty(),
        };
        d.validate()?;
        Ok(d)
    }

    /// The verdict at `key`: arity, chart, fixed slots, target pattern, then at the rule point: equations, bounds,
    /// every exception branch, recenter bounds, pole loci, elision, in this order.
    pub fn applies(&self, key: &[i32]) -> Verdict {
        if key.len() != self.arity {
            return Verdict::NotInCase(NotInCase::Arity {
                expected: self.arity,
                got: key.len(),
            });
        }
        if let Some(mask) = &self.chart {
            for (i, &x) in key.iter().enumerate() {
                if (mask[i] && x < 1) || (!mask[i] && x > 0) {
                    return Verdict::ChartMismatch { slot: i };
                }
            }
        }
        self.applies_in_chart(key)
    }

    /// The verdict without the chart test.
    pub fn applies_in_chart(&self, key: &[i32]) -> Verdict {
        if key.len() != self.arity || self.fixed.len() != self.arity {
            return Verdict::NotInCase(NotInCase::Arity {
                expected: self.arity,
                got: key.len(),
            });
        }
        for (i, f) in self.fixed.iter().enumerate() {
            if let Some(v) = f
                && key[i] != *v
            {
                return Verdict::NotInCase(NotInCase::FixedSlot {
                    slot: i,
                    fixed: *v,
                    got: key[i],
                });
            }
        }
        let point;
        let pt: &[i32] = match &self.target {
            TargetMap::Canonical => key,
            TargetMap::Pattern(t) => {
                match self.rule_point(t, key) {
                    Ok(p) => point = p,
                    Err(v) => return v,
                }
                &point
            }
        };
        self.at_rule_point(pt)
    }

    /// The engine's rule point (`find_rule`): `pt[a] = k[j] - o` for `n<a>+o` at slot j, a constant must equal the
    /// key; parameters no slot maps get the fixed value of their slot, else 0.
    ///
    /// Order (re-audit RB m1): every target CONSTANT is tested first (a cheap deselecting condition: the engine's
    /// i64 arithmetic skips the rule there whatever the other slots hold); only then are the parameter slots
    /// computed, and a rule point outside the `i32` range is `Error(Overflow)` (Undecided, fail closed: the engine
    /// would evaluate the conditions in i64 at such a point, this crate does not). Equations and exceptions, the
    /// other deselecting conditions, need the rule point and therefore come after it.
    fn rule_point(&self, t: &[TargetSlot], key: &[i32]) -> Result<Vec<i32>, Verdict> {
        if t.len() != self.arity {
            return Err(Verdict::NotInCase(NotInCase::Arity {
                expected: self.arity,
                got: t.len(),
            }));
        }
        for (j, s) in t.iter().enumerate() {
            if let TargetSlot::Const(c) = *s
                && key[j] as i64 != c
            {
                return Err(Verdict::NotInCase(NotInCase::TargetConstant {
                    slot: j,
                    constant: c,
                    got: key[j],
                }));
            }
        }
        let mut pt: Vec<Option<i32>> = vec![None; self.arity];
        for (j, s) in t.iter().enumerate() {
            match *s {
                TargetSlot::Const(_) => {}
                TargetSlot::Param { param, offset } => {
                    let v = (key[j] as i64)
                        .checked_sub(offset)
                        .and_then(|v| i32::try_from(v).ok())
                        .ok_or(Verdict::Error(EvalError::Overflow))?;
                    match pt.get_mut(param) {
                        Some(x) => *x = Some(v),
                        None => return Err(Verdict::Error(EvalError::Overflow)),
                    }
                }
            }
        }
        Ok(pt
            .iter()
            .zip(&self.fixed)
            .map(|(p, f)| p.unwrap_or(f.unwrap_or(0)))
            .collect())
    }

    fn at_rule_point(&self, pt: &[i32]) -> Verdict {
        for (j, e) in self.equations.iter().enumerate() {
            match e.eval(pt) {
                Ok(v) if v.is_zero() => {}
                Ok(v) if v.engine_zero_disagrees() => {
                    return Verdict::Error(EvalError::EngineZeroTest);
                }
                Ok(_) => return Verdict::NotInCase(NotInCase::Equation(j)),
                Err(err) => return Verdict::Error(err),
            }
        }
        for (j, e) in self.bounds.iter().enumerate() {
            match e.eval(pt) {
                Ok(v) if v.signum() >= 0 => {}
                Ok(_) => return Verdict::NotInCase(NotInCase::Bound(j)),
                Err(err) => return Verdict::Error(err),
            }
        }
        'b: for (bi, br) in self.exceptions.iter().enumerate() {
            for e in br {
                match e.eval(pt) {
                    Ok(v) if v.is_zero() => {}
                    Ok(v) if v.engine_zero_disagrees() => {
                        return Verdict::Error(EvalError::EngineZeroTest);
                    }
                    Ok(_) => continue 'b,
                    Err(err) => return Verdict::Error(err),
                }
            }
            return Verdict::OnException { branch: bi };
        }
        for (j, e) in self.recenter_bounds.iter().enumerate() {
            match e.eval(pt) {
                Ok(v) if v.signum() >= 0 => {}
                Ok(_) => return Verdict::RecenterBound(j),
                Err(err) => return Verdict::Error(err),
            }
        }
        for (pi, p) in self.poles.iter().enumerate() {
            match p.vanishes_at(pt) {
                Ok(true) => {
                    return Verdict::OnPole {
                        kind: p.kind,
                        index: pi,
                    };
                }
                Ok(false) => {}
                Err(err) => return Verdict::Error(err),
            }
        }
        if self.witness_elided {
            return Verdict::Elided;
        }
        Verdict::Applies
    }

    /// Fixed-slot pattern: slots with a fixed value, ascending (the dispatcher's index; target constants are not
    /// indexed and are tested by [`CaseDomain::applies`]).
    pub fn fixed_slots(&self) -> Vec<usize> {
        self.fixed
            .iter()
            .enumerate()
            .filter_map(|(i, f)| f.map(|_| i))
            .collect()
    }

    /// Canonical form for de-duplication, as the checker's `parse_cases.py`: equations = {n_s - v for fixed slots} +
    /// case equations, every literal made primitive with a canonical sign, as a sorted set; exceptions = sorted list of
    /// sorted sets. Chart, bounds and pole loci are appended verbatim (empty for case-domain exports).
    pub fn canonical_form(&self) -> CanonicalDomain {
        let mut eq: Vec<Poly> = self
            .fixed
            .iter()
            .enumerate()
            .filter_map(|(i, f)| f.map(|v| Poly::slot_minus(i, v as i128).primitive_normal()))
            .chain(self.equations.iter().map(Poly::primitive_normal))
            .collect();
        eq.sort();
        eq.dedup();
        let mut bad: Vec<Vec<Poly>> = self
            .exceptions
            .iter()
            .map(|b| {
                let mut v: Vec<Poly> = b.iter().map(Poly::primitive_normal).collect();
                v.sort();
                v.dedup();
                v
            })
            .collect();
        bad.sort();
        CanonicalDomain {
            equations: eq,
            exceptions: bad,
            target: self.target.clone(),
            chart: self.chart.clone(),
            bounds: self.bounds.clone(),
            recenter_bounds: self.recenter_bounds.clone(),
            poles: self
                .poles
                .iter()
                .map(|p| (p.kind as u8, p.coeffs.clone()))
                .collect(),
            witness_elided: self.witness_elided,
        }
    }
}

/// See [`CaseDomain::canonical_form`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CanonicalDomain {
    pub equations: Vec<Poly>,
    pub exceptions: Vec<Vec<Poly>>,
    pub target: TargetMap,
    pub chart: Option<Vec<bool>>,
    pub bounds: Vec<Poly>,
    pub recenter_bounds: Vec<Poly>,
    pub poles: Vec<(u8, Vec<Poly>)>,
    pub witness_elided: bool,
}
