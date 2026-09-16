//! Sufficient guard certificates on an affine target minus exact exclusions.
//!
//! If any base coefficient q of a guard is nonzero, the guard is nonzero in
//! the base field. For each native irreducible factor f of q we prove either
//! that f has no zero in the box, or that f=0 implies one *whole* excluded
//! conjunction. In particular, divisibility is e/f for every exclusion
//! equation e, never f/e and never just one equation of a conjunction.
//! A cold affine-first fallback retains the whole base-coefficient AND system.

use std::sync::Arc;

use symbolica::prelude::{IntegerRing, Matrix};

use crate::algebra::indexed::{
    IntegerZeroLocusDomainResolution, ceil_log2, integer_magnitude_bits,
};
use crate::algebra::{CoefficientPolynomial, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::cell::RuleCellLimits;
use crate::foundry::completion::LatticeBox;
use crate::foundry::parametric::{AffineApplicationDomain, AffineDomainRestriction};

use super::{SourcePortAuditError, error, validate_guard_with_limits};

mod conjunction;
mod factors;
mod singleton;

pub(in crate::foundry::artifact::source_port) fn validate_guard_on_domain_with_limits(
    context: &IndexedCoefficientContext,
    polynomial: &IndexedPolynomial,
    piece: &LatticeBox,
    sector: &[bool],
    target: Option<(&AffineApplicationDomain, &AffineDomainRestriction)>,
    exclusions: &[Arc<AffineApplicationDomain>],
    limits: RuleCellLimits,
) -> Result<(), SourcePortAuditError> {
    context
        .validate_polynomial_with_limits(polynomial, limits.indexed_algebra.exact_algebra)
        .map_err(error)?;
    if piece.arity() != sector.len()
        || sector.len() != context.index_count()
        || exclusions.len() > limits.max_guards
    {
        return Err(error(
            "affine guard has incompatible shape or exclusion budget",
        ));
    }
    for domain in target
        .iter()
        .map(|(domain, _)| *domain)
        .chain(exclusions.iter().map(Arc::as_ref))
    {
        validate_binding(context, domain, sector)?;
        if domain.equations().len() > limits.guard_algebra.max_coefficient_equations {
            return Err(error("affine guard predicate exceeds its equation budget"));
        }
    }
    if target.is_some_and(|(domain, _)| !contains_fixed_face(piece, domain)) {
        return Err(error(
            "affine guard box is not contained in its target fixed face",
        ));
    }
    // No new factorization or chart construction on the existing coordinate
    // success path. This is a sufficient proof on a superset of our domain.
    if validate_guard_with_limits(context, polynomial, piece, sector, limits).is_ok() {
        return Ok(());
    }
    let mut work = Work::default();
    let polynomial = restrict(context, polynomial.raw(), target, limits, &mut work)?;
    // Preserve the actual piece's fixed coordinates throughout coefficient
    // splitting, factor implications and conjunction charts. A temporary
    // bool-only specialization would lose this premise before exact lifting.
    let polynomial = singleton::restrict(context, &polynomial, piece, sector, limits, &mut work)?
        .unwrap_or(polynomial);
    if polynomial.is_zero() {
        return Err(error("guard vanishes identically on its affine target"));
    }
    if misses_target(
        context,
        &polynomial,
        piece,
        sector,
        target.map(|(domain, _)| domain),
        limits,
        &mut work,
    )? {
        return Ok(());
    }
    let mut predicates = Vec::new();
    for exclusion in exclusions {
        // A factor may force all equations but not an unrelated fixed face.
        // Requiring the entire box inside that face is conservative and
        // avoids silently replacing conjunction by its coupled equation.
        if !contains_fixed_face(piece, exclusion) {
            continue;
        }
        let mut equations = Vec::new();
        for equation in exclusion.equations() {
            if !is_index_affine(equation, context.base().variables().len()) {
                return Err(error("guard exclusion is not an index-affine equation"));
            }
            let equation = restrict(context, equation, target, limits, &mut work)?;
            equations.push(
                singleton::restrict(context, &equation, piece, sector, limits, &mut work)?
                    .unwrap_or(equation),
            );
        }
        if !equations.is_empty() {
            predicates.push(equations);
        }
    }
    let system = context
        .base_coefficient_system(&polynomial, limits.indexed_algebra, limits.guard_algebra)
        .map_err(error)?;
    let mut factor_work = 0;
    let mut consequences = Vec::new();
    let domain = conjunction::GuardDomain {
        piece,
        sector,
        target: target.map(|(domain, _)| domain),
    };
    for coefficient in system.equations() {
        let coefficient = coefficient.index_polynomial();
        if coefficient.is_zero() {
            continue;
        }
        match factors::classify(
            context,
            coefficient,
            &predicates,
            &domain,
            limits,
            &mut work,
            &mut factor_work,
        )? {
            factors::Consequence::Nonzero => return Ok(()),
            factors::Consequence::Affine(equation) => {
                // An originally affine coefficient is already a necessary
                // equation; do not spend a second retained-row slot on it.
                if !is_index_affine(coefficient.raw(), context.base().variables().len()) {
                    conjunction::retain_consequence(
                        &system,
                        &predicates,
                        &mut consequences,
                        equation,
                        limits,
                        &mut work,
                    )?;
                }
            }
            factors::Consequence::Unresolved => {}
        }
    }
    if conjunction::proves_excluded_on_domain(
        context,
        &system,
        &predicates,
        &consequences,
        domain,
        limits,
        &mut work,
        &mut factor_work,
    )? {
        return Ok(());
    }
    Err(error(
        "guard zero locus is not proved outside the complete affine application domain",
    ))
}

/// A coordinate root may meet the rectangular prefilter but miss the exact
/// target after its other coordinate bounds are retained. Native guard-locus
/// decomposition still supplies every candidate root; this only narrows the
/// caller-owned integer domain tested by that existing service.
fn misses_target(
    context: &IndexedCoefficientContext,
    polynomial: &IndexedPolynomial,
    piece: &LatticeBox,
    sector: &[bool],
    target: Option<&AffineApplicationDomain>,
    limits: RuleCellLimits,
    work: &mut Work,
) -> Result<bool, SourcePortAuditError> {
    use symbolica::prelude::Integer;
    // Admit the complete original before specialization: a singleton cannot
    // hide an oversized or unauthenticated input by cancelling its terms.
    let mut system = context
        .base_coefficient_system(polynomial, limits.indexed_algebra, limits.guard_algebra)
        .map_err(error)?;
    // A native affine factor can miss the complete box even when it is not a
    // coordinate hyperplane. Reuse the existing exact original-equation bounds
    // service; no new interval algebra or rectangular exclusion is introduced.
    if !polynomial.is_zero()
        && !polynomial.is_nonzero_constant()
        && is_index_affine(polynomial.raw(), context.base().variables().len())
    {
        conjunction::precharge_box_equation(
            polynomial.raw(),
            context.base().variables().len(),
            piece,
            sector,
            limits,
            work,
        )?;
        let first = context.base().variables().len();
        let indices: Vec<_> = (0..sector.len()).map(|axis| first + axis).collect();
        if AffineApplicationDomain::equation_proved_empty_in_box(
            polynomial.raw(),
            &indices,
            sector,
            piece,
        )
        .map_err(error)?
        {
            return Ok(true);
        }
    }
    if let Some(restricted) = singleton::restrict(context, polynomial, piece, sector, limits, work)?
    {
        system = context
            .base_coefficient_system(&restricted, limits.indexed_algebra, limits.guard_algebra)
            .map_err(error)?;
    }
    let resolution = context
        .integer_zero_locus_domain_resolution(&system, limits.guard_algebra, |axis, root| {
            let local = if sector[axis] {
                root - &Integer::one()
            } else {
                -root.clone()
            };
            if local < Integer::from(piece.lower()[axis])
                || piece.upper()[axis].is_some_and(|upper| local > Integer::from(upper))
            {
                return false;
            }
            let Some(target) = target else {
                return true;
            };
            // Not representable by our exact box carrier means inconclusive,
            // never permission to discard a large but valid integer root.
            let Some(local) = local.to_i64().and_then(|v| u64::try_from(v).ok()) else {
                return true;
            };
            let mut lower = piece.lower().to_vec();
            let mut upper = piece.upper().to_vec();
            lower[axis] = local;
            upper[axis] = Some(local);
            let Ok(face) = LatticeBox::try_new(lower, upper) else {
                return true;
            };
            !target.is_proved_empty_in_box(&face)
        })
        .map_err(error)?;
    Ok(matches!(
        resolution,
        IntegerZeroLocusDomainResolution::MissesDomain
    ))
}

fn validate_binding(
    context: &IndexedCoefficientContext,
    domain: &AffineApplicationDomain,
    sector: &[bool],
) -> Result<(), SourcePortAuditError> {
    let template = context.one();
    let first = context.base().variables().len();
    if !domain.is_authenticated()
        || domain.sector() != sector
        || domain.fixed().len() != sector.len()
        || domain.indices().len() != sector.len()
        || domain
            .indices()
            .iter()
            .enumerate()
            .any(|(axis, &position)| position != first + axis)
        || domain
            .equations()
            .iter()
            .any(|equation| equation.variables() != template.raw().numerator.variables())
    {
        return Err(error(
            "affine guard predicate has a foreign sector or index-variable map",
        ));
    }
    Ok(())
}

fn contains_fixed_face(piece: &LatticeBox, domain: &AffineApplicationDomain) -> bool {
    domain.fixed().iter().enumerate().all(|(axis, fixed)| {
        fixed.is_none_or(|value| {
            let local = if domain.sector()[axis] {
                i64::from(value) - 1
            } else {
                -i64::from(value)
            };
            u64::try_from(local).is_ok_and(|local| {
                piece.lower()[axis] == local && piece.upper()[axis] == Some(local)
            })
        })
    })
}

fn is_index_affine(polynomial: &CoefficientPolynomial, base_count: usize) -> bool {
    polynomial.exponents_iter().all(|powers| {
        powers[..base_count].iter().all(|&power| power == 0)
            && powers[base_count..]
                .iter()
                .map(|&power| usize::from(power))
                .sum::<usize>()
                <= 1
    })
}

#[derive(Default)]
struct Work {
    operations: usize,
    substitutions: usize,
    input_terms: usize,
}

impl Work {
    fn charge(
        &mut self,
        amount: usize,
        limits: RuleCellLimits,
    ) -> Result<(), SourcePortAuditError> {
        self.operations = self
            .operations
            .checked_add(amount)
            .ok_or_else(|| error("affine guard work overflow"))?;
        self.substitutions = self
            .substitutions
            .checked_add(1)
            .ok_or_else(|| error("affine guard operation count overflow"))?;
        if self.operations > limits.guard_algebra.max_exact_hyperplane_replay_work
            || self.substitutions
                > limits
                    .guard_algebra
                    .max_exact_hyperplane_replay_substitutions
        {
            return Err(error(
                "affine guard restriction/implication exceeds work budget",
            ));
        }
        Ok(())
    }
}

fn restrict(
    context: &IndexedCoefficientContext,
    raw: &CoefficientPolynomial,
    target: Option<(&AffineApplicationDomain, &AffineDomainRestriction)>,
    limits: RuleCellLimits,
    work: &mut Work,
) -> Result<IndexedPolynomial, SourcePortAuditError> {
    restrict_prepared(
        context,
        raw,
        target.map(|(domain, chart)| RestrictionTarget {
            chart,
            primitive: domain.primitive_matrix(),
            diagnostic_domain: Some(domain),
        }),
        limits,
        work,
    )
}

/// Shared admission/precharge for original and transient native charts.
/// A transient conjunction chart is never itself application-domain authority.
struct RestrictionTarget<'a> {
    chart: &'a AffineDomainRestriction,
    primitive: Option<&'a Matrix<IntegerRing>>,
    diagnostic_domain: Option<&'a AffineApplicationDomain>,
}

fn restrict_prepared(
    context: &IndexedCoefficientContext,
    raw: &CoefficientPolynomial,
    target: Option<RestrictionTarget<'_>>,
    limits: RuleCellLimits,
    work: &mut Work,
) -> Result<IndexedPolynomial, SourcePortAuditError> {
    work.input_terms = work
        .input_terms
        .checked_add(raw.nterms())
        .ok_or_else(|| error("affine guard input-term count overflow"))?;
    if raw.nterms() > limits.guard_algebra.max_input_terms
        || work.input_terms > limits.guard_algebra.max_exact_hyperplane_replay_terms
    {
        return Err(error("affine guard exceeds aggregate input-term budget"));
    }
    work.charge(raw.nterms(), limits)?;
    let input = context
        .admit_native_polynomial_result_with_limits(
            raw.clone(),
            limits.indexed_algebra.exact_algebra,
        )
        .map_err(error)?;
    // Reuse the coefficient-split admission checks (terms and integer bits)
    // before any native chart substitution owns expanding allocations.
    context
        .base_coefficient_system(&input, limits.indexed_algebra, limits.guard_algebra)
        .map_err(error)?;
    let Some(target) = target else {
        return Ok(input);
    };
    let chart = target.chart;
    let n = context.index_count();
    let degree = raw
        .exponents_iter()
        .map(|powers| {
            powers[context.base().variables().len()..]
                .iter()
                .map(|&p| usize::from(p))
                .sum::<usize>()
        })
        .max()
        .unwrap_or(0);
    if degree > limits.guard_algebra.max_factor_total_degree {
        return Err(error("affine guard chart input exceeds degree budget"));
    }
    // No substituted variable occurs: there is no expanding operation to
    // preflight. Input/map/bit/work/degree admission above still applies, and
    // callers must still prove nonvanishing on the full application domain.
    // Keeping scalar content is valid here because only the zero locus is
    // used; coefficient-value restrictions retain their separate exact path.
    if !chart.affects_polynomial(raw).map_err(error)? {
        return Ok(input);
    }
    // Count support separately for each original monomial. Powers of free
    // variables do not expand, and fixed-zero monomials vanish before the
    // coupled substitutions. Full input admission above remains mandatory.
    let (terms, expansion) = chart.restriction_term_bound(raw).map_err(error)?;
    if terms > limits.guard_algebra.max_input_terms
        || terms > limits.guard_algebra.max_exact_hyperplane_replay_terms
    {
        if let Some(domain) = target.diagnostic_domain {
            super::super::diagnostic::restriction_failure(
                super::super::diagnostic::RestrictionFailure {
                    reason: "affine guard chart exceeds prospective term budget",
                    polynomial: raw,
                    target: domain,
                    estimate: super::super::diagnostic::RestrictionEstimate {
                        variables: n,
                        index_degree: degree,
                        input_terms: raw.nterms(),
                        expansion,
                        prospective_terms: terms,
                        chart_bits: None,
                        input_coefficient_bits: None,
                        prospective_coefficient_bits: None,
                        prospective_total_bits: None,
                        max_input_terms: limits.guard_algebra.max_input_terms,
                        max_replay_terms: limits.guard_algebra.max_exact_hyperplane_replay_terms,
                        max_total_bits: limits.guard_algebra.max_total_integer_bits,
                        max_coefficient_bits: limits
                            .indexed_algebra
                            .max_specialization_integer_bits,
                    },
                },
            );
        }
        return Err(error("affine guard chart exceeds prospective term budget"));
    }
    // Hadamard/Cramer's-rule bound: one pivot minor supplies a common
    // denominator to the rational affine chart. No integer-lattice
    // assumption is made here, and the native chart does all algebra.
    let bits = target
        .primitive
        .ok_or_else(|| error("affine guard target lacks an authenticated matrix"))?
        .iter()
        .map(integer_magnitude_bits)
        .max()
        .unwrap_or(0);
    let chart_bits = usize::try_from(bits)
        .ok()
        .and_then(|v| v.checked_add(ceil_log2(n.max(1)) + 2))
        .and_then(|v| v.checked_mul(n.max(1)))
        .ok_or_else(|| error("affine guard chart bit estimate overflow"))?;
    let input_bits = raw
        .coefficients
        .iter()
        .map(integer_magnitude_bits)
        .max()
        .unwrap_or(0) as usize;
    let bits = chart_bits
        .checked_add(17 + ceil_log2(n + 1))
        .and_then(|v| v.checked_mul(degree))
        .and_then(|v| v.checked_add(input_bits + ceil_log2(raw.nterms().max(1)) + 2))
        .ok_or_else(|| error("affine guard prospective bits overflow"))?;
    let total_bits = terms
        .checked_mul(bits)
        .ok_or_else(|| error("affine guard prospective bits overflow"))?;
    if total_bits > limits.guard_algebra.max_total_integer_bits
        || bits > limits.indexed_algebra.max_specialization_integer_bits
    {
        if let Some(domain) = target.diagnostic_domain {
            super::super::diagnostic::restriction_failure(
                super::super::diagnostic::RestrictionFailure {
                    reason: "affine guard chart exceeds prospective integer-bit budget",
                    polynomial: raw,
                    target: domain,
                    estimate: super::super::diagnostic::RestrictionEstimate {
                        variables: n,
                        index_degree: degree,
                        input_terms: raw.nterms(),
                        expansion,
                        prospective_terms: terms,
                        chart_bits: Some(chart_bits),
                        input_coefficient_bits: Some(input_bits),
                        prospective_coefficient_bits: Some(bits),
                        prospective_total_bits: Some(total_bits),
                        max_input_terms: limits.guard_algebra.max_input_terms,
                        max_replay_terms: limits.guard_algebra.max_exact_hyperplane_replay_terms,
                        max_total_bits: limits.guard_algebra.max_total_integer_bits,
                        max_coefficient_bits: limits
                            .indexed_algebra
                            .max_specialization_integer_bits,
                    },
                },
            );
        }
        return Err(error(
            "affine guard chart exceeds prospective integer-bit budget",
        ));
    }
    work.charge(
        terms
            .saturating_mul(degree + 1)
            .saturating_mul(bits.div_ceil(usize::BITS as usize).max(1)),
        limits,
    )?;
    let raw = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        chart.restrict_equation(raw)
    }))
    .map_err(|_| error("native affine guard restriction panicked"))?
    .map_err(error)?;
    let output = context
        .admit_native_polynomial_result_with_limits(raw, limits.indexed_algebra.exact_algebra)
        .map_err(error)?;
    context
        .base_coefficient_system(&output, limits.indexed_algebra, limits.guard_algebra)
        .map_err(error)?;
    Ok(output)
}

#[cfg(test)]
mod tests;
