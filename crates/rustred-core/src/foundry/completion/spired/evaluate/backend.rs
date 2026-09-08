use symbolica::domains::finite_field::{FiniteFieldCore, FiniteFieldElement, ToFiniteField, Zp64};

use crate::algebra::CoefficientPolynomial;
use crate::identity::ParametricRelation;

use super::DirectShiftedSourceError;

/// Modular-image seam for immutable unshifted exact source data.
///
/// Conditions and term parts remain separate so samples are guard-rejected
/// before term evaluation and every denominator is checked explicitly before
/// division. This seam is independent of rational-function reconstruction.
pub(super) trait ShiftedCoefficientBackend {
    fn try_evaluate_conditions(
        &mut self,
        source_ordinal: usize,
        source: &ParametricRelation,
        point: &[FiniteFieldElement<u64>],
        field: &Zp64,
        output: &mut Vec<FiniteFieldElement<u64>>,
    ) -> Result<(), DirectShiftedSourceError>;

    fn try_evaluate_term_parts(
        &mut self,
        source_ordinal: usize,
        source: &ParametricRelation,
        point: &[FiniteFieldElement<u64>],
        field: &Zp64,
        output: &mut Vec<FiniteFieldElement<u64>>,
    ) -> Result<(), DirectShiftedSourceError>;
}

/// Allocation-free Symbolica sparse-polynomial evaluation after scratch has
/// been admitted by the caller.
///
/// The pinned Symbolica multi-output evaluator can preserve explicit
/// numerator/denominator outputs, but compiling it requires constructing Atom
/// programs and owns unbounded optimizer allocations. Adopting it also needs a
/// source-set preparation layer so compilation is shared across modular points.
/// Keep this bounded sparse fallback until representative release benchmarks
/// justify that architectural boundary; do not implement a local evaluator.
#[derive(Debug, Default)]
pub(super) struct SparseExactPolynomialBackend {
    #[cfg(test)]
    forced_zero_denominator: Option<usize>,
}

impl SparseExactPolynomialBackend {
    #[cfg(test)]
    pub(super) fn force_zero_denominator_for_test(&mut self, term_ordinal: usize) {
        self.forced_zero_denominator = Some(term_ordinal);
    }
}

impl ShiftedCoefficientBackend for SparseExactPolynomialBackend {
    fn try_evaluate_conditions(
        &mut self,
        _source_ordinal: usize,
        source: &ParametricRelation,
        point: &[FiniteFieldElement<u64>],
        field: &Zp64,
        output: &mut Vec<FiniteFieldElement<u64>>,
    ) -> Result<(), DirectShiftedSourceError> {
        output.clear();
        output.extend(
            source
                .nonzero_conditions()
                .iter()
                .map(|condition| evaluate_polynomial(condition.polynomial().raw(), point, field)),
        );
        Ok(())
    }

    fn try_evaluate_term_parts(
        &mut self,
        _source_ordinal: usize,
        source: &ParametricRelation,
        point: &[FiniteFieldElement<u64>],
        field: &Zp64,
        output: &mut Vec<FiniteFieldElement<u64>>,
    ) -> Result<(), DirectShiftedSourceError> {
        output.clear();
        for coefficient in source.terms().values() {
            output.push(evaluate_polynomial(
                &coefficient.raw().numerator,
                point,
                field,
            ));
            output.push(evaluate_polynomial(
                &coefficient.raw().denominator,
                point,
                field,
            ));
        }
        #[cfg(test)]
        if let Some(term_ordinal) = self.forced_zero_denominator.take()
            && let Some(denominator) =
                output.get_mut(term_ordinal.saturating_mul(2).saturating_add(1))
        {
            *denominator = field.to_element(0);
        }
        Ok(())
    }
}

pub(super) fn evaluate_polynomial(
    polynomial: &CoefficientPolynomial,
    point: &[FiniteFieldElement<u64>],
    field: &Zp64,
) -> FiniteFieldElement<u64> {
    polynomial.evaluate_with_coeff_map(
        |coefficient| coefficient.to_finite_field(field),
        point,
        field,
    )
}
