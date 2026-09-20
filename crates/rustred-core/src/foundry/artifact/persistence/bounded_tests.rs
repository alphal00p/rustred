//! Real bounded owners and structural/canonical cold-load controls.

use std::sync::Arc;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::foundry::artifact::SourcePortAudit;
use crate::persistence::{CoefficientId, CoefficientTableBuilder};
use crate::reduction::{Reducer, ReductionError};
use crate::solver::{SectorConfig, SectorSolveOptions, SectorSolver, SourceSystem};

use super::*;

pub(in crate::foundry::artifact) fn tadpole(degree: u64) -> ClosedArtifact {
    let context = CoefficientContext::new(["d"]);
    let family = IntegralFamily::new(
        "bounded-native-tadpole",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            context.integer(-1),
            vec![context.one()],
        )],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap();
    let zeros: Arc<[[bool; 1]]> = Arc::from([[false]]);
    let sources = SourceSystem::from_family(&family).unwrap();
    let solution = SectorSolver::new(
        &sources,
        [true],
        SectorConfig {
            zero_sectors: zeros.clone(),
            ..Default::default()
        },
    )
    .unwrap()
    .solve_sector(SectorSolveOptions::default())
    .unwrap();
    SourcePortAudit::try_new(&family, zeros)
        .unwrap()
        .install_complete_through_total_excess(family, [([true], None, solution)], degree)
        .unwrap()
}

fn record(degree: u64, root: &[bool], degrees: &[(&[bool], u64)]) -> Vec<u8> {
    let mut writer = Writer::new(Default::default());
    writer.u32(1).unwrap();
    writer.u8(1).unwrap();
    writer.u64(degree).unwrap();
    encode_bool_slice(&mut writer, root).unwrap();
    writer.usize(degrees.len(), "scope sectors").unwrap();
    for (sector, degree) in degrees {
        encode_bool_slice(&mut writer, sector).unwrap();
        writer.u64(*degree).unwrap();
    }
    writer.finish()
}

fn replace(bytes: &[u8], tag: SectionTag, replacement: &[u8], poison: bool) -> Vec<u8> {
    let envelope = inspect_program(bytes, Default::default()).unwrap();
    let sections: Vec<_> = envelope
        .sections()
        .iter()
        .map(|section| BinarySection {
            tag: section.tag,
            bytes: if section.tag == tag {
                replacement
            } else if poison && section.tag == SectionTag::SYMBOLICA_STATE {
                b"not native State"
            } else {
                section.bytes
            },
        })
        .collect();
    encode_program(envelope.kind(), &sections, Default::default()).unwrap()
}

pub(in crate::foundry::artifact) fn with_uniform_successor_degree(
    bytes: &[u8],
    degree: u64,
) -> Vec<u8> {
    let envelope = inspect_program(bytes, Default::default()).unwrap();
    let scope = bounded::decode(
        envelope.section(SectionTag::CERTIFICATE).unwrap(),
        Default::default(),
    )
    .unwrap();
    let degrees: Vec<_> = scope
        .degrees
        .iter()
        .map(|(sector, _)| (sector.active_bits(), degree))
        .collect();
    replace(
        bytes,
        SectionTag::CERTIFICATE,
        &record(scope.entry_degree, scope.root.active_bits(), &degrees),
        false,
    )
}

#[test]
fn bounded_native_k1_roundtrip_preserves_scope_sources_poles_and_entry_admission() {
    let artifact = tadpole(3);
    let bytes = artifact.encode_durable().unwrap();
    assert_eq!(
        inspect_program(&bytes, Default::default()).unwrap().kind(),
        BinaryProgramKind::BoundedCertified
    );
    let cold = ClosedArtifact::decode_durable(&bytes).unwrap();
    assert!(!cold.is_complete_unit_mass_vacuum());
    assert_eq!(
        cold.total_excess_scope()
            .unwrap()
            .max_entry_total_excess_degree(),
        3
    );
    assert_eq!(
        cold.total_excess_scope().unwrap().successor_degrees(),
        artifact.total_excess_scope().unwrap().successor_degrees()
    );
    assert_eq!(cold.source_relations(), artifact.source_relations());
    assert_eq!(
        cold.rule_cells()[0].guards(),
        artifact.rule_cells()[0].guards()
    );
    assert!(
        equivalent_generated_programs(&bytes, &cold.encode_durable().unwrap(), Default::default())
            .unwrap()
    );
    assert!(ClosedArtifact::decode_complete_unit_mass_vacuum(&bytes).is_err());
    let mut expected = Reducer::new(&artifact).unwrap();
    let mut actual = Reducer::new(&cold).unwrap();
    for power in [-3, 0, 1, 2, 4] {
        let key = IntegralKey::try_new([power]).unwrap();
        let left = actual
            .reduce_with_common_mass_squared(&key, &cold.coefficient_context().integer(3))
            .unwrap();
        let right = expected
            .reduce_with_common_mass_squared(&key, &artifact.coefficient_context().integer(3))
            .unwrap();
        assert_eq!(left, right);
        for (key, value) in left.terms() {
            assert_eq!(
                value.numerator.variables(),
                right.terms()[key].numerator.variables()
            );
            assert_eq!(
                value.denominator.variables(),
                right.terms()[key].denominator.variables()
            );
        }
    }
    for power in [-4, 5] {
        let before = actual.statistics();
        assert_eq!(
            actual.reduce_unit_mass(&IntegralKey::try_new([power]).unwrap()),
            Err(ReductionError::OutsideCertifiedTotalExcessDomain { maximum: 3 })
        );
        assert_eq!(actual.statistics(), before);
    }
}

#[test]
fn bounded_native_zero_parent_terminal_only_owner_replays_exact_cover() {
    let artifact = tadpole(0);
    assert!(artifact.rule_cells().is_empty());
    let bytes = artifact.encode_durable().unwrap();
    let cold = ClosedArtifact::decode_durable(&bytes).unwrap();
    assert!(cold.rule_cells().is_empty());
    assert_eq!(cold.masters(), artifact.masters());
    // The same terminal-only body does not prove even one additional dot.
    let widened = replace(
        &bytes,
        SectionTag::CERTIFICATE,
        &record(1, &[true], &[(&[true], 1)]),
        false,
    );
    assert!(ClosedArtifact::decode_durable(&widened).is_err());
}

#[test]
fn bounded_native_scope_preflight_rejects_poisoned_payload_before_state_import() {
    let bytes = tadpole(3).encode_durable().unwrap();
    let valid = record(3, &[true], &[(&[true], 3)]);
    let mut wrong_version = valid.clone();
    wrong_version[0] = 2;
    let mut wrong_convention = valid.clone();
    wrong_convention[4] = 2;
    let mut trailing = valid.clone();
    trailing.push(0);
    let mut non_boolean = valid.clone();
    non_boolean[21] = 2;
    for scope in [
        wrong_version,
        wrong_convention,
        trailing,
        non_boolean,
        record(3, &[true], &[(&[true], 2)]),
        record(3, &[true], &[(&[true], 3), (&[true], 3)]),
        record(3, &[true, true], &[(&[true, true], 3)]),
        record(3, &[false], &[(&[false], 3)]),
        record(3, &[true], &[(&[true, false], 3)]),
    ] {
        let changed = replace(&bytes, SectionTag::CERTIFICATE, &scope, true);
        assert!(
            matches!(
                ClosedArtifact::decode_durable(&changed),
                Err(ArtifactPersistenceError::SemanticMismatch { .. })
                    | Err(ArtifactPersistenceError::TrailingBytes { .. })
            ),
            "scope {scope:?}"
        );
    }
}

#[test]
fn bounded_native_scope_kind_sections_and_legacy_algorithm_fail_before_state() {
    let bytes = tadpole(3).encode_durable().unwrap();
    let envelope = inspect_program(&bytes, Default::default()).unwrap();
    for (kind, scope_present) in [
        (BinaryProgramKind::Certified, true),
        (BinaryProgramKind::BoundedCertified, false),
        (BinaryProgramKind::Candidates, true),
    ] {
        let sections: Vec<_> = envelope
            .sections()
            .iter()
            .filter(|section| scope_present || section.tag != SectionTag::CERTIFICATE)
            .map(|section| BinarySection {
                tag: section.tag,
                bytes: if section.tag == SectionTag::SYMBOLICA_STATE {
                    b"poison"
                } else {
                    section.bytes
                },
            })
            .collect();
        let invalid = encode_program(kind, &sections, Default::default()).unwrap();
        assert_eq!(
            ClosedArtifact::decode_durable(&invalid).unwrap_err(),
            ArtifactPersistenceError::SemanticMismatch {
                field: "certified native envelope kind or sections"
            }
        );
    }
    let legacy = super::super::derive_one_loop_unit_mass_tadpole()
        .unwrap()
        .encode_durable()
        .unwrap();
    let legacy_envelope = inspect_program(&legacy, Default::default()).unwrap();
    let scope = record(3, &[true], &[(&[true], 3)]);
    let mut sections = legacy_envelope.sections().to_vec();
    sections[0].bytes = b"poison";
    sections.push(BinarySection {
        tag: SectionTag::CERTIFICATE,
        bytes: &scope,
    });
    let invalid = encode_program(
        BinaryProgramKind::BoundedCertified,
        &sections,
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        ClosedArtifact::decode_durable(&invalid).unwrap_err(),
        ArtifactPersistenceError::SemanticMismatch {
            field: "bounded source-port algorithm"
        }
    );
}

#[test]
fn bounded_native_zero_only_root_remains_explicitly_unsupported_before_state() {
    let bytes = tadpole(0).encode_durable().unwrap();
    let unsupported = replace(
        &bytes,
        SectionTag::CERTIFICATE,
        &record(0, &[false], &[]),
        true,
    );
    assert_eq!(
        ClosedArtifact::decode_durable(&unsupported).unwrap_err(),
        ArtifactPersistenceError::SemanticMismatch {
            field: "zero-only bounded source-port roots are unsupported"
        }
    );
}

#[test]
fn bounded_native_scope_replays_census_and_envelopes_but_accepts_narrower_entry() {
    let bytes = tadpole(3).encode_durable().unwrap();
    for scope in [
        record(3, &[true], &[]),
        record(3, &[true], &[(&[false], 3)]),
        record(3, &[true], &[(&[true], 4)]),
    ] {
        assert!(
            ClosedArtifact::decode_durable(&replace(
                &bytes,
                SectionTag::CERTIFICATE,
                &scope,
                false
            ))
            .is_err()
        );
    }
    let narrow = replace(
        &bytes,
        SectionTag::CERTIFICATE,
        &record(1, &[true], &[(&[true], 3)]),
        false,
    );
    let artifact = ClosedArtifact::decode_durable(&narrow).unwrap();
    assert_eq!(
        artifact
            .total_excess_scope()
            .unwrap()
            .max_entry_total_excess_degree(),
        1
    );
    assert_eq!(
        artifact
            .total_excess_scope()
            .unwrap()
            .successor_degrees()
            .values()
            .copied()
            .collect::<Vec<_>>(),
        [3]
    );
    assert_eq!(
        Reducer::new(&artifact)
            .unwrap()
            .reduce_unit_mass(&IntegralKey::try_new([3]).unwrap()),
        Err(ReductionError::OutsideCertifiedTotalExcessDomain { maximum: 1 })
    );
}

#[test]
fn bounded_native_limits_remain_caller_owned_in_structural_and_replay_phases() {
    let bytes = tadpole(3).encode_durable().unwrap();
    let poisoned = replace(&bytes, SectionTag::SYMBOLICA_STATE, b"poison", false);
    for limits in [
        ArtifactLoadLimits {
            max_index_arity: 0,
            ..Default::default()
        },
        ArtifactLoadLimits {
            max_collection_entries: 0,
            ..Default::default()
        },
        ArtifactLoadLimits {
            cover_replay: ArtifactCoverReplayLimits {
                max_requested_boxes: 1,
                ..Default::default()
            },
            ..Default::default()
        },
        ArtifactLoadLimits {
            cover_replay: ArtifactCoverReplayLimits {
                max_requested_box_coordinate_cells: 1,
                ..Default::default()
            },
            ..Default::default()
        },
    ] {
        assert!(matches!(
            ClosedArtifact::decode_durable_with_limits(&poisoned, limits),
            Err(ArtifactPersistenceError::ResourceLimit { .. })
        ));
    }
    let mut limits = ArtifactLoadLimits::default();
    limits.rule_derivation.max_source_rows = 0;
    assert!(matches!(
        ClosedArtifact::decode_durable_with_limits(&bytes, limits),
        Err(ArtifactPersistenceError::ResourceLimit { .. })
    ));
    let limits = ArtifactEncodingLimits {
        max_collection_entries: 0,
        ..Default::default()
    };
    assert!(matches!(
        tadpole(3).encode_durable_with_limits(limits),
        Err(ArtifactPersistenceError::ResourceLimit { .. })
    ));
}

#[test]
fn bounded_native_complete_witness_rejects_unused_and_reordered_coefficients() {
    let artifact = tadpole(3);
    let bytes = artifact.encode_durable().unwrap();
    let envelope = inspect_program(&bytes, Default::default()).unwrap();
    let table = DecodedCoefficientTable::import_generated(
        envelope.section(SectionTag::SYMBOLICA_STATE).unwrap(),
        envelope.section(SectionTag::COEFFICIENTS).unwrap(),
        Default::default(),
    )
    .unwrap();
    for reverse in [false, true] {
        let mut builder = CoefficientTableBuilder::new(Default::default());
        let indices: Vec<_> = if reverse {
            (0..table.len()).rev().collect()
        } else {
            (0..table.len()).collect()
        };
        for index in indices {
            builder
                .intern(
                    table
                        .coefficient(CoefficientId::try_from_index(index).unwrap())
                        .unwrap(),
                )
                .unwrap();
        }
        if !reverse {
            builder
                .intern(&artifact.coefficient_context().integer(314159))
                .unwrap();
        }
        let native = builder.finish().unwrap();
        let altered = replace(
            &replace(&bytes, SectionTag::SYMBOLICA_STATE, &native.state, false),
            SectionTag::COEFFICIENTS,
            &native.atoms,
            false,
        );
        assert!(ClosedArtifact::decode_durable(&altered).is_err());
    }
}
