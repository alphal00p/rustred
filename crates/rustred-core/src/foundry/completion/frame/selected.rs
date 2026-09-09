//! Sparse selected-source construction over the common physical plan.

use std::cmp::Ordering;

use crate::identity::SelectedTranslatedSourceBatch;
use crate::sector::Mask;

use super::assemble::{
    OrderedTranslatedSources, SOURCE_INSTANCES, assemble_physical_plan, check_limit,
    total_translation_degree, try_vec,
};
use super::{PhysicalFrameError, PhysicalFrameLimits, SelectedSourceFrame};

impl SelectedSourceFrame {
    /// Consume one sealed sparse translated-source batch without completing
    /// the Cartesian product of its distinct offsets and ordinary sources.
    ///
    /// Rows use a deterministic physical chronology: L1 translation radius,
    /// sector-oriented signed offset lexicographic order, then stable source
    /// ordinal. The exact signed offset remains part of provenance identity;
    /// radius is scheduling metadata only. Translation limits have already
    /// been enforced by the selected-source generator, while this boundary
    /// applies the common physical-plan limits.
    pub(crate) fn try_new(
        selected: SelectedTranslatedSourceBatch,
        sector: Mask,
        limits: PhysicalFrameLimits,
    ) -> Result<Self, PhysicalFrameError> {
        Self::try_new_with_optional_request_order(selected, sector, None, limits)
    }

    /// Build the same exact selected-source frame while honoring one explicit
    /// physical row chronology.
    ///
    /// `request_order` must be an exact permutation of the batch's canonical
    /// request identity. It changes only elimination scheduling: the selected
    /// request owner remains canonical, and common physical assembly validates
    /// the resulting source permutation again.
    pub(crate) fn try_new_with_request_order(
        selected: SelectedTranslatedSourceBatch,
        sector: Mask,
        request_order: &[crate::identity::TranslatedSourceRequest],
        limits: PhysicalFrameLimits,
    ) -> Result<Self, PhysicalFrameError> {
        Self::try_new_with_optional_request_order(selected, sector, Some(request_order), limits)
    }

    fn try_new_with_optional_request_order(
        selected: SelectedTranslatedSourceBatch,
        sector: Mask,
        request_order: Option<&[crate::identity::TranslatedSourceRequest]>,
        limits: PhysicalFrameLimits,
    ) -> Result<Self, PhysicalFrameError> {
        if selected.is_empty() || selected.requests().len() != selected.sources().len() {
            return Err(PhysicalFrameError::Invariant {
                detail: "selected translated-source owner is empty or not request-complete",
            });
        }
        if selected.completed_source_row_count() == 0 {
            return Err(PhysicalFrameError::Invariant {
                detail: "selected translated-source owner has no source chronology",
            });
        }

        let arity = selected.requests()[0].offset().len();
        if sector.arity() != arity {
            return Err(PhysicalFrameError::WrongSectorArity {
                expected: arity,
                actual: sector.arity(),
            });
        }
        check_limit("physical-frame arity", arity, limits.max_arity)?;
        check_limit(
            SOURCE_INSTANCES,
            selected.len(),
            limits.max_source_instances,
        )?;

        if selected
            .requests()
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(PhysicalFrameError::Invariant {
                detail: "selected translated-source requests are not canonical and unique",
            });
        }

        let mut radii = try_vec("physical-frame selected-source radii", selected.len())?;
        for (source_index, (request, source)) in selected
            .requests()
            .iter()
            .zip(selected.sources())
            .enumerate()
        {
            if request.offset().len() != arity {
                return Err(PhysicalFrameError::WrongSourceOffsetArity {
                    row: source_index,
                    expected: arity,
                    actual: request.offset().len(),
                });
            }
            if request.source_ordinal() >= selected.completed_source_row_count() {
                return Err(PhysicalFrameError::Invariant {
                    detail: "selected source ordinal is outside its completed chronology",
                });
            }
            if source.provenance().source_ordinal() != request.source_ordinal()
                || source.provenance().offset() != request.offset()
            {
                return Err(PhysicalFrameError::Invariant {
                    detail: "selected request and translated-source provenance disagree",
                });
            }
            radii.push(total_translation_degree(request.offset())?);
        }

        let physical_source_indices = match request_order {
            Some(order) => try_explicit_source_order(&selected, order)?,
            None => {
                let mut order = try_vec(SOURCE_INSTANCES, selected.len())?;
                order.extend(0..selected.len());
                order.sort_unstable_by(|&left, &right| {
                    radii[left]
                        .cmp(&radii[right])
                        .then_with(|| {
                            sector_oriented_offset_order(
                                &sector,
                                selected.requests()[left].offset().values(),
                                selected.requests()[right].offset().values(),
                            )
                        })
                        .then_with(|| {
                            selected.requests()[left]
                                .source_ordinal()
                                .cmp(&selected.requests()[right].source_ordinal())
                        })
                });
                order
            }
        };

        let (
            family_fingerprint,
            context_fingerprint,
            completed_source_row_count,
            requests,
            sources,
        ) = selected.into_foundry_parts();
        debug_assert_eq!(requests.len(), sources.len());
        drop(requests);
        let plan = assemble_physical_plan(
            sector,
            OrderedTranslatedSources::new(
                family_fingerprint,
                context_fingerprint,
                sources,
                physical_source_indices,
            ),
            limits,
        )?;
        Ok(Self::from_parts(plan, completed_source_row_count))
    }
}

fn try_explicit_source_order(
    selected: &SelectedTranslatedSourceBatch,
    requested: &[crate::identity::TranslatedSourceRequest],
) -> Result<Vec<usize>, PhysicalFrameError> {
    if requested.len() != selected.len() {
        return Err(PhysicalFrameError::WrongExplicitSourceOrderLength {
            expected: selected.len(),
            actual: requested.len(),
        });
    }
    let mut first_positions = try_vec(SOURCE_INSTANCES, selected.len())?;
    first_positions.resize(selected.len(), None);
    let mut order = try_vec(SOURCE_INSTANCES, selected.len())?;
    for (position, request) in requested.iter().enumerate() {
        let source_index = selected
            .requests()
            .binary_search(request)
            .map_err(|_| PhysicalFrameError::ExplicitSourceOrderRequestAbsent { position })?;
        let first = first_positions
            .get_mut(source_index)
            .ok_or(PhysicalFrameError::Invariant {
                detail: "explicit source order escaped the selected request index",
            })?;
        if let Some(first_position) = *first {
            return Err(PhysicalFrameError::DuplicateExplicitSourceOrderRequest {
                first_position,
                second_position: position,
            });
        }
        *first = Some(position);
        order.push(source_index);
    }
    if first_positions.iter().any(Option::is_none) {
        return Err(PhysicalFrameError::Invariant {
            detail: "explicit source order omitted a canonical selected request",
        });
    }
    Ok(order)
}

fn sector_oriented_offset_order(sector: &Mask, left: &[i64], right: &[i64]) -> Ordering {
    debug_assert_eq!(left.len(), sector.arity());
    debug_assert_eq!(right.len(), sector.arity());
    left.iter()
        .zip(right)
        .zip(sector.active_bits())
        .find_map(|((&left, &right), &active)| {
            let order = if active {
                left.cmp(&right)
            } else {
                right.cmp(&left)
            };
            (order != Ordering::Equal).then_some(order)
        })
        .unwrap_or(Ordering::Equal)
}
