//! Compare the prior double normalization with the exact no-crossing shortcut.
//! The reference calls the existing native service; no alternate geometry CAS.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy)]
enum CancelAt {
    Never,
    BeforeFirstBoundary,
    AfterFirstBoundary,
}

#[derive(Clone)]
struct Input<const N: usize> {
    owner: [bool; N],
    shift: [i64; N],
    lower: [u64; N],
    upper: [Option<u64>; N],
    rank: Option<u32>,
    powers: DomainPowerBounds,
}

#[derive(Debug, PartialEq, Eq)]
struct Cell<const N: usize> {
    lower: Vec<u64>,
    upper: Vec<Option<u64>>,
    rank: Option<u32>,
    fixed: Result<Vec<(usize, i64)>, usize>,
    image: Result<([bool; N], Vec<u64>, Vec<Option<u64>>, Option<u32>, i128), &'static str>,
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome<const N: usize> {
    cells: Vec<Cell<N>>,
    failure: Option<OwnerAppliedFailure>,
    stats: OwnerAppliedStats,
}

fn run<const N: usize>(
    input: &Input<N>,
    reuse: bool,
    limits: OwnerAppliedLimits,
    cancel_at: CancelAt,
    previous_boundaries: usize,
) -> (Outcome<N>, usize) {
    let cancel = AtomicBool::new(false);
    let mut budget = Budget {
        limits,
        stats: OwnerAppliedStats {
            boundary_cells: previous_boundaries,
            ..Default::default()
        },
        cancel: &cancel,
    };
    let mut cells = Vec::new();
    let mut second_normalizations = 0;
    let result = (|| {
        let source = copy_box(&input.lower, &input.upper)?;
        for sign in sign_cells(&source, &input.owner, &input.shift, &mut budget)? {
            let Some((sign, sign_rank)) =
                normalize(sign, &input.owner, input.rank, input.powers, &mut budget)?
            else {
                continue;
            };
            let mut boundary = Boundaries::new(&sign, &input.owner, &input.shift, &budget)?;
            let has_crossings = boundary.has_crossings();
            if matches!(cancel_at, CancelAt::BeforeFirstBoundary) {
                cancel.store(true, Ordering::Release);
            }
            while let Some(cell) = boundary.next(&mut budget)? {
                let normalized = if reuse && !has_crossings {
                    // Exactly the optimized engine branch: next still executes.
                    Some((cell, sign_rank))
                } else {
                    second_normalizations += 1;
                    normalize(cell, &input.owner, sign_rank, input.powers, &mut budget)?
                };
                if let Some((cell, rank)) = normalized {
                    cells.push(Cell {
                        lower: cell.lower().to_vec(),
                        upper: cell.upper().to_vec(),
                        rank,
                        fixed: fixed(&cell, &input.owner, rank),
                        image: image(&cell, &input.owner, &input.shift, rank).map(|image| {
                            (
                                image.sector,
                                image.lower,
                                image.upper,
                                image.rank,
                                image.delta_rank,
                            )
                        }),
                    });
                }
                if matches!(cancel_at, CancelAt::AfterFirstBoundary) {
                    cancel.store(true, Ordering::Release);
                }
            }
        }
        Ok::<_, OwnerAppliedFailure>(())
    })();
    (
        Outcome {
            cells,
            failure: result.err(),
            stats: budget.stats,
        },
        second_normalizations,
    )
}

fn compare<const N: usize>(input: &Input<N>) -> (Outcome<N>, usize) {
    let (reference, old_count) = run(input, false, Default::default(), CancelAt::Never, 0);
    let (actual, new_count) = run(input, true, Default::default(), CancelAt::Never, 0);
    assert_eq!(actual, reference);
    assert!(new_count <= old_count);
    (actual, old_count - new_count)
}

#[test]
fn no_crossing_normalization_reuse_is_idempotent_with_correlated_rank_and_power_bounds() {
    let mut exercised = 0;
    for mask in 0..8 {
        let owner = [mask & 1 != 0, mask & 2 != 0, mask & 4 != 0];
        let shift = owner.map(|active| if active { 1 } else { -1 });
        for lower in [[0; 3], [0, 1, 2], [2, 0, 1]] {
            for upper in [[None; 3], [Some(2); 3], [Some(3), None, Some(4)]] {
                for rank in [None, Some(0), Some(2), Some(7)] {
                    for powers in [
                        DomainPowerBounds::default(),
                        DomainPowerBounds {
                            max_positive_power: Some(7),
                            min_power_difference: Some(-1),
                            max_power_difference: Some(3),
                        },
                        DomainPowerBounds {
                            max_positive_power: Some(5),
                            min_power_difference: Some(2),
                            max_power_difference: None,
                        },
                        DomainPowerBounds {
                            max_positive_power: None,
                            min_power_difference: Some(1),
                            max_power_difference: Some(1),
                        },
                    ] {
                        let (outcome, saved) = compare(&Input {
                            owner,
                            shift,
                            lower,
                            upper,
                            rank,
                            powers,
                        });
                        assert!(outcome.failure.is_none());
                        assert_eq!(saved, outcome.cells.len());
                        exercised += saved;
                    }
                }
            }
        }
    }
    assert!(
        exercised > 100,
        "the idempotence test must include nonempty domains"
    );
}

#[test]
fn unbounded_and_above_u32_implied_ranks_are_not_clipped_by_reuse() {
    let high = u64::from(u32::MAX) + 1;
    let input = Input {
        owner: [true, false],
        shift: [1, -1],
        lower: [0; 2],
        upper: [Some(0), None],
        rank: None,
        powers: DomainPowerBounds {
            min_power_difference: Some(1 - high as i64),
            ..Default::default()
        },
    };
    let (outcome, saved) = compare(&input);
    assert_eq!(saved, 1);
    assert_eq!(outcome.cells[0].rank, None);
    assert_eq!(outcome.cells[0].upper[1], Some(high));
    let (unbounded, saved) = compare(&Input {
        powers: DomainPowerBounds::default(),
        ..input
    });
    assert_eq!(saved, 1);
    assert_eq!(unbounded.cells[0].rank, None);
    assert_eq!(unbounded.cells[0].upper[1], None);
}

#[test]
fn no_crossing_effective_rank_preserves_forced_specialization_and_near_empty_cells() {
    let input = Input {
        owner: [true, false, false],
        shift: [1, -1, -1],
        lower: [0, 1, 2],
        upper: [None; 3],
        rank: None,
        powers: DomainPowerBounds {
            max_positive_power: Some(4),
            min_power_difference: Some(1),
            max_power_difference: None,
        },
    };
    let (outcome, saved) = compare(&input);
    assert_eq!(saved, 1);
    assert_eq!(outcome.cells[0].rank, Some(3));
    assert_eq!(outcome.cells[0].fixed, Ok(vec![(0, 4), (1, -1), (2, -2)]));
    let (empty, saved) = compare(&Input {
        powers: DomainPowerBounds {
            max_positive_power: Some(3),
            ..input.powers
        },
        ..input
    });
    assert_eq!(saved, 0);
    assert!(empty.cells.is_empty());
    assert_eq!(empty.stats.correlation_empty_cells, 1);
}

#[test]
fn actual_crossings_including_fixed_crossings_keep_legacy_face_normalization() {
    let mut tested = 0;
    for lower in [[0, 0], [1, 1]] {
        let upper = [Some(lower[0]), Some(lower[1])];
        let input = Input {
            owner: [true, false],
            shift: [-2, 2],
            lower,
            upper,
            rank: None,
            powers: DomainPowerBounds {
                max_positive_power: Some(5),
                min_power_difference: Some(-1),
                max_power_difference: Some(3),
            },
        };
        let (outcome, saved) = compare(&input);
        assert_eq!(
            saved, 0,
            "one-element crossing products must not use the shortcut"
        );
        assert_eq!(outcome.cells.len(), 1);
        tested += outcome.stats.boundary_cells;
    }
    let (outcome, saved) = compare(&Input {
        owner: [true, false],
        shift: [-3, 3],
        lower: [0; 2],
        upper: [Some(3); 2],
        rank: Some(3),
        powers: DomainPowerBounds {
            max_positive_power: Some(4),
            min_power_difference: Some(-1),
            max_power_difference: Some(2),
        },
    });
    assert!(outcome.failure.is_none());
    assert!(outcome.stats.boundary_cells > 1 && outcome.stats.sign_splits > 0);
    assert!(saved <= 1); // Only the single no-crossing sign region can reuse.
    assert_eq!(tested, 2);
}

#[test]
fn normalization_reuse_preserves_scratch_boundary_cancel_and_overflow_prefixes() {
    let defaults = OwnerAppliedLimits::default();
    let input = Input {
        owner: [true, false],
        shift: [1, -1],
        lower: [0; 2],
        upper: [Some(2); 2],
        rank: None,
        powers: DomainPowerBounds {
            max_positive_power: Some(3),
            min_power_difference: Some(0),
            max_power_difference: None,
        },
    };
    for shift in [[1, -1], [-3, 3]] {
        let input = Input {
            shift,
            ..input.clone()
        };
        for limits in [
            defaults,
            OwnerAppliedLimits {
                max_boundary_cells: 0,
                ..defaults
            },
            OwnerAppliedLimits {
                max_boundary_cells: 1,
                ..defaults
            },
            OwnerAppliedLimits {
                max_scratch_boxes: 12,
                ..defaults
            },
            OwnerAppliedLimits {
                max_scratch_coordinate_cells: 1,
                ..defaults
            },
        ] {
            for cancel in [
                CancelAt::Never,
                CancelAt::BeforeFirstBoundary,
                CancelAt::AfterFirstBoundary,
            ] {
                for previous in [0, usize::MAX] {
                    assert_eq!(
                        run(&input, true, limits, cancel, previous).0,
                        run(&input, false, limits, cancel, previous).0
                    );
                }
            }
        }
    }
    let (cancelled, _) = run(&input, true, defaults, CancelAt::BeforeFirstBoundary, 0);
    assert_eq!(cancelled.failure, Some(OwnerAppliedFailure::Cancelled));
    assert_eq!(cancelled.stats.boundary_cells, 0);
    let crossing = Input {
        shift: [-3, 3],
        ..input
    };
    let (partial, _) = run(&crossing, true, defaults, CancelAt::AfterFirstBoundary, 0);
    assert_eq!(partial.failure, Some(OwnerAppliedFailure::Cancelled));
    assert_eq!(partial.stats.boundary_cells, 1);
}
