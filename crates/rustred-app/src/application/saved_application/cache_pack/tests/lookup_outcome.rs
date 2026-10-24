use super::*;

#[test]
fn typed_lookup_budget_is_distinct_from_hit_and_verified_absence() {
    let scratch = scratch();
    let path = scratch.0.join("typed-outcome");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    for endpoint in [10, 11, 12] {
        writer
            .append_completed(&[record(0), record(endpoint)], &context())
            .unwrap();
    }
    writer.checkpoint().unwrap();
    make_legacy(&path);
    let tight = SavedApplicationCachePackLimits {
        diagnostics_enabled: true,
        max_lookup_chunks: 1,
        ..limits
    };
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), tight).unwrap();
    assert_eq!(
        reader.lookup_outcome(&key(5), &context()).unwrap(),
        SavedApplicationCacheLookupOutcome::WorkBudgetExhausted
    );
    let refused = reader.statistics();
    assert_eq!(refused.chunks_read, 1);
    assert_eq!(refused.lookup_limit_refusals, 1);
    assert_eq!(refused.misses, 0);
    assert_eq!(refused.hits, 0);
    assert_eq!(
        reader.lookup_outcome(&key(12), &context()).unwrap(),
        SavedApplicationCacheLookupOutcome::Hit(record(12))
    );
    assert_eq!(
        reader.lookup_outcome(&key(20), &context()).unwrap(),
        SavedApplicationCacheLookupOutcome::Absent
    );
    assert_eq!(reader.statistics().misses, 1);
    assert_eq!(
        reader.lookup(&key(5), &context()).unwrap_err().kind(),
        crate::AppErrorKind::Limit
    );
    assert_eq!(
        reader.statistics().lookup_limit_refusals,
        2,
        "strict wrapper retains refusal accounting"
    );
}

#[test]
fn typed_lookup_stops_before_ungranted_io_and_preserves_read_decode_grants() {
    let scratch = scratch();
    let path = scratch.0.join("typed-zero-grant");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    writer.append_completed(&[record(1)], &context()).unwrap();
    writer.checkpoint().unwrap();
    for tight in [
        SavedApplicationCachePackLimits {
            diagnostics_enabled: true,
            max_lookup_chunks: 0,
            ..limits
        },
        SavedApplicationCachePackLimits {
            diagnostics_enabled: true,
            max_lookup_encoded_bytes: 0,
            ..limits
        },
        SavedApplicationCachePackLimits {
            diagnostics_enabled: true,
            max_lookup_decoded_bytes: 0,
            ..limits
        },
    ] {
        let mut reader =
            SavedApplicationCachePack::open_generated(&path, &binding(1), tight).unwrap();
        assert_eq!(
            reader.lookup_outcome(&key(1), &context()).unwrap(),
            SavedApplicationCacheLookupOutcome::WorkBudgetExhausted
        );
        let stats = reader.statistics();
        assert_eq!(stats.chunks_read, 0);
        assert_eq!(stats.chunk_bytes_read, 0);
        assert_eq!(stats.decoded_chunk_bytes, 0);
        assert_eq!(stats.lookup_encoded_bytes_charged, 0);
        assert_eq!(stats.lookup_decoded_bytes_charged, 0);
        assert_eq!(stats.misses, 0);
    }
}

#[test]
fn ordinary_reads_preserve_exact_results_with_inactive_count_and_traffic_templates() {
    let scratch = scratch();
    let path = scratch.0.join("inactive-pack-diagnostics");
    let mut limits = SavedApplicationCachePackLimits {
        max_runs: 0,
        max_chunks: 0,
        max_lookup_chunks: 0,
        max_lookup_encoded_bytes: 0,
        max_lookup_decoded_bytes: 0,
        ..Default::default()
    };
    limits.chunk.max_records = 0;
    assert!(!limits.diagnostics_enabled);
    assert!(!limits.chunk.diagnostics_enabled);
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    for endpoint in [10, 11] {
        writer
            .append_completed(&[record(0), record(endpoint)], &context())
            .unwrap();
    }
    writer.checkpoint().unwrap();
    drop(writer);
    let mut ordinary =
        SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    assert_eq!(ordinary.statistics().runs, 2);
    assert_eq!(ordinary.statistics().chunks, 2);
    assert_eq!(ordinary.statistics().records, 4);
    assert_eq!(
        ordinary.lookup_outcome(&key(10), &context()).unwrap(),
        SavedApplicationCacheLookupOutcome::Hit(record(10))
    );
    assert_eq!(ordinary.statistics().lookup_limit_refusals, 0);
    assert_eq!(ordinary.statistics().chunks_read, 1);
    let mut diagnostic = SavedApplicationCachePack::open_generated(
        &path,
        &binding(1),
        SavedApplicationCachePackLimits {
            diagnostics_enabled: true,
            max_lookup_chunks: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        diagnostic.lookup_outcome(&key(10), &context()).unwrap(),
        SavedApplicationCacheLookupOutcome::WorkBudgetExhausted
    );
    assert_eq!(diagnostic.statistics().chunks_read, 0);
    assert_eq!(diagnostic.statistics().lookup_limit_refusals, 1);
}

#[test]
fn typed_lookup_never_turns_context_key_binding_corruption_or_io_errors_into_fallback() {
    let scratch = scratch();
    let path = scratch.0.join("typed-errors");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    writer.append_completed(&[record(1)], &context()).unwrap();
    writer.checkpoint().unwrap();
    drop(writer);
    assert!(SavedApplicationCachePack::open_generated(&path, &binding(2), limits).is_err());
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    let wrong_key = ApplicationCacheKey::route(&IntegralKey::try_new([1, 2]).unwrap());
    assert!(reader.lookup_outcome(&wrong_key, &context()).is_err());
    assert_eq!(reader.statistics().chunks_read, 0);
    assert!(
        reader
            .lookup_outcome(
                &key(1),
                &CoefficientContext::try_new(["wrong_context"]).unwrap()
            )
            .is_err()
    );
    let manifest = manifest(&path);
    let chunk_path = path.join("chunks").join(manifest.runs[0][0].file_name());
    let mut bytes = fs::read(&chunk_path).unwrap();
    bytes[0] ^= 1;
    fs::write(&chunk_path, bytes).unwrap();
    assert!(reader.lookup_outcome(&key(1), &context()).is_err());
    fs::remove_file(&chunk_path).unwrap();
    assert!(reader.lookup_outcome(&key(1), &context()).is_err());
    assert_eq!(reader.statistics().lookup_limit_refusals, 0);
    assert_eq!(reader.statistics().hits, 0);
    assert_eq!(reader.statistics().misses, 0);
}
