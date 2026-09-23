use super::*;
use crate::application::routed_campaign::walking::{
    initial_orthants::InitialOrthants,
    initial_overlap::{InitialOverlapIndex, InitialOverlapScope},
};

fn starting() -> State<1> {
    let mut q = Queue::with_policy(100, None, policy(2)).unwrap();
    q.delegation
        .as_mut()
        .unwrap()
        .begin_initial_admission()
        .unwrap();
    q.admit(domain(2, Some(5))).unwrap(); // Original D>=3 anchor.
    q.delegation
        .as_mut()
        .unwrap()
        .finish_initial_admission()
        .unwrap();
    q.admit(domain(6, Some(6))).unwrap(); // Reserved real job, not anchor.
    q.admit(domain(0, Some(3))).unwrap(); // Beyond H, transfer to ID3.
    q.admit(domain(0, Some(5))).unwrap(); // Full Q; inspect only D<=2.
    assert_eq!(q.delegation.as_ref().unwrap().delegated_to(2), Some(3));
    State::new(q, 0, None)
}

fn enabled_request() -> OwnerDomainWalkRequest {
    let mut r = request();
    r.scheduling_policy = policy(2);
    r.reuse_initial_d_bands = true;
    r
}

#[test]
fn initial_overlap_actual_native_residual_and_alias_match_across_workers() {
    let reducer = native_reducer();
    let mut reference = None;
    for workers in [1, 2, 6, 50] {
        if rustred::campaign::ParallelExecution::preflight_requested_core_budget(workers).is_err() {
            continue;
        }
        let mut s = starting();
        let mut r = enabled_request();
        r.workers = workers;
        execution::run(&mut s, &reducer, &r, &AtomicBool::new(false), &|_| {});
        assert!(s.error.is_none(), "{workers}: {:?}", s.error);
        assert_eq!(s.frontiers, 0);
        assert_eq!(s.queue.domains.len(), 4);
        assert_eq!(s.native_records, 3);
        let report = s.finalize_delegation().unwrap();
        assert_eq!(report["all_ledger_obligations_discharged"], true);
        assert_eq!(report["partial_initial_inspections"], 1);
        assert_eq!(report["delegated_resolved"], 1);
        let q = &s.records[3];
        assert!(q["stats"]["selected_pieces"].as_u64().unwrap() > 0);
        assert!(q["stats"]["successors"].as_u64().unwrap() > 0);
        assert_eq!(q["record_kind"], "partial_initial_overlap_inspection");
        assert_eq!(q["lower"], json!([0]));
        assert_eq!(q["upper"], json!([5]));
        assert_eq!(q["initial_overlap"]["anchor_id"], 0);
        assert_eq!(q["initial_overlap"]["cut"], 3);
        assert_eq!(q["local_inspection_finished"], false);
        assert_eq!(q["residual_inspection_finished"], true);
        assert_eq!(q["local_classification_discharged"], true);
        let records = no_seconds(json!(s.records));
        if let Some((a, b)) = &reference {
            assert_eq!(&records, a);
            assert_eq!(&report, b);
        } else {
            reference = Some((records, report));
        }
        assert!(s.uncommitted.is_empty());
    }
    assert!(reference.is_some());
    let mut full = starting();
    let mut r = enabled_request();
    r.reuse_initial_d_bands = false;
    execution::run(&mut full, &reducer, &r, &AtomicBool::new(false), &|_| {});
    assert!(full.error.is_none());
    assert_eq!(full.frontiers, 0);
    let report = full.finalize_delegation().unwrap();
    assert_eq!(report["all_ledger_obligations_discharged"], true);
    assert_eq!(report["partial_initial_inspections"], 0);
}

fn partial_finished() -> Finished {
    let powers = rustred::solver::DomainPowerBounds {
        max_power_difference: Some(2),
        ..Default::default()
    };
    let mut f = finish();
    f.stats = NativeStats::ApplyPartial(
        Default::default(),
        InitialOverlapScope {
            anchor_id: 0,
            cut: 3,
            residual_powers: powers,
        },
    );
    f
}
fn ready_partial(anchor_frontier: bool) -> State<1> {
    let mut s = starting();
    s.note_native_started(0).unwrap();
    if anchor_frontier {
        s.accept(
            Event::one(Effect::Frontier {
                value: json!({"kind":"anchor_source_guard"}),
                successor: false,
                conditional: false,
            }),
            &request(),
        )
        .unwrap();
    }
    s.commit(0, finish());
    s.note_native_started(1).unwrap();
    s.commit(1, finish());
    s.commit_delegated().unwrap();
    s.note_native_started(3).unwrap();
    s
}

#[test]
fn initial_overlap_frontier_anchor_blocks_successful_residual_and_alias() {
    let mut s = ready_partial(true);
    s.commit(3, partial_finished());
    assert_eq!(s.frontiers, 1);
    assert_eq!(s.records[0]["frontiers"][0]["kind"], "anchor_source_guard");
    let report = s.finalize_delegation().unwrap();
    assert_eq!(report["all_ledger_obligations_discharged"], false);
    assert_eq!(report["partial_initial_blocked"], 1);
    assert_eq!(report["delegated_frontier_blocked"], 1);
    assert_eq!(s.records[3]["local_classification_discharged"], false);
}

#[test]
fn initial_overlap_cleanup_cannot_discharge_error_free_buffered_residual() {
    for error in ["cancelled", "aggregate successor event allowance"] {
        let mut s = ready_partial(false);
        s.error = Some(error.into());
        let before = s.native_records;
        let mut leftovers = vec![(3, partial_finished())];
        execution::retain_leftovers(&mut s, &mut leftovers);
        assert_eq!(s.native_records, before + 1);
        assert_eq!(s.queue.next, 4);
        let report = s.finalize_delegation().unwrap();
        assert_eq!(report["all_ledger_obligations_discharged"], false);
        assert_eq!(report["partial_initial_blocked"], 1);
        assert_eq!(s.records[3]["residual_inspection_finished"], false);
        assert_eq!(s.records[3]["local_classification_discharged"], false);
    }
}

#[test]
fn initial_overlap_native_limits_are_not_reset_or_hidden_by_reuse() {
    let reducer = native_reducer();
    let s = starting();
    let initial = &s.queue.domains[..1];
    let index = InitialOverlapIndex::from_initial(initial, &AtomicBool::new(false));
    let mut r = enabled_request();
    r.matching.match_limits.max_rules = 0;
    let f = inspection::inspect(
        &reducer,
        &s.queue.domains[3],
        &r,
        &AtomicBool::new(false),
        &InitialOrthants::empty(),
        &index,
        &mut |_| ControlFlow::Continue(()),
    );
    assert!(f.error.is_some());
    assert!(f.initial_overlap_scope().is_some());
    assert_eq!(f.native_operations(), 0);
}

#[test]
fn initial_overlap_uncommitted_later_residual_keeps_scope_but_not_coverage() {
    let mut q = Queue::with_policy(100, None, policy(4)).unwrap();
    q.delegation
        .as_mut()
        .unwrap()
        .begin_initial_admission()
        .unwrap();
    q.admit(domain(2, Some(5))).unwrap();
    q.delegation
        .as_mut()
        .unwrap()
        .finish_initial_admission()
        .unwrap();
    q.admit(domain(6, Some(6))).unwrap();
    q.admit(domain(0, Some(5))).unwrap();
    let mut s = State::new(q, 0, None);
    s.note_native_started(2).unwrap();
    s.error = Some("cancelled".into());
    let mut leftovers = vec![(2, partial_finished())];
    execution::retain_leftovers(&mut s, &mut leftovers);
    assert_eq!(s.queue.next, 0);
    assert_eq!(s.native_records, 0);
    assert!(s.records.is_empty());
    assert_eq!(s.uncommitted.len(), 1);
    let record = &s.uncommitted[0];
    assert_eq!(record["committed"], false);
    assert_eq!(record["native_inspection_scope"], "low_D_residual_only");
    assert_eq!(record["initial_overlap"]["anchor_id"], 0);
    assert_eq!(record["initial_overlap"]["responsibility_published"], false);
    let report = s.finalize_delegation().unwrap();
    assert_eq!(report["partial_initial_inspections"], 0);
    assert_eq!(report["pending_native_publications"], 3);
    assert_eq!(report["all_ledger_obligations_discharged"], false);
}
