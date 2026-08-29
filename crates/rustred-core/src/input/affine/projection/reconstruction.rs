use std::collections::BTreeMap;

use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::family::AffineDenominator;

use super::super::budget::{
    coefficient_census, multiply_census, planned_coefficient_clone_census,
    planned_polynomial_clone_census, polynomial_census,
};
use super::super::construction::{check_limit, upper_triangular_count};
use super::super::error::SymbolicaAffineDenominatorError;
use super::super::model::SymbolicaAffineDenominatorCompiler;
use super::super::work::{
    CoefficientCensus, ExactWorkBudget, ProjectionAllocationBudget, ProjectionStats,
};
use super::classification::{ProjectionGroup, classify_numerator_term};
use super::rational::project_polynomial_prefix;

impl SymbolicaAffineDenominatorCompiler {
    pub(in crate::input::affine) fn project_affine_denominator(
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
}
