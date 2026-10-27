use super::tests::{Scratch, prepared_fixture};
use super::*;
use rustred::algebra::CoefficientContext;
use rustred::persistence::application_cache::{ApplicationCacheKey, ApplicationCacheRecord};
use rustred::reduction::{GuardedCacheBackingError, GuardedIntermediateCapture};
use rustred::solver::{
    CandidateRoutedActivity, CandidateRoutedApplicationEnvelope, CandidateRoutedApplicationEvent,
    CandidateRoutedApplicationObserver, CandidateRoutedApplicationStatus,
    CandidateRoutedCompletionSource, CandidateRoutedObservationOptions,
    CandidateRoutedObserverControl, CandidateRoutedObserverFault, CandidateRoutedObserverStatus,
};

fn immediate_options() -> CandidateRoutedObservationOptions {
    CandidateRoutedObservationOptions {
        interval: Duration::ZERO,
        ..Default::default()
    }
}

fn assert_same_reduction(actual: &SavedApplicationReduction, expected: &SavedApplicationReduction) {
    assert_eq!(actual.terms(), expected.terms());
    assert_eq!(actual.unresolved(), expected.unresolved());
    assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    assert_eq!(
        actual.terminal_substitution_additions(),
        expected.terminal_substitution_additions()
    );
}

fn assert_success_boundaries(documents: &[Value]) {
    let core_done = documents
        .iter()
        .position(|event| {
            event["observation_scope"] == "native_core"
                && event["event_kind"] == "core_finished"
                && event["status"] == "succeeded"
        })
        .expect("native success must be visible before terminal substitution");
    let terminal_start = documents
        .iter()
        .position(|event| event["event_kind"] == "terminal_substitution_started")
        .expect("the saved facade must observe its terminal substitution");
    assert!(core_done < terminal_start);
    assert_eq!(documents[core_done]["core_summary_live"], true);
    assert_eq!(documents[core_done]["core_summary_age_seconds"], 0.0);
    assert_eq!(documents[terminal_start]["core_summary_live"], false);
    assert!(documents[terminal_start]["core_summary_age_seconds"].is_number());
    assert_eq!(
        documents[terminal_start]["observation_scope"],
        "saved_application"
    );
    assert!(
        documents[..documents.len() - 1]
            .iter()
            .all(|event| event["request_finished"] == false)
    );
    let last = documents.last().unwrap();
    assert_eq!(last["observation_scope"], "saved_application");
    assert_eq!(last["event_kind"], "request_finished");
    assert_eq!(last["request_finished"], true);
    assert_eq!(last["status"], "succeeded");
}

#[test]
fn saved_observation_preserves_direct_and_routed_results_and_completion_scope() {
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let context = application.family().coefficient_context().clone();
    let input = [
        (IntegralKey::try_new([4]).unwrap(), context.integer(3)),
        (IntegralKey::try_new([2]).unwrap(), context.integer(-2)),
    ];
    for routed in [false, true] {
        application.clear_cache().unwrap();
        let expected = if routed {
            application.reduce_routed_linear_combination(input.clone())
        } else {
            application.reduce_linear_combination(input.clone())
        }
        .unwrap();
        application.clear_cache().unwrap();
        let mut documents = Vec::new();
        let mut terminal_events = Vec::new();
        let mut core_metrics = None;
        let mut final_metrics = None;
        let actual = {
            let mut sink = |event: &CandidateRoutedApplicationEnvelope<'_>| {
                documents.push(saved_application_observation_document(event));
                if let Some(terminal) = event.terminal {
                    terminal_events.push((event.event, terminal));
                }
                if event.event == CandidateRoutedApplicationEvent::CoreFinished {
                    core_metrics = Some(event.metrics);
                }
                if event.event == CandidateRoutedApplicationEvent::RequestFinished {
                    final_metrics = Some(event.metrics);
                }
                Ok(CandidateRoutedObserverControl::Continue)
            };
            let mut observer =
                CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
            let result = if routed {
                application.reduce_routed_linear_combination_with_observation(
                    input.clone(),
                    &mut observer,
                    None,
                )
            } else {
                application.reduce_linear_combination_with_observation(
                    input.clone(),
                    &mut observer,
                    None,
                )
            }
            .unwrap();
            assert_eq!(observer.status(), CandidateRoutedObserverStatus::Active);
            result
        };
        assert_same_reduction(&actual, &expected);
        assert_eq!(actual.statistics(), expected.statistics());
        assert_success_boundaries(&documents);
        assert!(core_metrics.is_some());
        assert_eq!(final_metrics, core_metrics);
        let (event, terminal) = terminal_events.last().unwrap();
        assert_eq!(*event, CandidateRoutedApplicationEvent::RequestFinished);
        assert_eq!(terminal.completed_rows, terminal.rows);
        assert!(terminal.rows > 0);
        assert_eq!(terminal.additions, actual.terminal_substitution_additions());
        assert_eq!(terminal.output_terms, Some(actual.terms().len()));
    }
}

#[derive(Default)]
struct Capture(BTreeMap<ApplicationCacheKey, ApplicationCacheRecord>);

impl GuardedIntermediateCapture for Capture {
    fn wants(
        &mut self,
        _: &ApplicationCacheKey,
        _: &CoefficientContext,
    ) -> Result<bool, GuardedCacheBackingError> {
        Ok(true)
    }

    fn store_completed(
        &mut self,
        record: &ApplicationCacheRecord,
        _: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        self.0.insert(record.key.clone(), record.clone());
        Ok(())
    }
}

#[test]
fn saved_cache_observation_preserves_capture_backing_hits_and_terminal_substitution() {
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let context = application.family().coefficient_context().clone();
    let binding = application.cache_binding([37; 32]);
    let key = IntegralKey::try_new([4]).unwrap();
    let input = [(key.clone(), context.one())];
    let expected = application
        .reduce_linear_combination(input.clone())
        .unwrap();
    application.clear_cache().unwrap();
    let mut capture = Capture::default();
    let mut documents = Vec::new();
    let cold = {
        let mut sink = |event: &CandidateRoutedApplicationEnvelope<'_>| {
            documents.push(saved_application_observation_document(event));
            Ok(CandidateRoutedObserverControl::Continue)
        };
        let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
        application
            .reduce_linear_combination_with_cache_and_observation(
                input.clone(),
                &binding,
                None,
                Some(&mut capture),
                &mut observer,
                None,
            )
            .unwrap()
    };
    assert_same_reduction(&cold, &expected);
    assert_success_boundaries(&documents);
    let root = application
        .completed_cache_record(&binding, &ApplicationCacheKey::route(&key))
        .unwrap()
        .unwrap();
    assert_eq!(capture.0[&root.key], root);
    for record in capture.0.values() {
        application
            .validate_completed_cache_record(&binding, record)
            .unwrap();
    }
    let records = capture.0.into_values().collect::<Vec<_>>();
    let (bytes, identity) = SavedApplicationCacheFile::encode_generated(
        &binding,
        &records,
        &context,
        Default::default(),
    )
    .unwrap();
    let mut backing = SavedApplicationCacheFile::decode_generated(
        &bytes,
        &binding,
        &identity,
        Default::default(),
    )
    .unwrap();
    application.clear_cache().unwrap();
    documents.clear();
    let mut backing_hit = false;
    let warm = {
        let mut sink = |event: &CandidateRoutedApplicationEnvelope<'_>| {
            backing_hit |= event.event
                == CandidateRoutedApplicationEvent::NodeCompleted(
                    CandidateRoutedCompletionSource::BackingHit,
                );
            documents.push(saved_application_observation_document(event));
            Ok(CandidateRoutedObserverControl::Continue)
        };
        let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
        application
            .reduce_routed_linear_combination_with_cache_and_observation(
                input,
                &binding,
                Some(&mut backing),
                None,
                &mut observer,
                None,
            )
            .unwrap()
    };
    assert_same_reduction(&warm, &expected);
    assert_success_boundaries(&documents);
    assert!(backing_hit);
    assert_eq!(warm.statistics().persistent_cache.requested_root_hits, 1);
    assert_eq!(warm.statistics().work.rule_applications(), 0);
}

#[test]
fn saved_observation_callback_errors_and_panics_preserve_the_exact_result() {
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let input = [(
        IntegralKey::try_new([4]).unwrap(),
        application.family().coefficient_context().one(),
    )];
    let expected = application
        .reduce_linear_combination(input.clone())
        .unwrap();
    for routed in [false, true] {
        for panic_in_callback in [false, true] {
            application.clear_cache().unwrap();
            let mut calls = 0;
            let mut calls_at_fault = None;
            let actual = {
                let mut sink = |event: &CandidateRoutedApplicationEnvelope<'_>| {
                    calls += 1;
                    if event.event == CandidateRoutedApplicationEvent::TerminalSubstitutionStarted {
                        calls_at_fault = Some(calls);
                        if panic_in_callback {
                            panic!("saved terminal observer failure");
                        }
                        return Err(());
                    }
                    Ok(CandidateRoutedObserverControl::Continue)
                };
                let mut observer =
                    CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
                let result = if routed {
                    application.reduce_routed_linear_combination_with_observation(
                        input.clone(),
                        &mut observer,
                        None,
                    )
                } else {
                    application.reduce_linear_combination_with_observation(
                        input.clone(),
                        &mut observer,
                        None,
                    )
                }
                .unwrap();
                assert_eq!(
                    observer.status(),
                    CandidateRoutedObserverStatus::Faulted(if panic_in_callback {
                        CandidateRoutedObserverFault::Panicked
                    } else {
                        CandidateRoutedObserverFault::CallbackError
                    })
                );
                result
            };
            assert_same_reduction(&actual, &expected);
            assert_eq!(Some(calls), calls_at_fault);
            let next = application
                .reduce_linear_combination(input.clone())
                .unwrap();
            assert_same_reduction(&next, &expected);
        }
    }
}

#[test]
fn saved_observation_precancellation_finishes_every_facade_path_without_substitution() {
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let input = [(
        IntegralKey::try_new([4]).unwrap(),
        application.family().coefficient_context().one(),
    )];
    let binding = application.cache_binding([43; 32]);
    let cancel = AtomicBool::new(true);
    for mode in 0..4 {
        application.clear_cache().unwrap();
        let mut documents = Vec::new();
        let error = {
            let mut sink = |event: &CandidateRoutedApplicationEnvelope<'_>| {
                documents.push(saved_application_observation_document(event));
                Ok(CandidateRoutedObserverControl::Continue)
            };
            let mut observer =
                CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
            match mode {
                0 => application.reduce_linear_combination_with_observation(
                    input.clone(),
                    &mut observer,
                    Some(&cancel),
                ),
                1 => application.reduce_routed_linear_combination_with_observation(
                    input.clone(),
                    &mut observer,
                    Some(&cancel),
                ),
                2 => application.reduce_linear_combination_with_cache_and_observation(
                    input.clone(),
                    &binding,
                    None,
                    None,
                    &mut observer,
                    Some(&cancel),
                ),
                _ => application.reduce_routed_linear_combination_with_cache_and_observation(
                    input.clone(),
                    &binding,
                    None,
                    None,
                    &mut observer,
                    Some(&cancel),
                ),
            }
            .unwrap_err()
        };
        assert!(error.to_string().contains("cancelled"));
        assert_eq!(application.cache_statistics().cached_integrals(), 0);
        assert!(documents.iter().all(|event| {
            event["event_kind"] != "terminal_substitution_started"
                && (event["observation_scope"] != "native_core"
                    || event["request_finished"] == false)
        }));
        let last = documents.last().unwrap();
        assert_eq!(last["observation_scope"], "saved_application");
        assert_eq!(last["event_kind"], "request_finished");
        assert_eq!(last["request_finished"], true);
        assert_eq!(last["status"], "failed");
    }
    assert!(application.reduce_linear_combination(input).is_ok());
}

#[test]
fn caller_observation_preserves_its_phase_and_running_status() {
    let mut envelope = CandidateRoutedApplicationEnvelope::new(
        CandidateRoutedApplicationEvent::CallerProgress,
        "caller-family",
    );
    envelope.activity = CandidateRoutedActivity::Caller;
    envelope.caller_phase = Some("special_symbolic");
    envelope.complexity.terms = Some(3);
    envelope.complexity.encoded_expression_bytes = Some(127);

    let document = saved_application_observation_document(&envelope);
    assert_eq!(document["observation_scope"], "caller");
    assert_eq!(document["event_kind"], "caller_progress");
    assert_eq!(document["activity"], "caller");
    assert_eq!(document["phase"], "special_symbolic");
    assert_eq!(document["caller_phase"], "special_symbolic");
    assert_eq!(document["status"], "running");
    assert_eq!(document["request_finished"], false);
    assert!(document["core"].is_null());
    assert!(document["terminal"].is_null());
    assert!(document["stored_state"].is_null());
    assert_eq!(document["complexity"]["terms"], 3);
    assert_eq!(document["complexity"]["encoded_expression_bytes"], 127);
    assert!(document["complexity"]["coefficient_bytes"].is_null());

    envelope.event = CandidateRoutedApplicationEvent::RequestFinished;
    envelope.status = CandidateRoutedApplicationStatus::Succeeded;
    envelope.request_finished = true;
    let document = saved_application_observation_document(&envelope);
    assert_eq!(document["observation_scope"], "caller");
    assert_eq!(document["event_kind"], "request_finished");
    assert_eq!(document["phase"], "special_symbolic");
    assert_eq!(document["status"], "succeeded");
    assert_eq!(document["request_finished"], true);
}

#[test]
fn caller_failure_keeps_the_successful_native_summary_explicitly_stale() {
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let input = [(
        IntegralKey::try_new([4]).unwrap(),
        application.family().coefficient_context().one(),
    )];
    let mut documents = Vec::new();
    {
        let mut sink = |event: &CandidateRoutedApplicationEnvelope<'_>| {
            documents.push(saved_application_observation_document(event));
            Ok(CandidateRoutedObserverControl::Continue)
        };
        let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
        application
            .reduce_linear_combination_with_observation(input, &mut observer, None)
            .unwrap();
        let mut caller = CandidateRoutedApplicationEnvelope::new(
            CandidateRoutedApplicationEvent::RequestFinished,
            application.family().fingerprint(),
        );
        caller.activity = CandidateRoutedActivity::Caller;
        caller.caller_phase = Some("numerical_materialization");
        caller.status = CandidateRoutedApplicationStatus::Failed;
        caller.request_finished = true;
        caller.snapshot = observer.last_snapshot();
        caller.metrics = observer.last_metrics();
        caller.stored_state = observer.last_stored_state();
        observer.emit(caller);
    }
    let last = documents.last().unwrap();
    assert_eq!(last["observation_scope"], "caller");
    assert_eq!(last["status"], "failed");
    assert_eq!(last["core"]["status"], "succeeded");
    assert_eq!(last["core_summary_scope"], "latest_native_request");
    assert_eq!(last["core_summary_live"], false);
    assert!(last["core_summary_age_seconds"].is_number());
    assert_eq!(last["metrics"]["completed_roots"], 1);
    assert_eq!(last["stored_state"]["unstarted_roots"], 0);
    assert_eq!(last["stored_state"]["scope"], "latest_native_request");
    assert_eq!(
        last["stored_state"]["sample_age_seconds"],
        last["core_summary_age_seconds"]
    );
}

#[test]
fn reused_observer_does_not_leak_previous_core_summary_after_binding_failure() {
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let input = [(
        IntegralKey::try_new([4]).unwrap(),
        application.family().coefficient_context().one(),
    )];
    let mut binding = application.cache_binding([53; 32]);
    binding.layer.push_str("-invalid");
    let mut documents = Vec::new();
    {
        let mut sink = |event: &CandidateRoutedApplicationEnvelope<'_>| {
            documents.push(saved_application_observation_document(event));
            Ok(CandidateRoutedObserverControl::Continue)
        };
        let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
        application
            .reduce_linear_combination_with_observation(input.clone(), &mut observer, None)
            .unwrap();
        assert!(observer.last_snapshot().is_some());
        assert!(observer.last_stored_state().is_some());
        assert!(observer.last_metrics().completed_roots > 0);
        let error = application
            .reduce_linear_combination_with_cache_and_observation(
                input,
                &binding,
                None,
                None,
                &mut observer,
                None,
            )
            .unwrap_err();
        assert!(error.to_string().contains("binding mismatch"));
        assert!(observer.last_snapshot().is_none());
        assert!(observer.last_stored_state().is_none());
        assert_eq!(observer.last_metrics(), Default::default());
    }
    let last = documents.last().unwrap();
    assert_eq!(last["observation_scope"], "saved_application");
    assert_eq!(last["event_kind"], "request_finished");
    assert_eq!(last["status"], "failed");
    assert_eq!(last["request_finished"], true);
    assert!(last["core"].is_null());
    assert!(last["stored_state"].is_null());
    assert!(last["metrics"]["output_terms"].is_null());
    assert_eq!(last["metrics"]["completed_roots"], 0);
}

#[test]
fn saved_terminal_cancellation_does_not_report_core_success_as_request_success() {
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let input = [(
        IntegralKey::try_new([4]).unwrap(),
        application.family().coefficient_context().one(),
    )];
    let cancel = AtomicBool::new(false);
    let mut documents = Vec::new();
    let error = {
        let mut sink = |event: &CandidateRoutedApplicationEnvelope<'_>| {
            documents.push(saved_application_observation_document(event));
            if event.event == CandidateRoutedApplicationEvent::TerminalSubstitutionStarted {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            Ok(CandidateRoutedObserverControl::Continue)
        };
        let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
        application
            .reduce_linear_combination_with_observation(input.clone(), &mut observer, Some(&cancel))
            .unwrap_err()
    };
    assert!(error.to_string().contains("cancelled"));
    assert!(
        documents
            .iter()
            .any(|event| event["event_kind"] == "core_finished"
                && event["status"] == "succeeded"
                && event["request_finished"] == false)
    );
    let last = documents.last().unwrap();
    assert_eq!(last["event_kind"], "request_finished");
    assert_eq!(last["status"], "failed");
    assert_eq!(last["request_finished"], true);
    assert_eq!(last["terminal"]["completed_rows"], 0);
    assert!(last["terminal"]["output_terms"].is_null());
    assert!(application.reduce_linear_combination(input).is_ok());
}

#[test]
fn saved_observer_fault_preserves_resource_cause_and_retry() {
    use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};
    let scratch = Scratch::new();
    let mut application = prepared_fixture(&scratch.0.join("source"));
    let input = [(
        IntegralKey::try_new([4]).unwrap(),
        application.family().coefficient_context().one(),
    )];
    let expired = ResourceBudget::new(ResourceLimits {
        max_process_rss_bytes: None,
        max_elapsed: Some(Duration::ZERO),
    });
    let mut sink = |_: &CandidateRoutedApplicationEnvelope<'_>| Err(());
    let mut observer = CandidateRoutedApplicationObserver::new(immediate_options(), &mut sink);
    let error = expired
        .run(|| {
            application.reduce_linear_combination_with_observation(
                input.clone(),
                &mut observer,
                None,
            )
        })
        .unwrap_err();
    assert_eq!(
        observer.status(),
        CandidateRoutedObserverStatus::Faulted(CandidateRoutedObserverFault::CallbackError)
    );
    assert_eq!(error.kind(), crate::AppErrorKind::Limit);
    assert!(matches!(
        error.resource_limit_error(),
        Some(ResourceLimitError::Elapsed { .. })
    ));
    assert!(application.reduce_linear_combination(input).is_ok());
}
