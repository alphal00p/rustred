use std::collections::BTreeSet;
use std::sync::Arc;

use crate::foundry::artifact::derive_two_loop_unit_mass_sunset;
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, StratumRegistryLimits,
};
use crate::identity::{IntegralShift, TranslatedSourceRequest};
use crate::sector::{
    CoordinatePriority, CoordinatePriorityLimits, Error as SectorError, InteriorBounds, Mask,
    OrderingPolicy, SectorMonotoneDomain,
};

use super::{
    SignedL1DepthShell, SignedL1ScheduleLimits, SignedL1ShellScheduler, SpiredAffineLatticeCase,
    SpiredCase, SpiredCoordinateFace, SpiredExecutionCase, SpiredFoundationError,
};

fn coordinate_stratum_for(family: &str, context: &str) -> DecoratedStratum {
    let sector = Mask::try_new([true, false, true]).unwrap();
    let domain = SectorMonotoneDomain::try_new_for_rule(
        sector,
        [
            InteriorBounds::new(1, 12),
            InteriorBounds::new(-2, -2),
            InteriorBounds::new(3, 9),
        ],
        &[0, 0, 0],
        &[] as &[&[i64]],
    )
    .unwrap();
    DecoratedStratum::try_guard_blind(family, context, domain, StratumRegistryLimits::default())
        .unwrap()
}

fn coordinate_stratum() -> DecoratedStratum {
    coordinate_stratum_for("spired-test-family", "spired-test-context")
}

fn target(values: &[i64]) -> IntegralShift {
    IntegralShift::try_new(values.iter().copied()).unwrap()
}

fn empty_owners(family: &str, context: &str, arity: usize) -> ImmutableOwnerSnapshot {
    ImmutableOwnerSnapshot::try_empty(family, context, arity, StratumRegistryLimits::default())
        .unwrap()
}

fn collect_shell(mut shell: SignedL1DepthShell, chunk_size: usize) -> Vec<TranslatedSourceRequest> {
    let expected = shell.request_count();
    let mut requests = Vec::new();
    while let Some(chunk) = shell.try_next_chunk(chunk_size).unwrap() {
        assert!(!chunk.is_empty());
        assert!(chunk.len() <= chunk_size);
        assert_eq!(chunk.first_request_ordinal(), requests.len());
        requests.extend_from_slice(chunk.requests());
        assert_eq!(shell.emitted_request_count(), requests.len());
        assert_eq!(shell.remaining_request_count(), expected - requests.len());
    }
    assert!(shell.is_exhausted());
    assert_eq!(requests.len(), expected);
    assert!(shell.try_next_chunk(chunk_size).unwrap().is_none());
    requests
}

fn distinct_offsets(
    requests: &[TranslatedSourceRequest],
    source_row_count: usize,
) -> Vec<Vec<i64>> {
    requests
        .chunks_exact(source_row_count)
        .map(|source_block| source_block[0].offset().values().to_vec())
        .collect()
}

fn brute_force_signed_l1_offsets(arity: usize, depth: usize) -> BTreeSet<Vec<i64>> {
    let width = 2 * depth + 1;
    let point_count = width.pow(u32::try_from(arity).unwrap());
    let signed_depth = i64::try_from(depth).unwrap();
    let mut offsets = BTreeSet::new();
    for mut ordinal in 0..point_count {
        let mut offset = vec![0_i64; arity];
        for coordinate in offset.iter_mut().rev() {
            *coordinate = i64::try_from(ordinal % width).unwrap() - signed_depth;
            ordinal /= width;
        }
        if offset
            .iter()
            .map(|coordinate| coordinate.unsigned_abs())
            .sum::<u64>()
            == u64::try_from(depth).unwrap()
        {
            offsets.insert(offset);
        }
    }
    offsets
}

#[test]
fn authenticated_coordinate_face_retains_full_execution_identity() {
    let stratum = coordinate_stratum();
    let owners = empty_owners("spired-test-family", "spired-test-context", 3);
    let target = target(&[0, 0, 0]);
    let execution = SpiredExecutionCase::try_new(
        SpiredCase::CoordinateFace(SpiredCoordinateFace::new(stratum.clone())),
        target.clone(),
        OrderingPolicy::default(),
        owners.clone(),
    )
    .unwrap();

    assert_eq!(execution.stratum(), &stratum);
    assert_eq!(execution.coordinate_face().stratum(), &stratum);
    assert_eq!(execution.arity(), 3);
    assert_eq!(execution.target_shift(), &target);
    assert_eq!(execution.ordering(), OrderingPolicy::default());
    assert!(execution.owner_snapshot().same_authority_as(&owners));
    assert_eq!(
        execution
            .stratum()
            .singleton_index_assignments()
            .collect::<Vec<_>>(),
        vec![(1, -2)]
    );
}

#[test]
fn affine_lattice_requests_fail_closed_at_the_execution_boundary() {
    let request = SpiredCase::AffineLattice(SpiredAffineLatticeCase::new(6));
    let owners = empty_owners("spired-test-family", "spired-test-context", 3);
    assert_eq!(
        SpiredExecutionCase::try_new(
            request,
            target(&[0, 0, 0]),
            OrderingPolicy::default(),
            owners,
        )
        .unwrap_err(),
        SpiredFoundationError::UnsupportedAffineLattice { ambient_arity: 6 }
    );
}

#[test]
fn coordinate_execution_joins_target_ordering_and_owner_scope() {
    let stratum = coordinate_stratum();
    let matching = || {
        empty_owners(
            stratum.family_fingerprint(),
            stratum.context_fingerprint(),
            3,
        )
    };
    let case = || SpiredCase::CoordinateFace(SpiredCoordinateFace::new(stratum.clone()));

    assert_eq!(
        SpiredExecutionCase::try_new(
            case(),
            target(&[0, 0]),
            OrderingPolicy::default(),
            matching(),
        )
        .unwrap_err(),
        SpiredFoundationError::WrongTargetArity {
            expected: 3,
            actual: 2,
        }
    );
    assert_eq!(
        SpiredExecutionCase::try_new(
            case(),
            target(&[0, 0, 0]),
            OrderingPolicy::default(),
            empty_owners(
                stratum.family_fingerprint(),
                stratum.context_fingerprint(),
                2,
            ),
        )
        .unwrap_err(),
        SpiredFoundationError::WrongOwnerArity {
            expected: 3,
            actual: 2,
        }
    );
    assert_eq!(
        SpiredExecutionCase::try_new(
            case(),
            target(&[0, 0, 0]),
            OrderingPolicy::default(),
            empty_owners("foreign-family", stratum.context_fingerprint(), 3),
        )
        .unwrap_err(),
        SpiredFoundationError::WrongOwnerFamily
    );
    assert_eq!(
        SpiredExecutionCase::try_new(
            case(),
            target(&[0, 0, 0]),
            OrderingPolicy::default(),
            empty_owners(stratum.family_fingerprint(), "foreign-context", 3),
        )
        .unwrap_err(),
        SpiredFoundationError::WrongOwnerContext
    );

    let priority =
        CoordinatePriority::try_new(2, &[1, 0], CoordinatePriorityLimits::default()).unwrap();
    let wrong_arity_ordering = OrderingPolicy::try_with_coordinate_priority(&priority).unwrap();
    assert_eq!(
        SpiredExecutionCase::try_new(case(), target(&[0, 0, 0]), wrong_arity_ordering, matching(),)
            .unwrap_err(),
        SpiredFoundationError::WrongOrderingArity {
            expected: 3,
            actual: 2,
        }
    );

    assert_eq!(
        SpiredExecutionCase::try_new(
            case(),
            target(&[-1, 0, 0]),
            OrderingPolicy::default(),
            matching(),
        )
        .unwrap_err(),
        SpiredFoundationError::InvalidTargetShift(SectorError::PivotLeavesParentSector {
            position: 0,
            shift: -1,
        })
    );
}

#[test]
fn custom_ordering_is_retained_and_owner_ordering_mismatch_is_rejected() {
    let stratum = coordinate_stratum();
    let priority =
        CoordinatePriority::try_new(3, &[2, 0, 1], CoordinatePriorityLimits::default()).unwrap();
    let ordering = OrderingPolicy::try_with_coordinate_priority(&priority).unwrap();
    let execution = SpiredExecutionCase::try_new(
        SpiredCase::CoordinateFace(SpiredCoordinateFace::new(stratum.clone())),
        target(&[0, 0, 0]),
        ordering,
        empty_owners(
            stratum.family_fingerprint(),
            stratum.context_fingerprint(),
            3,
        ),
    )
    .unwrap();
    assert_eq!(execution.ordering(), ordering);

    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let owners = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(&artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    assert_eq!(
        owners.canonicalizer_ordering(),
        Some(OrderingPolicy::default())
    );
    let stratum = coordinate_stratum_for(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
    );
    assert_eq!(
        SpiredExecutionCase::try_new(
            SpiredCase::CoordinateFace(SpiredCoordinateFace::new(stratum)),
            target(&[0, 0, 0]),
            OrderingPolicy::TestOnlyDistinct,
            owners,
        )
        .unwrap_err(),
        SpiredFoundationError::OwnerOrderingMismatch {
            requested: OrderingPolicy::TestOnlyDistinct,
            snapshot: OrderingPolicy::default(),
        }
    );
}

#[test]
fn signed_l1_shells_are_complete_lexicographic_and_source_fair() {
    let scheduler =
        SignedL1ShellScheduler::try_new(2, 3, SignedL1ScheduleLimits::default()).unwrap();
    assert_eq!(scheduler.arity(), 2);
    assert_eq!(scheduler.source_row_count(), 3);

    let shell = scheduler.try_depth_shell(2).unwrap();
    assert_eq!(shell.depth(), 2);
    assert_eq!(shell.offset_count(), 8);
    assert_eq!(shell.request_count(), 24);
    let requests = collect_shell(shell, 5);
    assert_eq!(
        distinct_offsets(&requests, 3),
        vec![
            vec![-2, 0],
            vec![-1, -1],
            vec![-1, 1],
            vec![0, -2],
            vec![0, 2],
            vec![1, -1],
            vec![1, 1],
            vec![2, 0],
        ]
    );
    assert!(requests.chunks_exact(3).all(|source_block| {
        source_block
            .iter()
            .map(|request| request.source_ordinal())
            .eq(0..3)
    }));
    for source_ordinal in 0..3 {
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.source_ordinal() == source_ordinal)
                .count(),
            8
        );
    }
}

#[test]
fn depth_zero_and_successive_shells_do_not_starve_any_source() {
    let scheduler =
        SignedL1ShellScheduler::try_new(3, 2, SignedL1ScheduleLimits::default()).unwrap();
    let zero = collect_shell(scheduler.try_depth_shell(0).unwrap(), 1);
    assert_eq!(distinct_offsets(&zero, 2), vec![vec![0, 0, 0]]);
    assert_eq!(
        zero.iter()
            .map(|request| request.source_ordinal())
            .collect::<Vec<_>>(),
        vec![0, 1]
    );

    let one = collect_shell(scheduler.try_depth_shell(1).unwrap(), 3);
    assert_eq!(
        distinct_offsets(&one, 2),
        vec![
            vec![-1, 0, 0],
            vec![0, -1, 0],
            vec![0, 0, -1],
            vec![0, 0, 1],
            vec![0, 1, 0],
            vec![1, 0, 0],
        ]
    );
    assert_eq!(one.len(), 12);
}

#[test]
fn whole_shell_budgets_reject_before_cursor_allocation() {
    let exact = SignedL1ShellScheduler::try_new(3, 2, SignedL1ScheduleLimits::default())
        .unwrap()
        .try_depth_shell(2)
        .unwrap();
    assert_eq!(exact.offset_count(), 18);
    assert_eq!(exact.request_count(), 36);

    let limits = SignedL1ScheduleLimits {
        max_offsets_per_shell: 17,
        ..SignedL1ScheduleLimits::default()
    };
    assert!(matches!(
        SignedL1ShellScheduler::try_new(3, 2, limits)
            .unwrap()
            .try_depth_shell(2),
        Err(SpiredFoundationError::ResourceLimit {
            resource: "signed-L1 offsets per depth shell",
            requested: 18,
            limit: 17,
        })
    ));

    let limits = SignedL1ScheduleLimits {
        max_requests_per_shell: 35,
        ..SignedL1ScheduleLimits::default()
    };
    assert!(matches!(
        SignedL1ShellScheduler::try_new(3, 2, limits)
            .unwrap()
            .try_depth_shell(2),
        Err(SpiredFoundationError::ResourceLimit {
            resource: "signed-L1 source requests per depth shell",
            requested: 36,
            limit: 35,
        })
    ));
}

#[test]
fn chunk_budgets_reject_without_advancing_the_cursor() {
    let limits = SignedL1ScheduleLimits {
        max_requests_per_chunk: 5,
        ..SignedL1ScheduleLimits::default()
    };
    let mut shell = SignedL1ShellScheduler::try_new(2, 2, limits)
        .unwrap()
        .try_depth_shell(1)
        .unwrap();
    assert!(matches!(
        shell.try_next_chunk(6),
        Err(SpiredFoundationError::ResourceLimit {
            resource: "signed-L1 source requests per chunk",
            requested: 6,
            limit: 5,
        })
    ));
    assert_eq!(shell.emitted_request_count(), 0);

    let limits = SignedL1ScheduleLimits {
        max_requests_per_chunk: 6,
        max_offset_coordinate_cells_per_chunk: 11,
        ..SignedL1ScheduleLimits::default()
    };
    let mut shell = SignedL1ShellScheduler::try_new(2, 2, limits)
        .unwrap()
        .try_depth_shell(1)
        .unwrap();
    assert!(matches!(
        shell.try_next_chunk(6),
        Err(SpiredFoundationError::ResourceLimit {
            resource: "signed-L1 offset coordinate cells per chunk",
            requested: 12,
            limit: 11,
        })
    ));
    assert_eq!(shell.emitted_request_count(), 0);
    assert_eq!(
        shell.try_next_chunk(0).unwrap_err(),
        SpiredFoundationError::EmptyRequestChunk
    );
}

#[test]
fn count_workspace_and_lattice_carrier_overflow_fail_before_enumeration() {
    let unlimited = SignedL1ScheduleLimits {
        max_arity: usize::MAX,
        max_source_rows: usize::MAX,
        max_depth: usize::MAX,
        max_offsets_per_shell: usize::MAX,
        max_requests_per_shell: usize::MAX,
        max_enumeration_workspace_entries: usize::MAX,
        max_requests_per_chunk: usize::MAX,
        max_offset_coordinate_cells_per_chunk: usize::MAX,
    };
    let width = usize::BITS as usize;
    assert!(matches!(
        SignedL1ShellScheduler::try_new(width, 1, unlimited)
            .unwrap()
            .try_depth_shell(width),
        Err(SpiredFoundationError::ResourceCountOverflow {
            resource: "signed-L1 offsets per depth shell",
        })
    ));
    assert!(matches!(
        SignedL1ShellScheduler::try_new(usize::MAX, 1, unlimited),
        Err(SpiredFoundationError::ResourceCountOverflow {
            resource: "signed-L1 enumeration workspace entries",
        })
    ));

    #[cfg(target_pointer_width = "64")]
    assert_eq!(
        SignedL1ShellScheduler::try_new(1, 1, unlimited)
            .unwrap()
            .try_depth_shell(usize::MAX)
            .unwrap_err(),
        SpiredFoundationError::DepthNotRepresentable { depth: usize::MAX }
    );
}

#[test]
fn invalid_scheduler_configuration_is_rejected() {
    assert_eq!(
        SignedL1ShellScheduler::try_new(0, 1, SignedL1ScheduleLimits::default()).unwrap_err(),
        SpiredFoundationError::EmptyIndexSpace
    );
    assert_eq!(
        SignedL1ShellScheduler::try_new(1, 0, SignedL1ScheduleLimits::default()).unwrap_err(),
        SpiredFoundationError::EmptySourceRows
    );
}

#[test]
fn small_l1_stream_matches_brute_force_across_arity_depth_and_chunkings() {
    for arity in 1..=4 {
        for depth in 0..=4 {
            let scheduler =
                SignedL1ShellScheduler::try_new(arity, 3, SignedL1ScheduleLimits::default())
                    .unwrap();
            let unit_chunks = collect_shell(scheduler.try_depth_shell(depth).unwrap(), 1);
            let mixed_chunks = collect_shell(
                scheduler.try_depth_shell(depth).unwrap(),
                2 + (arity + depth) % 7,
            );
            assert_eq!(unit_chunks, mixed_chunks);

            let offsets = distinct_offsets(&unit_chunks, 3);
            assert!(offsets.windows(2).all(|pair| pair[0] < pair[1]));
            assert_eq!(
                offsets.iter().cloned().collect::<BTreeSet<_>>(),
                brute_force_signed_l1_offsets(arity, depth)
            );
            assert!(unit_chunks.chunks_exact(3).all(|source_block| {
                source_block
                    .iter()
                    .map(|request| request.source_ordinal())
                    .eq(0..3)
            }));
        }
    }
}
