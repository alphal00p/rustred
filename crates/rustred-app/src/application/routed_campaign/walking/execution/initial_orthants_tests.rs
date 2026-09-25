use super::*;
use std::sync::Arc;

fn request() -> OwnerDomainWalkRequest {
    OwnerDomainWalkRequest::new(super::super::OwnerDomainMatchRequest::new(
        String::new(),
        String::new(),
    ))
}
fn reused(count: usize, successor: bool, conditional: bool) -> Event<1> {
    Event {
        count,
        effect: Effect::PreAdmittedOrthantReuse {
            target: 0,
            successor,
            conditional,
        },
    }
}

#[test]
fn initial_orthants_counted_prefix_preserves_caps_flags_and_separate_counters() {
    for (successor, conditional) in [(false, false), (true, false), (true, true)] {
        for cap in 0..=6 {
            let mut request = request();
            request.max_events = cap;
            let mut state = State::new(Queue::<1>::new(5, Some(0)), 0, None);
            let result = state.accept(reused(5, successor, conditional), &request);
            let accepted = cap.min(5);
            assert_eq!(result.is_ok(), cap >= 5);
            assert_eq!(state.events, accepted);
            assert_eq!(state.successors, if successor { accepted } else { 0 });
            assert_eq!(state.conditional, if conditional { accepted } else { 0 });
            assert_eq!(state.queue.deduplicated, accepted);
            assert_eq!(state.pre_admitted_orthant_hits, accepted);
            assert_eq!(state.job_local_reuse_hits, 0);
            assert_eq!(state.queue.exact_hits, 0);
            assert_eq!(state.queue.orthant_hits, 0);
            assert_eq!(state.queue.containment_checks, 0);
            assert_eq!(state.completed, 0);
        }
    }
}

#[test]
fn initial_orthants_overflow_and_mixed_markers_keep_exact_ordered_prefix() {
    for (field, expected) in [
        (0, "successor counter overflow"),
        (1, "conditional successor counter overflow"),
        (2, "pre-admitted orthant counter overflow"),
        (3, "reuse counter overflow"),
    ] {
        let request = request();
        let mut state = State::new(Queue::<1>::new(5, None), 0, None);
        match field {
            0 => state.successors = usize::MAX - 2,
            1 => state.conditional = usize::MAX - 2,
            2 => state.pre_admitted_orthant_hits = usize::MAX - 2,
            _ => state.queue.deduplicated = usize::MAX - 2,
        }
        assert_eq!(state.accept(reused(5, true, true), &request), Err(expected));
        assert_eq!(state.events, 2);
        assert_eq!(state.successors, if field == 0 { usize::MAX } else { 2 });
        assert_eq!(state.conditional, if field == 1 { usize::MAX } else { 2 });
        assert_eq!(
            state.pre_admitted_orthant_hits,
            if field == 2 { usize::MAX } else { 2 }
        );
        assert_eq!(
            state.queue.deduplicated,
            if field == 3 { usize::MAX } else { 2 }
        );
        assert_eq!(state.job_local_reuse_hits, 0);
        assert!(state.accept(reused(1, true, true), &request).is_err());
        assert_eq!(state.events, 2);
    }
    let mut request = request();
    request.max_events = 6;
    let mut state = State::new(Queue::<1>::new(5, None), 0, None);
    state.accept(reused(2, false, false), &request).unwrap();
    state
        .accept(
            Event::one(Effect::KnownReuse {
                successor: true,
                conditional: false,
            }),
            &request,
        )
        .unwrap();
    state
        .accept(
            Event::one(Effect::Frontier {
                value: json!({"kind":"kept"}),
                successor: false,
                conditional: false,
            }),
            &request,
        )
        .unwrap();
    assert!(state.accept(reused(3, true, true), &request).is_err());
    assert_eq!(
        (state.events, state.successors, state.conditional),
        (6, 3, 2)
    );
    assert_eq!(
        (
            state.pre_admitted_orthant_hits,
            state.job_local_reuse_hits,
            state.queue.deduplicated
        ),
        (4, 1, 5)
    );
    assert_eq!(state.details[0]["kind"], "kept");
    assert_eq!(state.frontiers, 1);
}

pub(super) fn native_fixture() -> RoutedCandidateReducer<1> {
    use crate::{
        CandidateOwnerBundle, FamilyCandidatesRequest, family_candidates,
        load_generated_candidate_owners,
    };
    let source = r#"
schema="rustred.project.toml.v1"
[family]
name="initial_orthants_native_fixture"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;
    let mut generation = FamilyCandidatesRequest::new(source);
    generation.numerical_depth = 0;
    generation.max_numerator_rank = Some(2);
    let saved = family_candidates(generation).unwrap();
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
    RoutedCandidateReducer::try_new(Arc::new(programs), [], Default::default()).unwrap()
}
fn initial_state() -> State<1> {
    let mut queue = Queue::new(100, None);
    for (lower, upper, rank, expected_admission) in [
        (2, Some(2), Some(11), (0, true)),
        (3, Some(3), Some(11), (1, true)),
        (0, None, Some(11), (2, true)),
        // There are no inactive axes, so both rays have exactly R=0.
        (0, None, None, (2, false)),
    ] {
        let admission = queue
            .admit(super::super::queue::Domain {
                powers: Default::default(),
                phase: Phase::Apply,
                owner: [true],
                lower: vec![lower],
                upper: vec![upper],
                rank,
            })
            .unwrap();
        assert_eq!(admission, expected_admission);
    }
    assert_eq!(
        queue
            .admit(super::super::queue::Domain::route_cover([true], Some(11)))
            .unwrap(),
        (3, true)
    );
    assert_eq!(queue.domains.len(), 4); // Narrow pending jobs were not retired.
    assert_eq!(queue.next, 0);
    State::new(queue, 0, None)
}
fn without_seconds(mut records: Vec<Value>) -> Vec<Value> {
    for record in &mut records {
        record.as_object_mut().unwrap().remove("seconds");
    }
    records
}
fn assert_equivalent(a: &State<1>, b: &State<1>) {
    assert_eq!(a.error, b.error);
    assert_eq!(a.queue.domains, b.queue.domains);
    assert_eq!(a.queue.next, b.queue.next);
    assert_eq!(a.queue.deduplicated, b.queue.deduplicated);
    assert_eq!(
        (
            a.events,
            a.successors,
            a.conditional,
            a.frontiers,
            a.completed,
            a.routed,
            a.route_masks
        ),
        (
            b.events,
            b.successors,
            b.conditional,
            b.frontiers,
            b.completed,
            b.routed,
            b.route_masks
        )
    );
    assert_eq!(a.queue.max_finite_rank, b.queue.max_finite_rank);
    assert_eq!(
        a.queue.unbounded_rank_domains,
        b.queue.unbounded_rank_domains
    );
    assert_eq!(
        without_seconds(a.records.clone()),
        without_seconds(b.records.clone())
    );
}

#[test]
fn initial_orthants_native_off_on_full_queue_and_serial_parallel_are_equivalent() {
    let reducer = native_fixture();
    let request = request();
    let mut baseline = initial_state();
    run_with_initial_orthants(
        &mut baseline,
        &reducer,
        &request,
        &AtomicBool::new(false),
        &|_| {},
        false,
    );
    assert!(baseline.error.is_none(), "{:?}", baseline.error);
    assert_eq!(baseline.completed, 4);
    assert_eq!(baseline.queue.next, baseline.queue.domains.len());
    assert_eq!(baseline.routed, 1);
    assert_eq!(baseline.frontiers, 0);
    assert_eq!(baseline.pre_admitted_orthant_hits, 0);
    for workers in [1, 2, 6] {
        if workers > 1
            && (rustred::campaign::ParallelExecution::preflight_requested_core_budget(workers)
                .is_err()
                || !symbolica::license::LicenseManager::is_licensed())
        {
            continue;
        }
        for enabled in [false, true] {
            let mut request = request.clone();
            request.workers = workers;
            let mut state = initial_state();
            run_with_initial_orthants(
                &mut state,
                &reducer,
                &request,
                &AtomicBool::new(false),
                &|_| {},
                enabled,
            );
            assert_equivalent(&baseline, &state);
            assert_eq!(state.parallel["active_workers"], 0);
            assert_eq!(state.parallel["attempted_events"], state.events);
            assert!(state.uncommitted.is_empty());
            if enabled {
                assert!(state.pre_admitted_orthant_hits > 0);
                assert_eq!(state.queue.exact_hits + state.queue.orthant_hits, 0);
                assert_eq!(state.job_local_reuse_hits, 0);
            } else {
                assert_eq!(state.pre_admitted_orthant_hits, 0);
            }
        }
    }
}

#[test]
fn initial_orthants_native_refusal_and_event_cap_are_not_completion_shortcuts() {
    let reducer = native_fixture();
    for native_refusal in [false, true] {
        let mut request = request();
        if native_refusal {
            request.matching.match_limits.max_rules = 0;
        } else {
            request.max_events = 2;
        }
        let mut before = initial_state();
        run_with_initial_orthants(
            &mut before,
            &reducer,
            &request,
            &AtomicBool::new(false),
            &|_| {},
            false,
        );
        let mut after = initial_state();
        run_with_initial_orthants(
            &mut after,
            &reducer,
            &request,
            &AtomicBool::new(false),
            &|_| {},
            true,
        );
        assert_equivalent(&before, &after);
        assert!(after.error.is_some());
        assert_eq!(after.completed, 0);
        assert!(after.queue.next < after.queue.domains.len());
        if native_refusal {
            assert_eq!(after.events, 0);
            assert_eq!(after.pre_admitted_orthant_hits, 0);
            assert_eq!(after.parallel["first_failure"]["kind"], "native_failure");
        } else {
            assert_eq!(after.events, 2);
            assert!(after.pre_admitted_orthant_hits > 0);
            assert_eq!(
                after.parallel["first_failure"]["kind"],
                "coordinator_admission"
            );
        }
    }
}
