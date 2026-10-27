use super::*;
use rustred::family::IntegralKey;
use std::collections::BTreeMap;

fn fixture() -> (
    ApplicationCacheBinding,
    CoefficientContext,
    Vec<ApplicationCacheRecord>,
) {
    let context = CoefficientContext::try_new(["single_file_cache_d"]).unwrap();
    let d = context.parameter("single_file_cache_d").unwrap();
    let guard = context
        .try_sub(&d, &context.integer(4), Default::default())
        .unwrap();
    let coefficient = context
        .try_div(&context.one(), &guard, Default::default())
        .unwrap();
    let key = |p: &[i64]| IntegralKey::try_new(p.iter().copied()).unwrap();
    let binding = ApplicationCacheBinding {
        artifact_digest: [33; 32],
        family_fingerprint: "single-file-tests".into(),
        physical_arity: 2,
        layer: "routed-before-terminal-substitution-v1".into(),
    };
    let records = vec![
        ApplicationCacheRecord {
            key: ApplicationCacheKey::route(&key(&[2, 1])),
            terms: BTreeMap::from([(key(&[1, 1]), coefficient.clone())]),
            nonzero_conditions: vec![guard.clone(), context.integer(-2), guard.clone()],
        },
        ApplicationCacheRecord {
            key: ApplicationCacheKey::apply(vec![true, true], &key(&[2, 1])),
            terms: BTreeMap::from([(key(&[1, 1]), coefficient)]),
            nonzero_conditions: vec![guard],
        },
    ];
    (binding, context, records)
}
fn encode_decoded(decoded: &[u8]) -> (Vec<u8>, SavedApplicationCacheFileIdentity) {
    let mut encoder = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::fast());
    encoder.write_all(decoded).unwrap();
    let encoded = encoder.finish().unwrap();
    let identity = SavedApplicationCacheFileIdentity {
        compressed_bytes: encoded.len() as u64,
        compressed_blake3: *blake3::hash(&encoded).as_bytes(),
        decoded_bytes: decoded.len() as u64,
        decoded_blake3: *blake3::hash(decoded).as_bytes(),
    };
    (encoded, identity)
}
fn reauthenticate(
    encoded: &[u8],
    mut identity: SavedApplicationCacheFileIdentity,
) -> SavedApplicationCacheFileIdentity {
    identity.compressed_bytes = encoded.len() as u64;
    identity.compressed_blake3 = *blake3::hash(encoded).as_bytes();
    identity
}

#[test]
fn deterministic_single_member_roundtrip_lazy_sharing_and_context_validation() {
    let (binding, context, records) = fixture();
    let limits = SavedApplicationCacheFileLimits::default();
    let (encoded, identity) =
        SavedApplicationCacheFile::encode_generated(&binding, &records, &context, limits).unwrap();
    let (encoded_again, identity_again) =
        SavedApplicationCacheFile::encode_generated(&binding, &records, &context, limits).unwrap();
    assert_eq!(encoded, encoded_again);
    assert_eq!(identity, identity_again);
    assert_eq!(&encoded[..2], &[0x1f, 0x8b]);
    let mut reader =
        SavedApplicationCacheFile::decode_generated(&encoded, &binding, &identity, limits).unwrap();
    assert_eq!(reader.binding(), &binding);
    assert_eq!(reader.identity(), &identity);
    assert_eq!(reader.statistics().unwrap().decoded_coefficients, 0);
    assert_eq!(
        reader.keys().collect::<Vec<_>>(),
        records.iter().map(|r| &r.key).collect::<Vec<_>>()
    );
    let mut fork = reader.fork_reader();
    assert!(Arc::ptr_eq(&reader.chunk, &fork.chunk));
    assert_eq!(
        reader.lookup(&records[0].key, &context).unwrap(),
        Some(records[0].clone())
    );
    assert_eq!(
        fork.lookup(&records[1].key, &context).unwrap(),
        Some(records[1].clone())
    );
    assert_eq!(reader.statistics().unwrap().decoded_coefficients, 3);
    assert_eq!(fork.statistics().unwrap().decoded_coefficients, 3);
    assert_eq!(fork.statistics().unwrap().hits, 1);
    let foreign = CoefficientContext::try_new(["bad_file_context"]).unwrap();
    assert!(reader.lookup(&records[0].key, &foreign).is_err());
    assert_eq!(reader.statistics().unwrap().hits, 1);
    let absent = ApplicationCacheKey::route(&IntegralKey::try_new([99, 1]).unwrap());
    assert_eq!(reader.lookup(&absent, &context).unwrap(), None);
    assert_eq!(reader.statistics().unwrap().misses, 1);
}

#[test]
fn identities_bind_full_campaign_and_both_transport_representations() {
    let (binding, context, records) = fixture();
    let limits = SavedApplicationCacheFileLimits::default();
    let (encoded, identity) =
        SavedApplicationCacheFile::encode_generated(&binding, &records, &context, limits).unwrap();
    for bad_binding in [
        ApplicationCacheBinding {
            artifact_digest: [77; 32],
            ..binding.clone()
        },
        ApplicationCacheBinding {
            family_fingerprint: "another-family".into(),
            ..binding.clone()
        },
        ApplicationCacheBinding {
            layer: "after-terminal-substitution".into(),
            ..binding.clone()
        },
        ApplicationCacheBinding {
            physical_arity: 3,
            ..binding.clone()
        },
    ] {
        assert!(
            SavedApplicationCacheFile::decode_generated(&encoded, &bad_binding, &identity, limits)
                .is_err()
        );
    }
    for bad_identity in [
        SavedApplicationCacheFileIdentity {
            compressed_bytes: identity.compressed_bytes - 1,
            ..identity
        },
        SavedApplicationCacheFileIdentity {
            compressed_blake3: [0; 32],
            ..identity
        },
        SavedApplicationCacheFileIdentity {
            decoded_bytes: identity.decoded_bytes - 1,
            ..identity
        },
        SavedApplicationCacheFileIdentity {
            decoded_bytes: identity.decoded_bytes + 1,
            ..identity
        },
        SavedApplicationCacheFileIdentity {
            decoded_blake3: [0; 32],
            ..identity
        },
    ] {
        assert!(
            SavedApplicationCacheFile::decode_generated(&encoded, &binding, &bad_identity, limits)
                .is_err()
        );
    }
}

#[test]
fn reauthenticated_extra_members_trailing_data_and_corruption_are_rejected() {
    let (binding, context, records) = fixture();
    let limits = SavedApplicationCacheFileLimits::default();
    let (encoded, identity) =
        SavedApplicationCacheFile::encode_generated(&binding, &records, &context, limits).unwrap();
    for extra in [&encoded[..], &[0][..]] {
        let mut bad = encoded.clone();
        bad.extend_from_slice(extra);
        let bad_identity = reauthenticate(&bad, identity);
        assert!(
            SavedApplicationCacheFile::decode_generated(&bad, &binding, &bad_identity, limits)
                .is_err()
        );
    }
    let mut corrupted = encoded.clone();
    // Re-authenticate external transport identity to exercise the gzip CRC.
    let crc = corrupted.len() - 8;
    corrupted[crc] ^= 1;
    let bad_identity = reauthenticate(&corrupted, identity);
    assert!(
        SavedApplicationCacheFile::decode_generated(&corrupted, &binding, &bad_identity, limits)
            .is_err()
    );
    let truncated = &encoded[..encoded.len() - 1];
    let bad_identity = reauthenticate(truncated, identity);
    assert!(
        SavedApplicationCacheFile::decode_generated(truncated, &binding, &bad_identity, limits)
            .is_err()
    );
}

#[test]
fn inner_framing_and_native_table_are_checked_before_lookup() {
    let (binding, context, records) = fixture();
    let limits = SavedApplicationCacheFileLimits::default();
    let (encoded, _) =
        SavedApplicationCacheFile::encode_generated(&binding, &records, &context, limits).unwrap();
    let mut decoded = Vec::new();
    GzDecoder::new(&encoded[..])
        .read_to_end(&mut decoded)
        .unwrap();
    for offset in [0, 16, 24, HEADER_BYTES] {
        let mut bad = decoded.clone();
        bad[offset] ^= 1;
        let (bad, identity) = encode_decoded(&bad);
        assert!(
            SavedApplicationCacheFile::decode_generated(&bad, &binding, &identity, limits).is_err()
        );
    }
    let mut bad_native = decoded;
    bad_native[HEADER_BYTES] ^= 1;
    let inner_hash = blake3::hash(&bad_native[HEADER_BYTES..]);
    bad_native[24..56].copy_from_slice(inner_hash.as_bytes());
    let (bad_native, identity) = encode_decoded(&bad_native);
    assert!(
        SavedApplicationCacheFile::decode_generated(&bad_native, &binding, &identity, limits)
            .is_err()
    );
}

#[test]
fn byte_and_index_limits_hold_even_when_caller_requests_unbounded_transport() {
    let (binding, context, records) = fixture();
    let limits = SavedApplicationCacheFileLimits::default();
    let (encoded, identity) =
        SavedApplicationCacheFile::encode_generated(&binding, &records, &context, limits).unwrap();
    let mut tight = limits;
    tight.max_compressed_bytes = encoded.len() - 1;
    assert!(
        SavedApplicationCacheFile::decode_generated(&encoded, &binding, &identity, tight).is_err()
    );
    assert!(
        SavedApplicationCacheFile::encode_generated(&binding, &records, &context, tight).is_err()
    );
    tight = limits;
    tight.chunk.binary.max_program_bytes = identity.decoded_bytes as usize - HEADER_BYTES - 1;
    assert!(
        SavedApplicationCacheFile::decode_generated(&encoded, &binding, &identity, tight).is_err()
    );
    tight = limits;
    tight.chunk.max_index_owned_bytes = 0;
    assert!(
        SavedApplicationCacheFile::decode_generated(&encoded, &binding, &identity, tight).is_err()
    );
    let mut unbounded = limits;
    unbounded.max_compressed_bytes = usize::MAX;
    unbounded.chunk.binary.max_program_bytes = usize::MAX;
    for oversized in [
        SavedApplicationCacheFileIdentity {
            compressed_bytes: MAX_COMPRESSED_BYTES as u64 + 1,
            ..identity
        },
        SavedApplicationCacheFileIdentity {
            decoded_bytes: MAX_DECODED_BYTES as u64 + 1,
            ..identity
        },
    ] {
        assert!(
            SavedApplicationCacheFile::decode_generated(&[], &binding, &oversized, unbounded)
                .is_err()
        );
    }
}

#[test]
fn readonly_backing_never_mutates_the_file_or_hides_invalid_records() {
    use rustred::reduction::{GuardedCacheBacking, GuardedCacheBackingErrorKind};
    let (binding, context, records) = fixture();
    let limits = SavedApplicationCacheFileLimits::default();
    let (encoded, identity) =
        SavedApplicationCacheFile::encode_generated(&binding, &records, &context, limits).unwrap();
    let mut reader =
        SavedApplicationCacheFile::decode_generated(&encoded, &binding, &identity, limits).unwrap();
    let backing: &mut dyn GuardedCacheBacking = &mut reader;
    assert_eq!(
        backing.lookup(&records[0].key, &context).unwrap(),
        Some(records[0].clone())
    );
    assert!(backing.store_completed(&records[1], &context).is_err());
    let bad_key = ApplicationCacheKey::route(&IntegralKey::try_new([1, 1, 1]).unwrap());
    assert_eq!(
        backing.lookup(&bad_key, &context).unwrap_err().kind,
        GuardedCacheBackingErrorKind::Invalid
    );
    assert_eq!(reader.identity(), &identity);
    assert_eq!(reader.keys().len(), records.len());
}

#[test]
fn explicit_file_open_checks_embedded_identity_and_authenticated_open_checks_manifest() {
    let (binding, context, records) = fixture();
    let limits = SavedApplicationCacheFileLimits::default();
    let (encoded, identity) =
        SavedApplicationCacheFile::encode_generated(&binding, &records, &context, limits).unwrap();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "rustred-single-cache-{}-{unique}.bin.gz",
        std::process::id()
    ));
    std::fs::write(&path, &encoded).unwrap();
    let mut explicit = SavedApplicationCacheFile::open(&path, &binding, limits).unwrap();
    assert_eq!(explicit.identity(), &identity);
    assert_eq!(
        explicit.lookup(&records[0].key, &context).unwrap(),
        Some(records[0].clone())
    );
    let authenticated =
        SavedApplicationCacheFile::open_generated(&path, &binding, &identity, limits).unwrap();
    assert_eq!(authenticated.identity(), &identity);
    let bad_identity = SavedApplicationCacheFileIdentity {
        compressed_blake3: [0; 32],
        ..identity
    };
    assert!(
        SavedApplicationCacheFile::open_generated(&path, &binding, &bad_identity, limits).is_err()
    );
    let mut corrupted = encoded;
    let crc = corrupted.len() - 8;
    corrupted[crc] ^= 1;
    std::fs::write(&path, corrupted).unwrap();
    assert!(SavedApplicationCacheFile::open(&path, &binding, limits).is_err());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn explicit_decode_rejects_native_length_overrun_underrun_and_empty_second_member() {
    let (binding, context, records) = fixture();
    let limits = SavedApplicationCacheFileLimits::default();
    let (encoded, _) =
        SavedApplicationCacheFile::encode_generated(&binding, &records, &context, limits).unwrap();
    let mut decoded = Vec::new();
    GzDecoder::new(&encoded[..])
        .read_to_end(&mut decoded)
        .unwrap();
    let native_length = u64::from_le_bytes(decoded[16..24].try_into().unwrap());
    for bad_length in [native_length - 1, native_length + 1] {
        let mut bad = decoded.clone();
        bad[16..24].copy_from_slice(&bad_length.to_le_bytes());
        let (bad, _) = encode_decoded(&bad);
        assert!(SavedApplicationCacheFile::decode(&bad, &binding, limits).is_err());
    }
    let (empty_member, _) = encode_decoded(&[]);
    let mut extra = encoded;
    extra.extend_from_slice(&empty_member);
    assert!(SavedApplicationCacheFile::decode(&extra, &binding, limits).is_err());
}

#[test]
fn explicit_heavy_grants_extend_compact_defaults_without_overflow() {
    let defaults = SavedApplicationCacheFileLimits::default();
    let mut heavy = defaults;
    heavy.max_compressed_bytes = usize::MAX;
    heavy.chunk.binary.max_program_bytes = usize::MAX;
    let identity = SavedApplicationCacheFileIdentity {
        compressed_bytes: MAX_COMPRESSED_BYTES as u64 + 1,
        compressed_blake3: [0; 32],
        decoded_bytes: MAX_DECODED_BYTES as u64 + 1,
        decoded_blake3: [0; 32],
    };
    assert!(validate_identity(&identity, defaults).is_err());
    assert!(validate_identity(&identity, heavy).is_ok());
    // This admission does not make missing/corrupt contents valid.
    let (binding, _, _) = fixture();
    assert!(SavedApplicationCacheFile::decode_generated(&[], &binding, &identity, heavy).is_err());
    assert_eq!(heavy.decoded_limit(), usize::MAX);
}

#[test]
fn streamed_export_preserves_native_rows_guards_and_cleans_spool() {
    let (binding, context, records) = fixture();
    let directory = test_directory("stream");
    let mut writer =
        SavedApplicationCacheFileWriter::new(&binding, &context, Default::default(), &directory)
            .unwrap();
    for record in &records {
        writer.push(record).unwrap();
    }
    let (encoded, identity, statistics) = writer.finish(Vec::new()).unwrap();
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
    assert_eq!(statistics.records, records.len());
    assert_eq!(statistics.unique_coefficients, 3);
    assert!(statistics.unique_coefficient_payload_bytes > 0);
    let mut reader = SavedApplicationCacheFile::decode_generated(
        &encoded,
        &binding,
        &identity,
        Default::default(),
    )
    .unwrap();
    for record in &records {
        assert_eq!(
            reader.lookup(&record.key, &context).unwrap().as_ref(),
            Some(record)
        );
    }
    assert_eq!(reader.statistics().unwrap().state_imports, 1);
    // The original buffered convenience writes the same native row/table
    // grammar. Ambient Symbolica state may add export metadata between calls.
    let (old, old_identity) = SavedApplicationCacheFile::encode_generated(
        &binding,
        &records,
        &context,
        Default::default(),
    )
    .unwrap();
    let mut old_reader = SavedApplicationCacheFile::decode_generated(
        &old,
        &binding,
        &old_identity,
        Default::default(),
    )
    .unwrap();
    for record in &records {
        assert_eq!(
            old_reader.lookup(&record.key, &context).unwrap().as_ref(),
            Some(record)
        );
    }
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn streaming_transport_grants_keep_typed_limit_errors() {
    let (binding, context, records) = fixture();
    let directory = test_directory("limits");
    let limits = SavedApplicationCacheFileLimits {
        max_compressed_bytes: 1,
        ..Default::default()
    };
    let mut writer =
        SavedApplicationCacheFileWriter::new(&binding, &context, limits, &directory).unwrap();
    for record in &records {
        writer.push(record).unwrap();
    }
    let error = writer.finish(Vec::new()).unwrap_err();
    assert_eq!(error.kind(), crate::AppErrorKind::Limit);
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
    let (encoded, _) = SavedApplicationCacheFile::encode_generated(
        &binding,
        &records,
        &context,
        Default::default(),
    )
    .unwrap();
    let error = SavedApplicationCacheFile::decode(&encoded, &binding, limits)
        .err()
        .unwrap();
    assert_eq!(error.kind(), crate::AppErrorKind::Limit);
    std::fs::remove_dir(directory).unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn file_readers_share_one_table_and_keep_private_store_until_last_fork_drops() {
    let (binding, context, records) = fixture();
    let directory = test_directory("file-backed");
    let (encoded, identity) = SavedApplicationCacheFile::encode_generated(
        &binding,
        &records,
        &context,
        Default::default(),
    )
    .unwrap();
    let path = directory.join("source.rrcache.gz");
    std::fs::write(&path, &encoded).unwrap();
    let options = SavedApplicationCacheFileOptions {
        decoded_directory: Some(directory.clone()),
        ..Default::default()
    };
    let mut reader =
        SavedApplicationCacheFile::open_with_options(&path, &binding, Some(&identity), &options)
            .unwrap();
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    assert_eq!(reader.statistics().unwrap().state_imports, 0);
    let mut fork = reader.fork_reader();
    assert!(Arc::ptr_eq(&reader.chunk, &fork.chunk));
    assert_eq!(
        reader.lookup(&records[0].key, &context).unwrap(),
        Some(records[0].clone())
    );
    assert_eq!(
        fork.lookup(&records[1].key, &context).unwrap(),
        Some(records[1].clone())
    );
    assert_eq!(reader.statistics().unwrap().state_imports, 1);
    assert_eq!(fork.statistics().unwrap().state_imports, 1);
    assert_eq!(reader.statistics().unwrap().decoded_coefficients, 3);
    drop(reader);
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    assert_eq!(
        fork.lookup(&records[0].key, &context).unwrap(),
        Some(records[0].clone())
    );
    drop(fork);
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    assert_eq!(std::fs::read(&path).unwrap(), encoded);
    // Failed mapped openings never retain a decoded temporary file.
    let wrong = SavedApplicationCacheFileIdentity {
        decoded_blake3: [0; 32],
        ..identity
    };
    assert!(
        SavedApplicationCacheFile::open_with_options(&path, &binding, Some(&wrong), &options)
            .is_err()
    );
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

fn test_directory(label: &str) -> std::path::PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "rustred-file-{label}-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    directory
}
