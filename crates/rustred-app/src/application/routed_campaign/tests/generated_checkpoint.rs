//! Actual native saved-owner preparation, completion, durable storage, cold
//! trusted reopen and further completion. No coefficient/master substitution.
//! Scope/cursor fixtures are synthetic generated metadata: these tests do NOT
//! establish native PrefixWindow successor binding or append-query coverage.
use super::*;
use crate::application::routed_campaign::finite_demand::{prepared::PreparedExecution, storage::*};
use rustred::{
    family::IntegralKey,
    solver::{
        CandidateEntryAdmission, CandidateRoutedCompletedTraceLimits,
        CandidateRoutedDeltaExportLimits, CandidateRoutedWorkBudget,
    },
};

fn limits() -> StorageLimits {
    StorageLimits {
        max_arity: 16,
        max_delta_records: 1000,
        max_delta_payload_bytes: 1_000_000,
        max_page_records: 2,
        max_page_bytes: 64 * 1024,
        max_cached_pages: 0,
        max_cached_page_bytes: 0,
        max_directory_bytes: 1_000_000,
        max_manifest_bytes: 1_000_000,
        max_state_bytes: 1_000_000,
        max_scope_rows: 10,
        max_census_records: 1000,
        max_runs: 100,
        max_pages: 1000,
        max_physical_files: 10000,
        max_disk_bytes: 10_000_000,
        max_lookup_pages: 100,
        max_lookup_bytes: 1_000_000,
        max_transient_bytes: 128 * 1024 * 1024,
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
fn initial() -> GenerationState {
    GenerationState {
        scope: ScopeBinding {
            original_source: [9; 32],
            rows: vec![ScopeRow {
                origin: "native-test".into(),
                role: ScopeRole::Required,
                exact_domain: [8; 32],
            }],
        },
        cursors: vec![CursorRecord {
            source_policy: [7; 32],
            completed_windows: 0,
            input_occurrences: 0,
            encoded_cursor: vec![1],
        }],
        completed_work: 0,
        input_occurrences: 0,
        census: RawCensus::default(),
    }
}
fn cursor(windows: u64, inputs: u64) -> Vec<CursorRecord> {
    vec![CursorRecord {
        source_policy: [7; 32],
        completed_windows: windows,
        input_occurrences: inputs,
        encoded_cursor: vec![1, windows as u8],
    }]
}
fn key(n: i64) -> IntegralKey {
    IntegralKey::try_new([n]).unwrap()
}

#[test]
fn generated_checkpoint_native_close_persist_drop_ram_reopen_extend_w1_w4() {
    let fixture = Fixture::new();
    let cancel = AtomicBool::new(false);
    for workers in [1, 4] {
        assert!(crate::test_gates::workers_or_skip(
            "native durable finite checkpoint",
            workers
        ));
        let prepared = PreparedExecution::<1>::prepare(&fixture.request, &cancel, &|_| {})
            .unwrap()
            .unwrap();
        let (whole, _) = prepared
            .reducer()
            .trace_targets_completed_with_entry_admission_and_observer(
                [key(4), key(5)],
                CandidateEntryAdmission::SavedGenerationScope,
                workers,
                budget(),
                Default::default(),
                CandidateRoutedCompletedTraceLimits {
                    max_completed_work: 1000,
                    max_merge_payload_bytes: 1_000_000,
                },
                &cancel,
                |_| {},
            )
            .unwrap();
        let expected_census = whole.census().clone();
        let expected_work = whole.completed_work();
        let expected_terminals: Vec<_> = whole
            .declared_terminals()
            .iter()
            .map(|k| k.powers().to_vec())
            .collect();
        drop(whole);
        let path = fixture.directory.join(format!("generated-{workers}"));
        let state = initial();
        let scope = state.scope.clone();
        let mut writer =
            StoreWriter::create(&path, prepared.execution().clone(), limits()).unwrap();
        writer.commit_generated(vec![], state, &cancel).unwrap();
        let reader = writer.current().unwrap().unwrap().reader(limits()).unwrap();
        let baseline = prepared
            .bind_generation(
                &reader,
                &scope,
                budget(),
                Default::default(),
                export(),
                &cancel,
            )
            .unwrap();
        let delta = baseline
            .trace_batch(
                [key(4)],
                CandidateEntryAdmission::SavedGenerationScope,
                workers,
                &cancel,
                |_| {},
            )
            .unwrap();
        assert!(delta.report().snapshot().rule_applications > 0);
        prepared
            .commit_delta_with_generated_metadata(
                &mut writer,
                delta,
                scope.clone(),
                cursor(1, 1),
                &cancel,
            )
            .unwrap();
        let first_digest = writer.current().unwrap().unwrap().digest();
        // Drop all reducers and decoded pages: the next engine is really
        // reconstructed from saved owner bytes and the committed disk state.
        drop(baseline);
        drop(reader);
        drop(writer);
        drop(prepared);
        let prepared = PreparedExecution::<1>::prepare(&fixture.request, &cancel, &|_| {})
            .unwrap()
            .unwrap();
        let mut writer =
            StoreWriter::resume_generated(&path, prepared.execution().clone(), limits()).unwrap();
        assert_eq!(writer.current().unwrap().unwrap().digest(), first_digest);
        let reader = writer.current().unwrap().unwrap().reader(limits()).unwrap();
        let baseline = prepared
            .bind_generation(
                &reader,
                &scope,
                budget(),
                Default::default(),
                export(),
                &cancel,
            )
            .unwrap();
        let delta = baseline
            .trace_batch(
                [key(5)],
                CandidateEntryAdmission::SavedGenerationScope,
                workers,
                &cancel,
                |_| {},
            )
            .unwrap();
        assert!(delta.report().snapshot().completed_baseline_hits > 0);
        assert_eq!(delta.report().snapshot().rule_applications, 1);
        assert!(delta.report().trace().declared_terminals().is_empty());
        prepared
            .commit_delta_with_generated_metadata(
                &mut writer,
                delta,
                scope.clone(),
                cursor(2, 2),
                &cancel,
            )
            .unwrap();
        let generation = writer.current().unwrap().unwrap();
        assert_eq!(generation.state().completed_work, expected_work as u64);
        assert_eq!(generation.state().input_occurrences, 2);
        assert_eq!(generation.state().census.terminals, expected_terminals);
        let expected: Vec<_> = expected_census
            .selections()
            .iter()
            .map(|(id, &multiplicity)| SelectionRecord {
                owner: id.owner.clone(),
                batch: id.batch_ordinal as u64,
                baseline: id.baseline_rule_ordinal as u64,
                selected: id.selected_rule_ordinal as u64,
                multiplicity,
            })
            .collect();
        assert_eq!(generation.state().census.selections, expected);
        assert!(reader.statistics().unwrap().page_reads > 0);
        assert_eq!(reader.statistics().unwrap().retained_pages, 0);
        // An old exact seal cannot be rebased onto a later CURRENT, even if it
        // only contains a dependency hit and no new Work.
        let stale = baseline
            .trace_batch(
                [key(4)],
                CandidateEntryAdmission::SavedGenerationScope,
                workers,
                &cancel,
                |_| {},
            )
            .unwrap();
        let current = generation.digest();
        assert!(
            prepared
                .commit_delta_with_generated_metadata(
                    &mut writer,
                    stale,
                    scope,
                    cursor(3, 3),
                    &cancel
                )
                .is_err()
        );
        assert_eq!(writer.current().unwrap().unwrap().digest(), current);
    }
}

#[test]
fn generated_checkpoint_changed_execution_and_evicted_corrupt_page_refuse() {
    let fixture = Fixture::new();
    let cancel = AtomicBool::new(false);
    let prepared = PreparedExecution::<1>::prepare(&fixture.request, &cancel, &|_| {})
        .unwrap()
        .unwrap();
    let path = fixture.directory.join("corrupt-generated");
    let state = initial();
    let scope = state.scope.clone();
    let mut writer = StoreWriter::create(&path, prepared.execution().clone(), limits()).unwrap();
    writer.commit_generated(vec![], state, &cancel).unwrap();
    let reader = writer.current().unwrap().unwrap().reader(limits()).unwrap();
    let baseline = prepared
        .bind_generation(
            &reader,
            &scope,
            budget(),
            Default::default(),
            export(),
            &cancel,
        )
        .unwrap();
    let delta = baseline
        .trace_batch(
            [key(4)],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            &cancel,
            |_| {},
        )
        .unwrap();
    prepared
        .commit_delta_with_generated_metadata(
            &mut writer,
            delta,
            scope.clone(),
            cursor(1, 1),
            &cancel,
        )
        .unwrap();
    let generation = writer.current().unwrap().unwrap().clone();
    let digest = generation.digest();
    let mut changed = fixture.request.clone();
    changed.reduction_limits.exact_algebra.max_term_operations += 1;
    changed.reduction_limits.indexed_algebra.exact_algebra = changed.reduction_limits.exact_algebra;
    changed.trace_limits.expansion.exact_algebra = changed.reduction_limits.exact_algebra;
    let other = PreparedExecution::<1>::prepare(&changed, &cancel, &|_| {})
        .unwrap()
        .unwrap();
    let reader = generation.reader(limits()).unwrap();
    assert!(
        other
            .bind_generation(
                &reader,
                &scope,
                budget(),
                Default::default(),
                export(),
                &cancel
            )
            .is_err()
    );
    let baseline = prepared
        .bind_generation(
            &reader,
            &scope,
            budget(),
            Default::default(),
            export(),
            &cancel,
        )
        .unwrap();
    for entry in std::fs::read_dir(path.join("pages")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|s| s == "rrwork") {
            let mut bytes = std::fs::read(&path).unwrap();
            bytes[0] ^= 1;
            std::fs::write(path, bytes).unwrap();
        }
    }
    let error = baseline
        .trace_batch(
            [key(4)],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            &cancel,
            |_| {},
        )
        .unwrap_err();
    assert!(error.report().complete_census().is_none());
    assert_eq!(writer.current().unwrap().unwrap().digest(), digest);
    assert_eq!(reader.statistics().unwrap().retained_pages, 0);
}
