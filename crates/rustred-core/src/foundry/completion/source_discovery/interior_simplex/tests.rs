use std::collections::BTreeSet;

use crate::foundry::completion::{LatticeBox, UncoveredPartition};
use crate::sector::Mask;

use super::simplex::try_simplex_sample_count;
use super::{
    InteriorSimplexFreeDimensionSelection, InteriorSimplexLimits, InteriorSimplexPlan,
    InteriorSimplexPlanError, InteriorSimplexScopePartition, InteriorSimplexTaskKey,
    try_plan_interior_simplex_samples, try_plan_interior_simplex_samples_at_free_dimension,
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
    assert_eq!(plan.finite_assignment_count(), 1);
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
        assert_eq!(task.key().finite_assignment_ordinal(), 0);
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
fn one_free_axis_pairs_every_simplex_offset_with_all_finite_assignments() {
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
    assert_eq!(plan.finite_assignment_count(), 4);
    assert_eq!(plan.simplex_sample_count(), 5);
    assert_eq!(plan.tasks().len(), 20);
    let mut expected = Vec::new();
    for free_offset in 0..=4 {
        for bounded in 9..=12 {
            expected.push(vec![4, 9 + free_offset, bounded]);
        }
    }
    assert_eq!(
        plan.tasks()
            .iter()
            .map(|task| task.lattice_target().to_vec())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        plan.tasks()
            .iter()
            .map(|task| task.key().finite_assignment_ordinal())
            .collect::<Vec<_>>(),
        (0..5).flat_map(|_| 0..4).collect::<Vec<_>>()
    );
}

#[test]
fn zero_one_slab_enumerates_both_bounded_layers() {
    let sector = all_active(2);
    let partition = UncoveredPartition::new(vec![lattice_box(&[0, 0], &[Some(1), None])], 0);
    let plan = one_box_plan(&sector, &partition, 1, 0);
    assert_eq!(plan.finite_assignment_count(), 2);
    assert_eq!(
        plan.tasks()
            .iter()
            .map(|task| {
                (
                    task.key().finite_assignment_ordinal(),
                    task.lattice_target().to_vec(),
                )
            })
            .collect::<Vec<_>>(),
        vec![(0, vec![0, 1]), (1, vec![1, 1])]
    );
    assert_ne!(plan.tasks()[0].key(), plan.tasks()[1].key());
}

#[test]
fn mixed_boxes_stream_cartesian_products_in_fair_assignment_rounds() {
    let sector = all_active(3);
    let partition = UncoveredPartition::new(
        vec![
            lattice_box(&[0, 10, 3], &[Some(1), None, Some(4)]),
            lattice_box(&[2, 20, 5], &[Some(2), None, Some(7)]),
        ],
        0,
    );
    let plan = one_box_plan(&sector, &partition, 1, 0);
    assert_eq!(plan.selected_box_count(), 2);
    assert_eq!(plan.finite_assignment_count(), 7);
    assert_eq!(
        plan.tasks()
            .iter()
            .map(|task| task.lattice_target().to_vec())
            .collect::<Vec<_>>(),
        vec![
            vec![0, 11, 3],
            vec![2, 21, 5],
            vec![0, 11, 4],
            vec![2, 21, 6],
            vec![1, 11, 3],
            vec![2, 21, 7],
            vec![1, 11, 4],
        ]
    );
}

#[test]
fn skewed_assignment_products_visit_only_live_boxes_in_canonical_order() {
    let sector = all_active(2);
    let long = UncoveredPartition::new(vec![lattice_box(&[0, 0], &[Some(4_095), None])], 0);
    let short = UncoveredPartition::new(
        (0..64)
            .map(|ordinal| {
                let coordinate = 10_000 + ordinal;
                lattice_box(&[coordinate, 0], &[Some(coordinate), None])
            })
            .collect(),
        0,
    );
    let plan = try_plan_interior_simplex_samples(
        51,
        [
            InteriorSimplexScopePartition::new("short", &sector, &short),
            InteriorSimplexScopePartition::new("long", &sector, &long),
        ],
        1,
        0,
        InteriorSimplexLimits::default(),
    )
    .unwrap();

    assert_eq!(plan.selected_box_count(), 65);
    assert_eq!(plan.finite_assignment_count(), 4_160);
    assert_eq!(plan.tasks().len(), 4_160);
    assert_eq!(plan.scheduler_workspace_entries(), 195);
    // Flattening: 2 * 65 boxes + 64 rounds. Offset seeding: 65 boxes.
    // The active frontier then visits exactly one live box per emitted task.
    assert_eq!(plan.scheduler_visit_count(), 194 + 65 + 4_160);
    let rectangular_visits = 4_096usize * 64 * 2;
    assert!(rectangular_visits > 100 * plan.scheduler_visit_count());

    let tasks = plan.tasks();
    assert_eq!(tasks[0].key().stable_scope_key(), "long");
    assert_eq!(tasks[0].key().finite_assignment_ordinal(), 0);
    assert_eq!(tasks[0].lattice_target(), &[0, 1]);
    for (short_ordinal, task) in tasks[1..65].iter().enumerate() {
        assert_eq!(task.key().stable_scope_key(), "short");
        assert_eq!(task.key().finite_assignment_ordinal(), 0);
        assert_eq!(task.lattice_target(), &[10_000 + short_ordinal as u64, 1]);
    }
    for (assignment_ordinal, task) in tasks[65..].iter().enumerate() {
        let assignment_ordinal = assignment_ordinal + 1;
        assert_eq!(task.key().stable_scope_key(), "long");
        assert_eq!(task.key().finite_assignment_ordinal(), assignment_ordinal);
        assert_eq!(
            task.lattice_target(),
            &[u64::try_from(assignment_ordinal).unwrap(), 1]
        );
    }
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
            (vec![0, 0], vec![4, 5, 6], vec![4, -5, 6]),
            (vec![0, 0], vec![4, 5, 7], vec![4, -5, 7]),
            (vec![0, 1], vec![4, 6, 5], vec![4, -6, 5]),
            (vec![0, 1], vec![4, 6, 6], vec![4, -6, 6]),
            (vec![0, 1], vec![4, 6, 7], vec![4, -6, 7]),
            (vec![1, 0], vec![5, 5, 5], vec![5, -5, 5]),
            (vec![1, 0], vec![5, 5, 6], vec![5, -5, 6]),
            (vec![1, 0], vec![5, 5, 7], vec![5, -5, 7]),
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

fn finite_product_two_scope_plan(reverse_inputs: bool, reverse_boxes: bool) -> InteriorSimplexPlan {
    let sector = all_active(3);
    let alpha =
        UncoveredPartition::new(vec![lattice_box(&[0, 0, 0], &[Some(1), None, Some(1)])], 0);
    let mut beta_boxes = vec![
        lattice_box(&[2, 0, 0], &[Some(3), None, Some(0)]),
        lattice_box(&[4, 0, 0], &[Some(4), None, Some(2)]),
    ];
    if reverse_boxes {
        beta_boxes.reverse();
    }
    let beta = UncoveredPartition::new(beta_boxes, 0);
    let alpha_scope = InteriorSimplexScopePartition::new("z-alpha", &sector, &alpha);
    let beta_scope = InteriorSimplexScopePartition::new("a-beta", &sector, &beta);
    let scopes = if reverse_inputs {
        vec![beta_scope, alpha_scope]
    } else {
        vec![alpha_scope, beta_scope]
    };
    try_plan_interior_simplex_samples(41, scopes, 1, 0, InteriorSimplexLimits::default()).unwrap()
}

#[test]
fn finite_product_chronology_is_independent_of_scope_and_box_input_order() {
    let forward = finite_product_two_scope_plan(false, false);
    let reversed = finite_product_two_scope_plan(true, true);
    assert_eq!(forward.finite_assignment_count(), 9);
    assert_eq!(semantic_tasks(&forward), semantic_tasks(&reversed));
    assert_eq!(
        forward
            .tasks()
            .iter()
            .map(|task| task.canonical_ordinal())
            .collect::<Vec<_>>(),
        (0..forward.tasks().len()).collect::<Vec<_>>()
    );
}

fn mixed_dimension_plan(
    reverse_inputs: bool,
    reverse_boxes: bool,
    selection: InteriorSimplexFreeDimensionSelection,
    limits: InteriorSimplexLimits,
) -> Result<InteriorSimplexPlan, InteriorSimplexPlanError> {
    let sector = all_active(3);
    let mut alpha_boxes = vec![
        lattice_box(&[0, 0, 0], &[None, None, Some(0)]),
        lattice_box(&[10, 0, 1], &[Some(11), None, Some(2)]),
    ];
    if reverse_boxes {
        alpha_boxes.reverse();
    }
    let alpha = UncoveredPartition::new(alpha_boxes, 0);
    let beta = UncoveredPartition::new(
        vec![lattice_box(&[20, 0, 3], &[Some(20), None, Some(5)])],
        0,
    );
    let alpha_scope = InteriorSimplexScopePartition::new("a-alpha", &sector, &alpha);
    let beta_scope = InteriorSimplexScopePartition::new("z-beta", &sector, &beta);
    let scopes = if reverse_inputs {
        vec![beta_scope, alpha_scope]
    } else {
        vec![alpha_scope, beta_scope]
    };
    match selection {
        InteriorSimplexFreeDimensionSelection::Maximal => {
            try_plan_interior_simplex_samples(71, scopes, 1, 0, limits)
        }
        InteriorSimplexFreeDimensionSelection::Exact(dimension) => {
            try_plan_interior_simplex_samples_at_free_dimension(71, scopes, dimension, 1, 0, limits)
        }
    }
}

#[test]
fn maximal_then_exact_lower_dimension_selects_all_boxes_and_assignments() {
    let maximal = mixed_dimension_plan(
        false,
        false,
        InteriorSimplexFreeDimensionSelection::Maximal,
        InteriorSimplexLimits::default(),
    )
    .unwrap();
    assert_eq!(
        maximal.free_dimension_selection(),
        InteriorSimplexFreeDimensionSelection::Maximal
    );
    assert_eq!(maximal.maximal_free_dimension(), 2);
    assert_eq!(maximal.selected_free_dimension(), 2);
    assert_eq!(maximal.selected_scope_count(), 1);
    assert_eq!(maximal.selected_box_count(), 1);
    assert_eq!(maximal.finite_assignment_count(), 1);
    assert_eq!(maximal.tasks()[0].lattice_target(), &[1, 1, 0]);

    let exact = mixed_dimension_plan(
        false,
        false,
        InteriorSimplexFreeDimensionSelection::Exact(1),
        InteriorSimplexLimits::default(),
    )
    .unwrap();
    assert_eq!(
        exact.free_dimension_selection(),
        InteriorSimplexFreeDimensionSelection::Exact(1)
    );
    assert_eq!(exact.maximal_free_dimension(), 2);
    assert_eq!(exact.selected_free_dimension(), 1);
    assert_eq!(exact.selected_scope_count(), 2);
    assert_eq!(exact.selected_box_count(), 2);
    assert_eq!(exact.finite_assignment_count(), 7);
    assert_eq!(exact.simplex_sample_count(), 1);
    assert_eq!(
        exact
            .tasks()
            .iter()
            .map(|task| task.lattice_target().to_vec())
            .collect::<Vec<_>>(),
        vec![
            vec![10, 1, 1],
            vec![20, 1, 3],
            vec![10, 1, 2],
            vec![20, 1, 4],
            vec![11, 1, 1],
            vec![20, 1, 5],
            vec![11, 1, 2],
        ]
    );
}

#[test]
fn exact_dimension_is_input_order_independent_and_matches_maximal_at_the_maximum() {
    let forward = mixed_dimension_plan(
        false,
        false,
        InteriorSimplexFreeDimensionSelection::Exact(1),
        InteriorSimplexLimits::default(),
    )
    .unwrap();
    let reversed = mixed_dimension_plan(
        true,
        true,
        InteriorSimplexFreeDimensionSelection::Exact(1),
        InteriorSimplexLimits::default(),
    )
    .unwrap();
    assert_eq!(semantic_tasks(&forward), semantic_tasks(&reversed));
    assert_eq!(
        forward.scheduler_visit_count(),
        reversed.scheduler_visit_count()
    );

    let maximal = mixed_dimension_plan(
        false,
        false,
        InteriorSimplexFreeDimensionSelection::Maximal,
        InteriorSimplexLimits::default(),
    )
    .unwrap();
    let exact_maximal = mixed_dimension_plan(
        false,
        false,
        InteriorSimplexFreeDimensionSelection::Exact(2),
        InteriorSimplexLimits::default(),
    )
    .unwrap();
    assert_eq!(semantic_tasks(&maximal), semantic_tasks(&exact_maximal));
    assert_eq!(
        maximal.selected_scope_count(),
        exact_maximal.selected_scope_count()
    );
    assert_eq!(
        maximal.selected_box_count(),
        exact_maximal.selected_box_count()
    );
    assert_eq!(
        maximal.finite_assignment_count(),
        exact_maximal.finite_assignment_count()
    );
    assert_eq!(
        maximal.scheduler_workspace_entries(),
        exact_maximal.scheduler_workspace_entries()
    );
    assert_eq!(
        maximal.scheduler_visit_count(),
        exact_maximal.scheduler_visit_count()
    );
    assert!(matches!(
        exact_maximal.validate_task(&maximal.tasks()[0]),
        Err(InteriorSimplexPlanError::StaleGeometryEpoch {
            expected_ordinal: 71,
            actual_ordinal: 71,
        })
    ));
}

#[test]
fn exact_dimension_rejects_zero_invalid_unavailable_and_one_below_resources() {
    assert_eq!(
        mixed_dimension_plan(
            false,
            false,
            InteriorSimplexFreeDimensionSelection::Exact(0),
            InteriorSimplexLimits::default(),
        )
        .unwrap_err(),
        InteriorSimplexPlanError::ZeroRequestedFreeDimension
    );
    assert_eq!(
        mixed_dimension_plan(
            false,
            false,
            InteriorSimplexFreeDimensionSelection::Exact(4),
            InteriorSimplexLimits::default(),
        )
        .unwrap_err(),
        InteriorSimplexPlanError::InvalidRequestedFreeDimension {
            requested: 4,
            maximal_input_arity: 3,
        }
    );

    let sector = all_active(3);
    let higher_only =
        UncoveredPartition::new(vec![lattice_box(&[0, 0, 0], &[None, None, Some(0)])], 0);
    assert_eq!(
        try_plan_interior_simplex_samples_at_free_dimension(
            71,
            [InteriorSimplexScopePartition::new(
                "higher-only",
                &sector,
                &higher_only,
            )],
            1,
            1,
            0,
            InteriorSimplexLimits::default(),
        )
        .unwrap_err(),
        InteriorSimplexPlanError::RequestedFreeDimensionUnavailable {
            requested: 1,
            maximal_available: 2,
        }
    );

    assert_eq!(
        mixed_dimension_plan(
            false,
            false,
            InteriorSimplexFreeDimensionSelection::Exact(3),
            InteriorSimplexLimits::default(),
        )
        .unwrap_err(),
        InteriorSimplexPlanError::RequestedFreeDimensionUnavailable {
            requested: 3,
            maximal_available: 2,
        }
    );
    assert_eq!(
        mixed_dimension_plan(
            false,
            false,
            InteriorSimplexFreeDimensionSelection::Exact(1),
            InteriorSimplexLimits {
                max_selected_boxes: 1,
                ..InteriorSimplexLimits::default()
            },
        )
        .unwrap_err(),
        InteriorSimplexPlanError::ResourceLimit {
            resource: "selected free-dimension boxes",
            requested: 2,
            limit: 1,
        }
    );
    assert_eq!(
        mixed_dimension_plan(
            false,
            false,
            InteriorSimplexFreeDimensionSelection::Exact(1),
            InteriorSimplexLimits {
                max_finite_assignments: 6,
                ..InteriorSimplexLimits::default()
            },
        )
        .unwrap_err(),
        InteriorSimplexPlanError::ResourceLimit {
            resource: "finite coordinate assignments",
            requested: 7,
            limit: 6,
        }
    );
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
            resource: "selected free-dimension boxes",
            requested: 1,
            limit: 0,
        })
    ));

    let finite_partition = UncoveredPartition::new(vec![lattice_box(&[0, 0], &[Some(2), None])], 0);
    let finite_scope =
        || InteriorSimplexScopePartition::new("finite-capped", &sector, &finite_partition);
    assert_eq!(
        try_plan_interior_simplex_samples(
            2,
            [finite_scope()],
            1,
            0,
            InteriorSimplexLimits {
                max_finite_assignments_per_box: 2,
                ..InteriorSimplexLimits::default()
            },
        )
        .unwrap_err(),
        InteriorSimplexPlanError::ResourceLimit {
            resource: "finite assignments per selected box",
            requested: 3,
            limit: 2,
        }
    );
    assert_eq!(
        try_plan_interior_simplex_samples(
            2,
            [finite_scope()],
            1,
            0,
            InteriorSimplexLimits {
                max_finite_assignments: 2,
                ..InteriorSimplexLimits::default()
            },
        )
        .unwrap_err(),
        InteriorSimplexPlanError::ResourceLimit {
            resource: "finite coordinate assignments",
            requested: 3,
            limit: 2,
        }
    );
    assert_eq!(
        try_plan_interior_simplex_samples(
            2,
            [finite_scope()],
            1,
            0,
            InteriorSimplexLimits {
                max_scheduler_workspace_entries: 2,
                ..InteriorSimplexLimits::default()
            },
        )
        .unwrap_err(),
        InteriorSimplexPlanError::ResourceLimit {
            resource: "scheduler workspace entries",
            requested: 3,
            limit: 2,
        }
    );
    assert_eq!(
        try_plan_interior_simplex_samples(
            2,
            [finite_scope()],
            1,
            0,
            InteriorSimplexLimits {
                // One box uses three flatten visits, one offset seed, and
                // three live assignment visits.
                max_scheduler_visits: 6,
                ..InteriorSimplexLimits::default()
            },
        )
        .unwrap_err(),
        InteriorSimplexPlanError::ResourceLimit {
            resource: "scheduler visits",
            requested: 7,
            limit: 6,
        }
    );
    let finite_overflow =
        UncoveredPartition::new(vec![lattice_box(&[0, 0], &[Some(u64::MAX), None])], 0);
    assert_eq!(
        try_plan_interior_simplex_samples(
            2,
            [InteriorSimplexScopePartition::new(
                "finite-overflow",
                &sector,
                &finite_overflow,
            )],
            1,
            0,
            InteriorSimplexLimits::default(),
        )
        .unwrap_err(),
        InteriorSimplexPlanError::ResourceCountOverflow {
            resource: "finite coordinate assignments",
        }
    );
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
