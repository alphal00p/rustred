//! Read-only structural census of the installed rules' guard and coefficient
//! polynomials (next-push plan W0.7, question Q3: are guard and coefficient
//! factors mostly affine in the indices?). Factorization is Symbolica's
//! native `Factorize::factor`; this module only groups the rule polynomials,
//! deduplicates them and classifies each returned factor by its degrees in
//! the index and base variables. It never feeds a walk, a proof or a cache.
use std::collections::{BTreeMap, HashMap};

#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;
use symbolica::poly::factor::Factorize;

use crate::algebra::CoefficientPolynomial;

use super::model::CandidateOwnerPrograms;

/// Which polynomials are factored, and the size cap per distinct polynomial.
#[derive(Clone, Copy, Debug)]
pub struct FactorCensusLimits {
    /// Distinct polynomials with more terms are counted but not factored.
    pub max_terms: usize,
    /// Also factor rational-coefficient numerators (large; off by default).
    pub coefficient_numerators: bool,
}

impl Default for FactorCensusLimits {
    fn default() -> Self {
        Self {
            max_terms: 4096,
            coefficient_numerators: false,
        }
    }
}

/// Tally of one polynomial role (equality guard, excluded conjunction,
/// term denominator or coefficient numerator). A term's coefficient
/// denominator is not a separate role: preparation stores the term
/// denominator as a copy of the coefficient's denominator
/// (`preparation/shared.rs`), so the two coincide by construction; the census
/// only counts terms where they differ (`coefficient_denominator_mismatches`).
#[derive(Clone, Debug, Default)]
pub struct FactorCensusRole {
    /// Polynomial occurrences in the installed rules.
    pub occurrences: usize,
    /// Distinct polynomials (exact equality).
    pub distinct: usize,
    /// Distinct polynomials above `max_terms`, not factored.
    pub skipped: usize,
    /// Distinct polynomials whose factorization panicked or returned nothing.
    pub failed: usize,
    /// Distinct factored polynomials whose every factor is constant,
    /// base-only or affine in the indices with integer coefficients.
    pub distinct_affine_only: usize,
    /// The same, weighted by occurrences.
    pub occurrences_affine_only: usize,
    /// Distinct factored polynomials whose every factor is at most affine in
    /// the indices (base-dependent offsets or slopes allowed).
    pub distinct_index_affine: usize,
    pub occurrences_index_affine: usize,
    /// Factor class -> (distinct polynomial-factor pairs, occurrence-weighted).
    pub factor_classes: BTreeMap<&'static str, (usize, usize)>,
    /// Distinct irreducible factors over all polynomials of the role.
    pub distinct_factors: usize,
    pub max_index_degree: u32,
    pub max_terms_seen: usize,
    /// Up to three example factors per class (Symbolica display), highest
    /// occurrence first.
    pub examples: BTreeMap<&'static str, Vec<(usize, String)>>,
}

#[derive(Clone, Debug, Default)]
pub struct OwnerFactorCensus {
    /// Owner support mask, coordinate 0 first.
    pub owner: String,
    pub batches: usize,
    pub rules: usize,
    pub affine_cases: usize,
    pub rhs_terms: usize,
    /// RHS terms whose coefficient denominator differs from the stored term
    /// denominator (expected 0: both are the same polynomial).
    pub coefficient_denominator_mismatches: usize,
    pub roles: BTreeMap<&'static str, FactorCensusRole>,
}

/// Class of one irreducible factor by its degree in the index variables and
/// where the base (kinematic/dimension) variables appear.
pub fn classify_factor(factor: &CoefficientPolynomial, index: &[bool]) -> (&'static str, u32) {
    let (mut di, mut db_mixed, mut db_const) = (0u32, 0u32, 0u32);
    for exponents in factor.exponents_iter() {
        let (mut si, mut sb) = (0u32, 0u32);
        for (k, &e) in exponents.iter().enumerate() {
            if index.get(k).copied().unwrap_or(false) {
                si += e as u32;
            } else {
                sb += e as u32;
            }
        }
        di = di.max(si);
        if si > 0 {
            db_mixed = db_mixed.max(sb);
        } else {
            db_const = db_const.max(sb);
        }
    }
    let class = match (di, db_mixed, db_const) {
        (0, _, 0) => "constant",
        (0, _, _) => "base_only",
        (1, 0, 0) => "affine_index",
        (1, 0, _) => "affine_index_base_offset",
        (1, _, _) => "affine_index_base_slope",
        _ => "nonlinear_index",
    };
    (class, di)
}

impl<const N: usize> CandidateOwnerPrograms<N> {
    /// One census per installed owner, in owner order.
    pub fn factor_census(&self, limits: FactorCensusLimits) -> Vec<OwnerFactorCensus> {
        let positions = self.context.index_variables();
        self.owners
            .iter()
            .map(|(sector, owner)| {
                let mut out = OwnerFactorCensus {
                    owner: sector.iter().map(|&b| if b { '1' } else { '0' }).collect(),
                    batches: owner.batches.len(),
                    ..Default::default()
                };
                let mut polys: BTreeMap<&'static str, HashMap<&CoefficientPolynomial, usize>> =
                    BTreeMap::new();
                let mut refs: Vec<(&'static str, &CoefficientPolynomial)> = Vec::new();
                for batch in &owner.batches {
                    for rule in &batch.rules {
                        out.rules += 1;
                        out.affine_cases += usize::from(rule.case.affine().is_some());
                        for p in &rule.equalities {
                            refs.push(("equality", p.raw()));
                        }
                        for branch in &rule.exceptions {
                            for p in branch {
                                refs.push(("excluded_conjunction", p.raw()));
                            }
                        }
                        for term in &rule.rhs {
                            out.rhs_terms += 1;
                            refs.push(("term_denominator", term.denominator.raw()));
                            let c = term.coefficient.raw();
                            if c.denominator != *term.denominator.raw() {
                                out.coefficient_denominator_mismatches += 1;
                            }
                            if limits.coefficient_numerators {
                                refs.push(("coefficient_numerator", &c.numerator));
                            }
                        }
                    }
                }
                for (role, p) in refs {
                    *polys.entry(role).or_default().entry(p).or_default() += 1;
                }
                for (role, map) in polys {
                    let nvars = map.keys().next().map_or(0, |p| p.nvars());
                    let mut index = vec![false; nvars];
                    for &position in positions.iter() {
                        if position < nvars {
                            index[position] = true;
                        }
                    }
                    let entries: Vec<(&CoefficientPolynomial, usize)> = map.into_iter().collect();
                    // Native builds factor in parallel; WebAssembly must not
                    // implicitly initialize Rayon's global OS worker pool.
                    #[cfg(not(target_arch = "wasm32"))]
                    let entries = entries.par_iter();
                    #[cfg(target_arch = "wasm32")]
                    let entries = entries.iter();
                    let factored: Vec<(usize, usize, Option<Vec<CoefficientPolynomial>>)> = entries
                        .map(|&(p, count)| {
                            if p.nterms() > limits.max_terms {
                                return (count, p.nterms(), None);
                            }
                            let factors =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    p.factor()
                                }))
                                .ok()
                                .filter(|f| !f.is_empty())
                                .map(|f| f.into_iter().map(|(factor, _)| factor).collect());
                            (count, p.nterms(), factors)
                        })
                        .collect();
                    let tally = out.roles.entry(role).or_default();
                    let mut distinct_factors: HashMap<CoefficientPolynomial, ()> = HashMap::new();
                    for (count, terms, factors) in factored {
                        tally.occurrences += count;
                        tally.distinct += 1;
                        tally.max_terms_seen = tally.max_terms_seen.max(terms);
                        let Some(factors) = factors else {
                            if terms > limits.max_terms {
                                tally.skipped += 1;
                            } else {
                                tally.failed += 1;
                            }
                            continue;
                        };
                        let mut affine_only = true;
                        let mut index_affine = true;
                        for factor in factors {
                            let (class, degree) = classify_factor(&factor, &index);
                            tally.max_index_degree = tally.max_index_degree.max(degree);
                            let entry = tally.factor_classes.entry(class).or_default();
                            entry.0 += 1;
                            entry.1 += count;
                            if class != "constant" {
                                let ex = tally.examples.entry(class).or_default();
                                if ex.len() < 3 || ex.last().is_some_and(|e| e.0 < count) {
                                    ex.push((count, format!("{factor}")));
                                    ex.sort_by(|a, b| b.0.cmp(&a.0));
                                    ex.truncate(3);
                                }
                            }
                            match class {
                                "constant" | "base_only" | "affine_index" => {}
                                "affine_index_base_offset" | "affine_index_base_slope" => {
                                    affine_only = false
                                }
                                _ => {
                                    affine_only = false;
                                    index_affine = false;
                                }
                            }
                            distinct_factors.insert(factor, ());
                        }
                        if affine_only {
                            tally.distinct_affine_only += 1;
                            tally.occurrences_affine_only += count;
                        }
                        if index_affine {
                            tally.distinct_index_affine += 1;
                            tally.occurrences_index_affine += count;
                        }
                    }
                    tally.distinct_factors = distinct_factors.len();
                }
                out
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::{CoefficientContext, IndexedCoefficientContext};

    /// Symbolica's factorization of (n0+2)(n0+3)(d n0 + n1)(n1 + d)(n0^2+1)(d-4)
    /// yields one factor per class; base variables precede index variables.
    #[test]
    fn classifies_symbolica_factors_by_index_and_base_degree() {
        let base = CoefficientContext::new(["d"]);
        let context = IndexedCoefficientContext::try_new(&base, "factor-census", 2).unwrap();
        let d = context.lift(&base.parameter("d").unwrap()).unwrap();
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let two = context.integer(2);
        let three = context.integer(3);
        let four = context.integer(4);
        let one = context.one();
        let factors = [
            context.add(&n0, &two).unwrap(),
            context.add(&n0, &three).unwrap(),
            context.add(&context.mul(&d, &n0).unwrap(), &n1).unwrap(),
            context.add(&n1, &d).unwrap(),
            context.add(&context.mul(&n0, &n0).unwrap(), &one).unwrap(),
            context.sub(&d, &four).unwrap(),
        ];
        let mut product = context.one();
        for f in &factors {
            product = context.mul(&product, f).unwrap();
        }
        let polynomial = context
            .numerator_condition_with_limits(&product, Default::default())
            .unwrap();
        let raw = polynomial.raw();
        let nvars = raw.nvars();
        assert_eq!(nvars, 3);
        let index = [false, true, true];
        let mut classes: Vec<&str> = raw
            .factor()
            .into_iter()
            .map(|(f, _)| classify_factor(&f, &index).0)
            .filter(|c| *c != "constant")
            .collect();
        classes.sort();
        assert_eq!(
            classes,
            [
                "affine_index",
                "affine_index",
                "affine_index_base_offset",
                "affine_index_base_slope",
                "base_only",
                "nonlinear_index",
            ]
        );
    }
}
