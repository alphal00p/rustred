use std::io::Cursor;

use super::*;
use crate::persistence::native::{LENGTH_BYTES, read_length};
use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};

fn fixture() -> (
    ApplicationCacheBinding,
    CoefficientContext,
    Vec<ApplicationCacheRecord>,
) {
    let binding = ApplicationCacheBinding {
        artifact_digest: [91; 32],
        family_fingerprint: "stream-writer-family".into(),
        physical_arity: 2,
        layer: "stream-test-native-raw".into(),
    };
    let context = CoefficientContext::new(["stream_writer_d"]);
    let d = context.parameter("stream_writer_d").unwrap();
    let guard = context
        .try_sub(&d, &context.integer(4), Default::default())
        .unwrap();
    let coefficient = context
        .try_div(&context.one(), &guard, Default::default())
        .unwrap();
    let integral = |powers: [i64; 2]| IntegralKey::try_new(powers).unwrap();
    let records = vec![
        ApplicationCacheRecord {
            key: ApplicationCacheKey::guarded_root(&integral([1, 1])),
            terms: BTreeMap::new(),
            nonzero_conditions: vec![guard.clone(), context.integer(-2), guard.clone()],
        },
        ApplicationCacheRecord {
            key: ApplicationCacheKey::route(&integral([1, 2])),
            terms: BTreeMap::from([(integral([1, 1]), coefficient.clone())]),
            nonzero_conditions: vec![guard.clone()],
        },
        ApplicationCacheRecord {
            key: ApplicationCacheKey::apply(vec![true, true], &integral([2, 1])),
            terms: BTreeMap::from([(integral([1, 1]), coefficient)]),
            nonzero_conditions: vec![context.integer(-2), guard.clone(), guard],
        },
    ];
    (binding, context, records)
}

// Independent pre-streaming framing oracle. It deliberately retains the old
// direct record-list/table construction rather than calling the new writer.
fn original_encode(
    binding: &ApplicationCacheBinding,
    records: &[ApplicationCacheRecord],
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"RR-CACHE-CHNK-01");
    out.extend_from_slice(&binding.artifact_digest);
    for text in [&binding.family_fingerprint, &binding.layer] {
        out.extend_from_slice(&(text.len() as u64).to_le_bytes());
        out.extend_from_slice(text.as_bytes());
    }
    out.extend_from_slice(&(binding.physical_arity as u64).to_le_bytes());
    out.extend_from_slice(&(records.len() as u64).to_le_bytes());
    let mut table = CoefficientTableBuilder::new(Default::default());
    for record in records {
        out.push(match record.key.stage() {
            ApplicationCacheStage::GuardedRoot => 0,
            ApplicationCacheStage::Route => 1,
            ApplicationCacheStage::Apply => 2,
        });
        if record.key.stage() == ApplicationCacheStage::Apply {
            out.extend(record.key.owner().iter().map(|value| u8::from(*value)));
        }
        for power in record.key.powers() {
            out.extend_from_slice(&power.to_le_bytes());
        }
        out.extend_from_slice(&(record.terms.len() as u64).to_le_bytes());
        for (target, coefficient) in &record.terms {
            for power in target.powers() {
                out.extend_from_slice(&power.to_le_bytes());
            }
            out.extend_from_slice(
                &(table.intern(coefficient).unwrap().index() as u32).to_le_bytes(),
            );
        }
        out.extend_from_slice(&(record.nonzero_conditions.len() as u64).to_le_bytes());
        for condition in &record.nonzero_conditions {
            out.extend_from_slice(&(table.intern(condition).unwrap().index() as u32).to_le_bytes());
        }
    }
    let table = table.finish().unwrap();
    out.extend_from_slice(&(table.state.len() as u64).to_le_bytes());
    out.extend_from_slice(&table.state);
    out.extend_from_slice(&(table.atoms.len() as u64).to_le_bytes());
    out.extend_from_slice(&table.atoms);
    out
}

// State exports can include ambient registries changed by parallel tests. Split
// that producer-owned state from the exact framing/record/atom bytes below.
fn split_state<'a>(
    bytes: &'a [u8],
    records: &[ApplicationCacheRecord],
) -> (&'a [u8], &'a [u8], &'a [u8]) {
    let mut source = &bytes[MAGIC.len() + 32..];
    for _ in 0..2 {
        let n = read_length(&mut source).unwrap();
        source = &source[n..];
    }
    let arity = read_length(&mut source).unwrap();
    assert_eq!(read_length(&mut source).unwrap(), records.len());
    for record in records {
        let owner_bytes = usize::from(record.key.stage() == ApplicationCacheStage::Apply) * arity;
        source = &source[1 + owner_bytes + arity * 8..];
        let terms = read_length(&mut source).unwrap();
        source = &source[terms * (arity * 8 + 4)..];
        let guards = read_length(&mut source).unwrap();
        source = &source[guards * 4..];
    }
    let prefix_len = bytes.len() - source.len();
    let state_len = read_length(&mut source).unwrap();
    let state = &source[..state_len];
    source = &source[state_len..];
    let atoms_len = read_length(&mut source).unwrap();
    assert_eq!(source.len(), atoms_len);
    (&bytes[..prefix_len], state, source)
}

#[test]
fn incremental_output_preserves_legacy_bytes_and_complete_guard_semantics() {
    let (binding, context, records) = fixture();
    let expected = original_encode(&binding, &records);
    let mut writer = ApplicationCacheChunkWriter::new(
        Cursor::new(Vec::new()),
        &binding,
        &context,
        Default::default(),
    )
    .unwrap();
    assert_eq!(writer.records_written(), 0);
    assert_eq!(writer.unique_coefficients(), 0);
    assert_eq!(writer.unique_coefficient_payload_bytes(), 0);
    let initial_bytes = writer.native_bytes_written();
    for (index, record) in records.iter().enumerate() {
        writer.push(record).unwrap();
        assert_eq!(writer.records_written(), index + 1);
        assert!(writer.native_bytes_written() > initial_bytes);
        assert!(writer.unique_coefficient_payload_bytes() > 0);
    }
    assert_eq!(writer.unique_coefficients(), 3);
    let (cursor, statistics) = writer.finish().unwrap();
    let actual = cursor.into_inner();
    assert_eq!(statistics.records, records.len());
    assert_eq!(statistics.native_bytes, actual.len() as u64);
    assert_eq!(statistics.unique_coefficients, 3);
    assert!(statistics.unique_coefficient_payload_bytes > 0);
    assert!(statistics.interner_index_lower_bound_bytes > 0);
    let (actual_prefix, _, actual_atoms) = split_state(&actual, &records);
    let (expected_prefix, _, expected_atoms) = split_state(&expected, &records);
    assert_eq!(actual_prefix, expected_prefix);
    assert_eq!(actual_atoms, expected_atoms);
    assert_eq!(
        statistics.encoded_coefficient_table_bytes,
        actual_atoms.len()
    );
    let inspected = ApplicationCacheChunk::inspect(&actual, &binding, Default::default()).unwrap();
    let reference =
        ApplicationCacheChunk::inspect(&expected, &binding, Default::default()).unwrap();
    for record in &records {
        assert_eq!(
            inspected.decode_record(&record.key, &context).unwrap(),
            Some(record.clone())
        );
        assert_eq!(
            reference.decode_record(&record.key, &context).unwrap(),
            Some(record.clone())
        );
    }
    let repeated =
        encode_application_cache_chunk(&binding, &records, &context, Default::default()).unwrap();
    let (prefix, _, atoms) = split_state(&repeated, &records);
    assert_eq!(actual_prefix, prefix);
    assert_eq!(actual_atoms, atoms);
}

#[test]
fn empty_and_embedded_chunks_patch_counts_and_restore_end_position() {
    let (binding, context, records) = fixture();
    for input in [&records[..0], &records[..]] {
        let mut destination = Cursor::new(b"caller-prefix".to_vec());
        let start = destination.get_ref().len() as u64;
        destination.set_position(start);
        let mut writer = ApplicationCacheChunkWriter::new(
            &mut destination,
            &binding,
            &context,
            Default::default(),
        )
        .unwrap();
        for record in input {
            writer.push(record).unwrap();
        }
        let (_, statistics) = writer.finish().unwrap();
        assert_eq!(destination.position(), destination.get_ref().len() as u64);
        assert_eq!(statistics.native_bytes, destination.position() - start);
        assert_eq!(&destination.get_ref()[..start as usize], b"caller-prefix");
        let decoded = ApplicationCacheChunk::inspect(
            &destination.get_ref()[start as usize..],
            &binding,
            Default::default(),
        )
        .unwrap();
        assert_eq!(decoded.len(), input.len());
    }
}

#[test]
fn ordering_and_invalid_payload_failures_poison_finalization() {
    let (binding, context, records) = fixture();
    for second in [&records[1], &records[0]] {
        let mut writer = ApplicationCacheChunkWriter::new(
            Cursor::new(Vec::new()),
            &binding,
            &context,
            Default::default(),
        )
        .unwrap();
        writer.push(&records[1]).unwrap();
        assert!(matches!(
            writer.push(second),
            Err(BinaryIoError::Invalid(
                "cache keys are not strictly ordered"
            ))
        ));
        assert!(matches!(
            writer.finish(),
            Err(BinaryIoError::Invalid("cache chunk writer is poisoned"))
        ));
    }
    let invalid = ApplicationCacheRecord {
        nonzero_conditions: vec![context.integer(0)],
        ..records[0].clone()
    };
    let mut writer = ApplicationCacheChunkWriter::new(
        Cursor::new(Vec::new()),
        &binding,
        &context,
        Default::default(),
    )
    .unwrap();
    assert!(writer.push(&invalid).is_err());
    assert!(writer.push(&records[0]).is_err());
    assert!(writer.finish().is_err());
}

#[derive(Debug)]
struct FailingOutput {
    cursor: Cursor<Vec<u8>>,
    writes: usize,
    fail_after: usize,
    fail_seek: bool,
}
impl Write for FailingOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if self.writes > self.fail_after {
            return Err(io::Error::other("injected native write failure"));
        }
        self.cursor.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Seek for FailingOutput {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        if self.fail_seek && matches!(from, SeekFrom::Start(_)) {
            return Err(io::Error::other("injected count patch failure"));
        }
        self.cursor.seek(from)
    }
}

#[test]
fn bounded_and_failing_destinations_never_report_complete_output() {
    let (binding, context, records) = fixture();
    let mut limits = ApplicationCacheIoLimits::default();
    limits.binary.max_program_bytes = 10;
    assert!(matches!(
        ApplicationCacheChunkWriter::new(Cursor::new(Vec::new()), &binding, &context, limits),
        Err(BinaryIoError::Limit {
            resource: "cache chunk bytes",
            ..
        })
    ));
    let output = FailingOutput {
        cursor: Cursor::new(Vec::new()),
        writes: 0,
        fail_after: usize::MAX,
        fail_seek: true,
    };
    let mut writer =
        ApplicationCacheChunkWriter::new(output, &binding, &context, Default::default()).unwrap();
    writer.push(&records[0]).unwrap();
    assert!(
        writer
            .finish()
            .unwrap_err()
            .to_string()
            .contains("injected count patch failure")
    );
    let output = FailingOutput {
        cursor: Cursor::new(Vec::new()),
        writes: 0,
        fail_after: 8,
        fail_seek: false,
    };
    // Header is eight writes; the first record fails rather than becoming a
    // silently skipped record followed by a valid-looking completed file.
    let mut writer =
        ApplicationCacheChunkWriter::new(output, &binding, &context, Default::default()).unwrap();
    assert!(writer.push(&records[0]).is_err());
    assert!(writer.finish().is_err());
    let budget = ResourceBudget::with_memory_monitor(
        ResourceLimits {
            max_process_rss_bytes: Some(1),
            ..Default::default()
        },
        || Ok(2),
    );
    let result = budget
        .run(|| encode_application_cache_chunk(&binding, &records, &context, Default::default()));
    assert!(matches!(
        result,
        Err(BinaryIoError::Resource(ResourceLimitError::Memory { .. }))
    ));
}

#[test]
fn declared_byte_and_diagnostic_limits_remain_explicit() {
    let (binding, context, records) = fixture();
    let encoded =
        encode_application_cache_chunk(&binding, &records, &context, Default::default()).unwrap();
    let mut limits = ApplicationCacheIoLimits::default();
    limits.binary.max_program_bytes = encoded.len() - 1;
    assert!(encode_application_cache_chunk(&binding, &records, &context, limits).is_err());
    limits = Default::default();
    limits.max_records = 0;
    encode_application_cache_chunk(&binding, &records, &context, limits).unwrap();
    limits.diagnostics_enabled = true;
    assert!(matches!(
        encode_application_cache_chunk(&binding, &records, &context, limits),
        Err(BinaryIoError::Limit {
            resource: "cache records",
            ..
        })
    ));
    let (_, state, atoms) = split_state(&encoded, &records);
    limits = Default::default();
    limits.binary.max_state_bytes = state.len() - 1;
    assert!(encode_application_cache_chunk(&binding, &records, &context, limits).is_err());
    limits = Default::default();
    limits.binary.max_total_atom_bytes = atoms.len() - 1;
    assert!(encode_application_cache_chunk(&binding, &records, &context, limits).is_err());
    assert_eq!(LENGTH_BYTES, 8);
}

#[test]
fn unordered_callbacks_emit_sorted_unique_rows_with_one_shared_table() {
    let (binding, context, records) = fixture();
    let mut spool = Cursor::new(b"spool-prefix".to_vec());
    spool.set_position(spool.get_ref().len() as u64);
    let mut writer = ApplicationCacheChunkWriter::new_unordered(
        &mut spool,
        &binding,
        &context,
        Default::default(),
    )
    .unwrap();
    for index in [2, 0, 1, 2, 0] {
        writer.push_unordered(&records[index]).unwrap();
    }
    assert_eq!(writer.records_written(), 3);
    assert_eq!(writer.unique_coefficients(), 3);
    let mut destination = Cursor::new(b"destination-prefix".to_vec());
    let prefix = destination.get_ref().len();
    destination.set_position(prefix as u64);
    let (_, statistics) = writer.finish_sorted_into(&mut destination).unwrap();
    assert_eq!(statistics.records, 3);
    assert_eq!(statistics.input_records, 5);
    assert_eq!(statistics.identical_duplicates, 2);
    assert_eq!(statistics.unique_coefficients, 3);
    assert!(statistics.unordered_index_owned_bytes > 0);
    assert!(statistics.unordered_spool_bytes < statistics.native_bytes);
    assert_eq!(
        statistics.native_bytes as usize + prefix,
        destination.get_ref().len()
    );
    assert_eq!(destination.position(), destination.get_ref().len() as u64);
    assert_eq!(&destination.get_ref()[..prefix], b"destination-prefix");
    let bytes = &destination.get_ref()[prefix..];
    let inspected = ApplicationCacheChunk::inspect(bytes, &binding, Default::default()).unwrap();
    assert_eq!(
        inspected.keys().collect::<Vec<_>>(),
        records.iter().map(|record| &record.key).collect::<Vec<_>>()
    );
    for record in &records {
        assert_eq!(
            inspected.decode_record(&record.key, &context).unwrap(),
            Some(record.clone())
        );
    }
    let (_, _, mut atoms) = split_state(bytes, &records);
    assert_eq!(read_length(&mut atoms).unwrap(), 3);
}

#[test]
fn unordered_duplicate_conflicts_include_coefficients_and_ordered_guards() {
    let (binding, context, records) = fixture();
    let mut coefficient_variant = records[1].clone();
    *coefficient_variant.terms.values_mut().next().unwrap() = context.integer(7);
    let mut guard_order_variant = records[0].clone();
    guard_order_variant.nonzero_conditions.swap(0, 1);
    let mut guard_multiplicity_variant = records[2].clone();
    guard_multiplicity_variant.nonzero_conditions.pop();
    for (base, variant) in [
        (&records[1], coefficient_variant),
        (&records[0], guard_order_variant),
        (&records[2], guard_multiplicity_variant),
    ] {
        let mut writer = ApplicationCacheChunkWriter::new_unordered(
            Cursor::new(Vec::new()),
            &binding,
            &context,
            Default::default(),
        )
        .unwrap();
        writer.push_unordered(base).unwrap();
        assert!(matches!(
            writer.push_unordered(&variant),
            Err(BinaryIoError::Invalid(
                "conflicting cache records for one exact key"
            ))
        ));
        assert!(matches!(
            writer.finish_sorted_into(Cursor::new(Vec::new())),
            Err(BinaryIoError::Invalid("cache chunk writer is poisoned"))
        ));
    }
}

#[test]
fn unordered_ingestion_retains_unique_interner_and_key_ranges_not_record_algebra() {
    let (binding, context, records) = fixture();
    let mut writer = ApplicationCacheChunkWriter::new_unordered(
        Cursor::new(Vec::new()),
        &binding,
        &context,
        Default::default(),
    )
    .unwrap();
    for index in (1..=128).rev() {
        // Each transient input is dropped before the next callback. Repeated
        // coefficient algebra remains represented by only two shared atom IDs.
        let record = ApplicationCacheRecord {
            key: ApplicationCacheKey::route(&IntegralKey::try_new([1, index]).unwrap()),
            terms: records[1].terms.clone(),
            nonzero_conditions: records[1].nonzero_conditions.clone(),
        };
        writer.push_unordered(&record).unwrap();
        assert_eq!(writer.unique_coefficients(), 2);
    }
    let (_, statistics) = writer.finish_sorted_into(Cursor::new(Vec::new())).unwrap();
    assert_eq!(statistics.records, 128);
    assert_eq!(statistics.input_records, 128);
    assert_eq!(statistics.identical_duplicates, 0);
    assert_eq!(statistics.unique_coefficients, 2);
    assert!(statistics.unordered_index_owned_bytes > statistics.unique_coefficient_payload_bytes);
}

#[test]
fn unordered_mode_index_copy_and_mixed_api_failures_are_explicit() {
    let (binding, context, records) = fixture();
    let limits = ApplicationCacheIoLimits {
        max_index_owned_bytes: 1,
        ..ApplicationCacheIoLimits::default()
    };
    let mut writer = ApplicationCacheChunkWriter::new_unordered(
        Cursor::new(Vec::new()),
        &binding,
        &context,
        limits,
    )
    .unwrap();
    assert!(matches!(
        writer.push_unordered(&records[0]),
        Err(BinaryIoError::Limit {
            resource: "unordered cache row index bytes",
            ..
        })
    ));
    assert_eq!(writer.records_written(), 0);
    assert!(writer.finish_sorted_into(Cursor::new(Vec::new())).is_err());

    let mut writer = ApplicationCacheChunkWriter::new_unordered(
        Cursor::new(Vec::new()),
        &binding,
        &context,
        Default::default(),
    )
    .unwrap();
    assert!(writer.push(&records[0]).is_err());
    assert!(writer.finish_sorted_into(Cursor::new(Vec::new())).is_err());
    let writer = ApplicationCacheChunkWriter::new_unordered(
        Cursor::new(Vec::new()),
        &binding,
        &context,
        Default::default(),
    )
    .unwrap();
    assert!(matches!(
        writer.finish(),
        Err(BinaryIoError::Invalid(
            "unordered cache writer requires finish_sorted_into"
        ))
    ));
    let mut writer = ApplicationCacheChunkWriter::new(
        Cursor::new(Vec::new()),
        &binding,
        &context,
        Default::default(),
    )
    .unwrap();
    assert!(writer.push_unordered(&records[0]).is_err());
    assert!(writer.finish().is_err());

    let mut writer = ApplicationCacheChunkWriter::new_unordered(
        Cursor::new(Vec::new()),
        &binding,
        &context,
        Default::default(),
    )
    .unwrap();
    writer.push_unordered(&records[0]).unwrap();
    let destination = FailingOutput {
        cursor: Cursor::new(Vec::new()),
        writes: 0,
        fail_after: 8,
        fail_seek: false,
    };
    assert!(
        writer
            .finish_sorted_into(destination)
            .unwrap_err()
            .to_string()
            .contains("injected native write failure")
    );
}
