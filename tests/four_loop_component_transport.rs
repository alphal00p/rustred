#![cfg(feature = "legacy-authored-oracles")]

use std::collections::{BTreeMap, BTreeSet};

use rustred::{
    ExactRational, FOUR_LOOP_COMPONENT_TRANSPORT_AFFINE_CONSTANTS,
    FOUR_LOOP_COMPONENT_TRANSPORT_COMPONENT_MAP_ENTRIES, FOUR_LOOP_COMPONENT_TRANSPORT_COMPONENTS,
    FOUR_LOOP_COMPONENT_TRANSPORT_CROSS_COEFFICIENTS,
    FOUR_LOOP_COMPONENT_TRANSPORT_LOCAL_COEFFICIENTS, FOUR_LOOP_COMPONENT_TRANSPORT_LOCAL_SLOTS,
    FOUR_LOOP_COMPONENT_TRANSPORT_LOOP_MAP_ENTRIES, FOUR_LOOP_COMPONENT_TRANSPORT_OCCURRENCES,
    FOUR_LOOP_COMPONENT_TRANSPORT_PARITY_PROJECTIONS, FOUR_LOOP_COMPONENT_TRANSPORT_PLANS,
    FOUR_LOOP_COMPONENT_TRANSPORT_RATIONAL_OPERATIONS,
    FOUR_LOOP_COMPONENT_TRANSPORT_SCALAR_BRANCHES,
    FOUR_LOOP_COMPONENT_TRANSPORT_SIGNED_LINE_REPLAYS,
    FOUR_LOOP_COMPONENT_TRANSPORT_TRANSFORMED_COEFFICIENTS, FOUR_LOOP_NEXT_MANIFEST_SEED_CHECKSUM,
    FourLoopComponentBasisColumn, FourLoopComponentScalarBranchKind, FourLoopComponentTransport,
    FourLoopComponentTransportConfig, FourLoopComponentTransportError,
    FourLoopComponentTransportStatus, FourLoopNextInventory, FourLoopNextInventoryConfig,
    MassiveVacuumMaster,
};

fn assert_preflight_resource(
    config: FourLoopComponentTransportConfig,
    expected_resource: &'static str,
) {
    assert!(matches!(
        FourLoopComponentTransport::preflight_config(config),
        Err(FourLoopComponentTransportError::ResourceLimit { resource, .. })
            if resource == expected_resource
    ));
}

#[test]
fn frozen_component_transport_resources_fail_before_inventory_work() {
    let defaults = FourLoopComponentTransportConfig::default();

    let mut config = defaults;
    config.max_plans = FOUR_LOOP_COMPONENT_TRANSPORT_PLANS - 1;
    assert_preflight_resource(config, "component transport plans");
    let mut config = defaults;
    config.max_occurrences = FOUR_LOOP_COMPONENT_TRANSPORT_OCCURRENCES - 1;
    assert_preflight_resource(config, "component transport occurrences");
    let mut config = defaults;
    config.max_components = FOUR_LOOP_COMPONENT_TRANSPORT_COMPONENTS - 1;
    assert_preflight_resource(config, "component transport components");
    let mut config = defaults;
    config.max_component_map_entries = FOUR_LOOP_COMPONENT_TRANSPORT_COMPONENT_MAP_ENTRIES - 1;
    assert_preflight_resource(config, "component map entries");
    let mut config = defaults;
    config.max_signed_line_replays = FOUR_LOOP_COMPONENT_TRANSPORT_SIGNED_LINE_REPLAYS - 1;
    assert_preflight_resource(config, "signed line replays");
    let mut config = defaults;
    config.max_local_slots = FOUR_LOOP_COMPONENT_TRANSPORT_LOCAL_SLOTS - 1;
    assert_preflight_resource(config, "complete local slots");
    let mut config = defaults;
    config.max_loop_map_entries = FOUR_LOOP_COMPONENT_TRANSPORT_LOOP_MAP_ENTRIES - 1;
    assert_preflight_resource(config, "retained loop-map entries");
    let mut config = defaults;
    config.max_transformed_coefficients =
        FOUR_LOOP_COMPONENT_TRANSPORT_TRANSFORMED_COEFFICIENTS - 1;
    assert_preflight_resource(config, "transformed coefficients");
    let mut config = defaults;
    config.max_affine_constants = FOUR_LOOP_COMPONENT_TRANSPORT_AFFINE_CONSTANTS - 1;
    assert_preflight_resource(config, "affine constants");
    let mut config = defaults;
    config.max_local_coefficients = FOUR_LOOP_COMPONENT_TRANSPORT_LOCAL_COEFFICIENTS - 1;
    assert_preflight_resource(config, "local coefficient inspections");
    let mut config = defaults;
    config.max_cross_coefficients = FOUR_LOOP_COMPONENT_TRANSPORT_CROSS_COEFFICIENTS - 1;
    assert_preflight_resource(config, "cross coefficient inspections");
    let mut config = defaults;
    config.max_parity_projections = FOUR_LOOP_COMPONENT_TRANSPORT_PARITY_PROJECTIONS - 1;
    assert_preflight_resource(config, "rank-one parity projections");
    let mut config = defaults;
    config.max_scalar_branches = FOUR_LOOP_COMPONENT_TRANSPORT_SCALAR_BRANCHES - 1;
    assert_preflight_resource(config, "scalar transport branches");
    let mut config = defaults;
    config.max_rational_operations = FOUR_LOOP_COMPONENT_TRANSPORT_RATIONAL_OPERATIONS - 1;
    assert_preflight_resource(config, "exact rational operations");
}

#[test]
fn exact_component_transport_covers_and_replays_the_full_boundary_census() {
    let inventory = FourLoopNextInventory::build(FourLoopNextInventoryConfig::default()).unwrap();
    let transport =
        FourLoopComponentTransport::build(&inventory, FourLoopComponentTransportConfig::default())
            .unwrap();
    let stats = transport.stats();
    eprintln!("four-loop component transport stats: {stats:#?}");

    assert_eq!(
        transport.status(),
        FourLoopComponentTransportStatus::ExactComponentTransport
    );
    assert_eq!(transport.source_schema(), FourLoopNextInventory::SCHEMA);
    assert_eq!(
        transport.source_seed_checksum(),
        FOUR_LOOP_NEXT_MANIFEST_SEED_CHECKSUM
    );
    assert_eq!(transport.plans().len(), FOUR_LOOP_COMPONENT_TRANSPORT_PLANS);
    assert_eq!(
        transport.occurrences().len(),
        FOUR_LOOP_COMPONENT_TRANSPORT_OCCURRENCES
    );
    assert_eq!(stats.plans(), transport.plans().len());
    assert_eq!(stats.occurrences(), transport.occurrences().len());
    assert_eq!(stats.components(), 2_423);
    assert_eq!(stats.component_map_entries(), 9_592);
    assert_eq!(stats.signed_line_replays(), 5_988);
    assert_eq!(stats.local_slots(), 6_928);
    assert_eq!(stats.loop_map_entries(), 17_056);
    assert_eq!(stats.transformed_coefficients(), 4_890);
    assert_eq!(stats.affine_constants(), 489);
    assert_eq!(stats.local_coefficients(), 3_182);
    assert_eq!(stats.cross_coefficients(), 1_708);
    assert_eq!(stats.parity_projections(), 1_116);
    assert_eq!(stats.scalar_branches(), 2_338);
    assert_eq!(stats.n0_plans(), 577);
    assert_eq!(stats.n1_plans(), 489);
    assert_eq!(
        stats.rational_operations(),
        FOUR_LOOP_COMPONENT_TRANSPORT_RATIONAL_OPERATIONS
    );

    let mut product_census = BTreeMap::<String, usize>::new();
    let mut repeated_component_plan = None;
    let mut b4_plan = None;
    let mut n1_plan = None;
    let mut parity_plan = None;
    let mut t1_four_plan = None;
    let mut t1_four_n1_parity = false;
    let mut b4_inactive_numerator_branch = false;
    let mut independently_counted_branches = 0_usize;
    let mut independently_counted_parity = 0_usize;
    for plan in transport.plans() {
        assert_eq!(plan.basis_columns().len(), 10);
        let mut component_product = BTreeMap::<MassiveVacuumMaster, u32>::new();
        let mut expected_offset = 0_usize;
        for (index, component) in plan.components().iter().enumerate() {
            assert_eq!(component.witness_index(), index);
            assert_eq!(component.reference_loop_offset(), expected_offset);
            expected_offset += component.master().loops();
            *component_product.entry(component.master()).or_default() += 1;
        }
        assert_eq!(expected_offset, 4);
        assert_eq!(&component_product, plan.key().product().factors());
        assert_eq!(
            plan.components()
                .iter()
                .map(|component| component.local_powers().len())
                .sum::<usize>()
                + plan
                    .basis_columns()
                    .iter()
                    .filter(|column| matches!(column, FourLoopComponentBasisColumn::Cross { .. }))
                    .count(),
            10
        );
        let mut local_columns = BTreeSet::new();
        let mut cross_columns = BTreeSet::new();
        for column in plan.basis_columns() {
            match *column {
                FourLoopComponentBasisColumn::Local {
                    component_index,
                    local_position,
                } => {
                    assert!(component_index < plan.components().len());
                    assert!(
                        local_position < plan.components()[component_index].local_powers().len()
                    );
                    assert!(local_columns.insert((component_index, local_position)));
                }
                FourLoopComponentBasisColumn::Cross {
                    left_component,
                    left_axis,
                    right_component,
                    right_axis,
                } => {
                    assert!(left_component < right_component);
                    assert!(right_component < plan.components().len());
                    assert!(left_axis < plan.components()[left_component].master().loops());
                    assert!(right_axis < plan.components()[right_component].master().loops());
                    assert!(cross_columns.insert((
                        left_component,
                        left_axis,
                        right_component,
                        right_axis,
                    )));
                }
            }
        }
        assert_eq!(
            local_columns.len(),
            plan.components()
                .iter()
                .map(|component| component.local_powers().len())
                .sum::<usize>()
        );
        independently_counted_branches += plan.scalar_branches().len();
        independently_counted_parity += plan.parity_witnesses().len();

        let negative_positions = plan
            .key()
            .powers()
            .iter()
            .enumerate()
            .filter_map(|(position, &power)| (power < 0).then_some((position, power)))
            .collect::<Vec<_>>();
        match (negative_positions.as_slice(), plan.affine_image()) {
            ([], None) => {
                assert_eq!(plan.scalar_branches().len(), 1);
                let branch = &plan.scalar_branches()[0];
                assert_eq!(branch.kind(), FourLoopComponentScalarBranchKind::Base);
                assert!(!branch.coefficient().is_zero());
                assert!(branch.lowered_component_powers().is_none());
                assert!(plan.parity_witnesses().is_empty());
            }
            ([(source_position, -1)], Some(image)) => {
                assert_eq!(image.source_position(), *source_position);
                assert!(
                    plan.scalar_branches()
                        .iter()
                        .all(|branch| branch.kind() != FourLoopComponentScalarBranchKind::Base)
                );
                let constants = plan
                    .scalar_branches()
                    .iter()
                    .filter(|branch| branch.kind() == FourLoopComponentScalarBranchKind::Constant)
                    .collect::<Vec<_>>();
                assert_eq!(constants.len(), usize::from(!image.constant().is_zero()));
                if let Some(branch) = constants.first() {
                    assert_eq!(branch.coefficient(), image.constant());
                    assert!(branch.lowered_component_powers().is_none());
                }
                for (basis_position, (&coefficient, column)) in image
                    .coefficients()
                    .iter()
                    .zip(plan.basis_columns())
                    .enumerate()
                {
                    match *column {
                        FourLoopComponentBasisColumn::Local {
                            component_index,
                            local_position,
                        } if !coefficient.is_zero() => {
                            let matching = plan
                                .scalar_branches()
                                .iter()
                                .filter(|branch| {
                                    branch.kind()
                                        == FourLoopComponentScalarBranchKind::Local {
                                            component_index,
                                            local_position,
                                        }
                                })
                                .collect::<Vec<_>>();
                            assert_eq!(matching.len(), 1);
                            let lowered = matching[0].lowered_component_powers().unwrap();
                            let base = plan.components()[component_index].local_powers();
                            assert_eq!(lowered.len(), base.len());
                            for position in 0..base.len() {
                                assert_eq!(
                                    lowered[position],
                                    base[position] - i32::from(position == local_position)
                                );
                            }
                            if plan.components()[component_index].master()
                                == MassiveVacuumMaster::B4
                                && matches!(local_position, 2 | 4)
                            {
                                b4_inactive_numerator_branch = true;
                            }
                        }
                        FourLoopComponentBasisColumn::Cross {
                            left_component,
                            left_axis,
                            right_component,
                            right_axis,
                        } if !coefficient.is_zero() => {
                            let matching = plan
                                .parity_witnesses()
                                .iter()
                                .filter(|parity| {
                                    parity.basis_position() == basis_position
                                        && parity.coefficient() == coefficient
                                        && parity.left_component() == left_component
                                        && parity.left_axis() == left_axis
                                        && parity.right_component() == right_component
                                        && parity.right_axis() == right_axis
                                })
                                .collect::<Vec<_>>();
                            assert_eq!(matching.len(), 1);
                            assert!(matching[0].left_rank_one_zero());
                            assert!(matching[0].right_rank_one_zero());
                        }
                        FourLoopComponentBasisColumn::Local { .. }
                        | FourLoopComponentBasisColumn::Cross { .. } => {}
                    }
                }
            }
            _ => panic!("transport plan is outside the exact N0/N1 domain"),
        }
        *product_census
            .entry(plan.key().product().to_string())
            .or_default() += 1;

        let mut master_counts = BTreeMap::new();
        for component in plan.components() {
            *master_counts.entry(component.master()).or_insert(0_usize) += 1;
        }
        if master_counts.values().any(|&count| count > 1) {
            repeated_component_plan.get_or_insert(plan);
        }
        if plan
            .components()
            .iter()
            .any(|component| component.master() == MassiveVacuumMaster::B4)
        {
            b4_plan.get_or_insert(plan);
        }
        if plan.affine_image().is_some() {
            n1_plan.get_or_insert(plan);
        }
        if !plan.parity_witnesses().is_empty() {
            parity_plan.get_or_insert(plan);
        }
        if plan.key().product().multiplicity(&MassiveVacuumMaster::T1) == 4 {
            t1_four_plan.get_or_insert(plan);
            if plan.affine_image().is_some() && !plan.parity_witnesses().is_empty() {
                t1_four_n1_parity = true;
            }
        }
    }
    eprintln!("four-loop component product census: {product_census:#?}");
    assert_eq!(
        product_census,
        BTreeMap::from([
            ("S2^2".to_owned(), 52),
            ("T1*B4".to_owned(), 223),
            ("T1*F5".to_owned(), 494),
            ("T1*M6".to_owned(), 106),
            ("T1^2*S2".to_owned(), 91),
            ("T1^4".to_owned(), 100),
        ])
    );
    assert!(repeated_component_plan.is_some());
    assert!(b4_plan.is_some());
    assert!(n1_plan.is_some());
    assert!(parity_plan.is_some());
    assert!(b4_inactive_numerator_branch);
    assert_eq!(independently_counted_branches, stats.scalar_branches());
    assert_eq!(independently_counted_parity * 2, stats.parity_projections());

    let repeated = repeated_component_plan.unwrap();
    for (index, component) in repeated.components().iter().enumerate() {
        assert_eq!(component.witness_index(), index);
    }
    let t1_four = t1_four_plan.unwrap();
    assert_eq!(t1_four.components().len(), 4);
    assert_eq!(
        t1_four
            .components()
            .iter()
            .map(|component| component.reference_loop_offset())
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    assert!(
        t1_four
            .components()
            .iter()
            .all(|component| component.master() == MassiveVacuumMaster::T1)
    );
    assert_eq!(
        t1_four
            .basis_columns()
            .iter()
            .filter(|column| matches!(column, FourLoopComponentBasisColumn::Cross { .. }))
            .count(),
        6
    );
    assert!(t1_four_n1_parity);
    let b4 = b4_plan.unwrap();
    let b4_component = b4
        .components()
        .iter()
        .find(|component| component.master() == MassiveVacuumMaster::B4)
        .unwrap();
    assert_eq!(b4_component.local_powers().len(), 6);
    assert_eq!(b4_component.local_powers()[2], 0);
    assert_eq!(b4_component.local_powers()[4], 0);
    for assignment in b4_component.line_assignments() {
        assert_eq!(
            assignment.local_position(),
            [0, 1, 3, 5][assignment.compact_reference_position()]
        );
    }
    let b4_index = b4_component.witness_index();
    let b4_assignment = b4_component
        .line_assignments()
        .iter()
        .position(|assignment| assignment.compact_reference_position() >= 2)
        .unwrap();
    let original_local = b4_component.line_assignments()[b4_assignment].local_position();
    let tampered_b4_lift =
        b4.with_line_local_position_for_replay(b4_index, b4_assignment, (original_local + 1) % 6);
    assert!(transport.replay_plan_candidate(&tampered_b4_lift).is_err());
    for occurrence in transport.occurrences() {
        let plan = &transport.plans()[occurrence.plan_index() as usize];
        assert_eq!(plan.leaf_id(), occurrence.leaf_id());
        assert_eq!(
            inventory.rows()[usize::from(occurrence.row_index())].paths()
                [occurrence.path_index() as usize]
                .leaf_id(),
            occurrence.leaf_id()
        );
    }

    transport.replay().unwrap();

    let first = &transport.plans()[0];
    let tampered_transform = first.with_loop_transform_entry_for_replay(
        0,
        0,
        first.loop_transform()[0][0] + ExactRational::ONE,
    );
    assert!(
        transport
            .replay_plan_candidate(&tampered_transform)
            .is_err()
    );

    let n1 = n1_plan.unwrap();
    let tampered_affine = n1.with_affine_coefficient_for_replay(
        0,
        n1.affine_image().unwrap().coefficients()[0] + ExactRational::ONE,
    );
    assert!(transport.replay_plan_candidate(&tampered_affine).is_err());
    let affine = n1.affine_image().unwrap();
    let tampered_constant = n1
        .with_affine_constant_for_replay(affine.constant() + n1.scalar_branches()[0].coefficient());
    assert!(transport.replay_plan_candidate(&tampered_constant).is_err());

    let component = &first.components()[0];
    let tampered_power =
        first.with_component_local_power_for_replay(0, 0, component.local_powers()[0] + 1);
    assert!(transport.replay_plan_candidate(&tampered_power).is_err());

    let replacement = first.basis_columns()[1];
    let tampered_basis = first.with_basis_column_for_replay(0, replacement);
    assert!(transport.replay_plan_candidate(&tampered_basis).is_err());

    let parity = parity_plan.unwrap();
    let tampered_parity = parity.with_parity_flag_for_replay(0, false, true);
    assert!(transport.replay_plan_candidate(&tampered_parity).is_err());

    assert!(transport.plans().iter().all(|plan| {
        plan.scalar_branches()
            .iter()
            .all(|branch| match branch.kind() {
                FourLoopComponentScalarBranchKind::Base => plan.affine_image().is_none(),
                FourLoopComponentScalarBranchKind::Constant
                | FourLoopComponentScalarBranchKind::Local { .. } => plan.affine_image().is_some(),
            })
    }));
}
