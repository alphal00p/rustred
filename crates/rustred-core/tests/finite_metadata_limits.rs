//! Public-codec resource regressions; no integral solving or CAS is involved.
use rustred::persistence::finite_work::{
    FRAME_LIMIT, GeneratedWorkErrorKind, decode_metadata_frame, encode_metadata_frame,
};

const MAGIC: &[u8] = b"FINITE-RESOURCE-TEST\0";
const LARGER: usize = FRAME_LIMIT + 4096;

#[test]
fn user_frame_budget_can_exceed_64_mib_without_changing_encoding() {
    let value = vec![3u8; FRAME_LIMIT + 1];
    let encoded = encode_metadata_frame(MAGIC, &value, LARGER).unwrap();
    assert!(encoded.len() > FRAME_LIMIT);
    assert!(decode_metadata_frame::<Vec<u8>>(MAGIC, &encoded, FRAME_LIMIT).is_err());
    assert_eq!(
        decode_metadata_frame::<Vec<u8>>(MAGIC, &encoded, LARGER).unwrap(),
        value
    );
}

#[test]
fn runtime_budget_matches_bincode_nested_allocation_accounting() {
    for size in [0, 1, 64, LARGER / 8 - 4, LARGER / 8 + 1] {
        let input = vec![vec![0u64; size]];
        let native = bincode::encode_to_vec(&input, bincode::config::standard()).unwrap();
        let reference = bincode::decode_from_slice::<Vec<Vec<u64>>, _>(
            &native,
            bincode::config::standard().with_limit::<LARGER>(),
        );
        let framed = encode_metadata_frame(MAGIC, &input, LARGER).unwrap();
        let actual = decode_metadata_frame::<Vec<Vec<u64>>>(MAGIC, &framed, LARGER);
        assert_eq!(actual.is_ok(), reference.is_ok(), "nested length {size}");
        if let Ok(value) = actual {
            assert_eq!(value, input);
        }
    }
}

#[test]
fn large_user_grants_do_not_disable_hostile_length_or_integrity_checks() {
    // Vec lengths use the same unsigned varint as usize. This header must be
    // rejected by bincode's allocation accounting before reserving the vector.
    let framed = encode_metadata_frame(MAGIC, &(usize::MAX / 8), LARGER).unwrap();
    assert_eq!(
        decode_metadata_frame::<Vec<u64>>(MAGIC, &framed, LARGER)
            .unwrap_err()
            .kind,
        GeneratedWorkErrorKind::Resource
    );
    let framed = encode_metadata_frame(MAGIC, &(7u8, 8u8), LARGER).unwrap();
    assert_eq!(
        decode_metadata_frame::<u8>(MAGIC, &framed, LARGER)
            .unwrap_err()
            .kind,
        GeneratedWorkErrorKind::Corrupt
    );
    let mut changed = encode_metadata_frame(MAGIC, &7u8, LARGER).unwrap();
    changed[MAGIC.len()] ^= 1;
    assert_eq!(
        decode_metadata_frame::<u8>(MAGIC, &changed, LARGER)
            .unwrap_err()
            .kind,
        GeneratedWorkErrorKind::Corrupt
    );
}
