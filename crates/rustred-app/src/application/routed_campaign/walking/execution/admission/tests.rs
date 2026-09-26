use super::super::super::{
    inspection::OptionalDiagnostic,
    queue::{Domain, Phase},
};
use super::*;
use rustred::solver::OwnerDomainMatchDisposition;

fn ordered_budget(workers: usize, cap: Option<usize>) -> WorkerBudget {
    WorkerBudget::new(
        workers,
        None,
        cap,
        super::super::super::OwnerDomainWalkPublicationPolicy::Ordered,
    )
}

fn request() -> OwnerDomainWalkRequest {
    OwnerDomainWalkRequest::new(super::super::super::OwnerDomainMatchRequest::new(
        String::new(),
        String::new(),
    ))
}
fn domain(lower: u64, upper: u64) -> Domain<2> {
    Domain {
        phase: Phase::Apply,
        owner: [true, false],
        lower: vec![lower, 0],
        upper: vec![Some(upper), Some(0)],
        rank: None,
        powers: Default::default(),
    }
}
fn seeded(max_domains: usize, cap: Option<usize>) -> State<2> {
    let mut queue = Queue::new(max_domains, cap);
    for i in 0..MIN_CANDIDATES as u64 {
        assert_eq!(
            queue.admit(domain(4 * i, 4 * i)).unwrap(),
            (i as usize, true)
        );
    }
    State::new(queue, 0, None)
}
fn stream() -> Vec<Event<2>> {
    let mut events = vec![Event {
        count: 2,
        effect: Effect::Count,
    }];
    for i in 0..48_u64 {
        let d = match i % 4 {
            0 => domain(i * 4, i * 4 + 2), // Retires a prior maximal candidate.
            1 => domain((i - 1) * 4 + 1, (i - 1) * 4 + 1), // Reuses a new earlier admission.
            2 => domain((i - 2) * 4, (i - 2) * 4 + 2), // Fresh exact-key priority.
            _ => domain(i * 4, i * 4),     // Existing exact key.
        };
        events.push(Event::one(Effect::Admit {
            domain: d,
            successor: true,
            conditional: i % 3 == 0,
        }));
        if i % 8 == 0 {
            events.push(Event {
                count: 3,
                effect: Effect::KnownReuse {
                    successor: true,
                    conditional: false,
                },
            });
            events.push(Event::one(Effect::PreAdmittedOrthantReuse {
                target: 0,
                successor: false,
                conditional: false,
            }));
            events.push(Event::one(Effect::Frontier {
                value: json!({"ordinal":i}),
                successor: false,
                conditional: false,
            }));
        }
    }
    events.insert(
        7,
        Event::one(Effect::Optional(OptionalDiagnostic {
            disposition: OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 1 },
            rank: Some(10),
            powers: Default::default(),
            lower: vec![0, 0],
            upper: vec![Some(1); 2],
            shift: vec![0, 0],
            ordinal: Some(0),
            resource: "test allowance",
            requested: 2,
            limit: 1,
        })),
    );
    events
}
fn serial(
    state: &mut State<2>,
    request: &OwnerDomainWalkRequest,
    events: Vec<Event<2>>,
) -> Result<(), &'static str> {
    for event in events {
        state.accept(event, request)?;
    }
    Ok(())
}
fn assert_same(expected: &State<2>, actual: &State<2>) {
    assert_eq!(expected.queue.domains, actual.queue.domains);
    assert_eq!(expected.queue.next, actual.queue.next);
    assert_eq!(
        (
            expected.events,
            expected.successors,
            expected.conditional,
            expected.frontiers
        ),
        (
            actual.events,
            actual.successors,
            actual.conditional,
            actual.frontiers
        )
    );
    assert_eq!(
        (
            expected.job_local_reuse_hits,
            expected.pre_admitted_orthant_hits,
            expected.queue.deduplicated,
            expected.queue.exact_hits,
            expected.queue.orthant_hits
        ),
        (
            actual.job_local_reuse_hits,
            actual.pre_admitted_orthant_hits,
            actual.queue.deduplicated,
            actual.queue.exact_hits,
            actual.queue.orthant_hits
        )
    );
    assert_eq!(expected.details, actual.details);
    assert_eq!(expected.refusals.records, actual.refusals.records);
    assert_eq!(
        expected.queue.containment_candidate_count(),
        actual.queue.containment_candidate_count()
    );
    assert_eq!(
        expected.queue.containment_semantic_hits,
        actual.queue.containment_semantic_hits
    );
    assert_eq!(
        expected.queue.containment_semantic_retirements,
        actual.queue.containment_semantic_retirements
    );
    assert_eq!(expected.completed, actual.completed);
}

#[test]
fn worker_budget_reserves_distinct_lookup_helpers_and_coordinator() {
    for requested in 1..=100 {
        for cap in [None, Some(50)] {
            let b = ordered_budget(requested, cap);
            assert_eq!(b.inspection + b.helpers + b.coordinator, requested);
            assert!(b.inspection >= 1);
            if cap.is_some() || requested < 5 {
                assert_eq!(b.helpers, 0);
            }
        }
    }
    let b = ordered_budget(50, None);
    assert_eq!((b.inspection, b.helpers, b.coordinator), (25, 24, 1));
}

#[test]
fn parallel_preparation_preserves_every_event_frontier_and_domain_cap_prefix() {
    let budget = ordered_budget(8, None);
    let engine = Engine::new(budget).unwrap();
    let cancellation = AtomicBool::new(false);
    let stop = AtomicBool::new(false);
    for event_cap in 0..=84 {
        let mut request = request();
        request.max_events = event_cap;
        let mut expected = seeded(1000, None);
        let mut actual = seeded(1000, None);
        let reference = serial(&mut expected, &request, stream());
        let parallel = engine.commit_chunk(
            &mut actual,
            &request,
            stream(),
            &cancellation,
            &stop,
            &mut |_| {},
        );
        assert_eq!(reference, parallel, "event cap {event_cap}");
        assert_same(&expected, &actual);
        assert_eq!(actual.admission.batches, 1);
        assert_eq!(actual.admission.preparations, 48); // Includes proposals past a cap.
        assert!(actual.admission.speculative_reverse_checks > 0);
        if event_cap >= 3 {
            // The first admission retires seeded point 0 through its
            // helper-prepared reverse set.
            assert!(
                actual.admission.prepared_retirements_applied > 0,
                "event cap {event_cap}"
            );
            assert_eq!(actual.admission.prepared_retire_fallbacks, 0);
        }
    }
    for (domains, frontiers) in [(128, 99), (130, 99), (1000, 0), (1000, 3), (1000, 99)] {
        let mut request = request();
        request.max_frontiers = frontiers;
        let mut expected = seeded(domains, None);
        let mut actual = seeded(domains, None);
        assert_eq!(
            serial(&mut expected, &request, stream()),
            engine.commit_chunk(
                &mut actual,
                &request,
                stream(),
                &cancellation,
                &stop,
                &mut |_| {}
            )
        );
        assert_same(&expected, &actual);
    }
}

#[test]
fn bit_prefilter_toggle_preserves_engine_results_and_counters() {
    let engine = Engine::new(ordered_budget(8, None)).unwrap();
    let cancellation = AtomicBool::new(false);
    let mut reference = seeded(1000, None);
    let mut filtered = seeded(1000, None);
    let mut unfiltered = seeded(1000, None);
    unfiltered.queue.disable_bit_prefilter();
    serial(&mut reference, &request(), stream()).unwrap();
    for state in [&mut filtered, &mut unfiltered] {
        engine
            .commit_chunk(
                state,
                &request(),
                stream(),
                &cancellation,
                &cancellation,
                &mut |_| {},
            )
            .unwrap();
        assert_same(&reference, state);
        assert_eq!(
            reference.queue.containment_maintenance_checks,
            state.queue.containment_maintenance_checks
        );
    }
    assert_eq!(
        filtered.queue.containment_checks,
        unfiltered.queue.containment_checks
    );
    let (f, u) = (&filtered.admission, &unfiltered.admission);
    assert_eq!(f.speculative_checks, u.speculative_checks);
    assert_eq!(f.speculative_reverse_checks, u.speculative_reverse_checks);
    assert_eq!(
        f.prepared_retirements_applied,
        u.prepared_retirements_applied
    );
    assert_eq!(
        u.speculative_forward_bit_rejections + u.speculative_reverse_bit_rejections,
        0
    );
    assert_eq!(
        filtered.queue.session.forward_callbacks,
        unfiltered.queue.session.forward_callbacks
    );
    assert_eq!(
        filtered.queue.session.reverse_callbacks,
        unfiltered.queue.session.reverse_callbacks
    );
    assert_eq!(unfiltered.queue.session.forward_bit_rejections, 0);
    let json = filtered.admission.json();
    assert!(json["prepared_retirements_applied"].as_u64().unwrap() > 0);
    assert_eq!(json["prepared_retire_fallbacks"], 0);
    println!(
        "engine_bit_prefilter filtered={} unfiltered={} session={:?}",
        json,
        unfiltered.admission.json(),
        filtered.queue.session
    );
}

#[test]
fn finite_comparison_caps_and_small_workloads_keep_the_serial_path() {
    let engine = Engine::new(ordered_budget(8, None)).unwrap();
    let cancellation = AtomicBool::new(false);
    for cap in [Some(100_000), None] {
        let mut actual = if cap.is_some() {
            seeded(1000, cap)
        } else {
            State::new(Queue::new(1000, None), 0, None)
        };
        engine
            .commit_chunk(
                &mut actual,
                &request(),
                stream(),
                &cancellation,
                &cancellation,
                &mut |_| {},
            )
            .unwrap();
        assert_eq!(actual.admission.batches, 0);
    }
    let mut actual = seeded(1000, None);
    let events = stream().into_iter().take(8).collect();
    engine
        .commit_chunk(
            &mut actual,
            &request(),
            events,
            &cancellation,
            &cancellation,
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(actual.admission.batches, 0);
}

#[test]
fn future_invalid_domain_does_not_jump_over_an_earlier_event_limit() {
    let engine = Engine::new(ordered_budget(8, None)).unwrap();
    let cancellation = AtomicBool::new(false);
    for limit in [1, 1000] {
        let mut request = request();
        request.max_events = limit;
        let make_events = || {
            let mut events = stream();
            let mut invalid = domain(5, 4);
            invalid.powers.min_power_difference = Some(20);
            invalid.powers.max_power_difference = Some(10);
            events.insert(
                10,
                Event::one(Effect::Admit {
                    domain: invalid,
                    successor: true,
                    conditional: false,
                }),
            );
            events
        };
        let mut expected = seeded(1000, None);
        let mut actual = seeded(1000, None);
        assert_eq!(
            serial(&mut expected, &request, make_events()),
            engine.commit_chunk(
                &mut actual,
                &request,
                make_events(),
                &cancellation,
                &cancellation,
                &mut |_| {}
            )
        );
        assert_same(&expected, &actual);
    }
}

#[test]
fn cancellation_or_producer_failure_after_preparation_publishes_nothing() {
    let engine = Engine::new(ordered_budget(8, None)).unwrap();
    for producer_failure in [false, true] {
        let cancellation = AtomicBool::new(false);
        let stop = AtomicBool::new(false);
        let mut state = seeded(1000, None);
        let before = state.queue.domains.clone();
        let prepared = engine.prepare(
            &state.queue,
            stream(),
            &cancellation,
            &stop,
            &mut state.admission,
        );
        if producer_failure {
            stop.store(true, Ordering::Release);
        } else {
            cancellation.store(true, Ordering::Release);
        }
        assert_eq!(
            Engine::commit_prepared(&mut state, &request(), prepared, &cancellation, &stop),
            Err("cancelled")
        );
        assert_eq!(state.events, 0);
        assert_eq!(state.queue.domains, before);
        assert_eq!(state.admission.preparations, 48);
    }
}

#[test]
fn explicit_inspector_extremes_preserve_admission_prefixes_and_cancellation() {
    for inspectors in [1, 7] {
        let budget = WorkerBudget::new(
            8,
            Some(inspectors),
            None,
            super::super::super::OwnerDomainWalkPublicationPolicy::Ordered,
        );
        let engine = Engine::new(budget).unwrap();
        assert_eq!(
            engine
                .pool
                .as_ref()
                .map_or(0, |pool| pool.current_num_threads()),
            7 - inspectors
        );
        let cancellation = AtomicBool::new(false);
        let stop = AtomicBool::new(false);
        for cap in [0, 1, 17, 84, 1000] {
            let mut request = request();
            request.max_events = cap;
            let mut expected = seeded(1000, None);
            let mut actual = seeded(1000, None);
            assert_eq!(
                serial(&mut expected, &request, stream()),
                engine.commit_chunk(
                    &mut actual,
                    &request,
                    stream(),
                    &cancellation,
                    &stop,
                    &mut |_| {}
                ),
            );
            assert_same(&expected, &actual);
        }
        for producer_failure in [false, true] {
            let cancellation = AtomicBool::new(false);
            let stop = AtomicBool::new(false);
            let mut actual = seeded(1000, None);
            let before = actual.queue.domains.clone();
            let prepared = engine.prepare(
                &actual.queue,
                stream(),
                &cancellation,
                &stop,
                &mut actual.admission,
            );
            if producer_failure {
                stop.store(true, Ordering::Release);
            } else {
                cancellation.store(true, Ordering::Release);
            }
            assert_eq!(
                Engine::commit_prepared(&mut actual, &request(), prepared, &cancellation, &stop),
                Err("cancelled")
            );
            assert_eq!(actual.events, 0);
            assert_eq!(actual.queue.domains, before);
        }
    }
}

#[test]
fn bounded_slices_stop_between_batches_without_discarding_admitted_obligations() {
    let engine = Engine::new(ordered_budget(8, None)).unwrap();
    let cancellation = AtomicBool::new(false);
    let stop = AtomicBool::new(false);
    let events = || {
        (0..600)
            .map(|i| {
                Event::one(Effect::Admit {
                    domain: domain(1000 + i * 4, 1000 + i * 4),
                    successor: true,
                    conditional: false,
                })
            })
            .collect::<Vec<_>>()
    };
    let mut expected = seeded(1000, None);
    serial(
        &mut expected,
        &request(),
        events().into_iter().take(BATCH_RECORDS).collect(),
    )
    .unwrap();
    let mut actual = seeded(1000, None);
    assert_eq!(
        engine.commit_chunk(
            &mut actual,
            &request(),
            events(),
            &cancellation,
            &stop,
            &mut |_| {
                cancellation.store(true, Ordering::Release);
            }
        ),
        Err("cancelled")
    );
    assert_same(&expected, &actual);
    assert_eq!(actual.admission.records, BATCH_RECORDS);
    assert_eq!(actual.queue.next, 0); // Admission never means completion.
}

#[test]
fn native_failure_retains_precedence_and_helpers_do_not_share_producer_pool() {
    if !symbolica::license::LicenseManager::is_licensed() {
        return;
    }
    let engine = Engine::new(ordered_budget(5, None)).unwrap();
    let cancellation = AtomicBool::new(false);
    let mut state = seeded(1000, None);
    let (_, snapshot, _) = parallel::with_pool::<2, _>(
        2,
        |_, _, _| unreachable!("no native job dispatched"),
        |pool| {
            pool.fail(Failure {
                id: Some(0),
                phase: Some(Phase::Apply),
                kind: "worker_panic",
                detail: "original failure".into(),
            });
            let error = engine
                .commit_chunk(
                    &mut state,
                    &request(),
                    stream(),
                    &cancellation,
                    &pool.stop,
                    &mut |_| {},
                )
                .unwrap_err();
            pool.fail(Failure {
                id: Some(0),
                phase: Some(Phase::Apply),
                kind: "coordinator_admission",
                detail: error.into(),
            });
        },
    );
    assert_eq!(snapshot["first_failure"]["detail"], "original failure");
    assert_eq!(state.events, 0);
}

#[test]
fn helper_panic_propagates_only_after_backpressured_native_worker_is_joined() {
    if !symbolica::license::LicenseManager::is_licensed() {
        return;
    }
    let engine = Engine::new(ordered_budget(5, None)).unwrap();
    let returned = AtomicBool::new(false);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        parallel::with_pool::<2, _>(
            2,
            |_, _, emit| {
                loop {
                    if emit(Event {
                        count: parallel::CHUNK_EVENTS,
                        effect: Effect::Count,
                    })
                    .is_break()
                    {
                        break;
                    }
                }
                returned.store(true, Ordering::Release);
                Finished {
                    stats: NativeStats::Apply(Default::default()),
                    error: None,
                    error_kind: "none",
                    seconds: 0.0,
                }
            },
            |pool| {
                assert!(pool.dispatch(0, std::sync::Arc::new(domain(0, 0))));
                let start = Instant::now();
                while pool.snapshot()["backpressured_workers"] != 1 {
                    assert!(
                        start.elapsed() < Duration::from_secs(5),
                        "producer failed to reach backpressure"
                    );
                    std::thread::yield_now();
                }
                engine
                    .pool
                    .as_ref()
                    .unwrap()
                    .install(|| panic!("injected admission helper panic"));
            },
        );
    }));
    let payload = result.expect_err("helper panic must propagate");
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"injected admission helper panic")
    );
    assert!(
        returned.load(Ordering::Acquire),
        "native producer must be joined before unwind escapes"
    );
}
