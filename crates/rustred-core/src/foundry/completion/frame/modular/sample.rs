use symbolica::domains::finite_field::{FiniteFieldCore, FiniteFieldElement, ToFiniteField, Zp64};
use symbolica::domains::{Field, Ring, RingOps};
use symbolica::prelude::Integer;
use symbolica::tensors::sparse::SparseMatrix;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::identity::TranslatedSource;

use super::super::PhysicalFramePlan;
use super::{
    ModularKernelError, ModularKernelLimits, ModularPhysicalFrame, ModularSampleFingerprint,
};

const POINT_COORDINATES: &str = "modular sample point coordinates";
const SOURCE_CONDITIONS: &str = "modular source conditions";
const STRUCTURAL_ENTRIES: &str = "modular structural entries";
const RETAINED_ENTRIES: &str = "modular retained nonzero entries";
const CSR_ROW_OFFSETS: &str = "modular CSR row offsets";
const EVALUATED_SOURCE_TERMS: &str = "modular evaluated exact-source terms";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScalarEvaluationError {
    DenominatorZero,
}

/// Row-independent failure while evaluating one complete exact translated
/// source at an already constructed finite-field point.
///
/// Condition ordinals and term ordinals are the exact sealed source order.
/// In particular, a caller can map them into its own row/column registry
/// without this primitive learning anything about a physical frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModularSourceEvaluationError {
    FrameContextMismatch,
    PointArityOverflow,
    WrongPointArity { expected: usize, actual: usize },
    ConditionContextMismatch { condition_ordinal: usize },
    ConditionZero { condition_ordinal: usize },
    TermContextMismatch { term_ordinal: usize },
    TermDenominatorZero { term_ordinal: usize },
    AllocationFailure { requested: usize },
}

/// A condition-admitted view of one exact translated source at one modular
/// point. Keeping condition admission separate lets physical-frame sampling
/// preserve its historical gate/CSR/term error chronology while sharing the
/// complete scalar evaluator with source-discovery callers.
struct ConditionAdmittedSource<'source, 'context, 'point, 'field> {
    context: &'context IndexedCoefficientContext,
    source: &'source TranslatedSource,
    point: &'point [FiniteFieldElement<u64>],
    field: &'field Zp64,
}

impl<'source, 'context, 'point, 'field> ConditionAdmittedSource<'source, 'context, 'point, 'field> {
    fn try_new(
        context: &'context IndexedCoefficientContext,
        source: &'source TranslatedSource,
        point: &'point [FiniteFieldElement<u64>],
        field: &'field Zp64,
    ) -> Result<Self, ModularSourceEvaluationError> {
        let expected_point_arity = context
            .base()
            .parameter_names()
            .len()
            .checked_add(context.index_count())
            .ok_or(ModularSourceEvaluationError::PointArityOverflow)?;
        if point.len() != expected_point_arity {
            return Err(ModularSourceEvaluationError::WrongPointArity {
                expected: expected_point_arity,
                actual: point.len(),
            });
        }
        for (condition_ordinal, nonzero) in source.nonzero_conditions().iter().enumerate() {
            context
                .validate_polynomial_context(nonzero.polynomial())
                .map_err(|_| ModularSourceEvaluationError::ConditionContextMismatch {
                    condition_ordinal,
                })?;
            if field.is_zero(&evaluate_polynomial(nonzero.polynomial(), point, field)) {
                return Err(ModularSourceEvaluationError::ConditionZero { condition_ordinal });
            }
        }
        Ok(Self {
            context,
            source,
            point,
            field,
        })
    }

    /// Evaluate every coefficient in exact term order, retaining modular
    /// zeros. The caller-owned buffer is reusable and is empty after any
    /// failure, so a partial source image can never escape this boundary.
    fn evaluate_all_into(
        self,
        output: &mut Vec<FiniteFieldElement<u64>>,
    ) -> Result<(), ModularSourceEvaluationError> {
        output.clear();
        output
            .try_reserve_exact(self.source.terms().len())
            .map_err(|_| ModularSourceEvaluationError::AllocationFailure {
                requested: self.source.terms().len(),
            })?;
        for (term_ordinal, coefficient) in self.source.terms().values().enumerate() {
            if self.context.bind_sealed(coefficient).is_err() {
                output.clear();
                return Err(ModularSourceEvaluationError::TermContextMismatch { term_ordinal });
            }
            let value = match evaluate_coefficient(coefficient, self.point, self.field) {
                Ok(value) => value,
                Err(ScalarEvaluationError::DenominatorZero) => {
                    output.clear();
                    return Err(ModularSourceEvaluationError::TermDenominatorZero { term_ordinal });
                }
            };
            output.push(value);
        }
        Ok(())
    }
}

fn evaluate_translated_source_at_point(
    context: &IndexedCoefficientContext,
    source: &TranslatedSource,
    point: &[FiniteFieldElement<u64>],
    field: &Zp64,
    output: &mut Vec<FiniteFieldElement<u64>>,
) -> Result<(), ModularSourceEvaluationError> {
    output.clear();
    ConditionAdmittedSource::try_new(context, source, point, field)?.evaluate_all_into(output)
}

impl ModularPhysicalFrame<'_> {
    /// Evaluate one complete exact translated source at this admitted sample.
    ///
    /// The finite-field domain and point cannot be supplied independently:
    /// both come from this sealed sample owner. Every source condition is
    /// checked before any coefficient, then every coefficient is evaluated in
    /// exact term order, including modular zeros and terms a downstream
    /// projection may not use. `output` is reusable and is empty after every
    /// failure. A caller supplying a source outside this plan must first
    /// authenticate that source's sealed batch against the plan scope; this
    /// scalar boundary additionally reports ordinal-local payload mismatches.
    pub(crate) fn try_evaluate_translated_source(
        &self,
        context: &IndexedCoefficientContext,
        source: &TranslatedSource,
        output: &mut Vec<FiniteFieldElement<u64>>,
    ) -> Result<(), ModularSourceEvaluationError> {
        output.clear();
        if context.fingerprint() != self.plan().context_fingerprint() {
            return Err(ModularSourceEvaluationError::FrameContextMismatch);
        }
        debug_assert_eq!(
            self.field().get_prime(),
            self.sample_fingerprint().modulus(),
            "admitted modular sample split its field from its point fingerprint"
        );
        evaluate_translated_source_at_point(context, source, self.point(), self.field(), output)
    }
}

impl PhysicalFramePlan {
    /// Evaluate every exact source row at one deterministic finite-field point.
    ///
    /// `chart_coordinates` are the nonnegative sector coordinates `x`.  They
    /// are mapped to actual indices as `n_i=x_i+1` on active slots and
    /// `n_i=-x_i` on inactive slots before native polynomial evaluation.
    pub(crate) fn try_modular_sample<'frame>(
        &'frame self,
        context: &IndexedCoefficientContext,
        modulus: u64,
        base_parameters: &[i64],
        chart_coordinates: &[u64],
        limits: ModularKernelLimits,
    ) -> Result<ModularPhysicalFrame<'frame>, ModularKernelError> {
        if context.fingerprint() != self.context_fingerprint() {
            return Err(ModularKernelError::WrongFrameContext);
        }
        validate_prime(modulus)?;
        let expected_base = context.base().parameter_names().len();
        if base_parameters.len() != expected_base {
            return Err(ModularKernelError::WrongBaseParameterArity {
                expected: expected_base,
                actual: base_parameters.len(),
            });
        }
        let arity = self.sector().arity();
        if context.index_count() != arity {
            return Err(ModularKernelError::WrongContextIndexArity {
                expected: arity,
                actual: context.index_count(),
            });
        }
        if chart_coordinates.len() != arity {
            return Err(ModularKernelError::WrongChartCoordinateArity {
                expected: arity,
                actual: chart_coordinates.len(),
            });
        }

        let point_count = checked_add(POINT_COORDINATES, expected_base, arity)?;
        check_limit(POINT_COORDINATES, point_count, limits.max_point_coordinates)?;
        check_limit(
            "modular matrix rows",
            self.row_count(),
            limits.max_matrix_rows,
        )?;
        check_limit(
            "modular matrix columns",
            self.columns().len(),
            limits.max_matrix_columns,
        )?;
        check_limit(
            STRUCTURAL_ENTRIES,
            self.entry_count(),
            limits.max_structural_entries,
        )?;
        // The sampled CSR can retain every structural entry.  Admit that
        // exact worst-case capacity before reserving either retained buffer.
        check_limit(
            RETAINED_ENTRIES,
            self.entry_count(),
            limits.max_retained_entries,
        )?;
        check_limit(
            CSR_ROW_OFFSETS,
            self.row_offsets().len(),
            limits.max_csr_row_offsets,
        )?;
        let row_count = checked_u32("modular matrix row count", self.row_count())?;
        let column_count = checked_u32("modular matrix column count", self.columns().len())?;

        let field = Zp64::new(modulus);
        let mut point = try_vec(POINT_COORDINATES, point_count)?;
        point.extend(
            base_parameters
                .iter()
                .map(|&value| Integer::from(value).to_finite_field(&field)),
        );
        for (&coordinate, &active) in chart_coordinates.iter().zip(self.sector().active_bits()) {
            let coordinate = coordinate.to_finite_field(&field);
            point.push(if active {
                field.add(&coordinate, &field.one())
            } else {
                field.neg(&coordinate)
            });
        }
        if point.len() != point_count {
            return Err(ModularKernelError::Invariant {
                detail: "constructed sample point has the wrong coordinate count",
            });
        }

        let condition_count = count_conditions(self)?;
        check_limit(
            SOURCE_CONDITIONS,
            condition_count,
            limits.max_source_conditions,
        )?;

        let mut values = try_vec(RETAINED_ENTRIES, self.entry_count())?;
        let mut column_indices = try_vec(RETAINED_ENTRIES, self.entry_count())?;
        let mut row_offsets = try_vec(CSR_ROW_OFFSETS, self.row_offsets().len())?;
        let mut evaluated_source = Vec::new();
        row_offsets.push(0usize);

        for row in 0..self.row_count() {
            let source = self
                .source_for_row(row)
                .ok_or(ModularKernelError::Invariant {
                    detail: "sample row is absent from its exact frame",
                })?;
            let admitted = ConditionAdmittedSource::try_new(context, source, &point, &field)
                .map_err(|error| map_source_evaluation_error(error, row, None))?;

            let structural_columns =
                self.column_indices_for_row(row)
                    .ok_or(ModularKernelError::Invariant {
                        detail: "sample row has invalid structural CSR bounds",
                    })?;
            if structural_columns.len() != source.terms().len() {
                return Err(ModularKernelError::Invariant {
                    detail: "sample row terms disagree with structural CSR",
                });
            }
            admitted
                .evaluate_all_into(&mut evaluated_source)
                .map_err(|error| {
                    map_source_evaluation_error(error, row, Some(structural_columns))
                })?;
            if evaluated_source.len() != structural_columns.len() {
                return Err(ModularKernelError::Invariant {
                    detail: "evaluated source terms disagree with structural CSR",
                });
            }
            for (value, &physical_column) in evaluated_source.iter().zip(structural_columns) {
                let physical_column = usize::try_from(physical_column).map_err(|_| {
                    ModularKernelError::Invariant {
                        detail: "physical CSR column does not fit usize",
                    }
                })?;
                if !field.is_zero(value) {
                    values.push(value.clone());
                    column_indices.push(checked_u32(
                        "modular retained physical column",
                        physical_column,
                    )?);
                }
            }
            row_offsets.push(values.len());
        }

        check_limit(RETAINED_ENTRIES, values.len(), limits.max_retained_entries)?;
        validate_csr(
            row_count,
            column_count,
            &values,
            &row_offsets,
            &column_indices,
            &field,
        )?;
        let matrix = SparseMatrix::from_csr(
            row_count,
            column_count,
            values,
            row_offsets,
            column_indices,
            field.clone(),
        );

        let sample = ModularSampleFingerprint::new(modulus, point.into_boxed_slice());
        Ok(ModularPhysicalFrame {
            plan: self,
            field,
            sample: std::sync::Arc::new(sample),
            matrix,
        })
    }
}

fn map_source_evaluation_error(
    error: ModularSourceEvaluationError,
    row: usize,
    structural_columns: Option<&[u32]>,
) -> ModularKernelError {
    match error {
        ModularSourceEvaluationError::FrameContextMismatch => ModularKernelError::WrongFrameContext,
        ModularSourceEvaluationError::ConditionContextMismatch { .. }
        | ModularSourceEvaluationError::TermContextMismatch { .. } => {
            ModularKernelError::WrongIndexedContext { row }
        }
        ModularSourceEvaluationError::ConditionZero { condition_ordinal } => {
            ModularKernelError::SourceConditionZero {
                row,
                condition: condition_ordinal,
            }
        }
        ModularSourceEvaluationError::TermDenominatorZero { term_ordinal } => {
            let Some(physical_column) = structural_columns
                .and_then(|columns| columns.get(term_ordinal))
                .and_then(|&column| usize::try_from(column).ok())
            else {
                return ModularKernelError::Invariant {
                    detail: "source-evaluation term is absent from structural CSR",
                };
            };
            ModularKernelError::CoefficientDenominatorZero {
                row,
                physical_column,
            }
        }
        ModularSourceEvaluationError::AllocationFailure { requested } => {
            ModularKernelError::AllocationFailure {
                resource: EVALUATED_SOURCE_TERMS,
                requested,
            }
        }
        ModularSourceEvaluationError::PointArityOverflow
        | ModularSourceEvaluationError::WrongPointArity { .. } => ModularKernelError::Invariant {
            detail: "exact-source evaluator received the wrong modular point arity",
        },
    }
}

fn validate_prime(modulus: u64) -> Result<(), ModularKernelError> {
    if modulus.is_multiple_of(2) {
        return Err(ModularKernelError::UnsupportedEvenModulus { modulus });
    }
    // Symbolica documents deterministic Miller--Rabin witnesses below
    // u64::MAX.  The excluded endpoint is 2^64-1 and therefore composite.
    if modulus == u64::MAX || !Integer::from(modulus).is_prime(0) {
        return Err(ModularKernelError::NonPrimeModulus { modulus });
    }
    Ok(())
}

fn count_conditions(plan: &PhysicalFramePlan) -> Result<usize, ModularKernelError> {
    let mut count = 0usize;
    for row in 0..plan.row_count() {
        let source = plan
            .source_for_row(row)
            .ok_or(ModularKernelError::Invariant {
                detail: "condition census row is absent from its exact frame",
            })?;
        count = checked_add(SOURCE_CONDITIONS, count, source.nonzero_conditions().len())?;
    }
    Ok(count)
}

fn evaluate_polynomial(
    polynomial: &IndexedPolynomial,
    point: &[symbolica::domains::finite_field::FiniteFieldElement<u64>],
    field: &Zp64,
) -> symbolica::domains::finite_field::FiniteFieldElement<u64> {
    polynomial.raw().evaluate_with_coeff_map(
        |coefficient| coefficient.to_finite_field(field),
        point,
        field,
    )
}

fn evaluate_coefficient(
    coefficient: &IndexedCoefficient,
    point: &[symbolica::domains::finite_field::FiniteFieldElement<u64>],
    field: &Zp64,
) -> Result<symbolica::domains::finite_field::FiniteFieldElement<u64>, ScalarEvaluationError> {
    let numerator = coefficient.raw().numerator.evaluate_with_coeff_map(
        |value| value.to_finite_field(field),
        point,
        field,
    );
    let denominator = coefficient.raw().denominator.evaluate_with_coeff_map(
        |value| value.to_finite_field(field),
        point,
        field,
    );
    if field.is_zero(&denominator) {
        return Err(ScalarEvaluationError::DenominatorZero);
    }
    Ok(field.div(&numerator, &denominator))
}

fn validate_csr(
    row_count: u32,
    column_count: u32,
    values: &[symbolica::domains::finite_field::FiniteFieldElement<u64>],
    row_offsets: &[usize],
    column_indices: &[u32],
    field: &Zp64,
) -> Result<(), ModularKernelError> {
    if values.len() != column_indices.len()
        || row_offsets.len() != row_count as usize + 1
        || row_offsets.first() != Some(&0)
        || row_offsets.last() != Some(&values.len())
        || row_offsets.windows(2).any(|pair| pair[0] > pair[1])
        || values.iter().any(|value| field.is_zero(value))
    {
        return Err(ModularKernelError::Invariant {
            detail: "sampled physical CSR failed shape or nonzero validation",
        });
    }
    for bounds in row_offsets.windows(2) {
        let row_columns =
            column_indices
                .get(bounds[0]..bounds[1])
                .ok_or(ModularKernelError::Invariant {
                    detail: "sampled physical CSR row bounds are invalid",
                })?;
        if row_columns.iter().any(|&column| column >= column_count)
            || row_columns.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ModularKernelError::Invariant {
                detail: "sampled physical CSR columns are unsorted or out of range",
            });
        }
    }
    Ok(())
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ModularKernelError> {
    if requested > limit {
        Err(ModularKernelError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ModularKernelError> {
    left.checked_add(right)
        .ok_or(ModularKernelError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ModularKernelError> {
    left.checked_mul(right)
        .ok_or(ModularKernelError::ResourceCountOverflow { resource })
}

pub(super) fn checked_u32(resource: &'static str, value: usize) -> Result<u32, ModularKernelError> {
    u32::try_from(value).map_err(|_| ModularKernelError::U32NotRepresentable { resource, value })
}

pub(super) fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, ModularKernelError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| ModularKernelError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}

#[cfg(test)]
pub(super) fn evaluate_coefficient_for_test(
    coefficient: &IndexedCoefficient,
    point: &[symbolica::domains::finite_field::FiniteFieldElement<u64>],
    field: &Zp64,
) -> bool {
    matches!(
        evaluate_coefficient(coefficient, point, field),
        Err(ScalarEvaluationError::DenominatorZero)
    )
}
