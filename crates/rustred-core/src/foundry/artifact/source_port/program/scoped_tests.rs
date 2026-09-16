//! Real generated scoped programs, not hand-authored reduction rules.
use std::sync::Arc;

use crate::family::{IntegralFamily, IntegralKey};
use crate::foundry::artifact::{ClosedArtifact, SourcePortAudit, SourcePortAuditError};
use crate::reduction::{Reducer, ReductionError};
use crate::sector::InteriorBounds;
use crate::solver::{SectorConfig, SectorSolution, SectorSolveOptions, SectorSolver, SourceSystem};

fn family() -> IntegralFamily {
    crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap()
}

fn zeros() -> Arc<[[bool; 3]]> {
    Arc::from([
        [false, false, false],
        [true, false, false],
        [false, true, false],
        [false, false, true],
    ])
}

fn solve(family: &IntegralFamily, sector: [bool; 3]) -> SectorSolution<3> {
    let sources = SourceSystem::from_family(family).unwrap();
    SectorSolver::new(
        &sources,
        sector,
        SectorConfig {
            zero_sectors: zeros(),
            ..Default::default()
        },
    )
    .unwrap()
    .solve_sector(SectorSolveOptions::default())
    .unwrap()
}

pub(in crate::foundry::artifact) fn generated_scope(root: [bool; 3]) -> ClosedArtifact {
    let family = family();
    let zero_sectors = zeros();
    let audit =
        SourcePortAudit::try_new_with_root_sector(&family, zero_sectors.clone(), root).unwrap();
    assert_eq!(
        audit.proved_zero_sector_count(),
        4,
        "global zero evidence must not be scoped away"
    );
    let mut solved = Vec::new();
    for bits in 0..8usize {
        let sector: [bool; 3] = std::array::from_fn(|axis| bits & (1 << axis) != 0);
        if !zero_sectors.contains(&sector)
            && sector
                .iter()
                .zip(root)
                .all(|(&active, allowed)| !active || allowed)
        {
            solved.push((sector, None, solve(&family, sector)));
        }
    }
    audit.install_complete(family, solved).unwrap()
}

#[test]
fn scoped_k3_cold_roundtrip_matches_full_family_and_restores_mass() {
    let full = generated_scope([true; 3]);
    let scoped = generated_scope([true, true, false]);
    let bytes = scoped.encode_durable().unwrap();
    assert_eq!(
        bytes,
        generated_scope([true, true, false])
            .encode_durable()
            .unwrap()
    );
    let cold = ClosedArtifact::decode_durable(&bytes).unwrap();
    assert_eq!(bytes, cold.encode_durable().unwrap());
    assert_ne!(bytes, full.encode_durable().unwrap());
    assert_eq!(cold.zero_sectors().len(), 4);
    assert!(
        cold.zero_sectors()
            .iter()
            .any(|zero| zero.sector().active_bits() == [false, false, true])
    );
    assert_eq!(
        cold.supported_root_power_bounds(),
        [
            InteriorBounds::new(i64::MIN, i64::MAX),
            InteriorBounds::new(i64::MIN, i64::MAX),
            InteriorBounds::new(i64::MIN, 0),
        ]
    );
    assert!(cold.masters().iter().all(|master| master.powers()[2] <= 0));
    let mut full_reducer = Reducer::new(&full).unwrap();
    let mut scoped_reducer = Reducer::new(&cold).unwrap();
    for powers in [[1, 1, 0], [2, 3, -2], [1, 2, -3], [0, 1, 0]] {
        let target = IntegralKey::try_new(powers).unwrap();
        assert_eq!(
            full_reducer.reduce_unit_mass(&target).unwrap().terms(),
            scoped_reducer.reduce_unit_mass(&target).unwrap().terms()
        );
        assert_eq!(
            full_reducer
                .reduce_with_common_mass_squared(&target, &full.coefficient_context().integer(3))
                .unwrap()
                .terms(),
            scoped_reducer
                .reduce_with_common_mass_squared(&target, &cold.coefficient_context().integer(3))
                .unwrap()
                .terms(),
        );
    }
    // Even a globally proved-zero sector outside the root is not an accepted
    // public reduction request. Validation precedes cache/zero shortcuts.
    for powers in [[1, 1, 1], [0, 0, 1]] {
        assert!(matches!(
            scoped_reducer.reduce_unit_mass(&IntegralKey::try_new(powers).unwrap()),
            Err(ReductionError::OutsideCertifiedRootDomain { position: 2, .. })
        ));
    }
}

#[test]
fn scoped_census_rejects_missing_or_outside_sectors_and_zero_only_is_explicit() {
    let family = family();
    let audit =
        SourcePortAudit::try_new_with_root_sector(&family, zeros(), [true, true, false]).unwrap();
    let missing = audit.install_complete(family, []).unwrap_err();
    assert!(missing.to_string().contains("does not exhaust"));

    let family = self::family();
    let audit =
        SourcePortAudit::try_new_with_root_sector(&family, zeros(), [true, true, false]).unwrap();
    assert!(
        audit
            .audit_sector([true; 3], None, &solve(&family, [true; 3]))
            .unwrap_err()
            .to_string()
            .contains("outside")
    );

    let audit = SourcePortAudit::try_new_with_root_sector(&family, zeros(), [false; 3]).unwrap();
    assert!(
        matches!(audit.install_complete(family, []), Err(SourcePortAuditError::Message(message)) if message.contains("zero-only"))
    );
}

pub(in crate::foundry::artifact) fn candidate(
    artifact: ClosedArtifact,
) -> crate::foundry::artifact::install::ClosingArtifactCandidate {
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
    crate::foundry::artifact::install::ClosingArtifactCandidate {
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
fn scoped_installer_rejects_outside_terminals_and_incompatible_root_bounds() {
    use crate::foundry::artifact::install::install_source_port;
    let mut outside = candidate(generated_scope([true, true, false]));
    outside
        .masters
        .insert(IntegralKey::try_new([1, 1, 1]).unwrap());
    assert!(install_source_port(outside).is_err());

    let mut finite = candidate(generated_scope([true, true, false]));
    finite.supported_root_power_bounds[2] = InteriorBounds::new(i64::MIN, -1);
    assert!(install_source_port(finite).is_err());

    // The explicit master keeps the sole nonzero sector in the census. Its
    // finite point cannot replace a rule cover of that unbounded sector.
    let mut missing_cover = candidate(generated_scope([true, true, false]));
    assert!(!missing_cover.rule_cells.is_empty());
    assert!(!missing_cover.masters.is_empty());
    missing_cover.rule_cells.clear();
    assert!(install_source_port(missing_cover).is_err());
}
