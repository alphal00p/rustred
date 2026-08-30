use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::family::IntegralKey;
use crate::identity::{IdentityConditionSource, IndexShift, ParametricRelation, RowId};
use crate::sector::{Mask, OrderingPolicy, SectorInteriorDomain, ShiftComplexityKey};

use super::error::ParametricRuleError;
use super::limits::ParametricRuleLimits;
use super::model::ParametricGuardOrigin;

#[derive(Debug)]
pub(super) struct PreparedProblem {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) context_fingerprint: Arc<String>,
    pub(super) anchor: IntegralKey,
    pub(super) ordering: OrderingPolicy,
    pub(super) domain: SectorInteriorDomain,
    pub(super) columns: Vec<OrderedShift>,
    pub(super) sources: Vec<PreparedSourceRow>,
}

#[derive(Debug)]
pub(super) struct OrderedShift {
    pub(super) shift: IndexShift,
    pub(super) complexity: ShiftComplexityKey,
}

#[derive(Debug)]
pub(super) struct PreparedSourceRow {
    pub(super) row_id: RowId,
    pub(super) entries: Vec<(u32, IndexedCoefficient)>,
    pub(super) guards: Vec<PreparedGuard>,
}

#[derive(Debug)]
pub(super) struct PreparedGuard {
    pub(super) polynomial: IndexedPolynomial,
    pub(super) origin: ParametricGuardOrigin,
}

struct UnmappedSourceRow {
    row_id: RowId,
    entries: Vec<(IndexShift, IndexedCoefficient)>,
    guards: Vec<PreparedGuard>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AnchorRequirement {
    Interior,
    SectorMonotoneReplay,
}

pub(super) fn prepare_problem(
    context: &IndexedCoefficientContext,
    relations: &[ParametricRelation],
    anchor: &[i64],
    ordering: OrderingPolicy,
    limits: ParametricRuleLimits,
) -> Result<PreparedProblem, ParametricRuleError> {
    prepare_problem_with_anchor_requirement(
        context,
        relations,
        anchor,
        ordering,
        limits,
        AnchorRequirement::Interior,
    )
}

pub(super) fn prepare_sector_monotone_problem(
    context: &IndexedCoefficientContext,
    relations: &[ParametricRelation],
    anchor: &[i64],
    ordering: OrderingPolicy,
    limits: ParametricRuleLimits,
) -> Result<PreparedProblem, ParametricRuleError> {
    prepare_problem_with_anchor_requirement(
        context,
        relations,
        anchor,
        ordering,
        limits,
        AnchorRequirement::SectorMonotoneReplay,
    )
}

fn prepare_problem_with_anchor_requirement(
    context: &IndexedCoefficientContext,
    relations: &[ParametricRelation],
    anchor: &[i64],
    ordering: OrderingPolicy,
    limits: ParametricRuleLimits,
    anchor_requirement: AnchorRequirement,
) -> Result<PreparedProblem, ParametricRuleError> {
    if relations.is_empty() {
        return Err(ParametricRuleError::EmptySourceRows);
    }
    check_limit(
        "parametric source rows",
        relations.len(),
        limits.max_source_rows,
    )?;
    if anchor.len() != context.index_count() {
        return Err(ParametricRuleError::WrongAnchorArity {
            expected: context.index_count(),
            actual: anchor.len(),
        });
    }
    let family_fingerprint = relations[0].family_fingerprint_owner();
    for (source_ordinal, relation) in relations.iter().enumerate() {
        relation
            .validate_context(context)
            .map_err(|_| ParametricRuleError::WrongSourceContext { source_ordinal })?;
        if relation.family_fingerprint_owner() != family_fingerprint {
            return Err(ParametricRuleError::WrongSourceFamily { source_ordinal });
        }
    }

    let prospective_terms = relations.iter().try_fold(0usize, |count, relation| {
        checked_add(
            "prospective parametric source terms",
            count,
            relation.terms().len(),
        )
    })?;
    let prospective_input_entries = checked_add(
        "prospective parametric source entries",
        prospective_terms,
        relations.len(),
    )?;
    check_limit(
        "prospective parametric source entries",
        prospective_input_entries,
        limits.max_input_nonzero_entries,
    )?;
    // Each distinct source shift may remain live through an Arc handle in the
    // foundry's columns and eventual rule. Before deduplication, the total
    // term count is the allocation-free upper bound on those unique buffers.
    // Multiple handles never multiply the coordinate-cell census.
    check_cell_limit(
        "prospective parametric shift coordinate cells",
        prospective_terms,
        context.index_count(),
        limits.max_index_coordinate_cells,
    )?;
    let mut unmapped = try_vec("prepared parametric source rows", relations.len())?;
    let mut all_shifts = try_vec("prospective parametric shift columns", prospective_terms)?;
    let mut physical_nonzeros = 0usize;
    let mut prepared_guard_origins = 0usize;
    let mut retained_condition_sources = 0usize;
    let mut guard_provenance_index_cells = 0usize;

    for (source_ordinal, relation) in relations.iter().enumerate() {
        let row_id = relation.row_id().clone();
        let prospective_guards = checked_add(
            "prospective parametric source guards",
            relation.nonzero_conditions().len(),
            relation.terms().len(),
        )?;
        let mut guards = try_vec("prepared parametric source guards", prospective_guards)?;

        for (condition_ordinal, condition) in relation.nonzero_conditions().iter().enumerate() {
            context.validate_polynomial_with_limits(
                condition.polynomial(),
                limits.indexed_algebra.exact_algebra,
            )?;
            if condition.polynomial().is_zero() {
                return Err(ParametricRuleError::IdenticallyZeroSourceCondition {
                    source_ordinal,
                    condition_ordinal,
                });
            }
            if condition.polynomial().is_nonzero_constant() {
                continue;
            }
            prepared_guard_origins = checked_add(
                "prepared parametric guard origins",
                prepared_guard_origins,
                1,
            )?;
            check_limit(
                "prepared parametric guard origins",
                prepared_guard_origins,
                limits.max_guard_origins,
            )?;
            retained_condition_sources = checked_add(
                "parametric guard provenance sources",
                retained_condition_sources,
                condition.sources().len(),
            )?;
            check_limit(
                "parametric guard provenance sources",
                retained_condition_sources,
                limits.max_guard_provenance_sources,
            )?;
            let condition_index_cells =
                condition
                    .sources()
                    .iter()
                    .try_fold(0usize, |cells, source| {
                        checked_add(
                            "parametric guard provenance index cells",
                            cells,
                            condition_source_index_cells(source),
                        )
                    })?;
            guard_provenance_index_cells = checked_add(
                "parametric guard provenance index cells",
                guard_provenance_index_cells,
                condition_index_cells,
            )?;
            check_limit(
                "parametric guard provenance index cells",
                guard_provenance_index_cells,
                limits.max_guard_provenance_index_cells,
            )?;
            let condition_sources = clone_condition_sources(condition.sources().iter())?;
            guards.push(PreparedGuard {
                polynomial: condition.polynomial().clone(),
                origin: ParametricGuardOrigin::SourceCondition {
                    source_ordinal,
                    row_id: row_id.clone(),
                    condition_ordinal,
                    condition_sources,
                },
            });
        }

        let mut entries = try_vec("prepared parametric source terms", relation.terms().len())?;
        for (shift, coefficient) in relation.terms() {
            context.validate_with_limits(coefficient, limits.indexed_algebra.exact_algebra)?;
            if coefficient.is_zero() {
                continue;
            }
            let coefficient_bound = context.bind_sealed(coefficient)?;
            let denominator = context.denominator_condition_from_bound(coefficient_bound)?;
            if !denominator.is_nonzero_constant() {
                prepared_guard_origins = checked_add(
                    "prepared parametric guard origins",
                    prepared_guard_origins,
                    1,
                )?;
                check_limit(
                    "prepared parametric guard origins",
                    prepared_guard_origins,
                    limits.max_guard_origins,
                )?;
                // The guard retains another `IndexShift` handle to this
                // source term's prospectively counted buffer. After column
                // deduplication it is rebound to the value-canonical column
                // handle, so duplicate-valued source buffers do not survive
                // into the returned rule. Only deep-cloned condition-source
                // offsets contribute to the provenance-cell census above.
                guards.push(PreparedGuard {
                    polynomial: denominator,
                    origin: ParametricGuardOrigin::SourceCoefficientDenominator {
                        source_ordinal,
                        row_id: row_id.clone(),
                        shift: shift.clone(),
                    },
                });
            }
            physical_nonzeros =
                checked_add("parametric source nonzero entries", physical_nonzeros, 1)?;
            let prospective_input = checked_add(
                "parametric source nonzero entries",
                physical_nonzeros,
                relations.len(),
            )?;
            check_limit(
                "parametric source nonzero entries",
                prospective_input,
                limits.max_input_nonzero_entries,
            )?;
            all_shifts.push(shift.clone());
            entries.push((shift.clone(), coefficient.clone()));
        }
        unmapped.push(UnmappedSourceRow {
            row_id,
            entries,
            guards,
        });
    }

    all_shifts.sort_unstable();
    all_shifts.dedup();
    check_limit(
        "parametric shift columns",
        all_shifts.len(),
        limits.max_shift_columns,
    )?;
    // The exact returned non-provenance payload is one buffer per unique
    // shift, plus the replay anchor which will be copied near the return
    // below. Source-coefficient guard origins are rebound to these canonical
    // column handles before the anchor allocation. Preflight that payload
    // before constructing any later owner.
    let live_index_coordinate_buffers = checked_add(
        "live parametric index-coordinate buffers",
        all_shifts.len(),
        1,
    )?;
    check_cell_limit(
        "live parametric index-coordinate cells",
        live_index_coordinate_buffers,
        context.index_count(),
        limits.max_index_coordinate_cells,
    )?;
    // Mask construction allocates one bool per index. Preflight it before
    // `Mask::try_from_indices` reserves or fills that buffer.
    check_limit(
        "parametric sector mask cells",
        anchor.len(),
        limits.max_sector_mask_cells,
    )?;
    let sector = Mask::try_from_indices(anchor)?;
    let mut shift_values = try_vec("sector-interior shift borrows", all_shifts.len())?;
    shift_values.extend(all_shifts.iter().map(IndexShift::values));
    // Every domain coordinate owns an inclusive lower and upper i64 endpoint.
    // Admit the complete buffer before its constructor allocates it.
    check_cell_limit(
        "parametric domain bound endpoint cells",
        anchor.len(),
        2,
        limits.max_domain_bound_endpoint_cells,
    )?;
    let domain = SectorInteriorDomain::try_maximal_for_shifts(sector.clone(), &shift_values)?;
    if anchor_requirement == AnchorRequirement::Interior && !domain.contains(anchor)? {
        return Err(ParametricRuleError::AnchorOutsideInterior);
    }
    if anchor_requirement == AnchorRequirement::Interior
        && domain
            .bounds()
            .iter()
            .all(|bounds| bounds.lower() == bounds.upper())
    {
        return Err(ParametricRuleError::DegenerateSinglePointInterior);
    }

    // Each structural key allocates one i128 offset per index. The mask clone
    // inside a key is another Arc handle and owns no new bool cells.
    check_cell_limit(
        "live parametric ordering-key coordinate cells",
        all_shifts.len(),
        context.index_count(),
        limits.max_ordering_key_coordinate_cells,
    )?;
    let mut columns = try_vec("ordered parametric shift columns", all_shifts.len())?;
    for shift in all_shifts {
        columns.push(OrderedShift {
            complexity: ordering.shift_complexity_key(&sector, shift.values())?,
            shift,
        });
    }
    columns.sort_unstable_by(|left, right| right.complexity.cmp(&left.complexity));

    let augmented_columns = checked_add(
        "parametric augmented columns",
        columns.len(),
        relations.len(),
    )?;
    check_limit(
        "parametric augmented columns",
        augmented_columns,
        limits.max_augmented_columns,
    )?;
    if u32::try_from(augmented_columns).is_err() {
        return Err(ParametricRuleError::ResourceLimit {
            resource: "Symbolica sparse column indices",
            requested: augmented_columns,
            limit: u32::MAX as usize,
        });
    }

    let mut lookup = try_vec("parametric shift column lookup", columns.len())?;
    lookup.extend(0..columns.len());
    lookup.sort_unstable_by(|&left, &right| columns[left].shift.cmp(&columns[right].shift));
    let mut sources = try_vec("indexed parametric source rows", unmapped.len())?;
    for mut row in unmapped {
        // Equal shifts from independently generated relations can be backed
        // by pointer-distinct Arcs. Origins must retain the canonical column
        // Arc, not whichever source-term Arc happened to create the guard,
        // for the unique-buffer coordinate census above to remain exact.
        for guard in &mut row.guards {
            if let ParametricGuardOrigin::SourceCoefficientDenominator { shift, .. } =
                &mut guard.origin
            {
                let column = find_shift_column(&columns, &lookup, shift)?;
                *shift = columns[column].shift.clone();
            }
        }
        let mut entries = try_vec("indexed parametric source entries", row.entries.len())?;
        for (shift, coefficient) in row.entries {
            let column = find_shift_column(&columns, &lookup, &shift)?;
            let column =
                u32::try_from(column).map_err(|_| ParametricRuleError::ReducerInvariant {
                    detail: "an admitted shift column does not fit u32",
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

    let anchor = IntegralKey::try_from_preallocated(copy_i64_slice(
        anchor,
        "parametric rule concrete replay anchor",
    )?)?;
    Ok(PreparedProblem {
        family_fingerprint,
        context_fingerprint: context.fingerprint_owner(),
        anchor,
        ordering,
        domain,
        columns,
        sources,
    })
}

fn find_shift_column(
    columns: &[OrderedShift],
    lookup: &[usize],
    shift: &IndexShift,
) -> Result<usize, ParametricRuleError> {
    let position = lookup
        .binary_search_by(|&column| columns[column].shift.cmp(shift))
        .map_err(|_| ParametricRuleError::ReducerInvariant {
            detail: "a prepared shift is absent from its column lookup",
        })?;
    Ok(lookup[position])
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

fn clone_condition_sources<'source>(
    sources: impl ExactSizeIterator<Item = &'source IdentityConditionSource>,
) -> Result<Box<[IdentityConditionSource]>, ParametricRuleError> {
    let mut retained = try_vec("parametric condition provenance", sources.len())?;
    for source in sources {
        retained.push(clone_condition_source(source)?);
    }
    Ok(retained.into_boxed_slice())
}

fn clone_condition_source(
    source: &IdentityConditionSource,
) -> Result<IdentityConditionSource, ParametricRuleError> {
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
                shift: copy_i64_slice(shift, "parametric condition source shift")?
                    .into_boxed_slice(),
            }
        }
        IdentityConditionSource::RelationCollectedTermDenominator { row, shift } => {
            IdentityConditionSource::RelationCollectedTermDenominator {
                row: row.clone(),
                shift: copy_i64_slice(shift, "parametric condition source shift")?
                    .into_boxed_slice(),
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
            offset: copy_i64_slice(offset, "parametric condition source offset")?
                .into_boxed_slice(),
        },
        IdentityConditionSource::IndexTranslation { offset } => {
            IdentityConditionSource::IndexTranslation {
                offset: copy_i64_slice(offset, "parametric condition source offset")?
                    .into_boxed_slice(),
            }
        }
    })
}

pub(super) fn copy_i64_slice(
    source: &[i64],
    resource: &'static str,
) -> Result<Vec<i64>, ParametricRuleError> {
    let mut retained = try_vec(resource, source.len())?;
    retained.extend_from_slice(source);
    Ok(retained)
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricRuleError> {
    left.checked_add(right)
        .ok_or(ParametricRuleError::ResourceCountOverflow { resource })
}

pub(super) fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricRuleError> {
    left.checked_mul(right)
        .ok_or(ParametricRuleError::ResourceCountOverflow { resource })
}

pub(super) fn check_cell_limit(
    resource: &'static str,
    containers: usize,
    cells_per_container: usize,
    limit: usize,
) -> Result<(), ParametricRuleError> {
    let requested = checked_mul(resource, containers, cells_per_container)?;
    check_limit(resource, requested, limit)
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ParametricRuleError> {
    if requested > limit {
        Err(ParametricRuleError::ResourceLimit {
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
) -> Result<Vec<T>, ParametricRuleError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| ParametricRuleError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}
