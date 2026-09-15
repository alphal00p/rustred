use super::*;
use crate::foundry::artifact::{
    derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use std::sync::Arc;

#[test]
fn semantic_snapshot_rejects_combined_instead_of_omitting_replay_fields() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let mut rule = artifact.rules()[0].clone();
    semantic::encode_rule_snapshot(&rule, &Writer::new(Default::default())).unwrap();
    rule.replace_replay_with_uncertified_combined_domain_for_test();
    assert!(matches!(
        semantic::encode_rule_snapshot(&rule, &Writer::new(Default::default())),
        Err(ArtifactPersistenceError::UnsupportedFeature { .. })
    ));
}

#[test]
fn direct_artifact_encoder_does_not_manufacture_a_combined_anchor() {
    let mut artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let bytes = encode(&artifact).unwrap();
    ClosedArtifact::decode_durable(&bytes).unwrap();
    artifact.rules[0].replace_replay_with_uncertified_combined_domain_for_test();
    assert!(matches!(
        encode(&artifact),
        Err(ArtifactPersistenceError::UnsupportedFeature { .. })
    ));
}

#[test]
fn existing_cell_grammar_rejects_combined_replay_in_every_sunset_cell() {
    // Rebuild rather than cloning the immutable owner or weakening its seal.
    let count = derive_two_loop_unit_mass_sunset()
        .unwrap()
        .rule_cells()
        .len();
    for ordinal in 0..count {
        let mut artifact = derive_two_loop_unit_mass_sunset().unwrap();
        encode(&artifact).unwrap();
        Arc::get_mut(&mut artifact.rule_cells[ordinal])
            .expect("fresh artifact owns the cell uniquely")
            .replace_replay_with_uncertified_combined_domain_for_test();
        assert!(matches!(
            encode(&artifact),
            Err(ArtifactPersistenceError::UnsupportedFeature { .. })
        ));
    }
}
