//! Deterministic structural ordering, not a replacement for native admission.
use super::{BaseCoefficientEquation, BaseCoefficientSystem, IndexedAlgebraError};

/// Any coefficient equation can disprove the simultaneous zero locus. Try a
/// sparse/simple one first without copying expressions or changing canonical
/// storage order. This is only a heuristic: each attempted equation still runs
/// every original payload, degree, prospective-work and native-output check.
/// In particular, computing a key must not reject an expensive equation before
/// a cheaper equation has had the opportunity to settle the predicate.
pub(super) fn inexpensive_first(
    system: &BaseCoefficientSystem,
    base_count: usize,
) -> Result<impl Iterator<Item = &BaseCoefficientEquation>, IndexedAlgebraError> {
    let mut indexed = Vec::new();
    indexed
        .try_reserve_exact(system.equations.len())
        .map_err(|_| IndexedAlgebraError::AllocationFailure {
            resource: "guard coefficient ordering",
            requested: system.equations.len(),
        })?;
    for (ordinal, equation) in system.equations.iter().enumerate() {
        let p = equation.index_polynomial.raw();
        let mut total_degree = 0u128;
        let mut max_degree = 0u16;
        for axis in base_count..p.nvars() {
            let degree = p.degree(axis);
            total_degree += u128::from(degree);
            max_degree = max_degree.max(degree);
        }
        indexed.push(((p.nterms(), total_degree, max_degree, ordinal), equation));
    }
    indexed.sort_unstable_by_key(|(key, _)| *key);
    Ok(indexed.into_iter().map(|(_, equation)| equation))
}

#[cfg(test)]
#[path = "ordering_tests.rs"]
mod tests;
