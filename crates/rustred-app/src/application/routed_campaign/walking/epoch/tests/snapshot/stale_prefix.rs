//! Native negative-prefix reuse. Helpers are deliberately unbound unless a
//! test explicitly constructs the production-only native session marker.
//! Snapshot metadata alone is never enough to enable the optimization.
use super::*;
use crate::application::routed_campaign::walking::epoch::merge::preparation::{
    Engine, Error, Limits,
};
use crate::application::routed_campaign::walking::epoch::snapshot::NativeSession;
use crate::application::routed_campaign::walking::epoch::store::LookupCounters;

const ROLLING: MergeConfig = MergeConfig {
    lockstep: false,
    ..CONFIG
};

fn native(session: u64) -> Option<NativeSession> {
    // Explicit only: neither resolved(), stale_case(), nor checked() mints it.
    Publication::<2>::native(session).native_session()
}

fn stale_case(
    initial: &[Domain<2>],
    query: &Domain<2>,
    publications: &[Domain<2>],
) -> (EpochState<2>, Dispatch, JobResult<2>) {
    let mut state = state_with(initial);
    let mut dispatch = Dispatch::new();
    let jobs = jobs(&mut state, &mut dispatch, 2);
    let view = state.store.snapshot(state.k).unwrap();
    let delayed = resolved(&jobs[0], std::slice::from_ref(query), Some(&view));
    assert_eq!(delayed.misses[0].target, None, "honest negative fixture");
    let earlier = resolved(&jobs[1], publications, Some(&view));
    drop(view);
    merge_cut_with(
        &mut state,
        &mut dispatch,
        &mut Rows(Vec::new()),
        vec![earlier],
        ROLLING,
    )
    .unwrap();
    assert_eq!(state.k, 1);
    (state, dispatch, delayed)
}

fn checked(
    state: &mut EpochState<2>,
    result: &JobResult<2>,
    binding: Option<NativeSession>,
) -> merge::Checked<2> {
    merge::p1_check_bound(state, vec![result.encode()], ROLLING, binding).unwrap()
}

fn assert_semantics(a: &merge::MergePlan<2>, b: &merge::MergePlan<2>) {
    assert_eq!(a.targets, b.targets);
    assert_eq!(a.transfers, b.transfers);
    assert_eq!(a.survivors.len(), b.survivors.len());
    for (a, b) in a.survivors.iter().zip(&b.survivors) {
        assert_eq!(a.image, b.image);
        assert_eq!(a.summary, b.summary);
        assert_eq!(a.query.core, b.query.core);
        assert_eq!(a.retire, b.retire);
    }
    assert_eq!(a.counters.miss_requests, b.counters.miss_requests);
    assert_eq!(a.counters.antichain_folded, b.counters.antichain_folded);
    assert_eq!(a.counters.verify, b.counters.verify);
    assert_eq!(a.counters.inspector.queries, b.counters.inspector.queries);
    assert_eq!(
        a.counters.inspector.stored_hits,
        b.counters.inspector.stored_hits
    );
    assert_eq!(
        a.counters.inspector.coordinator_miss_rechecks_skipped,
        b.counters.inspector.coordinator_miss_rechecks_skipped
    );
    // Physical aggregate candidates/tests are intentionally allowed to fall.
}

#[test]
fn native_stale_prefix_matches_new_exact_containment_orthant_and_no_hit() {
    let query = boxed([30, 0], [30, 0]);
    let initial = [boxed([10, 0], [10, 0]), boxed([20, 0], [20, 0])];
    let options = [
        vec![query.clone()],
        vec![boxed([29, 0], [31, 0])],
        vec![domain(APPLY, [0, 0], [None, None])],
        vec![boxed([50, 0], [50, 0])],
    ];
    for publications in options {
        let (mut state, _, result) = stale_case(&initial, &query, &publications);
        let unbound = checked(&mut state, &result, None);
        let reference = merge::p2_plan(&state, &unbound).unwrap();
        let bound = checked(&mut state, &result, native(result.seq >> 40));
        let optimized = merge::p2_plan(&state, &bound).unwrap();
        assert_semantics(&reference, &optimized);
        for helpers in [0, 1, 2] {
            let engine = Engine::new(
                helpers,
                Limits {
                    obligations: 100,
                    retirements: 100,
                },
            )
            .unwrap();
            let prepared = engine.prepare_observed(&state, &bound, || false).unwrap();
            assert_semantics(&optimized, &prepared.plan);
            assert_eq!(optimized.counters.lookup, prepared.plan.counters.lookup);
        }
    }
}

#[test]
fn native_stale_prefix_skips_old_block_inside_coarse_envelope() {
    // The two incomparable boxes have the same group signature (Amax=14,
    // Rmax=0,Dmin=10). Their shared envelope includes [5,5], although neither
    // box does. Existing forward-candidate accounting therefore observes
    // old work even when coordinate lanes reject before a native predicate.
    let left = domain([true, true], [0, 8], [Some(2), Some(10)]);
    let right = domain([true, true], [8, 0], [Some(10), Some(2)]);
    let query = domain([true, true], [5, 5], [Some(5), Some(5)]);
    let (mut state, _, result) = stale_case(
        &[left, right],
        &query,
        &[domain([true, true], [200, 0], [Some(200), Some(0)])],
    );
    let unbound = checked(&mut state, &result, None);
    let old = merge::p2_plan(&state, &unbound).unwrap();
    let bound = checked(&mut state, &result, native(result.seq >> 40));
    let new = merge::p2_plan(&state, &bound).unwrap();
    assert_semantics(&old, &new);
    assert!(old.counters.lookup.forward_candidates > new.counters.lookup.forward_candidates);
    assert_eq!(
        new.survivors.len(),
        1,
        "a negative still leaves native work"
    );
}

#[test]
fn unbound_wrong_session_and_all_miss_do_not_gain_prefix_eligibility() {
    let initial = [boxed([0, 0], [9, 9]), boxed([20, 0], [20, 0])];
    let mut state = state_with(&initial);
    let mut dispatch = Dispatch::new();
    let jobs = jobs(&mut state, &mut dispatch, 2);
    let view = state.store.snapshot(state.k).unwrap();
    let query = boxed([2, 0], [3, 0]);
    let mut delayed = resolved(&jobs[0], &[query.clone()], Some(&view));
    assert_eq!(delayed.misses[0].target, Some(0));
    forge_single_miss(&mut delayed);
    let earlier = resolved(&jobs[1], &[boxed([50, 0], [50, 0])], Some(&view));
    drop(view);
    merge_cut_with(
        &mut state,
        &mut dispatch,
        &mut Rows(Vec::new()),
        vec![earlier],
        ROLLING,
    )
    .unwrap();
    // The synthetic stale negative is healed by all fallback contexts.
    for binding in [None, Publication::<2>::new().native_session(), native(99)] {
        let c = checked(&mut state, &delayed, binding);
        let plan = merge::p2_plan(&state, &c).unwrap();
        assert!(plan.survivors.is_empty());
        assert_eq!(
            plan.targets[0][0].into_id(state.watermark()).unwrap().id(),
            0
        );
    }
    // An actual AllMiss result has no report, even with an explicit test
    // marker; the full store remains authoritative for finding its target.
    let all_miss = resolved(&jobs[0], &[query], None);
    let c = checked(&mut state, &all_miss, native(all_miss.seq >> 40));
    let plan = merge::p2_plan(&state, &c).unwrap();
    assert!(plan.survivors.is_empty());
    assert_eq!(
        plan.targets[0][0].into_id(state.watermark()).unwrap().id(),
        0
    );
}

fn lookup(state: &EpochState<2>, domain: &Domain<2>, first_id: usize) -> Option<u32> {
    let q = QueryImage::new(image(domain)).unwrap();
    let query = Query::new(q.core.clone(), q.image.phase());
    state
        .store
        .lookup_suffix_controlled(
            &q,
            &query,
            state.store.len(),
            first_id,
            &mut Default::default(),
            &mut Default::default(),
            || Ok(()),
        )
        .unwrap()
        .map(|hit| hit.0)
}

#[test]
fn suffix_keeps_global_retired_exact_orthant_and_lowest_live_winner() {
    let point = boxed([2, 0], [2, 0]);
    let mut state = state_with(&[point.clone(), boxed([0, 0], [9, 9])]);
    assert_eq!(
        lookup(&state, &point, state.store.len()),
        Some(0),
        "retired exact priority"
    );
    let full = domain(APPLY, [0, 0], [None, None]);
    let orthant = admit_initial(&mut state, &full).unwrap();
    assert_eq!(
        lookup(&state, &boxed([50, 0], [51, 0]), state.store.len()),
        Some(orthant)
    );
    assert_eq!(
        lookup(&state, &point, state.store.len()),
        Some(0),
        "exact before orthant"
    );

    let mut finite = state_with(&[boxed([50, 0], [50, 0])]);
    let prefix = finite.store.len();
    let left = admit_initial(&mut finite, &boxed([0, 0], [7, 7])).unwrap();
    admit_initial(&mut finite, &boxed([2, 0], [9, 9])).unwrap();
    let query = boxed([3, 0], [4, 0]);
    assert_eq!(lookup(&finite, &query, prefix), Some(left));
    assert_eq!(lookup(&finite, &query, prefix), lookup(&finite, &query, 0));
}

#[test]
fn rescue_session_boundary_refuses_old_result_and_falls_back_when_unbound() {
    let mut state = state_with(&[boxed([0, 0], [9, 9]), boxed([20, 0], [20, 0])]);
    state.store.enable_rescue_duplicates().unwrap();
    state.store.install_quarantine(vec![1]).unwrap();
    let mut dispatch = Dispatch::new();
    let job = jobs(&mut state, &mut dispatch, 1).remove(0);
    let query = boxed([2, 0], [3, 0]);
    let view = state.store.snapshot(state.k).unwrap();
    let old = resolved(&job, &[query.clone()], Some(&view));
    assert_eq!(old.misses[0].target, None);
    drop(view);
    state.store.install_quarantine(vec![0]).unwrap();
    // Model the existing durable replay operation, which replaces sequence
    // and version after rescue. Old result bytes cannot cross that boundary.
    state.k += 1;
    let mut replayed = job.clone();
    replayed.seq = (2 << 40) | 1;
    replayed.v0 = state.k;
    let watermark = state.watermark();
    let meta = state.in_flight.get_mut(&job.parent).unwrap();
    meta.seq = replayed.seq;
    meta.v0 = replayed.v0;
    meta.published_len = watermark;
    assert!(
        merge::p1_check_bound(&mut state, vec![old.encode()], ROLLING, native(2))
            .err()
            .unwrap()
            .0
            .contains("stale result")
    );
    let view = state.store.snapshot(state.k).unwrap();
    let fresh = resolved(&replayed, &[query], Some(&view));
    assert_eq!(fresh.misses[0].target, Some(0));
    drop(view);
    let c = checked(&mut state, &fresh, native(2));
    let plan = merge::p2_plan(&state, &c).unwrap();
    assert!(plan.survivors.is_empty());
    assert_eq!(plan.targets[0][0].into_id(watermark).unwrap().id(), 0);
}

#[test]
fn prefix_preserves_malformed_binding_digest_and_stored_target_failures() {
    let query = boxed([30, 0], [30, 0]);
    let (mut state, _, result) = stale_case(
        &[boxed([10, 0], [10, 0]), boxed([20, 0], [20, 0])],
        &query,
        &[boxed([50, 0], [50, 0])],
    );
    for mutate in 0..3 {
        let mut bad = result.clone();
        match mutate {
            0 => bad.seq += 1,
            1 => bad.lookup.as_mut().unwrap().published_len += 1,
            _ => bad.lookup.as_mut().unwrap().version += 1,
        }
        let old = merge::p1_check(&mut state, vec![bad.encode()], ROLLING)
            .err()
            .unwrap();
        let new = merge::p1_check_bound(&mut state, vec![bad.encode()], ROLLING, native(1))
            .err()
            .unwrap();
        assert_eq!(old.0, new.0);
    }
    let mut bad = result.clone();
    bad.misses[0].digest ^= 1;
    let old = checked(&mut state, &bad, None);
    let new = checked(&mut state, &bad, native(1));
    assert_eq!(
        merge::p2_plan(&state, &old).err().unwrap().0,
        merge::p2_plan(&state, &new).err().unwrap().0,
    );
    // A claimed positive remains subject to the same verifier, not prefix
    // selection. Keep report parity valid so the positive check is reached.
    let mut bad = result;
    bad.misses[0].target = Some(0);
    let work = bad.lookup.as_mut().unwrap();
    work.lookup.misses = 0;
    work.lookup.contained_hits = 1;
    work.verify.calls = 1;
    work.verify.accepted = 1;
    work.verify.raw_inclusions = 1;
    let old = checked(&mut state, &bad, None);
    let new = checked(&mut state, &bad, native(1));
    let old_error = merge::p2_plan(&state, &old).err().unwrap().0;
    let new_error = merge::p2_plan(&state, &new).err().unwrap().0;
    assert_eq!(old_error, new_error);
    assert!(new_error.contains("stored target 0 failed verify"));
}

#[test]
fn suffix_callbacks_and_cancelled_preparation_preserve_error_order() {
    let state = state_with(&[boxed([0, 0], [9, 9])]);
    let q = QueryImage::new(image(&boxed([20, 0], [20, 0]))).unwrap();
    let query = Query::new(q.core.clone(), q.image.phase());
    // Both before lookup and at its first index-group poll, cancellation is
    // propagated before a candidate is admitted. Skipped prefix callbacks are
    // not promised: the optimization legitimately removes old index work.
    for fail_at in [1, 2] {
        let mut errors = Vec::new();
        for first in [0, state.store.len()] {
            let mut calls = 0;
            let mut counts = LookupCounters::default();
            let error = state
                .store
                .lookup_suffix_controlled(
                    &q,
                    &query,
                    state.store.len(),
                    first,
                    &mut counts,
                    &mut VerifyCounters::default(),
                    || {
                        calls += 1;
                        if calls == fail_at {
                            Err("requested stop")
                        } else {
                            Ok(())
                        }
                    },
                )
                .err()
                .unwrap();
            assert_eq!(calls, fail_at);
            assert_eq!(counts, LookupCounters::default());
            errors.push(error);
        }
        assert_eq!(errors, ["requested stop", "requested stop"]);
    }
    let (mut state, _, result) = stale_case(
        &[boxed([10, 0], [10, 0]), boxed([20, 0], [20, 0])],
        &boxed([30, 0], [30, 0]),
        &[boxed([50, 0], [50, 0])],
    );
    let before = state.store.domains.clone();
    for binding in [None, native(1)] {
        let c = checked(&mut state, &result, binding);
        for helpers in [0, 1] {
            let engine = Engine::new(
                helpers,
                Limits {
                    obligations: 100,
                    retirements: 100,
                },
            )
            .unwrap();
            assert!(matches!(
                engine.prepare_observed(&state, &c, || true),
                Err(Error::Stopped)
            ));
            assert_eq!(state.store.domains, before, "no partial P2 mutation");
        }
    }
}

#[test]
fn native_context_does_not_change_existing_same_view_exact_only_semantics() {
    let mut state = state_with(&[boxed([0, 0], [9, 9])]);
    let mut dispatch = Dispatch::new();
    let job = jobs(&mut state, &mut dispatch, 1).remove(0);
    let view = state.store.snapshot(state.k).unwrap();
    let query = boxed([2, 0], [3, 0]);
    let mut result = resolved(&job, &[query], Some(&view));
    drop(view);
    forge_single_miss(&mut result);
    // Same-view negatives already use exact-only validation, independently
    // of the new private binding; retain that policy byte for byte.
    let old = checked(&mut state, &result, None);
    let new = checked(&mut state, &result, native(1));
    let old = merge::p2_plan(&state, &old).unwrap();
    let new = merge::p2_plan(&state, &new).unwrap();
    assert_semantics(&old, &new);
    assert_eq!(old.counters.lookup, new.counters.lookup);
    assert_eq!(new.counters.inspector.coordinator_miss_rechecks_skipped, 1);
    assert_eq!(new.survivors.len(), 1, "existing negative policy adds work");
}

#[test]
fn layered_native_stale_prefix_preserves_p3_records_edges_ledger_and_live_ids() {
    let run = |bound: bool| {
        let mut state = state_with(&[
            boxed([10, 0], [10, 0]),
            boxed([20, 0], [20, 0]),
            boxed([30, 0], [30, 0]),
        ]);
        let mut dispatch = Dispatch::new();
        let mut rows = Rows(Vec::new());
        let first = jobs(&mut state, &mut dispatch, 1).remove(0);
        let bootstrap = state.store.snapshot(state.k).unwrap();
        let old_box = boxed([40, 0], [44, 4]);
        let first = resolved(&first, &[old_box.clone()], Some(&bootstrap));
        merge_cut_with(&mut state, &mut dispatch, &mut rows, vec![first], ROLLING).unwrap();
        assert_eq!(state.k, 1);
        assert_eq!(state.store.domains[3], image(&old_box));
        // This is an actual layered snapshot after a publication, with its
        // older root held; the delayed result does not use a bootstrap view.
        let layered = state.store.snapshot(state.k).unwrap();
        assert!(!bootstrap.same_root(&layered));
        assert_eq!(layered.len(), 4);
        drop(bootstrap);
        let pair = jobs(&mut state, &mut dispatch, 2);
        let query = boxed([60, 0], [60, 0]);
        let delayed = resolved(&pair[0], &[query, old_box], Some(&layered));
        assert_eq!(delayed.misses[0].target, None);
        assert_eq!(delayed.misses[1].target, Some(3));
        let earlier = resolved(&pair[1], &[boxed([40, 0], [62, 4])], Some(&layered));
        merge_cut_with(&mut state, &mut dispatch, &mut rows, vec![earlier], ROLLING).unwrap();
        assert_eq!(state.k, 2);
        assert_eq!(state.store.len(), 5);
        assert!(!state.is_live(3));
        // Retired exact positives remain admissible. Only the honest negative
        // needs to find the newly appended ID4 through the aggregate suffix.
        drop(layered);
        let c = checked(&mut state, &delayed, if bound { native(1) } else { None });
        let plan = merge::p2(&mut state, &c).unwrap();
        assert_eq!(plan.targets[0][0].into_id(5).unwrap().id(), 4);
        assert_eq!(plan.targets[0][1].into_id(5).unwrap().id(), 3);
        assert!(plan.survivors.is_empty());
        merge::p3_preflight(&mut state, &c, &plan, &mut rows).unwrap();
        merge::p3_apply(
            &mut state,
            c,
            plan,
            ROLLING,
            &records::Builder,
            &mut rows,
            &mut |id, attempts| dispatch.requeue(id, attempts),
        )
        .unwrap();
        (state, rows)
    };
    let (old, old_rows) = run(false);
    let (new, new_rows) = run(true);
    assert_eq!(old_rows.0, new_rows.0);
    assert_eq!(old.store.domains, new.store.domains);
    assert_eq!(old.ledger.words(), new.ledger.words());
    assert_eq!(old.edges.log(), new.edges.log());
    assert_eq!(old.edges.records_digest(), new.edges.records_digest());
    assert_eq!(old.live, new.live);
    assert_eq!(old.nodes, new.nodes);
    assert_eq!(
        serde_json::to_value(old.counters).unwrap(),
        serde_json::to_value(new.counters).unwrap()
    );
}

#[test]
fn bound_stale_false_negative_adds_pending_work_not_coverage() {
    let mut state = state_with(&[boxed([0, 0], [9, 9]), boxed([20, 0], [20, 0])]);
    let mut dispatch = Dispatch::new();
    let pair = jobs(&mut state, &mut dispatch, 2);
    let view = state.store.snapshot(state.k).unwrap();
    let query = boxed([2, 0], [3, 0]);
    let mut delayed = resolved(&pair[0], &[query.clone()], Some(&view));
    assert_eq!(delayed.misses[0].target, Some(0));
    forge_single_miss(&mut delayed);
    let earlier = resolved(&pair[1], &[boxed([50, 0], [50, 0])], Some(&view));
    drop(view);
    let mut rows = Rows(Vec::new());
    merge_cut_with(&mut state, &mut dispatch, &mut rows, vec![earlier], ROLLING).unwrap();
    let c = checked(&mut state, &delayed, native(1));
    let plan = merge::p2(&mut state, &c).unwrap();
    assert_eq!(plan.survivors.len(), 1);
    assert_eq!(plan.survivors[0].image, image(&query));
    assert_eq!(plan.targets[0][0].into_id(3).unwrap().id(), 3);
    merge::p3_preflight(&mut state, &c, &plan, &mut rows).unwrap();
    merge::p3_apply(
        &mut state,
        c,
        plan,
        ROLLING,
        &records::Builder,
        &mut rows,
        &mut |id, attempts| dispatch.requeue(id, attempts),
    )
    .unwrap();
    assert!(matches!(entry(&state, 3), Entry6::Pending(_)));
    state.tracker.refresh(&AtomicBool::new(false), true);
    assert_eq!(state.tracker.closed(0), Some(false));
    assert_eq!(state.tracker.closed(3), Some(false));
}
