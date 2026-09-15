use super::*;
use crate::foundry::artifact::{
    derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};

fn candidate(artifact: ClosedArtifact) -> ClosingArtifactCandidate {
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
    ClosingArtifactCandidate {
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
    }
}

#[test]
fn generic_installer_rejects_combined_replay_on_an_otherwise_valid_direct_rule() {
    let mut candidate = candidate(derive_one_loop_unit_mass_tadpole().unwrap());
    validate_generic_bindings(&candidate).unwrap();
    candidate.rules[0].replace_replay_with_uncertified_combined_domain_for_test();
    assert!(matches!(
        validate_generic_bindings(&candidate),
        Err(ArtifactError::InvalidReplayEvidence { .. })
    ));
    assert!(matches!(
        install(candidate),
        Err(ArtifactError::InvalidReplayEvidence { .. })
    ));
}

#[test]
fn generic_and_cell_replay_gates_reject_combined_without_skipping_counts() {
    let mut candidate = candidate(derive_two_loop_unit_mass_sunset().unwrap());
    validate_generic_bindings(&candidate).unwrap();
    for cell in &mut candidate.rule_cells {
        validate_cell_replay(cell, &candidate.context).unwrap();
        Arc::get_mut(cell)
            .expect("fresh artifact owns the cell uniquely")
            .replace_replay_with_uncertified_combined_domain_for_test();
        assert!(matches!(
            validate_cell_replay(cell, &candidate.context),
            Err(ArtifactError::InvalidReplayEvidence { .. })
        ));
    }
    assert!(matches!(
        validate_generic_bindings(&candidate),
        Err(ArtifactError::InvalidReplayEvidence { .. })
    ));
    assert!(matches!(
        install(candidate),
        Err(ArtifactError::InvalidReplayEvidence { .. })
    ));
}
