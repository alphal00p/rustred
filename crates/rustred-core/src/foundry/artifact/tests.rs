use std::collections::BTreeSet;
use std::ops::Range;

use crate::algebra::{CoefficientContext, ExactAlgebraLimits};
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::foundry::parametric::{ParametricRuleLimits, derive_sector_interior_rule};
use crate::identity::ParametricIbpGenerator;
use crate::sector::{Mask, OrderingPolicy};

use super::error::{ArtifactError, ArtifactPersistenceError};
use super::install::{ClosingArtifactCandidate, install};
use super::model::{
    ArtifactSchemaVersion, ClosedArtifact, CommonMassHomogeneityProof, ZeroSectorTerminal,
    ZeroTerminalProof,
};
use super::one_loop::derive_one_loop_unit_mass_tadpole;
use super::persistence::{ArtifactEncodingLimits, ArtifactLoadLimits};

#[test]
fn generated_tadpole_installs_one_exact_closed_partition() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();

    assert_eq!(artifact.schema(), ArtifactSchemaVersion::V1);
    assert_eq!(artifact.arity(), 1);
    assert_eq!(
        artifact.algorithm_id(),
        "rustred.generated.one-loop-unit-mass-tadpole.v1"
    );
    assert_eq!(artifact.coefficient_context().parameter_names(), ["d"]);
    assert_eq!(artifact.masters().len(), 1);
    assert_eq!(artifact.masters().first().unwrap().powers(), [1]);
    assert_eq!(artifact.zero_sectors().len(), 1);
    assert_eq!(artifact.zero_sectors()[0].sector().active_bits(), [false]);
    assert_eq!(
        artifact.zero_sectors()[0].proof(),
        ZeroTerminalProof::ScalelessVacuumPolynomial
    );
    assert_eq!(artifact.source_relations().len(), 1);
    assert_eq!(artifact.rules().len(), 1);
    assert_eq!(
        artifact.common_mass_homogeneity(),
        Some(CommonMassHomogeneityProof::UniformVacuumMassSquared)
    );
    assert_eq!(artifact.rules()[0].pivot().values(), [1]);
    assert_eq!(artifact.rules()[0].right_hand_side().len(), 1);
    assert_eq!(
        artifact.rules()[0].right_hand_side()[0].shift().values(),
        [0]
    );
    assert!(artifact.rules()[0].right_hand_side()[0].descent().verify());

    let witness = artifact.validation();
    assert_eq!(witness.source_rows(), 1);
    assert_eq!(witness.replayed_source_rows(), 1);
    assert_eq!(witness.replayed_shift_columns(), 2);
    assert_eq!(witness.guarded_rules(), 1);
    assert_eq!(
        witness.universally_applicable_guards(),
        artifact.rules()[0].nonzero_guards().len()
    );
    assert_eq!(witness.master_terminals(), 1);
    assert_eq!(witness.zero_sector_terminals(), 1);
}

#[test]
fn vakint_sign_convention_gives_i2_equals_d_minus_two_over_two_i1() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let context = artifact.coefficient_context();
    let (actual, residual_guard) = artifact
        .indexed_context()
        .specialize(
            artifact.rules()[0].right_hand_side()[0].coefficient(),
            &[1],
            Default::default(),
        )
        .unwrap();
    assert!(residual_guard.is_none());

    let d_minus_two = context
        .try_sub(
            &context.parameter("d").unwrap(),
            &context.integer(2),
            ExactAlgebraLimits::default(),
        )
        .unwrap();
    let expected = context
        .try_div(
            &d_minus_two,
            &context.integer(2),
            ExactAlgebraLimits::default(),
        )
        .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn installer_rejects_an_unregistered_closure_shape() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let ClosedArtifact {
        schema,
        arity,
        family,
        context,
        source_relations,
        rules,
        masters,
        zero_sectors,
        common_mass_homogeneity,
        ..
    } = artifact;
    assert!(matches!(
        install(ClosingArtifactCandidate {
            schema,
            algorithm_id: "unregistered-closure",
            arity,
            family,
            context,
            source_relations,
            rules,
            masters,
            zero_sectors,
            common_mass_homogeneity,
        }),
        Err(ArtifactError::UnsupportedClosureShape)
    ));
}

#[test]
fn installer_checks_q_squared_minus_one_family_semantics_not_only_fingerprints() {
    let base = CoefficientContext::try_new(["d"]).unwrap();
    let dimension = base.parameter("d").unwrap();
    let family = IntegralFamily::new(
        "wrong-plus-sign-one-loop",
        vec!["k".to_owned()],
        Vec::new(),
        base.clone(),
        dimension,
        vec![AffineDenominator::new(base.one(), vec![base.one()])],
        Vec::new(),
        vec![base.zero()],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..batch.len())
        .map(|ordinal| batch.generate(ordinal))
        .collect();
    let source_relations = batch.complete(rows).unwrap().into_relations();
    let rule = derive_sector_interior_rule(
        generator.context(),
        &source_relations,
        &[1],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();
    let context = generator.context().clone();
    drop(generator);
    let mut masters = BTreeSet::new();
    masters.insert(IntegralKey::try_new([1]).unwrap());
    let zero_sectors = vec![ZeroSectorTerminal::new(
        Mask::try_from_indices(&[0]).unwrap(),
        ZeroTerminalProof::ScalelessVacuumPolynomial,
    )];
    assert!(matches!(
        install(ClosingArtifactCandidate {
            schema: ArtifactSchemaVersion::CURRENT,
            algorithm_id: "rustred.generated.one-loop-unit-mass-tadpole.v1",
            arity: 1,
            family,
            context,
            source_relations,
            rules: vec![rule],
            masters,
            zero_sectors,
            common_mass_homogeneity: Some(CommonMassHomogeneityProof::UniformVacuumMassSquared,),
        }),
        Err(ArtifactError::UnsupportedClosureShape)
    ));
}

#[test]
fn durable_encoding_is_deterministic_and_loads_a_sealed_equivalent() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let first = artifact.encode_durable().unwrap();
    let independently_derived = derive_one_loop_unit_mass_tadpole().unwrap();
    let second = independently_derived.encode_durable().unwrap();
    assert_eq!(first, second);
    let loaded = ClosedArtifact::decode_durable(&first).unwrap();
    assert_eq!(loaded.encode_durable().unwrap(), first);
    assert_eq!(loaded.schema(), artifact.schema());
    assert_eq!(loaded.algorithm_id(), artifact.algorithm_id());
    assert_eq!(loaded.family_fingerprint(), artifact.family_fingerprint());
    assert_eq!(loaded.context_fingerprint(), artifact.context_fingerprint());
    assert_eq!(loaded.source_relations(), artifact.source_relations());
    assert_eq!(loaded.rules(), artifact.rules());
    assert_eq!(loaded.masters(), artifact.masters());
    assert_eq!(loaded.zero_sectors(), artifact.zero_sectors());
    assert_eq!(loaded.validation(), artifact.validation());
}

fn durable_section(bytes: &[u8], wanted_tag: u16) -> Range<usize> {
    let mut offset = 16;
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
    encoded_blob(bytes, &mut offset)
}

fn family_coefficient_payload_bytes(bytes: &[u8]) -> usize {
    let family = durable_section(bytes, 2);
    let mut offset = family.start + 7 * 8;
    encoded_blob(bytes, &mut offset); // family name
    encoded_blob(bytes, &mut offset); // loop label
    encoded_blob(bytes, &mut offset); // parameter label
    (0..4).map(|_| encoded_blob(bytes, &mut offset).len()).sum()
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
        ArtifactPersistenceError::InvalidMagic
    );

    let mut wrong_schema = encoded.clone();
    wrong_schema[8..12].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(
        ClosedArtifact::decode_durable(&wrong_schema).unwrap_err(),
        ArtifactPersistenceError::UnsupportedSchema { actual: 2 }
    );

    assert!(matches!(
        ClosedArtifact::decode_durable(&encoded[..encoded.len() - 1]),
        Err(ArtifactPersistenceError::Truncated { .. })
    ));

    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        ClosedArtifact::decode_durable(&trailing).unwrap_err(),
        ArtifactPersistenceError::TrailingBytes { remaining: 1 }
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
            resource: "artifact bytes",
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

    let first_coefficient = first_family_coefficient_payload(&encoded);
    let aggregate_limits = ArtifactLoadLimits {
        max_total_coefficient_bytes: first_coefficient.len(),
        ..ArtifactLoadLimits::default()
    };
    assert!(matches!(
        ClosedArtifact::decode_durable_with_limits(&encoded, aggregate_limits),
        Err(ArtifactPersistenceError::ResourceLimit {
            resource: "aggregate coefficient bytes",
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
fn load_coefficient_budget_is_shared_across_family_and_semantic_replay() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let encoded = artifact.encode_durable().unwrap();
    let total = minimum_encoding_coefficient_budget(&artifact);
    let family = family_coefficient_payload_bytes(&encoded);
    let replay = total.checked_sub(family).unwrap();
    let one_below = total - 1;

    assert!(family > 0 && replay > 0);
    assert!(one_below > family);
    assert!(one_below > replay);
    assert!(matches!(
        ClosedArtifact::decode_durable_with_limits(
            &encoded,
            ArtifactLoadLimits {
                max_total_coefficient_bytes: one_below,
                ..ArtifactLoadLimits::default()
            }
        ),
        Err(ArtifactPersistenceError::ResourceLimit {
            resource: "aggregate coefficient bytes",
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
    assert_eq!(loaded.encode_durable().unwrap(), encoded);
}

#[test]
fn durable_loader_rejects_a_self_consistent_forged_source_and_rule_pair() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let ClosedArtifact {
        schema,
        arity,
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
        family,
        context,
        source_relations: forged_sources,
        rules: vec![forged_rule],
        masters,
        zero_sectors,
        common_mass_homogeneity,
    })
    .unwrap();
    let encoded = forged.encode_durable().unwrap();
    assert_eq!(
        ClosedArtifact::decode_durable(&encoded).unwrap_err(),
        ArtifactPersistenceError::SemanticMismatch {
            field: "source derivation witness",
        }
    );
}

#[test]
fn sparse_binary_coefficients_reject_compact_hostile_shapes_before_native_algebra() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let encoded = artifact.encode_durable().unwrap();
    let coefficient = first_family_coefficient_payload(&encoded);

    // Expression-like bytes are not a grammar alternative: no Symbolica Atom
    // parser is reachable from the durable coefficient boundary.
    let mut expression_like = encoded.clone();
    expression_like[coefficient.start] = b'(';
    assert_eq!(
        ClosedArtifact::decode_durable(&expression_like).unwrap_err(),
        ArtifactPersistenceError::InvalidCoefficient {
            field: "family dimension",
        }
    );

    // A claimed enormous expanded product is rejected from its sparse term
    // count before coefficient/exponent storage is allocated.
    let mut huge_product = encoded.clone();
    huge_product[coefficient.start + 1..coefficient.start + 9]
        .copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(matches!(
        ClosedArtifact::decode_durable(&huge_product),
        Err(ArtifactPersistenceError::ResourceLimit {
            resource: "polynomial terms",
            ..
        })
    ));

    // The first payload is `d`: after the tag, two counts, integer sign,
    // integer length, and one magnitude byte comes its sole u16 exponent.
    let exponent_offset = coefficient.start + 1 + 8 + 8 + 1 + 8 + 1;
    let mut huge_power = encoded;
    huge_power[exponent_offset..exponent_offset + 2].copy_from_slice(&65_u16.to_le_bytes());
    let mut strict = ArtifactLoadLimits::default();
    strict.family.exact_algebra.max_exponent = 64;
    assert_eq!(
        ClosedArtifact::decode_durable_with_limits(&huge_power, strict).unwrap_err(),
        ArtifactPersistenceError::ResourceLimit {
            resource: "coefficient exponent",
            requested: 65,
            limit: 64,
        }
    );
}

#[test]
fn one_loop_algorithm_shape_is_rejected_before_coefficient_work() {
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
    assert!(matches!(
        ClosedArtifact::decode_durable_with_limits(&encoded, family_limited),
        Err(ArtifactPersistenceError::Artifact(ArtifactError::Family(_)))
    ));

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
