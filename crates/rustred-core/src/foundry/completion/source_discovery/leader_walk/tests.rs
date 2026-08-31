use crate::foundry::completion::{LatticeBox, UncoveredPartition};
use crate::sector::Mask;

use super::{
    LeaderWalkDepth, LeaderWalkLimits, LeaderWalkPlan, LeaderWalkPlanError,
    LeaderWalkScopePartition, LeaderWalkTaskKey, try_plan_maximal_orthant_leader_walk,
};

fn lattice_box(lower: &[u64], upper: &[Option<u64>]) -> LatticeBox {
    LatticeBox::try_new(lower.iter().copied(), upper.iter().copied()).unwrap()
}

fn all_active(arity: usize) -> Mask {
    Mask::try_new(std::iter::repeat_n(true, arity)).unwrap()
}

fn path_star_partitions(reverse_star: bool) -> (Mask, UncoveredPartition, UncoveredPartition) {
    let sector = all_active(6);
    let path = UncoveredPartition::new(
        vec![lattice_box(
            &[0, 0, 0, 0, 0, 0],
            &[Some(0), None, None, None, None, None],
        )],
        0,
    );
    let mut star_boxes = vec![
        lattice_box(
            &[0, 0, 0, 0, 0, 0],
            &[Some(0), None, None, None, None, None],
        ),
        lattice_box(
            &[1, 0, 0, 0, 0, 0],
            &[Some(1), None, None, None, None, None],
        ),
        lattice_box(
            &[2, 0, 0, 0, 0, 0],
            &[Some(2), None, None, None, None, None],
        ),
    ];
    if reverse_star {
        star_boxes.reverse();
    }
    let star = UncoveredPartition::new(star_boxes, 0);
    (sector, path, star)
}

fn plan_path_star(reverse_inputs: bool, reverse_star: bool, epoch: u64) -> LeaderWalkPlan {
    let (sector, path, star) = path_star_partitions(reverse_star);
    // Deliberately oppose stable-key order. Canonical chronology is sector and
    // complete endpoint tuple, so the one-box path scope still precedes star.
    let path_scope = LeaderWalkScopePartition::new("z-path-scope", &sector, &path);
    let star_scope = LeaderWalkScopePartition::new("a-star-scope", &sector, &star);
    let scopes = if reverse_inputs {
        vec![star_scope, path_scope]
    } else {
        vec![path_scope, star_scope]
    };
    try_plan_maximal_orthant_leader_walk(epoch, scopes, LeaderWalkLimits::default()).unwrap()
}

#[derive(Debug, PartialEq, Eq)]
struct SemanticTask {
    depth: LeaderWalkDepth,
    canonical_ordinal: usize,
    key: LeaderWalkTaskKey,
    leader: Vec<u64>,
    shift: Vec<i64>,
}

fn semantic_tasks(plan: &LeaderWalkPlan) -> Vec<SemanticTask> {
    plan.waves()
        .into_iter()
        .flat_map(|wave| {
            wave.tasks().iter().map(|task| SemanticTask {
                depth: wave.depth(),
                canonical_ordinal: task.canonical_ordinal(),
                key: task.key().clone(),
                leader: task.leader().to_vec(),
                shift: task.target_shift().values().to_vec(),
            })
        })
        .collect()
}

#[test]
fn maximal_box_census_is_complete_and_scope_fair_for_one_plus_three() {
    let plan = plan_path_star(false, false, 7);
    assert_eq!(plan.epoch_ordinal(), 7);
    assert_eq!(plan.input_scope_count(), 2);
    assert_eq!(plan.selected_scope_count(), 2);
    assert_eq!(plan.selected_box_count(), 4);
    assert_eq!(plan.planned_task_count(), 24);
    assert_eq!(plan.maximal_free_dimension(), 5);

    let waves = plan.waves();
    assert_eq!(waves[0].depth(), LeaderWalkDepth::LowerCorner);
    assert_eq!(waves[1].depth(), LeaderWalkDepth::DepthOne);
    assert_eq!(waves[0].tasks().len(), 4);
    assert_eq!(waves[1].tasks().len(), 20);
    for wave in waves {
        // Round zero visits every represented scope before the second and
        // third boxes of the larger scope. Stable-key order is deliberately
        // the reverse of this sector/endpoint chronology.
        assert_eq!(wave.tasks()[0].key().stable_scope_key(), "z-path-scope");
        assert_eq!(wave.tasks()[1].key().stable_scope_key(), "a-star-scope");
        assert_eq!(
            wave.tasks()
                .iter()
                .filter(|task| task.key().stable_scope_key() == "z-path-scope")
                .count(),
            if wave.depth() == LeaderWalkDepth::LowerCorner {
                1
            } else {
                5
            }
        );
        assert_eq!(
            wave.tasks()
                .iter()
                .filter(|task| task.key().stable_scope_key() == "a-star-scope")
                .count(),
            if wave.depth() == LeaderWalkDepth::LowerCorner {
                3
            } else {
                15
            }
        );
    }
    assert!(
        waves[0]
            .tasks()
            .iter()
            .all(|task| task.key().depth_one_axis().is_none())
    );
    for task in waves[1].tasks() {
        let axis = task.key().depth_one_axis().unwrap();
        assert!(task.key().box_upper()[axis].is_none());
        assert_eq!(
            task.leader()
                .iter()
                .zip(task.key().box_lower())
                .enumerate()
                .filter(|entry| (entry.1).0 != (entry.1).1)
                .map(|(position, _)| position)
                .collect::<Vec<_>>(),
            vec![axis]
        );
        assert_eq!(task.leader()[axis], task.key().box_lower()[axis] + 1);
    }
}

#[test]
fn canonical_output_ignores_input_order_box_order_and_worker_completion_order() {
    let forward = plan_path_star(false, false, 11);
    let reversed = plan_path_star(true, true, 12);
    assert_eq!(semantic_tasks(&forward), semantic_tasks(&reversed));

    for wave in forward.waves() {
        let expected: Vec<_> = wave
            .tasks()
            .iter()
            .map(|task| (task.canonical_ordinal(), task.key().clone()))
            .collect();
        for worker_count in 1..=4 {
            let mut simulated_completion = Vec::new();
            for worker in (0..worker_count).rev() {
                simulated_completion.extend(
                    wave.tasks()
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
}

#[test]
fn mixed_sector_chart_conversion_produces_checked_corner_relative_shifts() {
    let sector = Mask::try_new([true, false, true]).unwrap();
    let partition =
        UncoveredPartition::new(vec![lattice_box(&[2, 3, 0], &[None, None, Some(0)])], 0);
    let plan = try_plan_maximal_orthant_leader_walk(
        1,
        [LeaderWalkScopePartition::new(
            "mixed-sector",
            &sector,
            &partition,
        )],
        LeaderWalkLimits::default(),
    )
    .unwrap();
    let waves = plan.waves();
    assert_eq!(waves[0].tasks()[0].leader(), &[2, 3, 0]);
    assert_eq!(waves[0].tasks()[0].target_shift().values(), &[2, -3, 0]);
    assert_eq!(waves[1].tasks().len(), 2);
    assert_eq!(waves[1].tasks()[0].key().depth_one_axis(), Some(0));
    assert_eq!(waves[1].tasks()[0].leader(), &[3, 3, 0]);
    assert_eq!(waves[1].tasks()[0].target_shift().values(), &[3, -3, 0]);
    assert_eq!(waves[1].tasks()[1].key().depth_one_axis(), Some(1));
    assert_eq!(waves[1].tasks()[1].leader(), &[2, 4, 0]);
    assert_eq!(waves[1].tasks()[1].target_shift().values(), &[2, -4, 0]);
}

#[test]
fn depth_one_overflow_and_global_task_caps_reject_the_whole_plan() {
    let sector = Mask::try_new([false]).unwrap();
    let overflowing = UncoveredPartition::new(vec![lattice_box(&[u64::MAX], &[None])], 0);
    assert!(matches!(
        try_plan_maximal_orthant_leader_walk(
            1,
            [LeaderWalkScopePartition::new(
                "overflow",
                &sector,
                &overflowing,
            )],
            LeaderWalkLimits::default(),
        ),
        Err(LeaderWalkPlanError::LeaderCoordinateOverflow {
            canonical_scope_ordinal: 0,
            box_ordinal: 0,
            position: 0,
        })
    ));

    let (_, path, _) = path_star_partitions(false);
    let active = all_active(6);
    let limits = LeaderWalkLimits {
        max_tasks: 1,
        ..LeaderWalkLimits::default()
    };
    assert!(matches!(
        try_plan_maximal_orthant_leader_walk(
            2,
            [LeaderWalkScopePartition::new("capped", &active, &path)],
            limits,
        ),
        Err(LeaderWalkPlanError::ResourceLimit {
            resource: "tasks across both waves",
            requested: 6,
            limit: 1,
        })
    ));

    let free_axis_limits = LeaderWalkLimits {
        max_selected_free_axis_cells: 4,
        ..LeaderWalkLimits::default()
    };
    assert!(matches!(
        try_plan_maximal_orthant_leader_walk(
            3,
            [LeaderWalkScopePartition::new(
                "free-axis-capped",
                &active,
                &path,
            )],
            free_axis_limits,
        ),
        Err(LeaderWalkPlanError::ResourceLimit {
            resource: "selected maximal-box free-axis cells",
            requested: 5,
            limit: 4,
        })
    ));

    // Six retained tasks (corner plus five individual depth-one axes), with a
    // six-coordinate leader and shift for each, require exactly 72 cells.
    let coordinate_limits = LeaderWalkLimits {
        max_task_coordinate_cells: 71,
        ..LeaderWalkLimits::default()
    };
    assert!(matches!(
        try_plan_maximal_orthant_leader_walk(
            4,
            [LeaderWalkScopePartition::new(
                "coordinate-capped",
                &active,
                &path,
            )],
            coordinate_limits,
        ),
        Err(LeaderWalkPlanError::ResourceLimit {
            resource: "leader-walk task coordinate cells",
            requested: 72,
            limit: 71,
        })
    ));
}

#[test]
fn representability_is_checked_by_the_sector_chart() {
    let sector = Mask::try_new([true]).unwrap();
    let partition = UncoveredPartition::new(vec![lattice_box(&[i64::MAX as u64], &[None])], 0);
    assert!(matches!(
        try_plan_maximal_orthant_leader_walk(
            3,
            [LeaderWalkScopePartition::new(
                "outside-carrier",
                &sector,
                &partition,
            )],
            LeaderWalkLimits::default(),
        ),
        Err(LeaderWalkPlanError::Geometry(_))
    ));
}

#[test]
fn equal_rebuilt_geometry_is_a_stale_epoch_and_census_is_only_structural() {
    let old = plan_path_star(false, false, 41);
    let rebuilt = plan_path_star(false, false, 41);
    let old_task = &old.waves()[0].tasks()[0];
    assert!(old.validate_task(old_task).is_ok());
    assert_eq!(old_task.epoch_ordinal(), 41);
    assert!(matches!(
        rebuilt.validate_task(old_task),
        Err(LeaderWalkPlanError::StaleGeometryEpoch {
            expected_ordinal: 41,
            actual_ordinal: 41,
        })
    ));

    let census = old.planning_envelope_census();
    assert_eq!(census.epoch_ordinal(), 41);
    assert_eq!(census.selected_scope_count(), 2);
    assert_eq!(census.selected_box_count(), 4);
    assert_eq!(census.planned_task_count(), 24);
    assert!(census.belongs_to(&old));
    assert!(!census.belongs_to(&rebuilt));
}

#[test]
fn finite_only_partitions_never_become_leader_walk_terminals() {
    let sector = all_active(2);
    let finite = UncoveredPartition::new(vec![lattice_box(&[0, 0], &[Some(2), Some(3)])], 0);
    assert_eq!(
        try_plan_maximal_orthant_leader_walk(
            5,
            [LeaderWalkScopePartition::new("finite", &sector, &finite)],
            LeaderWalkLimits::default(),
        )
        .unwrap_err(),
        LeaderWalkPlanError::NoUnboundedGeometry
    );
}
