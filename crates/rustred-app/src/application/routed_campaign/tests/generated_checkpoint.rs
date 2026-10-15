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
        raw: RawCensusLimits {
            max_page_records: 2,
            ..Default::default()
        },
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
        let authenticated = prepared
            .authenticate_generation(reader.snapshot().clone(), &scope, &cancel)
            .unwrap();
        let baseline = authenticated
            .bind(&reader, budget(), Default::default(), export(), &cancel)
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
                &authenticated,
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
        drop(authenticated);
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
        let authenticated = prepared
            .authenticate_generation(reader.snapshot().clone(), &scope, &cancel)
            .unwrap();
        let baseline = authenticated
            .bind(&reader, budget(), Default::default(), export(), &cancel)
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
        let outcome = prepared
            .commit_delta_with_generated_metadata(
                &authenticated,
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
        let mut actual_terminals = Vec::new();
        outcome
            .generation
            .visit_raw_census(limits().raw, &cancel, |kind, key| {
                if kind == RawKeyKind::Terminal {
                    actual_terminals.push(key.to_vec());
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(actual_terminals, expected_terminals);
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
                    &authenticated,
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
    let authenticated = prepared
        .authenticate_generation(reader.snapshot().clone(), &scope, &cancel)
        .unwrap();
    let baseline = authenticated
        .bind(&reader, budget(), Default::default(), export(), &cancel)
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
            &authenticated,
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
            .authenticate_generation(generation.clone(), &scope, &cancel)
            .is_err()
    );
    let authenticated = prepared
        .authenticate_generation(generation.clone(), &scope, &cancel)
        .unwrap();
    let baseline = authenticated
        .bind(&reader, budget(), Default::default(), export(), &cancel)
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

#[test]
fn prepared_generation_arc_identity_clone_and_uncertain_commit_are_explicit() {
    let fixture = Fixture::new();
    let cancel = AtomicBool::new(false);
    let prepared = PreparedExecution::<1>::prepare(&fixture.request, &cancel, &|_| {})
        .unwrap()
        .unwrap();
    let other = PreparedExecution::<1>::prepare(&fixture.request, &cancel, &|_| {})
        .unwrap()
        .unwrap();
    assert_eq!(prepared.execution(), other.execution());
    let path = fixture.directory.join("token-arcs");
    let state = initial();
    let scope = state.scope.clone();
    let mut writer = StoreWriter::create(&path, prepared.execution().clone(), limits()).unwrap();
    writer.commit_generated(vec![], state, &cancel).unwrap();
    let initial = writer.current().unwrap().unwrap().clone();
    let reader = initial.reader(limits()).unwrap();
    let token = prepared
        .authenticate_generation(initial.clone(), &scope, &cancel)
        .unwrap();
    let foreign = other
        .authenticate_generation(initial.clone(), &scope, &cancel)
        .unwrap();
    let baseline = token
        .bind(&reader, budget(), Default::default(), export(), &cancel)
        .unwrap();
    let delta = baseline
        .trace_batch(
            [key(2)],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            &cancel,
            |_| {},
        )
        .unwrap();
    assert!(
        other
            .commit_delta_with_generated_metadata(
                &foreign,
                &mut writer,
                delta,
                scope.clone(),
                cursor(1, 1),
                &cancel
            )
            .is_err()
    );
    assert_eq!(
        writer.current().unwrap().unwrap().digest(),
        initial.digest()
    );
    // Correct seal and committing engine; only the token's Arc is foreign.
    let delta = baseline
        .trace_batch(
            [key(2)],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            &cancel,
            |_| {},
        )
        .unwrap();
    assert!(
        prepared
            .commit_delta_with_generated_metadata(
                &foreign,
                &mut writer,
                delta,
                scope.clone(),
                cursor(1, 1),
                &cancel
            )
            .is_err()
    );
    assert_eq!(
        writer.current().unwrap().unwrap().digest(),
        initial.digest()
    );
    let delta = baseline
        .trace_batch(
            [key(2)],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            &cancel,
            |_| {},
        )
        .unwrap();
    let shared = prepared.clone();
    let outcome = shared
        .commit_delta_with_generated_metadata(
            &token,
            &mut writer,
            delta,
            scope.clone(),
            cursor(1, 1),
            &cancel,
        )
        .unwrap();
    let current = outcome.generation.generation().clone();
    assert_ne!(current.digest(), initial.digest());
    let reader = current.reader(limits()).unwrap();
    let baseline = outcome
        .generation
        .bind(&reader, budget(), Default::default(), export(), &cancel)
        .unwrap();
    // Current seal and correct Arc; only the captured predecessor is stale.
    let delta = baseline
        .trace_batch(
            [key(3)],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            &cancel,
            |_| {},
        )
        .unwrap();
    assert!(
        prepared
            .commit_delta_with_generated_metadata(
                &token,
                &mut writer,
                delta,
                scope.clone(),
                cursor(2, 2),
                &cancel
            )
            .is_err()
    );
    assert_eq!(
        writer.current().unwrap().unwrap().digest(),
        current.digest()
    );
    let delta = baseline
        .trace_batch(
            [key(3)],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            &cancel,
            |_| {},
        )
        .unwrap();
    writer.inject_current_uncertainty();
    let attempted = prepared.commit_delta_with_generated_metadata(
        &outcome.generation,
        &mut writer,
        delta,
        scope.clone(),
        cursor(2, 2),
        &cancel,
    );
    assert!(attempted.is_err()); // No successor token can escape an uncertain result.
    assert!(writer.is_poisoned());
    assert!(writer.current().is_err());
    drop(writer);
    let reopened =
        ValidatedGeneration::open_generated(&path, prepared.execution(), limits()).unwrap();
    assert_eq!(reopened.state().input_occurrences, 2);
    let restored = prepared
        .authenticate_generation(reopened, &scope, &cancel)
        .unwrap();
    assert_eq!(
        restored
            .visit_raw_census(limits().raw, &cancel, |_, _| Ok(()))
            .unwrap()
            .terminals,
        1
    );
}

#[test]
fn hot_prepared_advancement_never_reads_or_rewrites_prior_raw_pages() {
    let fixture = Fixture::new();
    let cancel = AtomicBool::new(false);
    let prepared = PreparedExecution::<1>::prepare(&fixture.request, &cancel, &|_| {})
        .unwrap()
        .unwrap();
    let path = fixture.directory.join("lazy-hot");
    let state = initial();
    let scope = state.scope.clone();
    let mut writer = StoreWriter::create(&path, prepared.execution().clone(), limits()).unwrap();
    writer.commit_generated(vec![], state, &cancel).unwrap();
    let reader = writer.current().unwrap().unwrap().reader(limits()).unwrap();
    let token = prepared
        .authenticate_generation(reader.snapshot().clone(), &scope, &cancel)
        .unwrap();
    let baseline = token
        .bind(&reader, budget(), Default::default(), export(), &cancel)
        .unwrap();
    let delta = baseline
        .trace_batch(
            [key(2)],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            &cancel,
            |_| {},
        )
        .unwrap();
    let outcome = prepared
        .commit_delta_with_generated_metadata(
            &token,
            &mut writer,
            delta,
            scope.clone(),
            cursor(1, 1),
            &cancel,
        )
        .unwrap();
    let raw_path = std::fs::read_dir(path.join("raw"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let original = std::fs::read(&raw_path).unwrap();
    let mut corrupt = original.clone();
    corrupt[0] ^= 1;
    std::fs::write(&raw_path, &corrupt).unwrap();
    let reader = outcome.generation.generation().reader(limits()).unwrap();
    let baseline = outcome
        .generation
        .bind(&reader, budget(), Default::default(), export(), &cancel)
        .unwrap();
    let delta = baseline
        .trace_batch(
            [key(2)],
            CandidateEntryAdmission::SavedGenerationScope,
            1,
            &cancel,
            |_| {},
        )
        .unwrap();
    assert_eq!(delta.report().snapshot().scheduled_nodes, 0);
    let next = prepared
        .commit_delta_with_generated_metadata(
            &outcome.generation,
            &mut writer,
            delta,
            scope,
            cursor(2, 2),
            &cancel,
        )
        .unwrap();
    assert_eq!(std::fs::read(&raw_path).unwrap(), corrupt);
    assert!(
        next.generation
            .visit_raw_census(limits().raw, &cancel, |_, _| Ok(()))
            .is_err()
    );
    std::fs::write(raw_path, original).unwrap();
    assert_eq!(
        next.generation
            .visit_raw_census(limits().raw, &cancel, |_, _| Ok(()))
            .unwrap()
            .terminals,
        1
    );
}

#[test]
fn prepared_raw_stream_refuses_wrong_native_terminal_and_zero_classifications() {
    let fixture = Fixture::new();
    let cancel = AtomicBool::new(false);
    let prepared = PreparedExecution::<1>::prepare(&fixture.request, &cancel, &|_| {})
        .unwrap()
        .unwrap();
    for (name, terminals, zeros, power) in [
        ("bad-terminal", vec![vec![2]], vec![], 2),
        ("bad-zero", vec![], vec![vec![1]], 1),
    ] {
        let path = fixture.directory.join(name);
        let mut writer =
            StoreWriter::create(&path, prepared.execution().clone(), limits()).unwrap();
        let mut state = initial();
        state.completed_work = 1;
        state.input_occurrences = 1;
        state.cursors = cursor(1, 1);
        let scope = state.scope.clone();
        let generation = writer
            .commit_synthetic_raw_keys_for_test(
                vec![StoredWork::Route(vec![power])],
                &terminals,
                &zeros,
                state,
                &cancel,
            )
            .unwrap()
            .generation;
        let token = prepared
            .authenticate_generation(generation, &scope, &cancel)
            .unwrap();
        assert_eq!(
            token
                .visit_raw_census(limits().raw, &cancel, |_, _| Ok(()))
                .unwrap_err()
                .kind(),
            crate::AppErrorKind::Serialization
        );
    }
}

#[test]
fn native_thousand_zero_pages_exceed_state_cap_cold_resume_and_all_hit_succeed() {
    use rustred::solver::{DomainPowerBounds, FiniteRootAdmission, RootRegionInput};
    let fixture = Fixture::new();
    let cancel = AtomicBool::new(false);
    let settings = StorageLimits {
        max_state_bytes: 2048,
        ..limits()
    };
    let prepared = PreparedExecution::<1>::prepare(&fixture.request, &cancel, &|_| {})
        .unwrap()
        .unwrap();
    let admission = FiniteRootAdmission::try_new(
        [RootRegionInput {
            support: [false],
            lower: vec![0],
            upper: vec![Some(1000)],
            rank: Some(1000),
            powers: DomainPowerBounds {
                max_positive_power: Some(0),
                min_power_difference: Some(-1000),
                max_power_difference: Some(0),
            },
        }],
        1,
    )
    .unwrap();
    let path = fixture.directory.join("native-many-zero");
    let state = initial();
    let scope = state.scope.clone();
    let mut writer = StoreWriter::create(&path, prepared.execution().clone(), settings).unwrap();
    writer.commit_generated(vec![], state, &cancel).unwrap();
    let reader = writer.current().unwrap().unwrap().reader(settings).unwrap();
    let token = prepared
        .authenticate_generation(reader.snapshot().clone(), &scope, &cancel)
        .unwrap();
    let baseline = token
        .bind(&reader, budget(), Default::default(), export(), &cancel)
        .unwrap();
    let delta = baseline
        .trace_batch(
            (-1000..0).map(key),
            CandidateEntryAdmission::ExplicitFinite(&admission),
            1,
            &cancel,
            |_| {},
        )
        .unwrap();
    assert_eq!(delta.report().trace().visited_zeros().len(), 1000);
    assert_eq!(delta.work().len(), 1000);
    prepared
        .commit_delta_with_generated_metadata(
            &token,
            &mut writer,
            delta,
            scope.clone(),
            cursor(1, 1000),
            &cancel,
        )
        .unwrap();
    assert!(1000 * std::mem::size_of::<i64>() > settings.max_state_bytes);
    assert!(
        writer
            .current()
            .unwrap()
            .unwrap()
            .state()
            .payload_bytes()
            .unwrap()
            < settings.max_state_bytes
    );
    drop(baseline);
    drop(token);
    drop(reader);
    drop(writer);
    drop(prepared);
    let prepared = PreparedExecution::<1>::prepare(&fixture.request, &cancel, &|_| {})
        .unwrap()
        .unwrap();
    let mut writer =
        StoreWriter::resume_generated(&path, prepared.execution().clone(), settings).unwrap();
    let token = prepared
        .authenticate_generation(writer.current().unwrap().unwrap().clone(), &scope, &cancel)
        .unwrap();
    let reader = token.generation().reader(settings).unwrap();
    let baseline = token
        .bind(&reader, budget(), Default::default(), export(), &cancel)
        .unwrap();
    let delta = baseline
        .trace_batch(
            [key(-501)],
            CandidateEntryAdmission::ExplicitFinite(&admission),
            1,
            &cancel,
            |_| {},
        )
        .unwrap();
    assert_eq!(delta.report().snapshot().scheduled_nodes, 0);
    assert_eq!(delta.report().snapshot().completed_baseline_hits, 1);
    let next = prepared
        .commit_delta_with_generated_metadata(
            &token,
            &mut writer,
            delta,
            scope,
            cursor(2, 1001),
            &cancel,
        )
        .unwrap();
    let summary = next
        .generation
        .visit_raw_census(settings.raw, &cancel, |kind, _| {
            assert_eq!(kind, RawKeyKind::Zero);
            Ok(())
        })
        .unwrap();
    assert_eq!((summary.zeros, summary.terminals), (1000, 0));
    assert!(summary.pages_read > 1);
}
