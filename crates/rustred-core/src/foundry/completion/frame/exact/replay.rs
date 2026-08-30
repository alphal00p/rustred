use symbolica::domains::SelfRing;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::completion::stratum::TargetColumnPartition;
use crate::identity::IdentityConditionSource;

use super::super::PhysicalFramePlan;
use super::super::modular::ModularHit;
use super::reduce::{ReducedExactCircuit, check_limit, checked_add, try_vec};
use super::{
    ExactCircuitError, ExactCircuitGuard, ExactCircuitGuardOrigin, ExactCircuitLift,
    ExactCircuitLimits, ExactCircuitReplayWitness, ExactCircuitTerm, ExactTargetCircuit,
};

const REPLAY_ACCUMULATORS: &str = "exact-circuit replay accumulators";
const REPLAY_EXACT_OPERATIONS: &str = "exact-circuit replay exact operations";
const REPLAY_SOURCE_TERMS: &str = "exact-circuit replay source terms";
const CIRCUIT_TERMS: &str = "exact-circuit residual terms";
const DEPENDENCY_OWNER_WITNESSES: &str = "exact-circuit dependency owner witnesses";
const GUARDS: &str = "exact-circuit nonzero guards";
const GUARD_ORIGINS: &str = "exact-circuit guard origins";
const CONDITION_SOURCES: &str = "exact-circuit condition-source provenance";

pub(super) fn replay_exact_circuit<'frame>(
    context: &IndexedCoefficientContext,
    hit: &ModularHit<'frame>,
    partition: &TargetColumnPartition<'frame>,
    reduced: ReducedExactCircuit,
    limits: ExactCircuitLimits,
) -> Result<ExactCircuitLift, ExactCircuitError> {
    let plan = partition.frame();
    let mut budget = ReplayBudget {
        used: 0,
        limit: limits.max_replay_exact_operations,
    };
    let mut replayed: Vec<Option<IndexedCoefficient>> =
        try_vec(REPLAY_ACCUMULATORS, plan.columns().len())?;
    replayed.resize_with(plan.columns().len(), || None);
    let mut source_terms = 0usize;

    for contribution in &reduced.source_combination {
        let frame_row = contribution.frame_row_ordinal();
        let expected_instance = plan.source_instances().get(frame_row).ok_or(
            ExactCircuitError::SelectedSourceRowOutOfRange {
                row: frame_row,
                rows: plan.row_count(),
            },
        )?;
        if expected_instance != contribution.source_instance() {
            return Err(ExactCircuitError::Invariant {
                detail: "exact source contribution identity differs from frame chronology",
            });
        }
        let source = plan.source_for_row(frame_row).ok_or(
            ExactCircuitError::SelectedSourceRowOutOfRange {
                row: frame_row,
                rows: plan.row_count(),
            },
        )?;
        let structural_columns =
            plan.column_indices_for_row(frame_row)
                .ok_or(ExactCircuitError::Invariant {
                    detail: "exact replay source has invalid physical CSR bounds",
                })?;
        if structural_columns.len() != source.terms().len() {
            return Err(ExactCircuitError::Invariant {
                detail: "exact replay source terms disagree with physical CSR",
            });
        }
        for ((_, source_coefficient), &physical_column) in
            source.terms().iter().zip(structural_columns)
        {
            source_terms = checked_add(REPLAY_SOURCE_TERMS, source_terms, 1)?;
            check_limit(
                REPLAY_SOURCE_TERMS,
                source_terms,
                limits.max_replay_source_terms,
            )?;
            budget.charge()?;
            let multiplier = context.bind_sealed(contribution.coefficient())?;
            let source_coefficient = context.bind_sealed(source_coefficient)?;
            let product = context.mul_bound_with_limits(
                multiplier,
                source_coefficient,
                limits.indexed_algebra.exact_algebra,
            )?;
            let physical_column =
                usize::try_from(physical_column).map_err(|_| ExactCircuitError::Invariant {
                    detail: "exact replay physical column does not fit usize",
                })?;
            let slot = replayed
                .get_mut(physical_column)
                .ok_or(ExactCircuitError::Invariant {
                    detail: "exact replay physical column is outside its accumulator",
                })?;
            if let Some(accumulator) = slot.take() {
                budget.charge()?;
                let accumulator = context.bind_sealed(&accumulator)?;
                let product = context.bind_sealed(&product)?;
                let sum = context.add_bound_with_limits(
                    accumulator,
                    product,
                    limits.indexed_algebra.exact_algebra,
                )?;
                if !sum.is_zero() {
                    *slot = Some(sum);
                }
            } else if !product.is_zero() {
                *slot = Some(product);
            }
        }
    }

    let mut residual_terms = try_vec(CIRCUIT_TERMS, partition.allowed_columns().len())?;
    for (physical_column, coefficient) in replayed.into_iter().enumerate() {
        budget.charge()?;
        if physical_column == partition.target_column() {
            let Some(coefficient) = coefficient else {
                return Err(ExactCircuitError::ReplayMismatch {
                    physical_column,
                    detail: "normalized target coefficient vanished",
                });
            };
            if !coefficient.raw().is_one() {
                return Err(ExactCircuitError::ReplayMismatch {
                    physical_column,
                    detail: "target coefficient is not the exact unit",
                });
            }
            continue;
        }
        if partition
            .forbidden_columns()
            .binary_search(&physical_column)
            .is_ok()
        {
            if coefficient.is_some_and(|value| !value.is_zero()) {
                return Err(ExactCircuitError::ReplayMismatch {
                    physical_column,
                    detail: "forbidden coefficient did not cancel",
                });
            }
            continue;
        }
        let Some(descriptor) = partition.allowed_descriptor(physical_column) else {
            return Err(ExactCircuitError::ReplayMismatch {
                physical_column,
                detail: "column is absent from the exhaustive target partition",
            });
        };
        let Some(coefficient) = coefficient else {
            continue;
        };
        if coefficient.is_zero() {
            continue;
        }
        let shift = plan
            .columns()
            .get(physical_column)
            .ok_or(ExactCircuitError::Invariant {
                detail: "allowed replay column is outside the physical frame",
            })?
            .clone();
        let mut proper_subsector_owners = try_vec(
            DEPENDENCY_OWNER_WITNESSES,
            descriptor.proper_subsector_owners().len(),
        )?;
        proper_subsector_owners.extend_from_slice(descriptor.proper_subsector_owners());
        residual_terms.push(ExactCircuitTerm::new(
            physical_column,
            shift,
            coefficient,
            descriptor.descent().clone(),
            proper_subsector_owners,
        ));
        check_limit(
            CIRCUIT_TERMS,
            residual_terms.len(),
            limits.max_circuit_terms,
        )?;
    }

    let guards = collect_guards(context, plan, &reduced, &residual_terms, limits)?;
    let replay = ExactCircuitReplayWitness::new(
        reduced.source_combination.len(),
        source_terms,
        plan.columns().len(),
        budget.used,
    );
    let target_shift = plan
        .columns()
        .get(partition.target_column())
        .ok_or(ExactCircuitError::Invariant {
            detail: "exact circuit target is outside the physical frame",
        })?
        .clone();
    Ok(ExactCircuitLift::Replayed(ExactTargetCircuit::new(
        hit.sample_fingerprint().clone(),
        partition.stratum_id().clone(),
        partition.snapshot_id().clone(),
        hit.diagnostics().clone(),
        partition.target_column(),
        target_shift,
        residual_terms,
        reduced.source_combination,
        reduced.pivot_guards,
        guards,
        replay,
    )))
}

fn collect_guards(
    context: &IndexedCoefficientContext,
    plan: &PhysicalFramePlan,
    reduced: &ReducedExactCircuit,
    residual_terms: &[ExactCircuitTerm],
    limits: ExactCircuitLimits,
) -> Result<Vec<ExactCircuitGuard>, ExactCircuitError> {
    let mut participating = try_vec(
        "exact-circuit participating frame rows",
        checked_add(
            "exact-circuit participating frame rows",
            reduced.source_combination.len(),
            reduced.pivot_guards.len(),
        )?,
    )?;
    participating.extend(
        reduced
            .source_combination
            .iter()
            .map(|contribution| contribution.frame_row_ordinal()),
    );
    participating.extend(
        reduced
            .pivot_guards
            .iter()
            .map(|guard| guard.frame_row_ordinal()),
    );
    participating.sort_unstable();
    participating.dedup();

    let mut collector = GuardCollector::new(context, limits)?;
    for frame_row in participating {
        let source = plan.source_for_row(frame_row).ok_or(
            ExactCircuitError::SelectedSourceRowOutOfRange {
                row: frame_row,
                rows: plan.row_count(),
            },
        )?;
        let source_instance = plan.source_instances()[frame_row].clone();
        for (condition_ordinal, condition) in source.nonzero_conditions().iter().enumerate() {
            let mut condition_sources = try_vec(CONDITION_SOURCES, condition.sources().len())?;
            for condition_source in condition.sources() {
                condition_sources.push(clone_condition_source(condition_source)?);
            }
            collector.insert(
                condition.polynomial().clone(),
                ExactCircuitGuardOrigin::SourceCondition {
                    frame_row_ordinal: frame_row,
                    source_instance: source_instance.clone(),
                    condition_ordinal,
                    condition_sources: condition_sources.into_boxed_slice(),
                },
            )?;
        }
        let structural_columns =
            plan.column_indices_for_row(frame_row)
                .ok_or(ExactCircuitError::Invariant {
                    detail: "guard source has invalid physical CSR bounds",
                })?;
        if structural_columns.len() != source.terms().len() {
            return Err(ExactCircuitError::Invariant {
                detail: "guard source terms disagree with physical CSR",
            });
        }
        for ((_, coefficient), &physical_column) in source.terms().iter().zip(structural_columns) {
            let coefficient = context.bind_sealed(coefficient)?;
            collector.insert(
                context.denominator_condition_from_bound(coefficient)?,
                ExactCircuitGuardOrigin::SourceCoefficientDenominator {
                    frame_row_ordinal: frame_row,
                    source_instance: source_instance.clone(),
                    physical_column: physical_column as usize,
                },
            )?;
        }
    }

    for pivot in &reduced.pivot_guards {
        collector.insert(
            pivot.nonzero_polynomial().clone(),
            ExactCircuitGuardOrigin::ReducerPivotNumerator {
                frame_row_ordinal: pivot.frame_row_ordinal(),
                source_instance: pivot.source_instance().clone(),
                physical_pivot_column: pivot.physical_pivot_column(),
            },
        )?;
        let coefficient = context.bind_sealed(pivot.coefficient())?;
        collector.insert(
            context.denominator_condition_from_bound(coefficient)?,
            ExactCircuitGuardOrigin::ReducerPivotDenominator {
                frame_row_ordinal: pivot.frame_row_ordinal(),
                source_instance: pivot.source_instance().clone(),
                physical_pivot_column: pivot.physical_pivot_column(),
            },
        )?;
    }
    for contribution in &reduced.source_combination {
        let coefficient = context.bind_sealed(contribution.coefficient())?;
        collector.insert(
            context.denominator_condition_from_bound(coefficient)?,
            ExactCircuitGuardOrigin::SourceMultiplierDenominator {
                frame_row_ordinal: contribution.frame_row_ordinal(),
                source_instance: contribution.source_instance().clone(),
            },
        )?;
    }
    for term in residual_terms {
        let coefficient = context.bind_sealed(term.coefficient())?;
        collector.insert(
            context.denominator_condition_from_bound(coefficient)?,
            ExactCircuitGuardOrigin::ResidualCoefficientDenominator {
                physical_column: term.physical_column(),
            },
        )?;
    }
    collector.finish()
}

struct GuardCollector<'context> {
    context: &'context IndexedCoefficientContext,
    limits: ExactCircuitLimits,
    guards: Vec<(IndexedPolynomial, Vec<ExactCircuitGuardOrigin>)>,
    origins: usize,
}

impl<'context> GuardCollector<'context> {
    fn new(
        context: &'context IndexedCoefficientContext,
        limits: ExactCircuitLimits,
    ) -> Result<Self, ExactCircuitError> {
        Ok(Self {
            context,
            limits,
            guards: try_vec(GUARDS, 0)?,
            origins: 0,
        })
    }

    fn insert(
        &mut self,
        polynomial: IndexedPolynomial,
        origin: ExactCircuitGuardOrigin,
    ) -> Result<(), ExactCircuitError> {
        self.context.validate_polynomial_with_limits(
            &polynomial,
            self.limits.indexed_algebra.exact_algebra,
        )?;
        if polynomial.is_zero() {
            return Err(ExactCircuitError::Invariant {
                detail: "a required exact-circuit nonzero guard is identically zero",
            });
        }
        if polynomial.is_nonzero_constant() {
            return Ok(());
        }
        if let Some((_, origins)) = self
            .guards
            .iter_mut()
            .find(|(existing, _)| existing == &polynomial)
        {
            if origins.contains(&origin) {
                return Ok(());
            }
            self.origins = checked_add(GUARD_ORIGINS, self.origins, 1)?;
            check_limit(GUARD_ORIGINS, self.origins, self.limits.max_guard_origins)?;
            origins
                .try_reserve_exact(1)
                .map_err(|_| ExactCircuitError::AllocationFailure {
                    resource: GUARD_ORIGINS,
                    requested: origins.len().saturating_add(1),
                })?;
            origins.push(origin);
            return Ok(());
        }
        let requested = checked_add(GUARDS, self.guards.len(), 1)?;
        check_limit(GUARDS, requested, self.limits.max_guards)?;
        self.origins = checked_add(GUARD_ORIGINS, self.origins, 1)?;
        check_limit(GUARD_ORIGINS, self.origins, self.limits.max_guard_origins)?;
        self.guards
            .try_reserve_exact(1)
            .map_err(|_| ExactCircuitError::AllocationFailure {
                resource: GUARDS,
                requested,
            })?;
        let mut origins = try_vec(GUARD_ORIGINS, 1)?;
        origins.push(origin);
        self.guards.push((polynomial, origins));
        Ok(())
    }

    fn finish(self) -> Result<Vec<ExactCircuitGuard>, ExactCircuitError> {
        let mut finished = try_vec(GUARDS, self.guards.len())?;
        for (polynomial, origins) in self.guards {
            finished.push(ExactCircuitGuard::new(polynomial, origins));
        }
        Ok(finished)
    }
}

struct ReplayBudget {
    used: usize,
    limit: usize,
}

impl ReplayBudget {
    fn charge(&mut self) -> Result<(), ExactCircuitError> {
        self.used = checked_add(REPLAY_EXACT_OPERATIONS, self.used, 1)?;
        check_limit(REPLAY_EXACT_OPERATIONS, self.used, self.limit)
    }
}

fn clone_condition_source(
    source: &IdentityConditionSource,
) -> Result<IdentityConditionSource, ExactCircuitError> {
    Ok(match source {
        IdentityConditionSource::FamilyInputCoefficientDenominator { location } => {
            IdentityConditionSource::FamilyInputCoefficientDenominator {
                location: location.clone(),
            }
        }
        IdentityConditionSource::FamilyBasisDeterminantNumerator => {
            IdentityConditionSource::FamilyBasisDeterminantNumerator
        }
        IdentityConditionSource::RelationConditionAttached { row } => {
            IdentityConditionSource::RelationConditionAttached { row: row.clone() }
        }
        IdentityConditionSource::RelationInputTermDenominator { row, shift } => {
            IdentityConditionSource::RelationInputTermDenominator {
                row: row.clone(),
                shift: try_boxed_i64(shift, CONDITION_SOURCES)?,
            }
        }
        IdentityConditionSource::RelationCollectedTermDenominator { row, shift } => {
            IdentityConditionSource::RelationCollectedTermDenominator {
                row: row.clone(),
                shift: try_boxed_i64(shift, CONDITION_SOURCES)?,
            }
        }
        IdentityConditionSource::RelationScaleFactorDenominator {
            target_row,
            source_row,
        } => IdentityConditionSource::RelationScaleFactorDenominator {
            target_row: target_row.clone(),
            source_row: source_row.clone(),
        },
        IdentityConditionSource::RelationTranslation {
            source_row,
            target_row,
            offset,
        } => IdentityConditionSource::RelationTranslation {
            source_row: source_row.clone(),
            target_row: target_row.clone(),
            offset: try_boxed_i64(offset, CONDITION_SOURCES)?,
        },
        IdentityConditionSource::IndexTranslation { offset } => {
            IdentityConditionSource::IndexTranslation {
                offset: try_boxed_i64(offset, CONDITION_SOURCES)?,
            }
        }
    })
}

fn try_boxed_i64(values: &[i64], resource: &'static str) -> Result<Box<[i64]>, ExactCircuitError> {
    let mut retained = try_vec(resource, values.len())?;
    retained.extend_from_slice(values);
    Ok(retained.into_boxed_slice())
}
