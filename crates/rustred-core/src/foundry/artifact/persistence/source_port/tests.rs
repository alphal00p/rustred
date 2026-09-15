//! Corruption tests serialize arithmetic descriptions, never private seals.
use crate::foundry::artifact::source_port::installed_k1_for_codec_test;

use super::super::coefficient::{encode_indexed_coefficient, encode_indexed_polynomial};
use super::super::semantic::{encode_bool_slice, encode_i64_slice};
use super::plans::{CellPlan, ParentPlan};
use super::*;

fn changed_rules(
    artifact: &ClosedArtifact,
    change: impl FnOnce(&mut Vec<ParentPlan>, &mut Vec<CellPlan>),
) -> Vec<u8> {
    let original = artifact.encode_durable().unwrap();
    let mut offset = 16;
    let (start, end) = loop {
        let tag = u16::from_le_bytes(original[offset..offset + 2].try_into().unwrap());
        let length =
            u64::from_le_bytes(original[offset + 2..offset + 10].try_into().unwrap()) as usize;
        if tag == super::super::RULES_SECTION {
            break (offset, offset + 10 + length);
        }
        offset += 10 + length;
    };
    let mut reader = Reader::root(&original[start + 10..end], Default::default()).unwrap();
    let (mut parents, mut cells) = plans::decode(
        &mut reader,
        &artifact.context,
        artifact.source_relations().len(),
    )
    .unwrap();
    reader.finish().unwrap();
    change(&mut parents, &mut cells);
    let mut writer = Writer::new(Default::default());
    writer.u16(plans::COMBINED_ORIGINAL_PLAN).unwrap();
    writer.usize(parents.len(), "parents").unwrap();
    for parent in parents {
        writer.usize(parent.fixed.len(), "fixed").unwrap();
        for fixed in parent.fixed {
            writer.usize(fixed.position(), "axis").unwrap();
            writer.i64(fixed.value()).unwrap();
        }
        writer.usize(parent.requests.len(), "requests").unwrap();
        for (request, coefficient) in parent.requests {
            writer.usize(request.source_ordinal(), "ordinal").unwrap();
            encode_i64_slice(&mut writer, request.offset().values()).unwrap();
            encode_indexed_coefficient(&mut writer, &coefficient).unwrap();
        }
        writer.usize(parent.conditions.len(), "conditions").unwrap();
        for condition in parent.conditions {
            encode_indexed_polynomial(&mut writer, &condition).unwrap();
        }
    }
    writer.usize(cells.len(), "cells").unwrap();
    for cell in cells {
        writer.usize(cell.parent, "parent").unwrap();
        encode_bool_slice(&mut writer, &cell.sector).unwrap();
        plans::encode_box(&mut writer, &cell.application).unwrap();
        writer.usize(cell.rhs.len(), "RHS").unwrap();
        for (shift, coefficient) in cell.rhs {
            encode_i64_slice(&mut writer, shift.values()).unwrap();
            encode_indexed_coefficient(&mut writer, &coefficient).unwrap();
        }
    }
    let replacement = writer.finish();
    let mut changed = original[..start + 2].to_vec();
    changed.extend_from_slice(&(replacement.len() as u64).to_le_bytes());
    changed.extend_from_slice(&replacement);
    changed.extend_from_slice(&original[end..]);
    changed
}

#[test]
fn original_domain_cold_corruption_replays_actual_weights_rhs_and_offsets() {
    use crate::identity::{IntegralShift, TranslatedSourceRequest};
    let artifact = installed_k1_for_codec_test();
    for mutation in 0..3 {
        let bytes = changed_rules(&artifact, |parents, cells| match mutation {
            0 => {
                parents[0].requests[0].1 = artifact
                    .context
                    .sub(&artifact.context.zero(), &parents[0].requests[0].1)
                    .unwrap()
            }
            1 => {
                cells[0].rhs[0].1 = artifact
                    .context
                    .sub(&artifact.context.zero(), &cells[0].rhs[0].1)
                    .unwrap()
            }
            2 => {
                let request = &parents[0].requests[0].0;
                parents[0].requests[0].0 = TranslatedSourceRequest::new(
                    request.source_ordinal(),
                    IntegralShift::try_new([request.offset().values()[0] + 1]).unwrap(),
                );
            }
            _ => unreachable!(),
        });
        assert!(
            matches!(
                ClosedArtifact::decode_durable(&bytes),
                Err(ArtifactPersistenceError::OriginalDomainReplay { .. })
            ),
            "mutation {mutation}"
        );
    }
}

#[test]
fn mathematical_infinity_cannot_be_replaced_by_a_large_finite_carrier_twin() {
    let artifact = installed_k1_for_codec_test();
    let bytes = changed_rules(&artifact, |_, cells| {
        assert_eq!(cells[0].application.upper(), &[None]);
        cells[0].application = crate::foundry::completion::LatticeBox::try_new(
            cells[0].application.lower().to_vec(),
            vec![Some(u64::MAX)],
        )
        .unwrap();
    });
    assert!(matches!(
        ClosedArtifact::decode_durable(&bytes),
        Err(ArtifactPersistenceError::Artifact(
            ArtifactError::UnsupportedClosureShape
        ))
    ));
}

#[test]
fn invalid_parent_references_and_source_ordinals_fail_before_replay() {
    use crate::identity::TranslatedSourceRequest;
    let artifact = installed_k1_for_codec_test();
    let wrong_parent = changed_rules(&artifact, |_, cells| cells[0].parent = 1);
    assert_eq!(
        ClosedArtifact::decode_durable(&wrong_parent).unwrap_err(),
        ArtifactPersistenceError::SemanticMismatch {
            field: "combined cell parent reference"
        }
    );
    let wrong_source = changed_rules(&artifact, |parents, _| {
        parents[0].requests[0].0 =
            TranslatedSourceRequest::new(1, parents[0].requests[0].0.offset().clone());
    });
    assert_eq!(
        ClosedArtifact::decode_durable(&wrong_source).unwrap_err(),
        ArtifactPersistenceError::SemanticMismatch {
            field: "original source ordinal"
        }
    );
}

#[test]
fn valid_parent_reference_cannot_borrow_another_parents_replayed_identity() {
    let artifact = installed_k1_for_codec_test();
    let bytes = changed_rules(&artifact, |parents, cells| {
        assert_eq!(parents.len(), 1);
        assert_eq!(cells.len(), 1);
        let original = &parents[0];
        let mut wrong = ParentPlan {
            fixed: original.fixed.clone(),
            requests: original.requests.clone(),
            conditions: original.conditions.clone(),
        };
        wrong.requests[0].1 = artifact
            .context
            .sub(&artifact.context.zero(), &wrong.requests[0].1)
            .unwrap();
        parents.push(wrong);
        // Both references are in range and occur in canonical first-use order.
        // The first identity remains valid; its proof cannot authorize the
        // unchanged RHS against the second parent's different weighted sum.
        cells.push(CellPlan {
            parent: 1,
            sector: cells[0].sector.clone(),
            application: crate::foundry::completion::LatticeBox::try_new(
                cells[0].application.lower().to_vec(),
                cells[0].application.upper().to_vec(),
            )
            .unwrap(),
            rhs: cells[0].rhs.clone(),
        });
    });
    assert!(matches!(
        ClosedArtifact::decode_durable(&bytes),
        Err(ArtifactPersistenceError::OriginalDomainReplay { .. })
    ));
}

#[test]
fn omitted_stored_poles_are_regenerated_before_canonical_payload_admission() {
    let artifact = installed_k1_for_codec_test();
    let bytes = changed_rules(&artifact, |parents, _| {
        assert!(!parents[0].conditions.is_empty());
        parents[0].conditions.clear();
    });
    // Untrusted omission cannot erase the poles recomputed from the original
    // sources and stored weights/RHS. Canonical reencoding exposes the omission.
    assert_eq!(
        ClosedArtifact::decode_durable(&bytes).unwrap_err(),
        ArtifactPersistenceError::SemanticMismatch {
            field: "combined canonical artifact payload"
        }
    );
}
