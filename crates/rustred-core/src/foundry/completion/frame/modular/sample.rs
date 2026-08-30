use symbolica::domains::finite_field::{ToFiniteField, Zp64};
use symbolica::domains::{Field, Ring, RingOps};
use symbolica::prelude::Integer;
use symbolica::tensors::sparse::SparseMatrix;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};

use super::super::PhysicalFramePlan;
use super::{
    ModularKernelError, ModularKernelLimits, ModularPhysicalFrame, ModularSampleFingerprint,
};

const POINT_COORDINATES: &str = "modular sample point coordinates";
const SOURCE_CONDITIONS: &str = "modular source conditions";
const STRUCTURAL_ENTRIES: &str = "modular structural entries";
const RETAINED_ENTRIES: &str = "modular retained nonzero entries";
const CSR_ROW_OFFSETS: &str = "modular CSR row offsets";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScalarEvaluationError {
    DenominatorZero,
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
        row_offsets.push(0usize);

        for row in 0..self.row_count() {
            let source = self
                .source_for_row(row)
                .ok_or(ModularKernelError::Invariant {
                    detail: "sample row is absent from its exact frame",
                })?;
            for (condition, nonzero) in source.nonzero_conditions().iter().enumerate() {
                context
                    .validate_polynomial_context(nonzero.polynomial())
                    .map_err(|_| ModularKernelError::WrongIndexedContext { row })?;
                if field.is_zero(&evaluate_polynomial(nonzero.polynomial(), &point, &field)) {
                    return Err(ModularKernelError::SourceConditionZero { row, condition });
                }
            }

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
            for ((_, coefficient), &physical_column) in
                source.terms().iter().zip(structural_columns)
            {
                context
                    .bind_sealed(coefficient)
                    .map_err(|_| ModularKernelError::WrongIndexedContext { row })?;
                let physical_column = usize::try_from(physical_column).map_err(|_| {
                    ModularKernelError::Invariant {
                        detail: "physical CSR column does not fit usize",
                    }
                })?;
                let value =
                    evaluate_coefficient(coefficient, &point, &field).map_err(
                        |error| match error {
                            ScalarEvaluationError::DenominatorZero => {
                                ModularKernelError::CoefficientDenominatorZero {
                                    row,
                                    physical_column,
                                }
                            }
                        },
                    )?;
                if !field.is_zero(&value) {
                    values.push(value);
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
