use super::super::writer::Stage;
use super::*;

#[test]
fn finite_store_precommit_interrupt_keeps_old_cursor_and_counts_orphans_on_resume() {
    for stage in [Stage::AfterPages, Stage::AfterState, Stage::BeforeCurrent] {
        let scratch = Scratch::new();
        let path = scratch.0.join("store");
        let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
        let first = commit(&mut writer, vec![key(1)], 1, 1);
        let old_tip = fs::read(path.join(wire::CURRENT)).unwrap();
        let before = writer.charged_disk_bytes();
        writer.inject_failure(stage);
        assert!(
            writer
                .commit_generated(vec![key(2)], state(2, 2), &AtomicBool::new(false))
                .is_err()
        );
        assert!(writer.is_poisoned());
        assert!(writer.current().is_err());
        assert_eq!(fs::read(path.join(wire::CURRENT)).unwrap(), old_tip);
        drop(writer);
        let mut writer = StoreWriter::resume_generated(&path, binding(), limits()).unwrap();
        assert_eq!(writer.current().unwrap().unwrap().digest(), first.digest());
        assert_eq!(
            writer
                .current()
                .unwrap()
                .unwrap()
                .cursor(0)
                .unwrap()
                .completed_windows(),
            1
        );
        assert!(writer.charged_disk_bytes() > before);
        assert_eq!(writer.charged_disk_bytes(), physical_bytes(&path));
        // Idempotently reuse complete orphan objects; do not adopt an orphan's
        // cursor or charge its bytes twice.
        let next = commit(&mut writer, vec![key(2)], 2, 2);
        assert_eq!(next.generation(), 2);
        assert_eq!(next.state().completed_work, 2);
        assert_eq!(writer.charged_disk_bytes(), physical_bytes(&path));
    }
}

#[test]
fn finite_store_postcommit_cleanup_failure_is_still_a_durable_commit() {
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    let first = commit(&mut writer, vec![key(1)], 1, 1);
    writer.inject_failure(Stage::AfterCurrent);
    let outcome = writer
        .commit_generated(vec![key(2)], state(2, 2), &AtomicBool::new(false))
        .unwrap();
    assert!(outcome.cleanup_warning.is_some());
    assert_eq!(outcome.generation.generation(), 2);
    assert_eq!(outcome.generation.manifest.previous, Some(first.digest()));
    assert!(!writer.is_poisoned());
    assert_eq!(writer.charged_disk_bytes(), physical_bytes(&path));
    drop(writer);
    let loaded = ValidatedGeneration::open_generated(&path, &binding(), limits()).unwrap();
    assert_eq!(loaded.digest(), outcome.generation.digest());
    assert_eq!(loaded.cursor(0).unwrap().input_occurrences(), 2);
}

#[test]
fn finite_store_uncertain_current_installation_poison_requires_reopen() {
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    commit(&mut writer, vec![key(1)], 1, 1);
    writer.inject_failure(Stage::CurrentUncertain);
    let error = writer
        .commit_generated(vec![key(2)], state(2, 2), &AtomicBool::new(false))
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Indeterminate);
    assert!(writer.is_poisoned());
    assert!(writer.current().is_err());
    assert_eq!(
        writer
            .commit_generated(vec![key(3)], state(3, 3), &AtomicBool::new(false))
            .unwrap_err()
            .kind,
        ErrorKind::Indeterminate
    );
    drop(writer);
    let writer = StoreWriter::resume_generated(&path, binding(), limits()).unwrap();
    assert_eq!(writer.current().unwrap().unwrap().generation(), 2);
    assert_eq!(
        writer
            .current()
            .unwrap()
            .unwrap()
            .reader(limits())
            .unwrap()
            .lookup(&key(2))
            .unwrap(),
        Lookup::Present
    );
}

#[test]
fn finite_store_cancel_after_pages_does_not_advance_cursor_or_adopt_orphans() {
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    let first = commit(&mut writer, vec![key(1)], 1, 1);
    writer.inject_failure(Stage::CancelAfterPages);
    let cancel = AtomicBool::new(false);
    let error = writer
        .commit_generated(vec![key(2)], state(2, 2), &cancel)
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Cancelled);
    assert!(cancel.load(Ordering::Acquire));
    assert_eq!(writer.current().unwrap().unwrap().digest(), first.digest());
    assert_eq!(writer.charged_disk_bytes(), physical_bytes(&path));
    drop(writer);
    let loaded = ValidatedGeneration::open_generated(&path, &binding(), limits()).unwrap();
    assert_eq!(loaded.digest(), first.digest());
    assert_eq!(
        loaded.reader(limits()).unwrap().lookup(&key(2)).unwrap(),
        Lookup::Absent
    );
}

#[test]
fn finite_store_writer_lease_and_stale_current_fail_closed() {
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    let first = commit(&mut writer, vec![key(1)], 1, 1);
    assert_eq!(
        StoreWriter::resume_generated(&path, binding(), limits())
            .err()
            .unwrap()
            .kind,
        ErrorKind::Locked
    );
    fs::remove_file(path.join(wire::CURRENT)).unwrap();
    assert_eq!(
        writer
            .commit_generated(vec![key(2)], state(2, 2), &AtomicBool::new(false))
            .unwrap_err()
            .kind,
        ErrorKind::Stale
    );
    assert!(writer.is_poisoned());
    // The earlier captured immutable snapshot remains inspectable, but it is
    // not relabeled as the latest authoritative CURRENT.
    assert_eq!(
        first.reader(limits()).unwrap().lookup(&key(1)).unwrap(),
        Lookup::Present
    );
}

#[test]
fn finite_store_missing_current_never_implies_an_unpublished_fresh_store() {
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    writer.inject_failure(Stage::BeforeCurrent);
    assert!(
        writer
            .commit_generated(vec![key(1)], state(1, 1), &AtomicBool::new(false))
            .is_err()
    );
    drop(writer);
    assert!(!path.join(wire::CURRENT).exists());
    assert_eq!(
        StoreWriter::resume_generated(&path, binding(), limits())
            .err()
            .unwrap()
            .kind,
        ErrorKind::Corrupt
    );
    // An explicitly new directory is a new run, not an inferred continuation.
    let retry_path = scratch.0.join("new-store");
    let mut writer = StoreWriter::create(&retry_path, binding(), limits()).unwrap();
    let loaded = commit(&mut writer, vec![key(1)], 1, 1);
    assert_eq!(loaded.generation(), 1);
    assert_eq!(writer.charged_disk_bytes(), physical_bytes(&retry_path));
    drop(writer);
    fs::remove_file(retry_path.join(wire::CURRENT)).unwrap();
    assert_eq!(
        StoreWriter::resume_generated(&retry_path, binding(), limits())
            .err()
            .unwrap()
            .kind,
        ErrorKind::Corrupt
    );
}

#[test]
fn finite_store_scope_role_domain_and_cursor_changes_are_not_appends() {
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    commit(&mut writer, vec![key(1)], 1, 1);
    let mut appended = state(1, 1);
    appended.scope.rows.push(ScopeRow {
        origin: "append:0".into(),
        role: ScopeRole::Auxiliary,
        exact_domain: [9; 32],
    });
    appended.cursors.push(CursorRecord {
        source_policy: [10; 32],
        completed_windows: 0,
        input_occurrences: 0,
        encoded_cursor: b"new-initial".to_vec(),
    });
    let generation = writer
        .commit_generated(vec![], appended.clone(), &AtomicBool::new(false))
        .unwrap()
        .generation;
    assert_eq!(generation.generation(), 2);
    for choice in 0..5 {
        let mut bad = appended.clone();
        match choice {
            0 => bad.scope.rows[0].role = ScopeRole::Auxiliary,
            1 => bad.scope.rows[0].exact_domain = [11; 32],
            2 => bad.scope.original_source = [12; 32],
            3 => {
                bad.scope.rows.pop();
                bad.cursors.pop();
            }
            _ => bad.cursors[0].encoded_cursor.push(0),
        }
        assert_eq!(
            writer
                .commit_generated(vec![], bad, &AtomicBool::new(false))
                .unwrap_err()
                .kind,
            ErrorKind::Stale
        );
        assert_eq!(
            writer.current().unwrap().unwrap().digest(),
            generation.digest()
        );
    }
}

#[test]
fn finite_store_raw_binding_is_independent_of_downstream_publication_files() {
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    let generation = commit(&mut writer, vec![key(1)], 1, 1);
    drop(writer);
    let mut changed = binding();
    changed.program_identity[0] ^= 1;
    assert!(ValidatedGeneration::open_generated(&path, &changed, limits()).is_err());
    changed = binding();
    changed.physical_arity = 3;
    assert!(ValidatedGeneration::open_generated(&path, &changed, limits()).is_err());
    // Storage isolation only, not a substitute for native refine/extend tests.
    fs::write(
        scratch.0.join("downstream-publication-fixture"),
        b"new terminal substitution phase",
    )
    .unwrap();
    let reopened = ValidatedGeneration::open_generated(&path, &binding(), limits()).unwrap();
    assert_eq!(reopened.digest(), generation.digest());
    assert_eq!(reopened.state().census, generation.state().census);
}

#[test]
fn finite_store_physical_disk_quota_counts_orphans_and_staging_files() {
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    let first = commit(&mut writer, vec![key(1)], 1, 1);
    let exact = writer.charged_disk_bytes();
    assert_eq!(exact, physical_bytes(&path));
    drop(writer);
    let mut writer = StoreWriter::resume_generated(
        &path,
        binding(),
        StorageLimits {
            max_disk_bytes: exact,
            ..limits()
        },
    )
    .unwrap();
    assert_eq!(
        writer
            .commit_generated(vec![key(2)], state(2, 2), &AtomicBool::new(false))
            .unwrap_err()
            .kind,
        ErrorKind::Resource
    );
    assert_eq!(writer.current().unwrap().unwrap().digest(), first.digest());
    drop(writer);
    fs::write(
        path.join("pages/orphan-uncharged-by-old-manifest"),
        vec![0; 1024],
    )
    .unwrap();
    fs::write(path.join(".CURRENT.rustred-tmp-test"), vec![0; 512]).unwrap();
    assert_eq!(
        StoreWriter::resume_generated(
            &path,
            binding(),
            StorageLimits {
                max_disk_bytes: exact + 1535,
                ..limits()
            }
        )
        .err()
        .unwrap()
        .kind,
        ErrorKind::Resource
    );
    let writer = StoreWriter::resume_generated(&path, binding(), limits()).unwrap();
    assert_eq!(writer.charged_disk_bytes(), exact + 1536);
    assert_eq!(writer.current().unwrap().unwrap().digest(), first.digest());
}

#[test]
fn finite_store_directory_delta_and_census_quota_refusals_do_not_commit() {
    for policy in [
        StorageLimits {
            max_runs: 1,
            ..limits()
        },
        StorageLimits {
            max_pages: 1,
            ..limits()
        },
        StorageLimits {
            max_census_records: 0,
            ..limits()
        },
    ] {
        let scratch = Scratch::new();
        let path = scratch.0.join("store");
        let mut writer = StoreWriter::create(&path, binding(), policy).unwrap();
        let first = commit(&mut writer, vec![key(1)], 1, 1);
        let mut next = state(2, 2);
        if policy.max_census_records == 0 {
            next.census.terminals.push(vec![2, 1]);
        }
        assert_eq!(
            writer
                .commit_generated(vec![key(2)], next, &AtomicBool::new(false))
                .unwrap_err()
                .kind,
            ErrorKind::Resource
        );
        assert_eq!(writer.current().unwrap().unwrap().digest(), first.digest());
    }
    for policy in [
        StorageLimits {
            max_delta_records: 0,
            ..limits()
        },
        StorageLimits {
            max_delta_payload_bytes: 0,
            ..limits()
        },
        StorageLimits {
            max_directory_bytes: 0,
            ..limits()
        },
        StorageLimits {
            max_manifest_bytes: 64,
            ..limits()
        },
        StorageLimits {
            max_state_bytes: 64,
            ..limits()
        },
        StorageLimits {
            max_page_bytes: 64,
            ..limits()
        },
    ] {
        let scratch = Scratch::new();
        let path = scratch.0.join("store");
        let mut writer = StoreWriter::create(&path, binding(), policy).unwrap();
        assert_eq!(
            writer
                .commit_generated(vec![key(1)], state(1, 1), &AtomicBool::new(false))
                .unwrap_err()
                .kind,
            ErrorKind::Resource
        );
        assert!(writer.current().unwrap().is_none());
        assert!(!path.join(wire::CURRENT).exists());
    }
    assert!(
        StoreWriter::create(
            &Scratch::new().0.join("store"),
            binding(),
            StorageLimits {
                max_transient_bytes: 1,
                ..limits()
            }
        )
        .is_err()
    );
}

#[test]
fn finite_store_atomic_staging_names_fit_the_file_count_grant() {
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    // writer.lock plus one final page would fit; simultaneous staging+final
    // pathnames would not. Refuse before installing even the first page.
    let mut writer = StoreWriter::create(
        &path,
        binding(),
        StorageLimits {
            max_physical_files: 2,
            ..limits()
        },
    )
    .unwrap();
    assert_eq!(
        writer
            .commit_generated(vec![key(1)], state(1, 1), &AtomicBool::new(false))
            .unwrap_err()
            .kind,
        ErrorKind::Resource
    );
    assert_eq!(fs::read_dir(path.join("pages")).unwrap().count(), 0);
    assert!(writer.current().unwrap().is_none());
}
