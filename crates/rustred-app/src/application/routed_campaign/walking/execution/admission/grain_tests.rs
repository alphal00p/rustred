//! Paired semantics checks for the test-only indexed minimum-grain experiment.
use super::super::super::{inspection::OptionalDiagnostic, queue::Domain};
use super::*;
use rustred::solver::OwnerDomainMatchDisposition;
use std::num::NonZeroUsize;

fn budget(workers: usize) -> WorkerBudget {
    WorkerBudget::new(
        workers,
        None,
        None,
        super::super::super::OwnerDomainWalkPublicationPolicy::Ordered,
    )
}
fn engines() -> [Engine; 2] {
    [
        Engine::new(budget(8)).unwrap(),
        Engine::new_with_min_task_len(budget(8), NonZeroUsize::new(8).unwrap()).unwrap(),
    ]
}
fn request() -> OwnerDomainWalkRequest {
    OwnerDomainWalkRequest::new(super::super::super::OwnerDomainMatchRequest::new(
        String::new(),
        String::new(),
    ))
}
fn domain(lower: u64, upper: Option<u64>) -> Domain<2> {
    Domain {
        phase: Phase::Apply,
        owner: [true, false],
        lower: vec![lower, 0],
        upper: vec![upper, Some(0)],
        rank: None,
        powers: Default::default(),
    }
}
fn seeded(count: usize, cap: Option<usize>) -> State<2> {
    let mut queue = Queue::new(10_000, cap);
    for i in 0..count as u64 {
        assert_eq!(
            queue.admit(domain(4 * i, Some(4 * i))).unwrap(),
            (i as usize, true)
        );
    }
    let mut state = State::new(queue, 0, None);
    state.replay = Some(replay::Replay::default());
    state
}
fn admissions(n: usize) -> Vec<Event<2>> {
    (0..n as u64)
        .map(|i| {
            // Snapshot retirement, fresh earlier containment, fresh exact reuse,
            // and unchanged exact reuse all occur in each four-record group.
            let d = match i % 4 {
                0 => domain(4 * i, Some(4 * i + 2)),
                1 => domain(4 * (i - 1) + 1, Some(4 * (i - 1) + 1)),
                2 => domain(4 * (i - 2), Some(4 * (i - 2) + 2)),
                _ => domain(4 * i, Some(4 * i)),
            };
            Event::one(Effect::Admit {
                domain: d,
                successor: true,
                conditional: i % 3 == 0,
            })
        })
        .collect()
}
fn mixed() -> Vec<Event<2>> {
    let mut stream = admissions(64);
    stream.insert(
        0,
        Event {
            count: 3,
            effect: Effect::Count,
        },
    );
    stream.insert(
        3,
        Event {
            count: 4,
            effect: Effect::KnownReuse {
                successor: true,
                conditional: true,
            },
        },
    );
    stream.insert(
        5,
        Event::one(Effect::PreAdmittedOrthantReuse {
            target: 0,
            successor: false,
            conditional: false,
        }),
    );
    stream.insert(
        7,
        Event::one(Effect::Frontier {
            value: json!({"source_ordinal":7}),
            successor: true,
            conditional: false,
        }),
    );
    stream.insert(
        9,
        Event::one(Effect::Optional(OptionalDiagnostic {
            disposition: OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 1 },
            rank: Some(10),
            powers: Default::default(),
            lower: vec![0, 0],
            upper: vec![Some(1); 2],
            shift: vec![0, 0],
            ordinal: Some(0),
            resource: "grain test allowance",
            requested: 2,
            limit: 1,
        })),
    );
    stream
}

// Compare complete queue/index contents, accounting, immutable domain IDs,
// canonical source progress and diagnostics. Only the outer owner-bucket map
// iteration is unordered; nested group/block/ID order remains significant.
fn canonical(state: &State<2>) -> Value {
    let mut queue = serde_json::to_value(&state.queue).unwrap();
    queue[2]
        .as_array_mut()
        .unwrap()
        .sort_by_cached_key(|bucket| serde_json::to_string(&(&bucket[0], &bucket[1])).unwrap());
    let mut progress = state.progress("test", 0, &json!({}));
    progress["parallel"]
        .as_object_mut()
        .unwrap()
        .remove("admission_preparation");
    json!({"queue_metadata":queue[0], "domains":queue[1], "buckets":queue[2], "ledger":queue[3],
        "progress":progress, "records":state.records, "details":state.details,
        "refusals":state.refusals, "optional":state.optional,
        "source":state.checkpoint_progress_metadata(), "error":state.error})
}
fn commit(
    engine: &Engine,
    state: &mut State<2>,
    req: &OwnerDomainWalkRequest,
    events: Vec<Event<2>>,
) -> Result<(), &'static str> {
    engine.commit_chunk(
        state,
        req,
        events,
        &AtomicBool::new(false),
        &AtomicBool::new(false),
        &mut |_| {},
    )
}
fn token_value(token: replay::Token, count: usize) -> Value {
    let mut replay = replay::Replay::default();
    replay.record_accepted(token, count).unwrap();
    replay.snapshot()
}
fn prepared_value(event: &PreparedEvent<2>) -> Value {
    match event {
        PreparedEvent::Original(event) => {
            json!({"original":token_value(replay::token(event).unwrap(), event.count)})
        }
        PreparedEvent::Invalid(error) => json!({"invalid":error}),
        PreparedEvent::Admission {
            count,
            successor,
            conditional,
            prepared,
            fingerprint,
        } => json!({
            "count":count, "successor":successor, "conditional":conditional,
            "checks":prepared.speculative_checks(),
            "fingerprint":fingerprint.map(|token| token_value(token, *count))}),
    }
}

#[test]
fn indexed_minimum_preserves_batch_edges_tokens_checks_and_ordered_outcomes() {
    let [default, grain] = engines();
    assert!(default.min_task_len.is_none());
    assert_eq!(grain.min_task_len.unwrap().get(), 8);
    assert_eq!(
        default.pool.as_ref().unwrap().current_num_threads(),
        grain.pool.as_ref().unwrap().current_num_threads()
    );
    let clear = AtomicBool::new(false);
    for n in [0, 1, 15, 16, 17, 255, 256, 257] {
        let mut a = seeded(MIN_CANDIDATES, None);
        let mut b = seeded(MIN_CANDIDATES, None);
        for offset in (0..n).step_by(BATCH_RECORDS) {
            let count = (n - offset).min(BATCH_RECORDS);
            let events = || admissions(n).into_iter().skip(offset).take(count).collect();
            let left = default.prepare_with_replay(
                &a.queue,
                events(),
                &clear,
                &clear,
                &mut a.admission,
                true,
            );
            let right = grain.prepare_with_replay(
                &b.queue,
                events(),
                &clear,
                &clear,
                &mut b.admission,
                true,
            );
            assert_eq!(left.len(), count);
            assert_eq!(right.len(), count);
            for (left, right) in left.into_iter().zip(right) {
                assert_eq!(prepared_value(&left), prepared_value(&right), "records {n}");
                assert_eq!(
                    left.commit(&mut a, &request()),
                    right.commit(&mut b, &request())
                );
                assert_eq!(canonical(&a), canonical(&b), "records {n}");
            }
        }
        assert_eq!(a.admission.records, b.admission.records);
        assert_eq!(a.admission.preparations, b.admission.preparations);
        assert_eq!(
            a.admission.speculative_checks,
            b.admission.speculative_checks
        );
        // Also exercise the actual chunk splitter, not only manual batches.
        let mut c = seeded(MIN_CANDIDATES, None);
        let mut d = seeded(MIN_CANDIDATES, None);
        assert_eq!(
            commit(&default, &mut c, &request(), admissions(n)),
            commit(&grain, &mut d, &request(), admissions(n))
        );
        assert_eq!(canonical(&c), canonical(&d));
        assert_eq!(canonical(&a), canonical(&c));
    }
}

#[test]
fn mixed_events_and_every_event_limit_keep_the_same_canonical_prefix() {
    let [default, grain] = engines();
    for limit in 0..=80 {
        let mut req = request();
        req.max_events = limit;
        let mut a = seeded(MIN_CANDIDATES, None);
        let mut b = seeded(MIN_CANDIDATES, None);
        assert_eq!(
            commit(&default, &mut a, &req, mixed()),
            commit(&grain, &mut b, &req, mixed()),
            "limit {limit}"
        );
        assert_eq!(canonical(&a), canonical(&b), "limit {limit}");
        assert_eq!(
            a.admission.speculative_checks,
            b.admission.speculative_checks
        );
    }
}

#[test]
fn geometry_invalid_future_and_counter_overflow_keep_ordered_authority() {
    let [default, grain] = engines();
    for (kind, limit) in [(0, 1000), (1, 1), (1, 1000)] {
        let events = || {
            let mut events = admissions(32);
            let mut d = domain(5, Some(4)); // Empty box, not omitted work.
            if kind == 1 {
                d.powers.min_power_difference = Some(20);
                d.powers.max_power_difference = Some(10);
            }
            events.insert(
                8,
                Event::one(Effect::Admit {
                    domain: d,
                    successor: true,
                    conditional: false,
                }),
            );
            events.push(Event::one(Effect::Admit {
                domain: domain(0, None),
                successor: true,
                conditional: true,
            }));
            events
        };
        let mut req = request();
        req.max_events = limit;
        let mut a = seeded(MIN_CANDIDATES, None);
        let mut b = seeded(MIN_CANDIDATES, None);
        assert_eq!(
            commit(&default, &mut a, &req, events()),
            commit(&grain, &mut b, &req, events())
        );
        assert_eq!(canonical(&a), canonical(&b));
    }
    for remaining in 0..8 {
        let mut a = seeded(MIN_CANDIDATES, None);
        let mut b = seeded(MIN_CANDIDATES, None);
        a.queue.containment_checks = usize::MAX - remaining;
        b.queue.containment_checks = usize::MAX - remaining;
        assert_eq!(
            commit(&default, &mut a, &request(), admissions(32)),
            commit(&grain, &mut b, &request(), admissions(32))
        );
        assert_eq!(canonical(&a), canonical(&b));
    }
}

#[test]
fn eligibility_and_finite_containment_caps_are_unchanged() {
    for (workers, candidates, cap, records) in [
        (1, 128, None, 16),
        (8, 127, None, 16),
        (8, 128, Some(100_000), 16),
        (8, 128, None, 15),
        (8, 128, None, 16),
    ] {
        let default = Engine::new(budget(workers)).unwrap();
        let grain =
            Engine::new_with_min_task_len(budget(workers), NonZeroUsize::new(8).unwrap()).unwrap();
        let mut a = seeded(candidates, cap);
        let mut b = seeded(candidates, cap);
        assert_eq!(
            commit(&default, &mut a, &request(), admissions(records)),
            commit(&grain, &mut b, &request(), admissions(records))
        );
        assert_eq!(canonical(&a), canonical(&b));
        let expected = usize::from(
            workers > 1
                && candidates >= MIN_CANDIDATES
                && cap.is_none()
                && records >= MIN_ADMISSIONS,
        );
        assert_eq!(
            (a.admission.batches, b.admission.batches),
            (expected, expected)
        );
    }
}

#[test]
fn cancelled_or_failed_preparations_never_publish_and_batches_still_stop_at_256() {
    for engine in engines() {
        for producer_failure in [false, true] {
            let cancellation = AtomicBool::new(false);
            let stop = AtomicBool::new(false);
            let mut state = seeded(MIN_CANDIDATES, None);
            let before = canonical(&state);
            let prepared = engine.prepare_with_replay(
                &state.queue,
                mixed(),
                &cancellation,
                &stop,
                &mut state.admission,
                true,
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
            assert_eq!(canonical(&state), before);
        }
        let cancellation = AtomicBool::new(false);
        let mut state = seeded(MIN_CANDIDATES, None);
        let mut heartbeats = 0;
        assert_eq!(
            engine.commit_chunk(
                &mut state,
                &request(),
                admissions(257),
                &cancellation,
                &AtomicBool::new(false),
                &mut |_| {
                    heartbeats += 1;
                    cancellation.store(true, Ordering::Release);
                }
            ),
            Err("cancelled")
        );
        assert_eq!(heartbeats, 1);
        assert_eq!(state.events, BATCH_RECORDS);
        assert_eq!(state.admission.records, BATCH_RECORDS);
        assert_eq!(
            state.replay.as_ref().unwrap().accepted_events(),
            BATCH_RECORDS
        );
    }
}

#[test]
fn replay_filters_prefix_before_grain_preparation_and_rejects_a_changed_prefix() {
    let [default, grain] = engines();
    let mut reference = seeded(MIN_CANDIDATES, None);
    commit(&default, &mut reference, &request(), mixed()).unwrap();
    let mut resumed_reference = None;
    for engine in [&default, &grain] {
        // Different record boundaries, including a compacted count callback,
        // must preserve the one canonical callback sequence.
        let mut state = seeded(MIN_CANDIDATES, None);
        commit(
            engine,
            &mut state,
            &request(),
            mixed().into_iter().take(17).collect(),
        )
        .unwrap();
        let saved = state.replay.as_ref().unwrap().snapshot();
        state.replay = Some(replay::Replay::restore(saved.clone()).unwrap());
        let before = canonical(&state);
        let mut changed = mixed();
        changed[0].count += 1;
        assert_eq!(
            commit(engine, &mut state, &request(), changed),
            Err("checkpoint replay prefix differs from committed callbacks")
        );
        assert_eq!(canonical(&state), before);
        state.replay = Some(replay::Replay::restore(saved).unwrap());
        let mut replayed = mixed();
        replayed[0].count = 1;
        replayed.insert(
            1,
            Event {
                count: 2,
                effect: Effect::Count,
            },
        );
        commit(engine, &mut state, &request(), replayed).unwrap();
        state.replay.as_ref().unwrap().finish().unwrap();
        if let Some(expected) = &resumed_reference {
            assert_eq!(&canonical(&state), expected);
        } else {
            resumed_reference = Some(canonical(&state));
        }
        // Immutable snapshots differ across restart, so attempted preparation
        // metrics are intentionally excluded; canonical work must not double.
        let mut expected = canonical(&reference);
        let mut actual = canonical(&state);
        // Comparison counts depend on the earlier accepted snapshot boundary;
        // replay equivalence concerns decisions, not retrospective speculation.
        for value in [&mut expected, &mut actual] {
            value["queue_metadata"]
                .as_object_mut()
                .unwrap()
                .remove("containment_checks");
            value["progress"]
                .as_object_mut()
                .unwrap()
                .remove("containment_checks");
        }
        assert_eq!(actual, expected);
    }
}

#[test]
fn replay_can_end_inside_a_compacted_count_before_parallel_admission() {
    for engine in engines() {
        let mut reference = seeded(MIN_CANDIDATES, None);
        commit(&engine, &mut reference, &request(), mixed()).unwrap();
        let mut state = seeded(MIN_CANDIDATES, None);
        commit(
            &engine,
            &mut state,
            &request(),
            vec![Event {
                count: 1,
                effect: Effect::Count,
            }],
        )
        .unwrap();
        let saved = state.replay.as_ref().unwrap().snapshot();
        state.replay = Some(replay::Replay::restore(saved).unwrap());
        commit(&engine, &mut state, &request(), mixed()).unwrap();
        state.replay.as_ref().unwrap().finish().unwrap();
        assert_eq!(canonical(&state), canonical(&reference));
        assert_eq!(state.admission.preparations, 64);
    }
}
