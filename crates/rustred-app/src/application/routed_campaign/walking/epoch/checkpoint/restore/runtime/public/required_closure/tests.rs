use super::*;
use crate::application::routed_campaign::walking::descendant_closure::Tracker;
use std::sync::atomic::AtomicBool;

fn query(id: &str, low: u64, high: u64, auxiliary: bool) -> Query {
    Query {
        id: id.into(),
        auxiliary,
        role_declared: true,
        owner: vec![true],
        lower: vec![low],
        upper: vec![Some(high)],
        rank: Some(0),
        powers: Default::default(),
    }
}

fn domain(query: &Query, phase: Phase) -> Domain<1> {
    Domain {
        phase,
        owner: [true],
        lower: query.lower.clone(),
        upper: query.upper.clone(),
        rank: query.rank,
        powers: query.powers,
    }
}

fn state(domains: &[Domain<1>]) -> epoch::state::EpochState<1> {
    let mut state = epoch::state::EpochState::new(100, 100, 100);
    for domain in domains {
        epoch::admit_initial(&mut state, domain).unwrap();
    }
    state.p0 = state.watermark();
    state.tracker = Tracker::new(state.p0 as usize);
    state
}

fn observe(
    observer: &mut Observation<1>,
    queries: &[Query],
    inputs: &[Value],
    state: &epoch::state::EpochState<1>,
) -> Value {
    observer.inputs(inputs, |id| {
        state.store.domains.get(id).map(|image| image.expand())
    });
    let roles = super::super::Roles::new(queries).unwrap();
    let mut census = roles.json(observer.admitted());
    observer.attach(&mut census, state);
    census
}

#[test]
fn required_closure_waits_for_completed_scan_and_reuses_snapshot() {
    let queries = vec![query("required", 0, 0, false)];
    let inputs = vec![json!({"id":"required","domain":0})];
    let mut state = state(&[domain(&queries[0], Phase::Apply)]);
    let mut observer = Observation::new(queries.iter()).unwrap();
    state.tracker.finish(0, true, true);
    let before = observe(&mut observer, &queries, &inputs, &state);
    assert_eq!(before["required_closed"], Value::Null);
    assert_eq!(before["required_closed_evaluated"], false);
    state.tracker.refresh(&AtomicBool::new(false), true);
    let after = observe(&mut observer, &queries, &inputs, &state);
    assert_eq!(after["required_closed"], 1);
    assert_eq!(after["required_closure"]["snapshot_stale"], false);
    for _ in 0..100 {
        assert_eq!(
            observe(&mut observer, &queries, &inputs, &state)["required_closed"],
            1
        );
    }
    assert_eq!(observer.evaluations, 1, "heartbeats must reuse the census");
    assert_eq!(
        state.tracker.counters().refresh_count,
        1,
        "observer never scans"
    );
}

#[test]
fn auxiliary_containing_root_can_cover_required_and_route_phase_matches_certifier() {
    let queries = vec![query("required", 1, 1, false), query("helper", 0, 2, true)];
    let inputs = vec![
        json!({"id":"required","domain":0}),
        json!({"id":"helper","domain":1}),
    ];
    let mut state = state(&[
        domain(&queries[0], Phase::Route),
        domain(&queries[1], Phase::Route),
    ]);
    assert_eq!(state.watermark(), 2);
    // The own root remains unsealed (e.g. a blocked ancestry); the independently
    // closed wider helper is the final certifier's valid containing witness.
    state.tracker.finish(1, true, true);
    state.tracker.refresh(&AtomicBool::new(false), true);
    let mut observer = Observation::new(queries.iter()).unwrap();
    let actual = observe(&mut observer, &queries, &inputs, &state);
    assert_eq!(actual["required"], 1);
    assert_eq!(actual["auxiliary"], 1);
    assert_eq!(actual["required_closed"], 1);
    let expected = rescue::query_certification(
        &queries
            .iter()
            .map(|q| (q.id.as_str(), q.auxiliary))
            .collect::<Vec<_>>(),
        &queries
            .iter()
            .map(|q| domain(q, Phase::Apply))
            .collect::<Vec<_>>(),
        &inputs,
        |id| state.store.domains.get(id).map(|image| image.expand()),
        |id| state.tracker.closed(id),
    );
    assert_eq!(
        actual["required_closed"],
        expected["required_queries_certified"]
    );
}

#[test]
fn cumulative_amendment_is_counted_beyond_the_base_prefix() {
    let queries = vec![query("base", 0, 0, false), query("extended", 1, 1, false)];
    let inputs = vec![
        json!({"id":"base","domain":0}),
        json!({"id":"extended","domain":1,"amendment":1}),
    ];
    let mut state = state(&[
        domain(&queries[0], Phase::Apply),
        domain(&queries[1], Phase::Apply),
    ]);
    state.p0 = 1;
    state.tracker = Tracker::new(1);
    state.tracker.discovered(2);
    state.tracker.finish(0, true, true);
    state.tracker.refresh(&AtomicBool::new(false), true);
    let mut observer = Observation::new(queries.iter()).unwrap();
    let partial = observe(&mut observer, &queries, &inputs[..1], &state);
    assert_eq!(partial["required"], 2);
    assert_eq!(partial["admitted_required"], 1);
    assert_eq!(partial["required_closed"], 1);
    let admitted = observe(&mut observer, &queries, &inputs, &state);
    assert_eq!(admitted["required_closed"], 1);
    assert_eq!(admitted["admitted_required"], 2);
    state.tracker.finish(1, true, true);
    let stale = observe(&mut observer, &queries, &inputs, &state);
    assert_eq!(
        stale["required_closed"], 1,
        "unscanned local completion is not recursive closure"
    );
    assert_eq!(stale["required_closure"]["snapshot_stale"], true);
    state.tracker.refresh(&AtomicBool::new(false), true);
    let closed = observe(&mut observer, &queries, &inputs, &state);
    assert_eq!(closed["required_closed"], 2);
    assert_eq!(
        state.tracker.counters().initial_closed,
        1,
        "base prefix stays unchanged"
    );
}

#[test]
fn alias_query_rows_count_separately_but_unadmitted_rows_do_not_close() {
    let queries = vec![query("one", 0, 0, false), query("two", 0, 0, false)];
    let inputs = vec![
        json!({"id":"one","domain":0}),
        json!({"id":"two","domain":0,"amendment":1}),
    ];
    let mut state = state(&[domain(&queries[0], Phase::Apply)]);
    state.tracker.finish(0, true, true);
    state.tracker.refresh(&AtomicBool::new(false), true);
    let mut observer = Observation::new(queries.iter()).unwrap();
    assert_eq!(
        observe(&mut observer, &queries, &inputs[..1], &state)["required_closed"],
        1
    );
    assert_eq!(
        observe(&mut observer, &queries, &inputs, &state)["required_closed"],
        2
    );
    assert_eq!(observer.indexed_roots.len(), 1);
}

#[test]
fn partial_cover_does_not_invent_a_union_certificate() {
    let queries = vec![
        query("spanning", 0, 2, false),
        query("left", 0, 0, true),
        query("right", 2, 2, true),
    ];
    let inputs = vec![
        json!({"id":"spanning","domain":null}),
        json!({"id":"left","domain":0}),
        json!({"id":"right","domain":1}),
    ];
    let mut state = state(&[
        domain(&queries[1], Phase::Apply),
        domain(&queries[2], Phase::Apply),
    ]);
    for root in 0..2 {
        state.tracker.finish(root, true, true);
    }
    state.tracker.refresh(&AtomicBool::new(false), true);
    let mut observer = Observation::new(queries.iter()).unwrap();
    assert_eq!(
        observe(&mut observer, &queries, &inputs, &state)["required_closed"],
        0
    );
}

#[test]
fn unavailable_tracker_invalidates_a_previously_good_census() {
    let queries = vec![query("required", 0, 0, false)];
    let inputs = vec![json!({"id":"required","domain":0})];
    let mut state = state(&[domain(&queries[0], Phase::Apply)]);
    state.tracker.finish(0, true, true);
    state.tracker.refresh(&AtomicBool::new(false), true);
    let mut observer = Observation::new(queries.iter()).unwrap();
    assert_eq!(
        observe(&mut observer, &queries, &inputs, &state)["required_closed"],
        1
    );
    state.tracker.disable("test unavailable");
    let unavailable = observe(&mut observer, &queries, &inputs, &state);
    assert_eq!(unavailable["required_closed"], Value::Null);
    assert_eq!(unavailable["required_closure"]["available"], false);
}

#[test]
fn restored_observer_rebuilds_from_inputs_without_a_new_closure_scan() {
    let queries = vec![query("required", 0, 0, false)];
    let inputs = vec![json!({"id":"required","domain":0})];
    let mut state = state(&[domain(&queries[0], Phase::Apply)]);
    state.tracker.finish(0, true, true);
    state.tracker.refresh(&AtomicBool::new(false), true);
    let counters = state.tracker.counters();
    let flags = state.tracker.node_flags().collect();
    state.tracker = Tracker::from_owned_parts(counters, flags, std::iter::empty()).unwrap();
    let mut observer = Observation::new(queries.iter()).unwrap();
    let restored = observe(&mut observer, &queries, &inputs, &state);
    assert_eq!(restored["required_closed"], 1);
    assert_eq!(
        restored["required_closure"]["snapshot_age_seconds"],
        Value::Null
    );
    assert_eq!(state.tracker.counters().refresh_count, 1);
}

#[test]
fn cancelled_scan_keeps_the_previous_required_snapshot() {
    let queries = vec![query("required", 0, 0, false)];
    let inputs = vec![json!({"id":"required","domain":0})];
    let mut state = state(&[domain(&queries[0], Phase::Apply)]);
    state.tracker.refresh(&AtomicBool::new(false), true);
    let mut observer = Observation::new(queries.iter()).unwrap();
    assert_eq!(
        observe(&mut observer, &queries, &inputs, &state)["required_closed"],
        0
    );
    state.tracker.finish(0, true, true);
    state.tracker.refresh(&AtomicBool::new(true), true);
    let stale = observe(&mut observer, &queries, &inputs, &state);
    assert_eq!(stale["required_closed"], 0);
    assert_eq!(stale["required_closure"]["snapshot_stale"], true);
    assert_eq!(observer.evaluations, 1);
}

#[test]
fn empty_query_waits_for_native_root_discharge() {
    let mut empty = query("empty", 0, 0, false);
    empty.powers.min_power_difference = Some(2); // The fixed n=1 violates D>=2.
    let queries = vec![empty];
    let inputs = vec![json!({"id":"empty","domain":0})];
    let mut state = state(&[domain(&queries[0], Phase::Apply)]);
    state.tracker.refresh(&AtomicBool::new(false), true);
    let mut observer = Observation::new(queries.iter()).unwrap();
    assert_eq!(
        observe(&mut observer, &queries, &inputs, &state)["required_closed"],
        0
    );
    state.tracker.finish(0, true, true);
    state.tracker.refresh(&AtomicBool::new(false), true);
    let actual = observe(&mut observer, &queries, &inputs, &state);
    let expected = rescue::query_certification(
        &[("empty", false)],
        &[domain(&queries[0], Phase::Apply)],
        &inputs,
        |id| state.store.domains.get(id).map(|image| image.expand()),
        |id| state.tracker.closed(id),
    );
    assert_eq!(
        actual["required_closed"],
        expected["required_queries_certified"]
    );
    assert_eq!(actual["required_closed"], 1);
}

#[test]
fn invalid_inventory_or_disabled_observer_does_not_claim_zero() {
    let queries = vec![query("required", 0, 0, false)];
    let state = state(&[domain(&queries[0], Phase::Apply)]);
    let mut observer = Observation::new(queries.iter()).unwrap();
    let wrong = vec![json!({"id":"other","domain":0})];
    let observed = observe(&mut observer, &queries, &wrong, &state);
    assert_eq!(observed["required_closed"], Value::Null);
    assert_eq!(
        observed["required_closure"]["reason"],
        "required-query input identity mismatch"
    );
    let mut disabled = Observation::disabled(1, "test allocation unavailable");
    let observed = observe(&mut disabled, &queries, &wrong, &state);
    assert_eq!(observed["admitted_required"], 1);
    assert_eq!(observed["required_closed"], Value::Null);
    assert_eq!(observed["required_closure"]["requested_query_rows"], 1);
}
