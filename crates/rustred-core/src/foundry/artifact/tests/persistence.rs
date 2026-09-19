use std::ops::Range;

use crate::foundry::parametric::{ParametricRuleLimits, derive_sector_interior_rule};
use crate::persistence::{
    BinaryIoError, BinaryIoLimits, BinarySection, CoefficientId, CoefficientTableBuilder,
    DecodedCoefficientTable, SectionTag, encode_program, equivalent_generated_programs,
    inspect_program,
};
use crate::sector::OrderingPolicy;

use super::error::{ArtifactError, ArtifactPersistenceError};
use super::install::{ClosingArtifactCandidate, install};
use super::model::ClosedArtifact;
use super::one_loop::derive_one_loop_unit_mass_tadpole;
use super::persistence::{ArtifactEncodingLimits, ArtifactLoadLimits};

#[test]
fn durable_encoding_is_deterministic_and_loads_a_sealed_equivalent() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let first = artifact.encode_durable().unwrap();
    let independently_derived = derive_one_loop_unit_mass_tadpole().unwrap();
    let second = independently_derived.encode_durable().unwrap();
    assert!(equivalent_generated_programs(&first, &second, Default::default()).unwrap());
    let loaded = ClosedArtifact::decode_durable(&first).unwrap();
    assert!(
        equivalent_generated_programs(
            &loaded.encode_durable().unwrap(),
            &first,
            Default::default()
        )
        .unwrap()
    );
    assert_eq!(loaded.schema(), artifact.schema());
    assert_eq!(loaded.algorithm_id(), artifact.algorithm_id());
    assert_eq!(loaded.family_fingerprint(), artifact.family_fingerprint());
    assert_eq!(loaded.context_fingerprint(), artifact.context_fingerprint());
    assert_eq!(loaded.source_relations(), artifact.source_relations());
    assert_eq!(loaded.rules(), artifact.rules());
    assert_eq!(loaded.masters(), artifact.masters());
    assert_eq!(loaded.zero_sectors(), artifact.zero_sectors());
    assert_eq!(loaded.validation(), artifact.validation());
    assert!(!loaded.is_complete_unit_mass_vacuum());
    assert!(matches!(
        ClosedArtifact::decode_complete_unit_mass_vacuum(&first),
        Err(ArtifactPersistenceError::SemanticMismatch {
            field: "complete unit-mass vacuum artifact capability"
        })
    ));
}

fn durable_section(bytes: &[u8], wanted_tag: u16) -> Range<usize> {
    let mut offset = native_section(bytes, SectionTag::PROGRAM).start + 16;
    for _ in 0..5 {
        let tag = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
        let len = usize::try_from(u64::from_le_bytes(
            bytes[offset + 2..offset + 10].try_into().unwrap(),
        ))
        .unwrap();
        let start = offset + 10;
        let end = start + len;
        if tag == wanted_tag {
            return start..end;
        }
        offset = end;
    }
    panic!("durable test fixture has no section tag {wanted_tag}")
}

fn native_section(bytes: &[u8], tag: SectionTag) -> Range<usize> {
    let envelope = inspect_program(bytes, Default::default()).unwrap();
    let section = envelope.section(tag).unwrap();
    let start = section.as_ptr() as usize - bytes.as_ptr() as usize;
    start..start + section.len()
}

fn encoded_usize(bytes: &[u8], offset: usize) -> usize {
    usize::try_from(u64::from_le_bytes(
        bytes[offset..offset + 8].try_into().unwrap(),
    ))
    .unwrap()
}

fn encoded_blob(bytes: &[u8], offset: &mut usize) -> Range<usize> {
    let len = encoded_usize(bytes, *offset);
    let start = *offset + 8;
    let end = start + len;
    *offset = end;
    start..end
}

fn metadata_arity_offset(bytes: &[u8]) -> usize {
    let metadata = durable_section(bytes, 1);
    let mut offset = metadata.start;
    encoded_blob(bytes, &mut offset);
    offset
}

fn first_family_coefficient_payload(bytes: &[u8]) -> Range<usize> {
    let family = durable_section(bytes, 2);
    // Six structural counts plus the sole denominator's coefficient count.
    let mut offset = family.start + 7 * 8;
    encoded_blob(bytes, &mut offset); // family name
    encoded_blob(bytes, &mut offset); // loop label
    encoded_blob(bytes, &mut offset); // parameter label
    offset..offset + 5 // rational role + shared native table ID
}

fn minimum_encoding_coefficient_budget(artifact: &ClosedArtifact) -> usize {
    let mut lower = 0;
    let mut upper = ArtifactEncodingLimits::default().max_total_coefficient_bytes;
    assert!(
        artifact
            .encode_durable_with_limits(ArtifactEncodingLimits {
                max_total_coefficient_bytes: upper,
                ..ArtifactEncodingLimits::default()
            })
            .is_ok()
    );
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        let succeeds = artifact
            .encode_durable_with_limits(ArtifactEncodingLimits {
                max_total_coefficient_bytes: middle,
                ..ArtifactEncodingLimits::default()
            })
            .is_ok();
        if succeeds {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    lower
}

fn source_witness(bytes: &[u8]) -> Range<usize> {
    let source = durable_section(bytes, 3);
    let mut offset = source.start + 8 + 2;
    let plan = encoded_blob(bytes, &mut offset);
    let mut plan_offset = plan.start + 8;
    encoded_blob(bytes, &mut plan_offset)
}

fn rule_witness(bytes: &[u8]) -> Range<usize> {
    let rules = durable_section(bytes, 4);
    let mut offset = rules.start + 8 + 2;
    let plan = encoded_blob(bytes, &mut offset);
    let mut plan_offset = plan.start;
    let anchor_len = encoded_usize(bytes, plan_offset);
    plan_offset += 8 + 8 * anchor_len;
    encoded_blob(bytes, &mut plan_offset); // ordering identifier
    encoded_blob(bytes, &mut plan_offset)
}

#[test]
fn durable_loader_rejects_corruption_schema_and_trailing_bytes() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let encoded = artifact.encode_durable().unwrap();

    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        ClosedArtifact::decode_durable(&wrong_magic).unwrap_err(),
        ArtifactPersistenceError::NativeTransport(BinaryIoError::Invalid(
            "magic does not identify a binary program"
        ))
    );

    let mut wrong_schema = encoded.clone();
    let schema_offset = native_section(&encoded, SectionTag::PROGRAM).start + 8;
    wrong_schema[schema_offset..schema_offset + 4].copy_from_slice(&3_u32.to_le_bytes());
    assert_eq!(
        ClosedArtifact::decode_durable(&wrong_schema).unwrap_err(),
        ArtifactPersistenceError::UnsupportedSchema { actual: 3 }
    );

    for version in [1_u32, 2, 4, 5] {
        let mut obsolete_schema = encoded.clone();
        obsolete_schema[schema_offset..schema_offset + 4].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            ClosedArtifact::decode_durable(&obsolete_schema).unwrap_err(),
            ArtifactPersistenceError::UnsupportedSchema { actual: version }
        );
    }

    assert!(matches!(
        ClosedArtifact::decode_durable(&encoded[..encoded.len() - 1]),
        Err(ArtifactPersistenceError::NativeTransport(
            BinaryIoError::Invalid(_)
        ))
    ));

    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        ClosedArtifact::decode_durable(&trailing).unwrap_err(),
        ArtifactPersistenceError::NativeTransport(BinaryIoError::Invalid(
            "trailing bytes after program sections"
        ))
    );
}

#[test]
fn durable_loader_rejects_family_source_rule_and_terminal_corruption() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let encoded = artifact.encode_durable().unwrap();

    let mut family_corruption = encoded.clone();
    let family = durable_section(&family_corruption, 2);
    family_corruption[family.start..family.start + 8].copy_from_slice(&2_u64.to_le_bytes());
    assert_eq!(
        ClosedArtifact::decode_durable(&family_corruption).unwrap_err(),
        ArtifactPersistenceError::SemanticMismatch {
            field: "one-loop family structural prelude",
        }
    );

    let mut source_corruption = encoded.clone();
    let source = source_witness(&source_corruption);
    source_corruption[source.end - 1] ^= 1;
    assert_eq!(
        ClosedArtifact::decode_durable(&source_corruption).unwrap_err(),
        ArtifactPersistenceError::SemanticMismatch {
            field: "source derivation witness",
        }
    );

    let mut rule_corruption = encoded.clone();
    let rule = rule_witness(&rule_corruption);
    rule_corruption[rule.end - 1] ^= 1;
    assert_eq!(
        ClosedArtifact::decode_durable(&rule_corruption).unwrap_err(),
        ArtifactPersistenceError::SemanticMismatch {
            field: "rule snapshot",
        }
    );

    let mut terminal_corruption = encoded;
    let terminal = durable_section(&terminal_corruption, 5);
    assert_eq!(terminal_corruption[terminal.end - 1], 1);
    terminal_corruption[terminal.end - 1] = 2;
    assert!(matches!(
        ClosedArtifact::decode_durable(&terminal_corruption),
        Err(ArtifactPersistenceError::UnsupportedFeature { .. })
    ));
}

#[test]
fn durable_codec_enforces_encode_and_load_limits_before_work() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let encode_limits = ArtifactEncodingLimits {
        max_artifact_bytes: 1,
        ..ArtifactEncodingLimits::default()
    };
    assert!(matches!(
        artifact.encode_durable_with_limits(encode_limits),
        Err(ArtifactPersistenceError::ResourceLimit {
            resource: "encoded artifact bytes",
            ..
        })
    ));

    let encoded = artifact.encode_durable().unwrap();
    let load_limits = ArtifactLoadLimits {
        max_artifact_bytes: encoded.len() - 1,
        ..ArtifactLoadLimits::default()
    };
    assert_eq!(
        ClosedArtifact::decode_durable_with_limits(&encoded, load_limits).unwrap_err(),
        ArtifactPersistenceError::ResourceLimit {
            resource: "program bytes",
            requested: encoded.len(),
            limit: encoded.len() - 1,
        }
    );

    let coefficient_limits = ArtifactLoadLimits {
        max_coefficient_bytes: 1,
        ..ArtifactLoadLimits::default()
    };
    assert!(matches!(
        ClosedArtifact::decode_durable_with_limits(&encoded, coefficient_limits),
        Err(ArtifactPersistenceError::ResourceLimit { .. })
    ));

    let atoms = native_section(&encoded, SectionTag::COEFFICIENTS);
    let aggregate_limits = ArtifactLoadLimits {
        max_total_coefficient_bytes: atoms.len() - 1,
        ..ArtifactLoadLimits::default()
    };
    assert!(matches!(
        ClosedArtifact::decode_durable_with_limits(&encoded, aggregate_limits),
        Err(ArtifactPersistenceError::ResourceLimit {
            resource: "coefficient table bytes",
            ..
        })
    ));

    let source = source_witness(&encoded);
    let rule = rule_witness(&encoded);
    let one_witness = source.len().max(rule.len());
    let witness_load_limits = ArtifactLoadLimits {
        max_total_witness_bytes: one_witness,
        ..ArtifactLoadLimits::default()
    };
    assert!(matches!(
        ClosedArtifact::decode_durable_with_limits(&encoded, witness_load_limits),
        Err(ArtifactPersistenceError::ResourceLimit {
            resource: "aggregate semantic witness bytes",
            ..
        })
    ));

    let witness_encode_limits = ArtifactEncodingLimits {
        max_total_witness_bytes: one_witness,
        ..ArtifactEncodingLimits::default()
    };
    assert!(matches!(
        artifact.encode_durable_with_limits(witness_encode_limits),
        Err(ArtifactPersistenceError::ResourceLimit {
            resource: "aggregate semantic witness bytes",
            ..
        })
    ));
}

#[test]
fn unique_native_table_budget_is_shared_across_family_and_semantic_replay() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let encoded = artifact.encode_durable().unwrap();
    let total = minimum_encoding_coefficient_budget(&artifact);
    let table_bytes = native_section(&encoded, SectionTag::COEFFICIENTS).len();
    let one_below = total - 1;

    assert_eq!(total, table_bytes);
    assert!(matches!(
        ClosedArtifact::decode_durable_with_limits(
            &encoded,
            ArtifactLoadLimits {
                max_total_coefficient_bytes: one_below,
                ..ArtifactLoadLimits::default()
            }
        ),
        Err(ArtifactPersistenceError::ResourceLimit {
            resource: "coefficient table bytes",
            requested,
            limit,
        }) if requested == total && limit == one_below
    ));

    let loaded = ClosedArtifact::decode_durable_with_limits(
        &encoded,
        ArtifactLoadLimits {
            max_total_coefficient_bytes: total,
            ..ArtifactLoadLimits::default()
        },
    )
    .unwrap();
    assert!(
        equivalent_generated_programs(
            &loaded.encode_durable().unwrap(),
            &encoded,
            Default::default()
        )
        .unwrap()
    );
}

#[test]
fn durable_loader_rejects_a_self_consistent_forged_source_and_rule_pair() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let ClosedArtifact {
        schema,
        arity,
        ordering,
        supported_root_power_bounds,
        family,
        context,
        source_relations,
        masters,
        zero_sectors,
        common_mass_homogeneity,
        ..
    } = artifact;
    let factor = context.integer(2);
    let forged_source = source_relations[0]
        .scaled_for_artifact_forgery_test(&context, &factor)
        .unwrap();
    assert_ne!(forged_source, source_relations[0]);
    let forged_sources = vec![forged_source];
    let forged_rule = derive_sector_interior_rule(
        &context,
        &forged_sources,
        &[1],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();
    let forged = install(ClosingArtifactCandidate {
        schema,
        algorithm_id: "rustred.generated.one-loop-unit-mass-tadpole.v1",
        arity,
        ordering,
        supported_root_power_bounds,
        family,
        context,
        source_relations: forged_sources,
        rules: vec![forged_rule],
        rule_cells: Vec::new(),
        canonicalizer: None,
        dependencies: Vec::new(),
        factorization_rules: Vec::new(),
        masters,
        zero_sectors,
        common_mass_homogeneity,
    })
    .unwrap();
    let encoded = forged.encode_durable().unwrap();
    // Source regeneration requests an original coefficient that the scaled
    // forgery did not retain. The lookup-only table rejects it before witness
    // byte comparison; replay cannot append a matching value to repair input.
    assert_eq!(
        ClosedArtifact::decode_durable(&encoded).unwrap_err(),
        ArtifactPersistenceError::SemanticMismatch {
            field: "replayed coefficient absent from native table",
        }
    );
}

#[test]
fn native_references_reject_wrong_roles_and_unknown_ids() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let encoded = artifact.encode_durable().unwrap();
    let coefficient = first_family_coefficient_payload(&encoded);

    // References are typed IDs, not an alternative expression grammar.
    let mut expression_like = encoded.clone();
    expression_like[coefficient.start] = b'(';
    assert_eq!(
        ClosedArtifact::decode_durable(&expression_like).unwrap_err(),
        ArtifactPersistenceError::InvalidCoefficient {
            field: "family dimension",
        }
    );

    let mut unknown_id = encoded;
    unknown_id[coefficient.start + 1..coefficient.end].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        ClosedArtifact::decode_durable(&unknown_id),
        Err(ArtifactPersistenceError::NativeTransport(
            BinaryIoError::Invalid(_)
        ))
    ));
}

fn replace_native_value(
    bytes: &[u8],
    changed_id: CoefficientId,
    replacement: &crate::algebra::Coefficient,
) -> Vec<u8> {
    let limits = BinaryIoLimits::default();
    let envelope = inspect_program(bytes, limits).unwrap();
    let table = DecodedCoefficientTable::import_generated(
        envelope.section(SectionTag::SYMBOLICA_STATE).unwrap(),
        envelope.section(SectionTag::COEFFICIENTS).unwrap(),
        limits,
    )
    .unwrap();
    let mut builder = CoefficientTableBuilder::new(limits);
    for index in 0..table.len() {
        let id = CoefficientId::try_from_index(index).unwrap();
        let value = if id == changed_id {
            replacement
        } else {
            table.coefficient(id).unwrap()
        };
        assert_eq!(
            builder.intern(value).unwrap(),
            id,
            "test replacement must preserve IDs"
        );
    }
    let encoded = builder.finish().unwrap();
    let sections = envelope
        .sections()
        .iter()
        .map(|section| BinarySection {
            tag: section.tag,
            bytes: if section.tag == SectionTag::SYMBOLICA_STATE {
                &encoded.state
            } else if section.tag == SectionTag::COEFFICIENTS {
                &encoded.atoms
            } else {
                section.bytes
            },
        })
        .collect::<Vec<_>>();
    encode_program(envelope.kind(), &sections, limits).unwrap()
}

#[test]
fn generated_native_values_remain_bound_to_exact_limits_and_context() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let encoded = artifact.encode_durable().unwrap();
    let coefficient = first_family_coefficient_payload(&encoded);
    let id = CoefficientId::try_from_index(u32::from_le_bytes(
        encoded[coefficient.start + 1..coefficient.end]
            .try_into()
            .unwrap(),
    ) as usize)
    .unwrap();
    let dimension = artifact
        .family()
        .coefficient_context()
        .parameter("d")
        .unwrap();
    // Serialize a valid generated native Atom, not a deliberately malformed
    // upstream binary payload: native import has a trusted-provenance contract.
    let huge_power = replace_native_value(&encoded, id, &dimension.pow(65));
    let mut strict = ArtifactLoadLimits::default();
    strict.family.exact_algebra.max_exponent = 64;
    let error = ClosedArtifact::decode_durable_with_limits(&huge_power, strict).unwrap_err();
    assert!(error.to_string().contains("65"), "{error}");
    assert!(error.to_string().contains("64"), "{error}");

    let foreign = crate::algebra::CoefficientContext::new(["foreign_native_dimension"]);
    let wrong_map = replace_native_value(
        &encoded,
        id,
        &foreign.parameter("foreign_native_dimension").unwrap(),
    );
    assert_eq!(
        ClosedArtifact::decode_durable(&wrong_map).unwrap_err(),
        ArtifactPersistenceError::InvalidCoefficient {
            field: "family dimension",
        }
    );
}

#[test]
fn one_loop_algorithm_shape_is_rejected_before_family_construction_and_replay() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let encoded = artifact.encode_durable().unwrap();

    let mut wrong_arity = encoded.clone();
    let arity = metadata_arity_offset(&wrong_arity);
    wrong_arity[arity..arity + 8].copy_from_slice(&2_u64.to_le_bytes());
    assert_eq!(
        ClosedArtifact::decode_durable(&wrong_arity).unwrap_err(),
        ArtifactPersistenceError::SemanticMismatch {
            field: "one-loop algorithm arity",
        }
    );

    let mut wrong_shape_and_hostile_coefficient = encoded.clone();
    let family = durable_section(&wrong_shape_and_hostile_coefficient, 2);
    wrong_shape_and_hostile_coefficient[family.start..family.start + 8]
        .copy_from_slice(&2_u64.to_le_bytes());
    let coefficient = first_family_coefficient_payload(&wrong_shape_and_hostile_coefficient);
    wrong_shape_and_hostile_coefficient[coefficient.start] = b'(';
    assert_eq!(
        ClosedArtifact::decode_durable(&wrong_shape_and_hostile_coefficient).unwrap_err(),
        ArtifactPersistenceError::SemanticMismatch {
            field: "one-loop family structural prelude",
        }
    );

    let mut excessive_denominator_shape = encoded;
    let family = durable_section(&excessive_denominator_shape, 2);
    let denominator_coefficient_count = family.start + 6 * 8;
    excessive_denominator_shape[denominator_coefficient_count..denominator_coefficient_count + 8]
        .copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(matches!(
        ClosedArtifact::decode_durable(&excessive_denominator_shape),
        Err(ArtifactPersistenceError::ResourceLimit {
            resource: "denominator coefficients",
            ..
        })
    ));
}

#[test]
fn durable_load_threads_explicit_family_source_and_rule_policies() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let encoded = artifact.encode_durable().unwrap();

    let mut family_limited = ArtifactLoadLimits::default();
    family_limited.family.max_scalar_products = 0;
    // The structural preflight rejects this before family construction or
    // source replay, retaining the caller's exact family limit. Native table
    // restoration has already happened under its own bounded framing policy.
    assert_eq!(
        ClosedArtifact::decode_durable_with_limits(&encoded, family_limited).unwrap_err(),
        ArtifactPersistenceError::ResourceLimit {
            resource: "family scalar products",
            requested: 1,
            limit: 0,
        }
    );

    let mut source_limited = ArtifactLoadLimits::default();
    source_limited
        .source_generation
        .context_limits
        .max_index_variables = 0;
    assert!(matches!(
        ClosedArtifact::decode_durable_with_limits(&encoded, source_limited),
        Err(ArtifactPersistenceError::Artifact(ArtifactError::Identity(
            _
        )))
    ));

    let mut relation_limited = ArtifactLoadLimits::default();
    relation_limited
        .source_generation
        .relation_limits
        .arithmetic
        .exact_algebra
        .max_polynomial_terms = 0;
    assert!(matches!(
        ClosedArtifact::decode_durable_with_limits(&encoded, relation_limited),
        Err(ArtifactPersistenceError::Artifact(ArtifactError::Identity(
            _
        )))
    ));

    let mut rule_limited = ArtifactLoadLimits::default();
    rule_limited.rule_derivation.max_source_rows = 0;
    assert!(matches!(
        ClosedArtifact::decode_durable_with_limits(&encoded, rule_limited),
        Err(ArtifactPersistenceError::Artifact(
            ArtifactError::ParametricRule(_)
        ))
    ));
}
