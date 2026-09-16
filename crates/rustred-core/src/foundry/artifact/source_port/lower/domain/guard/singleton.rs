//! Retain an exact singleton face through the whole guard-proof pipeline.
//! Algebra and its allocation preflight remain in the native-backed context.

use super::*;

pub(super) fn restrict(
    context: &IndexedCoefficientContext,
    polynomial: &IndexedPolynomial,
    piece: &LatticeBox,
    sector: &[bool],
    limits: RuleCellLimits,
    work: &mut Work,
) -> Result<Option<IndexedPolynomial>, SourcePortAuditError> {
    if piece.arity() != sector.len() || sector.len() != context.index_count() {
        return Err(error("affine guard singleton has incompatible arity"));
    }
    // Admit the entire original before a fixed zero can erase a term or an
    // oversized coefficient. Native specialization also preflights growth.
    context
        .base_coefficient_system(polynomial, limits.indexed_algebra, limits.guard_algebra)
        .map_err(error)?;
    let mut fixed = Vec::new();
    fixed
        .try_reserve_exact(sector.len())
        .map_err(|_| error("affine guard singleton allocation failed"))?;
    for (axis, &active) in sector.iter().enumerate() {
        if piece.upper()[axis] != Some(piece.lower()[axis])
            || !polynomial
                .raw()
                .contains(context.base().variables().len() + axis)
        {
            continue;
        }
        let local = i128::from(piece.lower()[axis]);
        let physical = if active { local + 1 } else { -local };
        // Do not narrow wide mathematical coordinates to the API's i64
        // carrier. Omitting such a restriction only enlarges the proof domain.
        if let Ok(physical) = i64::try_from(physical) {
            fixed.push((axis, physical));
        }
    }
    if fixed.is_empty() {
        return Ok(None);
    }
    let terms = fixed
        .len()
        .checked_mul(polynomial.raw().nterms())
        .ok_or_else(|| error("affine guard singleton replay term overflow"))?;
    work.input_terms = work
        .input_terms
        .checked_add(terms)
        .ok_or_else(|| error("affine guard input-term count overflow"))?;
    if work.input_terms > limits.guard_algebra.max_exact_hyperplane_replay_terms {
        return Err(error("affine guard exceeds aggregate input-term budget"));
    }
    for _ in &fixed {
        work.charge(polynomial.raw().nterms(), limits)?;
    }
    let restricted = context
        .specialize_fixed_polynomial(polynomial, &fixed, limits.indexed_algebra)
        .map_err(error)?;
    context
        .base_coefficient_system(&restricted, limits.indexed_algebra, limits.guard_algebra)
        .map_err(error)?;
    Ok(Some(restricted))
}
