//! Exact materialization of one post-hit relation constrained to use a later row.
//!
//! The ordinary exact lift orients the first row which pivots the target.
//! A post-hit search instead needs to ask whether a designated later source
//! can cancel every forbidden column with an already retained predecessor
//! set, while still producing a nonzero target coefficient.  This module
//! answers precisely that bounded question.  It does not search for rows and
//! it grants no authority before the common full-source replay succeeds.

use symbolica::domains::SelfRing;
use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::prelude::{IntegerRing, Z};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};
use crate::foundry::completion::stratum::TargetColumnPartition;

use super::super::PhysicalFramePlan;
use super::super::modular::ModularHit;
use super::model::{ExactCircuitPivotGuard, ExactFrameSourceContribution, ExactTargetCircuit};
use super::reduce::{
    ReducedExactCircuit, call_native, check_limit, checked_add, checked_u32,
    fixed_index_assignments, preflight, try_vec, validate_binding, validate_selected_rows,
};
use super::{ExactCircuitError, ExactCircuitLimits};

const ROOTED_SELECTED_ROWS: &str = "rooted exact-circuit selected source rows";
const ROOTED_AUGMENTED_COLUMNS: &str = "rooted exact-circuit augmented columns";
const ROOTED_PROJECTED_INPUT: &str = "rooted exact-circuit projected input entries";
const ROOTED_NATIVE_DECOMPOSITION: &str = "rooted exact-circuit native U/L entries";
const ROOTED_DEPENDENCIES: &str = "rooted exact-circuit pivot dependencies";
const ROOTED_SOURCE_COMBINATION: &str = "rooted exact-circuit source combination";
const ROOTED_TARGET_OPERATIONS: &str = "rooted exact-circuit target operations";

type NativeField = RationalPolynomialField<IntegerRing, u16>;

/// A checked bounded miss while forcing one later exact source row.
///
/// Neither variant says that a relation is absent from a larger translated
/// source set.  In particular, a zero target coefficient only rejects this
/// exact root-constrained combination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RootedExactCircuitMiss {
    ForbiddenColumnsDidNotCancel {
        root_frame_row: usize,
        first_uncancelled_physical_column: usize,
    },
    TargetCoefficientVanished {
        root_frame_row: usize,
    },
}

/// Exact result for one root-constrained post-hit candidate.
#[derive(Debug)]
pub(crate) enum RootedExactCircuitLift {
    Replayed(ExactTargetCircuit),
    Inconclusive(RootedExactCircuitMiss),
}

#[derive(Debug)]
struct RootedForwardRowMeta {
    frame_row: usize,
    reducer_row: usize,
    pivot_projected_column: usize,
    pivot_coefficient: IndexedCoefficient,
    pivot_dependencies: Vec<usize>,
}

enum RootedReduction {
    Normalized(ReducedExactCircuit),
    Inconclusive(RootedExactCircuitMiss),
}

/// Force `root_frame_row` to participate in an exact relation over
/// `predecessor_rows` and replay the resulting normalized circuit.
///
/// Predecessors must be strictly increasing frame ordinals and precede the
/// root in physical chronology.  Symbolica eliminates only the forbidden
/// block. The designated root provenance column is placed immediately
/// after that block and before every predecessor provenance column.  Hence a
/// root pivot on that column proves that all forbidden coefficients canceled,
/// keeps its coefficient exactly one, and retains the predecessor
/// multipliers to its right.  The target column is deliberately absent from
/// this reduction and is evaluated separately before normalization.
pub(crate) fn try_lift_rooted_exact_circuit<'frame>(
    context: &IndexedCoefficientContext,
    hit: &ModularHit<'frame>,
    partition: &TargetColumnPartition<'frame>,
    predecessor_rows: &[usize],
    root_frame_row: usize,
    limits: ExactCircuitLimits,
) -> Result<RootedExactCircuitLift, ExactCircuitError> {
    let fixed_indices = fixed_index_assignments(context, partition)?;
    match try_build_normalized_reduction(
        context,
        hit,
        partition,
        predecessor_rows,
        root_frame_row,
        &fixed_indices,
        limits,
    )? {
        RootedReduction::Normalized(reduced) => {
            match super::replay::replay_exact_circuit(
                context,
                hit,
                partition,
                &fixed_indices,
                reduced,
                limits,
            )? {
                super::ExactCircuitLift::Replayed(circuit) => {
                    Ok(RootedExactCircuitLift::Replayed(circuit))
                }
                super::ExactCircuitLift::ModularSupportDidNotLift(_) => {
                    Err(ExactCircuitError::Invariant {
                        detail: "rooted exact replay unexpectedly returned a modular support miss",
                    })
                }
            }
        }
        RootedReduction::Inconclusive(miss) => Ok(RootedExactCircuitLift::Inconclusive(miss)),
    }
}

#[allow(clippy::too_many_arguments)]
fn try_build_normalized_reduction<'frame>(
    context: &IndexedCoefficientContext,
    hit: &ModularHit<'frame>,
    partition: &TargetColumnPartition<'frame>,
    predecessor_rows: &[usize],
    root_frame_row: usize,
    fixed_indices: &[(usize, i64)],
    limits: ExactCircuitLimits,
) -> Result<RootedReduction, ExactCircuitError> {
    let plan = partition.frame();
    validate_binding(context, hit, partition, plan)?;
    let selected_count = checked_add(ROOTED_SELECTED_ROWS, predecessor_rows.len(), 1)?;
    check_limit(
        ROOTED_SELECTED_ROWS,
        selected_count,
        limits.max_selected_rows,
    )?;
    let mut selected = try_vec(ROOTED_SELECTED_ROWS, selected_count)?;
    selected.extend_from_slice(predecessor_rows);
    selected.push(root_frame_row);
    validate_selected_rows(plan, &selected, limits)?;
    preflight(context, plan, partition, &selected, limits)?;

    let forbidden = partition.forbidden_columns();
    let augmented_columns = checked_add(ROOTED_AUGMENTED_COLUMNS, forbidden.len(), selected_count)?;
    check_limit(
        ROOTED_AUGMENTED_COLUMNS,
        augmented_columns,
        limits.max_augmented_columns,
    )?;
    let native_columns = checked_u32(ROOTED_AUGMENTED_COLUMNS, augmented_columns)?;

    let field = NativeField::new(Z);
    let mut reducer = call_native("constructing the rooted exact sparse reducer", || {
        SparseRowReducer::new(native_columns, field, LuLMode::Full)
    })?;
    let mut metadata: Vec<RootedForwardRowMeta> = try_vec(ROOTED_SELECTED_ROWS, selected_count)?;
    let mut retained_dependency_entries = 0usize;

    // Predecessors retain their physical chronology.  The root is inserted
    // last even though its provenance column is ordered first.
    for (insertion_row, &frame_row) in selected.iter().enumerate() {
        let provenance_column = if insertion_row + 1 == selected_count {
            forbidden.len()
        } else {
            checked_add(
                ROOTED_AUGMENTED_COLUMNS,
                checked_add(ROOTED_AUGMENTED_COLUMNS, forbidden.len(), 1)?,
                insertion_row,
            )?
        };
        let (values, columns) = try_project_rooted_row(
            context,
            plan,
            partition,
            frame_row,
            provenance_column,
            fixed_indices,
            limits,
        )?;
        let pivot = call_native("adding a row to the rooted exact sparse reducer", || {
            reducer.add_row(&values, &columns)
        })?
        .ok_or(ExactCircuitError::Invariant {
            detail: "unique rooted provenance failed to keep an exact source row independent",
        })? as usize;
        let reducer_row =
            reducer
                .u()
                .nrows()
                .checked_sub(1)
                .ok_or(ExactCircuitError::Invariant {
                    detail: "rooted exact reducer has no U row after accepting a source",
                })? as usize;
        let (lower_row, lower_columns, lower_values) =
            reducer.l().last_row().ok_or(ExactCircuitError::Invariant {
                detail: "rooted exact reducer has no L row after accepting a source",
            })?;
        if lower_row as usize != insertion_row
            || lower_columns.last().copied().map(|row| row as usize) != Some(reducer_row)
        {
            return Err(ExactCircuitError::Invariant {
                detail: "rooted exact reducer lost source chronology or its L diagonal",
            });
        }
        let raw_pivot = lower_values
            .last()
            .cloned()
            .ok_or(ExactCircuitError::Invariant {
                detail: "rooted exact reducer L diagonal has no pivot coefficient",
            })?;
        if raw_pivot.is_zero() {
            return Err(ExactCircuitError::Invariant {
                detail: "rooted exact reducer returned an identically zero pivot",
            });
        }
        let pivot_coefficient = context
            .admit_native_result_with_limits(raw_pivot, limits.indexed_algebra.exact_algebra)?;

        let mut dependency_capacity = 1usize;
        for &dependency in &lower_columns[..lower_columns.len() - 1] {
            let dependency =
                usize::try_from(dependency).map_err(|_| ExactCircuitError::Invariant {
                    detail: "rooted exact L dependency does not fit usize",
                })?;
            let dependency = metadata
                .get(dependency)
                .ok_or(ExactCircuitError::Invariant {
                    detail: "rooted exact L dependency is outside prior chronology",
                })?;
            dependency_capacity = checked_add(
                ROOTED_DEPENDENCIES,
                dependency_capacity,
                dependency.pivot_dependencies.len(),
            )?;
        }
        check_limit(
            ROOTED_DEPENDENCIES,
            dependency_capacity,
            limits.max_pivot_dependency_entries,
        )?;
        let mut pivot_dependencies = try_vec(ROOTED_DEPENDENCIES, dependency_capacity)?;
        for &dependency in &lower_columns[..lower_columns.len() - 1] {
            let dependency = dependency as usize;
            pivot_dependencies.extend_from_slice(&metadata[dependency].pivot_dependencies);
        }
        pivot_dependencies.sort_unstable();
        pivot_dependencies.dedup();
        pivot_dependencies.push(metadata.len());
        retained_dependency_entries = checked_add(
            ROOTED_DEPENDENCIES,
            retained_dependency_entries,
            pivot_dependencies.len(),
        )?;
        check_limit(
            ROOTED_DEPENDENCIES,
            retained_dependency_entries,
            limits.max_pivot_dependency_entries,
        )?;
        metadata.push(RootedForwardRowMeta {
            frame_row,
            reducer_row,
            pivot_projected_column: pivot,
            pivot_coefficient,
            pivot_dependencies,
        });

        let decomposition_nonzeros = checked_add(
            ROOTED_NATIVE_DECOMPOSITION,
            reducer.u().nvalues(),
            reducer.l().nvalues(),
        )?;
        check_limit(
            ROOTED_NATIVE_DECOMPOSITION,
            decomposition_nonzeros,
            limits.max_native_decomposition_nonzero_entries,
        )?;
    }

    let root_meta = metadata.last().ok_or(ExactCircuitError::Invariant {
        detail: "rooted exact reduction retained no designated root metadata",
    })?;
    if root_meta.reducer_row + 1 != selected_count {
        return Err(ExactCircuitError::Invariant {
            detail: "designated root is not the final rooted reducer row",
        });
    }
    if root_meta.pivot_projected_column < forbidden.len() {
        return Ok(RootedReduction::Inconclusive(
            RootedExactCircuitMiss::ForbiddenColumnsDidNotCancel {
                root_frame_row,
                first_uncancelled_physical_column: forbidden[root_meta.pivot_projected_column],
            },
        ));
    }
    if root_meta.pivot_projected_column != forbidden.len() {
        return Err(ExactCircuitError::Invariant {
            detail: "designated root pivot escaped its earliest provenance column",
        });
    }

    let (_, root_columns, root_values) =
        reducer
            .u()
            .row_iter()
            .nth(root_meta.reducer_row)
            .ok_or(ExactCircuitError::Invariant {
                detail: "designated rooted U row is absent",
            })?;
    let mut source_combination = try_vec(
        ROOTED_SOURCE_COMBINATION,
        root_columns.len().min(selected_count),
    )?;
    let mut saw_root = false;
    for (&column, raw) in root_columns.iter().zip(root_values) {
        if raw.is_zero() {
            return Err(ExactCircuitError::Invariant {
                detail: "designated rooted U row exposes an explicit zero",
            });
        }
        let column = column as usize;
        if column < forbidden.len() {
            return Err(ExactCircuitError::Invariant {
                detail: "designated rooted U row retained a forbidden coefficient after a provenance pivot",
            });
        }
        let frame_row = if column == forbidden.len() {
            if saw_root || !raw.is_one() {
                return Err(ExactCircuitError::Invariant {
                    detail: "designated root provenance is repeated or not normalized to one",
                });
            }
            saw_root = true;
            root_frame_row
        } else {
            let predecessor =
                column
                    .checked_sub(forbidden.len() + 1)
                    .ok_or(ExactCircuitError::Invariant {
                        detail: "rooted provenance column underflowed predecessor chronology",
                    })?;
            *predecessor_rows
                .get(predecessor)
                .ok_or(ExactCircuitError::Invariant {
                    detail: "rooted provenance column is outside predecessor chronology",
                })?
        };
        let coefficient = context
            .admit_native_result_with_limits(raw.clone(), limits.indexed_algebra.exact_algebra)?;
        source_combination.push(ExactFrameSourceContribution::new(
            frame_row,
            plan.source_instances()[frame_row].clone(),
            coefficient,
        ));
    }
    if !saw_root || source_combination.is_empty() {
        return Err(ExactCircuitError::Invariant {
            detail: "designated rooted U row lost its mandatory root provenance",
        });
    }
    source_combination.sort_unstable_by_key(ExactFrameSourceContribution::frame_row_ordinal);
    check_limit(
        ROOTED_SOURCE_COMBINATION,
        source_combination.len(),
        limits.max_source_combination_terms,
    )?;
    if source_combination
        .windows(2)
        .any(|pair| pair[0].frame_row_ordinal() >= pair[1].frame_row_ordinal())
    {
        return Err(ExactCircuitError::Invariant {
            detail: "rooted exact source provenance is not unique frame chronology",
        });
    }

    let Some(target_coefficient) = try_compute_target_coefficient(
        context,
        plan,
        partition.target_column(),
        fixed_indices,
        &source_combination,
        limits,
    )?
    else {
        return Ok(RootedReduction::Inconclusive(
            RootedExactCircuitMiss::TargetCoefficientVanished { root_frame_row },
        ));
    };
    let normalization_operations = source_combination.len();
    check_limit(
        ROOTED_TARGET_OPERATIONS,
        normalization_operations,
        limits.max_replay_exact_operations,
    )?;
    let denominator = context.bind_sealed(&target_coefficient)?;
    let mut normalized = try_vec(ROOTED_SOURCE_COMBINATION, source_combination.len())?;
    for contribution in source_combination {
        let numerator = context.bind_sealed(contribution.coefficient())?;
        let coefficient = context.div_bound_with_limits(
            numerator,
            denominator,
            limits.indexed_algebra.exact_algebra,
        )?;
        if coefficient.is_zero() {
            continue;
        }
        normalized.push(ExactFrameSourceContribution::new(
            contribution.frame_row_ordinal(),
            contribution.source_instance().clone(),
            coefficient,
        ));
    }
    if normalized
        .last()
        .is_none_or(|contribution| contribution.frame_row_ordinal() != root_frame_row)
    {
        return Err(ExactCircuitError::Invariant {
            detail: "normalization lost the nonzero designated root contribution",
        });
    }

    let root_dependency = metadata.len() - 1;
    let mut pivot_guards = try_vec(
        ROOTED_DEPENDENCIES,
        root_meta.pivot_dependencies.len().saturating_sub(1),
    )?;
    for &dependency in &root_meta.pivot_dependencies {
        if dependency == root_dependency {
            continue;
        }
        let dependency = metadata
            .get(dependency)
            .ok_or(ExactCircuitError::Invariant {
                detail: "rooted pivot dependency is outside reducer metadata",
            })?;
        if dependency.pivot_projected_column >= forbidden.len() {
            return Err(ExactCircuitError::Invariant {
                detail: "rooted forbidden cancellation depends on a provenance pivot",
            });
        }
        let physical_pivot_column = forbidden[dependency.pivot_projected_column];
        let bound = context.bind_sealed(&dependency.pivot_coefficient)?;
        let numerator = context.numerator_condition_from_bound(bound)?;
        pivot_guards.push(ExactCircuitPivotGuard::new(
            dependency.frame_row,
            plan.source_instances()[dependency.frame_row].clone(),
            physical_pivot_column,
            dependency.pivot_coefficient.clone(),
            numerator,
        ));
    }

    Ok(RootedReduction::Normalized(ReducedExactCircuit {
        source_combination: normalized,
        pivot_guards,
    }))
}

#[allow(clippy::too_many_arguments)]
fn try_project_rooted_row(
    context: &IndexedCoefficientContext,
    plan: &PhysicalFramePlan,
    partition: &TargetColumnPartition<'_>,
    frame_row: usize,
    provenance_column: usize,
    fixed_indices: &[(usize, i64)],
    limits: ExactCircuitLimits,
) -> Result<(Vec<crate::algebra::Coefficient>, Vec<u32>), ExactCircuitError> {
    let source =
        plan.source_for_row(frame_row)
            .ok_or(ExactCircuitError::SelectedSourceRowOutOfRange {
                row: frame_row,
                rows: plan.row_count(),
            })?;
    let structural =
        plan.column_indices_for_row(frame_row)
            .ok_or(ExactCircuitError::Invariant {
                detail: "rooted exact source has invalid physical CSR bounds",
            })?;
    if structural.len() != source.terms().len() {
        return Err(ExactCircuitError::Invariant {
            detail: "rooted exact source terms disagree with physical CSR",
        });
    }
    let mut projected_entries = 1usize; // mandatory provenance unit
    for &physical_column in structural {
        if partition
            .forbidden_columns()
            .binary_search(&(physical_column as usize))
            .is_ok()
        {
            projected_entries = checked_add(ROOTED_PROJECTED_INPUT, projected_entries, 1)?;
        }
    }
    check_limit(
        ROOTED_PROJECTED_INPUT,
        projected_entries,
        limits.max_projected_input_nonzero_entries,
    )?;
    let capacity = projected_entries;
    let mut values = try_vec(ROOTED_PROJECTED_INPUT, capacity)?;
    let mut columns = try_vec(ROOTED_PROJECTED_INPUT, capacity)?;
    for ((shift, coefficient), &physical_column) in source.terms().iter().zip(structural) {
        context
            .bind_sealed(coefficient)
            .map_err(|_| ExactCircuitError::WrongIndexedContext { row: frame_row })?;
        let physical_column = physical_column as usize;
        if plan
            .columns()
            .get(physical_column)
            .is_none_or(|column| column.values() != shift.values())
        {
            return Err(ExactCircuitError::Invariant {
                detail: "rooted exact source term differs from its physical column",
            });
        }
        let Ok(projected) = partition
            .forbidden_columns()
            .binary_search(&physical_column)
        else {
            continue;
        };
        let (coefficient, _denominator_guard) = context.specialize_fixed_indices_sealed(
            coefficient,
            fixed_indices,
            limits.indexed_algebra,
        )?;
        if coefficient.is_zero() {
            continue;
        }
        values.push(coefficient.raw().clone());
        columns.push(checked_u32("rooted exact projected column", projected)?);
    }
    values.push(context.one().raw().clone());
    columns.push(checked_u32(
        "rooted exact provenance column",
        provenance_column,
    )?);
    if values.len() > capacity || columns.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ExactCircuitError::Invariant {
            detail: "rooted exact projected row has invalid sparse ordering",
        });
    }
    Ok((values, columns))
}

fn try_compute_target_coefficient(
    context: &IndexedCoefficientContext,
    plan: &PhysicalFramePlan,
    target_column: usize,
    fixed_indices: &[(usize, i64)],
    source_combination: &[ExactFrameSourceContribution],
    limits: ExactCircuitLimits,
) -> Result<Option<IndexedCoefficient>, ExactCircuitError> {
    let mut accumulator: Option<IndexedCoefficient> = None;
    let mut operations = 0usize;
    for contribution in source_combination {
        let frame_row = contribution.frame_row_ordinal();
        let source = plan.source_for_row(frame_row).ok_or(
            ExactCircuitError::SelectedSourceRowOutOfRange {
                row: frame_row,
                rows: plan.row_count(),
            },
        )?;
        let structural =
            plan.column_indices_for_row(frame_row)
                .ok_or(ExactCircuitError::Invariant {
                    detail: "rooted target source has invalid physical CSR bounds",
                })?;
        let mut row_target = None;
        let mut saw_target = false;
        for (coefficient, &physical_column) in source.terms().values().zip(structural) {
            if physical_column as usize != target_column {
                continue;
            }
            if saw_target {
                return Err(ExactCircuitError::Invariant {
                    detail: "rooted target source repeats the target column",
                });
            }
            saw_target = true;
            let (coefficient, _denominator_guard) = context.specialize_fixed_indices_sealed(
                coefficient,
                fixed_indices,
                limits.indexed_algebra,
            )?;
            row_target = (!coefficient.is_zero()).then_some(coefficient);
        }
        let Some(row_target) = row_target else {
            continue;
        };
        operations = checked_add(ROOTED_TARGET_OPERATIONS, operations, 1)?;
        check_limit(
            ROOTED_TARGET_OPERATIONS,
            operations,
            limits.max_replay_exact_operations,
        )?;
        let multiplier = context.bind_sealed(contribution.coefficient())?;
        let row_target = context.bind_sealed(&row_target)?;
        let product = context.mul_bound_with_limits(
            multiplier,
            row_target,
            limits.indexed_algebra.exact_algebra,
        )?;
        if let Some(previous) = accumulator.take() {
            operations = checked_add(ROOTED_TARGET_OPERATIONS, operations, 1)?;
            check_limit(
                ROOTED_TARGET_OPERATIONS,
                operations,
                limits.max_replay_exact_operations,
            )?;
            let previous = context.bind_sealed(&previous)?;
            let product = context.bind_sealed(&product)?;
            let sum = context.add_bound_with_limits(
                previous,
                product,
                limits.indexed_algebra.exact_algebra,
            )?;
            if !sum.is_zero() {
                accumulator = Some(sum);
            }
        } else if !product.is_zero() {
            accumulator = Some(product);
        }
    }
    Ok(accumulator)
}

#[cfg(test)]
mod tests {
    use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
    use crate::family::{AffineDenominator, IntegralFamily};
    use crate::foundry::completion::frame::modular::{
        ModularHit, ModularKernelLimits, ModularPhysicalFrame, ModularTargetQuery,
    };
    use crate::foundry::completion::frame::{
        OneSidedChartFrame, PhysicalFrameLimits, PhysicalFramePlan,
    };
    use crate::foundry::completion::stratum::{
        DecoratedStratum, ImmutableOwnerSnapshot, StratumRegistryLimits, TargetColumnPartition,
    };
    use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
    use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

    use super::super::model::ExactFrameSourceContribution;
    use super::super::reduce::fixed_index_assignments;
    use super::super::replay::replay_exact_circuit;
    use super::{
        RootedExactCircuitLift, RootedExactCircuitMiss, RootedReduction,
        try_build_normalized_reduction, try_lift_rooted_exact_circuit,
    };
    use crate::foundry::completion::frame::exact::{
        ExactCircuitError, ExactCircuitLift, ExactCircuitLimits, try_lift_exact_circuit,
    };

    const PRIME: u64 = 1_000_000_007;

    fn sunset_family() -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_one = coefficients.integer(-1);
        IntegralFamily::new(
            "rooted-exact-sunset",
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    minus_one.clone(),
                    vec![one.clone(), zero.clone(), zero.clone()],
                ),
                AffineDenominator::new(
                    minus_one.clone(),
                    vec![zero.clone(), zero.clone(), one.clone()],
                ),
                AffineDenominator::new(minus_one, vec![one.clone(), coefficients.integer(2), one]),
            ],
            Vec::new(),
            vec![zero.clone(), zero.clone(), zero],
        )
        .unwrap()
    }

    fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
        let prepared = generator.prepare_ordinary_ibp().unwrap();
        let rows = (0..prepared.len())
            .map(|ordinal| prepared.generate(ordinal))
            .collect();
        prepared.complete(rows).unwrap()
    }

    fn sunset_frame(degree: usize) -> (IndexedCoefficientContext, PhysicalFramePlan) {
        let family = sunset_family();
        let generator = ParametricIbpGenerator::try_new(&family).unwrap();
        let context = generator.context().clone();
        let completed = complete_ordinary(&generator);
        let plan = OneSidedChartFrame::try_new(
            &generator,
            &completed,
            Mask::try_new([true, true, true]).unwrap(),
            degree,
            PhysicalFrameLimits::default(),
        )
        .unwrap()
        .into_plan();
        (context, plan)
    }

    fn partition<'frame>(
        plan: &'frame PhysicalFramePlan,
        target: usize,
    ) -> TargetColumnPartition<'frame> {
        let shifts = plan
            .columns()
            .iter()
            .map(|shift| shift.values())
            .collect::<Vec<_>>();
        let domain = SectorMonotoneDomain::try_maximal_for_rule(
            plan.sector().clone(),
            plan.columns()[target].values(),
            &shifts,
        )
        .unwrap();
        let limits = StratumRegistryLimits::default();
        let stratum = DecoratedStratum::try_guard_blind(
            plan.family_fingerprint(),
            plan.context_fingerprint(),
            domain,
            limits,
        )
        .unwrap();
        let owners = ImmutableOwnerSnapshot::try_empty(
            plan.family_fingerprint(),
            plan.context_fingerprint(),
            plan.sector().arity(),
            limits,
        )
        .unwrap();
        TargetColumnPartition::try_new(
            plan,
            target,
            stratum,
            owners,
            OrderingPolicy::default(),
            limits,
        )
        .unwrap()
    }

    fn sample<'frame>(
        context: &IndexedCoefficientContext,
        plan: &'frame PhysicalFramePlan,
    ) -> ModularPhysicalFrame<'frame> {
        plan.try_modular_sample(
            context,
            PRIME,
            &[37],
            &[2, 2, 2],
            ModularKernelLimits::default(),
        )
        .unwrap()
    }

    fn hit_for_target<'frame>(
        context: &IndexedCoefficientContext,
        plan: &'frame PhysicalFramePlan,
        partition: &TargetColumnPartition<'frame>,
    ) -> ModularHit<'frame> {
        let sampled = sample(context, plan);
        let ModularTargetQuery::Hit(hit) = sampled
            .query_target(
                partition.target_column(),
                partition.forbidden_columns(),
                ModularKernelLimits::default(),
            )
            .unwrap()
        else {
            panic!("the selected sunset target must have a modular hit")
        };
        hit
    }

    fn rooted_sunset_fixture() -> (IndexedCoefficientContext, PhysicalFramePlan, usize, usize) {
        let (context, plan) = sunset_frame(1);
        // These identities are deliberately frame ordinals, not topology
        // labels. They pin a small deterministic exact regression where the
        // first target relation uses rows 4 and 7, while another relation is
        // rooted at later row 11.
        let target = 19;
        let later_root = 11;
        assert!(target < plan.columns().len());
        assert!(later_root < plan.row_count());
        (context, plan, target, later_root)
    }

    #[test]
    fn later_all_predecessors_plus_root_recovers_a_distinct_exact_relation() {
        let (context, plan, target, later_root) = rooted_sunset_fixture();
        let partition = partition(&plan, target);
        let hit = hit_for_target(&context, &plan, &partition);
        let ExactCircuitLift::Replayed(first) =
            try_lift_exact_circuit(&context, &hit, &partition, ExactCircuitLimits::default())
                .unwrap()
        else {
            panic!("the first exact sunset target support must replay")
        };
        let first_rows = first
            .source_combination()
            .iter()
            .map(ExactFrameSourceContribution::frame_row_ordinal)
            .collect::<Vec<_>>();
        assert_eq!(first_rows, [4, 7]);

        let predecessors = (0..later_root).collect::<Vec<_>>();
        let RootedExactCircuitLift::Replayed(later) = try_lift_rooted_exact_circuit(
            &context,
            &hit,
            &partition,
            &predecessors,
            later_root,
            ExactCircuitLimits::default(),
        )
        .unwrap() else {
            panic!("the later rooted sunset relation must replay exactly")
        };
        let later_rows = later
            .source_combination()
            .iter()
            .map(ExactFrameSourceContribution::frame_row_ordinal)
            .collect::<Vec<_>>();
        assert_ne!(later_rows, first_rows);
        assert_eq!(later_rows.last(), Some(&later_root));
        assert!(later_rows.len() > first_rows.len());
        assert_eq!(later.target_column(), target);
    }

    #[test]
    fn exact_zero_rooted_target_is_typed_inconclusive() {
        let (context, plan, target, _) = rooted_sunset_fixture();
        let partition = partition(&plan, target);
        let hit = hit_for_target(&context, &plan, &partition);
        let zero_root = 3;
        let outcome = try_lift_rooted_exact_circuit(
            &context,
            &hit,
            &partition,
            &(0..zero_root).collect::<Vec<_>>(),
            zero_root,
            ExactCircuitLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            outcome,
            RootedExactCircuitLift::Inconclusive(
                RootedExactCircuitMiss::TargetCoefficientVanished { root_frame_row: 3 }
            )
        ));
    }

    #[test]
    fn regenerated_full_source_replay_rejects_a_rooted_multiplier_mismatch() {
        let (context, plan, target, later_root) = rooted_sunset_fixture();
        let partition = partition(&plan, target);
        let hit = hit_for_target(&context, &plan, &partition);
        let fixed = fixed_index_assignments(&context, &partition).unwrap();
        let predecessors = (0..later_root).collect::<Vec<_>>();
        let RootedReduction::Normalized(mut reduced) = try_build_normalized_reduction(
            &context,
            &hit,
            &partition,
            &predecessors,
            later_root,
            &fixed,
            ExactCircuitLimits::default(),
        )
        .unwrap() else {
            panic!("the rooted fixture must produce a normalized exact combination")
        };
        let original = &reduced.source_combination[0];
        let doubled = context
            .add(original.coefficient(), original.coefficient())
            .unwrap();
        reduced.source_combination[0] = ExactFrameSourceContribution::new(
            original.frame_row_ordinal(),
            original.source_instance().clone(),
            doubled,
        );
        assert!(matches!(
            replay_exact_circuit(
                &context,
                &hit,
                &partition,
                &fixed,
                reduced,
                ExactCircuitLimits::default(),
            ),
            Err(ExactCircuitError::ReplayMismatch { .. })
        ));
    }

    #[test]
    fn rooted_selected_row_limit_fails_before_native_reduction() {
        let (context, plan, target, later_root) = rooted_sunset_fixture();
        let partition = partition(&plan, target);
        let hit = hit_for_target(&context, &plan, &partition);
        let mut limits = ExactCircuitLimits::default();
        limits.max_selected_rows = 0;
        assert_eq!(
            try_lift_rooted_exact_circuit(
                &context,
                &hit,
                &partition,
                &(0..later_root).collect::<Vec<_>>(),
                later_root,
                limits,
            )
            .unwrap_err(),
            ExactCircuitError::ResourceLimit {
                resource: "rooted exact-circuit selected source rows",
                requested: later_root + 1,
                limit: 0,
            }
        );
    }
}
