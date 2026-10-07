use super::*;
use std::time::Duration;

#[test]
fn schedule_retains_default_shape_and_separates_base_from_escrow() {
    use crate::application::routed_campaign::matching::OwnerDomainMatchRequest;
    let request = OwnerDomainWalkRequest {
        epoch_rolling: true,
        ..OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new("s".into(), "q".into()))
    };
    assert_eq!(
        schedule_json(&request, 76, 76, 16),
        json!({
            "kind":"rolling", "depth":5, "b":76, "window":76,
            "cut_size":16, "dispatch":"fifo", "publication_order":"oldest_sequence_prefix"
        })
    );
    let request = OwnerDomainWalkRequest {
        epoch_result_escrow_jobs: 32,
        epoch_result_escrow_bytes: Some(1 << 20),
        ..request
    };
    let extra = schedule_json(&request, 108, 76, 16);
    assert_eq!(extra["b"], 108);
    assert_eq!(extra["window"], 76);
    assert_eq!(extra["cut_size"], 16);
    assert_eq!(extra["depth"], 7);
    assert_eq!(extra["result_escrow_jobs"], 32);
    assert_eq!(extra["result_escrow_bytes"], 1 << 20);
    let inline = schedule_json(&request, 33, 1, 16);
    assert_eq!(inline["cut_size"], 1);
    assert_eq!(inline["depth"], 33);
}

#[test]
fn epoch_monitor_reservations_follow_execution_budget_without_duty_timings() {
    use crate::application::routed_campaign::walking::OwnerDomainWalkPublicationPolicy;
    for (workers, explicit, expected) in [
        (1, None, (1, 0, 0)),
        (50, None, (49, 0, 1)),
        (50, Some(20), (20, 29, 1)),
    ] {
        let budget = WorkerBudget::new(
            workers,
            explicit,
            None,
            OwnerDomainWalkPublicationPolicy::Epoch,
        );
        let value = reservation_json(budget);
        assert_eq!(value["inspection_worker_limit"], expected.0);
        assert_eq!(value["lookup_worker_limit"], expected.1);
        assert_eq!(value["coordinator_worker_limit"], expected.2);
        assert_eq!(value["requested_worker_budget"], workers);
        assert!(value.get("preparation_wall_seconds").is_none());
        assert!(value.get("ordered_commit_wall_seconds").is_none());
    }
}

#[test]
fn activity_fields_separate_computing_from_returned_and_unknown_inline() {
    let value = Activity {
        queued: 3,
        computing: 2,
        returned: 4,
        occupied: 9,
        ..Activity::default()
    };
    let parallel = activity_json(Some(value), "p3");
    assert_eq!(parallel["active_workers"], 2);
    assert_eq!(parallel["computing_workers"], 2);
    assert_eq!(parallel["queued_inspections"], 3);
    assert_eq!(parallel["finished_uncommitted_domains"], 4);
    assert_eq!(parallel["occupied_native_slots"], 9);
    assert_eq!(parallel["activity_observation_age_seconds"], 0.0);
    let escrow = activity_json(
        Some(Activity {
            escrow_returned: 7,
            ..value
        }),
        "p2",
    );
    assert_eq!(escrow["returned_inspections"], 4);
    assert_eq!(escrow["occupied_native_slots"], 9);
    assert_eq!(escrow["finished_uncommitted_domains"], 11);
    let inline = activity_json(
        Some(Activity {
            queued: 1,
            occupied: 1,
            inline: true,
            ..Activity::default()
        }),
        "inspect",
    );
    assert_eq!(inline["computing_workers"], Value::Null);
    assert_eq!(inline["active_workers"], Value::Null);
    assert_eq!(inline["queued_inspections"], Value::Null);
    assert_eq!(inline["activity_observation"], "inline_call_not_pollable");
    let inline_boundary = activity_json(
        Some(Activity {
            inline: true,
            ..Activity::default()
        }),
        "boundary",
    );
    assert_eq!(inline_boundary["active_workers"], Value::Null);
    assert_eq!(inline_boundary["computing_workers"], Value::Null);
    let unknown = activity_json(None, "drain_wait");
    assert_eq!(unknown["computing_workers"], Value::Null);
    assert_eq!(unknown["finished_uncommitted_domains"], Value::Null);
    assert_eq!(unknown["activity_observation_age_seconds"], Value::Null);
    let joined = activity_json(Some(Activity::default()), "joined");
    assert_eq!(joined["active_workers"], 0);
    assert_eq!(joined["finished_uncommitted_domains"], 0);
    assert_eq!(joined["workers_joined"], true);
}

#[test]
fn scalar_closure_matches_monitor_fields_without_refreshing() {
    use crate::application::routed_campaign::walking::{
        descendant_closure::Tracker,
        queue::{Domain, Phase},
    };
    let mut state = epoch::state::EpochState::<1>::new(100, 100, 100);
    for point in [0, 1] {
        epoch::admit_initial(
            &mut state,
            &Domain {
                phase: Phase::Apply,
                owner: [true],
                lower: vec![point],
                upper: vec![Some(point)],
                rank: None,
                powers: Default::default(),
            },
        )
        .unwrap();
    }
    state.p0 = state.watermark();
    state.tracker = Tracker::new(state.p0 as usize);
    state.tracker.finish(0, true, true);
    let compare = |state: &epoch::state::EpochState<1>| {
        let expected = state
            .tracker
            .json(state.watermark() as usize, state.p0 as usize);
        let actual = scalar_closure(state);
        for field in [
            "available",
            "initial_total",
            "initial_closed",
            "total_domains",
            "total_closed",
            "unresolved_domains",
            "locally_inspected",
            "dependency_edges",
            "graph_revision",
            "snapshot_revision",
            "snapshot_stale",
            "refresh_count",
            "refresh_seconds",
            "last_refresh_seconds",
            "scan_history",
            "reason",
        ] {
            assert_eq!(actual[field], expected[field], "{field}");
        }
        assert_eq!(actual["refreshed_for_result"], false);
        assert_eq!(actual["retained_storage_estimate_bytes"], Value::Null);
        actual
    };
    assert_eq!(compare(&state)["snapshot_stale"], true);
    state.tracker.refresh(&AtomicBool::new(false), true);
    assert_eq!(compare(&state)["initial_closed"], 1);
    let dispatch = epoch::dispatch::Dispatch::new();
    let live = scalar_progress(&state, &dispatch);
    assert_eq!(live["descendant_closure"]["initial_closed"], 1);
    assert_eq!(live["descendant_closure"]["refresh_count"], 1);
    assert_eq!(
        live["descendant_closure"]["scan_history"]["latest"]["total_closed"],
        1
    );
    assert_eq!(
        live["descendant_closure"]["scan_history"]["latest"]["total_domains"],
        2
    );
    assert_eq!(
        live["descendant_closure"]["refresh_policy"]["status"],
        "unchanged"
    );
    assert_eq!(live["encountered_numerator_rank"]["status"], "finite");
    assert_eq!(live["encountered_numerator_rank"]["maximum"], 0);
    assert_eq!(live["encountered_numerator_rank"]["finite_domains"], 2);
    assert_eq!(live["max_scheduled_finite_rank"], Value::Null);
    assert_eq!(
        live["unbounded_rank_domains"], 2,
        "legacy counter means no declared cap, not geometric infinity"
    );
    assert!(live["descendant_closure"]["snapshot_age_seconds"].is_number());
    assert_eq!(
        live["descendant_closure"]["refresh_policy"]["duty_bound"],
        0.01
    );
    assert_eq!(
        state.tracker.counters().refresh_count,
        1,
        "heartbeat performs no scan"
    );
    state.tracker.disable("test unavailable");
    assert_eq!(compare(&state)["unresolved_domains"], Value::Null);
    state.tracker = Tracker::new(0);
    assert_eq!(
        compare(&state)["reason"],
        "dependency domain inventory mismatch"
    );
}

#[test]
fn phase_accounting_does_not_emit_per_tiny_cut() {
    let mut time = Telemetry::new();
    let base = time.since;
    time.emitted = base;
    for index in 1..=20_000 {
        assert!(!time.change_at(
            PHASES[index % PHASES.len()],
            base + Duration::from_micros(index as u64)
        ));
    }
    assert!((time.wall.iter().sum::<f64>() - 0.020).abs() < 1e-9);
    assert!(time.change_at("checkpoint", base + Duration::from_secs(5)));
    assert!(!time.change_at("p1", base + Duration::from_secs(5)));
    assert!((time.wall.iter().sum::<f64>() - 5.0).abs() < 1e-9);
}

#[test]
fn query_role_census_keeps_116_required_and_67_auxiliary_rows() {
    // Synthetic inventory mechanics, not the frozen production query payload.
    let queries: Vec<_> = (0..183)
        .map(|index| Query {
            id: format!("row-{index}"),
            auxiliary: index >= 116,
            role_declared: true,
            owner: vec![true],
            lower: vec![0],
            upper: vec![Some(0)],
            rank: Some(0),
            powers: Default::default(),
        })
        .collect();
    let roles = Roles::new(&queries).unwrap();
    let empty = roles.json(0);
    assert_eq!(empty["required"], 116);
    assert_eq!(empty["auxiliary"], 67);
    assert_eq!(empty["unadmitted"], 183);
    let prefix = roles.json(120);
    assert_eq!(prefix["admitted_required"], 116);
    assert_eq!(prefix["admitted_auxiliary"], 4);
    assert_eq!(prefix["unadmitted"], 63);
    let full = roles.json(183);
    assert_eq!(full["required_closed"], Value::Null);
    assert_eq!(full["required_closed_evaluated"], false);
    let mut extended = roles;
    extended.append(&queries[..2]).unwrap();
    let pending = extended.json(183);
    assert_eq!(pending["required"], 118);
    assert_eq!(pending["original_required"], 116);
    assert_eq!(pending["appended_required"], 2);
    assert_eq!(pending["admitted_required"], 116);
    assert_eq!(pending["unadmitted"], 2);
    assert_eq!(pending["required_closed"], Value::Null);
    assert_eq!(extended.json(185)["admitted_required"], 118);
}

#[test]
fn observer_panic_is_a_cooperative_stop_not_engine_authority() {
    let cancel = AtomicBool::new(false);
    let failed = Cell::new(false);
    emit(
        &|_| panic!("broken progress sink"),
        &cancel,
        &failed,
        json!({"event":"checkpoint_saved"}),
    );
    assert!(cancel.load(Ordering::Acquire));
    assert!(failed.get());
    emit(
        &|_| panic!("must not be called again"),
        &cancel,
        &failed,
        Value::Null,
    );
}
