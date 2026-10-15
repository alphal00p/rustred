use super::*;
use crate::persistence::finite_work::*;
use crate::solver::candidate_reduction::owner_test_support::key;
use std::{
    collections::BTreeMap,
    sync::{Arc, Condvar, Mutex, atomic::AtomicUsize},
    time::Duration,
};

fn binding() -> GeneratedExecutionBinding {
    GeneratedExecutionBinding {
        physical_arity: 1,
        program_identity: [3; 32],
    }
}
fn limits() -> GeneratedDirectoryLimits {
    GeneratedDirectoryLimits {
        max_arity: 16,
        max_runs: 16,
        max_pages: 100,
        max_directory_payload_bytes: 1_000_000,
        max_page_records: 100,
        max_page_bytes: 100_000,
        max_referenced_bytes: 1_000_000,
        max_lookup_pages: 100,
        max_lookup_bytes: 1_000_000,
    }
}
fn budget() -> CandidateRoutedWorkBudget {
    CandidateRoutedWorkBudget {
        max_nodes: 1000,
        max_rule_applications: 1000,
        max_transport_calls: 1000,
        max_transport_operations: 100_000,
        max_transport_endpoints: 100_000,
        max_coalescing_additions: 100_000,
    }
}
fn export() -> CandidateRoutedDeltaExportLimits {
    CandidateRoutedDeltaExportLimits {
        max_records: 1000,
        max_live_payload_bytes: 1_000_000,
    }
}
#[derive(Default)]
struct Pages {
    bytes: BTreeMap<GeneratedWorkDigest, Vec<u8>>,
    calls: AtomicUsize,
}
impl Pages {
    fn directory(
        &mut self,
        work: Vec<GeneratedWorkKey>,
        generation: u8,
    ) -> TrustedGeneratedWorkDirectory {
        let runs = if work.is_empty() {
            vec![]
        } else {
            let record = GeneratedWorkPageRecord {
                execution: binding(),
                records: work,
            };
            let bytes =
                encode_metadata_frame(PAGE_MAGIC, &record, limits().max_page_bytes).unwrap();
            let reference = GeneratedWorkPageRef {
                object: GeneratedObjectRef {
                    digest: *blake3::hash(&bytes).as_bytes(),
                    bytes: bytes.len() as u64,
                },
                records: record.records.len() as u64,
                first: record.records.first().unwrap().clone(),
                last: record.records.last().unwrap().clone(),
            };
            self.bytes.insert(reference.object.digest, bytes);
            vec![vec![reference]]
        };
        TrustedGeneratedWorkDirectory::try_from_trusted_generated(
            binding(),
            [generation; 32],
            Arc::new(runs),
            limits(),
        )
        .unwrap()
    }
}
impl GeneratedPageProvider for Pages {
    fn visit_validated_page(
        &self,
        expected: &GeneratedWorkPageRef,
        allowance: u64,
        visitor: &mut dyn FnMut(&ValidatedWorkPage),
    ) -> GeneratedWorkResult<GeneratedPageReadUsage> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if expected.object.bytes > allowance {
            return Err(GeneratedWorkError::new(
                GeneratedWorkErrorKind::Resource,
                "read allowance",
            ));
        }
        let bytes = self
            .bytes
            .get(&expected.object.digest)
            .ok_or_else(|| GeneratedWorkError::new(GeneratedWorkErrorKind::Io, "page missing"))?;
        let page = ValidatedWorkPage::decode_generated(
            bytes,
            expected,
            &binding(),
            limits().max_page_bytes,
            limits().max_page_records,
        )?;
        visitor(&page);
        Ok(GeneratedPageReadUsage {
            disk_bytes: bytes.len() as u64,
            cache_hit: false,
        })
    }
}

#[test]
fn generated_native_delta_then_cold_page_reuse_matches_one_union_w1_w4() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    for workers in [1, 4] {
        let mut pages = Pages::default();
        let empty = pages.directory(vec![], 1);
        let baseline = reducer
            .bind_trusted_generated_baseline(
                &binding(),
                &empty,
                &pages,
                budget(),
                Default::default(),
                export(),
            )
            .unwrap();
        let a = baseline
            .trace_batch(
                [key([6])],
                CandidateEntryAdmission::SavedGenerationScope,
                workers,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
        assert_eq!(a.work().len(), 5);
        assert_eq!(a.predecessor(), [1; 32]);
        let (a_work, a_report) = a.into_parts();
        let old_work = a_work.clone();
        let census_a = a_report.complete_census().unwrap().clone();
        let directory = pages.directory(a_work, 2);
        let baseline = reducer
            .bind_trusted_generated_baseline(
                &binding(),
                &directory,
                &pages,
                budget(),
                Default::default(),
                export(),
            )
            .unwrap();
        let b = baseline
            .trace_batch(
                [key([5])],
                CandidateEntryAdmission::SavedGenerationScope,
                workers,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
        assert_eq!(b.work().len(), 3);
        assert_eq!(b.report().snapshot().completed_baseline_hits, 2);
        assert!(b.report().trace().declared_terminals().is_empty());
        assert!(pages.calls.load(Ordering::Relaxed) > 0);
        let whole = reducer
            .bind_trusted_generated_baseline(
                &binding(),
                &empty,
                &pages,
                budget(),
                Default::default(),
                export(),
            )
            .unwrap()
            .trace_batch(
                [key([6]), key([5])],
                CandidateEntryAdmission::SavedGenerationScope,
                workers,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
        let mut keys = old_work;
        keys.extend_from_slice(b.work());
        keys.sort();
        assert_eq!(keys, whole.work());
        assert_eq!(
            census_a
                .merged(
                    b.report().complete_census().unwrap(),
                    Default::default(),
                    &mut || Ok(())
                )
                .unwrap(),
            *whole.report().complete_census().unwrap()
        );
        assert_eq!(
            a_report.trace().declared_terminals(),
            whole.report().trace().declared_terminals()
        );
        let hit = baseline
            .trace_batch(
                [key([6])],
                CandidateEntryAdmission::SavedGenerationScope,
                workers,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
        assert!(hit.work().is_empty());
        assert_eq!(hit.report().snapshot().completed_baseline_hits, 1);
        assert!(
            hit.report()
                .complete_census()
                .unwrap()
                .selections()
                .is_empty()
        );
    }
}

#[test]
fn generated_invalid_start_and_wrong_binding_refuse_before_backing_hits() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    let mut pages = Pages::default();
    let directory = pages.directory(vec![GeneratedWorkKey::Route(vec![6])], 1);
    let mut wrong = binding();
    wrong.program_identity[0] ^= 1;
    assert!(
        reducer
            .bind_trusted_generated_baseline(
                &wrong,
                &directory,
                &pages,
                budget(),
                Default::default(),
                export()
            )
            .is_err()
    );
    let baseline = reducer
        .bind_trusted_generated_baseline(
            &binding(),
            &directory,
            &pages,
            budget(),
            Default::default(),
            export(),
        )
        .unwrap();
    let e = baseline
        .trace_batch(
            [key([6, 1])],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap_err();
    assert_eq!(e.report().snapshot().scheduled_nodes, 0);
    assert_eq!(pages.calls.load(Ordering::Relaxed), 0);
}

#[test]
fn generated_frontier_export_quota_and_final_cancel_cannot_mint_delta() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    let mut pages = Pages::default();
    let empty = pages.directory(vec![], 1);
    let baseline = reducer
        .bind_trusted_generated_baseline(
            &binding(),
            &empty,
            &pages,
            budget(),
            Default::default(),
            export(),
        )
        .unwrap();
    assert!(matches!(
        baseline.trace_batch(
            [key([6]), key([7])],
            CandidateEntryAdmission::SavedGenerationScope,
            4,
            &AtomicBool::new(false),
            |_| {}
        ),
        Err(CandidateRoutedCompletedTraceError::Frontier(_))
    ));
    let capped = reducer
        .bind_trusted_generated_baseline(
            &binding(),
            &empty,
            &pages,
            budget(),
            Default::default(),
            CandidateRoutedDeltaExportLimits {
                max_records: 4,
                ..export()
            },
        )
        .unwrap();
    assert!(matches!(
        capped.trace_batch(
            [key([6])],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            &AtomicBool::new(false),
            |_| {}
        ),
        Err(CandidateRoutedCompletedTraceError::Retention { .. })
    ));
    let cancel = AtomicBool::new(false);
    let mut finals = 0;
    let e = baseline
        .trace_batch(
            [key([6])],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            &cancel,
            |s| {
                if s.finished {
                    finals += 1;
                    if finals == 2 {
                        cancel.store(true, Ordering::Release);
                    }
                }
            },
        )
        .unwrap_err();
    assert_eq!(finals, 2);
    assert!(!e.report().snapshot().finished);
}

#[test]
fn generated_provider_error_is_typed_and_never_seals_a_miss() {
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    let mut pages = Pages::default();
    let directory = pages.directory(vec![GeneratedWorkKey::Route(vec![6])], 1);
    pages.bytes.clear();
    let baseline = reducer
        .bind_trusted_generated_baseline(
            &binding(),
            &directory,
            &pages,
            budget(),
            Default::default(),
            export(),
        )
        .unwrap();
    let e = baseline
        .trace_batch(
            [key([6])],
            CandidateEntryAdmission::SavedGenerationScope,
            4,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap_err();
    assert!(
        matches!(e,CandidateRoutedCompletedTraceError::Trace(ref e) if matches!(e.reason(),CandidateRoutedCampaignFailure::Backing(error) if error.kind==GeneratedWorkErrorKind::Io))
    );
    assert_eq!(e.report().snapshot().scheduled_nodes, 0);
    assert!(e.report().trace().declared_terminals().is_empty());
}

#[test]
fn generated_slow_page_provider_does_not_hold_shared_scheduler_lock() {
    struct Slow<'a> {
        pages: &'a Pages,
        state: Mutex<(bool, bool)>,
        wake: Condvar,
    }
    impl GeneratedPageProvider for Slow<'_> {
        fn visit_validated_page(
            &self,
            p: &GeneratedWorkPageRef,
            b: u64,
            v: &mut dyn FnMut(&ValidatedWorkPage),
        ) -> GeneratedWorkResult<GeneratedPageReadUsage> {
            let mut state = self.state.lock().unwrap();
            state.0 = true;
            self.wake.notify_all();
            while !state.1 {
                state = self.wake.wait(state).unwrap();
            }
            drop(state);
            self.pages.visit_validated_page(p, b, v)
        }
    }
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    let mut pages = Pages::default();
    let directory = pages.directory(vec![GeneratedWorkKey::Route(vec![6])], 1);
    let slow = Slow {
        pages: &pages,
        state: Mutex::new((false, false)),
        wake: Condvar::new(),
    };
    let cancel = AtomicBool::new(false);
    let mut shared = scheduler::Shared::new(&reducer, 1, &cancel);
    shared.set_completed_baseline(Some(scheduler::CompletedBaseline::Generated {
        directory: &directory,
        provider: &slow,
    }));
    std::thread::scope(|scope| {
        let handle = scope.spawn(|| shared.schedule(CandidateRoutedWork::Route(key([6]))));
        let state = slow.state.lock().unwrap();
        let (state, timeout) = slow
            .wake
            .wait_timeout_while(state, Duration::from_secs(5), |s| !s.0)
            .unwrap();
        assert!(!timeout.timed_out());
        drop(state);
        // Release the page from a watchdog even if a regression blocks this
        // snapshot, so the failed test cannot deadlock the test process.
        let (tx, rx) = std::sync::mpsc::channel();
        let slow_ref = &slow;
        let watchdog = scope.spawn(move || {
            let quick = rx.recv_timeout(Duration::from_secs(2)).is_ok();
            let mut state = slow_ref.state.lock().unwrap();
            state.1 = true;
            slow_ref.wake.notify_all();
            quick
        });
        assert_eq!(shared.snapshot().scheduled_nodes, 0);
        tx.send(()).unwrap();
        assert!(watchdog.join().unwrap(), "snapshot blocked behind page IO");
        handle.join().unwrap().unwrap();
    });
    assert_eq!(shared.snapshot().completed_baseline_hits, 1);
}

#[test]
fn generated_provider_panic_during_entry_preflight_is_typed_incomplete() {
    struct Panic;
    impl GeneratedPageProvider for Panic {
        fn visit_validated_page(
            &self,
            _: &GeneratedWorkPageRef,
            _: u64,
            _: &mut dyn FnMut(&ValidatedWorkPage),
        ) -> GeneratedWorkResult<GeneratedPageReadUsage> {
            panic!("injected provider panic")
        }
    }
    let reducer = super::super::tests::diamond(Default::default(), Default::default());
    let mut pages = Pages::default();
    let directory = pages.directory(vec![GeneratedWorkKey::Route(vec![6])], 1);
    let baseline = reducer
        .bind_trusted_generated_baseline(
            &binding(),
            &directory,
            &Panic,
            budget(),
            Default::default(),
            export(),
        )
        .unwrap();
    let e = baseline
        .trace_batch(
            [key([6])],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap_err();
    assert!(
        matches!(e,CandidateRoutedCompletedTraceError::Trace(ref e) if *e.reason()==CandidateRoutedCampaignFailure::WorkerPanicked)
    );
    assert!(e.report().complete_census().is_none());
}
