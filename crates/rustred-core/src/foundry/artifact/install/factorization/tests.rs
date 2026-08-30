use std::collections::BTreeSet;

use crate::family::IntegralKey;
use crate::foundry::artifact::factorization::{
    FactorizationFactor, FactorizationRule, UnimodularLoopBasis,
};
use crate::foundry::artifact::model::{
    ArtifactSchemaVersion, ArtifactValidationWitness, ClosedArtifact, CommonMassHomogeneityProof,
};
use crate::foundry::artifact::one_loop::derive_one_loop_unit_mass_tadpole;
use crate::foundry::artifact::three_loop::{canonical_family, canonical_s4};
use crate::foundry::artifact::two_loop::derive_two_loop_unit_mass_sunset;
use crate::identity::{ParametricIbpConfig, ParametricIbpGenerator};
use crate::reduction::Reducer;
use crate::sector::{InteriorBounds, Mask, SectorInteriorDomain};

use super::super::ClosingArtifactCandidate;
use super::validate_and_compile;

fn synthetic_three_loop_factorization_artifact() -> ClosedArtifact {
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())
            .unwrap();
    let context = generator.context().clone();
    drop(generator);

    let k3_times_k1 = FactorizationRule::new(
        SectorInteriorDomain::try_new(
            Mask::try_from_indices(&[0, 0, 1, 1, 1, 1]).unwrap(),
            [
                InteriorBounds::new(0, 0),
                InteriorBounds::new(0, 0),
                InteriorBounds::new(1, i64::MAX),
                InteriorBounds::new(1, i64::MAX),
                InteriorBounds::new(1, i64::MAX),
                InteriorBounds::new(1, i64::MAX),
            ],
        )
        .unwrap(),
        [
            // q0=k3-k1, q1=k1-k2: the K3 dependency denominators are
            // parent D4,D5,D6 in zero-based slots 3,4,5.
            FactorizationFactor::new(0, [3, 4, 5], [0, 1]),
            // q2=k3 owns parent D2.
            FactorizationFactor::new(1, [2], [2]),
        ],
        family.coefficient_context().one(),
        UnimodularLoopBasis::new(3, [-1, 0, 1, 1, -1, 0, 0, 0, 1]),
    );

    let k1_cubed = FactorizationRule::new(
        SectorInteriorDomain::try_new(
            Mask::try_from_indices(&[0, 0, 1, 1, 0, 1]).unwrap(),
            [
                InteriorBounds::new(0, 0),
                InteriorBounds::new(0, 0),
                InteriorBounds::new(1, i64::MAX),
                InteriorBounds::new(1, i64::MAX),
                InteriorBounds::new(0, 0),
                InteriorBounds::new(1, i64::MAX),
            ],
        )
        .unwrap(),
        [
            // q0=k3 owns parent D3.
            FactorizationFactor::new(1, [2], [0]),
            // q1=k3-k1 owns parent D4.
            FactorizationFactor::new(1, [3], [1]),
            // q2=k2-k3 owns parent D6.
            FactorizationFactor::new(1, [5], [2]),
        ],
        family.coefficient_context().one(),
        UnimodularLoopBasis::new(3, [0, 0, 1, -1, 0, 1, 0, 1, -1]),
    );
    let masters = BTreeSet::from([
        IntegralKey::try_new([0, 0, 1, 0, 1, 1]).unwrap(),
        IntegralKey::try_new([0, 0, 1, 1, 0, 1]).unwrap(),
        IntegralKey::try_new([0, 0, 1, 1, 1, 1]).unwrap(),
    ]);
    let mut candidate = ClosingArtifactCandidate {
        schema: ArtifactSchemaVersion::CURRENT,
        algorithm_id: "rustred.test.three-loop-k3-times-k1.v1",
        arity: 6,
        supported_root_power_bounds: vec![InteriorBounds::new(i64::MIN, i64::MAX); 6]
            .into_boxed_slice(),
        family,
        context,
        source_relations: Vec::new(),
        rules: Vec::new(),
        rule_cells: Vec::new(),
        canonicalizer: Some(canonicalizer),
        dependencies: vec![
            Box::new(derive_two_loop_unit_mass_sunset().unwrap()),
            Box::new(derive_one_loop_unit_mass_tadpole().unwrap()),
        ],
        factorization_rules: vec![k3_times_k1, k1_cubed],
        masters,
        zero_sectors: Vec::new(),
        common_mass_homogeneity: Some(CommonMassHomogeneityProof::UniformVacuumMassSquared),
    };
    validate_and_compile(&mut candidate).unwrap();
    let family_fingerprint = candidate.family.fingerprint_owner();
    ClosedArtifact {
        schema: candidate.schema,
        algorithm_id: candidate.algorithm_id,
        arity: candidate.arity,
        supported_root_power_bounds: candidate.supported_root_power_bounds,
        family: candidate.family,
        family_fingerprint,
        context: candidate.context,
        source_relations: candidate.source_relations,
        rules: candidate.rules,
        rule_cells: candidate.rule_cells,
        canonicalizer: candidate.canonicalizer,
        dependencies: candidate.dependencies,
        factorization_rules: candidate.factorization_rules,
        masters: candidate.masters,
        zero_sectors: candidate.zero_sectors,
        common_mass_homogeneity: candidate.common_mass_homogeneity,
        // This internal artifact isolates the generic installer/compiler and
        // reducer paths. It deliberately bypasses the still-absent registered
        // K6 closure verifier and therefore carries only an honest local
        // census, never durable/publication status.
        validation: ArtifactValidationWitness::new(0, 0, 0, 0, 0, 3, 0),
    }
}

#[test]
fn compiled_k3_times_k1_factorization_recurses_to_both_parent_terminals() {
    let artifact = synthetic_three_loop_factorization_artifact();
    let embeddings = artifact.factorization_rules()[0].master_embeddings();
    assert_eq!(embeddings.len(), 2);
    assert_eq!(
        embeddings
            .iter()
            .map(|embedding| (
                embedding.raw_parent_master().powers(),
                embedding.parent_terminal().powers(),
            ))
            .collect::<Vec<_>>(),
        [
            (&[0, 0, 1, 0, 1, 1][..], &[0, 0, 1, 0, 1, 1][..]),
            (&[0, 0, 1, 1, 1, 1][..], &[0, 0, 1, 1, 1, 1][..]),
        ]
    );

    let target = IntegralKey::try_new([0, 0, 1, 2, 2, 1]).unwrap();
    let mut parent_reducer = Reducer::new(&artifact).unwrap();
    let parent = parent_reducer.reduce_unit_mass(&target).unwrap();
    let dependency_target = IntegralKey::try_new([2, 2, 1]).unwrap();
    let dependency = Reducer::new(&derive_two_loop_unit_mass_sunset().unwrap())
        .unwrap()
        .reduce_unit_mass(&dependency_target)
        .unwrap();
    assert_eq!(parent.terms().len(), 2);
    assert_eq!(
        parent.coefficient(&IntegralKey::try_new([0, 0, 1, 0, 1, 1]).unwrap()),
        dependency.coefficient(&IntegralKey::try_new([0, 1, 1]).unwrap())
    );
    assert_eq!(
        parent.coefficient(&IntegralKey::try_new([0, 0, 1, 1, 1, 1]).unwrap()),
        dependency.coefficient(&IntegralKey::try_new([1, 1, 1]).unwrap())
    );
    assert!(parent_reducer.statistics().cache_hits() >= 1);
}

#[test]
fn compiled_k1_cubed_factorization_closes_the_extra_spanning_tree_orbit() {
    let artifact = synthetic_three_loop_factorization_artifact();
    let factorization = &artifact.factorization_rules()[1];
    assert_eq!(
        factorization.application_domain().sector().active_bits(),
        [false, false, true, true, false, true]
    );
    assert_eq!(factorization.master_embeddings().len(), 1);
    assert_eq!(
        factorization.master_embeddings()[0]
            .raw_parent_master()
            .powers(),
        [0, 0, 1, 1, 0, 1]
    );
    assert_eq!(
        factorization.master_embeddings()[0]
            .parent_terminal()
            .powers(),
        [0, 0, 1, 1, 0, 1]
    );

    let target = IntegralKey::try_new([0, 0, 2, 3, 0, 4]).unwrap();
    let parent = Reducer::new(&artifact)
        .unwrap()
        .reduce_unit_mass(&target)
        .unwrap();
    let tadpole = derive_one_loop_unit_mass_tadpole().unwrap();
    let mut tadpole_reducer = Reducer::new(&tadpole).unwrap();
    let master = IntegralKey::try_new([1]).unwrap();
    let factors = [2, 3, 4].map(|power| {
        tadpole_reducer
            .reduce_unit_mass(&IntegralKey::try_new([power]).unwrap())
            .unwrap()
            .coefficient(&master)
            .unwrap()
            .clone()
    });
    let context = artifact.coefficient_context();
    let expected = context
        .try_mul(&factors[0], &factors[1], Default::default())
        .and_then(|product| context.try_mul(&product, &factors[2], Default::default()))
        .unwrap();
    assert_eq!(parent.terms().len(), 1);
    assert_eq!(
        parent.coefficient(&IntegralKey::try_new([0, 0, 1, 1, 0, 1]).unwrap()),
        Some(&expected)
    );
}
