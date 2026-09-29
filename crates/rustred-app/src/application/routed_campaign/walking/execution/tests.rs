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
    assert_eq!(state.records.borrow().total(), 1);
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

#[test]
fn completed_escrow_event_cap_keeps_exact_publisher_prefix_and_later_attempts() {
    if !crate::test_gates::licensed_or_skip(
        "completed_escrow_event_cap_keeps_exact_publisher_prefix_and_later_attempts",
    ) {
        return;
    }
    let mut request = request();
    request.max_events = 4;
    let mut queue = Queue::<1>::new(8, None);
    for x in 0..4 {
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
    let mut state = State::new(queue, 0, None);
    let release = AtomicBool::new(false);
    let (_, snapshot, mut leftovers) = parallel::with_pool(
        2,
        |d, stop, emit| {
            if d.lower[0] == 0 {
                while !release.load(Ordering::Acquire) && !stop.load(Ordering::Acquire) {
                    std::thread::yield_now();
                }
            }
            let event = if d.lower[0] == 1 {
                // The ordinary Admit must precede this job's known-reuse run,
                // including when its entire stream waits in completed escrow.
                assert!(
                    emit(Event::one(Effect::Admit {
                        domain: d.clone(),
                        successor: true,
                        conditional: true,
                    }))
                    .is_continue()
                );
                Event {
                    count: 5,
                    effect: Effect::KnownReuse {
                        successor: true,
                        conditional: true,
                    },
                }
            } else {
                Event::one(Effect::Count)
            };
            let stopped = emit(event).is_break();
            Finished {
                stats: NativeStats::Apply(Default::default()),
                error: stopped.then(|| "cancelled".into()),
                error_kind: if stopped { "cancelled" } else { "none" },
                seconds: 0.0,
            }
        },
        |pool| {
            assert!(pool.dispatch(0, state.queue.domain_arc(0)));
            for id in 1..4 {
                assert!(pool.dispatch(id, state.queue.domain_arc(id)));
                let start = Instant::now();
                while pool.snapshot()["returned_inspections"] != id {
                    assert!(start.elapsed() < Duration::from_secs(5));
                    std::thread::yield_now();
                }
                pool.reclaim_finished(0);
            }
            assert_eq!(pool.snapshot()["completed_escrow_entries"], 3);
            release.store(true, Ordering::Release);
            'publish: loop {
                let id = state.queue.next;
                match pool.poll(id) {
                    Poll::Events(chunk) => {
                        for event in chunk {
                            if let Err(error) = state.accept(event, &request) {
                                state.error = Some(error.into());
                                pool.fail(Failure {
                                    id: Some(id),
                                    phase: Some(Phase::Apply),
                                    kind: "coordinator_admission",
                                    detail: error.into(),
                                });
                                break 'publish;
                            }
                        }
                    }
                    Poll::Finished(done) => state.commit(id, done),
                    Poll::Waiting => pool.wait(id),
                }
            }
        },
    );
    assert_eq!(state.queue.next, 1);
    assert_eq!(state.completed, 1);
    assert_eq!(
        (state.events, state.successors, state.conditional),
        (4, 3, 3)
    );
    assert_eq!(state.job_local_reuse_hits, 2);
    assert_eq!(state.queue.deduplicated, 3);
    assert_eq!(state.queue.exact_hits, 1);
    assert_eq!(snapshot["attempted_events"], 9);
    assert_eq!(snapshot["returned_inspections"], 4);
    assert_eq!(snapshot["finished_uncommitted_domains"], 3);
    assert_eq!(snapshot["worker_buffered_events"], 0);
    assert_eq!(leftovers.len(), 3);
    retain_leftovers(&mut state, &mut leftovers);
    assert_eq!(state.queue.next, 2);
    assert_eq!(state.completed, 1); // failed publisher is not completion
    assert_eq!(state.records.borrow().total(), 2);
    assert_eq!(state.uncommitted.len(), 2);
    assert_eq!(state.uncommitted[0]["id"], 2);
    assert_eq!(state.uncommitted[1]["id"], 3);
    assert_eq!(state.uncommitted[1]["lower"], json!([3]));
}

#[test]
fn per_domain_progress_events_stay_lean_while_heartbeats_carry_session_telemetry() {
    let state = State::new(Queue::<1>::new(5, None), 0, None);
    for event in ["domain_started", "domain_delegated"] {
        assert!(State::<1>::lean_event(event));
        let lean = state.progress(event, 0, &json!({}));
        assert!(lean.get("containment_prefilter").is_none(), "{event}");
        assert!(lean.get("coordinator_duty").is_none(), "{event}");
        assert!(
            lean["parallel"].get("containment_prefilter").is_none()
                && lean["parallel"].get("closure_refresh_policy").is_none()
                && lean["parallel"].get("coordinator_duty").is_none(),
            "{event}"
        );
        let admission = &lean["parallel"]["admission_preparation"];
        assert!(admission.get("coordinator_duty").is_none(), "{event}");
        assert!(
            admission.get("prepared_retirements_applied").is_none()
                && admission.get("speculative_reverse_checks").is_none(),
            "{event}"
        );
        assert_eq!(lean["containment_checks"], 0); // Historical keys stay.
        assert_eq!(admission["speculative_containment_checks"], 0);
        assert_eq!(lean["descendant_closure"]["refresh_count"], 0);
    }
    for event in ["domain_progress", "domain_draining"] {
        assert!(!State::<1>::lean_event(event));
        let detailed = state.progress(event, 0, &json!({}));
        // New session telemetry sits under `parallel`, which the strict
        // old-vs-new result comparison ignores; the top level and the
        // closure report keep their historical key sets on every event.
        assert!(detailed.get("containment_prefilter").is_none());
        assert_eq!(
            detailed["parallel"]["containment_prefilter"]["forward_callbacks"],
            0
        );
        assert_eq!(
            detailed["parallel"]["closure_refresh_policy"]["duty_bound"],
            0.01
        );
        assert!(
            detailed["descendant_closure"]
                .get("refresh_duty_bound")
                .is_none()
        );
        assert_eq!(
            detailed["parallel"]["admission_preparation"]["prepared_retirements_applied"],
            0
        );
        // Kernel attribution: helper scans under admission_preparation, the
        // coordinator's nested in the duty object (not a share; the monitor
        // keeps numeric duty entries only).
        let helper = &detailed["parallel"]["admission_preparation"]["speculative_kernel"];
        assert_eq!(helper["kernel"], "struct_of_arrays_u8_lanes_v1");
        assert_eq!(helper["candidates"], 0);
        assert!(helper["ns_per_candidate"].is_null());
        assert_eq!(
            detailed["parallel"]["admission_preparation"]["speculative_forward_exact_tests"],
            0
        );
        // Exactly one duty object, where heartbeat_metrics.py reads it.
        let duty = &detailed["parallel"]["coordinator_duty"];
        let kernel = &duty["admission_kernel"];
        assert_eq!(kernel["kernel"], "struct_of_arrays_u8_lanes_v1");
        assert_eq!(kernel["forward_scans"], 0);
        assert_eq!(kernel["reverse_exact_tests"], 0);
        assert!(kernel["forward_ns_per_candidate"].is_null());
        // Reverse cost per examined candidate (helper-decided ones included).
        assert_eq!(kernel["reverse_examined_candidates"], 0);
        assert_eq!(kernel["reverse_prepared_candidates"], 0);
        assert!(kernel["reverse_retire_ns_per_examined_candidate"].is_null());
        assert!(kernel.get("reverse_ns_per_candidate").is_none());
        // O(1) storage telemetry, with the pre-kernel analogue beside it.
        let storage = &detailed["parallel"]["queue_storage"];
        assert!(storage["total_excluding_index_bytes"].is_number());
        assert!(storage["scope"].is_string());
        assert!(duty["dispatch_seconds"].is_number());
        assert!(duty["ordered_commit_seconds"].is_number());
        assert!(duty["coordinator_elapsed_seconds"].is_null()); // Not started.
        assert!(detailed.get("coordinator_duty").is_none());
        assert!(
            detailed["parallel"]["admission_preparation"]
                .get("coordinator_duty")
                .is_none()
        );
    }
}

/// Key sets of a `domain_delegated` event journaled by the frozen pre-wave
/// binary 32fdec09 (Ready FG control, `TMP/fable51-controls/baseline-32fdec-ready/
/// fg/events.jsonl`), without the three keys the walk's outer observer adds
/// (`checkpoint`, `requested_max_queries`, `requested_max_query_bytes`).
const HISTORICAL_READY_TOP: [&str; 53] = [
    "commit_domain",
    "committed_domains",
    "committed_events",
    "completed_nodes",
    "conditional_successors",
    "containment_candidates",
    "containment_check_policy",
    "containment_checks",
    "containment_index_policy",
    "containment_maintenance_checks",
    "containment_retired_candidates",
    "containment_semantic_hits",
    "containment_semantic_retirements",
    "containment_summary_builds",
    "contiguous_publication_watermark",
    "deduplication_hits",
    "delegation",
    "descendant_closure",
    "event",
    "events",
    "exact_domain_hits",
    "frontiers",
    "full_orthant_hits",
    "id",
    "initial_entry_domains_inspected",
    "initial_entry_domains_published",
    "initial_entry_domains_total",
    "job_local_reuse_hits",
    "max_containment_checks",
    "max_scheduled_finite_rank",
    "native_processed_nodes",
    "operation",
    "owner",
    "parallel",
    "partial_initial_inspections",
    "pending_descendant_domains",
    "phase",
    "power_bounds",
    "pre_admitted_orthant_hits",
    "publication_policy",
    "queued_nodes",
    "ready_accepted_source_prefixes",
    "ready_prefix_tracking_scope",
    "ready_published_holes",
    "ready_stream_contexts",
    "reuse_initial_d_bands",
    "route_joint_support_masks_pruned",
    "route_masks",
    "routed_domains",
    "scheduled_nodes",
    "scheduling_policy",
    "successors",
    "unbounded_rank_domains",
];
const HISTORICAL_ADMISSION: [&str; 20] = [
    "batch_record_limit",
    "coordinator_worker_limit",
    "counter_saturated",
    "counter_scope",
    "inspection_worker_limit",
    "lookup_worker_limit",
    "minimum_admissions",
    "minimum_candidates",
    "ordered_commit_wall_seconds",
    "parallel_batches",
    "policy",
    "preparation_wall_seconds",
    "prepared_batch_records",
    "requested_worker_budget",
    "speculative_admission_requests",
    "speculative_check_scope",
    "speculative_containment_checks",
    "speculative_work_is_not_admission",
    "timing_scope",
    "total_compute_worker_limit",
];
const HISTORICAL_CLOSURE: [&str; 23] = [
    "available",
    "closed_counts_are_conservative_lower_bounds",
    "dependency_edges",
    "family_closure_claim",
    "graph_revision",
    "initial_closed",
    "initial_total",
    "last_refresh_seconds",
    "locally_inspected",
    "method",
    "reason",
    "refresh_count",
    "refresh_scratch_estimate_bytes",
    "refresh_seconds",
    "retained_storage_estimate_bytes",
    "scope",
    "snapshot_age_seconds",
    "snapshot_revision",
    "snapshot_stale",
    "storage_estimate_scope",
    "total_closed",
    "total_domains",
    "unresolved_domains",
];

fn sorted_keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("JSON object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    keys
}

/// Keys of `detailed` absent from `lean`; `lean` must be a subset.
fn added<'a>(lean: &Value, detailed: &'a Value) -> Vec<&'a str> {
    let lean = sorted_keys(lean);
    let detailed = sorted_keys(detailed);
    assert!(lean.iter().all(|key| detailed.contains(key)), "{lean:?}");
    detailed
        .into_iter()
        .filter(|key| !lean.contains(key))
        .collect()
}

/// Top-level keys of the frozen binary's Ready events that its Ordered FG
/// control (`TMP/fable51-controls/baseline-32fdec/fg/events.jsonl`) never
/// journals.
const HISTORICAL_READY_ONLY_TOP: [&str; 5] = [
    "publication_policy",
    "ready_accepted_source_prefixes",
    "ready_prefix_tracking_scope",
    "ready_published_holes",
    "ready_stream_contexts",
];

/// An empty walk after initial admission, under a Ready or Ordered
/// responsibility ledger.
fn ledger_state(ready: bool) -> State<1> {
    use super::super::delegation::Ledger;
    let lookahead = std::num::NonZeroUsize::new(2).unwrap();
    let mut ledger = if ready {
        Ledger::new_ready(lookahead, 5)
    } else {
        Ledger::new(lookahead, 5)
    }
    .unwrap();
    ledger.begin_initial_admission().unwrap();
    ledger.finish_initial_admission().unwrap();
    let mut queue = Queue::<1>::new(5, None);
    queue.delegation = Some(ledger);
    State::new(queue, 0, None)
}

/// Every per-domain event keeps the historical key set exactly, so a new
/// unconditional field anywhere in `progress`, `metrics_json`, `json_with` or
/// the closure report fails here; heartbeats add session telemetry only
/// under `parallel`, and exactly the pinned keys.
#[test]
fn per_domain_event_key_sets_are_frozen_and_heartbeats_only_add_pinned_telemetry() {
    let state = ledger_state(true);
    for event in ["domain_started", "domain_delegated"] {
        let lean = state.progress(event, 0, &json!({}));
        assert_eq!(sorted_keys(&lean), HISTORICAL_READY_TOP, "{event}");
        assert_eq!(sorted_keys(&lean["parallel"]), ["admission_preparation"]);
        assert_eq!(
            sorted_keys(&lean["parallel"]["admission_preparation"]),
            HISTORICAL_ADMISSION
        );
        assert_eq!(sorted_keys(&lean["descendant_closure"]), HISTORICAL_CLOSURE);
    }
    // The Ordered walk shares `progress` without the Ready-only keys.
    let ordered = ledger_state(false);
    let historical_ordered: Vec<&str> = HISTORICAL_READY_TOP
        .into_iter()
        .filter(|key| !HISTORICAL_READY_ONLY_TOP.contains(key))
        .collect();
    for event in ["domain_started", "domain_delegated"] {
        let lean = ordered.progress(event, 0, &json!({}));
        assert_eq!(sorted_keys(&lean), historical_ordered, "{event}");
        assert_eq!(sorted_keys(&lean["parallel"]), ["admission_preparation"]);
    }
    let lean = state.progress("domain_started", 0, &json!({}));
    for event in ["domain_progress", "domain_draining"] {
        let detailed = state.progress(event, 0, &json!({}));
        assert!(added(&lean, &detailed).is_empty(), "{event}");
        assert!(
            added(&lean["descendant_closure"], &detailed["descendant_closure"]).is_empty(),
            "{event}"
        );
        assert_eq!(
            added(&lean["parallel"], &detailed["parallel"]),
            [
                "closure_refresh_policy",
                "containment_prefilter",
                "coordinator_duty",
                "queue_storage"
            ]
        );
        assert_eq!(
            added(
                &lean["parallel"]["admission_preparation"],
                &detailed["parallel"]["admission_preparation"]
            ),
            [
                "prepared_retire_fallbacks",
                "prepared_retirement_limit",
                "prepared_retirement_scope",
                "prepared_retirements_applied",
                "prepared_retirements_trivial",
                "speculative_forward_bit_rejections",
                "speculative_forward_exact_tests",
                "speculative_kernel",
                "speculative_reverse_bit_rejections",
                "speculative_reverse_checks",
                "speculative_reverse_exact_tests"
            ]
        );
    }
}

/// `observe` hands each event kind its pool tier: per-domain events the lean
/// tier (the frozen binary's 33 `parallel` keys: no activity breakdown, no
/// per-slot arrays), heartbeats the detailed tier and only the drain events
/// the per-slot arrays. The tiers' contents are pinned in `parallel/tests.rs`;
/// this pins which tier each event receives.
#[test]
fn observe_attaches_the_lean_pool_tier_to_per_domain_events_only() {
    let mut state = ledger_state(true);
    let pool = parallel::Pool::<1>::new(3);
    let seen = RefCell::new(Vec::new());
    let events = [
        "domain_started",
        "domain_delegated",
        "domain_progress",
        "domain_draining",
    ];
    for event in events {
        observe(
            &mut state,
            &|value: Value| seen.borrow_mut().push(value),
            event,
            0,
            &pool,
        );
    }
    let with = |tier: Value, coordinator: &[&str]| {
        let mut keys: Vec<String> = tier.as_object().unwrap().keys().cloned().collect();
        keys.extend(coordinator.iter().map(|key| key.to_string()));
        keys.sort_unstable();
        keys
    };
    let lean = ["admission_preparation"];
    let heartbeat = [
        "admission_preparation",
        "closure_refresh_policy",
        "containment_prefilter",
        "coordinator_duty",
        "queue_storage",
    ];
    let expected = [
        with(pool.snapshot_lean(), &lean),
        with(pool.snapshot_lean(), &lean),
        with(pool.snapshot_detailed(), &heartbeat),
        with(pool.snapshot(), &heartbeat),
    ];
    let seen = seen.into_inner();
    assert_eq!(seen.len(), events.len());
    for ((event, observed), expected) in events.iter().zip(&seen).zip(expected) {
        assert_eq!(observed["event"], *event);
        assert_eq!(sorted_keys(&observed["parallel"]), expected, "{event}");
    }
    for observed in &seen[..2] {
        assert_eq!(observed["parallel"].as_object().unwrap().len(), 33);
        assert!(observed["parallel"].get("computing_workers").is_none());
    }
    assert!(seen[2]["parallel"]["computing_workers"].is_number());
    for observed in &seen[..3] {
        let parallel = observed["parallel"].as_object().unwrap();
        assert!(
            parallel.keys().all(|key| !key.starts_with("slot_")),
            "{}",
            observed["event"]
        );
    }
    assert_eq!(
        seen[3]["parallel"]["slot_busy_seconds"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn reused_lean_state_adds_resume_attempts_once_and_removes_detailed_extensions() {
    let pool = parallel::Pool::<1>::new(3);
    let mut state = ledger_state(true);
    let previous = json!({
        "attempted_events":3, "returned_inspections":5,
        "attempted_native_operations":7, "attempted_rule_checks":11,
        "attempted_predicates":13, "attempted_optional_coefficient_refusals":17
    });
    state.set_parallel(pool.snapshot(), &previous);
    assert!(state.parallel.get("slot_busy_seconds").is_some());
    assert!(state.parallel.get("coordinator_duty").is_some());
    for _ in 0..3 {
        let mut expected = state.enrich_with(pool.snapshot_lean(), true);
        let mut error = None;
        accumulate_attempts(&mut expected, &previous, &mut error);
        state.set_parallel_lean(pool.capture_lean(), &previous);
        assert_eq!(state.parallel, expected);
        for (key, value) in previous.as_object().unwrap() {
            assert_eq!(
                &state.parallel[key], value,
                "{key}: resume totals added twice"
            );
        }
        assert!(state.parallel.get("slot_busy_seconds").is_none());
        assert!(state.parallel.get("coordinator_duty").is_none());
        assert_eq!(state.parallel.as_object().unwrap().len(), 33);
        assert!(state.error.is_none());
    }
    // A fresh process/session without previous counters must not retain the
    // accumulation or its scope string from the reusable object.
    state.set_parallel_lean(pool.capture_lean(), &Value::Null);
    assert_eq!(state.parallel["returned_inspections"], 0);
    assert_eq!(
        state.parallel["native_attempt_counters_scope"],
        "returned_inspections_including_uncommitted_and_cancelled"
    );
}

#[test]
fn reused_lean_state_decodes_each_raw_failure_ticket_exactly_once() {
    let mut queue = Queue::<1>::new(8, None);
    for coordinate in 0..3 {
        assert_eq!(
            queue
                .admit(super::super::queue::Domain {
                    phase: Phase::Apply,
                    owner: [true],
                    lower: vec![coordinate],
                    upper: vec![Some(coordinate)],
                    rank: None,
                    powers: Default::default(),
                })
                .unwrap(),
            (coordinate as usize, true)
        );
    }
    let mut state = State::new(queue, 0, None);
    state.physical_enabled = true;
    let pool = parallel::Pool::<1>::new(1);
    let raw = Ticket {
        parent: 2,
        part: Some(1),
    }
    .encode(true)
    .unwrap();
    pool.fail(parallel::Failure {
        id: Some(raw),
        phase: Some(Phase::Apply),
        kind: "native_failure",
        detail: "physical test fault".into(),
    });
    for _ in 0..3 {
        state.set_parallel_lean(pool.capture_lean(), &Value::Null);
        for key in ["first_failure", "non_cancellation_failure"] {
            assert_eq!(state.parallel[key]["domain"], 2);
            assert_eq!(state.parallel[key]["physical_part"], 1);
            assert_eq!(state.parallel[key]["lower"], json!([2]));
            assert_eq!(state.parallel[key]["detail"], "physical test fault");
        }
    }
    // A later healthy snapshot cannot inherit either a fault or physical-only
    // keys when the receiving coordinator is no longer in physical mode.
    let healthy = parallel::Pool::<1>::new(1);
    state.physical_enabled = false;
    state.set_parallel_lean(healthy.capture_lean(), &Value::Null);
    assert!(state.parallel["first_failure"].is_null());
    assert!(state.parallel["non_cancellation_failure"].is_null());
    assert!(state.parallel.get("inspection_count_scope").is_none());
    assert!(
        state
            .parallel
            .get("physical_inspections_published")
            .is_none()
    );
}
