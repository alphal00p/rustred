use super::*;
fn request() -> OwnerDomainWalkRequest {
    OwnerDomainWalkRequest::new(super::super::OwnerDomainMatchRequest::new(
        String::new(),
        String::new(),
    ))
}

#[test]
fn job_local_reuse_counted_runs_preserve_every_event_cap_prefix() {
    for (successor, conditional) in [(false, false), (true, false), (true, true)] {
        for cap in 0..=6 {
            let mut request = request();
            request.max_events = cap;
            let mut state = State::new(Queue::<1>::new(5, Some(5)), 0, None);
            let result = state.accept(
                Event {
                    count: 5,
                    effect: Effect::KnownReuse {
                        successor,
                        conditional,
                    },
                },
                &request,
            );
            let accepted = cap.min(5);
            assert_eq!(result.is_ok(), cap >= 5);
            assert_eq!(state.events, accepted);
            assert_eq!(state.successors, if successor { accepted } else { 0 });
            assert_eq!(state.conditional, if conditional { accepted } else { 0 });
            assert_eq!(state.queue.deduplicated, accepted);
            assert_eq!(state.job_local_reuse_hits, accepted);
            assert_eq!(state.queue.exact_hits, 0);
            assert_eq!(state.queue.orthant_hits, 0);
            assert_eq!(state.completed, 0);
        }
    }
}

#[test]
fn job_local_reuse_overflow_keeps_sequential_prefix_and_atomic_counters() {
    for (field, expected) in [
        (0, "successor counter overflow"),
        (1, "conditional successor counter overflow"),
        (2, "job-local reuse counter overflow"),
        (3, "reuse counter overflow"),
    ] {
        let request = request();
        let mut state = State::new(Queue::<1>::new(5, Some(5)), 0, None);
        match field {
            0 => state.successors = usize::MAX - 2,
            1 => state.conditional = usize::MAX - 2,
            2 => state.job_local_reuse_hits = usize::MAX - 2,
            _ => state.queue.deduplicated = usize::MAX - 2,
        }
        assert_eq!(
            state.accept(
                Event {
                    count: 5,
                    effect: Effect::KnownReuse {
                        successor: true,
                        conditional: true
                    }
                },
                &request
            ),
            Err(expected)
        );
        assert_eq!(state.events, 2);
        assert_eq!(state.successors, if field == 0 { usize::MAX } else { 2 });
        assert_eq!(state.conditional, if field == 1 { usize::MAX } else { 2 });
        assert_eq!(
            state.job_local_reuse_hits,
            if field == 2 { usize::MAX } else { 2 }
        );
        assert_eq!(
            state.queue.deduplicated,
            if field == 3 { usize::MAX } else { 2 }
        );
        // No additional callback can be partially counted after the refusal.
        assert!(
            state
                .accept(
                    Event::one(Effect::KnownReuse {
                        successor: true,
                        conditional: true
                    }),
                    &request
                )
                .is_err()
        );
        assert_eq!(state.events, 2);
    }
}

#[test]
fn job_local_reuse_mixed_runs_do_not_erase_frontier_or_current_conditional_flag() {
    let mut request = request();
    request.max_events = 6;
    let mut state = State::new(Queue::<1>::new(5, Some(5)), 0, None);
    state
        .accept(
            Event {
                count: 2,
                effect: Effect::KnownReuse {
                    successor: true,
                    conditional: false,
                },
            },
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
    state.accept(Event::one(Effect::Count), &request).unwrap();
    assert!(
        state
            .accept(
                Event {
                    count: 3,
                    effect: Effect::KnownReuse {
                        successor: true,
                        conditional: true
                    }
                },
                &request
            )
            .is_err()
    );
    assert_eq!(
        (
            state.events,
            state.successors,
            state.conditional,
            state.job_local_reuse_hits
        ),
        (6, 4, 2, 4)
    );
    assert_eq!(state.details[0]["kind"], "kept");
    assert_eq!(state.frontiers, 1);
}
#[test]
fn symbolic_stream_compact_counts_keep_exact_event_cap() {
    let mut request = request();
    request.max_events = 5;
    let mut state = State::new(Queue::<1>::new(5, Some(5)), 0, None);
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
    let mut state = State::new(Queue::<1>::new(5, Some(5)), 1, None);
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
    let mut state = State::new(Queue::<1>::new(5, Some(5)), 0, None);
    for phase in [Phase::Apply, Phase::Route] {
        for rank in [Some(11), None] {
            let domain = super::super::queue::Domain {
                powers: Default::default(),
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
    let mut queue = Queue::<1>::new(8, Some(100));
    for x in 0..3 {
        queue
            .admit(super::super::queue::Domain {
                powers: Default::default(),
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
    let mut queue = Queue::<1>::new(8, Some(100));
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
