//! Genuine replayed cells exercise the final gate without solver-report authority.
use super::*;
use crate::foundry::artifact::source_port::{
    DEFAULT_PREDICATE_ATOMS, DEFAULT_PREDICATE_CONSISTENCY_WORK, installed_k1_for_codec_test,
    source_port_candidate_for_test,
};

fn k1() -> ClosingArtifactCandidate {
    source_port_candidate_for_test(installed_k1_for_codec_test())
}

fn install(
    candidate: ClosingArtifactCandidate,
    entry_degree: u64,
    degrees: Vec<(Mask, u64)>,
    geometry: CompletionGeometryLimits,
) -> Result<ClosedArtifact, ArtifactError> {
    let root = scope::root_from_bounds(&candidate.supported_root_power_bounds, candidate.arity)?;
    let entry = scope::EntryScope::try_new(
        &candidate.family,
        &root,
        scope::EntryDegreeBound::MaxTotalExcessDegree(entry_degree),
    )?;
    install_source_port_through_total_excess_with_limits(
        candidate,
        entry,
        degrees,
        geometry,
        DEFAULT_PREDICATE_CONSISTENCY_WORK,
        DEFAULT_PREDICATE_ATOMS,
    )
}

fn degree(value: u64) -> Vec<(Mask, u64)> {
    vec![(Mask::try_new([true]).unwrap(), value)]
}

#[test]
fn bounded_actual_cell_gate_requires_complete_unique_nonzero_degree_census() {
    let control = install(k1(), 3, degree(3), Default::default()).unwrap();
    assert!(!control.proof_scope.is_unrestricted());
    assert!(!control.is_complete_unit_mass_vacuum());
    assert!(control.encode_durable().is_err());
    for entries in [
        vec![],
        degree(2),
        vec![
            (Mask::try_new([true]).unwrap(), 3),
            (Mask::try_new([true]).unwrap(), 3),
        ],
        vec![(Mask::try_new([false]).unwrap(), 3)],
        vec![(Mask::try_new([true, true]).unwrap(), 3)],
    ] {
        assert!(install(k1(), 3, entries, Default::default()).is_err());
    }
}

#[test]
fn bounded_actual_cell_gate_rejects_wrong_entry_kind_and_root() {
    for (root, bound) in [
        ([true], scope::EntryDegreeBound::MaxNegativeIndexDegree(3)),
        ([false], scope::EntryDegreeBound::MaxTotalExcessDegree(3)),
    ] {
        let candidate = k1();
        let entry =
            scope::EntryScope::try_new(&candidate.family, &Mask::try_new(root).unwrap(), bound)
                .unwrap();
        assert!(
            install_source_port_through_total_excess_with_limits(
                candidate,
                entry,
                degree(3),
                Default::default(),
                DEFAULT_PREDICATE_CONSISTENCY_WORK,
                DEFAULT_PREDICATE_ATOMS
            )
            .is_err()
        );
    }
}

#[test]
fn bounded_actual_cell_gate_does_not_certify_larger_replay_endpoints() {
    let candidate = k1();
    let cell = &candidate.rule_cells[0];
    assert_eq!(
        cell.rule()
            .replay_evidence()
            .combined_original_domain()
            .unwrap()
            .application_boxes()[0]
            .upper(),
        &[None]
    );
    assert_eq!(cell.application_domain().bounds()[0].upper(), i64::MAX);
    // Active local coordinates are n-1. The executable domain ends one short
    // of this mathematical requested degree, despite its unbounded replay box.
    assert!(matches!(
        install(
            candidate,
            i64::MAX as u64,
            degree(i64::MAX as u64),
            Default::default()
        ),
        Err(ArtifactError::UnsupportedClosureShape)
    ));
    let endpoint = i64::MAX as u64 - 1;
    install(k1(), endpoint, degree(endpoint), Default::default()).unwrap();
}

#[test]
fn bounded_actual_cell_gate_rejects_actual_coverage_holes() {
    let mut no_corner = k1();
    no_corner.masters.clear();
    assert!(install(no_corner, 3, degree(3), Default::default()).is_err());
    let mut no_rule = k1();
    no_rule.rule_cells.clear();
    assert!(install(no_rule, 3, degree(3), Default::default()).is_err());
    // A terminal alone really does cover the zero-degree simplex.
    let mut corner_only = k1();
    corner_only.rule_cells.clear();
    install(corner_only, 0, degree(0), Default::default()).unwrap();
}

#[test]
fn bounded_actual_cell_gate_preserves_typed_caller_limits() {
    let geometry = CompletionGeometryLimits {
        max_requested_boxes: 1,
        ..Default::default()
    };
    assert!(matches!(
        install(k1(), 3, degree(3), geometry),
        Err(ArtifactError::ResourceLimit {
            resource: "combined cover boxes",
            requested: 2,
            limit: 1
        })
    ));
    let geometry = CompletionGeometryLimits {
        max_requested_box_coordinate_cells: 3,
        ..Default::default()
    };
    assert!(matches!(
        install(k1(), 3, degree(3), geometry),
        Err(ArtifactError::ResourceLimit {
            resource: "combined cover coordinate cells",
            requested: 4,
            limit: 3
        })
    ));
    let geometry = CompletionGeometryLimits {
        max_split_operations: 0,
        ..Default::default()
    };
    assert!(matches!(
        install(k1(), 3, degree(3), geometry),
        Err(ArtifactError::ResourceBudgetExhausted {
            resource: "structural-box split operations"
        })
    ));
}
