use super::*;
use crate::application::saved_application::tests::{Scratch, prepared_fixture};

fn key(power: i64) -> IntegralKey {
    IntegralKey::try_new([power]).unwrap()
}

fn fixture() -> (
    Scratch,
    PreparedSavedApplication<1>,
    ApplicationCacheBinding,
    Vec<ApplicationCacheRecord>,
) {
    let scratch = Scratch::new();
    let mut prepared = prepared_fixture(&scratch.0.join("source"));
    let binding = prepared.cache_binding([91; 32]);
    for power in [2, 4] {
        let one = prepared.family().coefficient_context().one();
        prepared
            .reduce_routed_linear_combination([(key(power), one)])
            .unwrap();
    }
    let mut records = Vec::new();
    for power in [2, 4] {
        for operational in [
            ApplicationCacheKey::route(&key(power)),
            ApplicationCacheKey::apply(vec![true], &key(power)),
        ] {
            let record = prepared
                .completed_cache_record(&binding, &operational)
                .unwrap()
                .expect("completed toy reduction must expose its operational record");
            prepared
                .validate_completed_cache_record(&binding, &record)
                .unwrap();
            records.push(record);
        }
    }
    records.sort_unstable_by(|left, right| left.key.cmp(&right.key));
    (scratch, prepared, binding, records)
}

#[test]
fn selected_file_exports_exact_subset_newest_guards_and_one_existing_table() {
    let (scratch, prepared, binding, records) = fixture();
    let directory = scratch.0.join("pack");
    let context = prepared.family().coefficient_context();
    let mut writer =
        SavedApplicationCacheWriter::create(&directory, binding.clone(), Default::default())
            .unwrap();
    writer.append_completed(&records, context).unwrap();
    let route = ApplicationCacheKey::route(&key(4));
    let apply = ApplicationCacheKey::apply(vec![true], &key(2));
    let mut newest = records
        .iter()
        .find(|record| record.key == route)
        .unwrap()
        .clone();
    // Extra sufficient guards change exact provenance without changing the
    // reduction identity on their domain. Duplicates and order must survive.
    let guard = context
        .try_sub(
            &context.parameter("d").unwrap(),
            &context.integer(11),
            Default::default(),
        )
        .unwrap();
    newest
        .nonzero_conditions
        .extend([guard.clone(), context.integer(-2), guard]);
    writer.append_completed(&[newest.clone()], context).unwrap();
    writer.checkpoint().unwrap();
    drop(writer);
    let mut reader =
        SavedApplicationCachePack::open_generated(&directory, &binding, Default::default())
            .unwrap();
    let before = prepared.cache_statistics();
    let limits = SavedApplicationCacheFileLimits {
        max_compressed_bytes: 95_000_000,
        ..Default::default()
    };
    let (bytes, identity) = prepared
        .encode_selected_completed_cache(
            &binding,
            &mut reader,
            &[apply.clone(), route.clone(), route.clone()],
            limits,
        )
        .unwrap();
    assert_eq!(prepared.cache_statistics(), before);
    assert_eq!(reader.statistics().chunks_read, 2);
    assert_eq!(identity.compressed_bytes, bytes.len() as u64);
    assert!(bytes.len() <= 95_000_000);
    // This is the existing, unchanged Native419 decoder and file format.
    let mut decoded =
        SavedApplicationCacheFile::decode_generated(&bytes, &binding, &identity, limits).unwrap();
    assert_eq!(decoded.statistics().unwrap().records, 2);
    assert_eq!(decoded.lookup(&route, context).unwrap(), Some(newest));
    let expected_apply = records
        .iter()
        .find(|record| record.key == apply)
        .unwrap()
        .clone();
    assert_eq!(
        decoded.lookup(&apply, context).unwrap(),
        Some(expected_apply)
    );
    assert_eq!(
        decoded
            .lookup(&ApplicationCacheKey::route(&key(2)), context)
            .unwrap(),
        None
    );
    for selected in decoded.keys() {
        let mut fork = decoded.fork_reader();
        prepared
            .validate_completed_cache_record(
                &binding,
                &fork.lookup(selected, context).unwrap().unwrap(),
            )
            .unwrap();
    }
    // No extra container or concatenated gzip members were introduced: the
    // original bounded one-table decoder accepts the complete output directly.
    assert_eq!(decoded.binding(), &binding);
}

#[test]
fn selected_file_rejects_missing_binding_and_output_allowance_without_reduction() {
    let (scratch, prepared, binding, records) = fixture();
    let directory = scratch.0.join("pack");
    let context = prepared.family().coefficient_context();
    let mut writer =
        SavedApplicationCacheWriter::create(&directory, binding.clone(), Default::default())
            .unwrap();
    writer.append_completed(&records, context).unwrap();
    writer.checkpoint().unwrap();
    drop(writer);
    let before = prepared.cache_statistics();
    let mut reader =
        SavedApplicationCachePack::open_generated(&directory, &binding, Default::default())
            .unwrap();
    assert!(
        prepared
            .encode_selected_completed_cache(
                &binding,
                &mut reader,
                &[ApplicationCacheKey::route(&key(99))],
                Default::default(),
            )
            .is_err()
    );
    let foreign = ApplicationCacheBinding {
        artifact_digest: [92; 32],
        ..binding.clone()
    };
    assert!(
        prepared
            .encode_selected_completed_cache(&foreign, &mut reader, &[], Default::default())
            .is_err()
    );
    let tiny = SavedApplicationCacheFileLimits {
        max_compressed_bytes: 1,
        ..Default::default()
    };
    assert!(
        prepared
            .encode_selected_completed_cache(&binding, &mut reader, &[records[0].key.clone()], tiny)
            .is_err()
    );
    assert_eq!(prepared.cache_statistics(), before);
}

#[test]
fn selected_file_owner_rejects_unresolved_terms_and_unsupported_operational_stages() {
    let (scratch, prepared, binding, records) = fixture();
    let context = prepared.family().coefficient_context();
    let mut unresolved = records[0].clone();
    unresolved.terms = BTreeMap::from([(key(99), context.one())]);
    let mut unsupported = records[0].clone();
    unsupported.key = ApplicationCacheKey::guarded_root(&key(2));
    for (index, record) in [unresolved, unsupported].into_iter().enumerate() {
        let directory = scratch.0.join(format!("invalid-{index}"));
        let mut writer =
            SavedApplicationCacheWriter::create(&directory, binding.clone(), Default::default())
                .unwrap();
        writer
            .append_completed(std::slice::from_ref(&record), context)
            .unwrap();
        writer.checkpoint().unwrap();
        drop(writer);
        let mut reader =
            SavedApplicationCachePack::open_generated(&directory, &binding, Default::default())
                .unwrap();
        assert!(
            prepared
                .encode_selected_completed_cache(
                    &binding,
                    &mut reader,
                    &[record.key],
                    Default::default(),
                )
                .is_err()
        );
    }
}
