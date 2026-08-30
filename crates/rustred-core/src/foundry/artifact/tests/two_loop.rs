use crate::family::IntegralKey;
use crate::reduction::Reducer;

use super::error::{ArtifactError, ArtifactPersistenceError};
use super::model::{ClosedArtifact, ZeroTerminalProof};
use super::persistence::ArtifactLoadLimits;
use super::two_loop::derive_two_loop_unit_mass_sunset;

#[test]
fn generated_sunset_installs_the_exact_five_cell_closed_partition() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    assert_eq!(artifact.algorithm_id(), super::two_loop::ALGORITHM_ID);
    assert_eq!(artifact.arity(), 3);
    assert_eq!(artifact.source_relations().len(), 4);
    assert!(artifact.rules().is_empty());
    assert_eq!(artifact.rule_cells().len(), 5);
    assert_eq!(artifact.canonicalizer().unwrap().group_order(), 6);
    assert_eq!(artifact.dependencies().len(), 1);
    assert_eq!(artifact.factorization_rules().len(), 1);
    assert_eq!(artifact.masters().len(), 2);
    assert_eq!(artifact.zero_sectors().len(), 4);
    for terminal in artifact.zero_sectors() {
        let expected = if terminal.sector().active_bits().iter().any(|&active| active) {
            ZeroTerminalProof::LeePomeranskyRankDeficiency
        } else {
            ZeroTerminalProof::ScalelessVacuumPolynomial
        };
        assert_eq!(terminal.proof(), expected);
    }
    let factorization = &artifact.factorization_rules()[0];
    assert_eq!(factorization.loop_basis().dimension(), 2);
    assert_eq!(factorization.loop_basis().row_major(), [0, 1, 1, 1]);
    assert_eq!(factorization.factors()[0].transformed_loop_positions(), [0]);
    assert_eq!(factorization.factors()[1].transformed_loop_positions(), [1]);
}

#[test]
fn sunset_durable_artifact_is_deterministic_authenticated_and_reducer_ready() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let first = artifact.encode_durable().unwrap();
    let second = artifact.encode_durable().unwrap();
    assert_eq!(first, second);

    let loaded = ClosedArtifact::decode_durable(&first).unwrap();
    assert_eq!(loaded.algorithm_id(), super::two_loop::ALGORITHM_ID);
    assert_eq!(loaded.rule_cells().len(), 5);
    assert_eq!(loaded.factorization_rules().len(), 1);
    let target = IntegralKey::try_new([2, 2, 1]).unwrap();
    let mut reducer = Reducer::new(&loaded).unwrap();
    let reduction = reducer.reduce_unit_mass(&target).unwrap();
    assert_eq!(reduction.target(), &target);
    assert!(
        reduction
            .terms()
            .keys()
            .all(|master| loaded.masters().contains(master))
    );

    let mut corrupted = first;
    let last = corrupted.last_mut().unwrap();
    *last ^= 1;
    assert!(matches!(
        ClosedArtifact::decode_durable(&corrupted),
        Err(ArtifactPersistenceError::SemanticMismatch {
            field: "two-loop complete artifact witness"
        })
    ));
}

#[test]
fn sunset_load_threads_explicit_family_source_and_rule_policies() {
    let encoded = derive_two_loop_unit_mass_sunset()
        .unwrap()
        .encode_durable()
        .unwrap();

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

    let mut rule_limited = ArtifactLoadLimits::default();
    rule_limited.rule_derivation.max_source_rows = 0;
    assert!(matches!(
        ClosedArtifact::decode_durable_with_limits(&encoded, rule_limited),
        Err(ArtifactPersistenceError::Artifact(
            ArtifactError::ParametricRule(_)
        ))
    ));
}
