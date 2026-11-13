//! scheme-certs: build the stored certificate of a replay-only Scheme rule with
//! RustRed's own machinery (development tooling; the shipped dispatch is not
//! touched).
//!
//! For one rule `Z(n) = Sum_j c_j Z(n + s_j)` (Forcer's terms) on its domain:
//! - the relation `R = L (Z(n) - Sum_j c_j Z(n + s_j))`, `L` the product of the
//!   distinct Forcer denominator factors (each at its largest power), in
//!   presentation coordinates and RustRed's sign, then restricted to the fixed
//!   slots of the pattern (as [`super::SchemeTemplate::extract`] restricts);
//! - a frame of RustRed's ordinary IBP rows `Row_(i,j)(n + t)` (the generator of
//!   the presentation, translated, restricted), with the reviewer's condition:
//!   no placement whose coefficient is nonzero at an ISP exponent >= 1 for some
//!   key of the domain (such a placement is dropped, checked layer by layer);
//! - the forbidden columns `F` = every frame column outside the support of `R`,
//!   the target the zero shift: the modular circuit choice
//!   ([`select_projected_source_circuit`]) and the exact source-weight
//!   reconstruction ([`reconstruct_projected_source_weights`], whose full
//!   product `W * A == r` is checked exactly in characteristic zero);
//! - the reconstructed row must be `R / R_0` exactly (else: another relation
//!   on the same support, reported); `lambda_i = R_0 w_i`, the common
//!   denominator `D` of the `lambda_i` (1: polynomial weights) multiplies the
//!   relation, `D R = Sum_i (D lambda_i) Row_i` identically, re-checked here
//!   term by term; the d-free factors of `D` are the pole layers to replay.
//!
//! The record is the scheme file's own format (`relation`, `certificate` with
//! `row` generators, Forcer slots, Forcer's `(-1)^(Sum s)` sign), so that the
//! program build ([`super::SchemeTemplate::extract`]) verifies it again,
//! independently of this search.
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use symbolica::domains::SelfRing;
use symbolica::poly::factor::Factorize;

use crate::algebra::{
    ExactAlgebraLimits, IndexedAlgebraLimits, IndexedCoefficient, IndexedCoefficientContext,
};
use crate::identity::{IndexShift, ParametricIbpGenerator, RowId};
use crate::solver::{
    ProjectedSourceWeightLimits, reconstruct_projected_source_weights,
    select_projected_source_circuit,
};

use super::super::error::PError;
use super::super::lane::{LaneRoles, OwnerLane};
use super::super::library::{PLibrary, POwnerSpec};
use super::super::presentation::Slot;
use super::{Builder, Cond, ParsedRule, layout, parity, scheme_at};

/// The search policy of one rule.
#[derive(Clone, Debug)]
pub struct CertifyOptions {
    /// Frame levels tried in order: 0 = every generator at the relation's
    /// shifts, 1 = every placement touching the relation's support, 2 = level
    /// 0 and the unit neighbours of the shifts, 3 = levels 1 and 2, 4 = level 2
    /// and every IBP row within L1 distance 2 of a shift, 5 = levels 1 and 4.
    pub levels: Vec<u8>,
    /// Largest frame (row placements) built at one level.
    pub max_rows: usize,
    /// Modular circuit choices tried per level (different random points).
    pub seeds: u64,
    pub max_degree: u16,
    /// Also search rules that carry a certificate (validation of the search
    /// against the stored ones); their stored certificate is ignored.
    pub redo: bool,
    /// Verified rules (stored and found here) are generators.
    pub chain: bool,
    /// Print the reconstruction's progress on stderr.
    pub verbose: bool,
    /// Search rules with eq: conditions on their whole box (see `certify`).
    pub eq_free: bool,
}

impl Default for CertifyOptions {
    fn default() -> Self {
        Self {
            levels: vec![0, 1, 2, 3],
            max_rows: 150_000,
            seeds: 2,
            max_degree: 64,
            redo: false,
            chain: true,
            verbose: false,
            eq_free: false,
        }
    }
}

/// The result for one rule.
#[derive(Clone, Debug, serde::Serialize)]
pub struct CertifyOutcome {
    pub rule: String,
    /// `certificate`, `no_circuit`, `other_relation`, `unsupported`, `error`.
    pub status: String,
    pub detail: String,
    pub level: Option<u8>,
    pub frame_rows: usize,
    pub frame_columns: usize,
    pub dropped_isp_placements: usize,
    pub circuit_rows: usize,
    pub relation_terms: usize,
    /// Free variables of the restricted problem (d and the free slots).
    pub variables: usize,
    /// Largest total degree of a certificate weight in the indices, and in d.
    pub weight_degree_n: u32,
    pub weight_degree_d: u32,
    /// `1` for polynomial weights; else the multiplier `D` of the relation.
    pub multiplier: String,
    /// The d-free irreducible factors of `D` (pole layers to replay).
    pub pole_layers: Vec<String>,
    pub seconds: f64,
    /// `{"relation": .., "certificate": ..}` in the scheme file's format.
    pub record: Option<serde_json::Value>,
}

impl CertifyOutcome {
    fn new(rule: &str) -> Self {
        Self {
            rule: rule.to_owned(),
            status: String::new(),
            detail: String::new(),
            level: None,
            frame_rows: 0,
            frame_columns: 0,
            dropped_isp_placements: 0,
            circuit_rows: 0,
            relation_terms: 0,
            variables: 0,
            weight_degree_n: 0,
            weight_degree_d: 0,
            multiplier: String::new(),
            pole_layers: Vec::new(),
            seconds: 0.0,
            record: None,
        }
    }
}

fn alg(error: impl std::fmt::Display) -> PError {
    PError::Generation(format!("scheme certificate: {error}"))
}

/// The replay-only rules of the owner's scheme (no certificate, not the corner,
/// not a permutation), in the stored order; with `redo`, every rule with
/// Forcer terms (not the corner, not a permutation).
pub fn uncertified_rules(
    library: &PLibrary,
    owner: &POwnerSpec,
    redo: bool,
) -> Result<Vec<String>, PError> {
    let (_, node) = owner_node(library, owner)?;
    let scheme = scheme_at(node)?;
    Ok((scheme.rules.iter())
        .filter(|rule| {
            !rule.master
                && rule.permutation.is_none()
                && (rule.certificate.is_none() || (redo && !rule.raw.is_empty()))
        })
        .map(|rule| rule.name.clone())
        .collect())
}

fn owner_node<'a>(
    library: &'a PLibrary,
    owner: &POwnerSpec,
) -> Result<(usize, &'a super::super::topology::NodeInfo), PError> {
    let LaneRoles::Scheme(_) = owner.roles else {
        return Err(PError::Contract("not a scheme owner".to_owned()));
    };
    if owner.lane != OwnerLane::Scheme {
        return Err(PError::Contract("not a scheme owner".to_owned()));
    }
    let index = owner
        .node
        .ok_or_else(|| PError::Contract("a scheme owner without a node".to_owned()))?;
    let dag = library
        .topology()
        .ok_or_else(|| PError::Contract("a scheme owner without a topology".to_owned()))?;
    let node = dag
        .nodes
        .get(index.0 as usize)
        .ok_or_else(|| PError::Contract(format!("scheme node {}", index.0)))?;
    Ok((index.0 as usize, node))
}

/// Certify the named rules of the owner's scheme, in the given order (the
/// stored order: a rule certified here becomes a generator of the later ones,
/// as the program build allows, inside its own domain only).
pub fn certify_rules(
    library: &PLibrary,
    owner: &POwnerSpec,
    names: &[String],
    options: &CertifyOptions,
    mut report: impl FnMut(&CertifyOutcome),
) -> Result<Vec<CertifyOutcome>, PError> {
    let (_, node) = owner_node(library, owner)?;
    let scheme = scheme_at(node)?;
    let p = library.presentation(owner.presentation);
    let layout = layout(scheme, &node.topo)?;
    if layout.slots != p.slots {
        return Err(PError::Contract(
            "the presentation is not the scheme's layout".to_owned(),
        ));
    }
    let generator = ParametricIbpGenerator::try_new(&p.family).map_err(alg)?;
    let batch = generator.prepare_ordinary_ibp().map_err(alg)?;
    let rows = (0..batch.len()).map(|row| batch.generate(row)).collect();
    let relations = batch.complete(rows).map_err(alg)?.into_relations();
    let context = generator.context().clone();
    let base = p.family.coefficient_context().clone();
    let d = base.parameter("d").ok_or(PError::ContextMismatch)?;
    let builder = Builder {
        context: &context,
        base: &base,
        perm: &layout.perm,
        d,
    };
    let mut gens: Vec<Gen> = Vec::new();
    for relation in &relations {
        if let RowId::OrdinaryIbp {
            contraction_momentum,
            differentiated_loop,
        } = relation.row_id()
        {
            gens.push(Gen {
                label: GenLabel::Row(*differentiated_loop, *contraction_momentum),
                terms: (relation.terms().iter())
                    .map(|(shift, value)| (shift.values().to_vec(), value.clone()))
                    .collect(),
                domain: None,
            });
        }
    }
    // Rules certified in the file AND rebuilt by the program build's check
    // (SchemeTemplate::extract) are generators, unless redone here; never one
    // with an eq:/ne: condition.
    if options.chain {
        let template = super::SchemeTemplate::extract(p, node).map_err(PError::from)?;
        let verified: BTreeSet<&str> = (template.rules().iter())
            .filter(|rule| rule.verification == super::Verification::Certificate)
            .map(|rule| rule.name.as_str())
            .collect();
        for rule in &scheme.rules {
            if !verified.contains(rule.name.as_str())
                || rule.certificate.is_none()
                || rule.relation.is_empty()
                || !rule.equal.is_empty()
                || !rule.distinct.is_empty()
                || (options.redo && names.contains(&rule.name))
            {
                continue;
            }
            let mut terms = Vec::new();
            for (shift, poly) in &rule.relation {
                terms.push((builder.shift(shift), builder.poly(poly, parity(shift))?));
            }
            gens.push(Gen {
                label: GenLabel::Rule(rule.name.clone()),
                terms,
                domain: Some(rule.pattern.clone()),
            });
        }
    }
    let lines = (p.slots.iter())
        .filter(|slot| matches!(slot, Slot::Line(_)))
        .count();
    let variables = Variables::new(&context, &builder, &layout.perm)?;
    let mut out = Vec::new();
    for name in names {
        let started = Instant::now();
        let mut outcome = CertifyOutcome::new(name);
        match scheme.rules.iter().find(|rule| &rule.name == name) {
            None => {
                outcome.status = "error".to_owned();
                outcome.detail = "no such rule".to_owned();
            }
            Some(rule) => {
                let problem = Problem {
                    context: &context,
                    builder: &builder,
                    perm: &layout.perm,
                    lines,
                    gens: &gens,
                    variables: &variables,
                };
                match problem.certify(rule, options, &mut outcome) {
                    Err(error) => {
                        outcome.status = "error".to_owned();
                        outcome.detail = format!("{} | {error}", outcome.detail);
                    }
                    Ok(Some(relation))
                        if options.chain && rule.equal.is_empty() && rule.distinct.is_empty() =>
                    {
                        gens.push(Gen {
                            label: GenLabel::Rule(rule.name.clone()),
                            terms: relation.into_iter().collect(),
                            domain: Some(rule.pattern.clone()),
                        });
                    }
                    Ok(_) => {}
                }
            }
        }
        outcome.seconds = started.elapsed().as_secs_f64();
        report(&outcome);
        out.push(outcome);
    }
    Ok(out)
}

/// A generator of a certificate: an ordinary IBP row, or a rule verified
/// before (used only where the target's domain, shifted, lies in its own).
struct Gen {
    label: GenLabel,
    /// `(shift, coefficient)`, presentation coordinates, RustRed sign; a rule's
    /// relation unrestricted.
    terms: Vec<(Vec<i64>, IndexedCoefficient)>,
    /// A rule's Forcer pattern.
    domain: Option<Vec<Cond>>,
}

#[derive(Clone, Debug)]
enum GenLabel {
    Row(usize, usize),
    Rule(String),
}

/// Native variable positions of `d` and of every presentation slot.
struct Variables {
    count: usize,
    d: usize,
    /// presentation slot -> native variable position
    slot: Vec<usize>,
    /// presentation slot -> Forcer slot (0-based)
    forcer: Vec<usize>,
}

fn single_variable(value: &IndexedCoefficient) -> Option<usize> {
    let numerator = &value.raw().numerator;
    let monomial = numerator.into_iter().next()?;
    if numerator.nterms() != 1 {
        return None;
    }
    let mut found = None;
    for (position, &e) in monomial.exponents.iter().enumerate() {
        if e == 1 && found.is_none() {
            found = Some(position);
        } else if e != 0 {
            return None;
        }
    }
    found
}

impl Variables {
    fn new(
        context: &IndexedCoefficientContext,
        builder: &Builder<'_>,
        perm: &[usize],
    ) -> Result<Self, PError> {
        let count = context.one().raw().numerator.get_vars_ref().len();
        let d = single_variable(&context.lift(&builder.d).map_err(alg)?)
            .ok_or_else(|| alg("d is not one native variable"))?;
        let mut slot = Vec::with_capacity(perm.len());
        for s in 0..perm.len() {
            slot.push(
                single_variable(&context.index(s).map_err(alg)?)
                    .ok_or_else(|| alg("an index is not one native variable"))?,
            );
        }
        let mut forcer = vec![0; perm.len()];
        for (a, &s) in perm.iter().enumerate() {
            forcer[s] = a;
        }
        Ok(Self {
            count,
            d,
            slot,
            forcer,
        })
    }

    /// `value` (a polynomial up to an integer denominator) as a Forcer
    /// `PolyRecord` (exponents `[d, n_1 .. n_14]`, `"num"` or `"num/den"`).
    fn record(&self, value: &IndexedCoefficient) -> Result<serde_json::Value, PError> {
        let raw = value.raw();
        if !raw.denominator.is_constant() {
            return Err(alg("a non-polynomial value in a record"));
        }
        if raw.numerator.get_vars_ref().len() != self.count {
            return Err(alg("a value with another variable map"));
        }
        let den = raw.denominator.lcoeff();
        let mut terms = Vec::new();
        for monomial in &raw.numerator {
            let mut exponents = vec![0u32; self.forcer.len() + 1];
            for (position, &e) in monomial.exponents.iter().enumerate() {
                if e == 0 {
                    continue;
                }
                if position == self.d {
                    exponents[0] = u32::from(e);
                } else if let Some(s) = self.slot.iter().position(|&v| v == position) {
                    exponents[self.forcer[s] + 1] = u32::from(e);
                } else {
                    return Err(alg("a value in a variable that is neither d nor an index"));
                }
            }
            let num = monomial.coefficient.clone();
            let g = num.gcd(&den);
            let (mut n, mut q) = (&num / &g, &den / &g);
            if q.is_negative() {
                n = -n;
                q = -q;
            }
            let text = if q.is_one() {
                n.to_string()
            } else {
                format!("{n}/{q}")
            };
            for part in text.split('/') {
                part.parse::<i64>()
                    .map_err(|_| alg(format!("coefficient {text} does not fit i64")))?;
            }
            terms.push(serde_json::json!([exponents, text]));
        }
        Ok(serde_json::Value::Array(terms))
    }

    /// `(degree in the indices, degree in d)` of a polynomial value.
    fn degrees(&self, value: &IndexedCoefficient) -> (u32, u32) {
        let (mut dn, mut dd) = (0u32, 0u32);
        for monomial in &value.raw().numerator {
            let mut n = 0u32;
            for (position, &e) in monomial.exponents.iter().enumerate() {
                if position == self.d {
                    dd = dd.max(u32::from(e));
                } else {
                    n += u32::from(e);
                }
            }
            dn = dn.max(n);
        }
        (dn, dd)
    }
}

struct Problem<'a> {
    context: &'a IndexedCoefficientContext,
    builder: &'a Builder<'a>,
    perm: &'a [usize],
    lines: usize,
    gens: &'a [Gen],
    variables: &'a Variables,
}

type Map = BTreeMap<Vec<i64>, IndexedCoefficient>;

impl Problem<'_> {
    fn limits() -> IndexedAlgebraLimits {
        IndexedAlgebraLimits::unbounded_resources()
    }

    fn shift(&self, values: &[i64]) -> Result<IndexShift, PError> {
        IndexShift::try_new(values.iter().copied(), values.len()).map_err(alg)
    }

    /// The common denominator of `values`: `D` with every `D * v` polynomial
    /// (integer denominators allowed).
    fn common_denominator<'v>(
        &self,
        values: impl IntoIterator<Item = &'v IndexedCoefficient>,
    ) -> Result<IndexedCoefficient, PError> {
        let c = self.context;
        let mut multiplier = c.one();
        for value in values {
            let scaled = c.mul(value, &multiplier).map_err(alg)?;
            let den = &scaled.raw().denominator;
            if den.is_constant() {
                continue;
            }
            let factor = crate::algebra::Coefficient {
                numerator: den.clone(),
                denominator: den.one(),
            };
            let factor = c
                .admit_native_result_with_limits(factor, ExactAlgebraLimits::unbounded_resources())
                .map_err(alg)?;
            multiplier = c.mul(&multiplier, &factor).map_err(alg)?;
        }
        Ok(multiplier)
    }

    /// Search one rule. On success the outcome carries the record and the
    /// cleared relation (unrestricted, presentation coordinates) is returned,
    /// for later rules to use as a generator.
    fn certify(
        &self,
        rule: &ParsedRule,
        options: &CertifyOptions,
        outcome: &mut CertifyOutcome,
    ) -> Result<Option<Map>, PError> {
        let c = self.context;
        let arity = self.perm.len();
        if rule.master || rule.permutation.is_some() || rule.raw.is_empty() {
            outcome.status = "unsupported".to_owned();
            outcome.detail = "master, permutation or a rule without Forcer terms".to_owned();
            return Ok(None);
        }
        if rule.certificate.is_some() && !options.redo {
            outcome.status = "unsupported".to_owned();
            outcome.detail = "the rule already has a certificate".to_owned();
            return Ok(None);
        }
        if !rule.equal.is_empty() && !options.eq_free {
            outcome.status = "unsupported".to_owned();
            outcome.detail = "an eq: slot condition (the domain is not a box)".to_owned();
            return Ok(None);
        }
        // With `eq_free`, a rule with eq: conditions is searched on the whole box
        // (the program build does not restrict equal slots): a certificate found
        // holds identically there, so also on the diagonal.
        // The domain in presentation slots.
        let mut pattern = vec![
            Cond {
                lo: i64::MIN,
                hi: i64::MAX
            };
            arity
        ];
        for (a, &cond) in rule.pattern.iter().enumerate() {
            pattern[self.perm[a]] = cond;
        }
        for (slot, cond) in pattern.iter_mut().enumerate() {
            // Apply keys: lines >= 1, ISPs <= 0.
            if slot < self.lines {
                cond.lo = cond.lo.max(1);
            } else {
                cond.hi = cond.hi.min(0);
            }
        }
        // The fixed slots, as the program build restricts (the stored pattern).
        let fixed: Vec<(usize, i64)> = (rule.pattern.iter().enumerate())
            .filter_map(|(a, cond)| cond.fixed().map(|v| (self.perm[a], v)))
            .collect();
        let restrict = |value: &IndexedCoefficient| -> Result<IndexedCoefficient, PError> {
            if fixed.is_empty() {
                return Ok(value.clone());
            }
            Ok(c.specialize_fixed_indices(value, &fixed, Self::limits())
                .map_err(alg)?
                .0)
        };
        // Forcer's terms -> the cleared relation (presentation coordinates, RustRed sign).
        let mut factors: Vec<(IndexedCoefficient, u32)> = Vec::new();
        let mut raw = Vec::new();
        for (shift, num, dens) in &rule.raw {
            let mut product = c.one();
            for (den, power) in dens {
                let den = self.builder.poly(den, 1)?;
                for _ in 0..*power {
                    product = c.mul(&product, &den).map_err(alg)?;
                }
                match factors
                    .iter_mut()
                    .find(|(f, _)| c.sub(f, &den).is_ok_and(|x| x.is_zero()))
                {
                    Some(entry) => entry.1 = entry.1.max(*power),
                    None => factors.push((den, *power)),
                }
            }
            raw.push((
                self.builder.shift(shift),
                self.builder.poly(num, parity(shift))?,
                product,
            ));
        }
        let mut lcm = c.one();
        for (factor, power) in &factors {
            for _ in 0..*power {
                lcm = c.mul(&lcm, factor).map_err(alg)?;
            }
        }
        let zero = vec![0_i64; arity];
        let mut relation: Map = BTreeMap::new();
        relation.insert(zero.clone(), lcm.clone());
        for (shift, num, product) in &raw {
            let term = c
                .div(&c.mul(&lcm, num).map_err(alg)?, product)
                .map_err(alg)?;
            let entry = relation.entry(shift.clone()).or_insert_with(|| c.zero());
            *entry = c.sub(entry, &term).map_err(alg)?;
        }
        relation.retain(|_, value| !value.is_zero());
        let clear = self.common_denominator(relation.values())?;
        if !clear.raw().is_one() {
            for value in relation.values_mut() {
                *value = c.mul(value, &clear).map_err(alg)?;
            }
        }
        let mut restricted: Map = BTreeMap::new();
        for (shift, value) in &relation {
            let value = restrict(value)?;
            if !value.is_zero() {
                restricted.insert(shift.clone(), value);
            }
        }
        if !restricted.contains_key(&zero) {
            outcome.status = "error".to_owned();
            outcome.detail = "the restricted pivot vanishes".to_owned();
            return Ok(None);
        }
        outcome.relation_terms = restricted.len();
        let support: BTreeSet<Vec<i64>> = restricted.keys().cloned().collect();
        // A fresh column only the relation row has: a frame row combination
        // that cancels every physical column against the relation is a
        // certificate `Sum w_i Row_i = R` (weight 1 on the relation row).
        let tau = self.shift(&vec![1_000_000_i64; arity])?;
        let mut relation_row = BTreeMap::new();
        relation_row.insert(tau.clone(), c.one());
        for (shift, value) in &restricted {
            relation_row.insert(
                self.shift(shift)?,
                c.neg_with_limits(value, ExactAlgebraLimits::unbounded_resources())
                    .map_err(alg)?,
            );
        }

        let mut notes = Vec::new();
        let mut status = "no_circuit";
        for &level in &options.levels {
            let Some(placements) = self.placements(level, rule, &support, options.max_rows) else {
                notes.push(format!(
                    "L{level}: more than {} placements",
                    options.max_rows
                ));
                continue;
            };
            // The frame.
            let mut frame: Vec<BTreeMap<IndexShift, IndexedCoefficient>> = Vec::new();
            let mut origin: Vec<(usize, Vec<i64>)> = Vec::new();
            let mut dropped = 0usize;
            'placement: for (g, t) in placements {
                let mut row = BTreeMap::new();
                for (sigma, value) in &self.gens[g].terms {
                    let at: Vec<i64> = sigma.iter().zip(&t).map(|(a, b)| a + b).collect();
                    let moved = c.translate(value, &t, Self::limits()).map_err(alg)?;
                    let moved = restrict(&moved)?;
                    if moved.is_zero() {
                        continue;
                    }
                    // No nonzero coefficient at an ISP exponent >= 1 on the domain.
                    for slot in self.lines..arity {
                        let cond = pattern[slot];
                        let low = cond.lo.max(1 - at[slot]);
                        if low > cond.hi {
                            continue;
                        }
                        if cond.fixed().is_some() || cond.hi - low > 64 {
                            dropped += 1;
                            continue 'placement;
                        }
                        for v in low..=cond.hi {
                            let layer = c
                                .specialize_fixed_indices(&moved, &[(slot, v)], Self::limits())
                                .map_err(alg)?
                                .0;
                            if !layer.is_zero() {
                                dropped += 1;
                                continue 'placement;
                            }
                        }
                    }
                    row.insert(self.shift(&at)?, moved);
                }
                if row.is_empty() {
                    continue;
                }
                frame.push(row);
                origin.push((g, t));
            }
            // Drop rows with a column no other row (nor the relation) has.
            loop {
                let mut count: BTreeMap<&IndexShift, usize> = BTreeMap::new();
                for row in frame.iter().chain(std::iter::once(&relation_row)) {
                    for key in row.keys() {
                        *count.entry(key).or_default() += 1;
                    }
                }
                let keep: Vec<bool> = (frame.iter())
                    .map(|row| row.keys().all(|key| count[key] > 1))
                    .collect();
                if keep.iter().all(|&k| k) {
                    break;
                }
                let mut i = 0;
                frame.retain(|_| {
                    i += 1;
                    keep[i - 1]
                });
                let mut i = 0;
                origin.retain(|_| {
                    i += 1;
                    keep[i - 1]
                });
            }
            let mut columns: BTreeSet<IndexShift> = BTreeSet::new();
            for row in &frame {
                columns.extend(row.keys().cloned());
            }
            columns.extend(relation_row.keys().filter(|key| **key != tau).cloned());
            outcome.level = Some(level);
            outcome.frame_rows = frame.len();
            outcome.frame_columns = columns.len();
            outcome.dropped_isp_placements = dropped;
            if frame.is_empty() {
                notes.push(format!("L{level}: empty frame after pruning"));
                continue;
            }
            frame.push(relation_row.clone());
            let relation_index = frame.len() - 1;
            let mut circuit = None;
            for seed in 0..options.seeds {
                if let Some(rows) =
                    select_projected_source_circuit(c, &frame, &tau, &columns, 0x5c4e_3e00 + seed)
                        .map_err(alg)?
                {
                    circuit = Some(rows);
                    break;
                }
            }
            let Some(circuit) = circuit else {
                notes.push(format!(
                    "L{level}: frame {} x {}: the relation is not in its span (mod p)",
                    frame.len() - 1,
                    columns.len()
                ));
                continue;
            };
            if circuit.last() != Some(&relation_index) {
                notes.push(format!("L{level}: a circuit without the relation row"));
                status = "error";
                continue;
            }
            outcome.circuit_rows = circuit.len() - 1;
            let rows: Vec<_> = circuit.iter().map(|&i| frame[i].clone()).collect();
            let mut used: BTreeSet<IndexShift> = BTreeSet::new();
            for row in &rows {
                used.extend(row.keys().filter(|key| **key != tau).cloned());
            }
            let limits = ProjectedSourceWeightLimits {
                arithmetic: ExactAlgebraLimits::unbounded_resources(),
                max_rows: usize::MAX,
                max_columns: usize::MAX,
                max_input_nonzeros: usize::MAX,
                max_coefficient_terms: usize::MAX,
                max_degree: options.max_degree,
                max_probes: usize::MAX,
                max_attempts: usize::MAX,
                max_primes: usize::MAX,
                max_cached_images: 4_000_000,
                max_cached_values: 200_000_000,
                max_weight_slots: usize::MAX,
            };
            let clock = Instant::now();
            if options.verbose {
                eprintln!(
                    "    {} L{level}: frame {} x {}, circuit {} rows x {} columns: reconstructing",
                    rule.name,
                    frame.len() - 1,
                    columns.len(),
                    circuit.len() - 1,
                    used.len()
                );
            }
            let verbose = options.verbose;
            let observe = |event: crate::solver::ProjectedSourceWeightEvent| {
                if verbose {
                    eprintln!("      {:>8.1} s {event:?}", clock.elapsed().as_secs_f64());
                }
            };
            let proposal = match reconstruct_projected_source_weights(
                c, &rows, &tau, &used, limits, observe,
            ) {
                Ok(proposal) => proposal,
                Err(error) => {
                    notes.push(format!(
                        "L{level}: circuit {}: reconstruction: {error}",
                        circuit.len() - 1
                    ));
                    status = "error";
                    continue;
                }
            };
            let last = rows.len() - 1;
            if proposal.image.len() != 1
                || !proposal
                    .weights
                    .get(&last)
                    .is_some_and(|w| w.raw().is_one())
            {
                notes.push(format!("L{level}: the relation row's weight is not 1"));
                status = "error";
                continue;
            }
            let lambdas: Vec<(usize, IndexedCoefficient)> = (proposal.weights.iter())
                .filter(|(position, _)| **position != last)
                .map(|(&position, weight)| (circuit[position], weight.clone()))
                .collect();
            let multiplier = self.common_denominator(lambdas.iter().map(|(_, l)| l))?;
            let mut weights = Vec::new();
            for (row, lambda) in &lambdas {
                weights.push((*row, c.mul(lambda, &multiplier).map_err(alg)?));
            }
            // Independent exact re-check: Sum W_i Row_i == restrict(D R).
            let mut sum: Map = BTreeMap::new();
            for (row, weight) in &weights {
                for (key, value) in &frame[*row] {
                    let product = c.mul(weight, value).map_err(alg)?;
                    let entry = sum.entry(key.values().to_vec()).or_insert_with(|| c.zero());
                    *entry = c.add(entry, &product).map_err(alg)?;
                }
            }
            sum.retain(|_, value| !value.is_zero());
            let mut ok = sum.len() == restricted.len();
            for (key, value) in &restricted {
                let scaled = c.mul(value, &multiplier).map_err(alg)?;
                ok &= sum
                    .get(key)
                    .is_some_and(|s| c.sub(s, &scaled).is_ok_and(|x| x.is_zero()));
            }
            if !ok {
                notes.push(format!(
                    "L{level}: the cleared certificate does not re-check"
                ));
                status = "error";
                continue;
            }
            // Record (Forcer slots and sign).
            let forcer_shift = |shift: &[i64]| -> Vec<i64> {
                let mut out = vec![0; arity];
                for (s, &v) in shift.iter().enumerate() {
                    out[self.variables.forcer[s]] = v;
                }
                out
            };
            let signed =
                |value: &IndexedCoefficient, shift: &[i64]| -> Result<IndexedCoefficient, PError> {
                    if parity(shift) == 1 {
                        Ok(value.clone())
                    } else {
                        c.neg_with_limits(value, ExactAlgebraLimits::unbounded_resources())
                            .map_err(alg)
                    }
                };
            let mut cleared: Map = BTreeMap::new();
            let mut relation_record = Vec::new();
            for (shift, value) in &relation {
                let scaled = c.mul(value, &multiplier).map_err(alg)?;
                let f = forcer_shift(shift);
                relation_record.push(serde_json::json!([
                    f,
                    self.variables.record(&signed(&scaled, &f)?)?
                ]));
                cleared.insert(shift.clone(), scaled);
            }
            let mut certificate_record = Vec::new();
            let (mut dn, mut dd) = (0, 0);
            let mut rule_generators = 0;
            for (row, weight) in &weights {
                let (g, t) = &origin[*row];
                let f = forcer_shift(t);
                let (n, dg) = self.variables.degrees(weight);
                dn = dn.max(n);
                dd = dd.max(dg);
                let record = self.variables.record(&signed(weight, &f)?)?;
                certificate_record.push(match &self.gens[*g].label {
                    GenLabel::Row(i, j) => serde_json::json!(["row", [i, j], f, record]),
                    GenLabel::Rule(name) => {
                        rule_generators += 1;
                        serde_json::json!(["rule", name, f, record])
                    }
                });
            }
            outcome.weight_degree_n = dn;
            outcome.weight_degree_d = dd;
            outcome.variables = proposal.active_variables;
            outcome.multiplier = if multiplier.raw().is_one() {
                "1".to_owned()
            } else {
                multiplier.to_expression().to_string()
            };
            if !multiplier.raw().is_one() {
                let d = self.variables.d;
                for (factor, _) in multiplier.raw().numerator.factor() {
                    let d_free = (&factor).into_iter().all(|m| m.exponents[d] == 0);
                    if d_free && !factor.is_constant() {
                        outcome.pole_layers.push(factor.to_string());
                    }
                }
            }
            outcome.status = "certificate".to_owned();
            notes.push(format!(
                "L{level}: certificate, {} entries ({rule_generators} through verified rules)",
                weights.len()
            ));
            outcome.detail = notes.join("; ");
            outcome.record = Some(serde_json::json!({
                "relation": relation_record,
                "certificate": certificate_record,
            }));
            return Ok(Some(cleared));
        }
        outcome.status = status.to_owned();
        outcome.detail = notes.join("; ");
        Ok(None)
    }

    /// Generator placements `(generator, offset)` of a level (presentation
    /// offsets), or `None` above the cap. A rule generator only where the
    /// target's stored domain, shifted by the offset, lies inside its own.
    fn placements(
        &self,
        level: u8,
        rule: &ParsedRule,
        support: &BTreeSet<Vec<i64>>,
        cap: usize,
    ) -> Option<Vec<(usize, Vec<i64>)>> {
        let mut offsets: BTreeSet<(usize, Vec<i64>)> = BTreeSet::new();
        let neighbours = |s: &Vec<i64>| -> Vec<Vec<i64>> {
            let mut out = Vec::new();
            for slot in 0..s.len() {
                for step in [-1, 1] {
                    let mut u = s.clone();
                    u[slot] += step;
                    out.push(u);
                }
            }
            out
        };
        for (g, generator) in self.gens.iter().enumerate() {
            for s in support {
                if level != 1 {
                    offsets.insert((g, s.clone()));
                }
                if level == 1 || level == 3 || level == 5 {
                    for (sigma, _) in &generator.terms {
                        let t: Vec<i64> = s.iter().zip(sigma).map(|(a, b)| a - b).collect();
                        offsets.insert((g, t));
                    }
                }
                if level >= 2 {
                    for u in neighbours(s) {
                        // Radius 2: ordinary IBP rows only (the rule generators
                        // stay at levels 0-3), else the frame explodes.
                        if level >= 4 && generator.domain.is_none() {
                            for w in neighbours(&u) {
                                offsets.insert((g, w));
                            }
                        }
                        offsets.insert((g, u));
                    }
                }
            }
            if offsets.len() > cap {
                return None;
            }
        }
        // Nearest the relation's support first (smaller circuits): the greedy
        // circuit choice keeps rows in this order.
        let distance = |t: &Vec<i64>| -> i64 {
            (support.iter())
                .map(|s| s.iter().zip(t).map(|(a, b)| (a - b).abs()).sum::<i64>())
                .min()
                .unwrap_or(0)
        };
        let mut offsets: Vec<(i64, usize, Vec<i64>)> = (offsets.into_iter())
            .map(|(g, t)| (distance(&t), g, t))
            .collect();
        offsets.sort();
        let offsets = offsets.into_iter().map(|(_, g, t)| (g, t));
        Some(
            offsets
                .into_iter()
                .filter(|(g, t)| match &self.gens[*g].domain {
                    None => true,
                    Some(domain) => (rule.pattern.iter().zip(domain).enumerate())
                        .all(|(a, (&mine, &theirs))| mine.shifted_inside(t[self.perm[a]], theirs)),
                })
                .collect(),
        )
    }
}
