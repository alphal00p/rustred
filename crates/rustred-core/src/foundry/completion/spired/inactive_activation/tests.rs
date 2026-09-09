use crate::algebra::{CoefficientContext, IndexedCoefficientContext};
use crate::foundry::completion::stratum::{
    DecoratedStratum, GuardBranch, GuardBranchIdentity, StratumRegistryLimits,
};
use crate::identity::IntegralShift;
use crate::sector::{InteriorBounds, Mask, SectorInteriorDomain, SectorMonotoneDomain};

use super::{
    SpiredInactiveActivationAnalysisLimits, SpiredInactiveActivationError,
    SpiredInactiveActivationLimits, SpiredReplayedInactiveActivationTerm,
    try_analyze_replayed_inactive_activations, try_decompose_inactive_activation,
};

fn domain(active: &[bool], bounds: &[(i64, i64)]) -> SectorMonotoneDomain {
    assert_eq!(active.len(), bounds.len());
    let pivot = vec![0_i64; active.len()];
    let rhs: [&[i64]; 0] = [];
    SectorMonotoneDomain::try_new_for_rule(
        Mask::try_new(active.iter().copied()).unwrap(),
        bounds
            .iter()
            .map(|&(lower, upper)| InteriorBounds::new(lower, upper)),
        &pivot,
        &rhs,
    )
    .unwrap()
}

fn shift(values: impl IntoIterator<Item = i64>) -> IntegralShift {
    IntegralShift::try_new(values).unwrap()
}

fn context(scope: &str, arity: usize) -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(&CoefficientContext::new(Vec::<String>::new()), scope, arity)
        .unwrap()
}

fn stratum(
    context: &IndexedCoefficientContext,
    active: &[bool],
    bounds: &[(i64, i64)],
) -> DecoratedStratum {
    DecoratedStratum::try_guard_blind(
        "spired-inactive-activation-analysis-family",
        context.fingerprint(),
        domain(active, bounds),
        StratumRegistryLimits::default(),
    )
    .unwrap()
}

fn cell_contains(domain: &SectorInteriorDomain, point: &[i64]) -> bool {
    domain.contains(point).unwrap()
}

#[test]
fn inactive_plus_one_splits_safe_interior_from_activation_face() {
    let parent = domain(&[false], &[(-3, 0)]);
    let decomposition = try_decompose_inactive_activation(
        &parent,
        &shift([1]),
        SpiredInactiveActivationLimits::default(),
    )
    .unwrap()
    .unwrap();

    assert_eq!(decomposition.parent_domain(), &parent);
    assert_eq!(decomposition.shift().values(), &[1]);
    assert_eq!(decomposition.affected_axes().len(), 1);
    let axis = decomposition.affected_axes()[0];
    assert_eq!(axis.position(), 0);
    assert_eq!(axis.shift(), 1);
    assert_eq!(axis.safe_bounds(), Some(InteriorBounds::new(-3, -1)));
    assert_eq!(axis.activation_bounds(), InteriorBounds::new(0, 0));
    assert_eq!(axis.activation_value_count(), 1);

    let safe = decomposition.safe_interior().unwrap();
    assert_eq!(safe.bounds(), &[InteriorBounds::new(-3, -1)]);
    assert!(safe.covers_shift(&[1]).unwrap());
    assert_eq!(decomposition.activation_slices().len(), 1);
    let face = &decomposition.activation_slices()[0];
    assert_eq!(face.first_activation_axis_ordinal(), 0);
    assert_eq!(face.activation_position(), 0);
    assert_eq!(face.activation_value(), 0);
    assert_eq!(face.domain().bounds(), &[InteriorBounds::new(0, 0)]);
    assert!(!face.domain().covers_shift(&[1]).unwrap());
}

#[test]
fn larger_positive_shift_materializes_every_finite_activation_value() {
    let parent = domain(&[false], &[(-5, 0)]);
    let decomposition = try_decompose_inactive_activation(
        &parent,
        &shift([3]),
        SpiredInactiveActivationLimits::default(),
    )
    .unwrap()
    .unwrap();

    assert_eq!(
        decomposition.safe_interior().unwrap().bounds(),
        &[InteriorBounds::new(-5, -3)]
    );
    assert_eq!(
        decomposition
            .activation_slices()
            .iter()
            .map(|slice| slice.activation_value())
            .collect::<Vec<_>>(),
        vec![-2, -1, 0]
    );
    assert_eq!(decomposition.retained_domain_count(), 4);
    assert_eq!(decomposition.retained_domain_bound_cells(), 4);
}

#[test]
fn multiple_axes_use_a_deterministic_disjoint_first_activation_partition() {
    let parent = domain(&[false, false], &[(-2, 0), (-2, 0)]);
    let decomposition = try_decompose_inactive_activation(
        &parent,
        &shift([1, 2]),
        SpiredInactiveActivationLimits::default(),
    )
    .unwrap()
    .unwrap();

    assert_eq!(
        decomposition
            .activation_slices()
            .iter()
            .map(|slice| (
                slice.first_activation_axis_ordinal(),
                slice.activation_position(),
                slice.activation_value(),
            ))
            .collect::<Vec<_>>(),
        vec![(0, 0, 0), (1, 1, -1), (1, 1, 0)]
    );
    assert_eq!(
        decomposition.safe_interior().unwrap().bounds(),
        &[InteriorBounds::new(-2, -1), InteriorBounds::new(-2, -2),]
    );
    assert_eq!(decomposition.retained_domain_count(), 4);
    assert_eq!(decomposition.retained_domain_bound_cells(), 8);
    assert!(
        decomposition
            .safe_interior()
            .unwrap()
            .covers_shift(&[1, 2])
            .unwrap()
    );
    for slice in decomposition.activation_slices() {
        assert!(!slice.domain().covers_shift(&[1, 2]).unwrap());
        for earlier in &decomposition.affected_axes()[..slice.first_activation_axis_ordinal()] {
            let translated_upper = i128::from(slice.domain().bounds()[earlier.position()].upper())
                + i128::from(earlier.shift());
            assert!(translated_upper <= 0);
        }
        let axis = decomposition.affected_axes()[slice.first_activation_axis_ordinal()];
        assert_eq!(axis.position(), slice.activation_position());
        assert!(i128::from(slice.activation_value()) + i128::from(axis.shift()) >= 1);
    }

    // The four retained cells are pairwise disjoint and cover the whole 3x3
    // parent box exactly once.
    for first in -2..=0 {
        for second in -2..=0 {
            let point = [first, second];
            let memberships = usize::from(
                decomposition
                    .safe_interior()
                    .is_some_and(|safe| cell_contains(safe, &point)),
            ) + decomposition
                .activation_slices()
                .iter()
                .filter(|slice| cell_contains(slice.domain(), &point))
                .count();
            assert_eq!(memberships, 1, "point {point:?}");
        }
    }
}

#[test]
fn active_and_nonactivating_inactive_coordinates_need_no_decomposition() {
    let parent = domain(&[true, false, false], &[(1, 4), (-5, -3), (-4, 0)]);
    assert!(
        try_decompose_inactive_activation(
            &parent,
            &shift([2, 2, -1]),
            SpiredInactiveActivationLimits::default(),
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn a_fully_activating_first_axis_makes_later_first_activation_cells_unreachable() {
    let parent = domain(&[false, false], &[(0, 0), (0, 0)]);
    let decomposition = try_decompose_inactive_activation(
        &parent,
        &shift([1, 1]),
        SpiredInactiveActivationLimits::default(),
    )
    .unwrap()
    .unwrap();

    assert!(decomposition.safe_interior().is_none());
    assert_eq!(decomposition.affected_axes().len(), 2);
    assert_eq!(decomposition.activation_slices().len(), 1);
    assert_eq!(
        decomposition.activation_slices()[0].domain().bounds(),
        &[InteriorBounds::new(0, 0), InteriorBounds::new(0, 0)]
    );
}

#[test]
fn wrong_arity_and_every_resource_ceiling_fail_closed() {
    let one = domain(&[false], &[(-5, 0)]);
    assert_eq!(
        try_decompose_inactive_activation(
            &one,
            &shift([1, 0]),
            SpiredInactiveActivationLimits::default(),
        )
        .unwrap_err(),
        SpiredInactiveActivationError::WrongShiftArity {
            expected: 1,
            actual: 2,
        }
    );

    let defaults = SpiredInactiveActivationLimits::default();
    for (limits, expected_resource, requested, limit) in [
        (
            SpiredInactiveActivationLimits {
                max_arity: 0,
                ..defaults
            },
            "index-space arity",
            1,
            0,
        ),
        (
            SpiredInactiveActivationLimits {
                max_affected_axes: 0,
                ..defaults
            },
            "affected axes",
            1,
            0,
        ),
        (
            SpiredInactiveActivationLimits {
                max_activation_slices: 2,
                ..defaults
            },
            "activation slices",
            3,
            2,
        ),
        (
            SpiredInactiveActivationLimits {
                max_retained_domains: 3,
                ..defaults
            },
            "retained domains",
            4,
            3,
        ),
        (
            SpiredInactiveActivationLimits {
                max_retained_domain_bound_cells: 3,
                ..defaults
            },
            "retained domain bound cells",
            4,
            3,
        ),
    ] {
        assert_eq!(
            try_decompose_inactive_activation(&one, &shift([3]), limits).unwrap_err(),
            SpiredInactiveActivationError::ResourceLimit {
                resource: expected_resource,
                requested,
                limit,
            }
        );
    }

    let two = domain(&[false, false], &[(-2, 0), (-2, 0)]);
    let bounds_budget = SpiredInactiveActivationLimits {
        max_retained_domain_bound_cells: 7,
        ..defaults
    };
    assert_eq!(
        try_decompose_inactive_activation(&two, &shift([1, 2]), bounds_budget).unwrap_err(),
        SpiredInactiveActivationError::ResourceLimit {
            resource: "retained domain bound cells",
            requested: 8,
            limit: 7,
        }
    );
}

#[test]
fn exact_coefficient_zero_on_activation_face_needs_no_obligation() {
    let context = context("spired-inactive-zero-face", 1);
    let parent = stratum(&context, &[false], &[(-3, 0)]);
    let raising = shift([1]);
    let coefficient = context.index(0).unwrap();
    let terms = [SpiredReplayedInactiveActivationTerm::new(
        7,
        &raising,
        &coefficient,
    )];

    let analysis = try_analyze_replayed_inactive_activations(
        &context,
        &parent,
        &terms,
        SpiredInactiveActivationAnalysisLimits::default(),
    )
    .unwrap();

    assert_eq!(analysis.parent_stratum_id(), parent.id());
    assert!(analysis.surviving_faces().is_empty());
    assert_eq!(analysis.application_cells().len(), 2);
    assert_eq!(
        analysis
            .application_cells()
            .iter()
            .map(|cell| (
                cell.domain().bounds().to_vec(),
                cell.pruned_physical_columns().to_vec(),
            ))
            .collect::<Vec<_>>(),
        [
            (vec![InteriorBounds::new(-3, -1)], vec![]),
            (vec![InteriorBounds::new(0, 0)], vec![7]),
        ]
    );
    assert_eq!(analysis.census().candidate_terms(), 1);
    assert_eq!(analysis.census().activating_terms(), 1);
    assert_eq!(analysis.census().face_specializations(), 1);
    assert_eq!(analysis.census().vanishing_activation_slices(), 1);
    assert_eq!(analysis.census().surviving_activation_slices(), 0);
    assert_eq!(analysis.census().partition_face_values(), 1);
    assert_eq!(analysis.census().pruned_physical_column_references(), 1);
}

#[test]
fn intersecting_vanishing_faces_receive_exact_cell_local_pruning() {
    let context = context("spired-inactive-vanishing-intersection", 2);
    let parent = stratum(&context, &[false, false], &[(-3, 0), (-3, 0)]);
    let first_raising = shift([1, 0]);
    let second_raising = shift([0, 1]);
    let first_coefficient = context.index(0).unwrap();
    let second_coefficient = context.index(1).unwrap();
    let terms = [
        SpiredReplayedInactiveActivationTerm::new(17, &first_raising, &first_coefficient),
        SpiredReplayedInactiveActivationTerm::new(23, &second_raising, &second_coefficient),
    ];

    let analysis = try_analyze_replayed_inactive_activations(
        &context,
        &parent,
        &terms,
        SpiredInactiveActivationAnalysisLimits::default(),
    )
    .unwrap();

    assert!(analysis.surviving_faces().is_empty());
    assert_eq!(
        analysis
            .application_cells()
            .iter()
            .map(|cell| (
                cell.domain().bounds().to_vec(),
                cell.pruned_physical_columns().to_vec(),
            ))
            .collect::<Vec<_>>(),
        [
            (
                vec![InteriorBounds::new(-3, -1), InteriorBounds::new(-3, -1),],
                vec![],
            ),
            (
                vec![InteriorBounds::new(-3, -1), InteriorBounds::new(0, 0)],
                vec![23],
            ),
            (
                vec![InteriorBounds::new(0, 0), InteriorBounds::new(-3, -1)],
                vec![17],
            ),
            (
                vec![InteriorBounds::new(0, 0), InteriorBounds::new(0, 0)],
                vec![17, 23],
            ),
        ]
    );
    assert_eq!(analysis.census().partition_face_values(), 2);
    assert_eq!(analysis.census().application_cells(), 4);
    assert_eq!(analysis.census().pruned_physical_column_references(), 4);
}

#[test]
fn surviving_exact_coefficient_creates_face_obligation_and_safe_complement() {
    let context = context("spired-inactive-surviving-face", 1);
    let parent = stratum(&context, &[false], &[(-3, 0)]);
    let raising = shift([1]);
    let coefficient = context.one();
    let terms = [SpiredReplayedInactiveActivationTerm::new(
        11,
        &raising,
        &coefficient,
    )];

    let analysis = try_analyze_replayed_inactive_activations(
        &context,
        &parent,
        &terms,
        SpiredInactiveActivationAnalysisLimits::default(),
    )
    .unwrap();

    assert_eq!(analysis.application_cells().len(), 1);
    assert_eq!(
        analysis.application_cells()[0].domain().bounds(),
        &[InteriorBounds::new(-3, -1)]
    );
    assert!(
        analysis.application_cells()[0]
            .pruned_physical_columns()
            .is_empty()
    );
    let [face] = analysis.surviving_faces() else {
        panic!("one exact activation face must survive")
    };
    assert_eq!((face.position(), face.value()), (0, 0));
    assert_eq!(face.source_physical_columns(), &[11]);
}

#[test]
fn unrelated_working_singleton_cannot_suppress_a_broad_face_specification() {
    let context = context("spired-inactive-working-singleton", 2);
    // The first singleton is intentionally indistinguishable here from a
    // finite-depth/representability envelope restriction. It must therefore
    // not be used to justify dropping the broad n2=0 equality face.
    let parent = stratum(&context, &[false, false], &[(0, 0), (-3, 0)]);
    let raising = shift([0, 1]);
    let coefficient = context.index(0).unwrap();
    let terms = [SpiredReplayedInactiveActivationTerm::new(
        31,
        &raising,
        &coefficient,
    )];

    let analysis = try_analyze_replayed_inactive_activations(
        &context,
        &parent,
        &terms,
        SpiredInactiveActivationAnalysisLimits::default(),
    )
    .unwrap();

    let [face] = analysis.surviving_faces() else {
        panic!("working-envelope singleton must not erase a logical face")
    };
    assert_eq!((face.position(), face.value()), (1, 0));
    assert_eq!(face.source_physical_columns(), &[31]);
    assert_eq!(analysis.application_cells().len(), 1);
    assert_eq!(
        analysis.application_cells()[0].domain().bounds(),
        &[InteriorBounds::new(0, 0), InteriorBounds::new(-3, -1),]
    );
}

#[test]
fn multiple_terms_and_axes_form_one_canonical_face_union() {
    let context = context("spired-inactive-canonical-union", 2);
    let parent = stratum(&context, &[false, false], &[(-3, 0), (-3, 0)]);
    let both_axes = shift([1, 1]);
    let first_axis = shift([2, 0]);
    let coefficient = context.one();
    let forward = [
        SpiredReplayedInactiveActivationTerm::new(9, &first_axis, &coefficient),
        SpiredReplayedInactiveActivationTerm::new(3, &both_axes, &coefficient),
    ];
    let reverse = [forward[1], forward[0]];

    let forward = try_analyze_replayed_inactive_activations(
        &context,
        &parent,
        &forward,
        SpiredInactiveActivationAnalysisLimits::default(),
    )
    .unwrap();
    let reverse = try_analyze_replayed_inactive_activations(
        &context,
        &parent,
        &reverse,
        SpiredInactiveActivationAnalysisLimits::default(),
    )
    .unwrap();
    assert_eq!(forward, reverse);

    assert_eq!(
        forward
            .surviving_faces()
            .iter()
            .map(|face| (
                face.position(),
                face.value(),
                face.source_physical_columns().to_vec(),
            ))
            .collect::<Vec<_>>(),
        [(0, -1, vec![9]), (0, 0, vec![3, 9]), (1, 0, vec![3]),]
    );
    assert_eq!(forward.application_cells().len(), 1);
    assert_eq!(
        forward.application_cells()[0].domain().bounds(),
        &[InteriorBounds::new(-3, -2), InteriorBounds::new(-3, -1),]
    );
    assert_eq!(forward.census().unique_surviving_faces(), 3);
    assert_eq!(forward.census().duplicate_surviving_face_sources(), 1);
}

#[test]
fn aggregate_resource_failure_is_typed_and_retains_no_hidden_state() {
    let context = context("spired-inactive-resource-atomic", 1);
    let parent = stratum(&context, &[false], &[(-3, 0)]);
    let raising = shift([2]);
    let coefficient = context.one();
    let terms = [SpiredReplayedInactiveActivationTerm::new(
        5,
        &raising,
        &coefficient,
    )];
    let limits = SpiredInactiveActivationAnalysisLimits {
        max_face_specializations: 1,
        ..Default::default()
    };

    assert_eq!(
        try_analyze_replayed_inactive_activations(&context, &parent, &terms, limits).unwrap_err(),
        SpiredInactiveActivationError::ResourceLimit {
            resource: "activation-face specializations",
            requested: 2,
            limit: 1,
        }
    );

    // The analyzer owns all prospective buffers. A failed call leaves no
    // mutated candidate, parent, or reusable state behind.
    let complete = try_analyze_replayed_inactive_activations(
        &context,
        &parent,
        &terms,
        SpiredInactiveActivationAnalysisLimits::default(),
    )
    .unwrap();
    assert_eq!(complete.surviving_faces().len(), 2);
}

#[test]
fn specialized_application_cell_budget_fails_before_cell_materialization() {
    let context = context("spired-inactive-application-cell-budget", 1);
    let parent = stratum(&context, &[false], &[(-3, 0)]);
    let raising = shift([1]);
    let coefficient = context.index(0).unwrap();
    let terms = [SpiredReplayedInactiveActivationTerm::new(
        29,
        &raising,
        &coefficient,
    )];
    let limits = SpiredInactiveActivationAnalysisLimits {
        max_application_cells: 1,
        ..Default::default()
    };

    assert_eq!(
        try_analyze_replayed_inactive_activations(&context, &parent, &terms, limits).unwrap_err(),
        SpiredInactiveActivationError::ResourceLimit {
            resource: "activation application cells",
            requested: 2,
            limit: 1,
        }
    );
}

#[test]
fn active_coordinates_and_nonactivating_shifts_are_exact_no_ops() {
    let context = context("spired-inactive-no-op", 3);
    let parent = stratum(
        &context,
        &[true, false, false],
        &[(1, 4), (-5, -3), (-4, 0)],
    );
    let harmless = shift([3, 2, -1]);
    let coefficient = context.one();
    let terms = [SpiredReplayedInactiveActivationTerm::new(
        2,
        &harmless,
        &coefficient,
    )];

    let analysis = try_analyze_replayed_inactive_activations(
        &context,
        &parent,
        &terms,
        SpiredInactiveActivationAnalysisLimits::default(),
    )
    .unwrap();
    assert!(analysis.surviving_faces().is_empty());
    assert_eq!(analysis.application_cells().len(), 1);
    assert_eq!(
        analysis.application_cells()[0].domain().bounds(),
        parent.domain().bounds()
    );
    assert!(
        analysis.application_cells()[0]
            .pruned_physical_columns()
            .is_empty()
    );
    assert_eq!(analysis.census().activating_terms(), 0);
    assert_eq!(analysis.census().face_specializations(), 0);
}

#[test]
fn guarded_parent_geometry_fails_closed_before_coefficient_work() {
    let context = context("spired-inactive-guarded-parent", 1);
    let guard_blind = stratum(&context, &[false], &[(-3, 0)]);
    let branch = GuardBranchIdentity::try_new(
        "owner-only-activation-branch",
        GuardBranch::NonZero,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let guarded = DecoratedStratum::try_new(
        guard_blind.family_fingerprint(),
        guard_blind.context_fingerprint(),
        guard_blind.domain().clone(),
        [branch],
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let raising = shift([1]);
    let coefficient = context.one();
    let terms = [SpiredReplayedInactiveActivationTerm::new(
        1,
        &raising,
        &coefficient,
    )];

    assert_eq!(
        try_analyze_replayed_inactive_activations(
            &context,
            &guarded,
            &terms,
            SpiredInactiveActivationAnalysisLimits::default(),
        )
        .unwrap_err(),
        SpiredInactiveActivationError::GuardedParentGeometry { guard_branches: 1 }
    );
}
