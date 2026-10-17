//! Public integer-only resident index checks and a bounded opt-in microbenchmark.
//! This does not generate IBPs or access a production campaign.
use rustred::persistence::finite_work::*;
use std::{
    collections::HashSet,
    hint::black_box,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

struct Pages {
    pages: Vec<(GeneratedWorkPageRef, ValidatedWorkPage)>,
}
impl GeneratedPageProvider for Pages {
    fn visit_validated_page(
        &self,
        expected: &GeneratedWorkPageRef,
        _: u64,
        visitor: &mut dyn FnMut(&ValidatedWorkPage),
    ) -> GeneratedWorkResult<GeneratedPageReadUsage> {
        visitor(
            &self
                .pages
                .iter()
                .find(|(reference, _)| reference == expected)
                .unwrap()
                .1,
        );
        Ok(GeneratedPageReadUsage {
            disk_bytes: 0,
            cache_hit: true,
        })
    }
}

fn fixture(
    arity: usize,
    runs: Vec<Vec<GeneratedWorkKey>>,
) -> (TrustedGeneratedWorkDirectory, Pages) {
    let execution = GeneratedExecutionBinding {
        physical_arity: arity.try_into().unwrap(),
        program_identity: [2; 32],
    };
    let mut pages = Vec::new();
    let mut references = Vec::new();
    for mut records in runs {
        records.sort_unstable();
        let record = GeneratedWorkPageRecord {
            execution: execution.clone(),
            records,
        };
        let bytes = encode_metadata_frame(PAGE_MAGIC, &record, usize::MAX).unwrap();
        let reference = GeneratedWorkPageRef {
            object: GeneratedObjectRef {
                digest: *blake3::hash(&bytes).as_bytes(),
                bytes: bytes.len() as u64,
            },
            records: record.records.len() as u64,
            first: record.records.first().unwrap().clone(),
            last: record.records.last().unwrap().clone(),
        };
        let page = ValidatedWorkPage::decode_generated(
            &bytes,
            &reference,
            &execution,
            bytes
                .len()
                .saturating_add(
                    work_payload_bytes(&record.records, record.records.capacity())
                        .unwrap()
                        .saturating_mul(2),
                )
                .max(64 * 1024 * 1024),
            record.records.len(),
        )
        .unwrap();
        references.push(vec![reference.clone()]);
        pages.push((reference, page));
    }
    let directory = TrustedGeneratedWorkDirectory::try_from_trusted_generated(
        execution,
        [3; 32],
        Arc::new(references),
        GeneratedDirectoryLimits {
            max_arity: usize::MAX,
            max_runs: usize::MAX,
            max_pages: usize::MAX,
            max_directory_payload_bytes: usize::MAX,
            max_page_records: usize::MAX,
            max_page_bytes: usize::MAX,
            max_referenced_bytes: u64::MAX,
            max_lookup_pages: usize::MAX,
            max_lookup_bytes: u64::MAX,
        },
    )
    .unwrap();
    (directory, Pages { pages })
}

fn key(arity: usize, n: i64, apply: bool) -> GeneratedWorkKey {
    let mut powers = vec![0; arity];
    powers[0] = n;
    if arity > 1 {
        powers[1] = i64::MIN;
    }
    if arity > 2 {
        powers[2] = i64::MAX;
    }
    if apply {
        GeneratedWorkKey::Apply {
            owner: powers.iter().map(|&n| n > 0).collect(),
            powers,
        }
    } else {
        GeneratedWorkKey::Route(powers)
    }
}

#[test]
fn packed_public_hydration_high_arity_exact_phase_and_invalid_owner() {
    for arity in [1, 15, 65, 129, 9000] {
        let keys = (0..6)
            .flat_map(|n| [key(arity, n, false), key(arity, n, true)])
            .collect::<Vec<_>>();
        let (directory, pages) = fixture(arity, vec![keys.clone()]);
        let resident = TrustedGeneratedResidentWork::hydrate(
            &directory,
            &pages,
            8 * 1024 * 1024,
            0,
            &AtomicBool::new(false),
        )
        .unwrap()
        .unwrap();
        assert_eq!(resident.statistics().unwrap().records, keys.len());
        for key in keys {
            assert!(resident.contains(&key).unwrap());
        }
        assert!(!resident.contains(&key(arity, 7, false)).unwrap());
        let mut malformed = key(arity, 1, true);
        if let GeneratedWorkKey::Apply { owner, .. } = &mut malformed {
            owner[0] = false;
        }
        assert_eq!(
            resident.contains(&malformed).unwrap_err().kind,
            GeneratedWorkErrorKind::Corrupt
        );
        assert_eq!(
            resident
                .contains(&key(arity + 1, 1, false))
                .unwrap_err()
                .kind,
            GeneratedWorkErrorKind::Corrupt
        );
        resident.validate_directory(&directory).unwrap();
    }
}

#[test]
fn packed_hydration_declines_budget_and_rejects_duplicates_and_cancellation() {
    let keys = (0..8).map(|n| key(65, n, n % 2 == 0)).collect::<Vec<_>>();
    let (directory, pages) = fixture(65, vec![keys.clone()]);
    let cancel = AtomicBool::new(false);
    assert!(
        TrustedGeneratedResidentWork::hydrate(&directory, &pages, 1024, 0, &cancel)
            .unwrap()
            .is_none()
    );
    cancel.store(true, Ordering::Release);
    assert_eq!(
        TrustedGeneratedResidentWork::hydrate(&directory, &pages, usize::MAX, 0, &cancel)
            .err()
            .unwrap()
            .kind,
        GeneratedWorkErrorKind::Cancelled
    );
    cancel.store(false, Ordering::Release);
    let (directory, pages) = fixture(65, vec![keys[..5].to_vec(), keys[4..].to_vec()]);
    assert_eq!(
        TrustedGeneratedResidentWork::hydrate(&directory, &pages, 1024 * 1024, 0, &cancel)
            .err()
            .unwrap()
            .kind,
        GeneratedWorkErrorKind::Corrupt
    );
}

#[test]
#[ignore = "measurement only: run explicitly with --ignored --nocapture"]
fn packed_vs_owned_key_hydration_lookup_and_drop_microbenchmark() {
    let records = std::env::var("RUSTRED_RESIDENT_BENCH_RECORDS")
        .ok()
        .map(|v| v.parse::<usize>().unwrap())
        .unwrap_or(200_000);
    let arity = std::env::var("RUSTRED_RESIDENT_BENCH_ARITY")
        .ok()
        .map(|v| v.parse::<usize>().unwrap())
        .unwrap_or(15);
    assert!(records > 0 && arity > 0);
    let keys = (0..records)
        .map(|n| key(arity, n.try_into().unwrap(), n % 2 == 0))
        .collect::<Vec<_>>();
    let (directory, pages) = fixture(arity, vec![keys.clone()]);
    // Boundary: already decoded/validated pages to immutable lookup index.
    let start = Instant::now();
    let resident = TrustedGeneratedResidentWork::hydrate(
        &directory,
        &pages,
        usize::MAX,
        0,
        &AtomicBool::new(false),
    )
    .unwrap()
    .unwrap();
    let packed_hydrate = start.elapsed().as_secs_f64();
    let statistics = resident.statistics().unwrap();
    let start = Instant::now();
    for key in &keys {
        assert!(black_box(resident.contains(black_box(key)).unwrap()));
    }
    let packed_lookup = start.elapsed().as_secs_f64();
    let start = Instant::now();
    drop(resident);
    let packed_drop = start.elapsed().as_secs_f64();

    let start = Instant::now();
    let owned: HashSet<GeneratedWorkKey> = keys.iter().cloned().collect();
    let owned_hydrate = start.elapsed().as_secs_f64();
    let start = Instant::now();
    for key in &keys {
        // Match the public resident contains() boundary, including its exact
        // arity/Apply-owner validation, not just the underlying table probe.
        key.validate(arity).unwrap();
        assert!(black_box(owned.contains(black_box(key))));
    }
    let owned_lookup = start.elapsed().as_secs_f64();
    let start = Instant::now();
    drop(owned);
    let owned_drop = start.elapsed().as_secs_f64();
    println!(
        "{{\"records\":{records},\"arity\":{arity},\"packed_payload\":{},\"packed_hydrate_seconds\":{packed_hydrate},\"packed_lookup_seconds\":{packed_lookup},\"packed_drop_seconds\":{packed_drop},\"owned_hydrate_seconds\":{owned_hydrate},\"owned_lookup_seconds\":{owned_lookup},\"owned_drop_seconds\":{owned_drop},\"boundary\":\"predecoded_pages_index_only_not_campaign\"}}",
        statistics.retained_payload_estimate
    );
}
