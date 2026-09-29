use super::*;
use std::time::Duration;

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
