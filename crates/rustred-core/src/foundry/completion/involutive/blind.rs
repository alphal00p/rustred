use std::sync::Arc;

use crate::sector::ShiftComplexityKey;

use super::super::{LatticeBox, UncoveredPartition};
use super::error::{check_limit, checked_add, checked_mul, checked_sort_coordinate_work, try_vec};
use super::janet::JanetDivisionGeometry;
use super::{
    ForwardShift, InvolutiveError, InvolutiveLimits, JanetProlongation, OreActionIdentity,
    OreOrderingAdapter,
};

/// One deterministically ranked exact residual box.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BlindDomainEntry {
    source_ordinal: usize,
    lower: ForwardShift,
    upper: Arc<Vec<Option<u64>>>,
    free_dimension: usize,
    varying_dimension: usize,
    lower_key: ShiftComplexityKey,
}

impl BlindDomainEntry {
    pub(crate) fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub(crate) fn lower(&self) -> &ForwardShift {
        &self.lower
    }

    pub(crate) fn upper(&self) -> &[Option<u64>] {
        self.upper.as_slice()
    }

    pub(crate) fn free_dimension(&self) -> usize {
        self.free_dimension
    }

    pub(crate) fn varying_dimension(&self) -> usize {
        self.varying_dimension
    }

    /// Whether the upward orthant of `origin` meets this exact residual box.
    fn intersects_upward_orthant(&self, origin: &ForwardShift) -> bool {
        origin.arity() == self.lower.arity()
            && self
                .upper
                .iter()
                .zip(origin.values())
                .all(|(&upper, &coordinate)| upper.is_none_or(|upper| coordinate <= upper))
    }
}

/// Proposal-only blind-domain chronology derived from an exact complement.
///
/// Retention truncation is explicit. A truncated schedule still ranks every
/// prolongation and never filters an unobserved candidate, so a heuristic
/// prefix cannot be confused with proof that no blind domain intersects it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BlindDomainSchedule {
    action: OreActionIdentity,
    arity: usize,
    total_box_count: usize,
    entries: Box<[BlindDomainEntry]>,
    truncated: bool,
}

impl BlindDomainSchedule {
    pub(crate) fn try_from_partition(
        partition: &UncoveredPartition,
        ordering: &OreOrderingAdapter,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        let total_box_count = partition.boxes().len();
        check_limit(
            "blind-domain boxes scanned",
            total_box_count,
            limits.max_blind_boxes_scanned,
        )?;
        let coordinate_cells = checked_mul(
            "blind-domain endpoint cells",
            total_box_count,
            ordering
                .arity()
                .checked_mul(2)
                .ok_or(InvolutiveError::ResourceCountOverflow {
                    resource: "blind-domain endpoint cells",
                })?,
        )?;
        check_limit(
            "blind-domain endpoint cells",
            coordinate_cells,
            limits.max_blind_coordinate_cells,
        )?;

        let retained_limit = total_box_count.min(limits.max_blind_boxes_retained);
        let mut entries = try_vec("blind-domain priority entries", retained_limit)?;
        for (source_ordinal, lattice_box) in partition.boxes().iter().enumerate() {
            require_box_arity(lattice_box, ordering.arity())?;
            if retained_limit == 0 {
                continue;
            }
            let lower = ForwardShift::try_new(lattice_box.lower().iter().copied(), limits)?;
            let lower_key = ordering.try_key(&lower)?;
            let mut upper = try_vec("blind-domain upper endpoints", ordering.arity())?;
            upper.extend_from_slice(lattice_box.upper());
            let entry = BlindDomainEntry {
                source_ordinal,
                lower,
                upper: Arc::new(upper),
                free_dimension: lattice_box.free_dimension(),
                varying_dimension: lattice_box.varying_dimension(),
                lower_key,
            };
            let position = entries
                .binary_search_by(|candidate| compare_entries(candidate, &entry))
                .unwrap_or_else(|position| position);
            if entries.len() < retained_limit {
                entries.insert(position, entry);
            } else if position < retained_limit {
                entries.pop();
                entries.insert(position, entry);
            }
        }
        let retained = entries.len();
        Ok(Self {
            action: ordering.identity().clone(),
            arity: ordering.arity(),
            total_box_count,
            truncated: retained < total_box_count,
            entries: entries.into_boxed_slice(),
        })
    }

    pub(crate) fn total_box_count(&self) -> usize {
        self.total_box_count
    }

    pub(crate) fn entries(&self) -> &[BlindDomainEntry] {
        &self.entries
    }

    pub(crate) fn is_truncated(&self) -> bool {
        self.truncated
    }

    /// True only when every exact residual box participated in the heuristic
    /// order. This remains diagnostic and grants no admission authority.
    pub(crate) fn has_complete_priority_view(&self) -> bool {
        !self.truncated
    }

    /// Return every input ordinal in deterministic blind-first order.
    /// Candidates missed by a retained/truncated box prefix remain present.
    pub(super) fn try_rank_prolongation_ordinals(
        &self,
        division: &(impl JanetDivisionGeometry + ?Sized),
        prolongations: &[JanetProlongation],
        ordering: &OreOrderingAdapter,
        limits: InvolutiveLimits,
    ) -> Result<Box<[usize]>, InvolutiveError> {
        ordering.require_action(&self.action)?;
        division.require_geometry_ordering(ordering)?;
        ordering.require_arity("blind-domain schedule", self.arity)?;
        for prolongation in prolongations {
            division.require_geometry_prolongation(prolongation, ordering)?;
        }
        check_limit(
            "blind-domain priority candidates",
            prolongations.len(),
            limits.max_priority_candidates,
        )?;
        let intersection_cells = checked_mul(
            "blind-domain priority intersection cells",
            checked_mul(
                "blind-domain priority intersection cells",
                prolongations.len(),
                self.entries.len(),
            )?,
            self.arity,
        )?;
        check_limit(
            "blind-domain priority intersection cells",
            intersection_cells,
            limits.max_blind_priority_intersection_cells,
        )?;
        let sort_work = checked_sort_coordinate_work(
            "blind-domain priority sort coordinate comparisons",
            prolongations.len(),
            self.arity,
        )?;
        check_limit(
            "blind-domain priority sort coordinate comparisons",
            sort_work,
            limits.max_blind_priority_sort_coordinate_comparisons,
        )?;
        let candidate_bytes = checked_mul(
            "blind-domain priority retained bytes",
            prolongations.len(),
            std::mem::size_of::<RankedProlongation>(),
        )?;
        let ordinal_bytes = checked_mul(
            "blind-domain priority retained bytes",
            prolongations.len(),
            std::mem::size_of::<usize>(),
        )?;
        let retained_bytes = checked_add(
            "blind-domain priority retained bytes",
            candidate_bytes,
            ordinal_bytes,
        )?;
        check_limit(
            "blind-domain priority retained bytes",
            retained_bytes,
            limits.max_blind_priority_retained_bytes,
        )?;
        let mut ranked = try_vec("blind-domain prioritized candidates", prolongations.len())?;
        for (ordinal, prolongation) in prolongations.iter().enumerate() {
            let first_intersection = self.entries.iter().position(|entry| {
                entry.intersects_upward_orthant(prolongation.target_leading_shift())
            });
            ranked.push(RankedProlongation {
                ordinal,
                first_intersection,
                target_key: prolongation.target_key().clone(),
                basis_ordinal: prolongation.basis_ordinal(),
                variable: prolongation.variable(),
            });
        }
        ranked.sort_unstable_by(|left, right| {
            compare_optional_rank(left.first_intersection, right.first_intersection)
                .then_with(|| left.target_key.cmp(&right.target_key))
                .then_with(|| left.basis_ordinal.cmp(&right.basis_ordinal))
                .then_with(|| left.variable.cmp(&right.variable))
                .then_with(|| left.ordinal.cmp(&right.ordinal))
        });
        let mut ordinals = try_vec("blind-domain ranked ordinals", ranked.len())?;
        ordinals.extend(ranked.into_iter().map(|candidate| candidate.ordinal));
        Ok(ordinals.into_boxed_slice())
    }
}

fn compare_entries(left: &BlindDomainEntry, right: &BlindDomainEntry) -> std::cmp::Ordering {
    right
        .free_dimension
        .cmp(&left.free_dimension)
        .then_with(|| right.varying_dimension.cmp(&left.varying_dimension))
        .then_with(|| left.lower_key.cmp(&right.lower_key))
        .then_with(|| left.lower.cmp(&right.lower))
        .then_with(|| left.upper.cmp(&right.upper))
        .then_with(|| left.source_ordinal.cmp(&right.source_ordinal))
}

struct RankedProlongation {
    ordinal: usize,
    first_intersection: Option<usize>,
    target_key: ShiftComplexityKey,
    basis_ordinal: usize,
    variable: usize,
}

fn compare_optional_rank(left: Option<usize>, right: Option<usize>) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn require_box_arity(lattice_box: &LatticeBox, expected: usize) -> Result<(), InvolutiveError> {
    if lattice_box.arity() == expected {
        Ok(())
    } else {
        Err(InvolutiveError::WrongArity {
            object: "blind-domain box",
            expected,
            actual: lattice_box.arity(),
        })
    }
}
