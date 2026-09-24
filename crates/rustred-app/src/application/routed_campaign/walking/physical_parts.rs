//! Exact physical source partitions; neither part is a queue obligation.
use std::sync::Arc;

use rustred::solver::OwnerAppliedLimits;
use serde::{Deserialize, Serialize};

use super::queue::{Domain, Phase};

/// Typed coordinator identity encoded into the existing opaque pool handle.
/// Whole=0, first=1, second=2 preserves parent then part publication order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Ticket {
    pub parent: usize,
    pub part: Option<u8>,
}
impl Ticket {
    pub fn encode(self, enabled: bool) -> Result<usize, &'static str> {
        if !enabled {
            return Ok(self.parent);
        }
        self.parent
            .checked_mul(3)
            .and_then(|id| id.checked_add(self.part.map_or(0, |part| usize::from(part) + 1)))
            .ok_or("physical inspection ticket overflow")
    }
    pub fn decode(id: usize, enabled: bool) -> Self {
        if enabled {
            Self {
                parent: id / 3,
                part: (id % 3 != 0).then(|| (id % 3 - 1) as u8),
            }
        } else {
            Self {
                parent: id,
                part: None,
            }
        }
    }
}

/// The completed prefix of a subdivided logical parent. The current part's
/// admitted stream/provenance is retained by the ordinary coordinator state.
#[derive(Default, Serialize, Deserialize)]
pub(super) struct Progress {
    pub parent: usize,
    pub completed: Vec<serde_json::Value>,
}

/// Read genuine per-call report statistics and checked-sum them for the one
/// logical parent. This is explicitly aggregate work, not a synthetic call.
pub(super) fn sum_stats(
    parts: &[serde_json::Value],
) -> Result<rustred::solver::OwnerAppliedStats, String> {
    let mut sum = rustred::solver::OwnerAppliedStats::default();
    for part in parts {
        let stats = &part["stats"];
        macro_rules! add {
            ($($field:ident),* $(,)?) => { $(
                let value = stats[stringify!($field)].as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or(concat!("invalid physical statistic ", stringify!($field)))?;
                sum.$field = sum.$field.checked_add(value).ok_or("physical statistics overflow")?;
            )* };
        }
        add!(
            selected_pieces,
            term_visits,
            shift_groups,
            boundary_cells,
            sign_splits,
            native_operations,
            optional_coefficient_refusals,
            optional_original_refusals,
            optional_coalesced_refusals,
            coalescing_additions,
            events,
            successors,
            conditional_successors,
            same_support_successors,
            strict_subsupport_successors,
            unsupported_support_successors,
            conditional_unsupported_support_successors,
            problems,
            zero_terms,
            cancelled_groups,
            zero_sector_groups,
            correlation_empty_cells
        );
        macro_rules! matching {
            ($($field:ident),* $(,)?) => { $(
                let value = stats["matching"][stringify!($field)].as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or(concat!("invalid physical matching statistic ", stringify!($field)))?;
                sum.matching.$field = sum.matching.$field.checked_add(value)
                    .ok_or("physical matching statistics overflow")?;
            )* };
        }
        matching!(
            rules,
            terminal_checks,
            predicates,
            pieces,
            cells,
            split_operations,
            coordinate_cells,
            rank_empty_cells,
            correlation_empty_cells,
            refinement_cells,
            refinement_steps
        );
    }
    Ok(sum)
}

/// Opt-in deterministic source subdivision. Coordinates are the native local
/// coordinates, independent of topology, owner names and physical loop count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplySubdivision {
    pub axis: usize,
    pub cut: u64,
}

impl ApplySubdivision {
    pub(super) fn parts<const N: usize>(self, domain: &Domain<N>) -> Option<[Arc<Domain<N>>; 2]> {
        if domain.phase != Phase::Apply || self.axis >= N {
            return None;
        }
        let upper = domain.upper[self.axis]?;
        if domain.lower[self.axis] > self.cut || self.cut >= upper {
            return None;
        }
        let next = self.cut.checked_add(1)?;
        let mut first = domain.clone();
        let mut second = domain.clone();
        first.upper[self.axis] = Some(self.cut);
        second.lower[self.axis] = next;
        Some([Arc::new(first), Arc::new(second)])
    }
}

/// Prospective shares of finite cumulative work. usize::MAX is the existing
/// caller convention for no practical work limit, not a finite share to halve.
/// Scratch and per-operation algebra admission remain unchanged.
pub(super) fn limits(parent: OwnerAppliedLimits, part: u8) -> OwnerAppliedLimits {
    assert!(part < 2);
    let share = |value: usize| {
        if value == usize::MAX {
            value
        } else if part == 0 {
            value / 2
        } else {
            value - value / 2
        }
    };
    let mut out = parent;
    macro_rules! share {
        ($($field:ident),* $(,)?) => { $(out.$field = share(parent.$field);)* };
    }
    share!(
        max_term_visits,
        max_shift_groups,
        max_boundary_cells,
        max_sign_splits,
        max_native_operations,
        max_events
    );
    macro_rules! matching {
        ($($field:ident),* $(,)?) => {
            $(out.matching.$field = share(parent.matching.$field);)*
        };
    }
    matching!(
        max_rules,
        max_terminal_checks,
        max_predicates,
        max_pieces,
        max_cells,
        max_split_operations,
        max_coordinate_cells,
        max_bounded_refinement_cells
    );
    out
}

pub(super) fn limits_json(limits: OwnerAppliedLimits) -> serde_json::Value {
    let m = limits.matching;
    serde_json::json!({
        "policy":"fixed_half_remainder_for_finite_cumulative_limits; usize_max_unlimited_preserved",
        "max_term_visits":limits.max_term_visits,"max_shift_groups":limits.max_shift_groups,
        "max_boundary_cells":limits.max_boundary_cells,"max_sign_splits":limits.max_sign_splits,
        "max_native_operations":limits.max_native_operations,"max_events":limits.max_events,
        "max_scratch_terms":limits.max_scratch_terms,"max_scratch_boxes":limits.max_scratch_boxes,
        "max_scratch_coordinate_cells":limits.max_scratch_coordinate_cells,
        "matching":{"max_rules":m.max_rules,"max_terminal_checks":m.max_terminal_checks,
            "max_predicates":m.max_predicates,"max_pieces":m.max_pieces,"max_cells":m.max_cells,
            "max_split_operations":m.max_split_operations,"max_coordinate_cells":m.max_coordinate_cells,
            "max_bounded_refinement_cells":m.max_bounded_refinement_cells},
        "guard_algebra_and_refinement_policy":"unchanged_from_parent",
        "memory_scope":"both_parts_share_process_allowance; scratch_limits_remain_per_call"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cut_preserves_every_non_axis_constraint_and_exact_union() {
        let parent = Domain {
            phase: Phase::Apply,
            owner: [false, true],
            lower: vec![2, 0],
            upper: vec![Some(8), Some(4)],
            rank: Some(9),
            powers: rustred::solver::DomainPowerBounds {
                max_positive_power: Some(5),
                min_power_difference: Some(-3),
                max_power_difference: Some(4),
            },
        };
        let parts = ApplySubdivision { axis: 0, cut: 4 }.parts(&parent).unwrap();
        for x in 0..=10 {
            let count = parts
                .iter()
                .filter(|p| p.lower[0] <= x && x <= p.upper[0].unwrap())
                .count();
            assert_eq!(count, usize::from((2..=8).contains(&x)));
        }
        for part in parts {
            assert_eq!(part.owner, parent.owner);
            assert_eq!(part.rank, parent.rank);
            assert_eq!(part.powers, parent.powers);
            assert_eq!(part.lower[1], parent.lower[1]);
            assert_eq!(part.upper[1], parent.upper[1]);
        }
        for cut in [0, 1, 8, u64::MAX] {
            assert!(ApplySubdivision { axis: 0, cut }.parts(&parent).is_none());
        }
        assert!(
            ApplySubdivision { axis: 2, cut: 4 }
                .parts(&parent)
                .is_none()
        );
    }

    #[test]
    fn finite_shares_do_not_multiply_work_or_reduce_unlimited_and_scratch() {
        let mut parent = OwnerAppliedLimits::default();
        parent.max_events = 7;
        parent.max_native_operations = usize::MAX;
        parent.matching.max_rules = 9;
        parent.matching.max_predicates = usize::MAX;
        let a = limits(parent, 0);
        let b = limits(parent, 1);
        assert_eq!((a.max_events, b.max_events), (3, 4));
        assert_eq!((a.matching.max_rules, b.matching.max_rules), (4, 5));
        for part in [a, b] {
            assert_eq!(part.max_native_operations, usize::MAX);
            assert_eq!(part.matching.max_predicates, usize::MAX);
            assert_eq!(part.max_scratch_terms, parent.max_scratch_terms);
            assert_eq!(part.max_scratch_boxes, parent.max_scratch_boxes);
            assert_eq!(
                part.max_scratch_coordinate_cells,
                parent.max_scratch_coordinate_cells
            );
        }
    }
}
