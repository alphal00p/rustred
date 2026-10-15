//! Persisted fixtures prove ENUMERATION continuity only, not reduction closure.
//! Their empty Work/census is deliberately not imported into a native baseline.
use super::*;
use crate::application::routed_campaign::finite_demand::storage::{
    self, GenerationState, RawCensus, RawExecutionBinding, ScopeBinding, ScopeRole, StorageLimits,
    StoreWriter, ValidatedGeneration,
};
use rustred::solver::DomainPowerBounds;
use std::{fs, path::PathBuf, sync::atomic::AtomicU64};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..128 {
            let path = std::env::temp_dir().join(format!(
                "rustred-prefix-restore-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("owned cursor test directory: {error}"),
            }
        }
        panic!("cannot acquire owned cursor test directory");
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn storage_limits() -> StorageLimits {
    StorageLimits {
        raw:Default::default(),
        max_arity: 32,
        max_delta_records: 0,
        max_delta_payload_bytes: 1024,
        max_page_records: 2,
        max_page_bytes: 65536,
        max_cached_pages: 0,
        max_cached_page_bytes: 0,
        max_directory_bytes: 1048576,
        max_manifest_bytes: 1048576,
        max_state_bytes: 1048576,
        max_scope_rows: 1,
        max_census_records: 0,
        max_runs: 0,
        max_pages: 0,
        max_physical_files: 4096,
        max_disk_bytes: 64 * 1024 * 1024,
        max_lookup_pages: 0,
        max_lookup_bytes: 0,
        max_transient_bytes: 128 * 1024 * 1024,
    }
}
fn limits(points: usize) -> WindowLimits {
    WindowLimits {
        max_seed_points: points,
        max_seed_bytes: 65536,
        max_positive_layers: 64,
        max_partition_checks: 10000,
    }
}
fn region<const N: usize>(
    support: [bool; N],
    a: u64,
    r: u32,
    lo: i64,
    hi: i64,
) -> RootRegionInput<N> {
    RootRegionInput {
        support,
        lower: vec![0; N],
        upper: vec![None; N],
        rank: Some(r),
        powers: DomainPowerBounds {
            max_positive_power: Some(a),
            min_power_difference: Some(lo),
            max_power_difference: Some(hi),
        },
    }
}
fn source<const N: usize>(
    region: RootRegionInput<N>,
    arity: usize,
    points: usize,
) -> PrefixWindows<N> {
    PrefixWindows::try_new(
        "enumeration-only:0",
        region,
        arity,
        limits(points),
        &AtomicBool::new(false),
    )
    .unwrap()
}
fn row() -> ScopeRow {
    ScopeRow {
        origin: "enumeration-only:0".into(),
        role: ScopeRole::Required,
        exact_domain: [5; 32],
    }
}
fn execution(arity: usize) -> RawExecutionBinding {
    RawExecutionBinding {
        physical_arity: u32::try_from(arity).unwrap(),
        program_identity: [7; 32],
    }
}
fn state(cursor: &PrefixCursor, binding: [u8; 32]) -> GenerationState {
    let occurrences = cursor.emitted().to_string().parse::<u64>().unwrap();
    GenerationState {
        scope: ScopeBinding {
            original_source: [4; 32],
            rows: vec![row()],
        },
        cursors: vec![storage::CursorRecord {
            source_policy: binding,
            completed_windows: cursor.windows(),
            input_occurrences: occurrences,
            encoded_cursor: cursor.encode().unwrap(),
        }],
        completed_work: 0,
        input_occurrences: occurrences,
        census: RawCensus::default(),
    }
}
fn persist<const N: usize>(
    source: &PrefixWindows<N>,
    state: GenerationState,
) -> (Scratch, ValidatedGeneration) {
    let scratch = Scratch::new();
    let directory = scratch.0.join("store");
    let binding = execution(source.arity);
    let mut writer = StoreWriter::create(&directory, binding.clone(), storage_limits()).unwrap();
    let committed = writer
        .commit_generated(Vec::new(), state, &AtomicBool::new(false))
        .unwrap()
        .generation;
    let reopened =
        ValidatedGeneration::open_generated(&directory, &binding, storage_limits()).unwrap();
    assert_eq!(committed.digest(), reopened.digest());
    (scratch, reopened)
}
fn exact_next<const N: usize>(source: &PrefixWindows<N>, a: &PrefixCursor, b: &PrefixCursor) {
    let cancel = AtomicBool::new(false);
    let expected = source.next_window(a, &cancel).unwrap();
    let actual = source.next_window(b, &cancel).unwrap();
    match (expected, actual) {
        (None, None) => {}
        (Some(expected), Some(actual)) => {
            assert_eq!(expected.targets(), actual.targets());
            assert_eq!(
                expected.transition().encode().unwrap(),
                actual.transition().encode().unwrap()
            );
            assert_eq!(expected.successor(), actual.successor());
        }
        _ => panic!("restored enumeration changed exhaustion"),
    }
}
fn every_boundary<const N: usize>(source: PrefixWindows<N>) {
    let scratch = Scratch::new();
    let directory = scratch.0.join("store");
    let binding = execution(source.arity);
    let mut writer = StoreWriter::create(&directory, binding.clone(), storage_limits()).unwrap();
    let mut cursor = source.initial_cursor();
    let cancel = AtomicBool::new(false);
    loop {
        let saved = writer
            .commit_generated(
                Vec::new(),
                state(&cursor, source.source_policy_binding()),
                &cancel,
            )
            .unwrap()
            .generation;
        let reopened =
            ValidatedGeneration::open_generated(&directory, &binding, storage_limits()).unwrap();
        assert_eq!(saved.digest(), reopened.digest());
        let restored = source
            .restore_trusted_generation_cursor(
                &reopened.cursor(0).unwrap(),
                reopened.digest(),
                &row(),
                &cancel,
            )
            .unwrap();
        assert_eq!(restored, cursor);
        exact_next(&source, &cursor, &restored);
        let (checked, calls) = source
            .validate_generated_cursor(cursor.0.clone(), &cancel)
            .unwrap();
        assert_eq!(checked, cursor);
        assert!(calls <= cursor.0.frames.len() + 1);
        let Some(window) = source.next_window(&cursor, &cancel).unwrap() else {
            break;
        };
        cursor = window.successor().clone();
    }
}

#[test]
fn trusted_restore_matches_every_boundary_for_all_supports_and_correlated_d_bands() {
    for bits in 0..8 {
        let support: [bool; 3] = std::array::from_fn(|axis| bits & (1 << axis) != 0);
        for (a, r, lo, hi) in [(6, 2, -2, 5), (5, 2, 1, 2)] {
            for points in [1, 3, 7] {
                every_boundary(source(region(support, a, r, lo, hi), 3, points));
            }
        }
    }
}

#[test]
fn trusted_restore_handles_initial_fixed_singleton_empty_and_padding_without_dummy_axes() {
    every_boundary(source(region([true, false], 0, 0, 0, 0), 2, 2));
    let mut singleton = region([true, false, true], 8, 4, -4, 8);
    singleton.lower = vec![2, 3, 1];
    singleton.upper = vec![Some(2), Some(3), Some(1)];
    every_boundary(source(singleton, 3, 2));
    let mut padded = region([true, false, false, false], 4, 2, -2, 4);
    padded.upper[2..].fill(Some(0));
    every_boundary(source(padded, 2, 2));
}

#[test]
fn trusted_restore_late_cursor_needs_only_current_depth_not_prior_windows() {
    let source = source(region([false], 0, 1_000_000, -1_000_000, 0), 1, 1);
    let mut cursor = source.initial_cursor();
    let cancel = AtomicBool::new(false);
    for _ in 0..2000 {
        cursor = source
            .next_window(&cursor, &cancel)
            .unwrap()
            .unwrap()
            .successor()
            .clone();
    }
    assert_eq!(cursor.windows(), 2000);
    let (_scratch, generation) = persist(&source, state(&cursor, source.source_policy_binding()));
    // The generation contains this cursor only, not 2,000 transition records.
    let restored = source
        .restore_trusted_generation_cursor(
            &generation.cursor(0).unwrap(),
            generation.digest(),
            &row(),
            &cancel,
        )
        .unwrap();
    let (_, calls) = source
        .validate_generated_cursor(restored.0.clone(), &cancel)
        .unwrap();
    assert_eq!(calls, 2);
    exact_next(&source, &cursor, &restored);
}

fn middle() -> (PrefixWindows<3>, PrefixCursor) {
    let source = source(region([false; 3], 0, 5, -5, 0), 3, 1);
    let mut cursor = source.initial_cursor();
    for _ in 0..8 {
        cursor = source
            .next_window(&cursor, &AtomicBool::new(false))
            .unwrap()
            .unwrap()
            .successor()
            .clone();
    }
    assert_eq!(cursor.0.frames.len(), 3);
    (source, cursor)
}
fn refuses(source: &PrefixWindows<3>, state: GenerationState) {
    let (_scratch, generation) = persist(source, state);
    let token = generation.cursor(0).unwrap();
    let bytes = token.encoded_cursor().to_vec();
    assert!(
        source
            .restore_trusted_generation_cursor(
                &token,
                generation.digest(),
                &row(),
                &AtomicBool::new(false)
            )
            .is_err()
    );
    assert_eq!(token.encoded_cursor(), bytes);
}

#[test]
fn trusted_restore_rejects_generation_scope_source_policy_counter_and_wire_mutations() {
    let (source, cursor) = middle();
    let saved = state(&cursor, source.source_policy_binding());
    let (_scratch, generation) = persist(&source, saved.clone());
    let token = generation.cursor(0).unwrap();
    let mut wrong_generation = generation.digest();
    wrong_generation[0] ^= 1;
    assert!(
        source
            .restore_trusted_generation_cursor(
                &token,
                wrong_generation,
                &row(),
                &AtomicBool::new(false)
            )
            .is_err()
    );
    for changed in [
        ScopeRow {
            origin: "changed".into(),
            ..row()
        },
        ScopeRow {
            role: ScopeRole::Auxiliary,
            ..row()
        },
        ScopeRow {
            exact_domain: [9; 32],
            ..row()
        },
    ] {
        assert!(
            source
                .restore_trusted_generation_cursor(
                    &token,
                    generation.digest(),
                    &changed,
                    &AtomicBool::new(false)
                )
                .is_err()
        );
    }
    let other = PrefixWindows::try_new(
        "changed-origin",
        source.region.clone(),
        source.arity,
        source.limits,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(
        other
            .restore_trusted_generation_cursor(
                &token,
                generation.digest(),
                &row(),
                &AtomicBool::new(false)
            )
            .is_err()
    );
    let other = PrefixWindows::try_new(
        "enumeration-only:0",
        source.region.clone(),
        source.arity,
        limits(2),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(
        other
            .restore_trusted_generation_cursor(
                &token,
                generation.digest(),
                &row(),
                &AtomicBool::new(false)
            )
            .is_err()
    );
    let mut changed = saved.clone();
    changed.cursors[0].source_policy = [9; 32];
    refuses(&source, changed);
    let mut changed = saved.clone();
    changed.cursors[0].completed_windows += 1;
    refuses(&source, changed);
    let mut changed = saved.clone();
    changed.cursors[0].input_occurrences += 1;
    changed.input_occurrences += 1;
    refuses(&source, changed);
    let mut changed = saved.clone();
    changed.cursors[0].encoded_cursor.push(0);
    refuses(&source, changed);
    let mut changed = saved;
    changed.cursors[0].encoded_cursor = vec![255];
    refuses(&source, changed);
    let error = source
        .restore_trusted_generation_cursor(
            &token,
            generation.digest(),
            &row(),
            &AtomicBool::new(true),
        )
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Cancelled);
}

#[test]
fn trusted_restore_rejects_inconsistent_dfs_frames_extrema_and_telescoping() {
    let (source, cursor) = middle();
    let mut mutations = Vec::new();
    let mut r = cursor.0.clone();
    r.version += 1;
    mutations.push(r);
    let mut r = cursor.0.clone();
    r.binding[0] ^= 1;
    mutations.push(r);
    let mut r = cursor.0.clone();
    r.prefix.pop();
    mutations.push(r);
    let mut r = cursor.0.clone();
    r.frames.clear();
    r.prefix.clear();
    mutations.push(r);
    let mut r = cursor.0.clone();
    r.prefix[0] = 1;
    mutations.push(r);
    let mut r = cursor.0.clone();
    r.finished = true;
    mutations.push(r);
    for index in 0..cursor.0.frames.len() {
        let mut r = cursor.0.clone();
        r.frames[index].lower += 1;
        mutations.push(r);
        let mut r = cursor.0.clone();
        r.frames[index].upper += 1;
        mutations.push(r);
        let mut r = cursor.0.clone();
        r.frames[index].current += 1;
        mutations.push(r);
        let mut r = cursor.0.clone();
        r.frames[index].parent_count += Integer::from(1);
        mutations.push(r);
        let mut r = cursor.0.clone();
        r.frames[index].completed_children += Integer::from(1);
        mutations.push(r);
        let mut r = cursor.0.clone();
        r.frames[index].completed_children = Integer::from(-1);
        mutations.push(r);
    }
    for record in mutations {
        let mut saved = state(&cursor, source.source_policy_binding());
        saved.cursors[0].encoded_cursor = encode(&record).unwrap();
        refuses(&source, saved);
    }
}

#[test]
fn trusted_restore_rejects_invented_empty_singleton_and_finished_counter_shapes() {
    let (source, cursor) = middle();
    let mut record = cursor.0.clone();
    record.finished = true;
    record.frames.clear();
    record.prefix.clear();
    let mut saved = state(&cursor, source.source_policy_binding());
    saved.cursors[0].encoded_cursor = encode(&record).unwrap();
    refuses(&source, saved);
    let empty = self::source(region([true, false, false], 0, 0, 0, 0), 3, 1);
    let mut record = empty.initial_cursor().0;
    record.windows = 1;
    let mut saved = state(&PrefixCursor(record), empty.source_policy_binding());
    let (_scratch, generation) = persist(&empty, saved.clone());
    assert!(
        empty
            .restore_trusted_generation_cursor(
                &generation.cursor(0).unwrap(),
                generation.digest(),
                &row(),
                &AtomicBool::new(false)
            )
            .is_err()
    );
    // A decoded Integer cannot be silently truncated into a generation's u64 counter.
    let mut record = cursor.0.clone();
    record.emitted = Integer::from(u64::MAX) + Integer::from(1);
    saved = state(&cursor, source.source_policy_binding());
    saved.cursors[0].encoded_cursor = encode(&record).unwrap();
    refuses(&source, saved);
    let mut fixed = region([true, false, true], 8, 4, -4, 8);
    fixed.lower = vec![2, 3, 1];
    fixed.upper = vec![Some(2), Some(3), Some(1)];
    let singleton = self::source(fixed, 3, 2);
    let done = singleton
        .next_window(&singleton.initial_cursor(), &AtomicBool::new(false))
        .unwrap()
        .unwrap()
        .successor()
        .clone();
    let mut invented = done.clone();
    invented.0.windows = 2;
    refuses(
        &singleton,
        state(&invented, singleton.source_policy_binding()),
    );
    let mut invented = done;
    invented.0.finished = false;
    refuses(
        &singleton,
        state(&invented, singleton.source_policy_binding()),
    );
}
