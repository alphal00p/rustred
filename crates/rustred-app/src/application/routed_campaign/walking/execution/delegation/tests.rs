use super::*;
use crate::application::routed_campaign::walking::{
    execution,
    queue::{Domain, Phase},
};
use std::num::NonZeroUsize;

fn policy(h: usize) -> SchedulingPolicy {
    SchedulingPolicy::TransferUnreserved {
        lookahead: NonZeroUsize::new(h).unwrap(),
    }
}
fn domain(lower: u64, upper: Option<u64>) -> Domain<1> {
    Domain {
        phase: Phase::Apply,
        owner: [true],
        lower: vec![lower],
        upper: vec![upper],
        rank: Some(11),
        powers: Default::default(),
    }
}
fn initial(policy: SchedulingPolicy) -> State<1> {
    let mut queue = Queue::with_policy(100, None, policy).unwrap();
    queue.admit(domain(2, Some(2))).unwrap();
    queue.admit(domain(3, Some(3))).unwrap();
    queue.admit(domain(0, None)).unwrap();
    State::new(queue, 0, None)
}
fn finish() -> Finished {
    Finished {
        stats: NativeStats::Apply(Default::default()),
        error: None,
        error_kind: "native",
        seconds: 0.0,
    }
}
fn publish_first_and_alias(state: &mut State<1>) {
    state.note_native_started(0).unwrap();
    state.commit(0, finish());
    state.commit_delegated().unwrap();
    assert_eq!(state.queue.next, 2);
}
fn request() -> OwnerDomainWalkRequest {
    OwnerDomainWalkRequest::new(crate::OwnerDomainMatchRequest::new(
        String::new(),
        String::new(),
    ))
}

#[test]
fn delegated_record_does_not_charge_events_or_fake_native_failure() {
    let mut state = initial(policy(1));
    publish_first_and_alias(&mut state);
    assert_eq!(state.native_records, 1);
    assert_eq!(state.completed, 1);
    assert_eq!(state.events, 0);
    assert_eq!(state.records[1]["record_kind"], "delegated_not_inspected");
    assert_eq!(state.records[1]["local_inspection_finished"], false);
    assert!(state.records[1].get("stats").is_none());
    assert!(state.records[1].get("seconds").is_none());
    assert_eq!(state.finalize_delegation().unwrap()["delegated_pending"], 1);
}

#[test]
fn native_frontier_is_retained_and_blocks_the_containing_representative() {
    let mut state = initial(policy(1));
    publish_first_and_alias(&mut state);
    state.note_native_started(2).unwrap();
    state
        .accept(
            Event::one(Effect::Frontier {
                value: json!({"kind":"source_validity_obligation", "witness":"preserved"}),
                successor: false,
                conditional: false,
            }),
            &request(),
        )
        .unwrap();
    state.commit(2, finish());
    assert_eq!(state.completed, 2); // Native completion is not frontier discharge.
    assert_eq!(state.frontiers, 1);
    assert_eq!(state.records[2]["frontiers"][0]["witness"], "preserved");
    let result = state.finalize_delegation().unwrap();
    assert_eq!(result["native_frontier_blocked"], 1);
    assert_eq!(result["delegated_frontier_blocked"], 1);
    assert_eq!(result["all_ledger_obligations_discharged"], false);
}

#[test]
fn cleanup_error_free_buffered_finished_never_erases_outer_failure_or_cancellation() {
    for error in ["coordinator event allowance", "cancelled"] {
        let mut state = initial(policy(1));
        publish_first_and_alias(&mut state);
        state.note_native_started(2).unwrap();
        state.error = Some(error.into());
        let before_cursor = state.queue.next;
        let before_native = state.native_records;
        let mut leftovers = vec![(2, finish())]; // Native succeeded, stream publication did not.
        execution::retain_leftovers(&mut state, &mut leftovers);
        assert_eq!(state.queue.next - before_cursor, 1);
        assert_eq!(state.native_records - before_native, 1);
        assert_eq!(state.completed, 1);
        assert_eq!(state.native_records, 2);
        assert_eq!(state.error.as_deref(), Some(error));
        let result = state.finalize_delegation().unwrap();
        assert_eq!(result["all_ledger_obligations_discharged"], false);
        let field = if error == "cancelled" {
            "delegated_cancelled"
        } else {
            "delegated_failure_blocked"
        };
        assert_eq!(result[field], 1);
    }
}

#[test]
fn cleanup_never_advances_alias_cursor_and_retains_later_real_attempt() {
    let mut queue = Queue::with_policy(100, None, policy(3)).unwrap();
    for power in 2..=5 {
        queue.admit(domain(power, Some(power))).unwrap();
    }
    queue.admit(domain(0, None)).unwrap();
    let mut state = State::new(queue, 0, None);
    for id in 0..2 {
        state.note_native_started(id).unwrap();
        state.commit(id, finish());
    }
    state.note_native_started(2).unwrap();
    state.note_native_started(4).unwrap(); // In fence while native ID 2 publishes.
    state.commit(2, finish()); // Cursor 3 is aliased to the actual running ID 4.
    state.error = Some("cancelled".into());
    let before = (state.queue.next, state.native_records);
    let mut leftovers = vec![(4, finish())];
    execution::retain_leftovers(&mut state, &mut leftovers);
    assert_eq!((state.queue.next, state.native_records), before);
    assert_eq!(state.uncommitted.len(), 1);
    assert_eq!(state.uncommitted[0]["id"], 4);
    assert_eq!(state.uncommitted[0]["committed"], false);
    assert!(state.current_is_delegated());
    assert_eq!(state.finalize_delegation().unwrap()["delegated_pending"], 1);
}

#[test]
fn streamed_successor_admission_installs_transfer_only_at_canonical_accept() {
    let mut queue = Queue::with_policy(100, None, policy(1)).unwrap();
    queue.admit(domain(2, Some(2))).unwrap();
    queue.admit(domain(3, Some(3))).unwrap();
    let mut state = State::new(queue, 0, None);
    state.note_native_started(0).unwrap();
    assert!(!state.current_is_delegated());
    state
        .accept(
            Event::one(Effect::Admit {
                domain: domain(0, None),
                successor: true,
                conditional: true,
            }),
            &request(),
        )
        .unwrap();
    assert_eq!(
        state.queue.delegation.as_ref().unwrap().delegated_to(1),
        Some(2)
    );
    assert_eq!(
        (state.events, state.successors, state.conditional),
        (1, 1, 1)
    );
    state.commit(0, finish());
    state.commit_delegated().unwrap();
    assert_eq!(state.native_records, 1);
    assert_eq!(state.finalize_delegation().unwrap()["delegated_pending"], 1);
}

#[test]
fn failed_streamed_admission_preserves_old_index_and_ledger_obligations() {
    let mut queue = Queue::with_policy(2, None, policy(1)).unwrap();
    queue.admit(domain(2, Some(2))).unwrap();
    queue.admit(domain(3, Some(3))).unwrap();
    let mut state = State::new(queue, 0, None);
    state.note_native_started(0).unwrap();
    let error = state
        .accept(
            Event::one(Effect::Admit {
                domain: domain(0, None),
                successor: true,
                conditional: false,
            }),
            &request(),
        )
        .unwrap_err();
    assert_eq!(error, "scheduled domain allowance");
    assert_eq!(state.events, 1);
    assert_eq!(state.queue.domains.len(), 2);
    assert_eq!(state.queue.containment_retired_candidates, 0);
    assert_eq!(state.queue.delegation.as_ref().unwrap().transfer_count(), 0);
    state.error = Some(error.into());
    state.commit(0, finish());
    assert_eq!(
        state.finalize_delegation().unwrap()["all_ledger_obligations_discharged"],
        false
    );
}

#[test]
fn event_cap_still_charges_native_prefix_and_keeps_frontier_payload() {
    let mut state = initial(policy(1));
    publish_first_and_alias(&mut state);
    state.note_native_started(2).unwrap();
    let mut request = request();
    request.max_events = 1;
    state
        .accept(
            Event::one(Effect::Frontier {
                value: json!({"kind":"kept_at_cap"}),
                successor: true,
                conditional: true,
            }),
            &request,
        )
        .unwrap();
    let error = state
        .accept(Event::one(Effect::Count), &request)
        .unwrap_err();
    state.error = Some(error.into());
    state.commit(2, finish());
    assert_eq!(state.events, 1);
    assert_eq!(state.frontiers, 1);
    assert_eq!(state.records[2]["frontiers"][0]["kind"], "kept_at_cap");
    assert_eq!(
        state.finalize_delegation().unwrap()["all_ledger_obligations_discharged"],
        false
    );
}

fn native_reducer() -> RoutedCandidateReducer<1> {
    use crate::{
        CandidateOwnerBundle, FamilyCandidatesRequest, family_candidates,
        load_generated_candidate_owners,
    };
    let source = r#"
schema="rustred.project.toml.v1"
[family]
name="delegation_native_control"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;
    let mut request = FamilyCandidatesRequest::new(source);
    request.numerical_depth = 0;
    request.max_numerator_rank = Some(2);
    let saved = family_candidates(request).unwrap();
    let owner = rustred::sector::Mask::try_new([true]).unwrap();
    let (_, programs) = load_generated_candidate_owners::<1>(
        &[CandidateOwnerBundle {
            bytes: saved.bundle(),
            owner_sector: &owner,
        }],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    RoutedCandidateReducer::try_new(std::sync::Arc::new(programs), [], Default::default()).unwrap()
}
fn no_seconds(mut value: Value) -> Value {
    fn strip(v: &mut Value) {
        match v {
            Value::Object(o) => {
                o.remove("seconds");
                for v in o.values_mut() {
                    strip(v);
                }
            }
            Value::Array(a) => a.iter_mut().for_each(strip),
            _ => {}
        }
    }
    strip(&mut value);
    value
}

#[test]
fn native_walk_transfer_runs_serial_two_six_fifty_and_default_keeps_every_job() {
    let reducer = native_reducer();
    let mut baseline = initial(SchedulingPolicy::InspectAll);
    let request = request();
    execution::run(
        &mut baseline,
        &reducer,
        &request,
        &AtomicBool::new(false),
        &|_| {},
    );
    assert!(baseline.error.is_none(), "{:?}", baseline.error);
    assert_eq!(baseline.frontiers, 0);
    assert_eq!(baseline.completed, 3);
    assert!(baseline.finalize_delegation().is_none());
    let mut expected = None;
    for workers in [1, 2, 6, 50] {
        if let Err(error) =
            rustred::campaign::ParallelExecution::preflight_requested_core_budget(workers)
        {
            eprintln!("native delegation control: skipping {workers} workers: {error}");
            continue;
        }
        let mut state = initial(policy(1));
        let mut request = request.clone();
        request.workers = workers;
        request.scheduling_policy = policy(1);
        execution::run(
            &mut state,
            &reducer,
            &request,
            &AtomicBool::new(false),
            &|_| {},
        );
        assert!(state.error.is_none(), "{workers}: {:?}", state.error);
        assert_eq!(state.frontiers, 0);
        assert_eq!(state.completed, 2);
        assert_eq!(state.native_records, 2);
        assert_eq!(state.queue.next, state.queue.domains.len());
        let result = state.finalize_delegation().unwrap();
        assert_eq!(result["all_ledger_obligations_discharged"], true);
        assert_eq!(result["delegated_resolved"], 1);
        let records = no_seconds(json!(state.records));
        if let Some((ref a, ref b)) = expected {
            assert_eq!(&records, a, "{workers} workers");
            assert_eq!(&result, b, "{workers} workers");
        } else {
            expected = Some((records, result));
        }
        assert!(state.uncommitted.is_empty());
    }
}

#[test]
fn native_walk_cancellation_at_alias_retains_unresolved_representative() {
    let reducer = native_reducer();
    for workers in [1, 2, 6] {
        if rustred::campaign::ParallelExecution::preflight_requested_core_budget(workers).is_err() {
            continue;
        }
        let mut state = initial(policy(1));
        let mut request = request();
        request.workers = workers;
        request.scheduling_policy = policy(1);
        let cancelled = AtomicBool::new(false);
        execution::run(&mut state, &reducer, &request, &cancelled, &|event| {
            if event["event"] == "domain_delegated" {
                cancelled.store(true, Ordering::Release);
            }
        });
        assert!(state.error.is_some());
        assert!(state.current_is_delegated() || state.queue.next == 2);
        let result = state.finalize_delegation().unwrap();
        assert_eq!(result["all_ledger_obligations_discharged"], false);
        assert_eq!(result["delegated_pending"], 1);
    }
}
