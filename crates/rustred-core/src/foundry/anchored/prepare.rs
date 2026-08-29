use std::sync::Arc;

use crate::algebra::{Coefficient, CoefficientPolynomial, IndexedCoefficientContext};
use crate::family::IntegralKey;
use crate::identity::{IdentityConditionSource, ParametricRelation, RowId};
use crate::sector::{ComplexityKey, OrderingPolicy};

use super::error::AnchoredRuleError;
use super::limits::AnchoredRuleLimits;
use super::model::GuardOrigin;

#[derive(Debug)]
pub(super) struct PreparedProblem {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) anchor: IntegralKey,
    pub(super) ordering: OrderingPolicy,
    pub(super) columns: Vec<OrderedIntegral>,
    pub(super) sources: Vec<PreparedSourceRow>,
}

#[derive(Debug)]
pub(super) struct OrderedIntegral {
    pub(super) key: IntegralKey,
    pub(super) complexity: ComplexityKey,
}

#[derive(Debug)]
pub(super) struct PreparedSourceRow {
    pub(super) row_id: RowId,
    pub(super) entries: Vec<(u32, Coefficient)>,
    pub(super) guards: Vec<PreparedGuard>,
}

#[derive(Debug)]
pub(super) struct PreparedGuard {
    pub(super) polynomial: CoefficientPolynomial,
    pub(super) origin: GuardOrigin,
}

struct UnmappedSourceRow {
    row_id: RowId,
    entries: Vec<(IntegralKey, Coefficient)>,
    guards: Vec<PreparedGuard>,
}

pub(super) fn prepare_problem(
    context: &IndexedCoefficientContext,
    relations: &[ParametricRelation],
    anchor: &[i64],
    ordering: OrderingPolicy,
    limits: AnchoredRuleLimits,
) -> Result<PreparedProblem, AnchoredRuleError> {
    if relations.is_empty() {
        return Err(AnchoredRuleError::EmptySourceRows);
    }
    check_limit(
        "anchored source rows",
        relations.len(),
        limits.max_source_rows,
    )?;
    if anchor.len() != context.index_count() {
        return Err(AnchoredRuleError::WrongAnchorArity {
            expected: context.index_count(),
            actual: anchor.len(),
        });
    }

    let family_fingerprint = relations[0].family_fingerprint_owner();
    for (source_ordinal, relation) in relations.iter().enumerate() {
        relation
            .validate_context(context)
            .map_err(|_| AnchoredRuleError::WrongSourceContext { source_ordinal })?;
        if relation.family_fingerprint_owner() != family_fingerprint {
            return Err(AnchoredRuleError::WrongSourceFamily { source_ordinal });
        }
    }

    let prospective_terms = relations.iter().try_fold(0usize, |count, relation| {
        checked_add(
            "prospective anchored source terms",
            count,
            relation.terms().len(),
        )
    })?;
    let prospective_input_entries = checked_add(
        "prospective anchored source entries",
        prospective_terms,
        relations.len(),
    )?;
    check_limit(
        "prospective anchored source entries",
        prospective_input_entries,
        limits.max_input_nonzero_entries,
    )?;

    let mut unmapped = try_vec("prepared anchored source rows", relations.len())?;
    let mut all_integrals = try_vec("prospective anchored integral keys", prospective_terms)?;
    let mut physical_nonzeros = 0usize;
    let mut prepared_guard_origins = 0usize;
    let mut retained_condition_sources = 0usize;
    let mut guard_provenance_index_cells = 0usize;

    for (source_ordinal, relation) in relations.iter().enumerate() {
        let row_id = relation.row_id().clone();
        let prospective_guards = checked_add(
            "prospective anchored source guards",
            relation.nonzero_conditions().len(),
            relation.terms().len(),
        )?;
        let mut guards = try_vec("prepared anchored source guards", prospective_guards)?;

        for (condition_ordinal, condition) in relation.nonzero_conditions().iter().enumerate() {
            let polynomial = context.specialize_polynomial(
                condition.polynomial(),
                anchor,
                limits.indexed_algebra,
            )?;
            if polynomial.is_zero() {
                return Err(AnchoredRuleError::UnsatisfiedSourceCondition {
                    source_ordinal,
                    condition_ordinal,
                });
            }
            if polynomial.is_constant() {
                continue;
            }
            prepared_guard_origins =
                checked_add("prepared anchored guard origins", prepared_guard_origins, 1)?;
            check_limit(
                "prepared anchored guard origins",
                prepared_guard_origins,
                limits.max_guard_origins,
            )?;
            retained_condition_sources = checked_add(
                "anchored guard provenance sources",
                retained_condition_sources,
                condition.sources().len(),
            )?;
            check_limit(
                "anchored guard provenance sources",
                retained_condition_sources,
                limits.max_guard_provenance_sources,
            )?;
            let condition_index_cells =
                condition
                    .sources()
                    .iter()
                    .try_fold(0usize, |cells, source| {
                        checked_add(
                            "anchored guard provenance index cells",
                            cells,
                            condition_source_index_cells(source),
                        )
                    })?;
            guard_provenance_index_cells = checked_add(
                "anchored guard provenance index cells",
                guard_provenance_index_cells,
                condition_index_cells,
            )?;
            check_limit(
                "anchored guard provenance index cells",
                guard_provenance_index_cells,
                limits.max_guard_provenance_index_cells,
            )?;
            let condition_sources = clone_condition_sources(condition.sources().iter().cloned())?;
            guards.push(PreparedGuard {
                polynomial,
                origin: GuardOrigin::SourceCondition {
                    source_ordinal,
                    row_id: row_id.clone(),
                    condition_ordinal,
                    condition_sources,
                },
            });
        }

        let mut entries = try_vec("prepared anchored source terms", relation.terms().len())?;
        for (shift, indexed_coefficient) in relation.terms() {
            // Every retained nonzero owns one key in its source row and one
            // clone in `all_integrals`. Admit the temporary candidate key
            // before specializing its coefficient and learning whether it
            // survives at this anchor.
            let retained_keys =
                checked_mul("live anchored integral-key count", physical_nonzeros, 2)?;
            let with_candidate = checked_add("live anchored integral-key count", retained_keys, 1)?;
            check_cell_limit(
                "live anchored integral-key power cells",
                with_candidate,
                context.index_count(),
                limits.max_integral_key_power_cells,
            )?;
            let key = add_anchor_shift(anchor, shift.values(), source_ordinal)?;
            let (coefficient, denominator) =
                context.specialize(indexed_coefficient, anchor, limits.indexed_algebra)?;
            if let Some(polynomial) = denominator.filter(|value| !value.is_constant()) {
                prepared_guard_origins =
                    checked_add("prepared anchored guard origins", prepared_guard_origins, 1)?;
                check_limit(
                    "prepared anchored guard origins",
                    prepared_guard_origins,
                    limits.max_guard_origins,
                )?;
                guard_provenance_index_cells = checked_add(
                    "anchored guard provenance index cells",
                    guard_provenance_index_cells,
                    shift.values().len(),
                )?;
                check_limit(
                    "anchored guard provenance index cells",
                    guard_provenance_index_cells,
                    limits.max_guard_provenance_index_cells,
                )?;
                guards.push(PreparedGuard {
                    polynomial,
                    origin: GuardOrigin::SourceCoefficientDenominator {
                        source_ordinal,
                        row_id: row_id.clone(),
                        shift: copy_i64_slice(shift.values(), "anchored guard source shift")?
                            .into_boxed_slice(),
                    },
                });
            }
            if coefficient.is_zero() {
                continue;
            }
            let next_physical_nonzeros =
                checked_add("anchored source nonzero entries", physical_nonzeros, 1)?;
            let retained_keys = checked_mul(
                "live anchored integral-key count",
                next_physical_nonzeros,
                2,
            )?;
            check_cell_limit(
                "live anchored integral-key power cells",
                retained_keys,
                context.index_count(),
                limits.max_integral_key_power_cells,
            )?;
            physical_nonzeros = next_physical_nonzeros;
            let prospective_input = checked_add(
                "anchored source nonzero entries",
                physical_nonzeros,
                relations.len(),
            )?;
            check_limit(
                "anchored source nonzero entries",
                prospective_input,
                limits.max_input_nonzero_entries,
            )?;
            all_integrals.push(clone_integral_key(&key)?);
            entries.push((key, coefficient));
        }
        unmapped.push(UnmappedSourceRow {
            row_id,
            entries,
            guards,
        });
    }

    all_integrals.sort_unstable();
    all_integrals.dedup();
    check_limit(
        "anchored integral columns",
        all_integrals.len(),
        limits.max_integral_columns,
    )?;
    let prepared_ordering_keys = checked_mul(
        "prepared ordering-key coordinate buffers",
        all_integrals.len(),
        2,
    )?;
    // A ComplexityKey retains both a sector-bit buffer and an index-excess
    // buffer, each with one coordinate per integral index.
    check_cell_limit(
        "live anchored ordering-key coordinate cells",
        prepared_ordering_keys,
        context.index_count(),
        limits.max_ordering_key_coordinate_cells,
    )?;
    let mut columns = try_vec("ordered anchored integral columns", all_integrals.len())?;
    for key in all_integrals {
        columns.push(OrderedIntegral {
            complexity: ordering.complexity_key(key.powers())?,
            key,
        });
    }
    columns.sort_unstable_by(|left, right| right.complexity.cmp(&left.complexity));

    let augmented_columns =
        checked_add("anchored augmented columns", columns.len(), relations.len())?;
    check_limit(
        "anchored augmented columns",
        augmented_columns,
        limits.max_augmented_columns,
    )?;
    if u32::try_from(augmented_columns).is_err() {
        return Err(AnchoredRuleError::ResourceLimit {
            resource: "Symbolica sparse column indices",
            requested: augmented_columns,
            limit: u32::MAX as usize,
        });
    }

    let mut lookup = try_vec("anchored integral column lookup", columns.len())?;
    lookup.extend(0..columns.len());
    lookup.sort_unstable_by(|&left, &right| columns[left].key.cmp(&columns[right].key));
    let mut sources = try_vec("indexed anchored source rows", unmapped.len())?;
    for row in unmapped {
        let mut entries = try_vec("indexed anchored source entries", row.entries.len())?;
        for (key, coefficient) in row.entries {
            let position = lookup
                .binary_search_by(|&column| columns[column].key.cmp(&key))
                .map_err(|_| AnchoredRuleError::ReducerInvariant {
                    detail: "a prepared integral is absent from its column lookup",
                })?;
            let column = u32::try_from(lookup[position]).map_err(|_| {
                AnchoredRuleError::ReducerInvariant {
                    detail: "an admitted integral column does not fit u32",
                }
            })?;
            entries.push((column, coefficient));
        }
        entries.sort_unstable_by_key(|(column, _)| *column);
        sources.push(PreparedSourceRow {
            row_id: row.row_id,
            entries,
            guards: row.guards,
        });
    }

    let retained_integral_keys = checked_add("live anchored integral-key count", columns.len(), 1)?;
    check_cell_limit(
        "live anchored integral-key power cells",
        retained_integral_keys,
        context.index_count(),
        limits.max_integral_key_power_cells,
    )?;
    let anchor = IntegralKey::try_from_preallocated(copy_i64_slice(
        anchor,
        "anchored rule integer assignment",
    )?)?;
    Ok(PreparedProblem {
        family_fingerprint,
        anchor,
        ordering,
        columns,
        sources,
    })
}

fn condition_source_index_cells(source: &IdentityConditionSource) -> usize {
    match source {
        IdentityConditionSource::RelationInputTermDenominator { shift, .. }
        | IdentityConditionSource::RelationCollectedTermDenominator { shift, .. } => shift.len(),
        IdentityConditionSource::RelationTranslation { offset, .. }
        | IdentityConditionSource::IndexTranslation { offset } => offset.len(),
        IdentityConditionSource::FamilyInputCoefficientDenominator { .. }
        | IdentityConditionSource::FamilyBasisDeterminantNumerator
        | IdentityConditionSource::RelationConditionAttached { .. }
        | IdentityConditionSource::RelationScaleFactorDenominator { .. } => 0,
    }
}

fn add_anchor_shift(
    anchor: &[i64],
    shift: &[i64],
    source_ordinal: usize,
) -> Result<IntegralKey, AnchoredRuleError> {
    debug_assert_eq!(anchor.len(), shift.len());
    let mut powers = try_vec("anchored integral powers", anchor.len())?;
    for (position, (&base, &offset)) in anchor.iter().zip(shift).enumerate() {
        powers.push(
            base.checked_add(offset)
                .ok_or(AnchoredRuleError::AnchorIndexOverflow {
                    source_ordinal,
                    position,
                })?,
        );
    }
    Ok(IntegralKey::try_from_preallocated(powers)?)
}

fn clone_condition_sources(
    sources: impl ExactSizeIterator<Item = IdentityConditionSource>,
) -> Result<Box<[IdentityConditionSource]>, AnchoredRuleError> {
    let mut retained = try_vec("anchored condition provenance", sources.len())?;
    retained.extend(sources);
    Ok(retained.into_boxed_slice())
}

pub(super) fn copy_i64_slice(
    source: &[i64],
    resource: &'static str,
) -> Result<Vec<i64>, AnchoredRuleError> {
    let mut retained = try_vec(resource, source.len())?;
    retained.extend_from_slice(source);
    Ok(retained)
}

pub(super) fn clone_integral_key(source: &IntegralKey) -> Result<IntegralKey, AnchoredRuleError> {
    Ok(IntegralKey::try_from_preallocated(copy_i64_slice(
        source.powers(),
        "anchored integral-key clone",
    )?)?)
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, AnchoredRuleError> {
    left.checked_add(right)
        .ok_or(AnchoredRuleError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, AnchoredRuleError> {
    left.checked_mul(right)
        .ok_or(AnchoredRuleError::ResourceCountOverflow { resource })
}

pub(super) fn check_cell_limit(
    resource: &'static str,
    containers: usize,
    cells_per_container: usize,
    limit: usize,
) -> Result<(), AnchoredRuleError> {
    let requested = checked_mul(resource, containers, cells_per_container)?;
    check_limit(resource, requested, limit)
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), AnchoredRuleError> {
    if requested > limit {
        Err(AnchoredRuleError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, AnchoredRuleError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| AnchoredRuleError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}
