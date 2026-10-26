use super::*;
use crate::AppError;

#[test]
fn selected_scan_deduplicates_preserves_guards_and_uses_captured_newest_records() {
    let scratch = scratch();
    let path = scratch.0.join("selected-newest");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    writer
        .append_completed(&[record(1), record(2), record(3)], &context())
        .unwrap();
    let mut replacement = record(2);
    replacement.terms.clear();
    replacement.nonzero_conditions.reverse();
    replacement
        .nonzero_conditions
        .insert(0, context().integer(7));
    writer
        .append_completed(&[replacement.clone()], &context())
        .unwrap();
    writer.checkpoint().unwrap();
    let mut captured =
        SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    let mut later = record(3);
    later.terms.clear();
    writer.append_completed(&[later], &context()).unwrap();
    writer.checkpoint().unwrap();
    let mut output = BTreeMap::new();
    let count = captured
        .visit_selected_completed_records(&[key(3), key(2), key(3)], &context(), |record| {
            assert!(output.insert(record.key.clone(), record).is_none());
            Ok(())
        })
        .unwrap();
    assert_eq!(count, 2);
    assert_eq!(
        output,
        BTreeMap::from([(key(2), replacement), (key(3), record(3))])
    );
    assert_eq!(captured.statistics().chunks_read, 2);
    assert_eq!(captured.statistics().hits, 0);
    assert_eq!(captured.statistics().misses, 0);
}

#[test]
fn selected_scan_keeps_route_and_apply_owner_identities_distinct() {
    let scratch = scratch();
    let path = scratch.0.join("selected-stages");
    let limits = SavedApplicationCachePackLimits::default();
    let mut positive = record(1);
    positive.key = ApplicationCacheKey::apply(vec![true], &IntegralKey::try_new([1]).unwrap());
    let mut negative = record(-1);
    negative.key = ApplicationCacheKey::apply(vec![false], &IntegralKey::try_new([-1]).unwrap());
    let mut records = vec![record(1), positive.clone(), negative.clone()];
    records.sort_by(|left, right| left.key.cmp(&right.key));
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    writer.append_completed(&records, &context()).unwrap();
    writer.checkpoint().unwrap();
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    let mut output = Vec::new();
    assert_eq!(
        reader
            .visit_selected_completed_records(
                &[positive.key.clone(), negative.key.clone()],
                &context(),
                |record| {
                    output.push(record);
                    Ok(())
                }
            )
            .unwrap(),
        2
    );
    assert_eq!(output, vec![negative, positive]);
    assert_eq!(reader.statistics().chunks_read, 1);
}

#[test]
fn selected_scan_skips_unrelated_corrupt_chunks_but_rejects_selected_corruption() {
    let scratch = scratch();
    let path = scratch.0.join("selected-corruption");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    writer.append_completed(&[record(1)], &context()).unwrap();
    writer.append_completed(&[record(100)], &context()).unwrap();
    writer.checkpoint().unwrap();
    drop(writer);
    let bad = &manifest(&path).runs[0][1];
    let bad_path = path.join("chunks").join(bad.file_name());
    let mut bytes = fs::read(&bad_path).unwrap();
    bytes[20] ^= 1;
    fs::write(bad_path, bytes).unwrap();
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    assert_eq!(
        reader
            .visit_selected_completed_records(&[key(1)], &context(), |selected| {
                assert_eq!(selected, record(1));
                Ok(())
            })
            .unwrap(),
        1
    );
    assert_eq!(reader.statistics().chunks_read, 1);
    assert!(
        reader
            .visit_selected_completed_records(&[key(100)], &context(), |_| {
                panic!("selected corruption precedes callback")
            })
            .is_err()
    );
}

#[test]
fn selected_scan_reports_missing_and_invalid_keys_without_lookup_fallback() {
    let scratch = scratch();
    let path = scratch.0.join("selected-missing");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    writer.append_completed(&[record(1)], &context()).unwrap();
    writer.checkpoint().unwrap();
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    let error = reader
        .visit_selected_completed_records(&[key(2), key(2)], &context(), |_| {
            panic!("missing key is never emitted")
        })
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("1 selected cache records are absent")
    );
    let invalid = ApplicationCacheKey::route(&IntegralKey::try_new([1, 2]).unwrap());
    assert!(
        reader
            .visit_selected_completed_records(&[invalid], &context(), |_| {
                panic!("invalid key is never emitted")
            })
            .is_err()
    );
    assert_eq!(reader.statistics().chunks_read, 0);
    assert_eq!(
        reader
            .visit_selected_completed_records(&[], &context(), |_| {
                panic!("empty selection is never emitted")
            })
            .unwrap(),
        0
    );
}

#[test]
fn selected_scan_reads_repeated_legacy_candidate_payload_once() {
    let scratch = scratch();
    let path = scratch.0.join("selected-repeated-chunk");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    writer
        .append_completed(&[record(1), record(3)], &context())
        .unwrap();
    writer
        .append_completed(&[record(1), record(3)], &context())
        .unwrap();
    writer.checkpoint().unwrap();
    drop(writer);
    assert_eq!(manifest(&path).runs.len(), 2);
    assert_eq!(physical_chunks(&path).0, 1);
    make_legacy(&path);
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    assert!(
        reader
            .visit_selected_completed_records(&[key(2)], &context(), |_| {
                panic!("in-range key absent from both copies is never emitted")
            })
            .is_err()
    );
    assert_eq!(reader.statistics().chunks_read, 1);
}

#[test]
fn selected_scan_decodes_only_requested_coefficients_under_output_grants() {
    let scratch = scratch();
    let path = scratch.0.join("selected-coefficients");
    let limits = SavedApplicationCachePackLimits::default();
    let selected = record(0);
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    writer
        .append_completed(&[selected.clone(), record(1)], &context())
        .unwrap();
    writer.checkpoint().unwrap();
    let mut tight = limits;
    tight.chunk.diagnostics_enabled = true;
    tight.chunk.max_decoded_coefficient_terms = selected.coefficient_census().unwrap().0;
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), tight).unwrap();
    assert_eq!(
        reader
            .visit_selected_completed_records(&[selected.key.clone()], &context(), |record| {
                assert_eq!(record, selected);
                Ok(())
            })
            .unwrap(),
        1
    );
    assert_eq!(reader.statistics().chunks_read, 1);
    let mut full = SavedApplicationCachePack::open_generated(&path, &binding(1), tight).unwrap();
    assert!(
        full.visit_completed_records(&context(), |_| Ok(()))
            .is_err()
    );
}

#[test]
fn selected_scan_preserves_callback_errors_and_admits_key_memory_first() {
    let scratch = scratch();
    let path = scratch.0.join("selected-resources");
    let limits = SavedApplicationCachePackLimits::default();
    let mut writer = SavedApplicationCacheWriter::create(&path, binding(1), limits).unwrap();
    writer
        .append_completed(&[record(1), record(2)], &context())
        .unwrap();
    writer.checkpoint().unwrap();
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), limits).unwrap();
    let sentinel = AppError::limit("selected consumer budget exhausted");
    let mut callbacks = 0;
    assert_eq!(
        reader
            .visit_selected_completed_records(&[key(1), key(2)], &context(), |_| {
                callbacks += 1;
                Err(sentinel.clone())
            })
            .unwrap_err(),
        sentinel
    );
    assert_eq!(callbacks, 1);
    let mut tight = limits;
    tight.max_directory_owned_bytes = reader.statistics().directory_owned_bytes;
    let mut reader = SavedApplicationCachePack::open_generated(&path, &binding(1), tight).unwrap();
    assert!(
        reader
            .visit_selected_completed_records(&[key(1)], &context(), |_| {
                panic!("key memory refusal precedes callback")
            })
            .is_err()
    );
    assert_eq!(reader.statistics().chunks_read, 0);
}
