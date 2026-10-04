//! Profiling is invocation-local observation, not a new CP6 or JobResult lane.
use super::native_equivalence::{closed_fixture, completed_snapshot};
use super::*;
use crate::application::routed_campaign::walking::{
    epoch::{
        checkpoint::Section,
        inspector::{Context, inspect_job, inspect_job_profiled, profile},
    },
    initial_overlap::InitialOverlapIndex,
};

fn authority_sections(restored: &Restored<1>, window: usize) -> Vec<Vec<u8>> {
    let boundary = MergeBoundary::borrow(&restored.state, &restored.dispatch, window).unwrap();
    Section::ALL
        .into_iter()
        .filter(|section| *section != Section::Dispatch)
        .map(|section| boundary.write_section(Vec::new(), section).unwrap().0)
        .collect()
}

#[test]
fn real_native_profile_on_off_preserve_records_graph_and_cold_checkpoint_sections() {
    if !crate::test_gates::licensed_or_skip("epoch observational profile native equivalence") {
        return;
    }
    let mut fixture = closed_fixture();
    fixture.request.workers = 1;
    fixture.request.epoch_rolling = true;
    fixture.request.epoch_cut_size = Some(2);
    fixture.request.epoch_window = Some(8);
    fixture.request.checkpoint = Some(crate::OwnerDomainWalkCheckpointOptions::new(
        &fixture.directory.0,
    ));
    let mut expected = None;
    for enabled in [false, true] {
        fixture.directory = Directory::new();
        fixture.request.checkpoint = Some(crate::OwnerDomainWalkCheckpointOptions::new(
            &fixture.directory.0,
        ));
        fixture.save_window(3, 0, false, 8);
        let mut restored = fixture.open().unwrap();
        let collector = enabled.then(profile::Collector::default);
        restored.rolling_diagnostics.wait_profile =
            collector.as_ref().map(|p| profile::Waits::new(p.origin()));
        let overlap = InitialOverlapIndex::empty();
        let inspect = |bytes: &[u8], cancellation: &AtomicBool| {
            let context = Context {
                reducer: &fixture.reducer,
                request: &fixture.request,
                overlap: &overlap,
                cancellation,
                g2: None,
                finite_account: None,
            };
            match &collector {
                Some(collector) => inspect_job_profiled(&context, bytes, None, collector),
                None => inspect_job(&context, bytes),
            }
        };
        assert_eq!(
            controller::run_observed(
                &mut restored,
                &fixture.identity(),
                8,
                1,
                MergeConfig {
                    lockstep: false,
                    ..config()
                },
                &|| Ok(()),
                &inspect,
                || None,
                |_| false,
                |_, _, _| {},
                None,
                |_, _, _, _| {},
                |_| {}
            )
            .unwrap(),
            Outcome::Drained
        );
        let report = serde_json::to_value(&restored.rolling_diagnostics).unwrap();
        if let Some(collector) = collector {
            let jobs = serde_json::to_value(collector.snapshot()).unwrap();
            assert!(jobs["jobs"].as_u64().unwrap() > 0);
            assert_eq!(jobs["enabled"], true);
            assert_eq!(
                report["wait_profile"]["calls"],
                report["blocking_poll_calls"]
            );
            assert_eq!(
                report["wait_profile"]["seconds"],
                report["blocking_poll_seconds"]
            );
        } else {
            assert!(report.get("wait_profile").is_none());
        }
        drop(restored);
        let mut cold = fixture.open().unwrap();
        assert!(
            cold.rolling_diagnostics.wait_profile.is_none(),
            "restore starts a new observation session"
        );
        let actual = (completed_snapshot(&mut cold), authority_sections(&cold, 8));
        if let Some(expected) = &expected {
            assert_eq!(&actual, expected);
        } else {
            expected = Some(actual);
        }
    }
}

#[test]
fn profiling_waits_do_not_change_empty_error_or_cancelled_publication() {
    for scenario in ["empty", "error", "cancel"] {
        let mut expected = None;
        for enabled in [false, true] {
            let mut fixture = Fixture::new();
            fixture.request.epoch_rolling = true;
            fixture.request.epoch_cut_size = Some(1);
            fixture.request.epoch_window = Some(3);
            fixture.request.checkpoint = Some(crate::OwnerDomainWalkCheckpointOptions::new(
                &fixture.directory.0,
            ));
            fixture.save_window(3, 0, false, 3);
            let mut restored = fixture.open().unwrap();
            restored.rolling_diagnostics.wait_profile = enabled.then(profile::Waits::default);
            let cancel = AtomicBool::new(false);
            let inspect = |bytes: &[u8], _: &AtomicBool| {
                let job = Job::<1>::decode(bytes).unwrap();
                if scenario == "cancel" {
                    cancel.store(true, Ordering::Relaxed);
                }
                result(&job, scenario == "error")
            };
            let outcome = controller::run_observed(
                &mut restored,
                &fixture.identity(),
                3,
                1,
                MergeConfig {
                    lockstep: false,
                    ..config()
                },
                &|| Ok(()),
                &inspect,
                || stop::requested(&cancel, None),
                |_| false,
                |_, _, _| {},
                None,
                |_, _, _, _| {},
                |_| {},
            )
            .unwrap();
            let actual = (
                format!("{outcome:?}"),
                authority_sections(&restored, 3),
                restored.state.edges.log().to_vec(),
                restored.records.total(),
            );
            if let Some(expected) = &expected {
                assert_eq!(&actual, expected, "{scenario}");
            } else {
                expected = Some(actual);
            }
            drop(restored);
            let cold = fixture.open().unwrap();
            assert!(cold.rolling_diagnostics.wait_profile.is_none());
        }
    }
}
