use super::super::super::super::anchors::{AnchorRecord, AnchorRef, AnchorScope};
use super::super::super::super::dispatch::Dispatch;
use super::super::super::super::ledger6::Transition;
use super::super::super::tests::{Directory, state};
use super::super::super::{Digest, MergeBoundary};
use super::super::FixedSection;
use super::*;
use std::fs;

fn encoded_anchors() -> (Vec<u8>, u64) {
    let mut state = state(4);
    state.p0 = 1;
    state.k = 1;
    for node in [1, 3] {
        state.ledger.apply(node, Transition::T2Reserve).unwrap();
        state
            .ledger
            .apply(
                node,
                Transition::T4Native {
                    epoch: 1,
                    residual: false,
                    dband: true,
                },
            )
            .unwrap();
        state
            .anchors
            .push(AnchorRecord {
                node,
                kind: AnchorKind::InitialDBand,
                dispatch_version: 0,
                scope: AnchorScope::DBandCut(-2),
                anchors: vec![AnchorRef {
                    anchor: 0,
                    lent: Lent::Full,
                    stamp: None,
                }],
            })
            .unwrap();
    }
    let dispatch = Dispatch::new();
    let boundary = MergeBoundary::borrow(&state, &dispatch, 16).unwrap();
    (
        boundary
            .write_section(Vec::new(), Section::Anchors)
            .unwrap()
            .0,
        2,
    )
}

fn receipt(directory: &Path, section: Section, bytes: &[u8]) -> SectionReceipt {
    fs::write(directory.join(section.filename(1)), bytes).unwrap();
    SectionReceipt {
        generation: 1,
        section,
        digest: Digest {
            bytes: bytes.len() as u64,
            blake3: *blake3::hash(bytes).as_bytes(),
        },
    }
}

#[test]
fn actual_anchor_writer_reuses_existing_codec_one_record_at_a_time() {
    let directory = Directory::new();
    let (bytes, count) = encoded_anchors();
    let file = receipt(&directory.0, Section::Anchors, &bytes);
    let decoded = anchors::<2>(&directory.0, &file, count, 4, 1, 1).unwrap();
    let reference = AnchorMap::decode(&bytes[28..], 2).unwrap();
    assert_eq!(decoded.len(), reference.len());
    assert_eq!(decoded.encode().unwrap(), bytes[28..]);
    for record in &reference {
        let got = decoded.get(record.node).unwrap();
        assert_eq!(got.kind, record.kind);
        assert_eq!(got.dispatch_version, record.dispatch_version);
        assert_eq!(got.d_band(), record.d_band());
    }
}

#[test]
fn variable_residual_and_full_cover_anchors_roundtrip_with_bounded_counts() {
    use super::super::super::super::{anchors::Piece, state::NODE_RESIDUAL};
    let directory = Directory::new();
    let mut state = state(4);
    state.p0 = 1;
    state.k = 3;
    for id in [2, 3] {
        state.ledger.apply(id, Transition::T2Reserve).unwrap();
        state
            .ledger
            .apply(
                id,
                Transition::T4Native {
                    epoch: u64::from(id),
                    residual: true,
                    dband: false,
                },
            )
            .unwrap();
        state.nodes[id as usize] |= NODE_RESIDUAL;
        state
            .anchors
            .push(AnchorRecord {
                node: id,
                kind: AnchorKind::G2Residual,
                dispatch_version: u64::from(id - 1),
                scope: AnchorScope::Residual(if id == 2 {
                    vec![Piece::band(2, Some(-2), Some(2))]
                } else {
                    Vec::new()
                }),
                anchors: vec![AnchorRef {
                    anchor: id - 1,
                    stamp: Some(u64::from(id - 1)),
                    lent: Lent::Full,
                }],
            })
            .unwrap();
    }
    state.counters.g2_records = 2;
    let dispatch = Dispatch::new();
    let original = MergeBoundary::borrow(&state, &dispatch, 16)
        .unwrap()
        .write_section(Vec::new(), Section::Anchors)
        .unwrap()
        .0;
    let file = receipt(&directory.0, Section::Anchors, &original);
    let decoded = anchors::<2>(&directory.0, &file, 2, 4, 1, 3).unwrap();
    assert_eq!(decoded.records(), state.anchors.records());
    for (at, value) in [(46, u32::MAX), (50, u32::MAX), (78, u32::MAX)] {
        let mut bad = original.clone();
        bad[at..at + 4].copy_from_slice(&value.to_le_bytes());
        let file = receipt(&directory.0, Section::Anchors, &bad);
        assert!(anchors::<2>(&directory.0, &file, 2, 4, 1, 3).is_err());
    }
    for end in [0, 27, 37, 61, original.len() - 1] {
        let file = receipt(&directory.0, Section::Anchors, &original[..end]);
        assert!(anchors::<2>(&directory.0, &file, 2, 4, 1, 3).is_err());
    }
}

#[test]
fn anchor_count_shape_version_range_and_provenance_mutations_refuse() {
    let directory = Directory::new();
    let (original, count) = encoded_anchors();
    // EPC6 header28 + anchor header10; each admitted record is48 bytes.
    for (offset, replacement) in [
        (20, u64::MAX.to_le_bytes().to_vec()),
        (28, 3u16.to_le_bytes().to_vec()),
        (30, u64::MAX.to_le_bytes().to_vec()),
        (38, 0u32.to_le_bytes().to_vec()), // anchored node below P0
        (38, 4u32.to_le_bytes().to_vec()), // node outside watermark
        (42, vec![1]),                     // unsupported G2 kind
        (43, vec![1]),                     // reserved flag
        (46, u32::MAX.to_le_bytes().to_vec()), // forged anchor count
        (50, u32::MAX.to_le_bytes().to_vec()), // forged scope length
        (54, 2u64.to_le_bytes().to_vec()), // dispatch after saved epoch
        (62, 1u32.to_le_bytes().to_vec()), // anchor outside protected prefix
        (66, vec![1]),                     // lent low slice instead of full
        (67, vec![1]),                     // anchor padding
        (70, 0u64.to_le_bytes().to_vec()), // unexpected stamp
        (86, 1u32.to_le_bytes().to_vec()), // duplicate node in second record
    ] {
        let mut bytes = original.clone();
        bytes[offset..offset + replacement.len()].copy_from_slice(&replacement);
        let file = receipt(&directory.0, Section::Anchors, &bytes);
        assert!(
            anchors::<2>(&directory.0, &file, count, 4, 1, 1).is_err(),
            "offset {offset}"
        );
    }
    let file = receipt(
        &directory.0,
        Section::Anchors,
        &original[..original.len() - 1],
    );
    assert!(anchors::<2>(&directory.0, &file, count, 4, 1, 1).is_err());
    let mut file = receipt(&directory.0, Section::Anchors, &original);
    file.digest.blake3[0] ^= 1;
    assert!(anchors::<2>(&directory.0, &file, count, 4, 1, 1).is_err());
}

#[test]
fn sparse_frontiers_roundtrip_and_order_zero_count_or_range_mutations_fail() {
    let directory = Directory::new();
    let mut state = state(4);
    state.frontier_counts.insert(1, 2);
    state.frontier_counts.insert(3, 1);
    let dispatch = Dispatch::new();
    let boundary = MergeBoundary::borrow(&state, &dispatch, 16).unwrap();
    let (original, _) = boundary
        .write_section(Vec::new(), Section::Frontiers)
        .unwrap();
    let file = receipt(&directory.0, Section::Frontiers, &original);
    let decoded = FixedSection::<2>::open(&directory.0, &file, 2)
        .unwrap()
        .frontiers(4)
        .unwrap();
    assert_eq!(decoded, state.frontier_counts);
    for (offset, value) in [(28, 4u32), (32, 0), (36, 1)] {
        let mut bytes = original.clone();
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        let file = receipt(&directory.0, Section::Frontiers, &bytes);
        assert!(
            FixedSection::<2>::open(&directory.0, &file, 2)
                .unwrap()
                .frontiers(4)
                .is_err()
        );
    }
}
