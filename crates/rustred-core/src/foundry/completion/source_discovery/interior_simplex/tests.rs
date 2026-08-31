use std::collections::BTreeSet;

use crate::foundry::completion::{LatticeBox, UncoveredPartition};
use crate::sector::Mask;

use super::simplex::try_simplex_sample_count;
use super::{
    InteriorSimplexLimits, InteriorSimplexPlan, InteriorSimplexPlanError,
    InteriorSimplexScopePartition, InteriorSimplexTaskKey, try_plan_interior_simplex_samples,
};

fn lattice_box(lower: &[u64], upper: &[Option<u64>]) -> LatticeBox {
    LatticeBox::try_new(lower.iter().copied(), upper.iter().copied()).unwrap()
}

fn all_active(arity: usize) -> Mask {
    Mask::try_new(std::iter::repeat_n(true, arity)).unwrap()
}

fn one_box_plan(
    sector: &Mask,
    partition: &UncoveredPartition,
    margin: u64,
    degree: usize,
) -> InteriorSimplexPlan {
    try_plan_interior_simplex_samples(
        17,
        [InteriorSimplexScopePartition::new(
            "one-scope",
            sector,
            partition,
        )],
        margin,
        degree,
        InteriorSimplexLimits::default(),
    )
    .unwrap()
}

#[test]
fn complete_simplex_matches_brute_force_without_duplicates() {
    let sector = all_active(3);
    let partition = UncoveredPartition::new(vec![lattice_box(&[2, 4, 6], &[None, None, None])], 0);
    let plan = one_box_plan(&sector, &partition, 2, 3);
    assert_eq!(plan.epoch_ordinal(), 17);
    assert_eq!(plan.input_scope_count(), 1);
    assert_eq!(plan.selected_scope_count(), 1);
    assert_eq!(plan.selected_box_count(), 1);
    assert_eq!(plan.maximal_free_dimension(), 3);
    assert_eq!(plan.interior_margin(), 2);
    assert_eq!(plan.polynomial_degree_ceiling(), 3);
    assert_eq!(plan.simplex_sample_count(), 20);
    assert_eq!(plan.tasks().len(), 20);

    let actual: BTreeSet<_> = plan
        .tasks()
        .iter()
        .map(|task| task.key().simplex_offset().to_vec())
        .collect();
    let mut expected = BTreeSet::new();
    for first in 0..=3 {
        for second in 0..=3 {
            for third in 0..=3 {
                if first + second + third <= 3 {
                    expected.insert(vec![first, second, third]);
                }
            }
        }
    }
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), plan.tasks().len());
    for task in plan.tasks() {
        assert_eq!(task.key().box_lower(), &[2, 4, 6]);
        assert_eq!(task.key().box_upper(), &[None, None, None]);
        assert_eq!(task.key().sector(), &sector);
        assert_eq!(
            task.target_shift().values(),
            task.lattice_target()
                .iter()
                .map(|&coordinate| i64::try_from(coordinate).unwrap())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn one_free_axis_is_the_complete_interval_and_bounded_axes_stay_at_lower_endpoints() {
    let sector = all_active(3);
    let partition = UncoveredPartition::new(
        vec![
            lattice_box(&[4, 7, 9], &[Some(4), None, Some(12)]),
            // A lower-free-dimension component is intentionally ignored.
            lattice_box(&[20, 0, 0], &[Some(20), Some(3), Some(2)]),
        ],
        0,
    );
    let plan = one_box_plan(&sector, &partition, 2, 4);
    assert_eq!(plan.maximal_free_dimension(), 1);
    assert_eq!(plan.selected_box_count(), 1);
    assert_eq!(plan.simplex_sample_count(), 5);
    assert_eq!(
        plan.tasks()
            .iter()
            .map(|task| task.lattice_target().to_vec())
            .collect::<Vec<_>>(),
        vec![
            vec![4, 9, 9],
            vec![4, 10, 9],
            vec![4, 11, 9],
            vec![4, 12, 9],
            vec![4, 13, 9],
        ]
    );
}

#[test]
fn mixed_sector_signs_are_converted_by_the_sector_chart() {
    let sector = Mask::try_new([true, false, true]).unwrap();
    let partition =
        UncoveredPartition::new(vec![lattice_box(&[2, 3, 5], &[None, None, Some(7)])], 0);
    let plan = one_box_plan(&sector, &partition, 2, 1);
    assert_eq!(
        plan.tasks()
            .iter()
            .map(|task| {
                (
                    task.key().simplex_offset().to_vec(),
                    task.lattice_target().to_vec(),
                    task.target_shift().values().to_vec(),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (vec![0, 0], vec![4, 5, 5], vec![4, -5, 5]),
            (vec![0, 1], vec![4, 6, 5], vec![4, -6, 5]),
            (vec![1, 0], vec![5, 5, 5], vec![5, -5, 5]),
        ]
    );
}

#[derive(Debug, PartialEq, Eq)]
struct SemanticTask {
    canonical_ordinal: usize,
    key: InteriorSimplexTaskKey,
    lattice_target: Vec<u64>,
    target_shift: Vec<i64>,
}

fn semantic_tasks(plan: &InteriorSimplexPlan) -> Vec<SemanticTask> {
    plan.tasks()
        .iter()
        .map(|task| SemanticTask {
            canonical_ordinal: task.canonical_ordinal(),
            key: task.key().clone(),
            lattice_target: task.lattice_target().to_vec(),
            target_shift: task.target_shift().values().to_vec(),
        })
        .collect()
}

fn two_scope_plan(reverse_inputs: bool, reverse_boxes: bool, epoch: u64) -> InteriorSimplexPlan {
    let sector = all_active(3);
    let path = UncoveredPartition::new(vec![lattice_box(&[0, 0, 0], &[Some(0), None, None])], 0);
    let mut star_boxes = vec![
        lattice_box(&[1, 0, 0], &[Some(1), None, None]),
        lattice_box(&[2, 0, 0], &[Some(2), None, None]),
    ];
    if reverse_boxes {
        star_boxes.reverse();
    }
    let star = UncoveredPartition::new(star_boxes, 0);
    let path_scope = InteriorSimplexScopePartition::new("z-path", &sector, &path);
    let star_scope = InteriorSimplexScopePartition::new("a-star", &sector, &star);
    let scopes = if reverse_inputs {
        vec![star_scope, path_scope]
    } else {
        vec![path_scope, star_scope]
    };
    try_plan_interior_simplex_samples(epoch, scopes, 1, 1, InteriorSimplexLimits::default())
        .unwrap()
}

#[test]
fn input_and_box_order_do_not_change_scope_round_robin_or_worker_merge_order() {
    let forward = two_scope_plan(false, false, 31);
    let reversed = two_scope_plan(true, true, 32);
    assert_eq!(semantic_tasks(&forward), semantic_tasks(&reversed));
    assert_eq!(forward.tasks().len(), 9);

    // Each simplex offset visits the first box in every represented scope
    // before returning to the larger scope's second box.
    for chunk in forward.tasks().chunks_exact(3) {
        assert_eq!(chunk[0].key().stable_scope_key(), "z-path");
        assert_eq!(chunk[1].key().stable_scope_key(), "a-star");
        assert_eq!(chunk[2].key().stable_scope_key(), "a-star");
    }

    let expected: Vec<_> = forward
        .tasks()
        .iter()
        .map(|task| (task.canonical_ordinal(), task.key().clone()))
        .collect();
    for worker_count in 1..=4 {
        let mut simulated_completion = Vec::new();
        for worker in (0..worker_count).rev() {
            simulated_completion.extend(
                forward
                    .tasks()
                    .iter()
                    .filter(|task| task.canonical_ordinal() % worker_count == worker)
                    .rev()
                    .map(|task| (task.canonical_ordinal(), task.key().clone())),
            );
        }
        simulated_completion.sort_unstable_by_key(|(ordinal, _)| *ordinal);
        assert_eq!(simulated_completion, expected);
    }
}

#[test]
fn coordinate_overflow_and_representative_global_caps_reject_the_whole_design() {
    let inactive = Mask::try_new([false]).unwrap();
    let overflowing = UncoveredPartition::new(vec![lattice_box(&[u64::MAX], &[None])], 0);
    assert!(matches!(
        try_plan_interior_simplex_samples(
            1,
            [InteriorSimplexScopePartition::new(
                "overflow",
                &inactive,
                &overflowing,
            )],
            1,
            0,
            InteriorSimplexLimits::default(),
        ),
        Err(InteriorSimplexPlanError::CoordinateOverflow {
            canonical_scope_ordinal: 0,
            box_ordinal: 0,
            position: 0,
        })
    ));

    let sector = all_active(2);
    let partition = UncoveredPartition::new(vec![lattice_box(&[0, 0], &[None, None])], 0);
    let scope = || InteriorSimplexScopePartition::new("capped", &sector, &partition);
    assert!(matches!(
        try_plan_interior_simplex_samples(
            1,
            [scope()],
            1,
            0,
            InteriorSimplexLimits {
                max_input_boxes: 0,
                ..InteriorSimplexLimits::default()
            },
        ),
        Err(InteriorSimplexPlanError::ResourceLimit {
            resource: "input uncovered boxes",
            requested: 1,
            limit: 0,
        })
    ));
    assert!(matches!(
        try_plan_interior_simplex_samples(
            1,
            [scope()],
            1,
            0,
            InteriorSimplexLimits {
                max_selected_boxes: 0,
                ..InteriorSimplexLimits::default()
            },
        ),
        Err(InteriorSimplexPlanError::ResourceLimit {
            resource: "selected maximal boxes",
            requested: 1,
            limit: 0,
        })
    ));
    assert_eq!(
        try_plan_interior_simplex_samples(2, [scope()], 0, 0, InteriorSimplexLimits::default(),)
            .unwrap_err(),
        InteriorSimplexPlanError::ZeroInteriorMargin
    );
    assert!(matches!(
        try_plan_interior_simplex_samples(
            3,
            [scope()],
            1,
            2,
            InteriorSimplexLimits {
                max_simplex_samples: 5,
                ..InteriorSimplexLimits::default()
            },
        ),
        Err(InteriorSimplexPlanError::ResourceLimit {
            resource: "complete simplex samples",
            requested: 6,
            limit: 5,
        })
    ));
    assert!(matches!(
        try_plan_interior_simplex_samples(
            4,
            [scope()],
            1,
            2,
            InteriorSimplexLimits {
                max_simplex_coordinate_cells: 11,
                ..InteriorSimplexLimits::default()
            },
        ),
        Err(InteriorSimplexPlanError::ResourceLimit {
            resource: "simplex offset coordinate cells",
            requested: 12,
            limit: 11,
        })
    ));
    assert!(matches!(
        try_plan_interior_simplex_samples(
            5,
            [scope()],
            1,
            2,
            InteriorSimplexLimits {
                max_tasks: 5,
                ..InteriorSimplexLimits::default()
            },
        ),
        Err(InteriorSimplexPlanError::ResourceLimit {
            resource: "interior-simplex tasks",
            requested: 6,
            limit: 5,
        })
    ));
    assert!(matches!(
        try_plan_interior_simplex_samples(
            6,
            [scope()],
            1,
            2,
            InteriorSimplexLimits {
                // Six tasks retain two target and two shift coordinates.
                max_task_coordinate_cells: 23,
                ..InteriorSimplexLimits::default()
            },
        ),
        Err(InteriorSimplexPlanError::ResourceLimit {
            resource: "interior-simplex task coordinate cells",
            requested: 24,
            limit: 23,
        })
    ));
    assert!(matches!(
        try_plan_interior_simplex_samples(
            7,
            [scope()],
            2,
            0,
            InteriorSimplexLimits {
                max_interior_margin: 1,
                ..InteriorSimplexLimits::default()
            },
        ),
        Err(InteriorSimplexPlanError::ValueLimit {
            resource: "interior margin",
            requested: 2,
            limit: 1,
        })
    ));
    assert!(matches!(
        try_simplex_sample_count(usize::MAX, 1),
        Err(InteriorSimplexPlanError::ResourceCountOverflow {
            resource: "simplex binomial upper argument",
        })
    ));
}

#[test]
fn rebuilt_equal_geometry_invalidates_old_work_without_granting_outcome_authority() {
    let sector = all_active(2);
    let partition = UncoveredPartition::new(vec![lattice_box(&[0, 0], &[None, None])], 0);
    let old = one_box_plan(&sector, &partition, 1, 1);
    let rebuilt = one_box_plan(&sector, &partition, 1, 1);
    let old_task = &old.tasks()[0];
    assert!(old.validate_task(old_task).is_ok());
    assert!(matches!(
        rebuilt.validate_task(old_task),
        Err(InteriorSimplexPlanError::StaleGeometryEpoch {
            expected_ordinal: 17,
            actual_ordinal: 17,
        })
    ));
}

#[test]
fn task_identity_binds_the_positive_margin_that_changes_its_target() {
    let sector = all_active(2);
    let partition = UncoveredPartition::new(vec![lattice_box(&[0, 0], &[None, None])], 0);
    let margin_one = one_box_plan(&sector, &partition, 1, 0);
    let margin_two = one_box_plan(&sector, &partition, 2, 0);
    let first = &margin_one.tasks()[0];
    let second = &margin_two.tasks()[0];

    assert_eq!(first.key().interior_margin(), 1);
    assert_eq!(second.key().interior_margin(), 2);
    assert_ne!(first.key(), second.key());
    assert_ne!(first.lattice_target(), second.lattice_target());
    assert_ne!(first.target_shift(), second.target_shift());
}

#[test]
fn interior_sample_reveals_product_invisible_to_corner_and_axis_boundary_probes() {
    let boundary_probes = [[0_i64, 0_i64], [1, 0], [0, 1]];
    assert!(boundary_probes.iter().all(|point| point[0] * point[1] == 0));

    let sector = all_active(2);
    let partition = UncoveredPartition::new(vec![lattice_box(&[0, 0], &[None, None])], 0);
    let plan = one_box_plan(&sector, &partition, 1, 0);
    assert_eq!(plan.tasks().len(), 1);
    let interior = plan.tasks()[0].lattice_target();
    assert_eq!(interior, &[1, 1]);
    assert_ne!(interior[0] * interior[1], 0);
}
