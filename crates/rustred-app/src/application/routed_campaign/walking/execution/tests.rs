use super::*;
fn request() -> OwnerDomainWalkRequest {
    OwnerDomainWalkRequest::new(super::super::OwnerDomainMatchRequest::new(
        String::new(),
        String::new(),
    ))
}
#[test]
fn symbolic_stream_compact_counts_keep_exact_event_cap() {
    let mut request = request();
    request.max_events = 5;
    let mut state = State::new(Queue::<1>::new(5, 5), 0, None);
    state
        .accept(
            Event {
                count: 4,
                effect: Effect::Count,
            },
            &request,
        )
        .unwrap();
    assert_eq!(
        state.accept(
            Event {
                count: 3,
                effect: Effect::Count
            },
            &request
        ),
        Err("aggregate successor event allowance")
    );
    assert_eq!(state.events, 5);
    assert_eq!(state.frontiers, 0);
}
#[test]
fn symbolic_stream_frontier_cap_includes_prior_input_and_route_obligations() {
    let mut request = request();
    request.max_frontiers = 2;
    let mut state = State::new(Queue::<1>::new(5, 5), 1, None);
    let frontier = || {
        Event::one(Effect::Frontier {
            value: json!({"kind":"test"}),
            successor: false,
            conditional: false,
        })
    };
    state.accept(frontier(), &request).unwrap();
    assert_eq!(
        state.accept(frontier(), &request),
        Err("retained frontier allowance")
    );
    assert_eq!(state.frontiers, 2);
    assert_eq!(state.details.len(), 1);
    assert_eq!(state.events, 2);
}
#[test]
fn symbolic_stream_admission_preserves_phase_rank_and_pending_semantics() {
    let request = request();
    let mut state = State::new(Queue::<1>::new(5, 5), 0, None);
    for phase in [Phase::Apply, Phase::Route] {
        for rank in [Some(11), None] {
            let domain = super::super::queue::Domain {
                phase,
                owner: [true],
                lower: vec![0],
                upper: vec![None],
                rank,
            };
            state
                .accept(
                    Event::one(Effect::Admit {
                        domain,
                        successor: true,
                        conditional: true,
                    }),
                    &request,
                )
                .unwrap();
        }
    }
    assert_eq!(state.queue.domains.len(), 4);
    assert_eq!(state.queue.next, 0);
    assert_eq!(state.completed, 0);
    assert_eq!(state.successors, 4);
    assert_eq!(state.conditional, 4);
}

#[test]
fn symbolic_stream_failed_publisher_does_not_commit_contiguous_speculative_results() {
    let mut queue = Queue::<1>::new(8, 100);
    for x in 0..3 {
        queue
            .admit(super::super::queue::Domain {
                phase: Phase::Apply,
                owner: [true],
                lower: vec![x],
                upper: vec![Some(x)],
                rank: Some(11),
            })
            .unwrap();
    }
    let mut state = State::new(queue, 0, Some("first failure".into()));
    let mut leftovers = (0..3)
        .map(|id| {
            (
                id,
                Finished {
                    stats: NativeStats::Apply(Default::default()),
                    error: None,
                    error_kind: "none",
                    seconds: 0.0,
                },
            )
        })
        .collect();
    retain_leftovers(&mut state, &mut leftovers);
    assert_eq!(state.queue.next, 1);
    assert_eq!(state.completed, 0);
    assert_eq!(state.records.len(), 1);
    assert_eq!(state.uncommitted.len(), 2);
    assert_eq!(state.uncommitted[1]["id"], 2);
    assert_eq!(state.uncommitted[1]["lower"], json!([2]));
    let enriched = state.enrich(json!({"first_failure":{"domain":2,"kind":"native_failure"}}));
    assert_eq!(enriched["first_failure"]["owner"], "1");
    assert_eq!(enriched["first_failure"]["lower"], json!([2]));
    assert_eq!(enriched["first_failure"]["rank"], 11);
}

#[test]
fn symbolic_stream_panic_retains_already_committed_frontier_provenance() {
    let mut queue = Queue::<1>::new(8, 100);
    queue
        .admit(super::super::queue::Domain::route_cover([true], None))
        .unwrap();
    let mut state = State::new(queue, 1, Some("worker panicked".into()));
    state.details.push(json!({"kind":"existing obligation"}));
    retain_leftovers(&mut state, &mut Vec::new());
    assert_eq!(state.queue.next, 0);
    assert_eq!(state.uncommitted.len(), 1);
    assert_eq!(state.uncommitted[0]["partial_publisher"], true);
    assert_eq!(
        state.uncommitted[0]["frontiers"][0]["kind"],
        "existing obligation"
    );
    assert!(state.uncommitted[0]["stats"].is_null());
}
