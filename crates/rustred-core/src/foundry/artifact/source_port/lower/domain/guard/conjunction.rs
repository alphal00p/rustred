//! Bounded affine consequences of one complete guard-coefficient conjunction.
//!
//! Inputs and exclusions have already been restricted by the same authenticated
//! target chart. Refinement proves containment on an affine superset of their
//! common zero locus; nonlinear siblings are never alternative branches.

use crate::algebra::indexed::BaseCoefficientSystem;
use crate::solver::AffineGeometryError;

use super::*;

const MAX_EQUATIONS: usize = 32;
const MAX_MATRIX_CELLS: usize = 65_536;
const MAX_POLYNOMIAL_CELLS: usize = 262_144;

pub(super) struct GuardDomain<'a> {
    pub piece: &'a LatticeBox,
    pub sector: &'a [bool],
    pub target: Option<&'a AffineApplicationDomain>,
}

pub(super) fn proves_excluded_on_domain(
    context: &IndexedCoefficientContext,
    system: &BaseCoefficientSystem,
    predicates: &[Vec<IndexedPolynomial>],
    domain: GuardDomain<'_>,
    limits: RuleCellLimits,
    work: &mut Work,
) -> Result<bool, SourcePortAuditError> {
    proves_excluded_using(
        context,
        system,
        predicates,
        Some(domain),
        limits,
        work,
        AffineDomainRestriction::from_equalities,
    )
}

#[cfg(test)]
pub(super) fn proves_excluded(
    context: &IndexedCoefficientContext,
    system: &BaseCoefficientSystem,
    predicates: &[Vec<IndexedPolynomial>],
    limits: RuleCellLimits,
    work: &mut Work,
) -> Result<bool, SourcePortAuditError> {
    proves_excluded_using(
        context,
        system,
        predicates,
        None,
        limits,
        work,
        AffineDomainRestriction::from_equalities,
    )
}

/// Private injection seam; production uses only the existing native reducer.
pub(super) fn proves_excluded_using<F>(
    context: &IndexedCoefficientContext,
    system: &BaseCoefficientSystem,
    predicates: &[Vec<IndexedPolynomial>],
    domain: Option<GuardDomain<'_>>,
    limits: RuleCellLimits,
    work: &mut Work,
    mut native: F,
) -> Result<bool, SourcePortAuditError>
where
    F: FnMut(
        &[Option<i16>],
        &[CoefficientPolynomial],
        &[usize],
    )
        -> Result<Option<(AffineDomainRestriction, Matrix<IntegerRing>)>, AffineGeometryError>,
{
    // No affine consequence to absorb: do not allocate or enter native algebra.
    if !system.equations().iter().any(|equation| {
        let raw = equation.index_polynomial().raw();
        !raw.is_zero() && is_index_affine(raw, context.base().variables().len())
    }) {
        return Ok(false);
    }
    let count = system.equations().len();
    if count > MAX_EQUATIONS || count > limits.guard_algebra.max_coefficient_equations {
        return Err(error(
            "affine guard conjunction exceeds its equation budget",
        ));
    }
    let mut cells = 0usize;
    // Full input admission precedes every successful contradiction/containment.
    // The caller constructed the system natively, but preserve its context and
    // resource obligations before retaining any additional polynomial copies.
    for polynomial in system
        .equations()
        .iter()
        .map(|equation| equation.index_polynomial())
        .chain(predicates.iter().flatten())
    {
        context
            .validate_polynomial_with_limits(polynomial, limits.indexed_algebra.exact_algebra)
            .map_err(error)?;
        context
            .base_coefficient_system(polynomial, limits.indexed_algebra, limits.guard_algebra)
            .map_err(error)?;
        cells = cells
            .checked_add(polynomial.raw().exponents.len())
            .ok_or_else(|| error("affine guard conjunction storage overflow"))?;
        if cells > MAX_POLYNOMIAL_CELLS {
            return Err(error("affine guard conjunction exceeds its storage budget"));
        }
    }
    work.charge(cells, limits)?;
    let n = context.index_count();
    let columns = n
        .checked_add(1)
        .ok_or_else(|| error("affine guard conjunction dimension overflow"))?;
    if n == 0
        || count
            .checked_mul(columns)
            .is_none_or(|v| v > MAX_MATRIX_CELLS)
    {
        return Err(error("affine guard conjunction exceeds its matrix budget"));
    }
    let first = context.base().variables().len();
    let indices: Vec<_> = (0..n).map(|axis| first + axis).collect();
    let fixed = vec![None; n];
    let mut pending: Vec<_> = system
        .equations()
        .iter()
        .map(|equation| equation.index_polynomial().clone())
        .collect();
    let mut affine = Vec::new();
    let mut rank = 0;
    loop {
        let old_count = affine.len();
        let mut nonlinear = Vec::new();
        for equation in pending {
            if equation.is_zero() {
                continue;
            }
            if equation.is_nonzero_constant() {
                return Ok(true);
            }
            if is_index_affine(equation.raw(), first) {
                affine.push(equation.raw().clone());
            } else {
                nonlinear.push(equation);
            }
        }
        if affine.len() == old_count {
            return Ok(false);
        }
        precharge_matrix(&affine, n, limits, work)?;
        let Some((chart, primitive)) =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                native(&fixed, &affine, &indices)
            }))
            .map_err(|_| error("native affine guard conjunction reduction panicked"))?
            .map_err(error)?
        else {
            return Ok(true);
        };
        if primitive.nrows() <= rank {
            return Ok(false);
        }
        rank = primitive.nrows();
        // Strict rank progress bounds iterations by the dynamic index arity.
        // Every prior affine equation remains present when new ones are added.
        for predicate in predicates {
            if predicate.is_empty() {
                continue;
            }
            let mut implied = true;
            for equation in predicate {
                if !restrict_prepared(
                    context,
                    equation.raw(),
                    Some(RestrictionTarget {
                        chart: &chart,
                        primitive: Some(&primitive),
                        diagnostic_domain: None,
                    }),
                    limits,
                    work,
                )?
                .is_zero()
                {
                    implied = false;
                    break;
                }
            }
            if implied {
                return Ok(true);
            }
        }
        pending = Vec::with_capacity(nonlinear.len());
        for equation in nonlinear {
            let restricted = restrict_prepared(
                context,
                equation.raw(),
                Some(RestrictionTarget {
                    chart: &chart,
                    primitive: Some(&primitive),
                    diagnostic_domain: None,
                }),
                limits,
                work,
            )?;
            if let Some(domain) = &domain
                && misses_target(
                    context,
                    &restricted,
                    domain.piece,
                    domain.sector,
                    domain.target,
                    limits,
                    work,
                )?
            {
                return Ok(true);
            }
            pending.push(restricted);
        }
    }
}

pub(super) fn precharge_matrix(
    equations: &[CoefficientPolynomial],
    n: usize,
    limits: RuleCellLimits,
    work: &mut Work,
) -> Result<(), SourcePortAuditError> {
    precharge_matrix_with_bits(equations, n, 0, limits, work)
}

/// The existing bool-only box service may specialize all singleton axes and
/// then exhaust one small supported finite axis. Charge all structurally
/// possible calls before entering it, including endpoint-induced bit growth.
pub(super) fn precharge_box_equation(
    equation: &CoefficientPolynomial,
    first_index: usize,
    piece: &LatticeBox,
    sector: &[bool],
    limits: RuleCellLimits,
    work: &mut Work,
) -> Result<(), SourcePortAuditError> {
    use crate::foundry::completion::MAX_BOUNDED_AXIS_FACES;

    if piece.arity() != sector.len() {
        return Err(error("affine guard box preflight has incompatible arity"));
    }
    let mut endpoint_bits = 0usize;
    let mut singletons = 0usize;
    let mut faces = None;
    for (axis, &active) in sector.iter().enumerate() {
        let lower = piece.lower()[axis];
        let upper = piece.upper()[axis];
        singletons += usize::from(upper == Some(lower));
        // Even absent variables are traversed by the existing singleton
        // substitution service. Endpoints are never narrowed to i16/i64.
        for endpoint in std::iter::once(lower).chain(upper) {
            let bits = (u64::BITS - endpoint.leading_zeros()) as usize + usize::from(active);
            endpoint_bits = endpoint_bits.max(bits);
        }
        if equation.contains(first_index + axis)
            && let Some(width) = upper.and_then(|upper| upper.checked_sub(lower))
            && width > 0
            && width < MAX_BOUNDED_AXIS_FACES as u64
        {
            let count = width as usize + 1;
            faces = Some(faces.map_or(count, |old: usize| old.min(count)));
        }
    }
    let input_bits = equation
        .coefficients
        .iter()
        .map(integer_magnitude_bits)
        .max()
        .unwrap_or(0) as usize;
    // This service receives affine equations only: endpoint multiplication
    // and summation of at most nterms constants bound all specialization bits.
    let substituted_bits = input_bits
        .checked_add(endpoint_bits)
        .and_then(|bits| bits.checked_add(ceil_log2(equation.nterms().max(1)) + 2))
        .ok_or_else(|| error("affine guard box specialization bit overflow"))?;
    let native_passes = 1 + faces.unwrap_or(0);
    for pass in 0..native_passes {
        precharge_matrix_with_bits(
            std::slice::from_ref(equation),
            sector.len(),
            substituted_bits,
            limits,
            work,
        )?;
        let substitutions = singletons + usize::from(pass != 0);
        for _ in 0..substitutions {
            work.input_terms = work
                .input_terms
                .checked_add(equation.nterms())
                .ok_or_else(|| error("affine guard box aggregate-term overflow"))?;
            if work.input_terms > limits.guard_algebra.max_exact_hyperplane_replay_terms {
                return Err(error(
                    "affine guard box exceeds aggregate input-term budget",
                ));
            }
            let operations = equation
                .nterms()
                .checked_mul(substituted_bits.div_ceil(usize::BITS as usize).max(1))
                .ok_or_else(|| error("affine guard box substitution-work overflow"))?;
            work.charge(operations, limits)?;
        }
    }
    Ok(())
}

fn precharge_matrix_with_bits(
    equations: &[CoefficientPolynomial],
    n: usize,
    substituted_bits: usize,
    limits: RuleCellLimits,
    work: &mut Work,
) -> Result<(), SourcePortAuditError> {
    let columns = n
        .checked_add(1)
        .ok_or_else(|| error("affine guard conjunction matrix overflow"))?;
    if equations.len() > MAX_EQUATIONS
        || equations.len() > limits.guard_algebra.max_coefficient_equations
    {
        return Err(error(
            "affine guard conjunction exceeds its equation budget",
        ));
    }
    let storage = equations.iter().try_fold(0usize, |total, equation| {
        total.checked_add(equation.exponents.len())
    });
    if storage.is_none_or(|cells| cells > MAX_POLYNOMIAL_CELLS) {
        return Err(error("affine guard conjunction exceeds its storage budget"));
    }
    let cells = equations
        .len()
        .checked_mul(columns)
        .filter(|&cells| cells <= MAX_MATRIX_CELLS)
        .ok_or_else(|| error("affine guard conjunction exceeds its matrix budget"))?;
    let input_bits = equations
        .iter()
        .flat_map(|equation| &equation.coefficients)
        .map(integer_magnitude_bits)
        .max()
        .unwrap_or(0) as usize;
    let input_bits = input_bits.max(substituted_bits);
    // Native RREF uses exact rational minors. This deliberately loose
    // Hadamard bound covers their numerator/denominator products, with no
    // assumption that a sparse input implies small intermediate integers.
    let bits = input_bits
        .checked_add(ceil_log2(columns) + 2)
        .and_then(|bits| bits.checked_mul(columns))
        .and_then(|bits| bits.checked_mul(4))
        .ok_or_else(|| error("affine guard conjunction bit estimate overflow"))?;
    let total_bits = cells
        .checked_mul(bits)
        .ok_or_else(|| error("affine guard conjunction total-bit overflow"))?;
    if bits > limits.indexed_algebra.max_specialization_integer_bits
        || total_bits > limits.guard_algebra.max_total_integer_bits
    {
        return Err(error(
            "affine guard conjunction exceeds its integer-bit budget",
        ));
    }
    let input_terms = equations.iter().try_fold(0usize, |total, equation| {
        total.checked_add(equation.nterms())
    });
    work.input_terms = work
        .input_terms
        .checked_add(input_terms.ok_or_else(|| error("affine guard conjunction term overflow"))?)
        .ok_or_else(|| error("affine guard conjunction aggregate-term overflow"))?;
    if work.input_terms > limits.guard_algebra.max_exact_hyperplane_replay_terms {
        return Err(error(
            "affine guard conjunction exceeds aggregate input-term budget",
        ));
    }
    let limbs = bits.div_ceil(usize::BITS as usize).max(1);
    let operations = cells
        .checked_mul(columns)
        .and_then(|value| value.checked_mul(limbs))
        .and_then(|value| value.checked_mul(limbs))
        .ok_or_else(|| error("affine guard conjunction matrix-work overflow"))?;
    work.charge(operations, limits)
}
