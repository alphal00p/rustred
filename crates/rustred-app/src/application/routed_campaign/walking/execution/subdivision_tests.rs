use super::super::{
    delegation::SchedulingPolicy,
    inspection::OptionalDiagnostic,
    physical_parts::ApplySubdivision,
    queue::{Domain, Phase},
};
use super::*;
use std::num::NonZeroUsize;

fn setup() -> (State<1>, OwnerDomainWalkRequest) {
    let mut request = OwnerDomainWalkRequest::new(crate::OwnerDomainMatchRequest::new(
        String::new(),
        String::new(),
    ));
    request.apply_subdivision = Some(ApplySubdivision { axis: 0, cut: 0 });
    let mut queue = Queue::with_policy(
        100,
        None,
        SchedulingPolicy::TransferUnreserved {
            lookahead: NonZeroUsize::new(1).unwrap(),
        },
    )
    .unwrap();
    queue
        .admit(Domain {
            phase: Phase::Apply,
            owner: [true],
            lower: vec![0],
            upper: vec![Some(2)],
            rank: Some(3),
            powers: Default::default(),
        })
        .unwrap();
    let mut state = State::new(queue, 0, None);
    state.physical_enabled = true;
    state.note_native_started(0).unwrap();
    (state, request)
}
fn finish(optional: usize) -> Finished {
    Finished {
        stats: NativeStats::Apply(rustred::solver::OwnerAppliedStats {
            events: optional,
            optional_coefficient_refusals: optional,
            optional_original_refusals: optional,
            ..Default::default()
        }),
        error: None,
        error_kind: "none",
        seconds: 0.125,
    }
}
fn refusal(part: u8) -> Event<1> {
    Event::one(Effect::Optional(OptionalDiagnostic {
        disposition: rustred::solver::OwnerDomainMatchDisposition::SelectedRule {
            batch: 0,
            rule: 1,
        },
        rank: Some(3),
        powers: Default::default(),
        lower: vec![u64::from(part)],
        upper: vec![Some(if part == 0 { 0 } else { 2 })],
        shift: vec![-1],
        ordinal: Some(2),
        resource: "test optional allowance",
        requested: 2,
        limit: 1,
    }))
}

#[test]
fn both_actual_parts_are_required_before_one_parent_publication() {
    let (mut state, request) = setup();
    state.accept(refusal(0), &request).unwrap();
    state.commit_physical(
        Ticket {
            parent: 0,
            part: Some(0),
        },
        finish(1),
        &request,
    );
    assert_eq!(state.queue.next, 0);
    assert_eq!(state.queue.domains.len(), 1); // No source part was admitted/deduplicated.
    assert_eq!(
        state
            .queue
            .delegation
            .as_ref()
            .unwrap()
            .native_publications(),
        0
    );
    assert_eq!(state.native_records, 0);
    state.refresh_closure(&AtomicBool::new(false), true);
    assert_eq!(state.closure_json()["initial_closed"], 0);
    assert_eq!(state.publisher_ticket(&request).part, Some(1));
    // A second first-original diagnostic belongs to the second native call.
    state.accept(refusal(1), &request).unwrap();
    state.commit_physical(
        Ticket {
            parent: 0,
            part: Some(1),
        },
        finish(1),
        &request,
    );
    assert!(state.error.is_none());
    assert_eq!(state.queue.next, 1);
    assert_eq!(state.native_records, 1);
    assert_eq!(state.physical_inspections_published, 2);
    assert_eq!(state.initial_entry_domains_inspected, 1);
    state.refresh_closure(&AtomicBool::new(false), true);
    assert_eq!(state.closure_json()["initial_closed"], 1);
    assert_eq!(state.records[0]["stats"]["optional_original_refusals"], 2);
    assert_eq!(state.records[0]["optional_refusals"][0]["physical_part"], 0);
    assert_eq!(state.records[0]["optional_refusals"][1]["physical_part"], 1);
    assert_eq!(
        state.finalize_delegation().unwrap()["all_ledger_obligations_discharged"],
        true
    );
}

#[test]
fn checkpoint_codec_retains_application_refinement_counts_in_completed_parts() {
    let (mut state, request) = setup();
    let mut finished = finish(0);
    finished.stats = NativeStats::Apply(rustred::solver::OwnerAppliedStats {
        application_refinement_steps: 3,
        application_refinement_cells: 7,
        ..Default::default()
    });
    state.commit_physical(
        Ticket {
            parent: 0,
            part: Some(0),
        },
        finished,
        &request,
    );
    let mut restored = super::super::checkpoint::round_trip_state(&state).unwrap();
    assert_eq!(restored.publisher_ticket(&request).part, Some(1));
    // Full restore returns unfinished native responsibilities to Reserved;
    // mirror the resumed dispatch before supplying the second part's Finished.
    assert!(restored.queue.delegation.as_ref().unwrap().can_dispatch(0));
    restored.note_native_started(0).unwrap();
    let mut finished = finish(0);
    finished.stats = NativeStats::Apply(rustred::solver::OwnerAppliedStats {
        application_refinement_steps: 5,
        application_refinement_cells: 11,
        ..Default::default()
    });
    restored.commit_physical(
        Ticket {
            parent: 0,
            part: Some(1),
        },
        finished,
        &request,
    );
    assert_eq!(restored.error, None);
    assert_eq!(
        restored.records[0]["stats"]["application_refinement_steps"],
        8
    );
    assert_eq!(
        restored.records[0]["stats"]["application_refinement_cells"],
        18
    );
    assert_eq!(
        restored.records[0]["physical_parts"][0]["stats"]["application_refinement_steps"],
        3
    );
    let again = super::super::checkpoint::round_trip_state(&restored).unwrap();
    assert_eq!(again.records, restored.records);
}

#[test]
fn native_refined_successor_prefix_survives_checkpoint_resume_w2_w6() {
    let reducer = super::initial_orthants_tests::native_fixture();
    for workers in [2, 6] {
        if let Err(error) =
            rustred::campaign::ParallelExecution::preflight_requested_core_budget(workers)
        {
            eprintln!("application-refinement checkpoint replay W{workers} skipped: {error:?}");
            continue;
        }
        if !symbolica::license::LicenseManager::is_licensed() {
            eprintln!(
                "application-refinement checkpoint replay W{workers} skipped: no licensed native execution"
            );
            continue;
        }
        let (_, mut request) = setup();
        request.workers = workers;
        request.apply_subdivision = Some(ApplySubdivision { axis: 0, cut: 1 });
        request.scheduling_policy = SchedulingPolicy::TransferUnreserved {
            lookahead: NonZeroUsize::new(1).unwrap(),
        };
        request.applied_limits.cell_refinement =
            rustred::solver::OwnerAppliedCellRefinement::SingleFiniteAxis {
                max_cardinality: NonZeroUsize::new(5).unwrap(),
            };
        let mut baseline = native_state(&request);
        run_checkpointed(
            &mut baseline,
            &reducer,
            &request,
            &AtomicBool::new(false),
            &|_| {},
            &mut |_| Ok(()),
        );
        assert_eq!(baseline.error, None);
        assert!(
            baseline.records[0]["stats"]["application_refinement_cells"]
                .as_u64()
                .unwrap()
                > 0
        );

        let mut prefix = native_state(&request);
        prefix.physical_enabled = true;
        prefix.replay = Some(replay::Replay::default());
        prefix.note_native_started(0).unwrap();
        let stop = AtomicBool::new(false);
        let initial = InitialOrthants::from_initial(&prefix.queue.domains, &stop);
        let parts = prefix.parts(0, &request).unwrap();
        let first = inspection::inspect_part(
            &reducer,
            &parts[0],
            &request,
            0,
            &stop,
            &initial,
            &mut |event| {
                prefix.accept(event, &request).unwrap();
                ControlFlow::Continue(())
            },
        );
        assert!(first.error.is_none());
        prefix.commit_physical(
            Ticket {
                parent: 0,
                part: Some(0),
            },
            first,
            &request,
        );
        let previous_successors = prefix.successors;
        let interrupted = inspection::inspect_part(
            &reducer,
            &parts[1],
            &request,
            1,
            &stop,
            &initial,
            &mut |event| {
                prefix.accept(event, &request).unwrap();
                if prefix.successors > previous_successors {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            },
        );
        assert!(interrupted.error.is_some());
        let partial = native_stats(interrupted.stats);
        assert!(partial["application_refinement_cells"].as_u64().unwrap() > 0);
        assert!(prefix.replay.as_ref().unwrap().accepted_events() > 1);
        prefix.checkpoint_paused = true;
        prefix
            .uncommitted
            .push(json!({"id":0,"physical_part":1,"committed":false,
            "stats":partial,"error":interrupted.error}));
        let mut resumed =
            super::super::checkpoint::round_trip_state_on_disk(&prefix, &request).unwrap();
        assert_eq!(resumed.uncommitted, prefix.uncommitted);
        run_checkpointed(
            &mut resumed,
            &reducer,
            &request,
            &AtomicBool::new(false),
            &|_| {},
            &mut |_| Ok(()),
        );
        assert_native_equivalent(&mut baseline, &mut resumed);
        eprintln!(
            "application-refinement checkpoint replay W{workers} executed: licensed native refined prefix, on-disk restore, exact completed records PASS"
        );
    }
}

#[test]
fn split_policy_does_not_apply_to_new_descendant_responsibilities() {
    let (mut state, request) = setup();
    let descendant = Domain {
        phase: Phase::Apply,
        owner: [true],
        lower: vec![0],
        upper: vec![Some(4)],
        rank: Some(4),
        powers: Default::default(),
    };
    let (id, _) = state.queue.admit(descendant).unwrap();
    assert_eq!(id, 1);
    assert!(
        request
            .apply_subdivision
            .unwrap()
            .parts(&state.queue.domains[id])
            .is_some()
    );
    assert!(state.parts(id, &request).is_none());
}

#[test]
fn completed_parent_moves_frontiers_once_and_retains_part_provenance() {
    let (mut state, request) = setup();
    for part in 0..2 {
        state
            .accept(
                Event::one(Effect::Frontier {
                    value: json!({"kind":"test", "payload":"retained once"}),
                    successor: false,
                    conditional: false,
                }),
                &request,
            )
            .unwrap();
        state.commit_physical(
            Ticket {
                parent: 0,
                part: Some(part),
            },
            finish(0),
            &request,
        );
        if part == 0 {
            assert_eq!(
                state.physical_progress.as_ref().unwrap().completed[0]["frontiers"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
        }
    }
    let parent = &state.records[0];
    assert_eq!(parent["frontiers"].as_array().unwrap().len(), 2);
    for part in 0..2 {
        assert_eq!(parent["frontiers"][part]["physical_part"], part);
        assert_eq!(parent["physical_parts"][part]["frontier_count"], 1);
        assert_eq!(parent["physical_parts"][part]["frontiers"], json!([]));
        assert_eq!(
            parent["physical_parts"][part]["frontiers_published_on_parent"],
            true
        );
    }
    assert_eq!(parent["local_classification_discharged"], false);
}

#[test]
fn failed_group_cleanup_cannot_publish_success_from_finished_sibling() {
    let (mut state, request) = setup();
    state.error = Some("cancelled".into());
    let mut failed = finish(0);
    failed.error = Some("cancelled".into());
    failed.error_kind = "cancelled";
    let mut leftovers = vec![(2, finish(0)), (1, failed)];
    retain_physical_leftovers(&mut state, &mut leftovers, &request);
    assert_eq!(state.queue.next, 1);
    assert_eq!(state.records.len(), 1);
    assert_eq!(state.completed, 0);
    assert_eq!(
        state.records[0]["physical_parts"].as_array().unwrap().len(),
        2
    );
    assert_eq!(state.records[0]["local_classification_discharged"], false);
    assert_eq!(state.finalize_delegation().unwrap()["native_cancelled"], 1);
}

#[test]
fn physical_checkpoint_retains_completed_first_part_and_only_replays_second_prefix() {
    let (mut state, request) = setup();
    state.replay = Some(replay::Replay::default());
    state.accept(refusal(0), &request).unwrap();
    state.commit_physical(
        Ticket {
            parent: 0,
            part: Some(0),
        },
        finish(1),
        &request,
    );
    state.accept(refusal(1), &request).unwrap();
    let saved = state.checkpoint_progress();
    let before = (state.events, state.queue.domains.len());
    state.restore_checkpoint_progress(saved).unwrap();
    assert_eq!(state.publisher_ticket(&request).part, Some(1));
    let mut replayed = refusal(1);
    assert!(!state.filter_replay(&mut replayed).unwrap());
    assert_eq!((state.events, state.queue.domains.len()), before);
    state.commit_physical(
        Ticket {
            parent: 0,
            part: Some(1),
        },
        finish(1),
        &request,
    );
    assert_eq!(state.queue.next, 1);
    assert_eq!(
        state.records[0]["physical_parts"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        state.records[0]["optional_refusals"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(state.events, 2);
}

#[test]
fn cancellation_racing_an_admission_or_replay_fault_cannot_make_a_resume_checkpoint() {
    // Both serial and pooled paths use this same fail-closed pause predicate.
    for error in [
        "checkpoint replay prefix differs from committed callbacks",
        "checkpoint replay ended before committed prefix",
        "coordinator accounting failure",
        "resumed attempted-work counter overflow",
    ] {
        assert!(!resumable_cancellation(
            true,
            Some("cancelled"),
            Some(error)
        ));
        assert!(!resumable_cancellation(
            true,
            Some("coordinator_admission"),
            Some(error)
        ));
    }
    assert!(!resumable_cancellation(true, Some("native_failure"), None));
    assert!(resumable_cancellation(true, Some("cancelled"), None));
    assert!(resumable_cancellation(
        true,
        Some("cancelled"),
        Some("cancelled")
    ));
    assert!(!resumable_cancellation(false, Some("cancelled"), None));
}

fn native_state(request: &OwnerDomainWalkRequest) -> State<1> {
    let mut queue = Queue::with_policy(100, None, request.scheduling_policy).unwrap();
    queue
        .admit(Domain {
            phase: Phase::Apply,
            owner: [true],
            lower: vec![0],
            upper: vec![Some(4)],
            rank: Some(0),
            powers: Default::default(),
        })
        .unwrap();
    State::new(queue, 0, None)
}

fn canonical(mut value: Value) -> Value {
    fn remove_times(value: &mut Value) {
        match value {
            Value::Object(object) => {
                object.remove("seconds");
                object.remove("physical_seconds_sum");
                for child in object.values_mut() {
                    remove_times(child);
                }
            }
            Value::Array(array) => {
                for child in array {
                    remove_times(child);
                }
            }
            _ => {}
        }
    }
    remove_times(&mut value);
    value
}

fn assert_native_equivalent(a: &mut State<1>, b: &mut State<1>) {
    assert_eq!(a.error, None);
    assert_eq!(b.error, None);
    assert_eq!(a.queue.next, a.queue.domains.len());
    assert_eq!(b.queue.next, b.queue.domains.len());
    assert_eq!(a.queue.domains, b.queue.domains);
    assert_eq!(canonical(json!(a.records)), canonical(json!(b.records)));
    assert_eq!(
        a.finalize_delegation().unwrap(),
        b.finalize_delegation().unwrap()
    );
    assert_eq!(
        (
            a.events,
            a.successors,
            a.conditional,
            a.frontiers,
            a.completed,
            a.native_records,
            a.initial_entry_domains_inspected,
            a.physical_inspections_published,
            a.queue.deduplicated,
            a.queue.containment_checks,
            a.queue.exact_hits
        ),
        (
            b.events,
            b.successors,
            b.conditional,
            b.frontiers,
            b.completed,
            b.native_records,
            b.initial_entry_domains_inspected,
            b.physical_inspections_published,
            b.queue.deduplicated,
            b.queue.containment_checks,
            b.queue.exact_hits
        ),
    );
}

#[test]
fn native_subdivision_repeated_on_disk_resume_preserves_parent_and_exact_prefix_w2_w6() {
    let reducer = super::initial_orthants_tests::native_fixture();
    for workers in [2, 6] {
        if rustred::campaign::ParallelExecution::preflight_requested_core_budget(workers).is_err()
            || !symbolica::license::LicenseManager::is_licensed()
        {
            continue;
        }
        let (_, mut request) = setup();
        request.workers = workers;
        request.apply_subdivision = Some(ApplySubdivision { axis: 0, cut: 1 });
        request.scheduling_policy = SchedulingPolicy::TransferUnreserved {
            lookahead: NonZeroUsize::new(1).unwrap(),
        };
        let mut baseline = native_state(&request);
        run_checkpointed(
            &mut baseline,
            &reducer,
            &request,
            &AtomicBool::new(false),
            &|_| {},
            &mut |_| Ok(()),
        );
        assert_eq!(baseline.error, None);
        assert_eq!(baseline.records[0]["physical_parts_expected"], 2);

        let mut interrupted = native_state(&request);
        let mut accepted_prefix = None;
        for _ in 0..2 {
            let stop = AtomicBool::new(false);
            let mut saved_prefix = None;
            run_checkpointed(
                &mut interrupted,
                &reducer,
                &request,
                &stop,
                &|_| {},
                &mut |state| {
                    // A real native callback prefix is published, but no Finished
                    // for part1 has been published. Part0 must survive both restarts.
                    if state
                        .physical_progress
                        .as_ref()
                        .is_some_and(|p| p.parent == 0 && p.completed.len() == 1)
                        && state
                            .replay
                            .as_ref()
                            .is_some_and(|r| r.accepted_events() > 0)
                    {
                        saved_prefix = Some((
                            state.events,
                            state.replay.as_ref().unwrap().accepted_events(),
                        ));
                        stop.store(true, Ordering::Release);
                    }
                    Ok(())
                },
            );
            assert_eq!(interrupted.error, None);
            assert!(
                interrupted.checkpoint_paused,
                "W{workers} did not pause within physical parent"
            );
            assert_eq!(interrupted.queue.next, 0);
            assert_eq!(interrupted.native_records, 0);
            assert!(saved_prefix.is_some());
            if let Some(previous) = accepted_prefix {
                assert_eq!(saved_prefix, Some(previous));
            }
            accepted_prefix = saved_prefix;
            interrupted =
                super::super::checkpoint::round_trip_state_on_disk(&interrupted, &request).unwrap();
            assert_eq!(interrupted.publisher_ticket(&request).part, Some(1));
            assert!(
                interrupted
                    .queue
                    .delegation
                    .as_ref()
                    .unwrap()
                    .can_dispatch(0)
            );
        }
        run_checkpointed(
            &mut interrupted,
            &reducer,
            &request,
            &AtomicBool::new(false),
            &|_| {},
            &mut |_| Ok(()),
        );
        assert_eq!(interrupted.parallel["active_workers"], 0);
        assert_native_equivalent(&mut baseline, &mut interrupted);
    }
}

#[test]
fn native_mid_stream_part_prefix_resumes_without_readmitting_callbacks() {
    let reducer = super::initial_orthants_tests::native_fixture();
    for workers in [2, 6] {
        if rustred::campaign::ParallelExecution::preflight_requested_core_budget(workers).is_err()
            || !symbolica::license::LicenseManager::is_licensed()
        {
            continue;
        }
        let (_, mut request) = setup();
        request.workers = workers;
        request.apply_subdivision = Some(ApplySubdivision { axis: 0, cut: 1 });
        request.scheduling_policy = SchedulingPolicy::TransferUnreserved {
            lookahead: NonZeroUsize::new(1).unwrap(),
        };
        let mut baseline = native_state(&request);
        run_checkpointed(
            &mut baseline,
            &reducer,
            &request,
            &AtomicBool::new(false),
            &|_| {},
            &mut |_| Ok(()),
        );
        assert_eq!(baseline.error, None);
        assert!(
            baseline.records[0]["physical_parts"][1]["stats"]["events"]
                .as_u64()
                .unwrap()
                > 1
        );

        // Stop an actual native call after its first logical callback, not a
        // fabricated Finished or a fully consumed worker chunk. The normal
        // coordinator then restores and verifies this strict stream prefix.
        let mut prefix = native_state(&request);
        prefix.physical_enabled = true;
        prefix.replay = Some(replay::Replay::default());
        prefix.note_native_started(0).unwrap();
        let stop = AtomicBool::new(false);
        let initial = InitialOrthants::from_initial(&prefix.queue.domains, &stop);
        let parts = prefix.parts(0, &request).unwrap();
        let first = inspection::inspect_part(
            &reducer,
            &parts[0],
            &request,
            0,
            &stop,
            &initial,
            &mut |event| {
                prefix.accept(event, &request).unwrap();
                ControlFlow::Continue(())
            },
        );
        assert!(first.error.is_none());
        prefix.commit_physical(
            Ticket {
                parent: 0,
                part: Some(0),
            },
            first,
            &request,
        );
        let interrupted = inspection::inspect_part(
            &reducer,
            &parts[1],
            &request,
            1,
            &stop,
            &initial,
            &mut |event| {
                prefix.accept(event, &request).unwrap();
                ControlFlow::Break(())
            },
        );
        assert!(interrupted.error.is_some());
        assert_eq!(prefix.replay.as_ref().unwrap().accepted_events(), 1);
        prefix.checkpoint_paused = true;
        prefix
            .uncommitted
            .push(json!({"id":0,"physical_part":1,"committed":false,
            "stats":native_stats(interrupted.stats),"error":interrupted.error}));
        let mut resumed =
            super::super::checkpoint::round_trip_state_on_disk(&prefix, &request).unwrap();
        run_checkpointed(
            &mut resumed,
            &reducer,
            &request,
            &AtomicBool::new(false),
            &|_| {},
            &mut |_| Ok(()),
        );
        assert_native_equivalent(&mut baseline, &mut resumed);

        // A changed prefix must fail closed even if a caller signal arrives
        // while the coordinator reports that replay failure.
        let mut corrupt = super::super::checkpoint::round_trip_state(&prefix).unwrap();
        let mut progress = corrupt.checkpoint_progress();
        let digest = progress["replay"]["digest"].as_array_mut().unwrap();
        digest[0] = json!(digest[0].as_u64().unwrap() ^ 1);
        corrupt.restore_checkpoint_progress(progress).unwrap();
        let signal = AtomicBool::new(false);
        run_checkpointed(
            &mut corrupt,
            &reducer,
            &request,
            &signal,
            &|event| {
                if !event["parallel"]["first_failure"].is_null() {
                    signal.store(true, Ordering::Release);
                }
            },
            &mut |_| Ok(()),
        );
        assert!(
            corrupt
                .error
                .as_deref()
                .is_some_and(|e| e.contains("checkpoint replay")),
            "{:?}",
            corrupt.error
        );
        assert!(!corrupt.checkpoint_paused);
    }
}
