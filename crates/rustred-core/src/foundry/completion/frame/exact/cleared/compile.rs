//! Fraction-free source reconstruction and semantic guard separation.

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::completion::frame::{PhysicalFramePlan, SourceInstanceId};

use super::super::{ExactCircuitGuardOrigin, ExactTargetCircuit};
use super::budget::{
    CONDITION_SOURCES, GUARD_ORIGINS, GUARDS, PHYSICAL_COLUMNS, PolynomialBudget,
    SOURCE_CONTRIBUTIONS, SOURCE_TERMS, check_limit, checked_add, try_vec,
};
use super::model::{
    ClearedCircuitError, ClearedCircuitLimits, ClearedExactCircuit, ClearedGuardTelemetry,
    ClearedPhysicalTerm, ClearedSemanticGuard, ClearedSemanticGuardOrigin, ClearedSourceCofactor,
};

struct PendingSource<'source> {
    frame_row_ordinal: usize,
    source_instance: SourceInstanceId,
    source: &'source crate::identity::TranslatedSource,
    row_denominator: IndexedPolynomial,
    scaled_multiplier: IndexedCoefficient,
}

/// Reconstruct and replay one canonical fraction-free source consequence.
///
/// This function is intentionally available only in test builds. Successful
/// reconstruction proves the displayed polynomial identity and its guard
/// provenance, but never completion or applicability on an integer fibre.
pub(crate) fn try_clear_exact_circuit(
    context: &IndexedCoefficientContext,
    plan: &PhysicalFramePlan,
    circuit: &ExactTargetCircuit,
    limits: ClearedCircuitLimits,
) -> Result<ClearedExactCircuit, ClearedCircuitError> {
    validate_binding(context, plan, circuit, limits)?;
    let mut budget = PolynomialBudget::new(context, limits);
    let one = budget.one_polynomial()?;
    let mut pending = try_vec(SOURCE_CONTRIBUTIONS, circuit.source_combination().len())?;
    let mut common_cofactor_denominator = one.clone();
    let mut source_terms = 0usize;

    for contribution in circuit.source_combination() {
        let frame_row = contribution.frame_row_ordinal();
        let source =
            plan.source_for_row(frame_row)
                .ok_or(ClearedCircuitError::InvalidCircuitBinding {
                    detail: "a source contribution is outside the physical frame",
                })?;
        let structural = plan.column_indices_for_row(frame_row).ok_or(
            ClearedCircuitError::InvalidCircuitBinding {
                detail: "a source contribution has invalid physical CSR bounds",
            },
        )?;
        if structural.len() != source.terms().len() {
            return Err(ClearedCircuitError::InvalidCircuitBinding {
                detail: "a source contribution disagrees with physical CSR",
            });
        }
        source_terms = checked_add(SOURCE_TERMS, source_terms, source.terms().len())?;
        check_limit(SOURCE_TERMS, source_terms, limits.max_source_terms)?;

        let mut row_denominator = one.clone();
        for coefficient in source.terms().values() {
            let denominator =
                context.denominator_condition_with_limits(coefficient, limits.exact_algebra)?;
            row_denominator = budget.polynomial_lcm(&row_denominator, &denominator)?;
        }
        let row_denominator_coefficient = budget.as_coefficient(&row_denominator)?;
        let scaled_multiplier =
            budget.div(contribution.coefficient(), &row_denominator_coefficient)?;
        let scaled_denominator =
            context.denominator_condition_with_limits(&scaled_multiplier, limits.exact_algebra)?;
        common_cofactor_denominator =
            budget.polynomial_lcm(&common_cofactor_denominator, &scaled_denominator)?;
        pending.push(PendingSource {
            frame_row_ordinal: frame_row,
            source_instance: contribution.source_instance().clone(),
            source,
            row_denominator,
            scaled_multiplier,
        });
    }

    let common_cofactor_denominator_coefficient =
        budget.as_coefficient(&common_cofactor_denominator)?;
    let mut cofactors = try_vec(SOURCE_CONTRIBUTIONS, pending.len())?;
    for source in &pending {
        let cofactor = budget.mul(
            &common_cofactor_denominator_coefficient,
            &source.scaled_multiplier,
        )?;
        cofactors.push(budget.require_polynomial(&cofactor)?);
    }

    // Remove only content common to the complete ordinary-source cofactor
    // vector and target coefficient. Dividing output columns alone would be
    // a saturation claim and would lose the explicit source proof.
    let mut common_content = common_cofactor_denominator.clone();
    for cofactor in &cofactors {
        common_content = budget.polynomial_gcd(&common_content, cofactor)?;
        if common_content.raw().is_one() {
            break;
        }
    }
    let target_coefficient =
        budget.exact_polynomial_division(&common_cofactor_denominator, &common_content)?;
    if target_coefficient.is_zero() {
        return Err(ClearedCircuitError::ZeroFinalTargetCoefficient);
    }
    for cofactor in &mut cofactors {
        *cofactor = budget.exact_polynomial_division(cofactor, &common_content)?;
    }

    let mut accumulators: Vec<Option<IndexedCoefficient>> =
        try_vec(PHYSICAL_COLUMNS, plan.columns().len())?;
    accumulators.resize_with(plan.columns().len(), || None);
    let mut source_cofactors = try_vec(SOURCE_CONTRIBUTIONS, pending.len())?;
    for (source, cofactor) in pending.iter().zip(&cofactors) {
        let row_denominator = budget.as_coefficient(&source.row_denominator)?;
        let cofactor_value = budget.as_coefficient(cofactor)?;
        let structural = plan
            .column_indices_for_row(source.frame_row_ordinal)
            .ok_or(ClearedCircuitError::InvalidCircuitBinding {
                detail: "a cleared source has invalid physical CSR bounds",
            })?;
        for ((_, coefficient), &physical_column) in source.source.terms().iter().zip(structural) {
            let row_polynomial = budget.mul(&row_denominator, coefficient)?;
            budget.require_polynomial(&row_polynomial)?;
            let contribution = budget.mul(&cofactor_value, &row_polynomial)?;
            budget.require_polynomial(&contribution)?;
            let physical_column = usize::try_from(physical_column).map_err(|_| {
                ClearedCircuitError::InvalidCircuitBinding {
                    detail: "a physical column does not fit usize",
                }
            })?;
            let slot = accumulators.get_mut(physical_column).ok_or(
                ClearedCircuitError::InvalidCircuitBinding {
                    detail: "a cleared source column is outside the physical frame",
                },
            )?;
            if let Some(accumulator) = slot.take() {
                let sum = budget.add(&accumulator, &contribution)?;
                if !sum.is_zero() {
                    *slot = Some(sum);
                }
            } else if !contribution.is_zero() {
                *slot = Some(contribution);
            }
        }
        source_cofactors.push(ClearedSourceCofactor {
            frame_row_ordinal: source.frame_row_ordinal,
            source_instance: source.source_instance.clone(),
            row_denominator: source.row_denominator.clone(),
            cofactor: cofactor.clone(),
        });
    }

    let target_value = budget.as_coefficient(&target_coefficient)?;
    let mut physical_terms = try_vec(PHYSICAL_COLUMNS, plan.columns().len())?;
    for (physical_column, coefficient) in accumulators.into_iter().enumerate() {
        let expected = if physical_column == circuit.target_column() {
            Some(target_value.clone())
        } else if let Some(residual) = circuit
            .residual_terms()
            .iter()
            .find(|term| term.physical_column() == physical_column)
        {
            Some(budget.mul(&target_value, residual.coefficient())?)
        } else {
            None
        };
        match (coefficient, expected) {
            (None, None) => {}
            (Some(actual), Some(expected)) => {
                if actual != expected {
                    return Err(ClearedCircuitError::ReplayMismatch {
                        physical_column,
                        detail: "polynomial coefficient differs from the normalized exact circuit",
                    });
                }
                let polynomial = budget.require_polynomial(&actual)?;
                physical_terms.push(ClearedPhysicalTerm {
                    physical_column,
                    coefficient: polynomial,
                });
            }
            (None, Some(expected)) if expected.is_zero() => {}
            (None, Some(_)) => {
                return Err(ClearedCircuitError::ReplayMismatch {
                    physical_column,
                    detail: "expected polynomial coefficient vanished",
                });
            }
            (Some(actual), None) if actual.is_zero() => {}
            (Some(_), None) => {
                return Err(ClearedCircuitError::ReplayMismatch {
                    physical_column,
                    detail: "unexpected polynomial coefficient survived",
                });
            }
        }
    }

    let before = classify_existing_guards(circuit);
    let mut guards = SemanticGuardCollector::new(context, limits, &mut budget);
    let mut condition_source_entries = 0usize;
    for source in &pending {
        for (condition_ordinal, condition) in source.source.nonzero_conditions().iter().enumerate()
        {
            condition_source_entries = checked_add(
                CONDITION_SOURCES,
                condition_source_entries,
                condition.sources().len(),
            )?;
            check_limit(
                CONDITION_SOURCES,
                condition_source_entries,
                limits.max_condition_source_entries,
            )?;
            let mut condition_sources = try_vec(CONDITION_SOURCES, condition.sources().len())?;
            condition_sources.extend(condition.sources().iter().cloned());
            guards.insert(
                condition.polynomial().clone(),
                ClearedSemanticGuardOrigin::SourceOrFamily(
                    ExactCircuitGuardOrigin::SourceCondition {
                        frame_row_ordinal: source.frame_row_ordinal,
                        source_instance: source.source_instance.clone(),
                        condition_ordinal,
                        condition_sources: condition_sources.into_boxed_slice(),
                    },
                ),
            )?;
        }
        let structural = plan
            .column_indices_for_row(source.frame_row_ordinal)
            .ok_or(ClearedCircuitError::InvalidCircuitBinding {
                detail: "a semantic source has invalid physical CSR bounds",
            })?;
        for ((_, coefficient), &physical_column) in source.source.terms().iter().zip(structural) {
            let denominator =
                context.denominator_condition_with_limits(coefficient, limits.exact_algebra)?;
            guards.insert(
                denominator,
                ClearedSemanticGuardOrigin::SourceOrFamily(
                    ExactCircuitGuardOrigin::SourceCoefficientDenominator {
                        frame_row_ordinal: source.frame_row_ordinal,
                        source_instance: source.source_instance.clone(),
                        physical_column: physical_column as usize,
                    },
                ),
            )?;
        }
    }
    let final_target_guard_retained = !target_coefficient.is_nonzero_constant();
    guards.insert(
        target_coefficient.clone(),
        ClearedSemanticGuardOrigin::FinalTargetCoefficient,
    )?;
    let semantic_guards = guards.finish();
    let after_source_or_family = semantic_guards
        .iter()
        .filter(|guard| {
            guard
                .origins
                .iter()
                .any(|origin| matches!(origin, ClearedSemanticGuardOrigin::SourceOrFamily(_)))
        })
        .count();
    let guard_telemetry = ClearedGuardTelemetry {
        before_unique: circuit.nonzero_guards().len(),
        before_source_or_family_only: before.0,
        before_intermediate_only: before.1,
        before_mixed: before.2,
        after_unique: semantic_guards.len(),
        after_source_or_family,
        final_target_guard_retained,
    };

    Ok(ClearedExactCircuit {
        target_column: circuit.target_column(),
        target_coefficient,
        source_cofactors: source_cofactors.into_boxed_slice(),
        physical_terms: physical_terms.into_boxed_slice(),
        semantic_guards: semantic_guards.into_boxed_slice(),
        guard_telemetry,
        exact_operations: budget.operations,
        gcd_term_pairs: budget.gcd_term_pairs,
        retained_polynomial_terms: budget.retained_terms,
    })
}

/// Compile only the mandatory final-target guard for adversarial tests.
pub(crate) fn try_compile_final_target_guard(
    context: &IndexedCoefficientContext,
    target_coefficient: &IndexedPolynomial,
    limits: ClearedCircuitLimits,
) -> Result<Vec<ClearedSemanticGuard>, ClearedCircuitError> {
    context.validate_polynomial_with_limits(target_coefficient, limits.exact_algebra)?;
    if target_coefficient.is_zero() {
        return Err(ClearedCircuitError::ZeroFinalTargetCoefficient);
    }
    let mut budget = PolynomialBudget::new(context, limits);
    let mut collector = SemanticGuardCollector::new(context, limits, &mut budget);
    collector.insert(
        target_coefficient.clone(),
        ClearedSemanticGuardOrigin::FinalTargetCoefficient,
    )?;
    Ok(collector.finish())
}

fn validate_binding(
    context: &IndexedCoefficientContext,
    plan: &PhysicalFramePlan,
    circuit: &ExactTargetCircuit,
    limits: ClearedCircuitLimits,
) -> Result<(), ClearedCircuitError> {
    if context.fingerprint() != plan.context_fingerprint() {
        return Err(ClearedCircuitError::WrongContext);
    }
    check_limit(
        SOURCE_CONTRIBUTIONS,
        circuit.source_combination().len(),
        limits.max_source_contributions,
    )?;
    check_limit(
        PHYSICAL_COLUMNS,
        plan.columns().len(),
        limits.max_physical_columns,
    )?;
    let target = plan.columns().get(circuit.target_column()).ok_or(
        ClearedCircuitError::InvalidCircuitBinding {
            detail: "the exact target is outside the physical frame",
        },
    )?;
    if target != circuit.target_shift() {
        return Err(ClearedCircuitError::InvalidCircuitBinding {
            detail: "the exact target shift differs from its physical column",
        });
    }
    if circuit
        .source_combination()
        .windows(2)
        .any(|pair| pair[0].frame_row_ordinal() >= pair[1].frame_row_ordinal())
    {
        return Err(ClearedCircuitError::InvalidCircuitBinding {
            detail: "source contributions are not in strict frame chronology",
        });
    }
    for contribution in circuit.source_combination() {
        let expected = plan
            .source_instances()
            .get(contribution.frame_row_ordinal())
            .ok_or(ClearedCircuitError::InvalidCircuitBinding {
                detail: "a source contribution is outside the physical frame",
            })?;
        if expected != contribution.source_instance() {
            return Err(ClearedCircuitError::InvalidCircuitBinding {
                detail: "source contribution provenance differs from frame chronology",
            });
        }
        context.validate_with_limits(contribution.coefficient(), limits.exact_algebra)?;
        if contribution.coefficient().is_zero() {
            return Err(ClearedCircuitError::InvalidCircuitBinding {
                detail: "the exact source combination contains a zero contribution",
            });
        }
    }
    Ok(())
}

fn classify_existing_guards(circuit: &ExactTargetCircuit) -> (usize, usize, usize) {
    let mut source_only = 0usize;
    let mut intermediate_only = 0usize;
    let mut mixed = 0usize;
    for guard in circuit.nonzero_guards() {
        let has_source = guard.origins().iter().any(is_source_or_family_origin);
        let has_intermediate = guard
            .origins()
            .iter()
            .any(|origin| !is_source_or_family_origin(origin));
        match (has_source, has_intermediate) {
            (true, false) => source_only += 1,
            (false, true) => intermediate_only += 1,
            (true, true) => mixed += 1,
            (false, false) => {}
        }
    }
    (source_only, intermediate_only, mixed)
}

fn is_source_or_family_origin(origin: &ExactCircuitGuardOrigin) -> bool {
    matches!(
        origin,
        ExactCircuitGuardOrigin::SourceCondition { .. }
            | ExactCircuitGuardOrigin::SourceCoefficientDenominator { .. }
    )
}

struct SemanticGuardCollector<'context, 'budget> {
    context: &'context IndexedCoefficientContext,
    limits: ClearedCircuitLimits,
    budget: &'budget mut PolynomialBudget<'context>,
    guards: Vec<(IndexedPolynomial, Vec<ClearedSemanticGuardOrigin>)>,
    origins: usize,
}

impl<'context, 'budget> SemanticGuardCollector<'context, 'budget> {
    fn new(
        context: &'context IndexedCoefficientContext,
        limits: ClearedCircuitLimits,
        budget: &'budget mut PolynomialBudget<'context>,
    ) -> Self {
        Self {
            context,
            limits,
            budget,
            guards: Vec::new(),
            origins: 0,
        }
    }

    fn insert(
        &mut self,
        polynomial: IndexedPolynomial,
        origin: ClearedSemanticGuardOrigin,
    ) -> Result<(), ClearedCircuitError> {
        self.context
            .validate_polynomial_with_limits(&polynomial, self.limits.exact_algebra)?;
        if polynomial.is_zero() {
            return Err(ClearedCircuitError::ZeroFinalTargetCoefficient);
        }
        if polynomial.is_nonzero_constant() {
            return Ok(());
        }
        self.budget.charge_operation()?;
        let polynomial = self.context.primitive_guard_associate_with_limits(
            &polynomial,
            self.limits.exact_algebra,
            self.limits.max_guard_serialization_bytes,
        )?;
        self.budget.retain(&polynomial)?;
        if let Some((_, origins)) = self
            .guards
            .iter_mut()
            .find(|(existing, _)| existing == &polynomial)
        {
            if !origins.contains(&origin) {
                let requested = checked_add(GUARD_ORIGINS, self.origins, 1)?;
                check_limit(GUARD_ORIGINS, requested, self.limits.max_guard_origins)?;
                origins.try_reserve_exact(1).map_err(|_| {
                    ClearedCircuitError::AllocationFailure {
                        resource: GUARD_ORIGINS,
                        requested: origins.len().saturating_add(1),
                    }
                })?;
                origins.push(origin);
                self.origins = requested;
            }
            return Ok(());
        }
        let requested = checked_add(GUARDS, self.guards.len(), 1)?;
        check_limit(GUARDS, requested, self.limits.max_guards)?;
        let origins = checked_add(GUARD_ORIGINS, self.origins, 1)?;
        check_limit(GUARD_ORIGINS, origins, self.limits.max_guard_origins)?;
        self.guards
            .try_reserve_exact(1)
            .map_err(|_| ClearedCircuitError::AllocationFailure {
                resource: GUARDS,
                requested,
            })?;
        self.guards.push((polynomial, vec![origin]));
        self.origins = origins;
        Ok(())
    }

    fn finish(self) -> Vec<ClearedSemanticGuard> {
        self.guards
            .into_iter()
            .map(|(polynomial, origins)| ClearedSemanticGuard {
                polynomial,
                origins: origins.into_boxed_slice(),
            })
            .collect()
    }
}
