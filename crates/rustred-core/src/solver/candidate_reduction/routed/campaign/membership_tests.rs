use super::*;
use crate::solver::candidate_reduction::owner_test_support::{input, key, programs};
use std::sync::{Arc, Barrier};

pub(super) fn fixture(
    limits: ReductionLimits,
    routing: super::super::super::RoutedCandidateLimits,
) -> RoutedCandidateReducer<1> {
    let family = Arc::new(crate::solver::tests::tadpole());
    let owner = input([true], Some(0), vec![], &[[1]]);
    RoutedCandidateReducer::try_new(programs(family, Some(0), vec![owner], limits), [], routing)
        .unwrap()
}

pub(super) fn route(power: i64) -> Work<1> {
    Work::Route(key([power]))
}

fn drain(shared: &Shared<'_, 1>, expected: &[Work<1>]) {
    for node in expected {
        let actual = shared.take().unwrap();
        assert_eq!(&actual, node);
        shared.finish(actual, Ok(()));
    }
    assert!(shared.take().is_none());
}

#[test]
fn membership_preprobe_racing_absent_keys_commit_once() {
    let reducer = fixture(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let shared = Shared::new(&reducer, 2, &cancel);
    let barrier = Barrier::new(2);
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let shared = &shared;
            let barrier = &barrier;
            scope.spawn(move || {
                let batch = shared.preprobe_batch(vec![route(2)]).unwrap();
                assert!(matches!(&batch.0[0], ProbedWork::NeedsCommit { .. }));
                barrier.wait();
                shared.commit_batch(batch).unwrap();
            });
        }
    });
    let snapshot = shared.snapshot();
    assert_eq!(snapshot.scheduled_nodes, 1);
    assert_eq!(snapshot.reachable_integrals, 1);
    assert_eq!(snapshot.queued_nodes, 1);
    assert_eq!(snapshot.deduplication_hits, 1);
    drain(&shared, &[route(2)]);
    assert!(shared.into_result().unwrap().snapshot().finished);
}

#[test]
fn membership_preprobe_racing_new_keys_cannot_overbook_global_limits() {
    for pending in [false, true] {
        let reducer = fixture(
            ReductionLimits {
                diagnostics_enabled: true,
                max_pending_frames: if pending { 1 } else { 10 },
                ..Default::default()
            },
            super::super::super::RoutedCandidateLimits {
                diagnostics_enabled: true,
                max_unique_nodes: if pending { 10 } else { 1 },
                ..Default::default()
            },
        );
        let cancel = AtomicBool::new(false);
        let shared = Shared::new(&reducer, 2, &cancel);
        let barrier = Barrier::new(2);
        let results = std::thread::scope(|scope| {
            let mut threads = Vec::new();
            for value in [1, 2] {
                let shared = &shared;
                let barrier = &barrier;
                threads.push(scope.spawn(move || {
                    let batch = shared.preprobe_batch(vec![route(value)]).unwrap();
                    barrier.wait();
                    shared.commit_batch(batch)
                }));
            }
            threads
                .into_iter()
                .map(|thread| thread.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        let expected = resource_error(
            if pending {
                "pending nodes"
            } else {
                "operational nodes"
            },
            2,
            1,
        );
        assert_eq!(
            results
                .into_iter()
                .filter_map(Result::err)
                .collect::<Vec<_>>(),
            vec![expected]
        );
        assert_eq!(shared.snapshot().scheduled_nodes, 1);
        assert_eq!(shared.snapshot().reachable_integrals, 1);
        assert_eq!(shared.snapshot().queued_nodes, 1);
        assert_eq!(shared.snapshot().deduplication_hits, 0);
        let node = shared.take().unwrap();
        shared.finish(node, Ok(()));
    }
}

#[test]
fn membership_preprobe_same_batch_absent_duplicates_recheck_and_keep_fifo() {
    let reducer = fixture(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let shared = Shared::new(&reducer, 1, &cancel);
    let batch = shared
        .preprobe_batch(vec![route(3), route(3), route(2)])
        .unwrap();
    assert!(
        batch
            .0
            .iter()
            .all(|item| matches!(item, ProbedWork::NeedsCommit { .. }))
    );
    shared.commit_batch(batch).unwrap();
    assert_eq!(shared.snapshot().scheduled_nodes, 2);
    assert_eq!(shared.snapshot().reachable_integrals, 2);
    assert_eq!(shared.snapshot().deduplication_hits, 1);
    drain(&shared, &[route(3), route(2)]);
}

#[test]
fn membership_preprobe_racing_route_and_apply_keep_both_phases_one_physical_key() {
    let reducer = fixture(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let shared = Shared::new(&reducer, 2, &cancel);
    let barrier = Barrier::new(2);
    let apply = Work::Apply {
        owner_sector: [true],
        target: key([1]),
    };
    std::thread::scope(|scope| {
        for node in [route(1), apply.clone()] {
            let shared = &shared;
            let barrier = &barrier;
            scope.spawn(move || {
                let batch = shared.preprobe_batch(vec![node]).unwrap();
                assert!(matches!(&batch.0[0], ProbedWork::NeedsCommit { .. }));
                barrier.wait();
                shared.commit_batch(batch).unwrap();
            });
        }
    });
    shared
        .schedule_batch(vec![route(1), apply.clone()])
        .unwrap();
    let snapshot = shared.snapshot();
    assert_eq!(snapshot.scheduled_nodes, 2);
    assert_eq!(snapshot.reachable_integrals, 1);
    assert_eq!(snapshot.deduplication_hits, 2);
    let mut received = BTreeSet::new();
    for _ in 0..2 {
        let node = shared.take().unwrap();
        received.insert(node.clone());
        shared.finish(node, Ok(()));
    }
    assert_eq!(received, BTreeSet::from([route(1), apply]));
    assert!(shared.into_result().unwrap().snapshot().finished);
}

#[test]
fn membership_preprobe_known_unknown_cap_failure_keeps_exact_prefix() {
    for pending in [false, true] {
        let reducer = fixture(
            ReductionLimits {
                diagnostics_enabled: true,
                max_pending_frames: if pending { 5 } else { 100 },
                ..Default::default()
            },
            super::super::super::RoutedCandidateLimits {
                diagnostics_enabled: true,
                max_unique_nodes: if pending { 100 } else { 5 },
                ..Default::default()
            },
        );
        let cancel = AtomicBool::new(false);
        let shared = Shared::new(&reducer, 1, &cancel);
        shared
            .schedule_batch(vec![route(1), route(2), route(3), route(4)])
            .unwrap();
        let active = shared.take().unwrap();
        // The last known key must not be counted across the earlier cap error.
        let batch = shared
            .preprobe_batch(vec![route(1), route(5), route(2), route(6), route(3)])
            .unwrap();
        assert!(matches!(&batch.0[0], ProbedWork::KnownPresent));
        assert!(matches!(&batch.0[1], ProbedWork::NeedsCommit { .. }));
        assert!(matches!(&batch.0[2], ProbedWork::KnownPresent));
        let error = shared.commit_batch(batch).unwrap_err();
        assert_eq!(
            error,
            resource_error(
                if pending {
                    "pending nodes"
                } else {
                    "operational nodes"
                },
                6,
                5
            )
        );
        let snapshot = shared.snapshot();
        assert_eq!(snapshot.scheduled_nodes, 5);
        assert_eq!(snapshot.reachable_integrals, 5);
        assert_eq!(snapshot.queued_nodes, 4);
        assert_eq!(snapshot.active_nodes, 1);
        assert_eq!(snapshot.deduplication_hits, 2);
        assert!(!shared.membership.probe(&route(6)).1);
        // Even exhausted caps permit already-committed joins.
        shared.schedule_batch(vec![route(1), route(5)]).unwrap();
        assert_eq!(shared.snapshot().deduplication_hits, 4);
        shared.finish(active.clone(), Err(error.clone()));
        let result = shared.into_result().unwrap_err();
        assert_eq!(result.reason(), &error);
        assert_eq!(result.snapshot().first_failure_work.as_ref(), Some(&active));
        assert!(!result.snapshot().finished);
    }
}

#[test]
fn membership_preprobe_late_invalid_owner_and_arity_preserve_prefix() {
    for invalid in [
        Work::Apply {
            owner_sector: [false],
            target: key([1]),
        },
        Work::Route(key([1, 1])),
    ] {
        let reducer = fixture(Default::default(), Default::default());
        let cancel = AtomicBool::new(false);
        let shared = Shared::new(&reducer, 1, &cancel);
        shared.schedule(route(1)).unwrap();
        let batch = shared
            .preprobe_batch(vec![route(1), route(2), invalid, route(3)])
            .unwrap();
        assert_eq!(batch.0.len(), 3);
        assert!(matches!(&batch.0[2], ProbedWork::Invalid(_)));
        assert!(matches!(
            shared.commit_batch(batch),
            Err(Failure::Trace(CandidateRoutedError::InvalidInput(_)))
        ));
        assert_eq!(shared.snapshot().scheduled_nodes, 2);
        assert_eq!(shared.snapshot().reachable_integrals, 2);
        assert_eq!(shared.snapshot().deduplication_hits, 1);
        assert!(!shared.membership.probe(&route(3)).1);
        drain(&shared, &[route(1), route(2)]);
    }
}

#[test]
fn membership_preprobe_cancel_or_peer_failure_before_commit_changes_no_counts() {
    for cancellation in [false, true] {
        let reducer = fixture(Default::default(), Default::default());
        let cancel = AtomicBool::new(false);
        let shared = Shared::new(&reducer, 1, &cancel);
        shared.schedule(route(1)).unwrap();
        let active = shared.take().unwrap();
        let batch = shared.preprobe_batch(vec![route(1), route(2)]).unwrap();
        let error = if cancellation {
            cancel.store(true, Ordering::Release);
            Failure::Cancelled
        } else {
            let original = Failure::Trace(CandidateRoutedError::InvalidInput(
                "first peer error".into(),
            ));
            shared.fail(original.clone());
            original
        };
        assert_eq!(shared.commit_batch(batch), Err(error.clone()));
        let snapshot = shared.snapshot();
        assert_eq!(snapshot.scheduled_nodes, 1);
        assert_eq!(snapshot.reachable_integrals, 1);
        assert_eq!(snapshot.deduplication_hits, 0);
        assert!(!shared.membership.probe(&route(2)).1);
        shared.finish(active, Err(Failure::WorkerPanicked));
        assert_eq!(shared.into_result().unwrap_err().reason(), &error);
    }
}

#[test]
fn membership_preprobe_duplicate_counter_overflow_keeps_exact_committed_prefix() {
    let reducer = fixture(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let shared = Shared::new(&reducer, 1, &cancel);
    shared.schedule(route(1)).unwrap();
    let batch = shared
        .preprobe_batch(vec![route(1), route(1), route(2)])
        .unwrap();
    shared.lock().dedup = usize::MAX - 1;
    assert_eq!(
        shared.commit_batch(batch),
        Err(resource_error(
            "deduplication counter",
            usize::MAX,
            usize::MAX
        ))
    );
    assert_eq!(shared.snapshot().deduplication_hits, usize::MAX);
    assert_eq!(shared.snapshot().scheduled_nodes, 1);
    assert!(!shared.membership.probe(&route(2)).1);
    drain(&shared, &[route(1)]);
}

#[test]
fn membership_preprobe_batch_bound_applies_even_to_all_known_keys() {
    let reducer = fixture(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let shared = Shared::new(&reducer, 1, &cancel);
    shared.schedule(route(1)).unwrap();
    assert!(
        shared
            .preprobe_batch(vec![route(1); PUBLICATION_BATCH_SIZE + 1])
            .is_err()
    );
    assert_eq!(shared.snapshot().scheduled_nodes, 1);
    assert_eq!(shared.snapshot().deduplication_hits, 0);
}

#[test]
fn membership_preprobe_does_not_hold_global_lock_and_probe_guard_is_released() {
    let reducer = fixture(Default::default(), Default::default());
    let cancel = AtomicBool::new(false);
    let shared = Shared::new(&reducer, 1, &cancel);
    shared.schedule(route(1)).unwrap();
    // A regression must fail without leaving the test suite deadlocked.
    let guard = shared.lock();
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    std::thread::scope(|scope| {
        let shared = &shared;
        scope.spawn(move || {
            send.send(shared.preprobe_batch(vec![route(1), route(2)]).unwrap())
                .unwrap();
        });
        let result = receive.recv_timeout(Duration::from_secs(5));
        drop(guard);
        let batch = result.expect("preprobe must not wait for the global state lock");
        // Its shard guard was dropped: ordinary ordered commit reacquires it.
        shared.commit_batch(batch).unwrap();
    });
    assert_eq!(shared.snapshot().deduplication_hits, 1);
    drain(&shared, &[route(1), route(2)]);
}

mod resident_preprobe_differential {
    use super::*;
    use crate::persistence::finite_work::{
        GeneratedDirectoryLimits, GeneratedExecutionBinding, GeneratedObjectRef,
        GeneratedPageReadUsage, GeneratedWorkError, GeneratedWorkErrorKind,
        GeneratedWorkPageRecord, GeneratedWorkPageRef, GeneratedWorkResult, PAGE_MAGIC,
        ValidatedWorkPage, encode_metadata_frame,
    };

    struct Pages {
        page: ValidatedWorkPage,
        fail: AtomicBool,
    }
    impl GeneratedPageProvider for Pages {
        fn visit_validated_page(
            &self,
            _: &GeneratedWorkPageRef,
            _: u64,
            visitor: &mut dyn FnMut(&ValidatedWorkPage),
        ) -> GeneratedWorkResult<GeneratedPageReadUsage> {
            if self.fail.load(Ordering::Acquire) {
                return Err(GeneratedWorkError::new(
                    GeneratedWorkErrorKind::Io,
                    "backing page became unavailable",
                ));
            }
            visitor(&self.page);
            Ok(GeneratedPageReadUsage {
                disk_bytes: 0,
                cache_hit: true,
            })
        }
    }
    struct Baseline {
        directory: TrustedGeneratedWorkDirectory,
        pages: Pages,
        resident: TrustedGeneratedResidentWork,
    }
    impl Baseline {
        fn new() -> Self {
            let execution = GeneratedExecutionBinding {
                physical_arity: 1,
                program_identity: [7; 32],
            };
            let record = GeneratedWorkPageRecord {
                execution: execution.clone(),
                records: vec![
                    GeneratedWorkKey::Route(vec![1]),
                    GeneratedWorkKey::Route(vec![3]),
                    GeneratedWorkKey::Apply {
                        owner: vec![true],
                        powers: vec![1],
                    },
                ],
            };
            let bytes = encode_metadata_frame(PAGE_MAGIC, &record, 10_000).unwrap();
            let reference = GeneratedWorkPageRef {
                object: GeneratedObjectRef {
                    digest: *blake3::hash(&bytes).as_bytes(),
                    bytes: bytes.len() as u64,
                },
                records: record.records.len() as u64,
                first: record.records.first().unwrap().clone(),
                last: record.records.last().unwrap().clone(),
            };
            let pages = Pages {
                page: ValidatedWorkPage::decode_generated(
                    &bytes, &reference, &execution, 10_000, 100,
                )
                .unwrap(),
                fail: AtomicBool::new(false),
            };
            let directory = TrustedGeneratedWorkDirectory::try_from_trusted_generated(
                execution,
                [8; 32],
                Arc::new(vec![vec![reference]]),
                GeneratedDirectoryLimits {
                    diagnostics_enabled: true,
                    max_arity: 1,
                    max_runs: 1,
                    max_pages: 1,
                    max_directory_payload_bytes: 100_000,
                    max_page_records: 100,
                    max_page_bytes: 10_000,
                    max_referenced_bytes: 100_000,
                    max_lookup_pages: 1,
                    max_lookup_bytes: 100_000,
                },
            )
            .unwrap();
            let resident = TrustedGeneratedResidentWork::hydrate(
                &directory,
                &pages,
                100_000,
                0,
                &AtomicBool::new(false),
            )
            .unwrap()
            .unwrap();
            Self {
                directory,
                pages,
                resident,
            }
        }
        fn shared<'a>(
            &'a self,
            reducer: &'a RoutedCandidateReducer<1>,
            cancel: &'a AtomicBool,
            resident: bool,
        ) -> Shared<'a, 1> {
            let mut shared = Shared::new(reducer, 1, cancel);
            shared.set_completed_baseline(Some(CompletedBaseline::Generated {
                directory: &self.directory,
                provider: &self.pages,
                resident: resident.then_some(&self.resident),
            }));
            shared.schedule(route(2)).unwrap();
            shared.schedule(apply(3)).unwrap();
            shared
        }
    }
    fn apply(power: i64) -> Work<1> {
        Work::Apply {
            owner_sector: [true],
            target: key([power]),
        }
    }

    // Frozen pre-optimization ordering, deliberately independent of the new
    // resident shortcut. It also serves as the baseline for the ignored probe.
    fn baseline_first(
        shared: &Shared<'_, 1>,
        nodes: Vec<Work<1>>,
    ) -> Result<ProbedBatch<1>, Failure> {
        limit(
            0,
            nodes.len(),
            Some(shared.publication_capacity()),
            "publication batch",
        )?;
        let mut batch = Vec::with_capacity(nodes.len());
        for node in nodes {
            if let Err(error) = Shared::validate_node(&node) {
                batch.push(ProbedWork::Invalid(error));
                break;
            }
            match shared.baseline_contains(&node) {
                Ok(true) => {
                    batch.push(ProbedWork::CompletedBaseline);
                    continue;
                }
                Ok(false) => {}
                Err(error) => {
                    batch.push(ProbedWork::Invalid(error));
                    break;
                }
            }
            let (shard, present) = shared.membership.probe(&node);
            batch.push(if present {
                ProbedWork::KnownPresent
            } else {
                ProbedWork::NeedsCommit { node, shard }
            });
        }
        Ok(ProbedBatch(batch))
    }
    fn markers(batch: &ProbedBatch<1>) -> Vec<(u8, Option<Work<1>>, Option<Failure>)> {
        batch
            .0
            .iter()
            .map(|marker| match marker {
                ProbedWork::KnownPresent => (0, None, None),
                ProbedWork::CompletedBaseline => (1, None, None),
                ProbedWork::NeedsCommit { node, .. } => (2, Some(node.clone()), None),
                ProbedWork::Invalid(error) => (3, None, Some(error.clone())),
            })
            .collect()
    }
    fn compare(
        old: &Shared<'_, 1>,
        new: &Shared<'_, 1>,
        nodes: Vec<Work<1>>,
        before_commit: impl FnOnce(),
    ) -> Result<(), Failure> {
        let old_batch = baseline_first(old, nodes.clone()).unwrap();
        let new_batch = new.preprobe_batch(nodes).unwrap();
        assert_eq!(markers(&old_batch), markers(&new_batch));
        before_commit();
        let old_result = old.commit_batch(old_batch);
        let new_result = new.commit_batch(new_batch);
        assert_eq!(old_result, new_result);
        let mut expected = old.snapshot();
        let mut actual = new.snapshot();
        expected.elapsed = Duration::ZERO;
        actual.elapsed = Duration::ZERO;
        assert_eq!(expected, actual);
        assert_eq!(old.lock().queue, new.lock().queue);
        assert_eq!(
            new.prefer_live_membership.load(Ordering::Relaxed),
            actual.deduplication_hits > actual.completed_baseline_hits
        );
        assert_eq!(
            old.prefer_live_membership.load(Ordering::Relaxed),
            new.prefer_live_membership.load(Ordering::Relaxed)
        );
        old_result
    }

    #[test]
    fn resident_preprobe_duplicate_and_baseline_heavy_batches_match_old_order() {
        let reducer = fixture(Default::default(), Default::default());
        let baseline = Baseline::new();
        for heavy in [route(2), route(1)] {
            let cancel = AtomicBool::new(false);
            let old = baseline.shared(&reducer, &cancel, true);
            let new = baseline.shared(&reducer, &cancel, true);
            compare(&old, &new, vec![heavy.clone()], || {}).unwrap();
            assert_eq!(
                new.prefer_live_membership.load(Ordering::Relaxed),
                heavy == route(2)
            );
            let mut nodes = vec![heavy; 240];
            nodes.extend([apply(2), route(4), route(4), apply(3), route(3), apply(1)]);
            compare(&old, &new, nodes, || {}).unwrap();
            // Baseline Route(3) and live Apply(3) remain different identities;
            // live Route(2) cannot suppress the newly scheduled Apply(2).
            assert_eq!(new.snapshot().scheduled_nodes, 4);
            assert_eq!(
                new.snapshot().completed_baseline_hits + new.snapshot().deduplication_hits,
                245
            );
            drain(&new, &[route(2), apply(3), apply(2), route(4)]);
        }
    }

    #[test]
    fn resident_preprobe_failures_cancellation_and_overflow_match_old_prefix() {
        let baseline = Baseline::new();
        for case in [
            "cap",
            "owner",
            "arity",
            "cancel",
            "overflow",
            "baseline-overflow",
        ] {
            let reducer = fixture(
                Default::default(),
                super::super::super::super::RoutedCandidateLimits {
                    diagnostics_enabled: true,
                    max_unique_nodes: if case == "cap" { 3 } else { 100 },
                    ..Default::default()
                },
            );
            let cancel = AtomicBool::new(false);
            let old = baseline.shared(&reducer, &cancel, true);
            let new = baseline.shared(&reducer, &cancel, true);
            compare(&old, &new, vec![route(2)], || {}).unwrap();
            assert!(new.prefer_live_membership.load(Ordering::Relaxed));
            let invalid = match case {
                "owner" => Work::Apply {
                    owner_sector: [false],
                    target: key([2]),
                },
                "arity" => Work::Route(key([2, 2])),
                _ => route(5),
            };
            let nodes = if case == "overflow" {
                vec![route(2), route(2), route(4)]
            } else if case == "baseline-overflow" {
                vec![route(1), route(1), route(2)]
            } else if case == "cap" {
                // The publication batch itself fits the cap; the second new
                // node must fail after the valid known/new prefix commits.
                vec![route(2), route(4), route(5)]
            } else {
                vec![route(2), route(4), route(3), invalid, route(2)]
            };
            let result = compare(&old, &new, nodes, || {
                if case == "cancel" {
                    cancel.store(true, Ordering::Release);
                } else if case == "overflow" {
                    old.lock().dedup = usize::MAX - 1;
                    new.lock().dedup = usize::MAX - 1;
                } else if case == "baseline-overflow" {
                    old.lock().baseline_hits = usize::MAX - 1;
                    new.lock().baseline_hits = usize::MAX - 1;
                }
            });
            assert!(result.is_err(), "{case}");
            if case == "cap" {
                assert_eq!(result, Err(resource_error("operational nodes", 4, 3)));
                assert_eq!(new.snapshot().scheduled_nodes, 3);
                assert_eq!(new.snapshot().deduplication_hits, 2);
                assert!(!new.membership.probe(&route(5)).1);
            } else if case == "cancel" {
                assert_eq!(new.snapshot().deduplication_hits, 1);
                assert_eq!(new.snapshot().completed_baseline_hits, 0);
            } else if case == "baseline-overflow" {
                assert_eq!(new.snapshot().completed_baseline_hits, usize::MAX);
                assert_eq!(new.snapshot().deduplication_hits, 1);
                assert!(!new.prefer_live_membership.load(Ordering::Relaxed));
            }
        }
    }

    #[test]
    fn resident_preprobe_choice_tracks_only_committed_hit_counts_and_can_reverse() {
        let reducer = fixture(Default::default(), Default::default());
        let cancel = AtomicBool::new(false);
        let baseline = Baseline::new();
        let old = baseline.shared(&reducer, &cancel, true);
        let new = baseline.shared(&reducer, &cancel, true);
        assert!(!new.prefer_live_membership.load(Ordering::Relaxed));
        // Live hits enable the shortcut; equal or dominant baseline hits use
        // the original ordering again. Preprobing alone never changes policy.
        for (nodes, expected) in [
            (vec![route(2)], true),
            (vec![route(1)], false),
            (vec![route(2), route(2)], true),
            (vec![route(1); 3], false),
        ] {
            compare(&old, &new, nodes, || {}).unwrap();
            assert_eq!(new.prefer_live_membership.load(Ordering::Relaxed), expected);
        }
        let before = new.snapshot();
        let pending = new.preprobe_batch(vec![route(2); 8]).unwrap();
        assert!(!new.prefer_live_membership.load(Ordering::Relaxed));
        cancel.store(true, Ordering::Release);
        assert_eq!(new.commit_batch(pending), Err(Failure::Cancelled));
        assert_eq!(new.snapshot().deduplication_hits, before.deduplication_hits);
        assert_eq!(
            new.snapshot().completed_baseline_hits,
            before.completed_baseline_hits
        );
        assert!(!new.prefer_live_membership.load(Ordering::Relaxed));
    }

    #[test]
    fn paged_preprobe_still_reports_provider_failure_for_a_live_duplicate() {
        let reducer = fixture(Default::default(), Default::default());
        let cancel = AtomicBool::new(false);
        let baseline = Baseline::new();
        let old = baseline.shared(&reducer, &cancel, false);
        let new = baseline.shared(&reducer, &cancel, false);
        compare(&old, &new, vec![route(2)], || {}).unwrap();
        assert!(new.prefer_live_membership.load(Ordering::Relaxed));
        baseline.pages.fail.store(true, Ordering::Release);
        assert!(matches!(
            compare(&old, &new, vec![route(2), route(4)], || {}),
            Err(Failure::Backing(GeneratedWorkError {
                kind: GeneratedWorkErrorKind::Io,
                ..
            }))
        ));
        assert_eq!(new.snapshot().deduplication_hits, 1);
        assert_eq!(new.snapshot().scheduled_nodes, 2);
    }

    #[test]
    #[ignore = "bounded old/new resident preprobe timing; run explicitly in an optimized build"]
    fn resident_preprobe_old_new_bounded_timing() {
        let reducer = fixture(Default::default(), Default::default());
        let cancel = AtomicBool::new(false);
        let baseline = Baseline::new();
        for (label, node) in [("live-duplicates", route(2)), ("baseline-hits", route(1))] {
            // Each workload starts from fresh membership/counters. Warmup is
            // a real committed batch, outside the clock and shared by both
            // timed algorithms; preprobe itself changes neither admitted set.
            let shared = baseline.shared(&reducer, &cancel, true);
            let expect_live_first = node == route(2);
            let nodes = vec![node; PUBLICATION_BATCH_SIZE];
            shared.schedule_batch(nodes.clone()).unwrap();
            assert_eq!(
                shared.prefer_live_membership.load(Ordering::Relaxed),
                expect_live_first
            );
            for fast_first in [false, true] {
                for fast in [fast_first, !fast_first] {
                    let started = Instant::now();
                    for _ in 0..2000 {
                        let batch = if fast {
                            shared.preprobe_batch(nodes.clone())
                        } else {
                            baseline_first(&shared, nodes.clone())
                        }
                        .unwrap();
                        std::hint::black_box(batch);
                    }
                    eprintln!(
                        "{label} adaptive={fast} live_first_hint={expect_live_first} elapsed={:?}",
                        started.elapsed()
                    );
                }
            }
        }
    }
}
