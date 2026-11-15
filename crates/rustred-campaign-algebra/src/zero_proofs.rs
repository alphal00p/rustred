//! Structural ZERO PROOFS of rule terms, on Symbolica (lane zeros2; interfaces/zero-proofs.md, closeloop.md 23).
//!
//! A term j of a rule record has the coefficient text N_j / D_j in the rule-point parameters n0..n{N-1} and d. A ZERO
//! PROOF shows that N_j vanishes IDENTICALLY on a LOCUS of rule points: the record's whole domain, or one hyperplane
//! n_p = v (e.g. where the term raises a numerator slot to a positive index), always intersected with the record's own
//! constraints. Method (exact polynomial algebra, no evaluation at sample points):
//! 1. the record's constants are substituted (parameters no target slot maps: fixed value or 0; parameters of fixed key
//!    slots), then the plane n_p = v (`MultivariatePolynomial::replace`, src/poly/polynomial.rs:2739);
//! 2. if the result is not the zero polynomial and the record has case equations, every equation linear in some parameter
//!    with coefficient +-1 is solved for it and substituted (`replace_with_poly`, :2987) into the numerator and the other
//!    equations (an equation that becomes a non-zero constant = the locus holds no rule point: `excluded`);
//! 3. `is_zero` (:1130) on the result: zero -> PROVED on the locus (for every value of the remaining parameters and d);
//! 4. an exception branch whose every polynomial vanishes identically on the locus removes it whole: `excluded`;
//! 5. the denominator on the locus: the zero polynomial -> not proved (the coefficient is undefined there); a d-coefficient
//!    (`to_univariate_polynomial_list`, :3759) that is a non-zero constant, or a factorisation (`Factorize::factor`,
//!    src/poly/factor.rs:2784) whose every factor has one, or is d-free and divides a one-polynomial exception branch, or
//!    is d-free, linear and of constant sign on the chart orthant -> `nowhere_zero`; else `where_regular`.
//!
//! Only the requested terms' texts are parsed (each side as a POLYNOMIAL, the strict forms of `coef_facts`: no division
//! inside a side, literal exponents <= `coef_facts::MAX_EXPONENT`), once per (record, term); every (term, locus) is
//! decided once. A zero at a key not covered by a proof falls back to the exact per-key evaluation (`coef_facts`
//! semantics: denominator identically zero in d at the rule point -> `unknown`, numerator -> `zero` / `nonzero`).
//! Generic (L8): loci come from the record's target map, the frame's chart and the rhs patterns; budgets are parameters.

use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

use rustred_campaign_core::accept::facts::split_ratio;
use rustred_campaign_core::accept::rule::RuleText;
use rustred_campaign_core::closure::CoefFact;
use rustred_campaign_core::closure::bodies::{RhsSlot, parse_rhs_pattern, rule_point};
use rustred_campaign_core::domain::{CaseDomain, TargetMap, TargetSlot, Verdict};
use rustred_campaign_core::import::zero_proofs::{
    DenStatus, Locus, VERDICT_PROVED, ZERO_PROOF_SCHEMA,
};
use serde_json::{Value, json};
use symbolica::domains::integer::{Integer, IntegerRing};
use symbolica::poly::factor::Factorize;
use symbolica::poly::polynomial::MultivariatePolynomial;

use super::coef_facts::side_budget;
use rustred_campaign_algebra::SymbolicaAlgebra;

type ZPoly = MultivariatePolynomial<IntegerRing, u16>;

/// Schema of the header line of a proof file written by `prove` (tables, sha256, digest, producer).
pub const HEADER_SCHEMA: &str = "rustred.campaign.zero-proof-header.v1";
/// Schema of a per-(record, key) answer of a zeros request (closeloop.md 23).
pub const ZERO_VERDICT_SCHEMA: &str = "rustred.campaign.zero-verdict.v1";
/// `program` / `version` of every proof line written here.
pub const PROGRAM: &str = "rr_coef_facts";
/// `method` of every proof line (described by [`METHOD_DOC`]).
pub const METHOD: &str = "structural.v1";
/// What `structural.v1` means.
pub const METHOD_DOC: &str = "structural: the term's numerator text read as a POLYNOMIAL (Symbolica try_to_polynomial), the record's \
     constants and the locus substituted exactly (replace / replace_with_poly for case equations solved for a unit \
     parameter), identically zero (is_zero); denominator classified on the locus (constant d-coefficient / factor test)";
/// The vendored Symbolica revision this producer is built from (recorded in every proof line).
pub const SYMBOLICA_REV: &str = "9cd57678";

/// Budgets of the prover (parameters, not constants of a node).
#[derive(Clone, Debug)]
pub struct ProverCfg {
    /// At most this many planes per (term, positive-numerator slot) are enumerated; more -> `not_proved` (budget).
    pub max_planes: usize,
    /// At most this many candidate loci are tried per (term, key) before the per-key evaluation.
    pub max_candidates: usize,
    /// A denominator restricted to a locus with more terms than this is not factored (`where_regular`).
    pub max_factor_terms: usize,
    /// Which outcomes are written as proof lines.
    pub report: Report,
}

impl Default for ProverCfg {
    fn default() -> Self {
        ProverCfg {
            max_planes: 64,
            max_candidates: 64,
            max_factor_terms: 400,
            report: Report::ProvedAndNotProved,
        }
    }
}

/// The outcome of one (term, locus).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Proved { den: DenStatus, equations: Vec<u32> },
    NotProved(String),
    Excluded(String),
}

#[derive(Clone, Debug)]
struct Term {
    num: ZPoly,
    den: Option<ZPoly>,
}

/// The tool object of a summary / version output.
pub fn tool() -> Value {
    json!({"program": PROGRAM, "version": env!("CARGO_PKG_VERSION"), "backend": "symbolica", "symbolica": SYMBOLICA_REV})
}

/// The compact tool word of every proof line: `program@version` (read by the core's loader for its allow-list).
pub fn tool_word() -> String {
    format!("{PROGRAM}@{}", env!("CARGO_PKG_VERSION"))
}

/// Which decided (term, locus) outcomes become proof lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Report {
    /// Proved loci only (what a consumer looks up).
    Proved,
    /// Proved and not proved (the coverage report of `prove`).
    ProvedAndNotProved,
    /// Every outcome, excluded loci included.
    All,
}

/// A record under proof: its constants, parameter bounds, equations, exceptions, term patterns, parsed terms and the
/// decided (term, locus) outcomes.
pub struct RecordProver<'r> {
    alg: &'r SymbolicaAlgebra,
    cfg: ProverCfg,
    pub rule: &'r RuleText,
    pub sha: String,
    arity: usize,
    dom: &'r CaseDomain,
    lines: Vec<bool>,
    /// Per parameter: Some(value) if constant over the record's domain.
    konst: Vec<Option<i64>>,
    lo: Vec<Option<i64>>,
    hi: Vec<Option<i64>>,
    consts: Vec<(usize, Integer)>,
    eqs: Option<Result<Vec<ZPoly>, String>>,
    excs: Option<Result<Vec<Vec<ZPoly>>, String>>,
    slots: Vec<Option<Vec<RhsSlot>>>,
    terms: HashMap<usize, Result<Term, String>>,
    pub outcomes: BTreeMap<(usize, Locus), (Outcome, f64)>,
    /// Outcomes decided since the last [`RecordProver::take_new`].
    fresh: Vec<(usize, Locus)>,
}

fn int(v: i64) -> Integer {
    Integer::from(v)
}

impl<'r> RecordProver<'r> {
    /// `lines`: the line mask of the frame (the chart orthant of the keys: line slots >= 1, numerator slots <= 0).
    pub fn new(
        alg: &'r SymbolicaAlgebra,
        cfg: ProverCfg,
        rule: &'r RuleText,
        lines: &[bool],
    ) -> Result<Self, String> {
        let dom = rule
            .domain
            .as_ref()
            .ok_or_else(|| format!("no domain: {}", rule.domain_error.as_deref().unwrap_or("?")))?;
        let n = dom.arity;
        if lines.len() != n {
            return Err(format!("frame of {} slots for arity {n}", lines.len()));
        }
        let sha = rule
            .line_sha256
            .strip_prefix("sha256:")
            .unwrap_or(&rule.line_sha256)
            .to_string();
        let mut konst: Vec<Option<i64>> = vec![None; n];
        let (mut lo, mut hi): (Vec<Option<i64>>, Vec<Option<i64>>) = (vec![None; n], vec![None; n]);
        let mut mapped = vec![false; n];
        let mut bound = |q: usize, k: usize, off: i64, konst: &mut Vec<Option<i64>>| {
            mapped[q] = true;
            if let Some(v) = dom.fixed[k] {
                konst[q] = Some(v as i64 - off);
            } else if lines[k] {
                let b = 1 - off;
                lo[q] = Some(lo[q].map_or(b, |x: i64| x.max(b)));
            } else {
                let b = -off;
                hi[q] = Some(hi[q].map_or(b, |x: i64| x.min(b)));
            }
        };
        match &dom.target {
            TargetMap::Canonical => {
                for k in 0..n {
                    bound(k, k, 0, &mut konst);
                }
            }
            TargetMap::Pattern(t) => {
                for (k, s) in t.iter().enumerate() {
                    if let TargetSlot::Param { param, offset } = *s
                        && param < n
                    {
                        bound(param, k, offset, &mut konst);
                    }
                }
            }
        }
        for q in 0..n {
            if !mapped[q] {
                konst[q] = Some(dom.fixed[q].unwrap_or(0) as i64);
            }
        }
        let consts = konst
            .iter()
            .enumerate()
            .filter_map(|(q, c)| c.map(|v| (q, int(v))))
            .collect();
        let slots = rule
            .rhs
            .iter()
            .map(|(p, _)| parse_rhs_pattern(p, n).ok())
            .collect();
        Ok(RecordProver {
            alg,
            cfg,
            rule,
            sha,
            arity: n,
            dom,
            lines: lines.to_vec(),
            konst,
            lo,
            hi,
            consts,
            eqs: None,
            excs: None,
            slots,
            terms: HashMap::new(),
            outcomes: BTreeMap::new(),
            fresh: vec![],
        })
    }

    pub fn n_terms(&self) -> usize {
        self.rule.rhs.len()
    }

    fn parse_side(&self, text: &str) -> Result<ZPoly, String> {
        let t = text.trim();
        side_budget(t)?;
        let alg = self.alg;
        let n = self.arity;
        let p = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            alg.parse_side_polynomial(t, n)
        }))
        .map_err(|_| "the polynomial parser failed on this side (refused)".to_string())??;
        if p.nvars() != n + 1 {
            return Err("variable map is not n0..n{N-1}, d".into());
        }
        Ok(p)
    }

    fn term(&mut self, j: usize) -> Result<&Term, String> {
        if !self.terms.contains_key(&j) {
            let r = match self.rule.rhs.get(j) {
                None => Err(format!("term {j} out of range")),
                Some((_, c)) => match split_ratio(c.trim()) {
                    Err(e) => Err(e),
                    Ok((n, d)) => self.parse_side(n).and_then(|num| {
                        let den = match d {
                            None => None,
                            Some(d) => Some(self.parse_side(d)?),
                        };
                        Ok(Term { num, den })
                    }),
                },
            };
            self.terms.insert(j, r);
        }
        self.terms[&j].as_ref().map_err(|e| e.clone())
    }

    fn equations(&mut self) -> Result<Vec<ZPoly>, String> {
        if self.eqs.is_none() {
            let r = self
                .rule
                .record
                .equations
                .iter()
                .map(|e| self.parse_side(e).map(|p| self.at_consts(&p)))
                .collect::<Result<Vec<_>, _>>();
            self.eqs = Some(r);
        }
        self.eqs.clone().expect("set")
    }

    fn exceptions(&mut self) -> Result<Vec<Vec<ZPoly>>, String> {
        if self.excs.is_none() {
            let r = self
                .rule
                .record
                .exceptions
                .iter()
                .map(|b| {
                    b.iter()
                        .map(|e| self.parse_side(e).map(|p| self.at_consts(&p)))
                        .collect::<Result<Vec<_>, _>>()
                })
                .collect::<Result<Vec<_>, _>>();
            self.excs = Some(r);
        }
        self.excs.clone().expect("set")
    }

    fn at_consts(&self, p: &ZPoly) -> ZPoly {
        let reps: Vec<(usize, &Integer)> = self.consts.iter().map(|(q, v)| (*q, v)).collect();
        p.replace_multiple(&reps, false)
    }

    fn at_locus(&self, p: &ZPoly, locus: Locus) -> ZPoly {
        let p = self.at_consts(p);
        match locus {
            Locus::Domain => p,
            Locus::Plane { param, value } => p.replace(param as usize, &int(value)),
        }
    }

    /// Decide (term j, locus) once (cached).
    pub fn prove(&mut self, j: usize, locus: Locus) -> Outcome {
        if let Some((o, _)) = self.outcomes.get(&(j, locus)) {
            return o.clone();
        }
        let t0 = Instant::now();
        let o = self.decide(j, locus);
        self.outcomes
            .insert((j, locus), (o.clone(), t0.elapsed().as_secs_f64()));
        self.fresh.push((j, locus));
        o
    }

    fn decide(&mut self, j: usize, locus: Locus) -> Outcome {
        if let Locus::Plane { param, value } = locus {
            if param as usize >= self.arity {
                return Outcome::NotProved(format!("parameter {param} out of range"));
            }
            if let Some(c) = self.konst[param as usize]
                && c != value
            {
                return Outcome::Excluded(format!("n{param} is the constant {c} on the record"));
            }
        }
        let term = match self.term(j) {
            Ok(t) => t.clone(),
            Err(e) => return Outcome::NotProved(format!("coefficient text: {e}")),
        };
        let eqs = match self.equations() {
            Ok(e) => e,
            Err(e) => return Outcome::NotProved(format!("case equation: {e}")),
        };
        let excs = match self.exceptions() {
            Ok(e) => e,
            Err(e) => return Outcome::NotProved(format!("exception: {e}")),
        };
        let dvar = self.arity;
        let mut num = self.at_locus(&term.num, locus);
        let mut den = term.den.as_ref().map(|d| self.at_locus(d, locus));
        let mut eq_l: Vec<ZPoly> = eqs
            .iter()
            .map(|e| match locus {
                Locus::Domain => e.clone(),
                Locus::Plane { param, value } => e.replace(param as usize, &int(value)),
            })
            .collect();
        let mut exc_l: Vec<Vec<ZPoly>> = excs
            .iter()
            .map(|b| {
                b.iter()
                    .map(|e| match locus {
                        Locus::Domain => e.clone(),
                        Locus::Plane { param, value } => e.replace(param as usize, &int(value)),
                    })
                    .collect()
            })
            .collect();
        let mut used: Vec<u32> = vec![];
        if eq_l.iter().any(|e| !e.is_zero() && e.is_constant()) {
            return Outcome::Excluded("a case equation is a non-zero constant on the locus".into());
        }
        if !num.is_zero() && !eq_l.is_empty() {
            // solve the equations for unit parameters, one after the other
            for i in 0..eq_l.len() {
                let e = eq_l[i].clone();
                if e.is_zero() {
                    continue;
                }
                if e.is_constant() {
                    return Outcome::Excluded(format!(
                        "case equation {i} is a non-zero constant on the locus"
                    ));
                }
                let Some((x, sol)) = unit_solution(&e, dvar) else {
                    continue;
                };
                num = num.replace_with_poly(x, &sol);
                den = den.map(|d| d.replace_with_poly(x, &sol));
                for f in eq_l.iter_mut() {
                    *f = f.replace_with_poly(x, &sol);
                }
                for b in exc_l.iter_mut() {
                    for f in b.iter_mut() {
                        *f = f.replace_with_poly(x, &sol);
                    }
                }
                used.push(i as u32);
                if eq_l.iter().any(|f| !f.is_zero() && f.is_constant()) {
                    return Outcome::Excluded(
                        "the case equations are inconsistent on the locus".into(),
                    );
                }
            }
        }
        if let Some(b) = exc_l
            .iter()
            .position(|b| !b.is_empty() && b.iter().all(ZPoly::is_zero))
        {
            return Outcome::Excluded(format!("exception branch {b} holds the whole locus"));
        }
        if !num.is_zero() {
            return Outcome::NotProved(
                "the numerator does not vanish identically on the locus".into(),
            );
        }
        let den_status = match &den {
            None => DenStatus::NowhereZero,
            Some(d) if d.is_zero() => {
                return Outcome::NotProved(
                    "the denominator vanishes identically on the locus (coefficient undefined)"
                        .into(),
                );
            }
            Some(d) => self.classify_den(d, &exc_l, locus),
        };
        Outcome::Proved {
            den: den_status,
            equations: used,
        }
    }

    fn classify_den(&self, d: &ZPoly, excs: &[Vec<ZPoly>], locus: Locus) -> DenStatus {
        let dvar = self.arity;
        if has_constant_d_coefficient(d, dvar) {
            return DenStatus::NowhereZero;
        }
        if d.nterms() > self.cfg.max_factor_terms {
            return DenStatus::WhereRegular;
        }
        let single: Vec<&ZPoly> = excs
            .iter()
            .filter(|b| b.len() == 1)
            .map(|b| &b[0])
            .filter(|e| !e.is_zero())
            .collect();
        for (f, _) in d.factor() {
            if f.is_constant() {
                continue;
            }
            if has_constant_d_coefficient(&f, dvar) {
                continue;
            }
            let dfree = f.degree(dvar) == 0;
            if dfree && single.iter().any(|e| divides(&f, e)) {
                continue;
            }
            if dfree && self.sign_definite(&f, locus) {
                continue;
            }
            return DenStatus::WhereRegular;
        }
        DenStatus::NowhereZero
    }

    /// A d-free polynomial of total degree <= 1 that never vanishes on the chart orthant (interval bounds per parameter).
    fn sign_definite(&self, f: &ZPoly, locus: Locus) -> bool {
        let (mut lo, mut hi) = (0i128, 0i128);
        let (mut lo_inf, mut hi_inf) = (false, false);
        for (c, e) in f.coefficients.iter().zip(f.exponents_iter()) {
            let Some(c) = c.to_i64() else {
                return false;
            };
            let c = c as i128;
            let deg: u32 = e.iter().map(|&x| u32::from(x)).sum();
            if deg == 0 {
                lo += c;
                hi += c;
                continue;
            }
            if deg != 1 {
                return false;
            }
            let q = e.iter().position(|&x| x == 1).expect("degree 1");
            if q >= self.arity {
                return false;
            }
            let (l, h) = match (self.konst[q], locus) {
                (Some(v), _) => (Some(v), Some(v)),
                (None, Locus::Plane { param, value }) if param as usize == q => {
                    (Some(value), Some(value))
                }
                _ => (self.lo[q], self.hi[q]),
            };
            let (cl, ch) = if c > 0 { (l, h) } else { (h, l) };
            match cl {
                Some(x) => lo += c * x as i128,
                None => lo_inf = true,
            }
            match ch {
                Some(x) => hi += c * x as i128,
                None => hi_inf = true,
            }
        }
        (!lo_inf && lo > 0) || (!hi_inf && hi < 0)
    }

    /// The positive-numerator loci of term j over the record's domain: (loci, budget / unbounded refusals).
    pub fn posnum_loci(&self, j: usize) -> (Vec<Locus>, Vec<String>) {
        let mut loci = vec![];
        let mut refused = vec![];
        let Some(Some(sl)) = self.slots.get(j) else {
            refused.push(format!("term {j}: unreadable pattern"));
            return (loci, refused);
        };
        for (i, s) in sl.iter().enumerate() {
            if self.lines[i] {
                continue;
            }
            match *s {
                RhsSlot::Const(c) => {
                    if c >= 1 {
                        loci.push(Locus::Domain);
                    }
                }
                RhsSlot::Param { param, offset } => {
                    if param >= self.arity {
                        refused.push(format!("term {j}: parameter {param} out of range"));
                        continue;
                    }
                    let need = 1 - offset as i64;
                    if let Some(c) = self.konst[param] {
                        if c >= need {
                            loci.push(Locus::Domain);
                        }
                        continue;
                    }
                    let from = self.lo[param].map_or(need, |l| l.max(need));
                    match self.hi[param] {
                        None => {
                            refused.push(format!(
                                "term {j} slot {i}: n{param} unbounded above (the locus is not a finite union of planes)"
                            ));
                            loci.push(Locus::Domain);
                        }
                        Some(h) if h < from => {}
                        Some(h) => {
                            if (h - from + 1) as usize > self.cfg.max_planes {
                                refused.push(format!(
                                    "term {j} slot {i}: {} planes above the budget {}",
                                    h - from + 1,
                                    self.cfg.max_planes
                                ));
                                loci.push(Locus::Domain);
                            } else {
                                for v in from..=h {
                                    loci.push(Locus::Plane {
                                        param: param as u32,
                                        value: v,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        loci.sort();
        loci.dedup();
        (loci, refused)
    }

    /// Whether term j can be a positive-numerator term somewhere on the domain.
    pub fn may_be_posnum(&self, j: usize) -> bool {
        let (l, r) = self.posnum_loci(j);
        !l.is_empty() || !r.is_empty()
    }

    /// Prove every positive-numerator locus of term j: true iff all are proved or excluded (a Domain proof covers all).
    pub fn prove_posnum(&mut self, j: usize) -> bool {
        let (loci, _refused) = self.posnum_loci(j);
        if loci.is_empty() {
            return true;
        }
        // the whole domain first (one proof covers every plane); an attempt that fails is not reported when the domain
        // is not itself a locus of the term (its planes are)
        let domain_needed = loci.contains(&Locus::Domain);
        if matches!(self.prove(j, Locus::Domain), Outcome::Proved { .. }) {
            return true;
        }
        if !domain_needed && self.fresh.last() == Some(&(j, Locus::Domain)) {
            self.fresh.pop();
        }
        let mut all = !loci.contains(&Locus::Domain);
        for l in loci {
            if l == Locus::Domain {
                continue;
            }
            if matches!(self.prove(j, l), Outcome::NotProved(_)) {
                all = false;
            }
        }
        all
    }

    /// The candidate loci of term j at rule point `pt` (term key `tk`): the domain, the planes of the slots the term makes
    /// positive, then the plane of every other non-constant parameter.
    fn candidates(&self, j: usize, pt: &[i32], tk: &[i32]) -> Vec<Locus> {
        let mut c = vec![Locus::Domain];
        if let Some(Some(sl)) = self.slots.get(j) {
            for (i, s) in sl.iter().enumerate() {
                if !self.lines[i]
                    && tk.get(i).is_some_and(|&x| x > 0)
                    && let RhsSlot::Param { param, .. } = *s
                    && param < self.arity
                    && self.konst[param].is_none()
                {
                    c.push(Locus::Plane {
                        param: param as u32,
                        value: pt[param] as i64,
                    });
                }
            }
        }
        for (q, &v) in pt.iter().enumerate().take(self.arity) {
            if self.konst[q].is_none() {
                let l = Locus::Plane {
                    param: q as u32,
                    value: v as i64,
                };
                if !c.contains(&l) {
                    c.push(l);
                }
            }
        }
        c.truncate(self.cfg.max_candidates.max(1));
        c
    }

    /// The exact per-key word of term j at rule point `pt` (coef_facts semantics).
    pub fn word_at(&mut self, j: usize, pt: &[i32]) -> CoefFact {
        let n = self.arity;
        let Ok(t) = self.term(j) else {
            return CoefFact::Unknown;
        };
        let vals: Vec<Integer> = pt.iter().map(|&x| int(x as i64)).collect();
        let reps: Vec<(usize, &Integer)> = vals.iter().enumerate().take(n).collect();
        if let Some(d) = &t.den
            && d.replace_multiple(&reps, true).is_zero()
        {
            return CoefFact::Unknown;
        }
        if t.num.replace_multiple(&reps, true).is_zero() {
            CoefFact::Zero
        } else {
            CoefFact::NonZero
        }
    }

    /// Whether the denominator of term j vanishes identically in d at rule point `pt`.
    fn den_vanishes_at(&mut self, j: usize, pt: &[i32]) -> bool {
        let n = self.arity;
        let Ok(t) = self.term(j) else {
            return true;
        };
        let Some(d) = &t.den else {
            return false;
        };
        let vals: Vec<Integer> = pt.iter().map(|&x| int(x as i64)).collect();
        let reps: Vec<(usize, &Integer)> = vals.iter().enumerate().take(n).collect();
        d.replace_multiple(&reps, true).is_zero()
    }

    /// The word of term j at `key` (structural first unless `per_key`), and how it was decided:
    /// `plane:<p>=<v>` / `domain` (a proof; a `where_regular` proof checks the denominator at the point: `+den`) or `key`.
    pub fn zero_word(
        &mut self,
        j: usize,
        pt: &[i32],
        tk: &[i32],
        per_key: bool,
    ) -> (CoefFact, String) {
        if !per_key {
            for l in self.candidates(j, pt, tk) {
                if let Outcome::Proved { den, .. } = self.prove(j, l) {
                    let by = match l {
                        Locus::Domain => "domain".to_string(),
                        Locus::Plane { param, value } => format!("plane:{param}={value}"),
                    };
                    return match den {
                        DenStatus::NowhereZero => (CoefFact::Zero, by),
                        DenStatus::WhereRegular => {
                            if self.den_vanishes_at(j, pt) {
                                (CoefFact::Unknown, format!("{by}+den"))
                            } else {
                                (CoefFact::Zero, format!("{by}+den"))
                            }
                        }
                    };
                }
            }
        }
        (self.word_at(j, pt), "key".into())
    }

    /// The proof lines of every outcome decided since the last call (as `cfg.report` selects).
    pub fn take_new(&mut self) -> Vec<Value> {
        let fresh = std::mem::take(&mut self.fresh);
        let report = self.cfg.report;
        fresh
            .into_iter()
            .filter_map(|(j, l)| {
                let (o, s) = &self.outcomes[&(j, l)];
                let keep = match (report, o) {
                    (_, Outcome::Proved { .. }) => true,
                    (Report::Proved, _) => false,
                    (Report::ProvedAndNotProved, Outcome::Excluded(_)) => false,
                    _ => true,
                };
                keep.then(|| proof_line(&self.sha, self.rule.index(), j, l, o, *s))
            })
            .collect()
    }

    /// The rule point of `key` (None: not in the record's case / no rule point).
    pub fn point(&self, key: &[i32]) -> Option<Vec<i32>> {
        let mut pt = vec![];
        rule_point(self.dom, key, &mut pt).then_some(pt)
    }

    /// The term key of term j at rule point `pt`.
    pub fn term_key(&self, j: usize, pt: &[i32]) -> Option<Vec<i32>> {
        let sl = self.slots.get(j)?.as_ref()?;
        sl.iter()
            .map(|s| match *s {
                RhsSlot::Param { param, offset } => pt.get(param)?.checked_add(offset),
                RhsSlot::Const(c) => Some(c),
            })
            .collect()
    }

    pub fn domain(&self) -> &CaseDomain {
        self.dom
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The INDEPENDENT re-check of the verifier (interfaces/zero-proofs.md 5). DESIGN REASON, not a Symbolica gap (L11): the
// verifier re-derives every proof on Symbolica (the prover's algebra) AND re-checks the numerator claim with the
// campaign core's own exact polynomial (`poly::Poly`, parsed by the core's own parser, exact `BigInt` coefficients):
// substituting integer values into a sparse polynomial and testing that every coefficient cancels is ~30 lines and shares
// no code with Symbolica, so a parser / substitution defect of either side shows up as a disagreement.
// ---------------------------------------------------------------------------------------------------------------------

/// The core polynomial of a side text with the values `subs` substituted: the surviving monomials (without the
/// substituted variables) and their exact coefficients (zero coefficients dropped).
pub fn core_substitute(
    text: &str,
    arity: usize,
    subs: &[(usize, i64)],
) -> Result<BTreeMap<Vec<(u16, u32)>, rustred_campaign_core::poly::BigInt>, String> {
    use rustred_campaign_core::poly::{BigInt, Poly};
    let p =
        Poly::parse_exact(text.trim(), arity, true).map_err(|e| format!("core parse: {e:?}"))?;
    let val: HashMap<u16, i64> = subs.iter().map(|&(q, v)| (q as u16, v)).collect();
    let mut acc: BTreeMap<Vec<(u16, u32)>, BigInt> = BTreeMap::new();
    for (m, c) in p.exact_terms() {
        let mut c = c;
        let mut rest = Vec::with_capacity(m.len());
        for (var, e) in m {
            match val.get(&var) {
                Some(&v) => {
                    let b = BigInt::from_i128(v as i128);
                    for _ in 0..e {
                        c = c.mul(&b);
                    }
                }
                None => rest.push((var, e)),
            }
        }
        let e = acc.entry(rest).or_insert_with(BigInt::zero);
        *e = e.add(&c);
    }
    acc.retain(|_, c| !c.is_zero());
    Ok(acc)
}

/// Independent: the numerator text vanishes identically after the substitutions.
pub fn core_vanishes(text: &str, arity: usize, subs: &[(usize, i64)]) -> Result<bool, String> {
    Ok(core_substitute(text, arity, subs)?.is_empty())
}

/// Independent: after the substitutions some power of d has a non-zero CONSTANT coefficient (the denominator never
/// vanishes identically in d).
pub fn core_constant_d_coefficient(
    text: &str,
    arity: usize,
    subs: &[(usize, i64)],
) -> Result<bool, String> {
    use rustred_campaign_core::poly::D_VAR;
    let m = core_substitute(text, arity, subs)?;
    let mut by_d: BTreeMap<u32, (bool, bool)> = BTreeMap::new(); // d power -> (has pure term, has a term with n)
    for mono in m.keys() {
        let dpow = mono.iter().find(|(v, _)| *v == D_VAR).map_or(0, |x| x.1);
        let has_n = mono.iter().any(|(v, _)| *v != D_VAR);
        let e = by_d.entry(dpow).or_insert((false, false));
        if has_n {
            e.1 = true;
        } else {
            e.0 = true;
        }
    }
    Ok(by_d.values().any(|&(pure, n)| pure && !n))
}

impl RecordProver<'_> {
    /// The substitutions of a locus: the record's constants and the plane.
    pub fn substitutions(&self, locus: Locus) -> Vec<(usize, i64)> {
        let mut s: Vec<(usize, i64)> = self
            .konst
            .iter()
            .enumerate()
            .filter_map(|(q, c)| c.map(|v| (q, v)))
            .collect();
        if let Locus::Plane { param, value } = locus
            && !s.iter().any(|(q, _)| *q == param as usize)
        {
            s.push((param as usize, value));
        }
        s
    }

    /// The numerator / denominator texts of term j (split_ratio forms).
    pub fn side_texts(&self, j: usize) -> Result<(String, Option<String>), String> {
        let (_, c) = self.rule.rhs.get(j).ok_or("term out of range")?;
        let (n, d) = split_ratio(c.trim())?;
        Ok((n.to_string(), d.map(str::to_string)))
    }

    pub fn arity(&self) -> usize {
        self.arity
    }
}

/// A proof line (interfaces/zero-proofs.md 2).
pub fn proof_line(
    sha: &str,
    rule: Option<i64>,
    j: usize,
    l: Locus,
    o: &Outcome,
    secs: f64,
) -> Value {
    let mut v = json!({"schema": ZERO_PROOF_SCHEMA, "record_sha256": sha, "rule": rule, "term": j, "locus": l.json(),
        "method": METHOD, "tool": tool_word(), "seconds": (secs * 1e6).round() / 1e6});
    match o {
        Outcome::Proved { den, equations } => {
            v["verdict"] = json!(VERDICT_PROVED);
            v["denominator"] = json!(den.word());
            v["equations"] = json!(equations);
        }
        Outcome::NotProved(r) => {
            v["verdict"] = json!("not_proved");
            v["reason"] = json!(r);
        }
        Outcome::Excluded(r) => {
            v["verdict"] = json!("excluded");
            v["reason"] = json!(r);
        }
    }
    v
}

/// e = s*x + r with s = +-1 a constant and r free of x (x != d): x = -s*r.
fn unit_solution(e: &ZPoly, dvar: usize) -> Option<(usize, ZPoly)> {
    for x in 0..e.nvars() {
        if x == dvar || e.degree(x) != 1 {
            continue;
        }
        let parts = e.to_univariate_polynomial_list(x);
        let mut r = e.zero();
        let mut s: Option<ZPoly> = None;
        for (c, deg) in parts {
            if deg == 0 {
                r = c;
            } else {
                s = Some(c);
            }
        }
        let s = s?;
        if !s.is_constant() {
            continue;
        }
        let sc = s.lcoeff();
        if sc == int(1) {
            return Some((x, -r));
        }
        if sc == int(-1) {
            return Some((x, r));
        }
    }
    None
}

fn has_constant_d_coefficient(p: &ZPoly, dvar: usize) -> bool {
    if p.is_zero() {
        return false;
    }
    if p.degree(dvar) == 0 {
        return p.is_constant();
    }
    p.to_univariate_polynomial_list(dvar)
        .iter()
        .any(|(c, _)| !c.is_zero() && c.is_constant())
}

/// Whether f divides e exactly (both non-zero).
fn divides(f: &ZPoly, e: &ZPoly) -> bool {
    if f.is_zero() || e.is_zero() {
        return false;
    }
    let (_, r) = e.quot_rem(f, false);
    r.is_zero()
}

/// The answer line of a zeros request at (record, key) (closeloop.md 23).
#[allow(clippy::too_many_arguments)]
pub fn verdict_line(
    sha: &str,
    rule: Option<i64>,
    key: &[i32],
    in_case: bool,
    branch: Option<usize>,
    n_terms: usize,
    words: &[(usize, CoefFact, String)],
    per_key: bool,
    secs: f64,
) -> Value {
    let w = |c: &CoefFact| match c {
        CoefFact::Zero => "zero",
        CoefFact::NonZero => "nonzero",
        CoefFact::Unknown => "unknown",
    };
    json!({"schema": ZERO_VERDICT_SCHEMA, "producer": "exact_eval", "backend": "symbolica",
        "method": if per_key { "zeros:per_key" } else { "zeros:structural" },
        "record_sha256": sha, "rule": rule, "index": rule, "key": key, "in_case": in_case, "branch": branch,
        "n_terms": n_terms,
        "terms": words.iter().map(|(j, c, by)| json!([j, w(c), by])).collect::<Vec<_>>(),
        "seconds": (secs * 1e3).round() / 1e3})
}

/// Answer one zeros request (one record at its keys, the asked terms per key): the per-key verdict lines and the proof
/// lines of every (term, locus) decided on the way.
pub fn answer_record(
    p: &mut RecordProver<'_>,
    keys: &[Vec<i32>],
    terms: &[Vec<usize>],
    per_key: bool,
) -> (Vec<Value>, Vec<Value>) {
    let mut verdicts = vec![];
    let mut proofs = vec![];
    let n_terms = p.n_terms();
    for (k, asked) in keys.iter().zip(terms) {
        let t0 = Instant::now();
        let (in_case, branch) = match p.domain().applies(k) {
            Verdict::Applies
            | Verdict::Elided
            | Verdict::WitnessNotCarried
            | Verdict::OnPole { .. } => (true, None),
            Verdict::OnException { branch } => (true, Some(branch)),
            _ => (false, None),
        };
        let mut words = vec![];
        match (in_case && branch.is_none()).then(|| p.point(k)).flatten() {
            None => {
                for &j in asked {
                    words.push((j, CoefFact::Unknown, "unusable".to_string()));
                }
            }
            Some(pt) => {
                for &j in asked {
                    let (c, by) = match p.term_key(j, &pt) {
                        Some(tk) => p.zero_word(j, &pt, &tk, per_key),
                        None => (CoefFact::Unknown, "pattern".to_string()),
                    };
                    words.push((j, c, by));
                }
            }
        }
        proofs.extend(p.take_new());
        verdicts.push(verdict_line(
            &p.sha,
            p.rule.index(),
            k,
            in_case,
            branch,
            n_terms,
            &words,
            per_key,
            t0.elapsed().as_secs_f64(),
        ));
    }
    (verdicts, proofs)
}

/// Counts of [`prove_posnum_record`].
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct PosnumCounts {
    pub records: u64,
    pub records_refused: u64,
    pub posnum_terms: u64,
    pub terms_fully_proved: u64,
    pub loci: u64,
    pub proved: u64,
    pub proved_nowhere_zero: u64,
    pub not_proved: u64,
    pub excluded: u64,
    pub budget_refusals: u64,
}

impl PosnumCounts {
    pub fn add(&mut self, o: &PosnumCounts) {
        self.records += o.records;
        self.records_refused += o.records_refused;
        self.posnum_terms += o.posnum_terms;
        self.terms_fully_proved += o.terms_fully_proved;
        self.loci += o.loci;
        self.proved += o.proved;
        self.proved_nowhere_zero += o.proved_nowhere_zero;
        self.not_proved += o.not_proved;
        self.excluded += o.excluded;
        self.budget_refusals += o.budget_refusals;
    }
}

/// Prove every positive-numerator locus of every term of one record; the proof lines (one per decided (term, locus)).
pub fn prove_posnum_record(p: &mut RecordProver<'_>, c: &mut PosnumCounts) -> Vec<Value> {
    c.records += 1;
    for j in 0..p.n_terms() {
        let (loci, refused) = p.posnum_loci(j);
        if loci.is_empty() && refused.is_empty() {
            continue;
        }
        c.posnum_terms += 1;
        c.budget_refusals += refused.len() as u64;
        if p.prove_posnum(j) {
            c.terms_fully_proved += 1;
        }
    }
    for (j, l) in p.fresh.clone() {
        c.loci += 1;
        match &p.outcomes[&(j, l)].0 {
            Outcome::Proved { den, .. } => {
                c.proved += 1;
                if *den == DenStatus::NowhereZero {
                    c.proved_nowhere_zero += 1;
                }
            }
            Outcome::Excluded(_) => c.excluded += 1,
            Outcome::NotProved(_) => c.not_proved += 1,
        }
    }
    p.take_new()
}
