use super::*;
use std::sync::Arc;

#[test]
fn root_zero_census_preserves_all_physical_jobs_and_never_marks_omitted_supports_zero() {
    let family = Arc::new(preparation::family(K3, InputFormat::Toml).unwrap());
    let root = [true, true, false];
    let full = preparation::prepare_shared::<3>(Arc::clone(&family), &root, None).unwrap();
    let limited = preparation::prepare_shared_root::<3>(family, &root, None).unwrap();
    assert_eq!(full.sectors, limited.sectors);
    assert!(limited.zeros.iter().all(|mask| !mask[2]));
    let expected: Vec<_> = full.zeros.iter().copied().filter(|mask| !mask[2]).collect();
    assert_eq!(limited.zeros.as_ref(), expected.as_slice());
}

#[test]
fn candidate_shape_has_no_arbitrary_arity_ceiling_and_rejects_empty_families() {
    assert!(preparation::validate_arity(21).is_ok());
    assert!(preparation::validate_arity(0).is_err());
    assert!(preparation::validate_arity(128).is_ok());
    let selected = FamilyCandidatesRequest::parse_selected_sectors(&["1".repeat(21)]).unwrap();
    assert_eq!(selected, vec![vec![true; 21]]);
    assert_eq!(
        super::super::selection::canonical_masks(&[true; 21], Some(&selected), 22).unwrap(),
        Some(selected)
    );
}

#[test]
fn caller_bundle_grants_above_default_are_not_clamped_and_decoder_remains_bounded() {
    let large = CandidateBundleLimits {
        max_bundle_bytes: MAX_CANDIDATE_BUNDLE_BYTES + 1,
        ..Default::default()
    };
    assert_eq!(large.bundle_byte_limit(), MAX_CANDIDATE_BUNDLE_BYTES + 1);
    let data = vec![vec![1u64; 200]; 2];
    let bytes = bincode::encode_to_vec(&data, bincode::config::standard()).unwrap();
    assert!(codec::decode_record::<Vec<Vec<u64>>>(&bytes, bytes.len()).is_err());
    assert_eq!(
        codec::decode_record::<Vec<Vec<u64>>>(&bytes, large.bundle_byte_limit()).unwrap(),
        (data, bytes.len())
    );
    let hostile = bincode::encode_to_vec(usize::MAX / 8, bincode::config::standard()).unwrap();
    assert!(codec::decode_record::<Vec<u64>>(&hostile, 4096).is_err());
}
