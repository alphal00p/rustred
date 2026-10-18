//! Encoded permissions are independent of actual native decoder allocations.
use rustred::persistence::finite_work::{
    GeneratedWorkErrorKind, decode_metadata_frame, decode_metadata_frame_with_allocation_limit,
    encode_metadata_frame,
};

#[test]
fn unlimited_encoded_permission_does_not_disable_explicit_decoder_budget() {
    let value = vec![vec![0i64; 1024]; 3];
    let encoded = encode_metadata_frame(b"metadata-budget", &value, usize::MAX).unwrap();
    assert!(encoded.len() < 4096);
    let failed = decode_metadata_frame_with_allocation_limit::<Vec<Vec<i64>>>(
        b"metadata-budget",
        &encoded,
        usize::MAX,
        4096,
    )
    .unwrap_err();
    assert_eq!(failed.kind, GeneratedWorkErrorKind::Resource);
    assert_eq!(
        decode_metadata_frame_with_allocation_limit::<Vec<Vec<i64>>>(
            b"metadata-budget",
            &encoded,
            usize::MAX,
            128 * 1024
        )
        .unwrap(),
        value
    );
    assert_eq!(
        decode_metadata_frame::<Vec<Vec<i64>>>(b"metadata-budget", &encoded, 4096).unwrap(),
        value
    );
}

#[test]
fn independent_decoder_budget_preserves_frame_digest_length_and_trailing_checks() {
    let encoded = encode_metadata_frame(b"metadata-budget", &vec![4u64; 16], 4096).unwrap();
    assert!(
        decode_metadata_frame_with_allocation_limit::<Vec<u64>>(
            b"metadata-budget",
            &encoded,
            encoded.len() - 1,
            usize::MAX
        )
        .is_err()
    );
    let mut corrupt = encoded.clone();
    corrupt[20] ^= 1;
    assert_eq!(
        decode_metadata_frame_with_allocation_limit::<Vec<u64>>(
            b"metadata-budget",
            &corrupt,
            usize::MAX,
            4096
        )
        .unwrap_err()
        .kind,
        GeneratedWorkErrorKind::Corrupt
    );
    assert!(
        decode_metadata_frame_with_allocation_limit::<u64>(
            b"metadata-budget",
            &encoded,
            usize::MAX,
            4096
        )
        .is_err()
    );
}
