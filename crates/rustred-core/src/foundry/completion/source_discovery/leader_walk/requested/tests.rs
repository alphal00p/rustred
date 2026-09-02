use crate::foundry::completion::{LatticeBox, LatticePoint, SectorChart, UncoveredPartition};
use crate::sector::Mask;

use super::super::LeaderWalkLimits;
use super::{RequestedDomain, RequestedDomainScopePartition, try_plan_requested_domains};

fn point(coordinates: &[u64]) -> LatticePoint {
    LatticePoint::try_new(coordinates.iter().copied()).unwrap()
}

fn request(coordinates: &[u64], symbolic_axes: &[usize]) -> RequestedDomain {
    RequestedDomain::new(point(coordinates), symbolic_axes.iter().copied())
}

fn lattice_box(lower: &[u64], upper: &[Option<u64>]) -> LatticeBox {
    LatticeBox::try_new(lower.iter().copied(), upper.iter().copied()).unwrap()
}

#[test]
fn requested_domains_attach_all_residuals_skip_only_full_coverage_and_interleave_scopes() {
    let sector = Mask::try_new([true, false]).unwrap();
    let first_partition = UncoveredPartition::new(
        vec![
            lattice_box(&[0, 0], &[Some(1), None]),
            lattice_box(&[2, 0], &[None, None]),
        ],
        0,
    );
    let second_partition = UncoveredPartition::new(vec![lattice_box(&[0, 0], &[None, Some(1)])], 0);
    let first_requests = [
        request(&[0, 3], &[0, 1]),
        request(&[4, 2], &[0]),
        request(&[9, 1], &[1]),
    ];
    let second_requests = [request(&[2, 1], &[0, 1]), request(&[2, 4], &[0, 1])];
    let plan = try_plan_requested_domains(
        17,
        [
            RequestedDomainScopePartition::new(
                "a-first",
                &sector,
                &first_partition,
                &first_requests,
            ),
            RequestedDomainScopePartition::new(
                "b-second",
                &sector,
                &second_partition,
                &second_requests,
            ),
        ],
        LeaderWalkLimits::default(),
    )
    .unwrap();

    assert_eq!(plan.epoch_ordinal(), 17);
    assert_eq!(plan.input_scope_count(), 2);
    assert_eq!(plan.requested_domain_count(), 5);
    assert_eq!(plan.fully_covered_domain_count(), 1);
    assert_eq!(
        plan.tasks()
            .iter()
            .map(|task| (
                task.key().stable_scope_key(),
                task.key().requested_ordinal(),
                task.leader().to_vec(),
                task.target_shift().values().to_vec(),
            ))
            .collect::<Vec<_>>(),
        vec![
            ("b-second", 0, vec![2, 1], vec![2, -1]),
            ("a-first", 0, vec![0, 3], vec![0, -3]),
            // Both residuals retry the same requested recurrence family.
            // The second residual leader is geometry, not a new translation.
            ("a-first", 0, vec![2, 3], vec![0, -3]),
            ("a-first", 1, vec![4, 2], vec![4, -2]),
            ("a-first", 2, vec![9, 1], vec![9, -1]),
        ]
    );
    assert_eq!(plan.tasks()[0].key().box_lower(), [0, 0]);
    assert_eq!(plan.tasks()[3].key().box_lower(), [2, 0]);
    assert_eq!(plan.tasks()[3].key().symbolic_axes(), [0]);
    assert_eq!(plan.tasks()[3].key().fixed_indices(), [1, 0]);
}

#[test]
fn duplicate_identity_includes_the_requested_face() {
    let sector = Mask::try_new([true, true]).unwrap();
    let partition = UncoveredPartition::new(vec![lattice_box(&[0, 0], &[None, None])], 0);
    let same_point_distinct_faces = [request(&[1, 0], &[0]), request(&[1, 0], &[1])];
    let plan = try_plan_requested_domains(
        0,
        [RequestedDomainScopePartition::new(
            "scope",
            &sector,
            &partition,
            &same_point_distinct_faces,
        )],
        LeaderWalkLimits::default(),
    )
    .unwrap();
    assert_eq!(plan.tasks().len(), 2);
    assert_eq!(plan.tasks()[0].leader(), plan.tasks()[1].leader());
    assert_ne!(
        plan.tasks()[0].key().symbolic_axes(),
        plan.tasks()[1].key().symbolic_axes()
    );

    let exact_duplicates = [request(&[1, 0], &[0]), request(&[1, 0], &[0])];
    let error = try_plan_requested_domains(
        0,
        [RequestedDomainScopePartition::new(
            "scope",
            &sector,
            &partition,
            &exact_duplicates,
        )],
        LeaderWalkLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        super::super::LeaderWalkPlanError::DuplicateRequestedDomain {
            first_request_ordinal: 0,
            duplicate_request_ordinal: 1,
            ..
        }
    ));
}

#[test]
fn fixed_base_plus_shift_reconstructs_nontrivial_pivot_literal() {
    let sector = Mask::try_new([true, true]).unwrap();
    let partition = UncoveredPartition::new(vec![lattice_box(&[0, 0], &[None, None])], 0);
    // Chart [0,1] is the exact integral [1,2]. Only axis zero remains
    // symbolic; the second pivot is therefore the nontrivial fixed literal 2.
    let requests = [request(&[0, 1], &[0])];
    let plan = try_plan_requested_domains(
        0,
        [RequestedDomainScopePartition::new(
            "scope", &sector, &partition, &requests,
        )],
        LeaderWalkLimits::default(),
    )
    .unwrap();
    let task = &plan.tasks()[0];
    assert_eq!(task.key().fixed_indices(), [1, 1]);
    assert_eq!(task.target_shift().values(), [0, 1]);
    let base_chart = task.base_probe_chart_origin().collect::<Vec<_>>();
    assert_eq!(base_chart, [1, 0]);
    let base = SectorChart::new(sector)
        .to_integral(&point(&base_chart))
        .unwrap();
    assert_eq!(
        base.powers()
            .iter()
            .zip(task.target_shift().values())
            .map(|(base, shift)| base + shift)
            .collect::<Vec<_>>(),
        [2, 2]
    );
    assert_eq!(base.powers()[1], task.key().fixed_indices()[1]);
}

#[test]
fn covered_minimal_point_does_not_hide_an_uncovered_requested_domain_tail() {
    let sector = Mask::try_new([true, true]).unwrap();
    // [0,0] is covered, but the requested x-ray has an uncovered residual
    // starting at [2,0].
    let partition = UncoveredPartition::new(vec![lattice_box(&[2, 0], &[None, None])], 0);
    let requests = [request(&[0, 0], &[0])];
    let plan = try_plan_requested_domains(
        0,
        [RequestedDomainScopePartition::new(
            "scope", &sector, &partition, &requests,
        )],
        LeaderWalkLimits::default(),
    )
    .unwrap();
    assert_eq!(plan.fully_covered_domain_count(), 0);
    assert_eq!(plan.tasks().len(), 1);
    let task = &plan.tasks()[0];
    assert_eq!(task.key().requested_domain_lower(), [0, 0]);
    assert_eq!(task.key().requested_domain_upper(), [None, Some(0)]);
    assert_eq!(task.leader(), [2, 0]);
    assert_eq!(task.target_shift().values(), [0, 0]);
    assert_eq!(task.key().residual_domain_upper(), [None, Some(0)]);
}

#[test]
fn finite_carrier_tail_never_becomes_a_machine_endpoint_translation() {
    let sector = Mask::try_new([true]).unwrap();
    let carrier_upper = i64::MAX as u64 - 1;
    let partition = UncoveredPartition::new(
        vec![lattice_box(&[carrier_upper - 1], &[Some(carrier_upper)])],
        0,
    );
    let requests = [request(&[1], &[0])];
    let plan = try_plan_requested_domains(
        3,
        [RequestedDomainScopePartition::new(
            "finite-tail",
            &sector,
            &partition,
            &requests,
        )],
        LeaderWalkLimits::default(),
    )
    .unwrap();
    assert_eq!(plan.tasks().len(), 1);
    assert_eq!(plan.tasks()[0].leader(), [carrier_upper - 1]);
    assert_eq!(plan.tasks()[0].target_shift().values(), [1]);
}

#[test]
fn base_probe_offsets_respect_singleton_finite_and_unbounded_residual_extent() {
    let sector = Mask::try_new([true]).unwrap();
    let partition = UncoveredPartition::new(
        vec![
            lattice_box(&[0], &[Some(0)]),
            lattice_box(&[1], &[Some(2)]),
            lattice_box(&[3], &[None]),
        ],
        0,
    );
    let requests = [request(&[0], &[0])];
    let plan = try_plan_requested_domains(
        0,
        [RequestedDomainScopePartition::new(
            "scope", &sector, &partition, &requests,
        )],
        LeaderWalkLimits::default(),
    )
    .unwrap();
    assert_eq!(plan.tasks().len(), 3);
    assert_eq!(plan.tasks()[0].leader(), [0]);
    assert_eq!(
        plan.tasks()[0]
            .base_probe_chart_origin()
            .collect::<Vec<_>>(),
        [0]
    );
    assert_eq!(plan.tasks()[1].leader(), [1]);
    assert_eq!(
        plan.tasks()[1]
            .base_probe_chart_origin()
            .collect::<Vec<_>>(),
        [1]
    );
    assert_eq!(plan.tasks()[2].leader(), [3]);
    assert_eq!(
        plan.tasks()[2]
            .base_probe_chart_origin()
            .collect::<Vec<_>>(),
        [1]
    );
}

#[test]
fn rebuilt_equal_geometry_rejects_old_requested_tasks() {
    let sector = Mask::try_new([true]).unwrap();
    let partition = UncoveredPartition::new(vec![lattice_box(&[0], &[None])], 0);
    let requests = [request(&[2], &[0])];
    let build = || {
        try_plan_requested_domains(
            9,
            [RequestedDomainScopePartition::new(
                "scope", &sector, &partition, &requests,
            )],
            LeaderWalkLimits::default(),
        )
        .unwrap()
    };
    let first = build();
    let rebuilt = build();
    assert!(first.validate_task(&first.tasks()[0]).is_ok());
    assert!(rebuilt.validate_task(&first.tasks()[0]).is_err());
}
