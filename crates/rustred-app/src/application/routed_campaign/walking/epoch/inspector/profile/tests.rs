use super::*;
use crate::application::routed_campaign::walking::{
    epoch::resolve::Resolver,
    inspection::{Finished, NativeStats},
    queue::{CompactDomain, Domain, Phase},
};
use std::ops::ControlFlow;
use std::time::Duration;

fn job(seq: u64) -> Job<1> {
    Job {
        seq,
        parent: 0,
        v0: 0,
        attempts: 0,
        flags: 0,
        image: CompactDomain::try_from_domain(&Domain {
            phase: Phase::Apply,
            owner: [true],
            lower: vec![0],
            upper: vec![None],
            rank: Some(2),
            powers: Default::default(),
        })
        .unwrap(),
    }
}

fn activity(started: bool, returned: bool) -> Option<(Activity, Option<Status>)> {
    Some((
        Activity {
            computing: usize::from(started && !returned),
            queued: usize::from(!started),
            returned: 7 + usize::from(returned),
            occupied: 8,
            ..Default::default()
        },
        Some(Status {
            key: 8,
            started,
            returned,
        }),
    ))
}

#[test]
fn missing_sequence_is_in_required_prefix_not_first_issued_or_later_tail() {
    let order = VecDeque::from([3, 8, 12]);
    let mut results: BTreeMap<u64, Vec<u8>> = BTreeMap::from([(3, vec![])]);
    assert_eq!(earliest_missing(&order, &results, 2), Some(8));
    results.insert(8, vec![]);
    assert_eq!(earliest_missing(&order, &results, 2), None);
    assert_eq!(earliest_missing(&order, &results, 3), Some(12));
}

#[test]
fn escrow_wait_classifies_logical_credits_not_recycled_pool_slots() {
    let physical = Some((
        Activity {
            computing: 1,
            occupied: 1,
            ..Default::default()
        },
        Some(Status {
            key: 8,
            started: true,
            returned: false,
        }),
    ));
    let full = WaitSample::capture_with_inventory(
        false,
        true,
        Some(8),
        16,
        4,
        10,
        physical,
        Some((16, 15)),
    );
    assert_eq!(full.kind, WaitKind::PrefixCreditBlocked);
    assert_eq!(full.returned, 0);
    assert_eq!(full.logical_returned, Some(15));
    let room =
        WaitSample::capture_with_inventory(false, true, Some(8), 16, 4, 10, physical, Some((8, 7)));
    assert_eq!(room.kind, WaitKind::PrefixComputing);
}

#[test]
fn wait_classes_are_exclusive_and_use_exact_prefix_status_not_tail_activity() {
    let capture = |publication, issued, missing, a| {
        WaitSample::capture(publication, issued, missing, 8, 4, 10, a).kind
    };
    assert_eq!(
        capture(true, false, None, None),
        WaitKind::PublicationRetention
    );
    assert_eq!(capture(false, false, None, None), WaitKind::NoIssuedWork);
    assert_eq!(
        capture(false, true, None, activity(true, true)),
        WaitKind::PrefixReady
    );
    assert_eq!(
        capture(false, true, Some(8), activity(false, false)),
        WaitKind::PrefixQueued
    );
    assert_eq!(
        capture(false, true, Some(8), activity(true, false)),
        WaitKind::PrefixCreditBlocked
    );
    assert_eq!(
        capture(false, true, Some(8), activity(true, true)),
        WaitKind::ReceiptDrain
    );
    assert_eq!(capture(false, true, Some(8), None), WaitKind::Unknown);
    let (mut a, status) = activity(true, false).unwrap();
    a.queued = 2; // A tail is queued, but the actual missing prefix is running.
    assert_eq!(
        capture(false, true, Some(8), Some((a, status))),
        WaitKind::PrefixComputing
    );
    a.queued = 0;
    a.computing = 4; // Fully occupied inspectors are not idle-credit starvation.
    assert_eq!(
        capture(false, true, Some(8), Some((a, status))),
        WaitKind::PrefixComputing
    );
    a.inline = true;
    assert_eq!(
        capture(false, true, Some(8), Some((a, status))),
        WaitKind::InlineInspection
    );
}

#[test]
fn wait_buckets_reconcile_and_coalesce_same_blocker_with_shared_clock() {
    let origin = Instant::now();
    let mut waits = Waits::new(origin);
    for (at, duration, running) in [(1, 0.25, false), (2, 0.5, true)] {
        waits.record(
            WaitSample::capture(false, true, Some(8), 8, 4, 10, activity(running, false)),
            origin + Duration::from_secs(at),
            duration,
        );
    }
    waits.finish();
    let sample = waits.longest_blocker_windows[0].unwrap();
    assert_eq!(sample.polls, 2);
    assert_eq!(sample.seconds, 0.75);
    assert_eq!(sample.started_seconds, 1.0);
    assert_eq!(sample.ended_seconds, 2.5);
    assert!(sample.mixed_kinds);
    assert_eq!(
        waits.calls,
        waits.buckets.iter().map(|b| b.calls).sum::<u64>()
    );
    assert_eq!(
        waits.seconds,
        waits.buckets.iter().map(|b| b.seconds).sum::<f64>()
    );
    assert_eq!(waits.computing_worker_seconds, 0.5);
    assert_eq!(waits.pending_work_wait_seconds, 0.75);
    assert!(waits.current_blocker.is_none());
}

#[test]
fn retained_wait_memory_is_fixed_and_unknown_activity_is_not_zero_evidence() {
    let origin = Instant::now();
    let mut waits = Waits::new(origin);
    for seq in 0..100 {
        waits.record(
            WaitSample::capture(false, true, Some(seq), 8, 4, 1, None),
            origin,
            seq as f64 + 1.0,
        );
    }
    waits.finish();
    assert_eq!(waits.longest_blocker_windows.len(), RETAIN);
    assert!(waits.longest_blocker_windows.iter().all(Option::is_some));
    assert_eq!(waits.longest_blocker_windows[0].unwrap().seconds, 100.0);
    assert_eq!(waits.missing_activity_seconds, waits.seconds);
}

#[test]
fn empty_error_cancel_and_panicked_observations_keep_absent_events_null() {
    let collector = Collector::default();
    for outcome in [
        Outcome::default(),
        Outcome {
            error: true,
            ..Default::default()
        },
        Outcome {
            cancel_requested: true,
            ..Default::default()
        },
        Outcome {
            panic: true,
            ..Default::default()
        },
    ] {
        collector.record(&job(1), Observation::new(Instant::now()), outcome);
    }
    let report = collector.snapshot();
    assert_eq!((report.jobs, report.eventless, report.admitless), (4, 4, 4));
    assert_eq!(
        (report.errors, report.cancel_requested, report.panics),
        (1, 1, 1)
    );
    for sample in report.slowest.iter().flatten() {
        assert_eq!(sample.first_event_seconds, None);
        assert_eq!(sample.first_admit_seconds, None);
        assert!(sample.started_seconds >= 0.0);
    }
}

#[test]
fn collector_poison_is_diagnostic_only_and_top_storage_stays_bounded() {
    let collector = Collector::default();
    for seq in 0..100 {
        collector.record(
            &job(seq),
            Observation::new(Instant::now()),
            Outcome::default(),
        );
    }
    assert_eq!(
        collector.snapshot().slowest.iter().flatten().count(),
        RETAIN
    );
    let _ = std::panic::catch_unwind(|| {
        let _held = collector.jobs.lock().unwrap();
        panic!("diagnostic-only synthetic poison");
    });
    collector.record(
        &job(100),
        Observation::new(Instant::now()),
        Outcome::default(),
    );
    assert!(collector.snapshot().poisoned);
}

#[test]
fn enabled_observation_preserves_exact_resolver_bytes_for_empty_success_error_cancel() {
    for (empty, error) in [
        (true, "none"),
        (false, "none"),
        (false, "cancelled"),
        (false, "native_failure"),
    ] {
        let run = |enabled: bool| {
            let collector = Collector::default();
            let mut observation = enabled.then(|| Observation::new(Instant::now()));
            let mut resolver = Resolver::<1>::new();
            if !empty {
                for event in [
                    Event::one(Effect::Count),
                    Event::one(Effect::Admit {
                        domain: job(0).image.expand(),
                        successor: true,
                        conditional: false,
                    }),
                ] {
                    if let Some(observation) = &mut observation {
                        observation.event(&event);
                    }
                    assert_eq!(resolver.emit(event), ControlFlow::Continue(()));
                }
            }
            let bytes = resolver
                .finish(
                    &job(0),
                    Finished {
                        stats: NativeStats::Apply(Default::default()),
                        error: (error != "none").then(|| "same controlled failure".into()),
                        error_kind: error,
                        seconds: 0.0,
                    },
                )
                .encode();
            if let Some(mut observation) = observation {
                observation.visitor_finished();
                collector.record(
                    &job(0),
                    observation,
                    Outcome {
                        error: error != "none",
                        ..Default::default()
                    },
                );
            }
            bytes
        };
        assert_eq!(run(false), run(true), "{empty}/{error}");
    }
}
