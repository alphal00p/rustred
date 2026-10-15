use super::super::raw::{RawDelta, RawPage};
use super::*;
use std::mem::size_of;

fn rows(values: &[i64]) -> Vec<Vec<i64>> {
    values.iter().map(|&n| vec![n, 1]).collect()
}
#[test]
fn paged_raw_extreme_record_count_and_missing_decode_budget_refuse_before_allocation() {
    let scratch = Scratch::new();
    let path = scratch.0.join("huge-capacity");
    for raw in [
        RawCensusLimits {
            max_page_records: usize::MAX,
            max_page_bytes: 64,
            ..limits().raw
        },
        RawCensusLimits {
            max_decode_transient_bytes: 1,
            ..limits().raw
        },
    ] {
        let settings = StorageLimits { raw, ..limits() };
        let error = match StoreWriter::create(&path, binding(), settings) {
            Ok(_) => panic!("unbounded capacity admitted"),
            Err(e) => e,
        };
        assert_eq!(error.kind, ErrorKind::Resource);
        assert!(!path.exists());
    }
}
fn raw_path(generation: &ValidatedGeneration, kind: RawKeyKind) -> PathBuf {
    let page = &generation.manifest.raw.runs(kind)[0][0];
    generation
        .directory
        .join("raw")
        .join(wire::name(&page.object.digest, "rrraw"))
}
fn collect(
    generation: &ValidatedGeneration,
    limits: RawCensusLimits,
) -> Result<(RawCensusSummary, Vec<(RawKeyKind, Vec<i64>)>)> {
    let mut keys = Vec::new();
    let summary = generation.visit_raw_census(
        limits,
        &AtomicBool::new(false),
        |_, _| Ok(()),
        |kind, key| {
            keys.push((kind, key.to_vec()));
            Ok(())
        },
    )?;
    Ok((summary, keys))
}
#[test]
fn paged_raw_union_overlaps_and_eviction_never_infers_distinct_from_reference_count() {
    let scratch = Scratch::new();
    let path = scratch.0.join("raw-union");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    let terminal = rows(&[1, 3, 5]);
    let zero = rows(&[0, 2, 4]);
    writer
        .commit_generated_with_raw(
            (0..6).map(key).collect(),
            RawDelta::slices(&terminal, &zero),
            state(6, 1),
            &AtomicBool::new(false),
        )
        .unwrap();
    let terminal2 = rows(&[3, 5, 7]);
    let zero2 = rows(&[2, 4, 6]);
    let generation = writer
        .commit_generated_with_raw(
            (10..16).map(key).collect(),
            RawDelta::slices(&terminal2, &zero2),
            state(12, 2),
            &AtomicBool::new(false),
        )
        .unwrap()
        .generation;
    assert_eq!(
        generation
            .raw_census_directory()
            .stored_references(RawKeyKind::Terminal)
            .unwrap(),
        6
    );
    assert_eq!(
        generation
            .raw_census_directory()
            .stored_references(RawKeyKind::Zero)
            .unwrap(),
        6
    );
    for cached in [0, 1, 16] {
        let (summary, actual) = collect(
            &generation,
            RawCensusLimits {
                max_merge_cached_pages: cached,
                ..limits().raw
            },
        )
        .unwrap();
        assert_eq!((summary.terminals, summary.zeros), (4, 4));
        assert_eq!(
            actual.iter().map(|(_, k)| k[0]).collect::<Vec<_>>(),
            (0..8).collect::<Vec<_>>()
        );
        assert!(summary.pages_read >= 8);
        if cached == 1 {
            assert!(summary.page_evictions > 0);
        }
    }
}
#[test]
fn paged_raw_complete_payload_can_exceed_small_state_without_cold_key_reads() {
    let scratch = Scratch::new();
    let path = scratch.0.join("raw-large");
    let settings = StorageLimits {
        max_state_bytes: 2048,
        max_delta_records: 2000,
        max_pages: 2000,
        max_physical_files: 5000,
        raw: RawCensusLimits {
            max_page_records: 32,
            ..limits().raw
        },
        ..limits()
    };
    let mut writer = StoreWriter::create(&path, binding(), settings).unwrap();
    let zeros = (0..1000).rev().map(|n| vec![-n, 0]).collect::<Vec<_>>();
    let work = zeros.iter().cloned().map(StoredWork::Route).collect();
    let first = writer
        .commit_generated_with_raw(
            work,
            RawDelta::slices(&[], &zeros),
            state(1000, 1),
            &AtomicBool::new(false),
        )
        .unwrap()
        .generation;
    assert!(zeros.len() * 2 * size_of::<i64>() > settings.max_state_bytes);
    assert!(first.state().payload_bytes().unwrap() < settings.max_state_bytes);
    let page = raw_path(&first, RawKeyKind::Zero);
    let original = fs::read(&page).unwrap();
    let mut changed = original.clone();
    changed[0] ^= 1;
    fs::write(&page, &changed).unwrap();
    drop(writer);
    let reopened = ValidatedGeneration::open_generated(&path, &binding(), settings).unwrap();
    assert_eq!(reopened.digest(), first.digest());
    assert!(collect(&reopened, settings.raw).is_err());
    fs::write(page, original).unwrap();
    assert_eq!(collect(&reopened, settings.raw).unwrap().0.zeros, 1000);
}
#[test]
fn paged_raw_cannot_be_read_as_work_and_wrong_kind_or_counts_fail() {
    let scratch = Scratch::new();
    let path = scratch.0.join("raw-kind");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    let terminals = rows(&[1, 3]);
    let generation = writer
        .commit_generated_with_raw(
            vec![key(1), key(3)],
            RawDelta::slices(&terminals, &[]),
            state(2, 1),
            &AtomicBool::new(false),
        )
        .unwrap()
        .generation;
    let bytes = fs::read(raw_path(&generation, RawKeyKind::Terminal)).unwrap();
    assert!(wire::decode::<wire::Page>(wire::PAGE_MAGIC, &bytes, limits().max_page_bytes).is_err());
    let reference = &generation.manifest.raw.terminals[0][0];
    let mut wrong = reference.clone();
    wrong.kind = RawKeyKind::Zero;
    assert!(
        super::super::raw::decode_page(&generation.directory, &wrong, &binding(), limits().raw)
            .is_err()
    );
    let mut wrong = reference.clone();
    wrong.records += 1;
    assert!(
        super::super::raw::decode_page(&generation.directory, &wrong, &binding(), limits().raw)
            .is_err()
    );
    let mut wrong_binding = binding();
    wrong_binding.program_identity[0] ^= 1;
    assert!(
        super::super::raw::decode_page(
            &generation.directory,
            reference,
            &wrong_binding,
            limits().raw
        )
        .is_err()
    );
    let mut page: RawPage = wire::decode(
        super::super::raw::MAGIC,
        &bytes,
        limits().raw.max_page_bytes,
    )
    .unwrap();
    page.keys.reverse();
    let changed =
        wire::encode(super::super::raw::MAGIC, &page, limits().raw.max_page_bytes).unwrap();
    let mut wrong = reference.clone();
    wrong.object.digest = *blake3::hash(&changed).as_bytes();
    wrong.object.bytes = changed.len() as u64;
    fs::write(
        generation
            .directory
            .join("raw")
            .join(wire::name(&wrong.object.digest, "rrraw")),
        changed,
    )
    .unwrap();
    assert!(
        super::super::raw::decode_page(&generation.directory, &wrong, &binding(), limits().raw)
            .is_err()
    );
}
#[test]
fn paged_raw_merge_quota_cross_kind_and_final_cancel_fail_closed() {
    let scratch = Scratch::new();
    let path = scratch.0.join("raw-conflict");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    let keys = rows(&[1, 3]);
    let generation = writer
        .commit_generated_with_raw(
            (1..5).map(key).collect(),
            RawDelta::slices(&keys, &keys),
            state(4, 1),
            &AtomicBool::new(false),
        )
        .unwrap()
        .generation;
    assert!(
        collect(&generation, limits().raw)
            .unwrap_err()
            .to_string()
            .contains("conflict")
    );
    for policy in [
        RawCensusLimits {
            max_merge_runs: 0,
            ..limits().raw
        },
        RawCensusLimits {
            max_merge_payload_bytes: 1,
            ..limits().raw
        },
        RawCensusLimits {
            max_merge_read_bytes: 0,
            ..limits().raw
        },
    ] {
        assert_eq!(
            collect(&generation, policy).unwrap_err().kind,
            ErrorKind::Resource
        );
    }
    let scratch = Scratch::new();
    let mut writer = StoreWriter::create(&scratch.0.join("cancel"), binding(), limits()).unwrap();
    let generation = writer
        .commit_generated_with_raw(
            vec![key(1)],
            RawDelta::slices(&rows(&[1]), &[]),
            state(1, 1),
            &AtomicBool::new(false),
        )
        .unwrap()
        .generation;
    let cancel = AtomicBool::new(false);
    let error = generation
        .visit_raw_census(
            limits().raw,
            &cancel,
            |_, _| Ok(()),
            |_, _| {
                cancel.store(true, Ordering::Release);
                Ok(())
            },
        )
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Cancelled);
}

#[test]
fn paged_raw_nonempty_transaction_failures_preserve_exact_current_prefix() {
    use super::super::writer::Stage;
    for stage in [
        Stage::AfterPages,
        Stage::BeforeCurrent,
        Stage::CancelAfterPages,
        Stage::CurrentUncertain,
        Stage::AfterCurrent,
    ] {
        let scratch = Scratch::new();
        let path = scratch.0.join("raw-atomic");
        let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
        let terminal = rows(&[1]);
        let first = writer
            .commit_generated_with_raw(
                vec![key(1)],
                RawDelta::slices(&terminal, &[]),
                state(1, 1),
                &AtomicBool::new(false),
            )
            .unwrap()
            .generation;
        writer.inject_failure(stage);
        let zero = vec![vec![0, 0]];
        let result = writer.commit_generated_with_raw(
            vec![StoredWork::Route(vec![0, 0])],
            RawDelta::slices(&[], &zero),
            state(2, 2),
            &AtomicBool::new(false),
        );
        let committed = matches!(stage, Stage::CurrentUncertain | Stage::AfterCurrent);
        if matches!(stage, Stage::AfterCurrent) {
            assert!(result.unwrap().cleanup_warning.is_some());
        } else {
            assert!(result.is_err());
        }
        drop(writer);
        let resumed = ValidatedGeneration::open_generated(&path, &binding(), limits()).unwrap();
        assert_eq!(resumed.generation(), if committed { 2 } else { 1 });
        if !committed {
            assert_eq!(resumed.digest(), first.digest());
        }
        let (summary, keys) = collect(&resumed, limits().raw).unwrap();
        assert_eq!(
            (summary.terminals, summary.zeros),
            (1, u64::from(committed))
        );
        assert!(keys.contains(&(RawKeyKind::Terminal, vec![1, 1])));
        assert_eq!(
            resumed.state().cursors[0].completed_windows,
            if committed { 2 } else { 1 }
        );
        let writer = StoreWriter::resume_generated(&path, binding(), limits()).unwrap();
        assert_eq!(writer.charged_disk_bytes(), physical_bytes(&path));
    }
}

#[test]
fn paged_raw_missing_file_and_duplicate_payload_fail_as_errors() {
    let scratch = Scratch::new();
    let path = scratch.0.join("raw-errors");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    let terminal = rows(&[1, 3]);
    let generation = writer
        .commit_generated_with_raw(
            vec![key(1), key(3)],
            RawDelta::slices(&terminal, &[]),
            state(2, 1),
            &AtomicBool::new(false),
        )
        .unwrap()
        .generation;
    let reference = &generation.manifest.raw.terminals[0][0];
    let duplicate = RawPage {
        execution: binding(),
        kind: RawKeyKind::Terminal,
        keys: rows(&[1, 1]),
    };
    let bytes = wire::encode(
        super::super::raw::MAGIC,
        &duplicate,
        limits().raw.max_page_bytes,
    )
    .unwrap();
    let mut bad = reference.clone();
    bad.object.digest = *blake3::hash(&bytes).as_bytes();
    bad.object.bytes = bytes.len() as u64;
    bad.last = bad.first.clone();
    fs::write(
        path.join("raw")
            .join(wire::name(&bad.object.digest, "rrraw")),
        bytes,
    )
    .unwrap();
    assert_eq!(
        super::super::raw::decode_page(&path, &bad, &binding(), limits().raw)
            .unwrap_err()
            .kind,
        ErrorKind::Corrupt
    );
    fs::remove_file(raw_path(&generation, RawKeyKind::Terminal)).unwrap();
    assert_eq!(
        collect(&generation, limits().raw).unwrap_err().kind,
        ErrorKind::Io
    );
    assert_eq!(
        ValidatedGeneration::open_generated(&path, &binding(), limits())
            .unwrap_err()
            .kind,
        ErrorKind::Io
    );
}

#[test]
fn paged_raw_state_version_does_not_reinterpret_monolithic_census_bytes() {
    let bytes = wire::encode(
        b"RUSTRED-FINITE-CURRENT-1\0",
        &wire::Tip {
            generation: 1,
            manifest: wire::ObjectRef {
                digest: [0; 32],
                bytes: 100,
            },
        },
        wire::TIP_LIMIT,
    )
    .unwrap();
    assert!(wire::decode::<wire::Tip>(wire::TIP_MAGIC, &bytes, wire::TIP_LIMIT).is_err());
}
