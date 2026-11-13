//! Case domains of rules and their verdict at a key.
//!
//! A [`CaseDomain`] is the set of keys a parametric rule applies to:
//! - **chart** (optional): the orthant test (line slots >= 1, numerator slots <= 0), reported as
//!   [`Verdict::ChartMismatch`] and kept separate from the case;
//! - **case**: fixed slots, equations (polynomials that must vanish), lower bounds (polynomials that must be >= 0,
//!   used by the T162 bank's `domain_atoms`, i.e. chart and recenter bounds written as atoms);
//! - **exceptions**: an OR-list of AND-lists of polynomials; the rule does NOT apply where every polynomial of some
//!   AND-list vanishes ([`Verdict::OnException`] names the branch);
//! - **pole loci** (d-dependent conditions: written RHS denominators, header source conditions, witness loci): a
//!   polynomial `P(n,d) = sum_j c_j(n) d^j` forbids the key iff EVERY `c_j(n)` vanishes, i.e. `P` vanishes identically
//!   in `d` at the key ([`Verdict::OnPole`]). No factorisation is needed: a product of polynomials in `Q[d]` is zero iff
//!   a factor is;
//! - **elided witness loci**: a record whose `witness_loci` holds the literal `"elided"` cannot be decided where it would
//!   otherwise apply: [`Verdict::Elided`] (never a pass).
//!
//! Domain literals (equations, bounds, exceptions) must be d-free: parsing asserts it ([`DomainError::Poly`] with
//! `PolyError::ContainsD`). Evaluation is exact (i128 with an arbitrary-precision fallback).
//!
//! Differences from the engine's matcher (`rustred-core/src/node/mastermap.rs` `find_rule`) are listed in the
//! interface document `plan/auto-campaign/interfaces/campaign-core-domain.md`.

use std::fmt;

use crate::key::{FrameError, NodeFrame};
use crate::poly::{EvalError, Poly, PolyError};
use crate::record::{BankPredicate, RuleRecord};

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
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DomainError::Poly { field, text, err } => write!(f, "{field} literal {text:?}: {err}"),
            DomainError::Frame(e) => write!(f, "{e}"),
            DomainError::Malformed(s) => write!(f, "malformed record: {s}"),
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
    /// True iff the polynomial vanishes identically in d at the key.
    pub fn vanishes_at(&self, key: &[i32]) -> Result<bool, EvalError> {
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
    /// Pole locus number `index` (of kind `kind`) vanishes identically in d.
    OnPole {
        kind: PoleKind,
        index: usize,
    },
    /// The key would be in the domain, but the record's witness loci are elided: undecidable, fail closed.
    Elided,
    /// Evaluation failed (should not happen for well-formed domains).
    Error(EvalError),
}

impl Verdict {
    pub fn applies(&self) -> bool {
        matches!(self, Verdict::Applies)
    }
}

/// What to take from a raw rule record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DomainOptions {
    /// Check the chart orthant (default true).
    pub chart: bool,
    /// Evaluate `witness_loci` as pole loci and report `Elided` for elided ones (default true). The engine writes
    /// the factors of the witness poles into `exceptions` already, so this is a redundancy check.
    pub witness: bool,
    /// Add the recenter bounds (line slot n_i >= max(1, 1 - recenter_i), numerator n_j <= min(0, -recenter_j)) as
    /// case bounds (default false: the engine's matcher does not test them).
    pub recenter_bounds: bool,
}

impl Default for DomainOptions {
    fn default() -> Self {
        DomainOptions {
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
    pub fixed: Vec<Option<i32>>,
    pub equations: Vec<Poly>,
    /// Must be >= 0.
    pub bounds: Vec<Poly>,
    /// OR of AND-lists.
    pub exceptions: Vec<Vec<Poly>>,
    pub poles: Vec<PoleLocus>,
    pub witness_elided: bool,
}

fn parse_list(v: &[String], arity: usize, field: &'static str) -> Result<Vec<Poly>, DomainError> {
    v.iter()
        .map(|s| {
            Poly::parse_d_free(s, arity).map_err(|err| DomainError::Poly {
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
        let p = Poly::parse(s, arity, true).map_err(|err| DomainError::Poly {
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
            equations: vec![],
            bounds: vec![],
            exceptions: vec![],
            poles: vec![],
            witness_elided: false,
        }
    }

    /// From an rr_node `table_dump.v1` rule record or a case-domain export record.
    pub fn from_rule_record(
        r: &RuleRecord,
        frame: &NodeFrame,
        opts: DomainOptions,
    ) -> Result<Self, DomainError> {
        frame.require_integer_only()?;
        let n = frame.arity();
        if r.fixed.len() != n {
            return Err(DomainError::Malformed(format!(
                "fixed of length {} for arity {n}",
                r.fixed.len()
            )));
        }
        let chart = if opts.chart {
            let mask = match &r.chart {
                Some(c) => NodeFrame::from_mask(c)?,
                None => frame.clone(),
            };
            if mask.arity() != n {
                return Err(DomainError::Malformed(format!(
                    "chart {:?} for arity {n}",
                    r.chart
                )));
            }
            Some((0..n).map(|i| mask.is_line(i)).collect())
        } else {
            None
        };
        let mut d = CaseDomain {
            arity: n,
            chart,
            fixed: fixed_i32(&r.fixed)?,
            equations: parse_list(&r.equations, n, "equation")?,
            bounds: vec![],
            exceptions: r
                .exceptions
                .iter()
                .map(|b| parse_list(b, n, "exception"))
                .collect::<Result<_, _>>()?,
            poles: vec![],
            witness_elided: false,
        };
        if opts.witness {
            if let Some(w) = &r.witness_loci {
                let (elided, listed): (Vec<String>, Vec<String>) =
                    w.iter().cloned().partition(|s| s.trim() == "elided");
                d.witness_elided = !elided.is_empty();
                parse_poles(&listed, n, PoleKind::Witness, &mut d.poles)?;
            }
        }
        if opts.recenter_bounds {
            if let Some(rc) = &r.recenter {
                let mask = d
                    .chart
                    .clone()
                    .unwrap_or_else(|| (0..n).map(|i| frame.is_line(i)).collect());
                for (i, &c) in rc.iter().enumerate().take(n) {
                    let c = c as i128;
                    d.bounds.push(if mask[i] {
                        Poly::slot_minus(i, (1i128).max(1 - c)) // n_i - max(1, 1-c) >= 0
                    } else {
                        Poly::slot_minus(i, (0i128).min(-c)).neg().expect("small") // min(0,-c) - n_i >= 0
                    });
                }
            }
        }
        Ok(d)
    }

    /// From a T162 bank predicate (`RULE-PREDICATES2440.json`). The chart is given by the `GE` atoms, so no separate
    /// chart test is attached. Exceptions = written exceptions plus the cached loci; pole loci = written
    /// denominators, header source conditions and witness loci.
    pub fn from_bank_predicate(p: &BankPredicate, arity: usize) -> Result<Self, DomainError> {
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
            let q = Poly::parse_d_free(t, arity).map_err(|err| DomainError::Poly {
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
        parse_poles(
            p.witness_loci.as_deref().unwrap_or(&[]),
            arity,
            PoleKind::Witness,
            &mut poles,
        )?;
        Ok(CaseDomain {
            arity,
            chart: None,
            fixed,
            equations,
            bounds,
            exceptions,
            poles,
            witness_elided: false,
        })
    }

    /// The verdict at `key`: chart, fixed slots, equations, bounds, every exception branch, pole loci, elision, in
    /// this order.
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

    /// The verdict without the chart test (key length must equal the arity).
    pub fn applies_in_chart(&self, key: &[i32]) -> Verdict {
        for (i, f) in self.fixed.iter().enumerate() {
            if let Some(v) = f {
                if key[i] != *v {
                    return Verdict::NotInCase(NotInCase::FixedSlot {
                        slot: i,
                        fixed: *v,
                        got: key[i],
                    });
                }
            }
        }
        for (j, e) in self.equations.iter().enumerate() {
            match e.eval(key) {
                Ok(v) if v.is_zero() => {}
                Ok(_) => return Verdict::NotInCase(NotInCase::Equation(j)),
                Err(err) => return Verdict::Error(err),
            }
        }
        for (j, e) in self.bounds.iter().enumerate() {
            match e.eval(key) {
                Ok(v) if v.signum() >= 0 => {}
                Ok(_) => return Verdict::NotInCase(NotInCase::Bound(j)),
                Err(err) => return Verdict::Error(err),
            }
        }
        'b: for (bi, br) in self.exceptions.iter().enumerate() {
            for e in br {
                match e.is_zero_at(key) {
                    Ok(true) => {}
                    Ok(false) => continue 'b,
                    Err(err) => return Verdict::Error(err),
                }
            }
            return Verdict::OnException { branch: bi };
        }
        for (pi, p) in self.poles.iter().enumerate() {
            match p.vanishes_at(key) {
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

    /// Fixed-slot pattern: slots with a fixed value, ascending.
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
            chart: self.chart.clone(),
            bounds: self.bounds.clone(),
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
    pub chart: Option<Vec<bool>>,
    pub bounds: Vec<Poly>,
    pub poles: Vec<(u8, Vec<Poly>)>,
    pub witness_elided: bool,
}
