use super::*;

fn key(values: &[i64]) -> IntegralKey {
    IntegralKey::try_new(values.iter().copied()).unwrap()
}
fn binding() -> ApplicationCacheBinding {
    ApplicationCacheBinding {
        artifact_digest: [17; 32],
        family_fingerprint: "cache-test-family".into(),
        physical_arity: 2,
        layer: "raw-declared-unit-mass-v1".into(),
    }
}
fn fixture() -> (CoefficientContext, Vec<ApplicationCacheRecord>) {
    let context = CoefficientContext::new(["cache_test_d"]);
    let d = context.parameter("cache_test_d").unwrap();
    let guard = context
        .try_sub(&d, &context.integer(4), Default::default())
        .unwrap();
    let value = context
        .try_div(&context.one(), &guard, Default::default())
        .unwrap();
    let records = vec![
        ApplicationCacheRecord {
            key: ApplicationCacheKey::route(&key(&[-2, 1])),
            terms: BTreeMap::new(),
            nonzero_conditions: vec![guard.clone(), context.integer(-2), guard.clone()],
        },
        ApplicationCacheRecord {
            key: ApplicationCacheKey::route(&key(&[2, 1])),
            terms: BTreeMap::from([(key(&[1, 1]), value)]),
            nonzero_conditions: vec![guard],
        },
    ];
    (context, records)
}

#[test]
fn selected_native_records_preserve_empty_outputs_poles_and_guard_order() {
    let (context, records) = fixture();
    let limits = ApplicationCacheIoLimits::default();
    let encoded = encode_application_cache_chunk(&binding(), &records, &context, limits).unwrap();
    let inspected = ApplicationCacheChunk::inspect(&encoded, &binding(), limits).unwrap();
    assert_eq!(inspected.len(), 2);
    assert_eq!(inspected.first_key(), Some(&records[0].key));
    assert_eq!(inspected.last_key(), Some(&records[1].key));
    for record in &records {
        assert_eq!(
            inspected
                .decode_record(&record.key, &context)
                .unwrap()
                .as_ref(),
            Some(record)
        );
    }
    assert!(
        inspected
            .decode_record(&ApplicationCacheKey::route(&key(&[7, 1])), &context)
            .unwrap()
            .is_none()
    );
}

#[test]
fn wrong_binding_context_keys_and_truncation_are_not_cache_hits() {
    let (context, records) = fixture();
    let limits = ApplicationCacheIoLimits::default();
    let encoded = encode_application_cache_chunk(&binding(), &records, &context, limits).unwrap();
    for altered in [
        ApplicationCacheBinding {
            artifact_digest: [18; 32],
            ..binding()
        },
        ApplicationCacheBinding {
            family_fingerprint: "other".into(),
            ..binding()
        },
        ApplicationCacheBinding {
            layer: "post-terminal-map".into(),
            ..binding()
        },
        ApplicationCacheBinding {
            physical_arity: 3,
            ..binding()
        },
    ] {
        assert!(ApplicationCacheChunk::inspect(&encoded, &altered, limits).is_err());
    }
    let inspected = ApplicationCacheChunk::inspect(&encoded, &binding(), limits).unwrap();
    let foreign = CoefficientContext::new(["foreign_cache_d"]);
    assert!(inspected.decode_record(&records[1].key, &foreign).is_err());
    for end in [0, 16, encoded.len() - 1] {
        assert!(ApplicationCacheChunk::inspect(&encoded[..end], &binding(), limits).is_err());
    }
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(ApplicationCacheChunk::inspect(&trailing, &binding(), limits).is_err());
    let bad = ApplicationCacheRecord {
        key: ApplicationCacheKey::apply(vec![false, true], &key(&[1, 1])),
        ..records[1].clone()
    };
    assert!(encode_application_cache_chunk(&binding(), &[bad], &context, limits).is_err());
    assert!(
        encode_application_cache_chunk(
            &binding(),
            &[records[1].clone(), records[0].clone()],
            &context,
            limits
        )
        .is_err()
    );
}

#[test]
fn current_decode_census_and_collection_limits_are_enforced() {
    let (context, records) = fixture();
    let limits = ApplicationCacheIoLimits::default();
    let encoded = encode_application_cache_chunk(&binding(), &records, &context, limits).unwrap();
    let tight = ApplicationCacheIoLimits {
        max_records: 1,
        ..limits
    };
    assert!(ApplicationCacheChunk::inspect(&encoded, &binding(), tight).is_err());
    let tight = ApplicationCacheIoLimits {
        max_decoded_coefficient_bytes: 0,
        ..limits
    };
    let inspected = ApplicationCacheChunk::inspect(&encoded, &binding(), tight).unwrap();
    assert!(inspected.decode_record(&records[1].key, &context).is_err());
    let tight = ApplicationCacheIoLimits {
        max_guards_per_record: 2,
        ..limits
    };
    assert!(ApplicationCacheChunk::inspect(&encoded, &binding(), tight).is_err());
    let mut zero_guard = records[0].clone();
    zero_guard.nonzero_conditions.push(context.zero());
    assert!(encode_application_cache_chunk(&binding(), &[zero_guard], &context, limits).is_err());
}
