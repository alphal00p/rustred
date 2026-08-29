use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::coefficient::SerializedRational;
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::{AtomCore, AtomView, CoefficientView, Q, Z};

use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::budget::{
    coefficient_census, compiled_retained_byte_bound, exact_operation_allocation_envelope,
    multiply_census, planned_coefficient_clone_census, planned_operation_polynomial_census,
    planned_polynomial_clone_census, polynomial_census, retained_variable_map_arc_bytes,
    signed_i64_magnitude_bits, verify_operation_result_envelope,
};
use super::construction::{
    check_limit, checked_atom_shape, maximum_combined_symbol_bytes, upper_triangular_count,
};
use super::error::SymbolicaAffineDenominatorError;
use super::evaluate::CheckedEvaluator;
use super::model::{CompiledSymbolicaAffineDenominator, SymbolicaAffineDenominatorCompiler};
use super::normalize::{
    charge_dense_degree_box, normalized_expression_census, normalized_expression_render_byte_bound,
};
use super::projection::{
    ProjectionGroup, classify_numerator_term, coefficient_contains_momentum,
    lift_polynomial_prefix, momentum_degree, polynomial_contains_momentum,
    project_polynomial_prefix, reject_momentum_denominator,
};
use super::work::{
    BinaryOperation, CoefficientCensus, ExactOperationAllocationEnvelope, ExactWorkBudget,
    ProjectionAllocationBudget,
};

impl SymbolicaAffineDenominatorCompiler {
    /// Compile an already parsed Atom on the authenticated combined map.
    pub fn compile(
        &self,
        source: AtomView<'_>,
    ) -> Result<CompiledSymbolicaAffineDenominator, SymbolicaAffineDenominatorError> {
        catch_unwind(AssertUnwindSafe(|| self.compile_inner(source))).map_err(|_| {
            SymbolicaAffineDenominatorError::SymbolicaPanic {
                stage: "checked expression evaluation",
            }
        })?
    }

    pub(super) fn compile_inner(
        &self,
        source: AtomView<'_>,
    ) -> Result<CompiledSymbolicaAffineDenominator, SymbolicaAffineDenominatorError> {
        let input_bytes = source.get_byte_size();
        check_limit(
            "input expression bytes",
            input_bytes,
            self.limits.max_input_bytes,
        )?;
        checked_atom_shape(source, self.limits)?;
        let fixed_retained_bytes = compiled_retained_byte_bound(input_bytes, 0, 0, 0)?;
        check_limit(
            "compiled fixed retained bytes",
            fixed_retained_bytes,
            self.limits.max_compiled_retained_bytes,
        )?;

        let mut evaluator = CheckedEvaluator::new(self);
        let evaluated = evaluator.evaluate(source, true)?;
        self.combined
            .validate_with_limits(&evaluated, self.limits.exact_algebra)?;
        self.validate_retained_shape(&evaluated)?;
        reject_momentum_denominator(&evaluated, self.base_count())?;

        // Bound the Atom that rational-polynomial conversion will construct
        // before asking Symbolica to allocate it.
        let normalized_census = normalized_expression_census(&evaluated)?;
        check_limit(
            "normalized expression nodes",
            normalized_census.nodes,
            self.limits.max_normalized_expression_nodes,
        )?;
        check_limit(
            "normalized expression integer bits",
            normalized_census.integer_bits,
            self.limits.max_normalized_expression_integer_bits,
        )?;
        let maximum_symbol_bytes = maximum_combined_symbol_bytes(&self.combined)?;
        let normalized_render_byte_bound =
            normalized_expression_render_byte_bound(normalized_census, maximum_symbol_bytes)?;
        if normalized_render_byte_bound > self.limits.max_normalized_expression_bytes {
            return Err(
                SymbolicaAffineDenominatorError::NormalizedExpressionTooLarge {
                    requested: normalized_render_byte_bound,
                    limit: self.limits.max_normalized_expression_bytes,
                },
            );
        }
        let normalized_expression = evaluated.to_expression();
        let normalized_expression_bytes = normalized_expression.as_view().get_byte_size();
        if normalized_expression_bytes > self.limits.max_normalized_expression_bytes {
            return Err(
                SymbolicaAffineDenominatorError::NormalizedExpressionTooLarge {
                    requested: normalized_expression_bytes,
                    limit: self.limits.max_normalized_expression_bytes,
                },
            );
        }

        let (affine_denominator, projection_stats) = self.project_affine_denominator(
            &evaluated,
            &mut evaluator.work,
            &mut evaluator.projection_work,
        )?;
        let variable_map_arc_bytes = retained_variable_map_arc_bytes(
            std::iter::once(affine_denominator.constant())
                .chain(affine_denominator.coefficients().iter()),
        )?;
        let compiled_retained_bytes = compiled_retained_byte_bound(
            input_bytes,
            normalized_expression_bytes,
            projection_stats.projected_retained_bytes,
            variable_map_arc_bytes,
        )?;
        check_limit(
            "compiled retained bytes",
            compiled_retained_bytes,
            self.limits.max_compiled_retained_bytes,
        )?;
        Ok(CompiledSymbolicaAffineDenominator {
            source: source.to_owned(),
            normalized_expression,
            affine_denominator,
        })
    }

    /// Evaluate an Atom in the same checked parser, proving it is momentum free.
    pub fn parse_base_coefficient(
        &self,
        source: AtomView<'_>,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        catch_unwind(AssertUnwindSafe(|| {
            let input_bytes = source.get_byte_size();
            check_limit(
                "input expression bytes",
                input_bytes,
                self.limits.max_input_bytes,
            )?;
            checked_atom_shape(source, self.limits)?;
            let mut evaluator = CheckedEvaluator::new(self);
            let value = evaluator.evaluate(source, false)?;
            self.combined
                .validate_with_limits(&value, self.limits.exact_algebra)?;
            self.validate_retained_shape(&value)?;
            if coefficient_contains_momentum(&value, self.base_count())? {
                return Err(SymbolicaAffineDenominatorError::BaseCoefficientContainsMomentum);
            }
            self.project_complete_coefficient(
                &value,
                &mut evaluator.work,
                &mut evaluator.projection_work,
            )
        }))
        .map_err(|_| SymbolicaAffineDenominatorError::SymbolicaPanic {
            stage: "base-coefficient evaluation",
        })?
    }

    pub(super) fn base_count(&self) -> usize {
        self.coefficients.parameter_names().len()
    }

    pub(super) fn validate_retained_shape(
        &self,
        coefficient: &Coefficient,
    ) -> Result<CoefficientCensus, SymbolicaAffineDenominatorError> {
        let numerator_terms = coefficient.numerator.nterms();
        let denominator_terms = coefficient.denominator.nterms();
        check_limit(
            "combined numerator terms",
            numerator_terms,
            self.limits.max_combined_polynomial_terms,
        )?;
        check_limit(
            "combined denominator terms",
            denominator_terms,
            self.limits.max_combined_polynomial_terms,
        )?;
        let all_terms = numerator_terms.checked_add(denominator_terms).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "combined polynomial terms",
            },
        )?;
        let exponent_entries = all_terms
            .checked_mul(self.combined.parameter_names().len())
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "combined exponent entries",
            })?;
        check_limit(
            "combined exponent entries",
            exponent_entries,
            self.limits.max_combined_exponent_entries,
        )?;
        let census = coefficient_census(coefficient)?;
        check_limit(
            "combined coefficient integer bits",
            census.integer_bits,
            self.limits.max_coefficient_integer_bits,
        )?;
        check_limit(
            "combined retained bytes",
            census.retained_bytes,
            self.limits.max_combined_retained_bytes,
        )?;
        Ok(census)
    }

    pub(super) fn preflight_binary_shape(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        operation: BinaryOperation,
        work: &mut ExactWorkBudget,
    ) -> Result<ExactOperationAllocationEnvelope, SymbolicaAffineDenominatorError> {
        charge_dense_degree_box(
            left,
            right,
            operation,
            self.combined.parameter_names().len(),
            self.limits,
            work,
        )?;
        let allocation = exact_operation_allocation_envelope(
            left,
            right,
            operation,
            self.combined.parameter_names().len(),
        )?;
        check_limit(
            "combined exact-operation numerator term envelope",
            allocation.numerator_terms,
            self.limits.max_combined_polynomial_terms,
        )?;
        check_limit(
            "combined exact-operation denominator term envelope",
            allocation.denominator_terms,
            self.limits.max_combined_polynomial_terms,
        )?;
        check_limit(
            "combined exact-operation exponent-entry envelope",
            allocation.census.exponent_entries,
            self.limits.max_combined_exponent_entries,
        )?;
        check_limit(
            "combined exact-operation integer bits",
            allocation.census.integer_bits,
            self.limits.max_coefficient_integer_bits,
        )?;
        check_limit(
            "combined exact-operation retained bytes",
            allocation.census.retained_bytes,
            self.limits.max_combined_retained_bytes,
        )?;
        Ok(allocation)
    }

    pub(super) fn checked_add(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        work: &mut ExactWorkBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let allocation = self.preflight_binary_shape(left, right, BinaryOperation::Add, work)?;
        let result = self
            .combined
            .try_add(left, right, self.limits.exact_algebra)?;
        let actual = self.validate_retained_shape(&result)?;
        verify_operation_result_envelope(&result, actual, allocation)?;
        Ok(result)
    }

    pub(super) fn checked_mul(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        work: &mut ExactWorkBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let allocation =
            self.preflight_binary_shape(left, right, BinaryOperation::Multiply, work)?;
        let result = self
            .combined
            .try_mul(left, right, self.limits.exact_algebra)?;
        let actual = self.validate_retained_shape(&result)?;
        verify_operation_result_envelope(&result, actual, allocation)?;
        Ok(result)
    }

    pub(super) fn checked_div(
        &self,
        numerator: &Coefficient,
        denominator: &Coefficient,
        work: &mut ExactWorkBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let allocation =
            self.preflight_binary_shape(numerator, denominator, BinaryOperation::Divide, work)?;
        let result = self
            .combined
            .try_div(numerator, denominator, self.limits.exact_algebra)?;
        let actual = self.validate_retained_shape(&result)?;
        verify_operation_result_envelope(&result, actual, allocation)?;
        Ok(result)
    }

    pub(super) fn numeric_coefficient(
        &self,
        atom: AtomView<'_>,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let AtomView::Num(number) = atom else {
            return Err(SymbolicaAffineDenominatorError::UnsupportedNumericAtom(
                atom.to_owned(),
            ));
        };
        let (numerator_bits, denominator_bits) = match number.get_coeff_view() {
            CoefficientView::Natural(real_numerator, real_denominator, imaginary, _)
                if imaginary == 0 =>
            {
                (
                    signed_i64_magnitude_bits(real_numerator),
                    signed_i64_magnitude_bits(real_denominator),
                )
            }
            CoefficientView::Large(real, imaginary) if imaginary.is_zero() => match real {
                SerializedRational::Natural(numerator, denominator) => (
                    signed_i64_magnitude_bits(numerator),
                    signed_i64_magnitude_bits(denominator),
                ),
                // The packed large-rational fields are intentionally opaque.
                // Their complete serialized Atom size is a conservative bit
                // envelope and can be inspected without cloning GMP storage.
                SerializedRational::Large(_) => {
                    let bits = atom.get_byte_size().checked_mul(8).ok_or(
                        SymbolicaAffineDenominatorError::ResourceCountOverflow {
                            resource: "numeric Atom magnitude bits",
                        },
                    )?;
                    (bits, bits)
                }
            },
            _ => {
                return Err(SymbolicaAffineDenominatorError::UnsupportedNumericAtom(
                    atom.to_owned(),
                ));
            }
        };
        let mut planned = planned_operation_polynomial_census(
            1,
            self.combined.parameter_names().len(),
            numerator_bits,
        )?;
        planned.checked_add_assign(
            planned_operation_polynomial_census(
                1,
                self.combined.parameter_names().len(),
                denominator_bits,
            )?,
            "numeric Atom allocation envelope",
        )?;
        check_limit(
            "numeric Atom integer bits",
            planned.integer_bits,
            self.limits.max_coefficient_integer_bits,
        )?;
        check_limit(
            "numeric Atom retained bytes",
            planned.retained_bytes,
            self.limits.max_combined_retained_bytes,
        )?;
        let result = atom
            .try_to_rational_polynomial(&Q, &Z, Some(self.combined.variables().clone()))
            .map_err(|_| {
                SymbolicaAffineDenominatorError::UnsupportedNumericAtom(atom.to_owned())
            })?;
        self.combined
            .validate_with_limits(&result, self.limits.exact_algebra)?;
        self.validate_retained_shape(&result)?;
        Ok(result)
    }

    pub(super) fn validate_vector_linear(
        &self,
        coefficient: &Coefficient,
        argument: usize,
        atom: AtomView<'_>,
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

    pub(super) fn contract_explicit_scalar_product(
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

    pub(super) fn lift_base_coefficient(
        &self,
        coefficient: &Coefficient,
        projection_work: &mut ProjectionAllocationBudget,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let combined_variables = self.combined.parameter_names().len();
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.numerator, combined_variables)?,
            self.limits,
            "aggregate lifted numerator terms",
        )?;
        projection_work.charge(
            planned_polynomial_clone_census(&coefficient.denominator, combined_variables)?,
            self.limits,
            "aggregate lifted denominator terms",
        )?;
        let numerator = lift_polynomial_prefix(
            &coefficient.numerator,
            &self.combined.template().numerator,
            self.base_count(),
            self.limits.max_combined_exponent_entries,
        )?;
        let denominator = lift_polynomial_prefix(
            &coefficient.denominator,
            &self.combined.template().denominator,
            self.base_count(),
            self.limits.max_combined_exponent_entries,
        )?;
        let lifted = Coefficient::from_num_den(numerator, denominator, &Z, false);
        self.combined
            .validate_with_limits(&lifted, self.limits.exact_algebra)?;
        self.validate_retained_shape(&lifted)?;
        Ok(lifted)
    }
}
