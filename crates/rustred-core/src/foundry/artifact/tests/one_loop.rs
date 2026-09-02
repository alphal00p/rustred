use std::collections::BTreeSet;

use crate::algebra::{CoefficientContext, ExactAlgebraLimits};
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::foundry::parametric::{ParametricRuleLimits, derive_sector_interior_rule};
use crate::identity::ParametricIbpGenerator;
use crate::sector::{Mask, OrderingPolicy};

use super::error::ArtifactError;
use super::install::{ClosingArtifactCandidate, install};
use super::model::{
    ArtifactSchemaVersion, ClosedArtifact, CommonMassHomogeneityProof, ZeroSectorTerminal,
    ZeroTerminalProof,
};
use super::one_loop::derive_one_loop_unit_mass_tadpole;

#[test]
fn generated_tadpole_installs_one_exact_closed_partition() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();

    assert_eq!(artifact.schema(), ArtifactSchemaVersion::V4);
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
        ordering,
        supported_root_power_bounds,
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
            ordering,
            supported_root_power_bounds,
            family,
            context,
            source_relations,
            rules,
            rule_cells: Vec::new(),
            canonicalizer: None,
            dependencies: Vec::new(),
            factorization_rules: Vec::new(),
            masters,
            zero_sectors,
            common_mass_homogeneity,
        }),
        Err(ArtifactError::UnsupportedClosureShape)
    ));
}

#[test]
fn installer_checks_every_direct_rule_against_one_explicit_ordering_authority() {
    let mut artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    artifact.duplicate_first_rule_with_ordering_for_test(OrderingPolicy::TestOnlyDistinct);
    let ClosedArtifact {
        schema,
        algorithm_id,
        arity,
        ordering,
        supported_root_power_bounds,
        family,
        context,
        source_relations,
        rules,
        rule_cells,
        canonicalizer,
        dependencies,
        factorization_rules,
        masters,
        zero_sectors,
        common_mass_homogeneity,
        ..
    } = artifact;

    assert!(matches!(
        install(ClosingArtifactCandidate {
            schema,
            algorithm_id,
            arity,
            ordering,
            supported_root_power_bounds,
            family,
            context,
            source_relations,
            rules,
            rule_cells,
            canonicalizer,
            dependencies,
            factorization_rules,
            masters,
            zero_sectors,
            common_mass_homogeneity,
        }),
        Err(ArtifactError::InvalidOrderingAuthority { ordinal: 1, .. })
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
            ordering: OrderingPolicy::default(),
            supported_root_power_bounds: vec![crate::sector::InteriorBounds::new(
                i64::MIN,
                i64::MAX,
            )]
            .into_boxed_slice(),
            family,
            context,
            source_relations,
            rules: vec![rule],
            rule_cells: Vec::new(),
            canonicalizer: None,
            dependencies: Vec::new(),
            factorization_rules: Vec::new(),
            masters,
            zero_sectors,
            common_mass_homogeneity: Some(CommonMassHomogeneityProof::UniformVacuumMassSquared,),
        }),
        Err(ArtifactError::UnsupportedClosureShape)
    ));
}
