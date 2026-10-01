use super::*;
use crate::application::routed_campaign::walking::epoch::{
    resolve::Resolver,
    snapshot::{Publication, Snapshot},
};
use crate::application::routed_campaign::walking::inspection::{
    Effect, Event, Finished, NativeStats,
};
use crate::application::routed_campaign::walking::queue::Query;
use std::ops::ControlFlow;

mod stale_prefix;

fn resolved(job: &Job<2>, queries: &[Domain<2>], view: Option<&Snapshot<2>>) -> JobResult<2> {
    let mut resolver = view.map_or_else(Resolver::new, Resolver::with_snapshot);
    for domain in queries {
        assert_eq!(
            resolver.emit(Event::one(Effect::Admit {
                domain: domain.clone(),
                successor: domain.phase == Phase::Apply,
                conditional: false,
            })),
            ControlFlow::Continue(())
        );
    }
    resolver.finish(
        job,
        Finished {
            stats: NativeStats::Apply(rustred::solver::OwnerAppliedStats {
                events: queries.len(),
                successors: queries.iter().filter(|q| q.phase == Phase::Apply).count(),
                ..Default::default()
            }),
            error: None,
            error_kind: "none",
            seconds: 0.0,
        },
    )
}

#[test]
fn snapshot_lookup_matches_all_miss_records_edges_and_canonical_targets() {
    let mut route = boxed([0, 0], [9, 9]);
    route.phase = Phase::Route;
    let initial = [
        boxed([2, 0], [2, 0]),
        boxed([0, 0], [9, 9]),
        domain(OTHER, [0, 0], [None, None]),
        route.clone(),
    ];
    let mut correlated = domain(APPLY, [0, 0], [None, Some(0)]);
    correlated.powers.max_positive_power = Some(3);
    let mut route_query = boxed([1, 0], [3, 0]);
    route_query.phase = Phase::Route;
    let queries = [
        initial[0].clone(),
        boxed([1, 0], [5, 0]),
        domain(OTHER, [1, 0], [Some(4), None]),
        correlated,
        route_query,
        boxed([20, 0], [24, 0]),
        boxed([21, 0], [22, 0]),
        initial[0].clone(),
    ];
    let mut off = state_with(&initial);
    let mut on = state_with(&initial);
    let mut off_dispatch = Dispatch::new();
    let mut on_dispatch = Dispatch::new();
    let off_job = jobs(&mut off, &mut off_dispatch, 1).remove(0);
    let on_job = jobs(&mut on, &mut on_dispatch, 1).remove(0);
    let old = resolved(&off_job, &queries, None);
    let snapshot = on.store.snapshot(on.k).unwrap();
    assert_eq!(snapshot.domains, off.store.domains);
    assert_eq!(snapshot.domains, on.store.domains);
    let shared = on.store.snapshot(on.k).unwrap();
    assert!(
        shared.same_root(&snapshot),
        "readers share one immutable root"
    );
    drop(shared);
    let new = resolved(&on_job, &queries, Some(&snapshot));
    assert_eq!(new.job_duplicates, 1);
    assert_eq!(new.misses[0].target, Some(0), "retired exact still wins");
    assert_eq!(new.misses[1].target, Some(1), "minimum live containment");
    assert_eq!(new.misses[2].target, Some(2), "dominant orthant");
    assert_eq!(new.misses[3].target, Some(1), "nonraw correlated summary");
    assert_eq!(new.misses[4].target, Some(3), "Route keeps its own phase");
    assert_eq!(new.misses[5].target, None);
    assert_eq!(JobResult::decode(&new.encode()).unwrap(), new);
    drop(snapshot);
    let mut old_rows = Rows(Vec::new());
    let mut new_rows = Rows(Vec::new());
    merge_cut(&mut off, &mut off_dispatch, &mut old_rows, vec![old]).unwrap();
    merge_cut(&mut on, &mut on_dispatch, &mut new_rows, vec![new]).unwrap();
    assert_eq!(old_rows.0, new_rows.0);
    assert_eq!(off.store.domains, on.store.domains);
    assert_eq!(off.ledger.words(), on.ledger.words());
    assert_eq!(off.edges.log(), on.edges.log());
    assert_eq!(off.live, on.live);
    assert_eq!(off.nodes, on.nodes);
    let mut a = serde_json::to_value(off.counters).unwrap();
    let mut b = serde_json::to_value(on.counters).unwrap();
    assert!(a["miss_requests"].as_u64().unwrap() > b["miss_requests"].as_u64().unwrap());
    a.as_object_mut().unwrap().remove("miss_requests");
    b.as_object_mut().unwrap().remove("miss_requests");
    assert_eq!(a, b, "only the explicit work counter differs");
    assert_eq!(on.inspector_lookup.stored_hits, 5);
    assert_eq!(on.inspector_lookup.coordinator_miss_rechecks_skipped, 2);
    assert_eq!(off.inspector_lookup.coordinator_miss_rechecks_skipped, 0);
    assert!(
        on.verify.calls > off.verify.calls,
        "independent P2 rechecks are charged"
    );
}

#[test]
fn held_snapshot_does_not_prevent_canonical_publication_or_change_old_lookup() {
    let mut state = state_with(&[boxed([0, 0], [9, 9])]);
    let mut dispatch = Dispatch::new();
    let job = jobs(&mut state, &mut dispatch, 1).remove(0);
    let checked = merge::p1_check(&mut state, vec![result(&job, &[]).encode()], CONFIG).unwrap();
    let plan = merge::p2(&mut state, &checked).unwrap();
    let before = (
        state.store.domains.clone(),
        state.ledger.words().to_vec(),
        state.live.clone(),
    );
    let slot = Publication::new();
    slot.publish(state.store.snapshot(state.k).unwrap())
        .unwrap();
    let lease = slot.acquire().unwrap();
    slot.clear().unwrap();
    assert!(state.store.unique_mut().is_ok());
    admit_initial(&mut state, &boxed([20, 0], [21, 0])).unwrap();
    assert_eq!(lease.domains, before.0);
    assert_eq!(lease.len(), 1);
    assert_eq!(state.store.len(), 2);
    let newer = state.store.snapshot(state.k + 1).unwrap();
    assert_eq!(newer.domains, state.store.domains);
    assert_eq!(state.store.retained().0, 2);
    assert_eq!(lease.len(), 1, "held historical view remains unchanged");
    drop(newer);
    drop(lease);
    assert!(state.store.unique_mut().is_ok());
    merge::p3_preflight(&mut state, &checked, &plan, &mut Rows(Vec::new())).unwrap();
}

#[test]
fn snapshot_target_and_identity_mutations_fail_closed() {
    for mutation in 0..9 {
        let mut state = state_with(&[boxed([0, 0], [9, 9])]);
        let mut dispatch = Dispatch::new();
        let job = jobs(&mut state, &mut dispatch, 1).remove(0);
        let view = state.store.snapshot(state.k).unwrap();
        let mut r = resolved(&job, &[boxed([2, 0], [3, 0])], Some(&view));
        drop(view);
        match mutation {
            0 => r.lookup.as_mut().unwrap().version += 1,
            1 => r.lookup.as_mut().unwrap().published_len += 1,
            2 => r.lookup = None,
            3 => r.misses[0].target = Some(state.watermark()),
            4 => r.misses[0].digest ^= 1,
            5 => {
                let d = domain(OTHER, [2, 0], [Some(3), Some(0)]);
                r.misses[0].image = image(&d);
                r.misses[0].digest = image(&d).digest().0;
            }
            6 => r.lookup.as_mut().unwrap().verify.accepted += 1,
            7 => r.lookup.as_mut().unwrap().lookup.forward_candidates = u64::MAX,
            8 => r.lookup.as_mut().unwrap().seconds = f64::NAN,
            _ => unreachable!(),
        }
        let outcome = merge::p1_check(&mut state, vec![r.encode()], CONFIG)
            .and_then(|checked| merge::p2(&mut state, &checked));
        assert!(outcome.is_err(), "mutation {mutation}");
        assert_eq!(state.watermark(), 1);
        assert_eq!(state.edges.edges(), 0);
    }
    let mut state = state_with(&[boxed([0, 0], [9, 9])]);
    let job = jobs(&mut state, &mut Dispatch::new(), 1).remove(0);
    let mut old_format = result(&job, &[]).encode();
    old_format[..4].copy_from_slice(b"ERS3");
    assert!(JobResult::<2>::decode(&old_format).is_err());
}

#[test]
fn a_historical_snapshot_positive_cannot_reauthorize_a_quarantined_target() {
    let mut state = state_with(&[boxed([0, 0], [9, 9])]);
    let job = jobs(&mut state, &mut Dispatch::new(), 1).remove(0);
    let view = state.store.snapshot(state.k).unwrap();
    let result = resolved(&job, &[boxed([2, 0], [3, 0])], Some(&view));
    assert_eq!(result.misses[0].target, Some(0));
    drop(view); // Real rescue runs before new views and discards old worker bytes.
    state.store.enable_rescue_duplicates().unwrap();
    state.store.install_quarantine(vec![1]).unwrap();
    let checked = merge::p1_check(&mut state, vec![result.encode()], CONFIG).unwrap();
    assert!(
        merge::p2(&mut state, &checked).is_err(),
        "geometry alone cannot renew excluded authority"
    );
    assert_eq!(state.edges.edges(), 0);
}

#[test]
fn snapshot_raw_positive_cannot_bypass_native_summary_validity() {
    for bad_powers in [false, true] {
        let mut state = state_with(&[boxed([0, 0], [9, 9])]);
        let mut dispatch = Dispatch::new();
        let job = jobs(&mut state, &mut dispatch, 1).remove(0);
        let view = state.store.snapshot(state.k).unwrap();
        let mut r = resolved(&job, &[boxed([2, 0], [3, 0])], Some(&view));
        drop(view);
        let mut malformed = boxed([3, 0], [2, 0]);
        if bad_powers {
            malformed = boxed([2, 0], [3, 0]);
            malformed.powers.min_power_difference = Some(5);
            malformed.powers.max_power_difference = Some(4);
        }
        r.misses[0].image = image(&malformed);
        r.misses[0].digest = r.misses[0].image.digest().0;
        assert!(state.store.domains[0].contains(&r.misses[0].image));
        assert!(QueryImage::new(r.misses[0].image).is_err());
        let checked = merge::p1_check(&mut state, vec![r.encode()], CONFIG).unwrap();
        assert!(merge::p2(&mut state, &checked).is_err());
    }
}

#[test]
fn snapshot_resolver_keeps_range_summary_and_empty_phase_obligations() {
    let state = state_with(&[boxed([0, 0], [9, 9])]);
    let view = state.store.snapshot(0).unwrap();
    let job = Job {
        seq: 1,
        parent: 0,
        v0: 0,
        attempts: 0,
        flags: 0,
        image: state.store.domains[0],
    };
    for (domain, reason) in [
        (boxed([65535, 0], [65535, 0]), BreakReason::ResolverRange),
        (boxed([3, 0], [2, 0]), BreakReason::ResolverSummary),
    ] {
        let mut resolver = Resolver::with_snapshot(&view);
        assert_eq!(
            resolver.emit(Event::one(Effect::Admit {
                domain,
                successor: true,
                conditional: false
            })),
            ControlFlow::Break(())
        );
        assert_eq!(resolver.finish_panic(&job, 0.0).break_reason, reason);
    }
    let mut empty_other = domain(OTHER, [0, 0], [Some(0), Some(0)]);
    empty_other.powers.max_positive_power = Some(0);
    assert!(
        QueryImage::new(image(&empty_other))
            .unwrap()
            .core
            .is_empty()
    );
    let r = resolved(&job, &[empty_other], Some(&view));
    assert_eq!(
        r.misses[0].target, None,
        "EMPTY must not bypass owner/phase"
    );
}

/// Mutate only the lookup answer/accounting of one otherwise valid obligation.
/// A false negative is not a proof and must never directly discharge that work.
fn forge_single_miss(result: &mut JobResult<2>) {
    assert_eq!(result.misses.len(), 1);
    result.misses[0].target = None;
    let report = result.lookup.as_mut().unwrap();
    report.lookup = super::super::store::LookupCounters {
        misses: 1,
        ..Default::default()
    };
    report.verify = VerifyCounters::default();
}

#[test]
fn snapshot_miss_exact_and_retired_exact_contradictions_are_fatal() {
    for retired in [false, true] {
        let query = boxed([2, 0], [2, 0]);
        let mut initial = vec![query.clone()];
        if retired {
            initial.push(boxed([0, 0], [9, 9]));
        }
        let mut state = state_with(&initial);
        assert_eq!(state.is_live(0), !retired);
        let job = jobs(&mut state, &mut Dispatch::new(), 1).remove(0);
        let view = state.store.snapshot(state.k).unwrap();
        let mut r = resolved(&job, &[query], Some(&view));
        assert_eq!(r.misses[0].target, Some(0));
        drop(view);
        forge_single_miss(&mut r);
        let checked = merge::p1_check(&mut state, vec![r.encode()], CONFIG).unwrap();
        let error = match merge::p2(&mut state, &checked) {
            Err(error) => error,
            Ok(_) => panic!("exact contradiction admitted"),
        };
        assert!(error.0.contains("miss contradicts exact image"));
        assert_eq!(state.store.len(), initial.len());
        assert_eq!(state.edges.edges(), 0);
        assert_eq!(state.inspector_lookup.coordinator_miss_rechecks_skipped, 0);
    }
}

#[test]
fn snapshot_false_negative_subset_only_adds_unresolved_work() {
    let mut state = state_with(&[boxed([0, 0], [9, 9])]);
    let mut dispatch = Dispatch::new();
    let job = jobs(&mut state, &mut dispatch, 1).remove(0);
    let query = boxed([2, 0], [3, 0]);
    let view = state.store.snapshot(state.k).unwrap();
    let mut r = resolved(&job, &[query.clone()], Some(&view));
    assert_eq!(r.misses[0].target, Some(0));
    drop(view);
    forge_single_miss(&mut r);
    let checked = merge::p1_check(&mut state, vec![r.encode()], CONFIG).unwrap();
    let plan = merge::p2(&mut state, &checked).unwrap();
    assert_eq!(plan.survivors.len(), 1);
    assert_eq!(plan.survivors[0].image, image(&query));
    assert!(plan.transfers.is_empty());
    assert_eq!(plan.targets[0][0].into_id(1).unwrap().id(), 1);
    merge::p3_preflight(&mut state, &checked, &plan, &mut Rows(Vec::new())).unwrap();
    merge::p3_apply(
        &mut state,
        checked,
        plan,
        CONFIG,
        &records::Builder,
        &mut Rows(Vec::new()),
        &mut |id, attempts| dispatch.requeue(id, attempts),
    )
    .unwrap();
    assert_eq!(state.store.len(), 2, "extra work, not record/ID identity");
    assert!(matches!(entry(&state, 1), Entry6::Pending(_)));
    state
        .tracker
        .refresh(&std::sync::atomic::AtomicBool::new(false), true);
    assert_eq!(state.tracker.closed(0), Some(false));
    assert_eq!(state.tracker.closed(1), Some(false));
    assert_eq!(state.inspector_lookup.coordinator_miss_rechecks_skipped, 1);
}

#[test]
fn snapshot_miss_bypass_requires_current_lockstep_report_and_exact_image_check() {
    // Missing report takes the historical lookup path even if the caller
    // replaces a contained answer with None. No negative certificate is assumed.
    let mut state = state_with(&[boxed([0, 0], [9, 9])]);
    let job = jobs(&mut state, &mut Dispatch::new(), 1).remove(0);
    let view = state.store.snapshot(state.k).unwrap();
    let mut r = resolved(&job, &[boxed([2, 0], [3, 0])], Some(&view));
    drop(view);
    forge_single_miss(&mut r);
    r.lookup = None;
    let checked = merge::p1_check(&mut state, vec![r.encode()], CONFIG).unwrap();
    let plan = merge::p2(&mut state, &checked).unwrap();
    assert!(plan.survivors.is_empty());
    assert_eq!(plan.targets[0][0].into_id(1).unwrap().id(), 0);
    assert_eq!(plan.counters.inspector.coordinator_miss_rechecks_skipped, 0);

    for stale in [false, true] {
        let mut state = state_with(&[boxed([0, 0], [9, 9])]);
        let job = jobs(&mut state, &mut Dispatch::new(), 1).remove(0);
        let view = state.store.snapshot(state.k).unwrap();
        let mut r = resolved(&job, &[boxed([20, 0], [21, 0])], Some(&view));
        drop(view);
        let mut config = CONFIG;
        if stale {
            r.lookup.as_mut().unwrap().version += 1;
        } else {
            config.lockstep = false;
            // Rolling accepts an honest same-view report; the independently
            // checked miss still traverses ordinary antichain/retirement.
            let checked = merge::p1_check(&mut state, vec![r.encode()], config).unwrap();
            let plan = merge::p2(&mut state, &checked).unwrap();
            assert_eq!(plan.survivors.len(), 1);
            assert_eq!(plan.counters.inspector.coordinator_miss_rechecks_skipped, 1);
            continue;
        }
        assert!(merge::p1_check(&mut state, vec![r.encode()], config).is_err());
        assert_eq!(state.store.len(), 1);
        assert_eq!(state.inspector_lookup.coordinator_miss_rechecks_skipped, 0);
    }

    // Inject a hash collision with a different canonical image. The reused
    // exact-index API must compare the complete image, not treat a key as proof.
    let mut state = state_with(&[boxed([0, 0], [9, 9])]);
    let query = boxed([20, 0], [21, 0]);
    state
        .store
        .unique_mut()
        .unwrap()
        .exact
        .insert(image(&query).digest().0, 0);
    let job = jobs(&mut state, &mut Dispatch::new(), 1).remove(0);
    let view = state.store.snapshot(state.k).unwrap();
    let r = resolved(&job, &[query], Some(&view));
    assert_eq!(r.misses[0].target, None);
    drop(view);
    let checked = merge::p1_check(&mut state, vec![r.encode()], CONFIG).unwrap();
    let plan = merge::p2(&mut state, &checked).unwrap();
    assert_eq!(plan.survivors.len(), 1);
    assert_eq!(plan.counters.inspector.coordinator_miss_rechecks_skipped, 1);
}

#[test]
fn snapshot_mixed_cut_requeue_retains_cross_parent_antichain_and_accounting() {
    let initial = [
        boxed([0, 0], [9, 9]),
        domain(OTHER, [0, 0], [Some(9), Some(9)]),
    ];
    let queries = [
        vec![boxed([1, 0], [3, 0]), boxed([20, 0], [25, 0])],
        vec![boxed([21, 0], [24, 0]), boxed([20, 0], [25, 0])],
    ];
    let run = |snapshot: bool| {
        let mut state = state_with(&initial);
        let mut dispatch = Dispatch::new();
        let first = jobs(&mut state, &mut dispatch, 2);
        let make = |state: &EpochState<2>, batch: &[Job<2>]| {
            let view = snapshot.then(|| state.store.snapshot(state.k).unwrap());
            batch
                .iter()
                .map(|job| resolved(job, &queries[job.parent as usize], view.as_ref()))
                .collect::<Vec<_>>()
        };
        let discarded = make(&state, &first);
        let checked = merge::p1_check(
            &mut state,
            discarded.iter().map(JobResult::encode).collect(),
            CONFIG,
        )
        .unwrap();
        merge::discard_cut(&mut state, &checked, &mut |id, attempts| {
            dispatch.requeue(id, attempts)
        })
        .unwrap();
        assert_eq!(state.inspector_lookup.coordinator_miss_rechecks_skipped, 0);
        assert_eq!(state.store.len(), 2);
        let replay = jobs(&mut state, &mut dispatch, 2);
        for (before, after) in first.iter().zip(&replay) {
            assert_eq!(before.parent, after.parent);
            assert_eq!(before.image, after.image);
            assert_eq!(before.v0, after.v0);
            assert_ne!(before.seq, after.seq);
        }
        let results = make(&state, &replay);
        let mut rows = Rows(Vec::new());
        merge_cut(&mut state, &mut dispatch, &mut rows, results).unwrap();
        (state, rows)
    };
    let (off, off_rows) = run(false);
    let (on, on_rows) = run(true);
    assert_eq!(off_rows.0, on_rows.0);
    assert_eq!(off.store.domains, on.store.domains);
    assert_eq!(off.ledger.words(), on.ledger.words());
    assert_eq!(off.edges.log(), on.edges.log());
    assert_eq!(off.nodes, on.nodes);
    assert_eq!(off.live, on.live);
    assert_eq!(on.store.len(), 3, "one cross-parent antichain survivor");
    assert_eq!(on.inspector_lookup.queries, 4);
    assert_eq!(on.inspector_lookup.stored_hits, 1);
    assert_eq!(on.inspector_lookup.coordinator_miss_rechecks_skipped, 3);
    assert_eq!(off.inspector_lookup.coordinator_miss_rechecks_skipped, 0);
    assert_eq!(
        on.lookup.misses, off.lookup.misses,
        "no imaginary second miss probe"
    );
    assert!(
        on.verify.calls > off.verify.calls,
        "positive recheck still charged"
    );
}

#[test]
fn stale_snapshot_miss_rechecks_exact_and_containing_new_publications() {
    for exact in [false, true] {
        let mut state = state_with(&[boxed([10, 0], [10, 0]), boxed([20, 0], [20, 0])]);
        let mut dispatch = Dispatch::new();
        let jobs = jobs(&mut state, &mut dispatch, 2);
        let old = state.store.snapshot(0).unwrap();
        let query = boxed([30, 0], [30, 0]);
        let delayed = resolved(&jobs[0], &[query.clone()], Some(&old));
        assert_eq!(delayed.misses[0].target, None);
        let publication = if exact {
            query
        } else {
            boxed([29, 0], [31, 0])
        };
        let earlier = resolved(&jobs[1], &[publication], Some(&old));
        let config = merge::MergeConfig {
            lockstep: false,
            ..CONFIG
        };
        let checked = merge::p1_check(&mut state, vec![earlier.encode()], config).unwrap();
        let plan = merge::p2(&mut state, &checked).unwrap();
        assert_eq!(plan.survivors.len(), 1);
        let mut rows = Rows(Vec::new());
        merge::p3_preflight(&mut state, &checked, &plan, &mut rows).unwrap();
        merge::p3_apply(
            &mut state,
            checked,
            plan,
            config,
            &records::Builder,
            &mut rows,
            &mut |id, attempts| dispatch.requeue(id, attempts),
        )
        .unwrap();
        assert_eq!(state.k, 1);
        assert_eq!(
            old.len(),
            2,
            "publication cannot mutate the old lookup view"
        );
        let checked = merge::p1_check(&mut state, vec![delayed.encode()], config).unwrap();
        let plan = merge::p2(&mut state, &checked).unwrap();
        assert!(
            plan.survivors.is_empty(),
            "a stale miss is not negative authority"
        );
        assert_eq!(plan.targets[0][0].into_id(3).unwrap().id(), 2);
        assert_eq!(plan.counters.inspector.coordinator_miss_rechecks_skipped, 0);
    }
}

#[test]
fn shared_lookup_roots_allow_overlap_and_bound_retained_versions() {
    use super::super::snapshot::{MAX_LOOKUP_DELTA_BYTES, MAX_LOOKUP_LAG};
    let mut state = state_with(&[boxed([0, 0], [0, 0])]);
    let old = state.store.snapshot(0).unwrap();
    admit_initial(&mut state, &boxed([10, 0], [10, 0])).unwrap();
    let second = state.store.snapshot(1).unwrap();
    admit_initial(&mut state, &boxed([20, 0], [20, 0])).unwrap();
    let refreshed = state.store.snapshot(2).unwrap();
    assert_eq!(state.store.retained().0, 3);
    assert!(state.store.retained().1 <= MAX_LOOKUP_DELTA_BYTES);
    assert!(!state.store.publication_room(MAX_LOOKUP_LAG, 0, 0));
    assert!(
        !state
            .store
            .publication_room(2, 1, MAX_LOOKUP_DELTA_BYTES / 4 + 1)
    );
    assert!(!refreshed.same_root(&second));
    assert_eq!(second.len(), 2, "older readers are not mutated by refresh");
    assert_eq!(refreshed.domains, state.store.domains);
    assert_eq!(old.len(), 1);
    drop(old);
    drop(second);
    drop(refreshed);
    drop(state.store.snapshot(2).unwrap());
    assert_eq!(
        state.store.retained().1,
        0,
        "returned views allow complete journal reclamation"
    );
    assert!(
        state
            .store
            .publication_room(2, 1, MAX_LOOKUP_DELTA_BYTES / 4 + 1),
        "a large legal cut has a quiescent direct-publication path"
    );
}

#[test]
fn unchanged_geometry_does_not_force_a_drain_after_many_empty_cuts() {
    use super::super::snapshot::MAX_LOOKUP_LAG;
    let state = state_with(&[boxed([0, 0], [0, 0])]);
    let old = state.store.snapshot(0).unwrap();
    for version in 1..MAX_LOOKUP_LAG * 3 {
        let current = state.store.snapshot(version).unwrap();
        assert!(old.same_root(&current));
        assert_eq!(old.version, 0, "an issued lease is not relabeled");
        assert_eq!(current.version, version);
        assert!(state.store.publication_room(version, 1, 0));
    }
}

#[test]
fn interrupted_lookup_bootstrap_and_delta_replay_leave_canonical_state_saveable() {
    let mut state = state_with(&[boxed([0, 0], [0, 0])]);
    let mut checks = 0;
    assert!(
        state
            .store
            .try_snapshot_with(0, &mut || {
                checks += 1;
                if checks == 2 {
                    Err("test stop")
                } else {
                    Ok(())
                }
            })
            .is_err()
    );
    assert_eq!(state.store.retained().0, 0);
    assert_eq!(state.store.len(), 1);
    let old = state.store.snapshot(0).unwrap();
    admit_initial(&mut state, &boxed([10, 0], [10, 0])).unwrap();
    checks = 0;
    assert!(
        state
            .store
            .try_snapshot_with(1, &mut || {
                checks += 1;
                if checks == 2 {
                    Err("test stop")
                } else {
                    Ok(())
                }
            })
            .is_err()
    );
    assert_eq!(old.len(), 1);
    assert_eq!(state.store.len(), 2);
    let resumed = state.store.snapshot(1).unwrap();
    assert_eq!(resumed.domains, state.store.domains);
    let query = QueryImage::new(state.store.domains[1]).unwrap();
    let native = Query::new(query.core.clone(), query.image.phase());
    assert_eq!(
        resumed
            .lookup(
                &query,
                &native,
                resumed.len(),
                &mut Default::default(),
                &mut Default::default()
            )
            .unwrap()
            .unwrap()
            .0,
        1
    );
}

#[test]
fn quiescent_oversized_cut_rebuilds_one_shared_root_after_publication() {
    let mut state = state_with(&[boxed([0, 0], [0, 0])]);
    let mut dispatch = Dispatch::new();
    let job = jobs(&mut state, &mut dispatch, 1).remove(0);
    drop(state.store.snapshot(0).unwrap());
    let output = resolved(&job, &[boxed([0, 0], [5, 0])], None);
    let checked = merge::p1_check(&mut state, vec![output.encode()], CONFIG).unwrap();
    let plan = merge::p2(&mut state, &checked).unwrap();
    assert_eq!(plan.survivors.len(), 1);
    assert_eq!(plan.survivors[0].retire, [0]);
    let mut rows = Rows(Vec::new());
    merge::p3_preflight(&mut state, &checked, &plan, &mut rows).unwrap();
    // Exercise the quiescent oversized-journal preflight with a zero-byte
    // threshold rather than constructing eight million retirement IDs.
    state
        .store
        .prepare_direct_for_test(
            1,
            &[plan.survivors[0].digest],
            plan.survivors.iter().map(|s| s.retire.as_slice()),
        )
        .unwrap();
    merge::p3_apply(
        &mut state,
        checked,
        plan,
        CONFIG,
        &records::Builder,
        &mut rows,
        &mut |id, attempts| dispatch.requeue(id, attempts),
    )
    .unwrap();
    let first = state.store.snapshot(1).unwrap();
    assert_eq!(state.store.retained().1, 0);
    assert_eq!(first.domains, state.store.domains);
    admit_initial(&mut state, &boxed([10, 0], [10, 0])).unwrap();
    let second = state.store.snapshot(2).unwrap();
    assert_eq!(second.domains, state.store.domains);
    let q = QueryImage::new(first.domains[1]).unwrap();
    let native = Query::new(q.core.clone(), q.image.phase());
    for view in [&first, &second] {
        assert_eq!(
            view.lookup(
                &q,
                &native,
                view.len(),
                &mut Default::default(),
                &mut Default::default()
            )
            .unwrap()
            .unwrap()
            .0,
            1
        );
    }
}

#[test]
fn rescue_lookup_views_mirror_exclusions_and_legitimate_equal_replacements() {
    let domain = boxed([1, 1], [2, 2]);
    let mut state = state_with(&[domain.clone()]);
    state.store.enable_rescue_duplicates().unwrap();
    state.store.install_quarantine(vec![1]).unwrap();
    let q = QueryImage::new(state.store.domains[0]).unwrap();
    let query = Query::new(q.core.clone(), q.image.phase());
    let lookup = |view: &Snapshot<2>| {
        view.lookup(
            &q,
            &query,
            view.len(),
            &mut Default::default(),
            &mut Default::default(),
        )
        .unwrap()
        .map(|hit| hit.0)
    };
    let old = state.store.snapshot(0).unwrap();
    assert!(old.rescue_duplicates);
    assert_eq!(old.quarantine, [1]);
    assert_eq!(lookup(&old), None);
    assert_eq!(admit_initial(&mut state, &domain).unwrap(), 1);
    let new = state.store.snapshot(1).unwrap();
    assert_eq!(new.domains[0], new.domains[1]);
    assert_eq!(
        lookup(&new),
        Some(1),
        "delta replay admits the new equal representative"
    );
    assert_eq!(
        lookup(&old),
        None,
        "old immutable cut keeps its original exclusions"
    );
    assert!(
        state.store.install_quarantine(vec![0]).is_err(),
        "amendment authority cannot change across an active worker lease"
    );
    assert_eq!(state.store.quarantine, [1]);
    drop(old);
    drop(new);
    state.store.install_quarantine(vec![0]).unwrap();
    assert_eq!(
        state.store.retained().0,
        0,
        "boundary changes invalidate idle projections"
    );
    let restored = state.store.snapshot(2).unwrap();
    assert_eq!(restored.domains, state.store.domains);
    assert_eq!(
        lookup(&restored),
        Some(0),
        "bootstrap preserves historical equal groups and oldest admissible lookup"
    );
}
