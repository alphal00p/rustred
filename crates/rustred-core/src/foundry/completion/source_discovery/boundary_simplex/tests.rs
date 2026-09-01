use std::collections::BTreeSet;

use crate::foundry::completion::{LatticeBox, UncoveredPartition};
use crate::sector::Mask;

use super::super::interior_simplex::{
    InteriorSimplexLimits, InteriorSimplexScopePartition,
    try_plan_interior_simplex_samples_at_free_dimension,
};

use super::{
    BoundarySimplexLimits, BoundarySimplexPlan, BoundarySimplexPlanError,
    BoundarySimplexSamplingProfile, BoundarySimplexScopePartition,
    try_plan_boundary_simplex_samples,
};

fn lattice_box(lower: &[u64], upper: &[Option<u64>]) -> LatticeBox {
    LatticeBox::try_new(lower.iter().copied(), upper.iter().copied()).unwrap()
}

fn all_active(arity: usize) -> Mask {
    Mask::try_new(std::iter::repeat_n(true, arity)).unwrap()
}

fn simplex(margin: u64, degree: usize) -> BoundarySimplexSamplingProfile {
    BoundarySimplexSamplingProfile::Simplex {
        interior_margin: margin,
        polynomial_degree_ceiling: degree,
    }
}

fn one_scope_plan(
    sector: &Mask,
    partition: &UncoveredPartition,
    parent_dimension: usize,
    codimension: usize,
    profile: BoundarySimplexSamplingProfile,
    limits: BoundarySimplexLimits,
) -> Result<BoundarySimplexPlan, BoundarySimplexPlanError> {
    try_plan_boundary_simplex_samples(
        17,
        [BoundarySimplexScopePartition::new(
            "one-scope",
            sector,
            partition,
        )],
        parent_dimension,
        codimension,
        profile,
        limits,
    )
}

fn semantic_tasks(
    plan: &BoundarySimplexPlan,
) -> Vec<(String, Vec<usize>, usize, Vec<u64>, Vec<u64>)> {
    plan.tasks()
        .iter()
        .map(|task| {
            (
                task.key().stable_scope_key().to_owned(),
                task.key().pinned_axes().to_vec(),
                task.key().finite_assignment_ordinal(),
                task.key().simplex_offset().to_vec(),
                task.lattice_target().to_vec(),
            )
        })
        .collect()
}

#[test]
fn codimension_one_matches_brute_force_faces_assignments_and_simplex() {
    let sector = all_active(4);
    let partition = UncoveredPartition::new(
        vec![lattice_box(&[2, 4, 6, 10], &[None, None, None, Some(11)])],
        0,
    );
    let plan = one_scope_plan(
        &sector,
        &partition,
        3,
        1,
        simplex(2, 2),
        BoundarySimplexLimits::default(),
    )
    .unwrap();
    assert_eq!(plan.parent_free_dimension(), 3);
    assert_eq!(plan.boundary_codimension(), 1);
    assert_eq!(plan.face_dimension(), 2);
    assert_eq!(plan.selected_parent_box_count(), 1);
    assert_eq!(plan.boundary_face_count(), 3);
    assert_eq!(plan.parent_finite_assignment_count(), 2);
    assert_eq!(plan.face_finite_assignment_count(), 6);
    assert_eq!(plan.simplex_sample_count(), 6);
    assert_eq!(plan.tasks().len(), 36);

    let actual = plan
        .tasks()
        .iter()
        .map(|task| {
            (
                task.key().pinned_axes().to_vec(),
                task.key().finite_assignment_ordinal(),
                task.key().simplex_offset().to_vec(),
                task.lattice_target().to_vec(),
            )
        })
        .collect::<BTreeSet<_>>();
    let mut expected = BTreeSet::new();
    for pinned in 0..3 {
        let remaining = (0..3).filter(|axis| *axis != pinned).collect::<Vec<_>>();
        for bounded in 10..=11 {
            for first in 0..=2 {
                for second in 0..=2 {
                    if first + second > 2 {
                        continue;
                    }
                    let offset = vec![first, second];
                    let mut target = vec![2, 4, 6, bounded];
                    target[remaining[0]] += 2 + first;
                    target[remaining[1]] += 2 + second;
                    expected.insert((
                        vec![pinned],
                        usize::try_from(bounded - 10).unwrap(),
                        offset,
                        target,
                    ));
                }
            }
        }
    }
    assert_eq!(actual, expected);

    // Offset zero first; assignment rounds interleave all lexicographic faces.
    assert_eq!(
        plan.tasks()[..6]
            .iter()
            .map(|task| (
                task.key().pinned_axes().to_vec(),
                task.key().finite_assignment_ordinal(),
                task.key().simplex_offset().to_vec(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (vec![0], 0, vec![0, 0]),
            (vec![1], 0, vec![0, 0]),
            (vec![2], 0, vec![0, 0]),
            (vec![0], 1, vec![0, 0]),
            (vec![1], 1, vec![0, 0]),
            (vec![2], 1, vec![0, 0]),
        ]
    );
    for (ordinal, task) in plan.tasks().iter().enumerate() {
        assert_eq!(task.canonical_ordinal(), ordinal);
        assert_eq!(task.epoch_ordinal(), 17);
        assert_eq!(task.key().parent_box_lower(), &[2, 4, 6, 10]);
        assert_eq!(task.key().parent_box_upper(), &[None, None, None, Some(11)]);
    }
}

fn order_independence_plan(reverse_scopes: bool, reverse_boxes: bool) -> BoundarySimplexPlan {
    let sector = all_active(3);
    let mut alpha_boxes = vec![
        lattice_box(&[0, 0, 4], &[None, None, Some(5)]),
        lattice_box(&[10, 0, 7], &[None, None, Some(7)]),
    ];
    if reverse_boxes {
        alpha_boxes.reverse();
    }
    let alpha = UncoveredPartition::new(alpha_boxes, 0);
    let beta = UncoveredPartition::new(vec![lattice_box(&[20, 0, 8], &[None, None, Some(10)])], 0);
    let alpha_scope = BoundarySimplexScopePartition::new("alpha", &sector, &alpha);
    let beta_scope = BoundarySimplexScopePartition::new("beta", &sector, &beta);
    let scopes = if reverse_scopes {
        vec![beta_scope, alpha_scope]
    } else {
        vec![alpha_scope, beta_scope]
    };
    try_plan_boundary_simplex_samples(
        23,
        scopes,
        2,
        1,
        simplex(1, 0),
        BoundarySimplexLimits::default(),
    )
    .unwrap()
}

#[test]
fn canonical_face_and_active_assignment_chronology_ignore_input_order() {
    let forward = order_independence_plan(false, false);
    let reversed = order_independence_plan(true, true);
    assert_eq!(semantic_tasks(&forward), semantic_tasks(&reversed));
    assert_eq!(
        forward.scheduler_visit_count(),
        reversed.scheduler_visit_count()
    );
    assert_eq!(forward.boundary_face_count(), 6);
    assert_eq!(forward.face_finite_assignment_count(), 12);
    assert_eq!(forward.tasks().len(), 12);
    // Every face gets assignment zero before longer products advance.
    assert!(
        forward.tasks()[..6]
            .iter()
            .all(|task| task.key().finite_assignment_ordinal() == 0)
    );
}

#[test]
fn bulk_codimension_zero_and_all_pinned_vertex_are_explicit() {
    let sector = all_active(3);
    let bulk_partition =
        UncoveredPartition::new(vec![lattice_box(&[2, 4, 9], &[None, None, Some(10)])], 0);
    let bulk = one_scope_plan(
        &sector,
        &bulk_partition,
        2,
        0,
        simplex(1, 1),
        BoundarySimplexLimits::default(),
    )
    .unwrap();
    assert_eq!(bulk.boundary_face_count(), 1);
    assert_eq!(bulk.tasks().len(), 6);
    assert!(
        bulk.tasks()
            .iter()
            .all(|task| task.key().pinned_axes().is_empty())
    );
    assert!(
        bulk.tasks()
            .iter()
            .all(|task| task.key().remaining_axes() == [0, 1])
    );

    let vertex = one_scope_plan(
        &sector,
        &bulk_partition,
        2,
        2,
        BoundarySimplexSamplingProfile::Vertex,
        BoundarySimplexLimits::default(),
    )
    .unwrap();
    assert_eq!(vertex.face_dimension(), 0);
    assert_eq!(vertex.simplex_sample_count(), 1);
    assert_eq!(vertex.tasks().len(), 2);
    assert_eq!(
        vertex
            .tasks()
            .iter()
            .map(|task| task.lattice_target().to_vec())
            .collect::<Vec<_>>(),
        vec![vec![2, 4, 9], vec![2, 4, 10]]
    );
    assert!(
        vertex
            .tasks()
            .iter()
            .all(|task| task.key().simplex_offset().is_empty())
    );

    let finite_partition = UncoveredPartition::new(
        vec![lattice_box(&[3, 5, 7], &[Some(4), Some(6), Some(7)])],
        0,
    );
    let finite = one_scope_plan(
        &sector,
        &finite_partition,
        0,
        0,
        BoundarySimplexSamplingProfile::Vertex,
        BoundarySimplexLimits::default(),
    )
    .unwrap();
    assert_eq!(finite.tasks().len(), 4);
    assert_eq!(
        finite
            .tasks()
            .iter()
            .map(|task| task.lattice_target().to_vec())
            .collect::<Vec<_>>(),
        vec![vec![3, 5, 7], vec![3, 6, 7], vec![4, 5, 7], vec![4, 6, 7],]
    );

    assert_eq!(
        one_scope_plan(
            &sector,
            &bulk_partition,
            2,
            2,
            simplex(1, 0),
            BoundarySimplexLimits::default(),
        )
        .unwrap_err(),
        BoundarySimplexPlanError::SimplexProfileRequiresPositiveFaceDimension
    );
    assert_eq!(
        one_scope_plan(
            &sector,
            &bulk_partition,
            2,
            1,
            BoundarySimplexSamplingProfile::Vertex,
            BoundarySimplexLimits::default(),
        )
        .unwrap_err(),
        BoundarySimplexPlanError::VertexProfileRequiresZeroFaceDimension { actual: 1 }
    );
}

#[test]
fn mixed_sector_chart_conversion_and_epoch_nonaliasing_are_exact() {
    let sector = Mask::try_new([false, true, false]).unwrap();
    let partition =
        UncoveredPartition::new(vec![lattice_box(&[2, 3, 1], &[None, None, Some(1)])], 0);
    let first = one_scope_plan(
        &sector,
        &partition,
        2,
        1,
        simplex(1, 0),
        BoundarySimplexLimits::default(),
    )
    .unwrap();
    let second = one_scope_plan(
        &sector,
        &partition,
        2,
        1,
        simplex(1, 0),
        BoundarySimplexLimits::default(),
    )
    .unwrap();
    let task = &first.tasks()[0];
    assert_eq!(task.key().pinned_axes(), &[0]);
    assert_eq!(task.lattice_target(), &[2, 4, 1]);
    assert_eq!(task.target_shift().values(), &[-2, 4, -1]);
    first.validate_task(task).unwrap();
    assert!(matches!(
        second.validate_task(task),
        Err(BoundarySimplexPlanError::StaleGeometryEpoch {
            expected_ordinal: 17,
            actual_ordinal: 17,
        })
    ));
}

#[test]
fn combinatorial_and_scheduler_caps_reject_whole_design_one_below() {
    let sector = all_active(4);
    let partition = UncoveredPartition::new(vec![lattice_box(&[0; 4], &[None; 4])], 0);
    assert_eq!(
        one_scope_plan(
            &sector,
            &partition,
            4,
            2,
            simplex(1, 0),
            BoundarySimplexLimits {
                max_faces_per_parent: 5,
                ..BoundarySimplexLimits::default()
            },
        )
        .unwrap_err(),
        BoundarySimplexPlanError::ResourceLimit {
            resource: "boundary faces per parent",
            requested: 6,
            limit: 5,
        }
    );
    assert_eq!(
        one_scope_plan(
            &sector,
            &partition,
            4,
            2,
            simplex(1, 0),
            BoundarySimplexLimits {
                max_tasks: 5,
                ..BoundarySimplexLimits::default()
            },
        )
        .unwrap_err(),
        BoundarySimplexPlanError::ResourceLimit {
            resource: "boundary-simplex tasks",
            requested: 6,
            limit: 5,
        }
    );
    assert_eq!(
        one_scope_plan(
            &sector,
            &partition,
            4,
            2,
            simplex(1, 0),
            BoundarySimplexLimits {
                max_task_coordinate_cells: 47,
                ..BoundarySimplexLimits::default()
            },
        )
        .unwrap_err(),
        BoundarySimplexPlanError::ResourceLimit {
            resource: "boundary-simplex task coordinate cells",
            requested: 48,
            limit: 47,
        }
    );
    assert_eq!(
        one_scope_plan(
            &sector,
            &partition,
            4,
            2,
            simplex(1, 0),
            BoundarySimplexLimits {
                max_subset_unrank_work: 95,
                ..BoundarySimplexLimits::default()
            },
        )
        .unwrap_err(),
        BoundarySimplexPlanError::ResourceLimit {
            resource: "boundary subset unrank work",
            requested: 96,
            limit: 95,
        }
    );
    let baseline = one_scope_plan(
        &sector,
        &partition,
        4,
        2,
        simplex(1, 0),
        BoundarySimplexLimits::default(),
    )
    .unwrap();
    let visits = baseline.scheduler_visit_count();
    assert_eq!(
        one_scope_plan(
            &sector,
            &partition,
            4,
            2,
            simplex(1, 0),
            BoundarySimplexLimits {
                max_scheduler_visits: visits - 1,
                ..BoundarySimplexLimits::default()
            },
        )
        .unwrap_err(),
        BoundarySimplexPlanError::ResourceLimit {
            resource: "scheduler visits",
            requested: visits,
            limit: visits - 1,
        }
    );
}

#[test]
fn checked_combinatorial_overflow_is_typed_before_enumeration() {
    let sector = all_active(68);
    let partition = UncoveredPartition::new(vec![lattice_box(&[0; 68], &[None; 68])], 0);
    assert_eq!(
        one_scope_plan(
            &sector,
            &partition,
            68,
            34,
            simplex(1, 0),
            BoundarySimplexLimits::default(),
        )
        .unwrap_err(),
        BoundarySimplexPlanError::ResourceCountOverflow {
            resource: "boundary faces per parent",
        }
    );

    let one_axis_sector = all_active(1);
    let one_axis = UncoveredPartition::new(vec![lattice_box(&[0], &[None])], 0);
    assert_eq!(
        one_scope_plan(
            &one_axis_sector,
            &one_axis,
            1,
            0,
            simplex(1, usize::MAX),
            BoundarySimplexLimits {
                max_polynomial_degree_ceiling: usize::MAX,
                ..BoundarySimplexLimits::default()
            },
        )
        .unwrap_err(),
        BoundarySimplexPlanError::ResourceCountOverflow {
            resource: "simplex binomial upper argument",
        }
    );
}

#[test]
fn rev9_shaped_faces_construct_codimension_one_two_and_three_targets() {
    let sector = all_active(6);
    let partition = UncoveredPartition::new(
        vec![
            lattice_box(
                &[2, 4, 0, 0, 0, 0],
                &[None, None, Some(0), None, None, None],
            ),
            lattice_box(
                &[2, 4, 4, 0, 0, 0],
                &[None, None, None, Some(0), None, None],
            ),
            lattice_box(
                &[2, 4, 4, 4, 0, 0],
                &[None, None, None, None, Some(0), None],
            ),
        ],
        0,
    );
    for (codimension, target, pinned) in [
        (1, vec![4, 4, 0, 2, 2, 2], vec![1]),
        (2, vec![4, 4, 4, 0, 2, 2], vec![1, 2]),
        (3, vec![4, 4, 4, 4, 0, 2], vec![1, 2, 3]),
    ] {
        let plan = one_scope_plan(
            &sector,
            &partition,
            5,
            codimension,
            simplex(2, 0),
            BoundarySimplexLimits::default(),
        )
        .unwrap();
        let task = plan
            .tasks()
            .iter()
            .find(|task| task.lattice_target() == target)
            .expect("the generic face plan must contain the shaped boundary target");
        assert_eq!(task.key().pinned_axes(), pinned);
        assert_eq!(task.key().face_dimension(), 5 - codimension);
    }
}

#[test]
fn codimension_zero_is_exactly_the_existing_interior_design() {
    let sector = all_active(4);
    let partition = UncoveredPartition::new(
        vec![
            lattice_box(&[0, 2, 7, 9], &[None, None, Some(8), Some(9)]),
            lattice_box(&[5, 1, 3, 4], &[None, None, Some(3), Some(6)]),
        ],
        0,
    );
    let boundary = one_scope_plan(
        &sector,
        &partition,
        2,
        0,
        simplex(2, 2),
        BoundarySimplexLimits::default(),
    )
    .unwrap();
    let interior = try_plan_interior_simplex_samples_at_free_dimension(
        17,
        [InteriorSimplexScopePartition::new(
            "one-scope",
            &sector,
            &partition,
        )],
        2,
        2,
        2,
        InteriorSimplexLimits::default(),
    )
    .unwrap();

    assert_eq!(
        boundary.selected_parent_box_count(),
        interior.selected_box_count()
    );
    assert_eq!(
        boundary.boundary_face_count(),
        interior.selected_box_count()
    );
    assert_eq!(
        boundary.parent_finite_assignment_count(),
        interior.finite_assignment_count()
    );
    assert_eq!(
        boundary.face_finite_assignment_count(),
        interior.finite_assignment_count()
    );
    assert_eq!(
        boundary.simplex_sample_count(),
        interior.simplex_sample_count()
    );
    assert_eq!(boundary.tasks().len(), interior.tasks().len());
    let boundary_semantics = boundary
        .tasks()
        .iter()
        .map(|task| {
            (
                task.key().finite_assignment_ordinal(),
                task.key().simplex_offset().to_vec(),
                task.lattice_target().to_vec(),
                task.target_shift().values().to_vec(),
            )
        })
        .collect::<Vec<_>>();
    let interior_semantics = interior
        .tasks()
        .iter()
        .map(|task| {
            (
                task.key().finite_assignment_ordinal(),
                task.key().simplex_offset().to_vec(),
                task.lattice_target().to_vec(),
                task.target_shift().values().to_vec(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(boundary_semantics, interior_semantics);
}

#[test]
fn exact_parent_dimension_selection_preserves_the_true_input_maximum() {
    let sector = all_active(4);
    let partition = UncoveredPartition::new(
        vec![
            lattice_box(&[0; 4], &[None, None, None, Some(0)]),
            lattice_box(&[2; 4], &[None, None, Some(2), Some(2)]),
            lattice_box(&[4; 4], &[Some(4); 4]),
        ],
        0,
    );
    let selected = one_scope_plan(
        &sector,
        &partition,
        2,
        1,
        simplex(1, 0),
        BoundarySimplexLimits::default(),
    )
    .unwrap();
    assert_eq!(selected.parent_free_dimension(), 2);
    assert_eq!(selected.maximal_available_free_dimension(), 3);
    assert_eq!(selected.selected_parent_box_count(), 1);

    assert_eq!(
        one_scope_plan(
            &sector,
            &partition,
            5,
            0,
            simplex(1, 0),
            BoundarySimplexLimits::default(),
        )
        .unwrap_err(),
        BoundarySimplexPlanError::InvalidParentFreeDimension {
            requested: 5,
            maximal_input_arity: 4,
        }
    );
    assert_eq!(
        one_scope_plan(
            &sector,
            &partition,
            1,
            0,
            simplex(1, 0),
            BoundarySimplexLimits::default(),
        )
        .unwrap_err(),
        BoundarySimplexPlanError::ParentFreeDimensionUnavailable {
            requested: 1,
            maximal_available: 3,
        }
    );
    assert_eq!(
        one_scope_plan(
            &sector,
            &partition,
            2,
            3,
            BoundarySimplexSamplingProfile::Vertex,
            BoundarySimplexLimits::default(),
        )
        .unwrap_err(),
        BoundarySimplexPlanError::InvalidBoundaryCodimension {
            parent_free_dimension: 2,
            requested: 3,
        }
    );
}

#[test]
fn high_dimension_codimension_one_streams_exactly_sixty_four_faces() {
    let sector = all_active(64);
    let partition = UncoveredPartition::new(vec![lattice_box(&[0; 64], &[None; 64])], 0);
    let plan = one_scope_plan(
        &sector,
        &partition,
        64,
        1,
        simplex(1, 0),
        BoundarySimplexLimits::default(),
    )
    .unwrap();
    assert_eq!(plan.boundary_face_count(), 64);
    assert_eq!(plan.tasks().len(), 64);
    assert_eq!(plan.subset_unrank_work_upper_bound(), 64 * 64 * 64);
    assert_eq!(plan.scheduler_visit_count(), 195);
    assert_eq!(plan.scheduler_workspace_entries(), 192);
    assert_eq!(
        plan.tasks()
            .iter()
            .map(|task| task.key().pinned_axes()[0])
            .collect::<Vec<_>>(),
        (0..64).collect::<Vec<_>>()
    );
}

#[test]
fn codimension_two_matches_a_d4_product_oracle_and_exact_telemetry() {
    let sector = all_active(5);
    let alpha = UncoveredPartition::new(
        vec![lattice_box(
            &[0, 2, 4, 6, 10],
            &[None, None, None, None, Some(11)],
        )],
        0,
    );
    let beta = UncoveredPartition::new(
        vec![lattice_box(
            &[20, 22, 24, 26, 30],
            &[None, None, None, None, Some(32)],
        )],
        0,
    );
    let plan = try_plan_boundary_simplex_samples(
        31,
        [
            BoundarySimplexScopePartition::new("beta", &sector, &beta),
            BoundarySimplexScopePartition::new("alpha", &sector, &alpha),
        ],
        4,
        2,
        simplex(1, 2),
        BoundarySimplexLimits::default(),
    )
    .unwrap();

    let q = 6usize;
    let parent_count = 2usize;
    let round_count = 1usize;
    let assignment_sum = 5usize;
    let face_count = q * parent_count;
    let face_assignments = q * assignment_sum;
    let simplex_count = 6usize;
    assert_eq!(plan.boundary_face_count(), face_count);
    assert_eq!(plan.face_finite_assignment_count(), face_assignments);
    assert_eq!(plan.simplex_sample_count(), simplex_count);
    assert_eq!(plan.tasks().len(), face_assignments * simplex_count);
    assert_eq!(plan.subset_unrank_work_upper_bound(), face_count * 4 * 4);
    assert_eq!(
        plan.scheduler_visit_count(),
        2 * parent_count
            + round_count
            + face_count
            + simplex_count * (face_count + face_assignments)
    );
    assert_eq!(
        plan.scheduler_workspace_entries(),
        (2 * parent_count + 2 * round_count + face_count).max(3 * face_count)
    );

    let actual = plan
        .tasks()
        .iter()
        .map(|task| {
            (
                task.key().stable_scope_key().to_owned(),
                task.key().pinned_axes().to_vec(),
                task.key().finite_assignment_ordinal(),
                task.key().simplex_offset().to_vec(),
                task.lattice_target().to_vec(),
            )
        })
        .collect::<BTreeSet<_>>();
    let mut expected = BTreeSet::new();
    for (scope_key, lower, finite_values) in [
        ("alpha", [0, 2, 4, 6], 10..=11),
        ("beta", [20, 22, 24, 26], 30..=32),
    ] {
        for first_pinned in 0..4 {
            for second_pinned in (first_pinned + 1)..4 {
                let pinned = vec![first_pinned, second_pinned];
                let remaining = (0..4)
                    .filter(|axis| !pinned.contains(axis))
                    .collect::<Vec<_>>();
                for finite_value in finite_values.clone() {
                    for first_offset in 0..=2 {
                        for second_offset in 0..=2 {
                            if first_offset + second_offset > 2 {
                                continue;
                            }
                            let mut target = lower.to_vec();
                            target.push(finite_value);
                            target[remaining[0]] += 1 + first_offset;
                            target[remaining[1]] += 1 + second_offset;
                            expected.insert((
                                scope_key.to_owned(),
                                pinned.clone(),
                                usize::try_from(finite_value - *finite_values.start()).unwrap(),
                                vec![first_offset, second_offset],
                                target,
                            ));
                        }
                    }
                }
            }
        }
    }
    assert_eq!(actual, expected);
}

#[test]
fn aggregate_caps_precede_input_retention_and_worst_target_conversion() {
    let sector = all_active(4);
    let two_boxes = UncoveredPartition::new(
        vec![
            lattice_box(&[0; 4], &[None; 4]),
            lattice_box(&[2; 4], &[None; 4]),
        ],
        0,
    );
    assert_eq!(
        one_scope_plan(
            &sector,
            &two_boxes,
            4,
            1,
            simplex(1, 0),
            BoundarySimplexLimits {
                max_input_boxes: 1,
                ..BoundarySimplexLimits::default()
            },
        )
        .unwrap_err(),
        BoundarySimplexPlanError::ResourceLimit {
            resource: "input uncovered boxes",
            requested: 2,
            limit: 1,
        }
    );

    let overflowing_target =
        UncoveredPartition::new(vec![lattice_box(&[u64::MAX; 4], &[None; 4])], 0);
    assert_eq!(
        one_scope_plan(
            &sector,
            &overflowing_target,
            4,
            2,
            simplex(1, 0),
            BoundarySimplexLimits {
                max_subset_unrank_work: 95,
                ..BoundarySimplexLimits::default()
            },
        )
        .unwrap_err(),
        BoundarySimplexPlanError::ResourceLimit {
            resource: "boundary subset unrank work",
            requested: 96,
            limit: 95,
        }
    );

    let baseline = one_scope_plan(
        &sector,
        &two_boxes,
        4,
        1,
        simplex(1, 0),
        BoundarySimplexLimits::default(),
    )
    .unwrap();
    assert_eq!(
        one_scope_plan(
            &sector,
            &two_boxes,
            4,
            1,
            simplex(1, 0),
            BoundarySimplexLimits {
                max_scheduler_workspace_entries: baseline.scheduler_workspace_entries() - 1,
                ..BoundarySimplexLimits::default()
            },
        )
        .unwrap_err(),
        BoundarySimplexPlanError::ResourceLimit {
            resource: "scheduler workspace entries",
            requested: baseline.scheduler_workspace_entries(),
            limit: baseline.scheduler_workspace_entries() - 1,
        }
    );
}
