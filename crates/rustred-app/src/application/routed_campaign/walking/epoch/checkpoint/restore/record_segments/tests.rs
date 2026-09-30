use super::super::super::tests::Directory;
use super::*;
use crate::application::routed_campaign::walking::epoch::record_store::Sidecar;
use serde_json::{Value, json};
use std::fs;

fn receipt(directory: &Path, bytes: &[u8], count: u64) -> FileRef {
    fs::write(directory.join("registry"), bytes).unwrap();
    FileRef {
        key: "record-segments".into(),
        file: "registry".into(),
        count,
        bytes: bytes.len() as u64,
        blake3: *blake3::hash(bytes).as_bytes(),
    }
}

fn sample(id: u32) -> crate::application::routed_campaign::walking::epoch::records::typed::Record {
    let image =
        crate::application::routed_campaign::walking::queue::CompactDomain::<1>::try_from_domain(
            &crate::application::routed_campaign::walking::queue::Domain {
                phase: crate::application::routed_campaign::walking::queue::Phase::Apply,
                owner: [true],
                lower: vec![0],
                upper: vec![Some(1)],
                rank: Some(2),
                powers: Default::default(),
            },
        )
        .unwrap();
    crate::application::routed_campaign::walking::epoch::records::typed::Record::alias(
        id,
        &image,
        id + 1,
        1,
        false,
    )
}

fn fixture() -> (Directory, Vec<Segment>) {
    let directory = Directory::new();
    let mut sidecar = Sidecar::new(directory.0.clone(), 1);
    sidecar.push(&sample(0)).unwrap();
    sidecar.seal(1, 2).unwrap();
    sidecar.push(&sample(1)).unwrap();
    sidecar.seal(2, 4).unwrap();
    (directory, sidecar.closed().to_vec())
}

#[test]
fn actual_sidecar_registry_roundtrips_in_final_owned_form() {
    let (directory, segments) = fixture();
    let file = receipt(&directory.0, &serde_json::to_vec(&segments).unwrap(), 2);
    // Unreferenced tails are not accepted as an additional committed record.
    fs::write(directory.0.join(record_store::file_name(3)), b"orphan").unwrap();
    assert_eq!(read(&directory.0, &file, 3, 2).unwrap(), segments);
    let file = receipt(&directory.0, b"[]", 0);
    assert!(read(&directory.0, &file, 3, 0).unwrap().is_empty());
    assert!(read(&directory.0, &file, 3, 1).is_err());
}

#[test]
fn descriptor_bound_is_fixed_shape_not_a_record_line_limit() {
    let largest = Segment {
        generation: u64::MAX,
        file: record_store::file_name(u64::MAX),
        first: u64::MAX,
        count: u64::MAX,
        bytes: u64::MAX,
        blake3: "f".repeat(64),
    };
    assert!((serde_json::to_vec(&largest).unwrap().len() as u64) < DESCRIPTOR_BYTES);
    let directory = Directory::new();
    // This registry layer authenticates arbitrary binary bodies; the authority
    // decoder is separately tested. A large body must not borrow descriptor RAM.
    let bytes = vec![b'x'; 1024 * 1024];
    let segment = Segment {
        generation: 1,
        file: record_store::file_name(1),
        first: 0,
        count: 1,
        bytes: bytes.len() as u64,
        blake3: blake3::hash(&bytes).to_hex().to_string(),
    };
    fs::write(directory.0.join(&segment.file), bytes).unwrap();
    let expected = vec![segment];
    let file = receipt(&directory.0, &serde_json::to_vec(&expected).unwrap(), 1);
    assert_eq!(read(&directory.0, &file, 1, 1).unwrap(), expected);
}

#[test]
fn descriptor_counts_ranges_paths_and_tiling_mutations_fail() {
    let (directory, segments) = fixture();
    for (row, key, replacement) in [
        (0, "generation", json!(0)),
        (0, "file", json!("../records-00000000000000000001.bin")),
        (0, "file", json!("domains-00000000000000000001.bin")),
        (0, "first", json!(1)),
        (0, "count", json!(0)),
        (0, "count", json!(u64::MAX)),
        (0, "bytes", json!(0)),
        (0, "blake3", json!("F".repeat(64))),
        (0, "blake3", json!("0".repeat(63))),
        (1, "generation", json!(1)),
        (1, "generation", json!(4)),
        (1, "first", json!(0)),
        (1, "unexpected", json!(true)),
    ] {
        let mut value = serde_json::to_value(&segments).unwrap();
        value[row][key] = replacement;
        let file = receipt(&directory.0, &serde_json::to_vec(&value).unwrap(), 2);
        assert!(read(&directory.0, &file, 3, 2).is_err(), "row{row} {key}");
    }
    let mut file = receipt(&directory.0, &serde_json::to_vec(&segments).unwrap(), 2);
    file.count = u64::MAX;
    assert!(read(&directory.0, &file, 3, 2).is_err());
    file.count = 1;
    assert!(read(&directory.0, &file, 3, 2).is_err());
}

#[test]
fn one_large_descriptor_cannot_borrow_the_whole_registry_budget() {
    let (directory, segments) = fixture();
    let mut value: Value = serde_json::to_value(&segments).unwrap();
    value[0]["blake3"] = "a".repeat(700).into();
    let bytes = serde_json::to_vec(&value).unwrap();
    // Leave room globally, but the individual descriptor exceeds512 bytes.
    let file = receipt(&directory.0, &bytes, 3);
    assert!(file.bytes <= 3 * DESCRIPTOR_BYTES + 2);
    assert!(
        read(&directory.0, &file, 3, 3)
            .unwrap_err()
            .to_string()
            .contains("fixed shape")
    );
}

#[test]
fn registry_and_every_sealed_body_require_complete_length_and_digest() {
    let (directory, segments) = fixture();
    let original = serde_json::to_vec(&segments).unwrap();
    let mut file = receipt(&directory.0, &original, 2);
    file.blake3[0] ^= 1;
    assert!(read(&directory.0, &file, 3, 2).is_err());
    for trailing in [b" true".as_slice(), b"x".as_slice()] {
        let mut bytes = original.clone();
        bytes.extend_from_slice(trailing);
        let file = receipt(&directory.0, &bytes, 2);
        assert!(read(&directory.0, &file, 3, 2).is_err());
    }
    let file = receipt(&directory.0, &original, 2);
    let path = directory.0.join(&segments[1].file);
    let mut bytes = fs::read(&path).unwrap();
    bytes[0] ^= 1;
    fs::write(&path, bytes).unwrap();
    assert!(read(&directory.0, &file, 3, 2).is_err());
}
