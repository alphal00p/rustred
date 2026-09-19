//! Corruption tests serialize arithmetic descriptions, never private seals.
use crate::foundry::artifact::source_port::installed_k1_for_codec_test;
use crate::persistence::{
    BinaryProgramKind, BinarySection, DecodedCoefficientTable, SectionTag, encode_program,
    inspect_program,
};
use std::rc::Rc;

use super::super::coefficient::{encode_indexed_coefficient, encode_indexed_polynomial};
use super::super::semantic::{encode_bool_slice, encode_i64_slice};
use super::plans::{CellPlan, ParentPlan};
use super::*;

fn changed_rules(
    artifact: &ClosedArtifact,
    change: impl FnOnce(&mut Vec<ParentPlan>, &mut Vec<CellPlan>),
) -> Vec<u8> {
    changed_plan(artifact, |_, parents, cells| change(parents, cells))
}

fn changed_plan(
    artifact: &ClosedArtifact,
    change: impl FnOnce(&mut crate::sector::Mask, &mut Vec<ParentPlan>, &mut Vec<CellPlan>),
) -> Vec<u8> {
    let encoded = artifact.encode_durable().unwrap();
    let envelope = inspect_program(&encoded, Default::default()).unwrap();
    let table = Rc::new(
        DecodedCoefficientTable::import_generated(
            envelope.section(SectionTag::SYMBOLICA_STATE).unwrap(),
            envelope.section(SectionTag::COEFFICIENTS).unwrap(),
            Default::default(),
        )
        .unwrap(),
    );
    let original = envelope.section(SectionTag::PROGRAM).unwrap();
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
    let mut reader = Reader::with_table(
        &original[start + 10..end],
        Default::default(),
        table.clone(),
    )
    .unwrap();
    let (mut root, mut parents, mut cells) = plans::decode(
        &mut reader,
        &artifact.context,
        artifact.source_relations().len(),
    )
    .unwrap();
    reader.finish().unwrap();
    change(&mut root, &mut parents, &mut cells);
    let mut writer = Writer::seeded_for_test(&table).unwrap();
    writer.u16(plans::COMBINED_ORIGINAL_PLAN).unwrap();
    encode_bool_slice(&mut writer, root.active_bits()).unwrap();
    writer.usize(parents.len(), "parents").unwrap();
    for parent in parents {
        writer.usize(parent.fixed.len(), "fixed").unwrap();
        for fixed in parent.fixed {
            writer.usize(fixed.position(), "axis").unwrap();
            writer.i64(fixed.value()).unwrap();
        }
        match parent.affine {
            None => writer.u8(0).unwrap(),
            Some(affine) => {
                writer.u8(1).unwrap();
                plans::encode_affine_domain(&mut writer, &affine).unwrap();
            }
        }
        writer
            .usize(parent.affine_exclusions.len(), "affine exclusions")
            .unwrap();
        for excluded in parent.affine_exclusions.iter() {
            plans::encode_affine_domain(&mut writer, excluded).unwrap();
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
    let (replacement, native) = writer.finish_native().unwrap();
    let mut changed = original[..start + 2].to_vec();
    changed.extend_from_slice(&(replacement.len() as u64).to_le_bytes());
    changed.extend_from_slice(&replacement);
    changed.extend_from_slice(&original[end..]);
    encode_program(
        BinaryProgramKind::Certified,
        &[
            BinarySection {
                tag: SectionTag::SYMBOLICA_STATE,
                bytes: &native.state,
            },
            BinarySection {
                tag: SectionTag::COEFFICIENTS,
                bytes: &native.atoms,
            },
            BinarySection {
                tag: SectionTag::PROGRAM,
                bytes: &changed,
            },
        ],
        Default::default(),
    )
    .unwrap()
}

#[test]
fn cold_scope_cannot_be_widened_shrunk_or_rebound_without_new_coverage() {
    use crate::foundry::artifact::source_port::generated_scoped_k3_for_codec_test;
    use crate::sector::Mask;

    let scoped = generated_scoped_k3_for_codec_test([true, true, false]);
    for new_root in [
        [true, true, true],
        [false, true, false],
        [true, false, true], // Same cardinality, different physical axes.
    ] {
        let bytes = changed_plan(&scoped, |root, _, _| {
            *root = Mask::try_new(new_root).unwrap();
        });
        assert!(
            ClosedArtifact::decode_durable(&bytes).is_err(),
            "forged root {new_root:?}"
        );
    }
    let wrong_arity = changed_plan(&scoped, |root, _, _| {
        *root = Mask::try_new([true, false]).unwrap();
    });
    assert_eq!(
        ClosedArtifact::decode_durable(&wrong_arity).unwrap_err(),
        ArtifactPersistenceError::SemanticMismatch {
            field: "combined root-sector arity"
        }
    );
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
            affine: original.affine.clone(),
            affine_exclusions: original.affine_exclusions.clone(),
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
            field: "complete native artifact witness"
        }
    );
}

fn coupled_exclusion_fixture() -> (
    crate::algebra::IndexedCoefficientContext,
    [Arc<crate::foundry::parametric::AffineApplicationDomain>; 2],
) {
    use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
    use crate::foundry::parametric::AffineApplicationDomain;
    use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};

    let context = IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["d"]),
        "affine-exclusion-codec",
        2,
    )
    .unwrap();
    let domains = [1, 2].map(|slope| {
        let equation = context
            .sub(
                &context.index(0).unwrap(),
                &context
                    .mul(&context.integer(slope), &context.index(1).unwrap())
                    .unwrap(),
            )
            .unwrap()
            .raw()
            .numerator
            .clone();
        let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
            &CoordinateCase::generic(),
            &[equation],
            &[1, 2],
            &[true, true],
        )
        .unwrap() else {
            panic!("fixture must retain a coupled equality")
        };
        Arc::new(AffineApplicationDomain::from_case(&case, &[true, true]).unwrap())
    });
    (context, domains)
}

// Deliberately unsealed arithmetic plans: these codec probes cannot publish a
// rule. Each parent differs only in its exact exclusion, not weights or RHS.
fn exclusion_plan_bytes(
    context: &crate::algebra::IndexedCoefficientContext,
    exclusions: &[Arc<crate::foundry::parametric::AffineApplicationDomain>],
) -> (Vec<u8>, Rc<DecodedCoefficientTable>) {
    let mut writer = Writer::new(Default::default());
    writer.u16(plans::COMBINED_ORIGINAL_PLAN).unwrap();
    encode_bool_slice(&mut writer, &vec![true; context.index_count()]).unwrap();
    writer.usize(exclusions.len(), "parents").unwrap();
    for excluded in exclusions {
        writer.usize(0, "fixed").unwrap();
        writer.u8(0).unwrap();
        writer.usize(1, "affine exclusions").unwrap();
        plans::encode_affine_domain(&mut writer, excluded).unwrap();
        writer.usize(1, "requests").unwrap();
        writer.usize(0, "ordinal").unwrap();
        encode_i64_slice(&mut writer, &vec![0; context.index_count()]).unwrap();
        encode_indexed_coefficient(&mut writer, &context.one()).unwrap();
        writer.usize(0, "conditions").unwrap();
    }
    writer.usize(exclusions.len(), "cells").unwrap();
    for parent in 0..exclusions.len() {
        writer.usize(parent, "parent").unwrap();
        encode_bool_slice(&mut writer, &vec![true; context.index_count()]).unwrap();
        plans::encode_box(
            &mut writer,
            &crate::foundry::completion::LatticeBox::try_new(
                vec![0; context.index_count()],
                vec![None; context.index_count()],
            )
            .unwrap(),
        )
        .unwrap();
        writer.usize(1, "RHS").unwrap();
        let mut shift = vec![0; context.index_count()];
        shift[0] = -1;
        encode_i64_slice(&mut writer, &shift).unwrap();
        encode_indexed_coefficient(&mut writer, &context.one()).unwrap();
    }
    writer.finish_for_test().unwrap()
}

#[test]
fn coupled_exclusion_plans_retain_exact_predicates_and_distinct_parents() {
    let (context, exclusions) = coupled_exclusion_fixture();
    let (bytes, table) = exclusion_plan_bytes(&context, &exclusions);
    let mut reader = Reader::with_table(&bytes, Default::default(), table).unwrap();
    let (_, parents, cells) = plans::decode(&mut reader, &context, 1).unwrap();
    reader.finish().unwrap();
    assert_eq!(parents.len(), 2);
    assert_eq!(
        cells.iter().map(|cell| cell.parent).collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(parents[0].requests, parents[1].requests);
    assert_eq!(cells[0].rhs, cells[1].rhs);
    for (parent, expected) in parents.iter().zip(&exclusions) {
        assert_eq!(parent.affine_exclusions.as_ref(), &[Arc::clone(expected)]);
        let mut original = Writer::new(Default::default());
        plans::encode_affine_domain(&mut original, expected).unwrap();
        let mut loaded = Writer::new(Default::default());
        plans::encode_affine_domain(&mut loaded, &parent.affine_exclusions[0]).unwrap();
        assert_eq!(loaded.finish(), original.finish());
    }
    assert_ne!(parents[0].affine_exclusions, parents[1].affine_exclusions);
    assert!(parents[0].affine_exclusions[0].contains_powers(&[2, 2]));
    assert!(!parents[1].affine_exclusions[0].contains_powers(&[2, 2]));
    assert!(parents[1].affine_exclusions[0].contains_powers(&[4, 2]));
}

#[test]
fn affine_exclusion_codec_rejects_old_tag_wrong_axis_map_and_sector() {
    let (context, exclusions) = coupled_exclusion_fixture();
    let (original, table) = exclusion_plan_bytes(&context, &exclusions[..1]);
    // tag(2), root mask length(8)+bits, parent count(8), fixed count(8),
    // absent-target tag(1), exclusion count(8), then the exact domain payload.
    let domain = 2 + 8 + context.index_count() + 8 + 8 + 1 + 8;
    for (mutation, field) in [
        (0, "combined original plan tag"),
        (1, "affine index-variable positions"),
        (2, "affine index-variable positions"),
        (3, "affine exclusion/cell binding"),
    ] {
        let mut bytes = original.clone();
        match mutation {
            0 => bytes[..2].copy_from_slice(&0x703u16.to_le_bytes()),
            // A parameter position must never masquerade as an integral axis.
            1 => bytes[domain + 28..domain + 36].copy_from_slice(&0u64.to_le_bytes()),
            // An internally valid but permuted map likewise changes ownership.
            2 => {
                bytes[domain + 28..domain + 36].copy_from_slice(&2u64.to_le_bytes());
                bytes[domain + 36..domain + 44].copy_from_slice(&1u64.to_le_bytes());
            }
            3 => bytes[domain + 8] = 0,
            _ => unreachable!(),
        }
        let mut reader = Reader::with_table(&bytes, Default::default(), table.clone()).unwrap();
        assert_eq!(
            plans::decode(&mut reader, &context, 1).err().unwrap(),
            ArtifactPersistenceError::SemanticMismatch { field },
            "mutation {mutation}"
        );
    }
}

#[test]
fn affine_exclusion_codec_checks_count_budget_and_truncated_predicates() {
    let (context, exclusions) = coupled_exclusion_fixture();
    let (bytes, table) = exclusion_plan_bytes(&context, &exclusions[..1]);
    let mut limits = ArtifactLoadLimits::default();
    limits.rule_cells.max_guards = 0;
    let mut reader = Reader::with_table(&bytes, limits, table.clone()).unwrap();
    assert_eq!(
        plans::decode(&mut reader, &context, 1).err().unwrap(),
        ArtifactPersistenceError::ResourceLimit {
            resource: "affine exclusions",
            requested: 1,
            limit: 0,
        }
    );
    let mut encoded = Writer::new(Default::default());
    plans::encode_affine_domain(&mut encoded, &exclusions[0]).unwrap();
    let domain_bytes = encoded.finish();
    let domain = 2 + 8 + context.index_count() + 8 + 8 + 1 + 8;
    for boundary in [
        domain - 8,
        domain - 1,
        domain,
        domain + 8,
        domain + 28,
        domain + domain_bytes.len() - 1,
    ] {
        let mut reader =
            Reader::with_table(&bytes[..boundary], Default::default(), table.clone()).unwrap();
        assert!(
            matches!(
                plans::decode(&mut reader, &context, 1),
                Err(ArtifactPersistenceError::Truncated { .. })
            ),
            "boundary {boundary}"
        );
    }
}

#[test]
fn cold_artifact_rejects_affine_exclusion_axis_and_sector_substitution() {
    use crate::foundry::parametric::AffineApplicationDomain;
    use symbolica::prelude::{Integer, IntegerRing, Matrix};
    let artifact = installed_k1_for_codec_test();
    for (indices, sector, field) in [
        (vec![0], vec![true], "affine index-variable positions"),
        (vec![1], vec![false], "affine exclusion/cell binding"),
    ] {
        // Structural untrusted data, not a solver-issued affine rule. The
        // cold loader must reject these bindings before any replay authority.
        let domain = AffineApplicationDomain::from_persisted(
            sector.into_boxed_slice(),
            vec![None].into_boxed_slice(),
            indices.into_boxed_slice(),
            vec![artifact.context.index(0).unwrap().raw().numerator.clone()].into_boxed_slice(),
            Matrix::from_linear(vec![Integer::from(1), Integer::from(0)], 1, 2, IntegerRing)
                .unwrap(),
            true,
        )
        .unwrap();
        let bytes = changed_rules(&artifact, |parents, _| {
            parents[0].affine_exclusions = Arc::from([Arc::new(domain)]);
        });
        assert_eq!(
            ClosedArtifact::decode_durable(&bytes).unwrap_err(),
            ArtifactPersistenceError::SemanticMismatch { field }
        );
    }
}

#[test]
fn cold_parent_rebuilds_exclusion_matrix_and_chart_from_exact_equations() {
    use crate::foundry::cell::SourceViewBatch;
    use crate::foundry::parametric::AffineApplicationDomain;
    use crate::identity::{IntegralShift, ParametricIbpGenerator};
    use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};
    use symbolica::prelude::{Integer, IntegerRing, Matrix};

    let family = crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..batch.len())
        .map(|ordinal| batch.generate(ordinal))
        .collect();
    let completed = batch.complete(rows).unwrap();
    let translated = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0; 3]).unwrap()],
            Default::default(),
        )
        .unwrap();
    let sources =
        Arc::new(SourceViewBatch::try_select(translated, &[0], Default::default()).unwrap());
    let context = generator.context();
    let first = context.base().variables().len();
    let equation = context
        .sub(&context.index(0).unwrap(), &context.index(1).unwrap())
        .unwrap()
        .raw()
        .numerator
        .clone();
    let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
        &CoordinateCase::generic(),
        &[equation],
        &[first, first + 1, first + 2],
        &[true; 3],
    )
    .unwrap() else {
        panic!("expected a coupled exclusion")
    };
    let correct = AffineApplicationDomain::from_case(&case, &[true; 3]).unwrap();
    for mutation in 0..3 {
        let original = correct.primitive_matrix().unwrap();
        let mut entries = original
            .row_iter()
            .flat_map(|row| row.iter().cloned())
            .collect::<Vec<_>>();
        if mutation == 1 {
            entries[0] += Integer::from(1);
        }
        let matrix = Matrix::from_linear(
            entries,
            original.nrows() as u32,
            original.ncols() as u32,
            IntegerRing,
        )
        .unwrap();
        let domain = AffineApplicationDomain::from_persisted(
            correct.sector().to_vec().into_boxed_slice(),
            correct.fixed().to_vec().into_boxed_slice(),
            correct.indices().to_vec().into_boxed_slice(),
            correct.equations().to_vec().into_boxed_slice(),
            matrix,
            correct.has_integral_chart().unwrap() ^ (mutation == 2),
        )
        .unwrap();
        let (bytes, table) = exclusion_plan_bytes(context, &[Arc::new(domain)]);
        let mut reader = Reader::with_table(&bytes, Default::default(), table).unwrap();
        let (_, mut plans, _) = plans::decode(&mut reader, context, 4).unwrap();
        reader.finish().unwrap();
        let plan = plans.pop().unwrap();
        let result = PreparedOriginalDomain::try_new(
            context,
            Arc::clone(&sources),
            vec![(0, sources.relations()[0].row_id().clone(), context.one())],
            Vec::new(),
            None,
            plan.affine_exclusions,
            Vec::new(),
            Default::default(),
        );
        if mutation == 0 {
            assert!(result.is_ok(), "unchanged decoded witness rejected");
        } else {
            assert!(
                result
                    .err()
                    .unwrap()
                    .to_string()
                    .contains("cached matrix/chart"),
                "mutation {mutation}"
            );
        }
    }
}
