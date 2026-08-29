use std::collections::BTreeMap;

use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::family::AffineDenominator;

use super::budget::{
    coefficient_census, exact_operation_allocation_envelope, multiply_census,
    planned_coefficient_clone_census, planned_polynomial_clone_census, polynomial_census,
    verify_operation_result_envelope,
};
use super::construction::{check_limit, upper_triangular_count, upper_triangular_index};
use super::error::SymbolicaAffineDenominatorError;
use super::model::SymbolicaAffineDenominatorCompiler;
use super::normalize::charge_dense_degree_box;
use super::work::{
    BinaryOperation, CoefficientCensus, ExactWorkBudget, ProjectionAllocationBudget,
    ProjectionStats,
};

impl SymbolicaAffineDenominatorCompiler {
    pub(super) fn project_affine_denominator(
        &self,
        coefficient: &Coefficient,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<(AffineDenominator, ProjectionStats), SymbolicaAffineDenominatorError> {
        let base_count = self.base_count();
        let loops = self.loop_momenta.len();
        let externals = self.external_momenta.len();
        let loop_loop_count = upper_triangular_count(loops)?;
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.denominator, base_count)?,
            self.limits,
            "aggregate projected denominator terms",
        )?;
        let denominator = project_polynomial_prefix(
            &coefficient.denominator,
            &self.coefficients.template().denominator,
            base_count,
            self.limits.max_combined_exponent_entries,
        )?;
        let mut group_counts = BTreeMap::<ProjectionGroup, usize>::new();
        for (term, exponents) in coefficient.numerator.exponents_iter().enumerate() {
            let group = classify_numerator_term(
                exponents,
                self.combined.parameter_names().len(),
                base_count,
                loops,
                externals,
                loop_loop_count,
                term,
            )?;
            if let Some(count) = group_counts.get_mut(&group) {
                *count = count.checked_add(1).ok_or(
                    SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "projected group terms",
                    },
                )?;
            } else {
                let requested = projection_work
                    .groups
                    .checked_add(group_counts.len())
                    .and_then(|value| value.checked_add(1))
                    .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "aggregate projection groups",
                    })?;
                check_limit(
                    "aggregate projection groups",
                    requested,
                    self.limits.max_projection_groups,
                )?;
                projection_work.charge(
                    CoefficientCensus {
                        retained_bytes: std::mem::size_of::<(ProjectionGroup, usize)>()
                            .checked_add(64)
                            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                                resource: "projection count-group metadata bytes",
                            })?,
                        ..CoefficientCensus::default()
                    },
                    self.limits,
                    "aggregate projection count-group metadata terms",
                )?;
                group_counts.insert(group, 1);
            }
        }
        let projected_numerator_terms =
            group_counts.values().try_fold(0usize, |total, count| {
                total.checked_add(*count).ok_or(
                    SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "projected numerator terms",
                    },
                )
            })?;
        check_limit(
            "projected numerator terms",
            projected_numerator_terms,
            self.limits.max_projected_polynomial_terms,
        )?;
        let projected_exponent_entries = projected_numerator_terms.checked_mul(base_count).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "projected numerator exponent entries",
            },
        )?;
        check_limit(
            "projected numerator exponent entries",
            projected_exponent_entries,
            self.limits.max_projected_exponent_entries,
        )?;

        let projection_groups = group_counts.len();
        check_limit(
            "projection groups",
            projection_groups,
            self.limits.max_projection_groups,
        )?;
        let projection_denominator_replication_terms = projection_groups
            .checked_mul(denominator.nterms())
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "projection denominator replication terms",
            })?;
        check_limit(
            "projection denominator replication terms",
            projection_denominator_replication_terms,
            self.limits.max_projection_denominator_replication_terms,
        )?;
        let replication_entries = projection_denominator_replication_terms
            .checked_mul(base_count)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "projection denominator replication exponent entries",
            })?;
        check_limit(
            "projection denominator replication exponent entries",
            replication_entries,
            self.limits
                .max_projection_denominator_replication_exponent_entries,
        )?;
        let projection_gram_operations = group_counts
            .keys()
            .filter(|group| matches!(group, ProjectionGroup::ExternalPair(_, _)))
            .count()
            .checked_mul(2)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "projection Gram operations",
            })?;
        check_limit(
            "projection Gram operations",
            projection_gram_operations,
            self.limits.max_projection_gram_operations,
        )?;
        projection_work.charge_structure(
            projection_groups,
            projection_denominator_replication_terms,
            projection_gram_operations,
            self.limits,
        )?;

        let denominator_replication_census = multiply_census(
            polynomial_census(&denominator)?,
            projection_groups,
            "projection denominator replication census",
        )?;
        projection_work.charge(
            denominator_replication_census,
            self.limits,
            "aggregate projection denominator replication terms",
        )?;
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.numerator, base_count)?,
            self.limits,
            "aggregate projected group-polynomial terms",
        )?;
        let group_metadata_bytes = projection_groups
            .checked_mul(
                std::mem::size_of::<(ProjectionGroup, CoefficientPolynomial)>()
                    .checked_add(64)
                    .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "projection group metadata bytes",
                    })?,
            )
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "projection group metadata bytes",
            })?;
        projection_work.charge(
            CoefficientCensus {
                retained_bytes: group_metadata_bytes,
                ..CoefficientCensus::default()
            },
            self.limits,
            "aggregate projection group metadata terms",
        )?;

        let mut groups = BTreeMap::<ProjectionGroup, CoefficientPolynomial>::new();
        for (group, count) in group_counts {
            groups.insert(
                group,
                self.coefficients
                    .template()
                    .numerator
                    .zero_with_capacity(count),
            );
        }
        for (term, (integer, exponents)) in coefficient
            .numerator
            .coefficients
            .iter()
            .zip(coefficient.numerator.exponents_iter())
            .enumerate()
        {
            let group = classify_numerator_term(
                exponents,
                self.combined.parameter_names().len(),
                base_count,
                loops,
                externals,
                loop_loop_count,
                term,
            )?;
            groups
                .get_mut(&group)
                .ok_or(
                    SymbolicaAffineDenominatorError::InternalVerificationFailure {
                        detail: "projected group count was not retained",
                    },
                )?
                .append_monomial(integer.clone(), &exponents[..base_count]);
        }

        let zero_slots = self.coordinates.len().checked_add(1).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "projected affine coordinate baseline",
            },
        )?;
        projection_work.charge(
            multiply_census(
                planned_coefficient_clone_census(self.coefficients.template(), self.base_count())?,
                zero_slots,
                "projected affine coordinate baseline",
            )?,
            self.limits,
            "aggregate projected affine coordinate baseline terms",
        )?;
        let zero = self.coefficients.zero();
        let mut constant = zero.clone();
        let mut coordinates = Vec::new();
        coordinates
            .try_reserve_exact(self.coordinates.len())
            .map_err(|_| SymbolicaAffineDenominatorError::AllocationFailure {
                resource: "affine scalar-product coefficients",
                requested: self.coordinates.len(),
            })?;
        coordinates.resize_with(self.coordinates.len(), || zero.clone());
        for (group, numerator) in groups {
            let value =
                self.projected_rational(numerator, denominator.clone(), work, projection_work)?;
            match group {
                ProjectionGroup::Constant => constant = value,
                ProjectionGroup::Coordinate(position) => {
                    let target = coordinates.get_mut(position).ok_or(
                        SymbolicaAffineDenominatorError::InternalVerificationFailure {
                            detail: "quadratic coordinate index is out of range",
                        },
                    )?;
                    *target = value;
                }
                ProjectionGroup::ExternalPair(left, right) => {
                    let gram = self
                        .external_gram
                        .get(left)
                        .and_then(|row| row.get(right))
                        .ok_or(
                            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                                detail: "external Gram coordinate is out of range",
                            },
                        )?;
                    let contribution =
                        self.projected_checked_mul(&value, gram, work, projection_work)?;
                    constant = self.projected_checked_add(
                        &constant,
                        &contribution,
                        work,
                        projection_work,
                    )?;
                }
            }
        }
        self.coefficients
            .validate_with_limits(&constant, self.limits.exact_algebra)?;
        for value in &coordinates {
            self.coefficients
                .validate_with_limits(value, self.limits.exact_algebra)?;
        }
        let mut output_census = coefficient_census(&constant)?;
        for value in &coordinates {
            output_census
                .checked_add_assign(coefficient_census(value)?, "projected affine-row census")?;
        }
        check_limit(
            "projected polynomial terms",
            output_census.polynomial_terms,
            self.limits.max_projected_polynomial_terms,
        )?;
        check_limit(
            "projected exponent entries",
            output_census.exponent_entries,
            self.limits.max_projected_exponent_entries,
        )?;
        check_limit(
            "projected integer bits",
            output_census.integer_bits,
            self.limits.max_projected_integer_bits,
        )?;
        check_limit(
            "projected retained bytes",
            output_census.retained_bytes,
            self.limits.max_projected_retained_bytes,
        )?;
        Ok((
            AffineDenominator::new(constant, coordinates),
            ProjectionStats {
                projected_retained_bytes: output_census.retained_bytes,
            },
        ))
    }

    pub(super) fn project_complete_coefficient(
        &self,
        coefficient: &Coefficient,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.numerator, self.base_count())?,
            self.limits,
            "aggregate projected complete numerator terms",
        )?;
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.denominator, self.base_count())?,
            self.limits,
            "aggregate projected complete denominator terms",
        )?;
        let numerator = project_polynomial_prefix(
            &coefficient.numerator,
            &self.coefficients.template().numerator,
            self.base_count(),
            self.limits.max_combined_exponent_entries,
        )?;
        let denominator = project_polynomial_prefix(
            &coefficient.denominator,
            &self.coefficients.template().denominator,
            self.base_count(),
            self.limits.max_combined_exponent_entries,
        )?;
        self.projected_rational(numerator, denominator, work, projection_work)
    }

    pub(super) fn projected_rational(
        &self,
        numerator: CoefficientPolynomial,
        denominator: CoefficientPolynomial,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        if numerator.is_zero() {
            projection_work.charge(
                planned_coefficient_clone_census(self.coefficients.template(), self.base_count())?,
                self.limits,
                "aggregate projected zero coefficient terms",
            )?;
            return Ok(self.coefficients.zero());
        }
        let numerator: Coefficient = numerator.into();
        let denominator: Coefficient = denominator.into();
        self.projected_checked_div(&numerator, &denominator, work, projection_work)
    }

    pub(super) fn validate_projected_coefficient(
        &self,
        coefficient: &Coefficient,
    ) -> Result<(), SymbolicaAffineDenominatorError> {
        self.coefficients
            .validate_with_limits(coefficient, self.limits.exact_algebra)?;
        let census = coefficient_census(coefficient)?;
        check_limit(
            "one projected coefficient integer bits",
            census.integer_bits,
            self.limits.max_projected_integer_bits,
        )
    }

    pub(super) fn projected_checked_add(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        charge_dense_degree_box(
            left,
            right,
            BinaryOperation::Add,
            self.base_count(),
            self.limits,
            work,
        )?;
        let allocation = exact_operation_allocation_envelope(
            left,
            right,
            BinaryOperation::Add,
            self.base_count(),
        )?;
        projection_work.charge(
            allocation.census,
            self.limits,
            "aggregate projected exact-operation terms",
        )?;
        let result = self
            .coefficients
            .try_add(left, right, self.limits.exact_algebra)?;
        self.validate_projected_coefficient(&result)?;
        verify_operation_result_envelope(&result, coefficient_census(&result)?, allocation)?;
        Ok(result)
    }

    pub(super) fn projected_checked_mul(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        charge_dense_degree_box(
            left,
            right,
            BinaryOperation::Multiply,
            self.base_count(),
            self.limits,
            work,
        )?;
        let allocation = exact_operation_allocation_envelope(
            left,
            right,
            BinaryOperation::Multiply,
            self.base_count(),
        )?;
        projection_work.charge(
            allocation.census,
            self.limits,
            "aggregate projected exact-operation terms",
        )?;
        let result = self
            .coefficients
            .try_mul(left, right, self.limits.exact_algebra)?;
        self.validate_projected_coefficient(&result)?;
        verify_operation_result_envelope(&result, coefficient_census(&result)?, allocation)?;
        Ok(result)
    }

    pub(super) fn projected_checked_div(
        &self,
        numerator: &Coefficient,
        denominator: &Coefficient,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        charge_dense_degree_box(
            numerator,
            denominator,
            BinaryOperation::Divide,
            self.base_count(),
            self.limits,
            work,
        )?;
        let allocation = exact_operation_allocation_envelope(
            numerator,
            denominator,
            BinaryOperation::Divide,
            self.base_count(),
        )?;
        projection_work.charge(
            allocation.census,
            self.limits,
            "aggregate projected exact-operation terms",
        )?;
        let result =
            self.coefficients
                .try_div(numerator, denominator, self.limits.exact_algebra)?;
        self.validate_projected_coefficient(&result)?;
        verify_operation_result_envelope(&result, coefficient_census(&result)?, allocation)?;
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ProjectionGroup {
    Constant,
    Coordinate(usize),
    ExternalPair(usize, usize),
}

#[allow(clippy::too_many_arguments)]
pub(super) fn classify_numerator_term(
    exponents: &[u16],
    expected_variables: usize,
    base_count: usize,
    loops: usize,
    externals: usize,
    loop_loop_count: usize,
    numerator_term: usize,
) -> Result<ProjectionGroup, SymbolicaAffineDenominatorError> {
    if exponents.len() != expected_variables {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "combined numerator exponent row has the wrong length",
            },
        );
    }
    match momentum_degree(exponents, base_count)? {
        0 => Ok(ProjectionGroup::Constant),
        1 => Err(SymbolicaAffineDenominatorError::MomentumDegreeOne { numerator_term }),
        2 => classify_quadratic_group(
            &exponents[base_count..],
            loops,
            externals,
            loop_loop_count,
            numerator_term,
        ),
        degree => Err(SymbolicaAffineDenominatorError::MomentumDegreeTooHigh {
            numerator_term,
            degree,
        }),
    }
}

fn classify_quadratic_group(
    momentum_exponents: &[u16],
    loops: usize,
    externals: usize,
    loop_loop_count: usize,
    numerator_term: usize,
) -> Result<ProjectionGroup, SymbolicaAffineDenominatorError> {
    let mut first = None;
    let mut second = None;
    for (position, &exponent) in momentum_exponents.iter().enumerate() {
        if exponent == 0 {
            continue;
        }
        if first.is_none() {
            first = Some((position, exponent));
        } else if second.is_none() {
            second = Some((position, exponent));
        } else {
            return Err(
                SymbolicaAffineDenominatorError::InvalidQuadraticMomentumMonomial {
                    numerator_term,
                },
            );
        }
    }
    let (left, right) = match (first, second) {
        (Some((position, 2)), None) => (position, position),
        (Some((left, 1)), Some((right, 1))) => (left, right),
        _ => {
            return Err(
                SymbolicaAffineDenominatorError::InvalidQuadraticMomentumMonomial {
                    numerator_term,
                },
            );
        }
    };
    let momentum_count = loops.checked_add(externals).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "quadratic momentum positions",
        },
    )?;
    if right >= momentum_count {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "quadratic momentum position exceeds the combined map",
            },
        );
    }
    match (left < loops, right < loops) {
        (true, true) => Ok(ProjectionGroup::Coordinate(upper_triangular_index(
            left, right, loops,
        )?)),
        (true, false) => {
            let external = right - loops;
            let offset = left.checked_mul(externals).and_then(|value| {
                loop_loop_count
                    .checked_add(value)
                    .and_then(|value| value.checked_add(external))
            });
            Ok(ProjectionGroup::Coordinate(offset.ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "loop-external coordinate index",
                },
            )?))
        }
        (false, true) => Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "canonical momentum exponents reversed a loop-external pair",
            },
        ),
        (false, false) => Ok(ProjectionGroup::ExternalPair(left - loops, right - loops)),
    }
}

pub(super) fn project_polynomial_prefix(
    source: &CoefficientPolynomial,
    target_template: &CoefficientPolynomial,
    retained: usize,
    max_exponent_entries: usize,
) -> Result<CoefficientPolynomial, SymbolicaAffineDenominatorError> {
    if target_template.variables.len() != retained {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "base projection target has the wrong variable count",
            },
        );
    }
    let expected_source_entries = source.nterms().checked_mul(source.variables.len()).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "source projection exponent entries",
        },
    )?;
    if source.exponents.len() != expected_source_entries {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "source projection polynomial has a malformed exponent layout",
            },
        );
    }
    let target_entries = source.nterms().checked_mul(retained).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "target projection exponent entries",
        },
    )?;
    check_limit(
        "target projection exponent entries",
        target_entries,
        max_exponent_entries,
    )?;
    let mut target = target_template.zero_with_capacity(source.nterms());
    for (integer, exponents) in source.coefficients.iter().zip(source.exponents_iter()) {
        if exponents.len() != source.variables.len() || exponents.len() < retained {
            return Err(
                SymbolicaAffineDenominatorError::InternalVerificationFailure {
                    detail: "combined polynomial exponent row is too short",
                },
            );
        }
        if exponents[retained..].iter().any(|exponent| *exponent != 0) {
            return Err(SymbolicaAffineDenominatorError::BaseCoefficientContainsMomentum);
        }
        target.append_monomial(integer.clone(), &exponents[..retained]);
    }
    Ok(target)
}

pub(super) fn lift_polynomial_prefix(
    source: &CoefficientPolynomial,
    target_template: &CoefficientPolynomial,
    retained: usize,
    max_exponent_entries: usize,
) -> Result<CoefficientPolynomial, SymbolicaAffineDenominatorError> {
    if source.variables.len() != retained || target_template.variables.len() < retained {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "base lift uses incompatible variable maps",
            },
        );
    }
    let target_variables = target_template.variables.len();
    let target_entries = source.nterms().checked_mul(target_variables).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "base lift exponent entries",
        },
    )?;
    check_limit(
        "base lift exponent entries",
        target_entries,
        max_exponent_entries,
    )?;
    let mut exponent_row = Vec::new();
    exponent_row
        .try_reserve_exact(target_variables)
        .map_err(|_| SymbolicaAffineDenominatorError::AllocationFailure {
            resource: "base lift exponent row",
            requested: target_variables,
        })?;
    exponent_row.resize(target_variables, 0u16);
    let mut target = target_template.zero_with_capacity(source.nterms());
    for (integer, exponents) in source.coefficients.iter().zip(source.exponents_iter()) {
        if exponents.len() != retained {
            return Err(
                SymbolicaAffineDenominatorError::InternalVerificationFailure {
                    detail: "base lift source exponent row has the wrong width",
                },
            );
        }
        exponent_row[..retained].copy_from_slice(exponents);
        target.append_monomial(integer.clone(), &exponent_row);
    }
    Ok(target)
}

pub(super) fn reject_momentum_denominator(
    coefficient: &Coefficient,
    base_count: usize,
) -> Result<(), SymbolicaAffineDenominatorError> {
    if polynomial_contains_momentum(&coefficient.denominator, base_count)? {
        Err(SymbolicaAffineDenominatorError::MomentumDependentRationalDenominator)
    } else {
        Ok(())
    }
}

pub(super) fn coefficient_contains_momentum(
    coefficient: &Coefficient,
    base_count: usize,
) -> Result<bool, SymbolicaAffineDenominatorError> {
    Ok(
        polynomial_contains_momentum(&coefficient.numerator, base_count)?
            || polynomial_contains_momentum(&coefficient.denominator, base_count)?,
    )
}

pub(super) fn polynomial_contains_momentum(
    polynomial: &CoefficientPolynomial,
    base_count: usize,
) -> Result<bool, SymbolicaAffineDenominatorError> {
    if polynomial.variables.len() < base_count {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "polynomial variable map is shorter than the base map",
            },
        );
    }
    Ok(polynomial.exponents_iter().any(|exponents| {
        exponents[base_count..]
            .iter()
            .any(|exponent| *exponent != 0)
    }))
}

pub(super) fn momentum_degree(
    exponents: &[u16],
    base_count: usize,
) -> Result<u32, SymbolicaAffineDenominatorError> {
    let suffix = exponents.get(base_count..).ok_or(
        SymbolicaAffineDenominatorError::InternalVerificationFailure {
            detail: "polynomial exponent row is shorter than the base map",
        },
    )?;
    suffix.iter().try_fold(0u32, |degree, exponent| {
        degree.checked_add(u32::from(*exponent)).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "momentum degree",
            },
        )
    })
}
