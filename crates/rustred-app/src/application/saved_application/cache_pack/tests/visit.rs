use super::*;
use crate::AppError;

#[test]
fn completed_scan_reads_each_chunk_once_and_visits_latest_records_only() {
    let scratch = scratch();
    let path = scratch.0.join("visitor");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    writer
        .append_completed(&[record(1), record(2), record(3)], &context())
        .unwrap();
    let mut replacement = record(2);
    replacement.terms.clear();
    writer
        .append_completed(&[replacement.clone(), record(4)], &context())
        .unwrap();
    writer.checkpoint().unwrap();
    let mut captured =
        SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    writer.append_completed(&[record(5)], &context()).unwrap();
    writer.checkpoint().unwrap();
    writer.append_completed(&[record(6)], &context()).unwrap();
    let mut result = BTreeMap::new();
    assert_eq!(
        captured
            .visit_completed_records(&context(), |record| {
                assert!(result.insert(record.key.clone(), record).is_none());
                Ok(())
            })
            .unwrap(),
        4
    );
    assert_eq!(
        result,
        BTreeMap::from([
            (key(1), record(1)),
            (key(2), replacement),
            (key(3), record(3)),
            (key(4), record(4))
        ])
    );
    assert_eq!(captured.statistics().chunks_read, 2);
    assert_eq!(captured.statistics().hits, 0);
    assert_eq!(captured.statistics().misses, 0);
}

#[test]
fn completed_scan_propagates_callback_corruption_and_collection_limits() {
    let scratch = scratch();
    let path = scratch.0.join("visitor-bounds");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    writer
        .append_completed(&[record(1), record(2)], &context())
        .unwrap();
    writer.checkpoint().unwrap();
    drop(writer);
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    let mut callbacks = 0;
    let sentinel = AppError::limit("consumer output budget exhausted");
    let error = reader
        .visit_completed_records(&context(), |_| {
            callbacks += 1;
            Err(sentinel.clone())
        })
        .unwrap_err();
    assert_eq!(error, sentinel);
    assert_eq!(callbacks, 1);
    let mut tight = limits;
    tight.max_directory_owned_bytes = reader.statistics().directory_owned_bytes;
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), tight).unwrap();
    assert!(
        reader
            .visit_completed_records(&context(), |_| panic!("index refusal precedes callback"))
            .is_err()
    );
    // Term census is stable across native decoding; allocation capacities may
    // conservatively make decoded byte census larger than the source value.
    let (one_record_terms, _) = record(1).coefficient_census().unwrap();
    tight = limits;
    tight.chunk.diagnostics_enabled = true;
    tight.chunk.max_decoded_coefficient_terms = one_record_terms;
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), tight).unwrap();
    let mut callbacks = 0;
    assert!(
        reader
            .visit_completed_records(&context(), |_| {
                callbacks += 1;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(callbacks, 1);
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    let chunk = &manifest(&path).runs[0][0];
    let chunk_path = path.join("chunks").join(chunk.file_name());
    let mut bytes = fs::read(&chunk_path).unwrap();
    bytes[20] ^= 1;
    fs::write(chunk_path, bytes).unwrap();
    assert!(
        reader
            .visit_completed_records(&context(), |_| panic!("corruption precedes callback"))
            .is_err()
    );
}
