use std::collections::BTreeMap;

use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::super::budget::{
    exact_operation_allocation_envelope, multiply_census, planned_coefficient_clone_census,
    planned_polynomial_clone_census, polynomial_census,
};
use super::super::construction::{check_limit, upper_triangular_count};
use super::super::error::SymbolicaAffineDenominatorError;
use super::super::model::SymbolicaAffineDenominatorCompiler;
use super::super::projection::{
    ProjectionGroup, classify_numerator_term, momentum_degree, polynomial_contains_momentum,
    project_polynomial_prefix,
};
use super::super::work::{
    BinaryOperation, CoefficientCensus, ExactWorkBudget, ProjectionAllocationBudget,
};

impl SymbolicaAffineDenominatorCompiler {
    pub(in crate::input::affine) fn validate_vector_linear(
        &self,
        coefficient: &Coefficient,
        argument: usize,
        atom: symbolica::prelude::AtomView<'_>,
    ) -> Result<(), SymbolicaAffineDenominatorError> {
        if polynomial_contains_momentum(&coefficient.denominator, self.base_count())? {
            return Err(
                SymbolicaAffineDenominatorError::InvalidScalarProductArgument {
                    argument,
                    atom: atom.to_owned(),
                },
            );
        }
        for exponents in coefficient.numerator.exponents_iter() {
            if momentum_degree(exponents, self.base_count())? != 1 {
                return Err(
                    SymbolicaAffineDenominatorError::InvalidScalarProductArgument {
                        argument,
                        atom: atom.to_owned(),
                    },
                );
            }
        }
        Ok(())
    }

    pub(in crate::input::affine) fn contract_explicit_scalar_product(
        &self,
        coefficient: Coefficient,
        work: &mut ExactWorkBudget,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let base_count = self.base_count();
        let loops = self.loop_momenta.len();
        let externals = self.external_momenta.len();
        let loop_loop_count = upper_triangular_count(loops)?;
        let mut external_counts = BTreeMap::<(usize, usize), usize>::new();
        let mut residual_terms = 0usize;
        for (term, exponents) in coefficient.numerator.exponents_iter().enumerate() {
            match classify_numerator_term(
                exponents,
                self.combined.parameter_names().len(),
                base_count,
                loops,
                externals,
                loop_loop_count,
                term,
            )? {
                ProjectionGroup::ExternalPair(left, right) => {
                    let pair = (left, right);
                    if let Some(count) = external_counts.get_mut(&pair) {
                        *count = count.checked_add(1).ok_or(
                            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                                resource: "explicit scalar-product external terms",
                            },
                        )?;
                    } else {
                        let requested = projection_work
                            .groups
                            .checked_add(external_counts.len())
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
                                retained_bytes: std::mem::size_of::<((usize, usize), usize)>()
                                    .checked_add(64)
                                    .ok_or(
                                        SymbolicaAffineDenominatorError::ResourceCountOverflow {
                                            resource: "explicit scalar-product group metadata bytes",
                                        },
                                    )?,
                                ..CoefficientCensus::default()
                            },
                            self.limits,
                            "aggregate explicit scalar-product group metadata terms",
                        )?;
                        // `BTreeMap::insert` is the first allocation for this
                        // unique group and happens only after admission.
                        external_counts.insert(pair, 1);
                    }
                }
                ProjectionGroup::Coordinate(_) => {
                    residual_terms = residual_terms.checked_add(1).ok_or(
                        SymbolicaAffineDenominatorError::ResourceCountOverflow {
                            resource: "explicit scalar-product residual terms",
                        },
                    )?;
                }
                ProjectionGroup::Constant => {
                    return Err(
                        SymbolicaAffineDenominatorError::InternalVerificationFailure {
                            detail: "homogeneous scalar product produced a constant numerator term",
                        },
                    );
                }
            }
        }
        if external_counts.is_empty() {
            return Ok(coefficient);
        }

        let groups = external_counts.len();
        let denominator_terms = coefficient.denominator.nterms();
        let denominator_replication_terms = groups.checked_mul(denominator_terms).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "explicit scalar-product denominator replication terms",
            },
        )?;
        let gram_operations = groups.checked_mul(2).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "explicit scalar-product Gram operations",
            },
        )?;
        projection_work.charge_structure(
            groups,
            denominator_replication_terms,
            gram_operations,
            self.limits,
        )?;
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.denominator, base_count)?,
            self.limits,
            "aggregate explicit scalar-product denominator terms",
        )?;
        let denominator = project_polynomial_prefix(
            &coefficient.denominator,
            &self.coefficients.template().denominator,
            base_count,
            self.limits.max_projected_exponent_entries,
        )?;
        projection_work.charge(
            multiply_census(
                polynomial_census(&denominator)?,
                groups,
                "explicit scalar-product denominator replication census",
            )?,
            self.limits,
            "aggregate explicit scalar-product denominator replication terms",
        )?;
        // Both allocations below are charged with the complete source support;
        // this safely overbounds the disjoint external/residual partition.
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.numerator, base_count)?,
            self.limits,
            "aggregate explicit scalar-product external group terms",
        )?;
        projection_work.charge(
            planned_polynomial_clone_census(
                &coefficient.numerator,
                self.combined.parameter_names().len(),
            )?,
            self.limits,
            "aggregate explicit scalar-product residual terms",
        )?;
        let external_group_metadata_bytes = groups
            .checked_mul(
                std::mem::size_of::<((usize, usize), CoefficientPolynomial)>()
                    .checked_add(64)
                    .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                        resource: "explicit scalar-product group metadata bytes",
                    })?,
            )
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "explicit scalar-product group metadata bytes",
            })?;
        projection_work.charge(
            CoefficientCensus {
                retained_bytes: external_group_metadata_bytes,
                ..CoefficientCensus::default()
            },
            self.limits,
            "aggregate explicit scalar-product group metadata terms",
        )?;

        let mut external_groups = BTreeMap::<(usize, usize), CoefficientPolynomial>::new();
        for (pair, count) in external_counts {
            external_groups.insert(
                pair,
                self.coefficients
                    .template()
                    .numerator
                    .zero_with_capacity(count),
            );
        }
        let mut residual_numerator = coefficient.numerator.zero_with_capacity(residual_terms);
        for (term, (integer, exponents)) in coefficient
            .numerator
            .coefficients
            .iter()
            .zip(coefficient.numerator.exponents_iter())
            .enumerate()
        {
            match classify_numerator_term(
                exponents,
                self.combined.parameter_names().len(),
                base_count,
                loops,
                externals,
                loop_loop_count,
                term,
            )? {
                ProjectionGroup::ExternalPair(left, right) => external_groups
                    .get_mut(&(left, right))
                    .ok_or(
                        SymbolicaAffineDenominatorError::InternalVerificationFailure {
                            detail: "explicit scalar-product group count was not retained",
                        },
                    )?
                    .append_monomial(integer.clone(), &exponents[..base_count]),
                ProjectionGroup::Coordinate(_) => {
                    residual_numerator.append_monomial(integer.clone(), exponents)
                }
                ProjectionGroup::Constant => {
                    return Err(
                        SymbolicaAffineDenominatorError::InternalVerificationFailure {
                            detail: "homogeneous scalar product produced a constant numerator term",
                        },
                    );
                }
            }
        }

        let mut residual = if residual_numerator.is_zero() {
            self.combined.zero()
        } else {
            let numerator: Coefficient = residual_numerator.into();
            projection_work.charge(
                planned_polynomial_clone_census(
                    &coefficient.denominator,
                    self.combined.parameter_names().len(),
                )?,
                self.limits,
                "aggregate explicit scalar-product residual denominator terms",
            )?;
            let denominator_coefficient: Coefficient = coefficient.denominator.clone().into();
            projection_work.charge(
                exact_operation_allocation_envelope(
                    &numerator,
                    &denominator_coefficient,
                    BinaryOperation::Divide,
                    self.combined.parameter_names().len(),
                )?
                .census,
                self.limits,
                "aggregate explicit scalar-product residual division terms",
            )?;
            self.checked_div(&numerator, &denominator_coefficient, work)?
        };
        projection_work.charge(
            planned_coefficient_clone_census(self.coefficients.template(), self.base_count())?,
            self.limits,
            "aggregate explicit scalar-product accumulator terms",
        )?;
        let external_zero = self.coefficients.zero();
        let mut external_constant = external_zero;
        for ((left, right), numerator) in external_groups {
            let value =
                self.projected_rational(numerator, denominator.clone(), work, projection_work)?;
            let gram = self
                .external_gram
                .get(left)
                .and_then(|row| row.get(right))
                .ok_or(
                    SymbolicaAffineDenominatorError::InternalVerificationFailure {
                        detail: "explicit scalar-product Gram coordinate is out of range",
                    },
                )?;
            let contribution = self.projected_checked_mul(&value, gram, work, projection_work)?;
            external_constant = self.projected_checked_add(
                &external_constant,
                &contribution,
                work,
                projection_work,
            )?;
        }
        let lifted = self.lift_base_coefficient(&external_constant, projection_work)?;
        projection_work.charge(
            exact_operation_allocation_envelope(
                &residual,
                &lifted,
                BinaryOperation::Add,
                self.combined.parameter_names().len(),
            )?
            .census,
            self.limits,
            "aggregate explicit scalar-product lifted addition terms",
        )?;
        residual = self.checked_add(&residual, &lifted, work)?;
        Ok(residual)
    }
}
