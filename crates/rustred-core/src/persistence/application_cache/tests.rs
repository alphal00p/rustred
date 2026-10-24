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
    assert_eq!(inspected.keys().len(), records.len());
    assert_eq!(
        inspected.keys().collect::<Vec<_>>(),
        records.iter().map(|record| &record.key).collect::<Vec<_>>()
    );
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
fn diagnostic_counts_and_explicit_coefficient_byte_limits_are_enforced() {
    let (context, records) = fixture();
    let limits = ApplicationCacheIoLimits::default();
    let encoded = encode_application_cache_chunk(&binding(), &records, &context, limits).unwrap();
    let tight = ApplicationCacheIoLimits {
        diagnostics_enabled: true,
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
        diagnostics_enabled: true,
        max_guards_per_record: 2,
        ..limits
    };
    assert!(ApplicationCacheChunk::inspect(&encoded, &binding(), tight).is_err());
    let mut zero_guard = records[0].clone();
    zero_guard.nonzero_conditions.push(context.zero());
    assert!(encode_application_cache_chunk(&binding(), &[zero_guard], &context, limits).is_err());
}

#[test]
fn retained_chunk_shares_table_and_revalidates_selected_context() {
    use std::sync::Arc;
    let (context, records) = fixture();
    let limits = ApplicationCacheIoLimits::default();
    let encoded = encode_application_cache_chunk(&binding(), &records, &context, limits).unwrap();
    let chunk = Arc::new(
        OwnedApplicationCacheChunk::inspect(Arc::from(encoded), &binding(), limits).unwrap(),
    );
    assert_eq!(chunk.decoded_coefficient_count().unwrap(), 0);
    assert_eq!(
        chunk.keys().collect::<Vec<_>>(),
        records.iter().map(|r| &r.key).collect::<Vec<_>>()
    );
    assert!(chunk.index_owned_bytes() > 0);
    assert_eq!(
        chunk.decode_record(&records[1].key, &context).unwrap(),
        Some(records[1].clone())
    );
    let count = chunk.decoded_coefficient_count().unwrap();
    assert_eq!(count, 2);
    assert_eq!(
        Arc::clone(&chunk)
            .decode_record(&records[1].key, &context)
            .unwrap(),
        Some(records[1].clone())
    );
    assert_eq!(chunk.decoded_coefficient_count().unwrap(), count);
    let foreign = CoefficientContext::new(["wrong_owned_context"]);
    assert!(chunk.decode_record(&records[1].key, &foreign).is_err());
    assert_eq!(
        chunk.decode_record(&records[0].key, &context).unwrap(),
        Some(records[0].clone())
    );
    assert_eq!(chunk.decoded_coefficient_count().unwrap(), count + 1);
    assert_eq!(
        chunk
            .decode_record(&ApplicationCacheKey::route(&key(&[99, 1])), &context)
            .unwrap(),
        None
    );
}

#[test]
fn retained_chunk_index_budget_and_framing_are_checked_before_lookup() {
    use std::sync::Arc;
    let (context, records) = fixture();
    let limits = ApplicationCacheIoLimits::default();
    let encoded = encode_application_cache_chunk(&binding(), &records, &context, limits).unwrap();
    let owned = OwnedApplicationCacheChunk::inspect(Arc::from(encoded.clone()), &binding(), limits)
        .unwrap();
    let tight = ApplicationCacheIoLimits {
        max_index_owned_bytes: owned.index_owned_bytes() - 1,
        ..limits
    };
    assert!(
        OwnedApplicationCacheChunk::inspect(Arc::from(encoded.clone()), &binding(), tight).is_err()
    );
    let tight = ApplicationCacheIoLimits {
        max_index_owned_bytes: 0,
        ..limits
    };
    assert!(ApplicationCacheChunk::inspect(&encoded, &binding(), tight).is_err());
    let mut trailing = encoded;
    trailing.push(0);
    assert!(OwnedApplicationCacheChunk::inspect(Arc::from(trailing), &binding(), limits).is_err());
}

#[test]
fn shared_decoded_census_limits_unique_coefficients_across_records() {
    use std::sync::Arc;
    let context = CoefficientContext::new(["shared_budget_d"]);
    let one = context.one();
    let budget = coefficient_cache_weight([&one]).unwrap();
    let records = [1, 2].map(|i| ApplicationCacheRecord {
        key: ApplicationCacheKey::route(&key(&[i, 1])),
        terms: BTreeMap::from([(key(&[1, 1]), context.integer(i))]),
        nonzero_conditions: vec![],
    });
    let limits = ApplicationCacheIoLimits {
        diagnostics_enabled: true,
        max_decoded_coefficient_terms: budget.coefficient_terms,
        ..Default::default()
    };
    let encoded = encode_application_cache_chunk(&binding(), &records, &context, limits).unwrap();
    let chunk =
        OwnedApplicationCacheChunk::inspect(Arc::from(encoded), &binding(), limits).unwrap();
    assert_eq!(
        chunk.decode_record(&records[0].key, &context).unwrap(),
        Some(records[0].clone())
    );
    assert_eq!(
        chunk.decode_record(&records[0].key, &context).unwrap(),
        Some(records[0].clone())
    );
    assert!(matches!(
        chunk.decode_record(&records[1].key, &context),
        Err(BinaryIoError::Limit {
            resource: "shared decoded cache coefficient terms",
            ..
        })
    ));
    assert_eq!(chunk.decoded_coefficient_count().unwrap(), 1);
    assert_eq!(
        chunk.decoded_coefficient_census().unwrap().0,
        budget.coefficient_terms
    );
    // Rejected growth does not poison previously usable entries.
    assert_eq!(
        chunk.decode_record(&records[0].key, &context).unwrap(),
        Some(records[0].clone())
    );
}

#[test]
fn normal_chunks_can_exceed_the_retained_record_diagnostic_template() {
    let context = CoefficientContext::new(["normal_cache_d"]);
    let limits = ApplicationCacheIoLimits::default();
    assert!(!limits.diagnostics_enabled);
    assert_eq!(limits.max_records, 4096);
    assert_eq!(limits.max_index_owned_bytes, usize::MAX);
    assert_eq!(limits.max_decoded_coefficient_bytes, usize::MAX);
    let records = (1..=4097)
        .map(|i| ApplicationCacheRecord {
            key: ApplicationCacheKey::route(&key(&[i, 1])),
            terms: BTreeMap::new(),
            nonzero_conditions: Vec::new(),
        })
        .collect::<Vec<_>>();
    let encoded = encode_application_cache_chunk(&binding(), &records, &context, limits).unwrap();
    let inspected = ApplicationCacheChunk::inspect(&encoded, &binding(), limits).unwrap();
    assert_eq!(inspected.keys().count(), records.len());
    let diagnostic = ApplicationCacheIoLimits {
        diagnostics_enabled: true,
        ..limits
    };
    assert!(matches!(
        encode_application_cache_chunk(&binding(), &records, &context, diagnostic),
        Err(BinaryIoError::Limit {
            resource: "cache records",
            ..
        })
    ));
    assert!(matches!(
        ApplicationCacheChunk::inspect(&encoded, &binding(), diagnostic),
        Err(BinaryIoError::Limit {
            resource: "cache records",
            ..
        })
    ));
}

#[test]
fn customized_count_templates_are_inactive_without_diagnostics() {
    let (context, records) = fixture();
    let limits = ApplicationCacheIoLimits {
        max_arity: 0,
        max_identity_bytes: 0,
        max_records: 0,
        max_terms_per_record: 0,
        max_guards_per_record: 0,
        max_decoded_coefficient_terms: 0,
        ..Default::default()
    };
    let encoded = encode_application_cache_chunk(&binding(), &records, &context, limits).unwrap();
    let inspected = ApplicationCacheChunk::inspect(&encoded, &binding(), limits).unwrap();
    for record in &records {
        assert_eq!(
            inspected.decode_record(&record.key, &context).unwrap(),
            Some(record.clone())
        );
    }
    let wrong_arity = ApplicationCacheBinding {
        physical_arity: 3,
        ..binding()
    };
    assert!(ApplicationCacheChunk::inspect(&encoded, &wrong_arity, limits).is_err());
    assert!(matches!(
        binding().validate(ApplicationCacheIoLimits {
            diagnostics_enabled: true,
            ..limits
        }),
        Err(BinaryIoError::Limit {
            resource: "cache arity",
            ..
        })
    ));
}

#[test]
fn malformed_record_counts_fail_framing_before_index_allocation() {
    let context = CoefficientContext::new(["malformed_cache_d"]);
    let binding = binding();
    let limits = ApplicationCacheIoLimits::default();
    let mut encoded = encode_application_cache_chunk(&binding, &[], &context, limits).unwrap();
    let count_offset = 16 + 32 + 8 + binding.family_fingerprint.len() + 8 + binding.layer.len() + 8;
    encoded[count_offset..count_offset + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(matches!(
        ApplicationCacheChunk::inspect(&encoded, &binding, limits),
        Err(BinaryIoError::Invalid("truncated cache row directory"))
            | Err(BinaryIoError::Invalid("length exceeds usize"))
    ));
}

#[test]
fn native_coefficient_census_resource_errors_remain_typed() {
    let resource = symbolica::utils::resources::ResourceLimitError::Memory {
        rss_bytes: 40,
        additional_bytes: 3,
        limit_bytes: 42,
    };
    let error = reduction_error(crate::reduction::ReductionError::Resources(
        resource.clone(),
    ));
    assert_eq!(error, BinaryIoError::Resource(resource.clone()));
    assert_eq!(
        std::error::Error::source(&error)
            .unwrap()
            .downcast_ref::<symbolica::utils::resources::ResourceLimitError>(),
        Some(&resource)
    );
}

#[test]
fn resource_checked_transport_clone_preserves_payload_and_typed_memory_refusal() {
    use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};

    let (_, records) = fixture();
    for record in &records {
        assert_eq!(record.try_clone_with_resources().unwrap(), *record);
    }
    let budget = ResourceBudget::with_memory_monitor(
        ResourceLimits {
            max_process_rss_bytes: Some(110),
            ..Default::default()
        },
        || Ok(100),
    );
    assert!(matches!(
        budget.run(|| records[1].try_clone_with_resources()),
        Err(BinaryIoError::Resource(ResourceLimitError::Memory {
            rss_bytes: 100,
            additional_bytes: 16,
            limit_bytes: 110,
        }))
    ));
    assert_eq!(records[1].try_clone_with_resources().unwrap(), records[1]);
}
