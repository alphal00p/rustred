use crate::family::IntegralKey;
use crate::foundry::artifact::{ArtifactPersistenceError, derive_one_loop_unit_mass_tadpole};
use crate::reduction::Reducer;

use super::super::super::tests::{solved_tadpole, tadpole};

#[test]
fn source_port_k1_installs_into_existing_artifact_and_reducer() {
    let (audit, solution) = solved_tadpole();
    let artifact = audit
        .install_complete(tadpole(), [([true], None, solution)])
        .unwrap();
    assert_eq!(artifact.rule_cells().len(), 1);
    assert_eq!(artifact.masters().len(), 1);
    let rule = artifact.rule_cells()[0].rule();
    assert!(rule.replay_evidence().combined_original_domain().is_some());
    assert!(rule.anchor().is_none());
    assert!(rule.replay().is_none());
    assert!(rule.concrete_replay().is_none());
    assert!(rule.pivot_guard().is_none());
    assert!(rule.elimination_pivot_guards().is_empty());
    assert!(matches!(
        artifact.encode_durable(),
        Err(ArtifactPersistenceError::UnsupportedFeature { .. })
    ));
    let reference = derive_one_loop_unit_mass_tadpole().unwrap();
    let mut native = Reducer::new(&artifact).unwrap();
    let mut anchored = Reducer::new(&reference).unwrap();
    for n in [-3, 0, 1, 2, 3, 7] {
        let target = IntegralKey::try_new([n]).unwrap();
        let actual = native.reduce_unit_mass(&target).unwrap();
        let expected = anchored.reduce_unit_mass(&target).unwrap();
        assert_eq!(actual.terms(), expected.terms(), "n={n}");
        let before = native.statistics().cache_hits();
        let cached = native.reduce_unit_mass(&target).unwrap();
        assert_eq!(actual.terms(), cached.terms());
        if n > 0 {
            assert!(native.statistics().cache_hits() > before);
        }
        let mass = artifact.coefficient_context().integer(3);
        let actual = native
            .reduce_with_common_mass_squared(&target, &mass)
            .unwrap();
        let expected = anchored
            .reduce_with_common_mass_squared(&target, &mass)
            .unwrap();
        assert_eq!(actual.terms(), expected.terms(), "mass squared=3, n={n}");
    }
}

#[test]
fn original_domain_installer_rejects_corrupted_checked_payloads() {
    for mutation in 0..4 {
        let (audit, solution) = solved_tadpole();
        let mut program = audit
            .retain_program(tadpole(), [([true], None, solution)])
            .unwrap();
        let sector = program.sectors.get_mut(&[true]).unwrap();
        match mutation {
            0 => {
                sector.rules[0].ordinary.contributions[0].weight =
                    -sector.rules[0].ordinary.contributions[0].weight.clone()
            }
            1 => sector.rules[0].rhs[0].coefficient = -sector.rules[0].rhs[0].coefficient.clone(),
            2 => {
                sector.rules[0].application = vec![
                    crate::foundry::completion::LatticeBox::try_new(vec![0], vec![None]).unwrap(),
                ]
            }
            3 => sector.terminals.clear(),
            _ => unreachable!(),
        }
        assert!(program.install().is_err(), "mutation {mutation}");
    }
}

fn generated<const N: usize>(
    family: crate::family::IntegralFamily,
) -> crate::foundry::artifact::ClosedArtifact {
    use crate::sector::{Mask, zero};
    use crate::solver::{SectorConfig, SectorSolveOptions, SectorSolver, SourceSystem};
    use std::sync::Arc;
    let analyzer = zero::Analyzer::try_unrestricted(&family).unwrap();
    let mut zeros = Vec::new();
    let mut nonzeros = Vec::new();
    for bits in 0usize..(1usize << N) {
        let sector: [bool; N] = std::array::from_fn(|axis| bits & (1 << axis) != 0);
        match analyzer.analyze(&Mask::try_new(sector).unwrap()).unwrap() {
            zero::Decision::ProvedZero(_) => zeros.push(sector),
            _ => nonzeros.push(sector),
        }
    }
    drop(analyzer);
    let zeros: Arc<[[bool; N]]> = zeros.into();
    let sources = SourceSystem::from_family(&family).unwrap();
    let mut solutions = Vec::new();
    for sector in nonzeros {
        let solver = SectorSolver::new(
            &sources,
            sector,
            SectorConfig {
                zero_sectors: zeros.clone(),
                ..Default::default()
            },
        )
        .unwrap();
        solutions.push((
            sector,
            None,
            solver.solve_sector(SectorSolveOptions::default()).unwrap(),
        ));
    }
    let count: usize = solutions
        .iter()
        .map(|(_, _, solution)| solution.rules.len())
        .sum();
    eprintln!(
        "source-directed K{N}: sectors={}, generated_rules={count}",
        solutions.len()
    );
    let audit = crate::foundry::artifact::SourcePortAudit::try_new(&family, zeros).unwrap();
    let artifact = audit.install_complete(family, solutions).unwrap();
    eprintln!(
        "installed K{N}: cells={}, terminals={}, zeros={}",
        artifact.rule_cells().len(),
        artifact.masters().len(),
        artifact.zero_sectors().len()
    );
    artifact
}

#[test]
fn source_port_k3_full_installation_matches_existing_reducer() {
    use std::collections::BTreeMap;
    let artifact = generated::<3>(
        crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap(),
    );
    assert_eq!(artifact.masters().len(), 4);
    let reference = crate::foundry::artifact::derive_two_loop_unit_mass_sunset().unwrap();
    let mut native = Reducer::new(&artifact).unwrap();
    let mut anchored = Reducer::new(&reference).unwrap();
    for powers in [
        [1, 1, 1],
        [2, 1, 1],
        [1, 2, 3],
        [0, 1, 1],
        [-1, 2, 1],
        [2, -2, 3],
        [0, 0, 1],
    ] {
        let target = IntegralKey::try_new(powers).unwrap();
        let actual = native.reduce_unit_mass(&target).unwrap();
        // The new autonomous producer retains three distinct pinch corners.
        // Project only for this post-generation comparison through the old
        // reducer's exact symmetry convention; no reference enters discovery.
        let mut canonical = BTreeMap::new();
        let context = artifact.coefficient_context();
        for (master, coefficient) in actual.terms() {
            for (key, scalar) in anchored.reduce_unit_mass(master).unwrap().terms() {
                let product = context
                    .try_mul(coefficient, scalar, Default::default())
                    .unwrap();
                let value = canonical
                    .entry(key.clone())
                    .or_insert_with(|| context.zero());
                *value = context
                    .try_add(value, &product, Default::default())
                    .unwrap();
            }
        }
        canonical.retain(|_, value| !value.is_zero());
        assert_eq!(
            &canonical,
            anchored.reduce_unit_mass(&target).unwrap().terms(),
            "{powers:?}"
        );
    }
}

#[test]
#[ignore = "complete canonical K6 in-memory artifact and runtime gate"]
fn source_port_k6_full_installation_and_existing_reducer() {
    let artifact =
        generated::<6>(crate::foundry::artifact::three_loop::canonical_family().unwrap());
    assert_eq!(artifact.masters().len(), 38);
    assert_eq!(artifact.zero_sectors().len(), 26);
    assert!(artifact.rule_cells().iter().all(|cell| {
        cell.rule()
            .replay_evidence()
            .combined_original_domain()
            .is_some()
    }));
    let mut reducer = Reducer::new(&artifact).unwrap();
    for master in artifact.masters() {
        assert!(
            master
                .powers()
                .iter()
                .all(|value| *value == 0 || *value == 1)
        );
        let fixed = reducer.reduce_unit_mass(master).unwrap();
        assert_eq!(fixed.terms().len(), 1);
        assert_eq!(
            fixed.terms().get(master),
            Some(&artifact.coefficient_context().one())
        );
        let mut probe = master.powers().to_vec();
        let active = probe.iter().position(|value| *value == 1).unwrap();
        probe[active] = 2;
        if let Some(inactive) = probe.iter().position(|value| *value == 0) {
            probe[inactive] = -1;
        }
        let target = IntegralKey::try_new(probe).unwrap();
        let reduced = reducer.reduce_unit_mass(&target).unwrap();
        assert!(
            reduced
                .terms()
                .keys()
                .all(|terminal| artifact.masters().contains(terminal))
        );
    }
    assert!(matches!(
        artifact.encode_durable(),
        Err(ArtifactPersistenceError::UnsupportedFeature { .. })
    ));
}
