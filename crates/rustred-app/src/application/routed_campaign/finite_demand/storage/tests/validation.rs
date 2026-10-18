//! Typed metadata reuse never changes cold validation or page integrity.
use super::*;

#[test]
fn unlimited_metadata_permission_uses_actual_transient_budget_and_cold_roundtrip() {
    let scratch = Scratch::new();
    let path = scratch.0.join("metadata-ram");
    let p = StorageLimits {
        max_manifest_bytes: usize::MAX,
        max_state_bytes: usize::MAX,
        max_transient_bytes: 128 * 1024 * 1024,
        ..limits()
    };
    let mut writer = StoreWriter::create(&path, binding(), p).unwrap();
    let mut reservations = Vec::new();
    let generation = writer
        .commit_generated_with_directory_accounting(
            vec![key(1), key(2)],
            state(2, 1),
            &AtomicBool::new(false),
            &mut |memory| {
                reservations.push(memory);
                Ok(())
            },
        )
        .unwrap()
        .generation;
    assert_eq!(reservations.len(), 4);
    assert_eq!(reservations[0].transient_bytes, 0);
    assert!(reservations[1].transient_bytes > 0);
    assert_eq!(
        reservations[3].retained_bytes,
        generation.metadata_payload_bytes().unwrap()
    );
    assert!(reservations[3].retained_bytes < 1024 * 1024);
    assert_eq!(
        wire::metadata_allocation_budget(usize::MAX, 100, 1000).unwrap(),
        900
    );
    assert!(wire::metadata_allocation_budget(usize::MAX, 1001, 1000).is_err());
    assert_eq!(
        wire::metadata_allocation_budget(1024, 100, 128 * 1024 * 1024).unwrap(),
        wire::MAX_FRAME
    );
    drop(writer);
    let cold = ValidatedGeneration::open_generated(&path, &binding(), p).unwrap();
    assert_eq!(cold.digest(), generation.digest());
    assert_eq!(
        cold.reader(p).unwrap().lookup(&key(1)).unwrap(),
        Lookup::Present
    );
    assert!(cold.metadata_payload_bytes().unwrap() < 1024 * 1024);
}

#[test]
fn metadata_transient_decoder_budget_checks_nested_allocations_and_actual_bytes() {
    let value = vec![vec![0i64; 1000]; 3];
    let frame = wire::encode(b"ram-test", &value, usize::MAX).unwrap();
    assert!(frame.len() < 4096);
    assert!(
        wire::decode_metadata::<Vec<Vec<i64>>>(b"ram-test", &frame, usize::MAX, frame.len() + 1000)
            .is_err()
    );
    assert_eq!(
        wire::decode_metadata::<Vec<Vec<i64>>>(b"ram-test", &frame, usize::MAX, 128 * 1024)
            .unwrap(),
        value
    );
    assert!(
        wire::decode_metadata::<Vec<Vec<i64>>>(b"ram-test", &frame, usize::MAX, frame.len() - 1)
            .is_err()
    );
    let mut slack = frame.clone();
    slack.reserve(1024 * 1024);
    assert_eq!(
        wire::decode_metadata::<Vec<Vec<i64>>>(b"ram-test", &slack, usize::MAX, 128 * 1024)
            .unwrap(),
        value
    );
    assert!(
        wire::decode_metadata_buffer::<Vec<Vec<i64>>>(b"ram-test", &slack, usize::MAX, 128 * 1024)
            .is_err()
    );
}
// Add below existing cfg(test) storage validation tests after current test
// artifact is frozen. This is synthetic metadata, never closure authority.
#[test]
#[ignore = "fully optimized metadata benchmark; no native closure claim"]
fn large_history_manifest_phase_benchmark() {
    let count: usize = std::env::var("RUSTRED_METADATA_BENCH_RUNS")
        .unwrap_or_else(|_| "149442".into())
        .parse()
        .unwrap();
    let arity: usize = std::env::var("RUSTRED_METADATA_BENCH_ARITY")
        .unwrap_or_else(|_| "15".into())
        .parse()
        .unwrap();
    let repeats: usize = std::env::var("RUSTRED_METADATA_BENCH_REPETITIONS")
        .unwrap_or_else(|_| "3".into())
        .parse()
        .unwrap();
    assert!(count > 0 && arity > 0 && arity <= 256);
    let execution = RawExecutionBinding {
        physical_arity: arity as u32,
        program_identity: [7; 32],
    };
    let p = StorageLimits {
        max_arity: 256,
        max_runs: usize::MAX,
        max_pages: usize::MAX,
        max_directory_bytes: 1 << 30,
        max_manifest_bytes: 512 << 20,
        max_disk_bytes: u64::MAX,
        max_transient_bytes: 1 << 30,
        max_cached_pages: 0,
        max_cached_page_bytes: 0,
        ..limits()
    };
    let page = |n: usize| wire::PageRef {
        object: wire::ObjectRef {
            digest: *blake3::hash(&(n as u64).to_le_bytes()).as_bytes(),
            bytes: 128,
        },
        records: 1,
        first: StoredWork::Route(vec![n as i64; arity]),
        last: StoredWork::Route(vec![n as i64; arity]),
    };
    let runs: Vec<_> = (0..count)
        .map(|n| vec![page(n * 2), page(n * 2 + 1)])
        .collect();
    let state = state((count * 2) as u64, 1);
    let manifest = wire::Manifest {
        execution: execution.clone(),
        generation: 2,
        previous: Some([3; 32]),
        state: wire::ObjectRef {
            digest: [4; 32],
            bytes: 128,
        },
        scope_digest: wire::scope_digest(&state.scope, p.max_state_bytes).unwrap(),
        runs: Arc::new(runs),
        raw: Arc::new(RawCensusDirectory::default()),
    };
    let valid = wire::validate_manifest(&manifest, &execution, p).unwrap();
    assert_eq!(valid.0, (count * 2) as u64);
    let frame = wire::encode(wire::MANIFEST_MAGIC, &manifest, p.max_manifest_bytes).unwrap();
    assert_eq!(
        wire::encode_manifest(&manifest, p.max_manifest_bytes).unwrap(),
        frame
    );
    let decoded: wire::Manifest =
        wire::decode(wire::MANIFEST_MAGIC, &frame, p.max_manifest_bytes).unwrap();
    assert_eq!(decoded, manifest);
    for repeat in 0..repeats {
        let t = std::time::Instant::now();
        let cloned = std::hint::black_box(manifest.runs.as_ref().clone());
        let clone_seconds = t.elapsed().as_secs_f64();
        drop(cloned);
        let t = std::time::Instant::now();
        let encoded = wire::encode(wire::MANIFEST_MAGIC, &manifest, p.max_manifest_bytes).unwrap();
        let encode_seconds = t.elapsed().as_secs_f64();
        let t = std::time::Instant::now();
        let sized_encoded = wire::encode_manifest(&manifest, p.max_manifest_bytes).unwrap();
        let sized_encode_seconds = t.elapsed().as_secs_f64();
        assert_eq!(sized_encoded, encoded);
        let t = std::time::Instant::now();
        let decoded: wire::Manifest =
            wire::decode(wire::MANIFEST_MAGIC, &encoded, p.max_manifest_bytes).unwrap();
        let decode_seconds = t.elapsed().as_secs_f64();
        let t = std::time::Instant::now();
        let charged = wire::measure_manifest_directory(&decoded, &execution, p).unwrap();
        let accounting_seconds = t.elapsed().as_secs_f64();
        let t = std::time::Instant::now();
        let validated = wire::validate_manifest(&decoded, &execution, p).unwrap();
        let validate_seconds = t.elapsed().as_secs_f64();
        assert_eq!(charged, validated.1);
        eprintln!(
            "{}",
            serde_json::json!({"benchmark":"manifest_history_phases",
            "repeat":repeat,"runs":count,"pages":count*2,"arity":arity,
            "encoded_bytes":encoded.len(),"directory_payload_bytes":charged,
            "clone_seconds":clone_seconds,"encode_seconds":encode_seconds,
            "sized_encode_seconds":sized_encode_seconds,
            "decode_seconds":decode_seconds,"accounting_seconds":accounting_seconds,
            "validate_seconds":validate_seconds,"synthetic_only":true})
        );
    }
    // Pure representation experiment. The codec encodes Arc<T> identically
    // to T; this does not install a new directory representation or capability.
    let shared: Vec<_> = manifest.runs.iter().cloned().map(Arc::new).collect();
    assert_eq!(
        wire::encode(b"runs", &shared, p.max_manifest_bytes).unwrap(),
        wire::encode(b"runs", manifest.runs.as_ref(), p.max_manifest_bytes).unwrap()
    );
    let t = std::time::Instant::now();
    let cloned = std::hint::black_box(shared.clone());
    let elapsed = t.elapsed().as_secs_f64();
    assert!(Arc::ptr_eq(&shared[0], &cloned[0]));
    eprintln!(
        "{}",
        serde_json::json!({"benchmark":"shared_run_clone_prototype",
        "runs":count,"seconds":elapsed,"wire_identical":true,
        "implementation_installed":false,"synthetic_only":true})
    );
}

#[test]
fn directory_capacity_measurement_matches_validation_and_grants_no_authority() {
    let mut run = Vec::with_capacity(8);
    for n in 0..3i64 {
        let mut first = Vec::with_capacity(12);
        first.extend([n, 1]);
        let mut last = Vec::with_capacity(9);
        last.extend([n, 1]);
        run.push(wire::PageRef {
            object: wire::ObjectRef {
                digest: *blake3::hash(&n.to_le_bytes()).as_bytes(),
                bytes: 128,
            },
            records: 1,
            first: StoredWork::Route(first),
            last: StoredWork::Route(last),
        });
    }
    let mut runs = Vec::with_capacity(10);
    runs.push(run);
    let compare = |runs: &Vec<Vec<wire::PageRef>>| {
        let measured = rustred::persistence::finite_work::directory_payload_bytes(runs).unwrap();
        let validated = rustred::persistence::finite_work::validate_directory(
            &binding(),
            runs,
            limits().directory_limits(),
        )
        .unwrap();
        assert_eq!(measured, validated.payload_bytes);
        measured
    };
    let original = compare(&runs);
    let encoded = wire::encode(b"capacity-test", &runs, wire::MAX_FRAME).unwrap();
    let decoded: Vec<Vec<wire::PageRef>> =
        wire::decode(b"capacity-test", &encoded, wire::MAX_FRAME).unwrap();
    assert!(compare(&decoded) < original);
    // Measuring bytes is not authorization to consume malformed references.
    let duplicate = runs[0][0].clone();
    runs[0].push(duplicate);
    assert!(rustred::persistence::finite_work::directory_payload_bytes(&runs).is_ok());
    assert!(
        rustred::persistence::finite_work::validate_directory(
            &binding(),
            &runs,
            limits().directory_limits()
        )
        .is_err()
    );
}

#[test]
fn validated_generation_reuses_metadata_with_original_or_disabled_page_cache() {
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    let p = limits();
    let mut writer = StoreWriter::create(&path, binding(), p).unwrap();
    let first = commit(&mut writer, vec![key(1), key(2)], 2, 1);
    for cache in [
        (p.max_cached_pages, p.max_cached_page_bytes),
        (0, 0),
        (10, 1024 * 1024),
    ] {
        let reader = first
            .reader(StorageLimits {
                max_cached_pages: cache.0,
                max_cached_page_bytes: cache.1,
                ..p
            })
            .unwrap();
        assert!(reader.statistics().unwrap().metadata_validation_reused);
        assert_eq!(reader.directory().generation(), first.digest());
        assert_eq!(reader.lookup(&key(1)).unwrap(), Lookup::Present);
    }
    let second = commit(&mut writer, vec![key(3)], 3, 2);
    assert_ne!(first.digest(), second.digest());
    assert_eq!(
        first.reader(p).unwrap().lookup(&key(3)).unwrap(),
        Lookup::Absent
    );
    assert_eq!(
        second.reader(p).unwrap().lookup(&key(3)).unwrap(),
        Lookup::Present
    );
    drop(writer);
    let cold = ValidatedGeneration::open_generated(&path, &binding(), p).unwrap();
    assert!(
        cold.reader(p)
            .unwrap()
            .statistics()
            .unwrap()
            .metadata_validation_reused
    );
    assert_eq!(cold.digest(), second.digest());
    let decoded = cold.reader(p).unwrap().directory().statistics();
    let retained = second.reader(p).unwrap().directory().statistics();
    assert_eq!(decoded.records, retained.records);
    assert_eq!(decoded.pages, retained.pages);
    assert_eq!(decoded.referenced_bytes, retained.referenced_bytes);
    // Keeping an already readable constructed manifest can retain Vec slack;
    // each representation is charged for its own real capacities.
    assert_eq!(
        decoded.payload_bytes,
        rustred::persistence::finite_work::directory_payload_bytes(&cold.manifest.runs).unwrap()
    );
    assert_eq!(
        retained.payload_bytes,
        rustred::persistence::finite_work::directory_payload_bytes(&second.manifest.runs).unwrap()
    );
}

#[test]
fn metadata_memo_does_not_promote_stricter_structural_or_lookup_grants() {
    let scratch = Scratch::new();
    let mut writer = StoreWriter::create(&scratch.0.join("store"), binding(), limits()).unwrap();
    let mut s = state(2, 1);
    s.census.selections.push(SelectionRecord {
        owner: vec![true, true],
        batch: 0,
        baseline: 0,
        selected: 0,
        multiplicity: 1,
    });
    let generation = writer
        .commit_generated(vec![key(1), key(2)], s, &AtomicBool::new(false))
        .unwrap()
        .generation;
    for field in [
        "arity",
        "runs",
        "pages",
        "directory",
        "page-records",
        "page-bytes",
        "state",
        "scope",
        "census",
        "disk",
        "transient",
    ] {
        let mut p = limits();
        match field {
            "arity" => p.max_arity = 1,
            "runs" => p.max_runs = 0,
            "pages" => p.max_pages = 0,
            "directory" => p.max_directory_bytes = 1,
            "page-records" => p.max_page_records = 1,
            "page-bytes" => p.max_page_bytes = 1,
            "state" => p.max_state_bytes = 1,
            "scope" => p.max_scope_rows = 0,
            "census" => p.max_census_records = 0,
            "disk" => p.max_disk_bytes = 1,
            "transient" => p.max_transient_bytes = 1,
            _ => unreachable!(),
        }
        assert!(generation.reader(p).is_err(), "{field}");
    }
    for field in ["raw", "lookup-pages", "lookup-bytes", "larger-directory"] {
        let mut p = limits();
        match field {
            "raw" => p.raw.max_runs += 1,
            "lookup-pages" => p.max_lookup_pages = 0,
            "lookup-bytes" => p.max_lookup_bytes = 0,
            "larger-directory" => p.max_directory_bytes += 1,
            _ => unreachable!(),
        }
        let reader = generation.reader(p).unwrap();
        assert!(
            !reader.statistics().unwrap().metadata_validation_reused,
            "{field}"
        );
        if field.starts_with("lookup-") {
            assert_eq!(
                reader.lookup(&key(1)).unwrap_err().kind,
                ErrorKind::Resource
            );
        } else {
            assert_eq!(reader.lookup(&key(1)).unwrap(), Lookup::Present);
        }
    }
}

#[test]
fn metadata_memo_does_not_survive_untrusted_cold_reads_or_mask_page_corruption() {
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    let p = StorageLimits {
        max_cached_pages: 0,
        max_cached_page_bytes: 0,
        ..limits()
    };
    let mut writer = StoreWriter::create(&path, binding(), p).unwrap();
    let generation = commit(&mut writer, vec![key(1)], 1, 1);
    drop(writer);
    let manifest = path
        .join("generations")
        .join(wire::name(&generation.digest(), "rrgen"));
    let original = fs::read(&manifest).unwrap();
    let mut corrupted = original.clone();
    corrupted[0] ^= 1;
    fs::write(&manifest, corrupted).unwrap();
    assert!(ValidatedGeneration::open_generated(&path, &binding(), p).is_err());
    // The separately captured generation is immutable; its already checked
    // directory is not reinterpreted from the now corrupt on-disk manifest.
    let reader = generation.reader(p).unwrap();
    assert!(reader.statistics().unwrap().metadata_validation_reused);
    assert_eq!(reader.lookup(&key(1)).unwrap(), Lookup::Present);
    fs::write(&manifest, original).unwrap();
    let page = page_path(&generation, 0);
    let mut bytes = fs::read(&page).unwrap();
    bytes[0] ^= 1;
    fs::write(&page, bytes).unwrap();
    assert_eq!(reader.lookup(&key(1)).unwrap_err().kind, ErrorKind::Corrupt);
    assert_eq!(reader.statistics().unwrap().absent, 0);
}

#[test]
#[ignore = "release-only synthetic metadata benchmark; no native closure claim"]
fn large_history_validated_reader_metadata_benchmark() {
    let runs: usize = std::env::var("RUSTRED_METADATA_BENCH_RUNS")
        .unwrap_or_else(|_| "149442".into())
        .parse()
        .unwrap();
    let repetitions: usize = std::env::var("RUSTRED_METADATA_BENCH_REPETITIONS")
        .unwrap_or_else(|_| "10".into())
        .parse()
        .unwrap();
    let scratch = Scratch::new();
    let p = StorageLimits {
        max_runs: usize::MAX,
        max_pages: usize::MAX,
        max_directory_bytes: 512 * 1024 * 1024,
        max_manifest_bytes: 64 * 1024 * 1024,
        max_disk_bytes: u64::MAX,
        max_cached_pages: 0,
        max_cached_page_bytes: 0,
        ..limits()
    };
    let runs: Vec<_> = (0..runs)
        .map(|n| {
            let digest = *blake3::hash(&(n as u64).to_le_bytes()).as_bytes();
            vec![wire::PageRef {
                object: wire::ObjectRef { digest, bytes: 128 },
                records: 1,
                first: key(n as i64),
                last: key(n as i64),
            }]
        })
        .collect();
    let state = state(runs.len() as u64, 1);
    let state_bytes = wire::encode(
        wire::STATE_MAGIC,
        &wire::StateFile {
            execution: binding(),
            generation: 2,
            state: state.clone(),
        },
        p.max_state_bytes,
    )
    .unwrap();
    let manifest = wire::Manifest {
        execution: binding(),
        generation: 2,
        previous: Some([3; 32]),
        state: wire::ObjectRef {
            digest: *blake3::hash(&state_bytes).as_bytes(),
            bytes: state_bytes.len() as u64,
        },
        scope_digest: wire::scope_digest(&state.scope, p.max_state_bytes).unwrap(),
        runs: Arc::new(runs),
        raw: Arc::new(RawCensusDirectory::default()),
    };
    let encoded = wire::encode(wire::MANIFEST_MAGIC, &manifest, wire::MAX_FRAME).unwrap();
    let digest = *blake3::hash(&encoded).as_bytes();
    let directory = wire::validate_manifest_directory(&manifest, digest, &binding(), p).unwrap();
    wire::validate_state(&state, &binding(), directory.statistics().records, p).unwrap();
    let generation = ValidatedGeneration {
        directory: scratch.0.clone(),
        digest,
        manifest: Arc::new(manifest),
        state: Arc::new(state),
        validation: Arc::new(ValidatedMetadata {
            grants: p.validation_key(),
            directory,
        }),
    };
    for cached in [false, true] {
        let mut selected = p;
        if !cached {
            selected.max_lookup_bytes += 1;
        } // Force identical full validation work.
        let started = std::time::Instant::now();
        for _ in 0..repetitions {
            let reader = std::hint::black_box(generation.reader(selected).unwrap());
            assert_eq!(
                reader.statistics().unwrap().metadata_validation_reused,
                cached
            );
            assert_eq!(
                reader.directory().statistics().records,
                generation.state().completed_work
            );
        }
        eprintln!(
            "{}",
            serde_json::json!({"benchmark":"validated_reader_metadata", "cached":cached,
            "runs":generation.manifest.runs.len(),"repetitions":repetitions,
            "seconds":started.elapsed().as_secs_f64(),"synthetic_only":true})
        );
    }
}
