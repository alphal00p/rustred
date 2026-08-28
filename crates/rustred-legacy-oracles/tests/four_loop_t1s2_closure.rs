use std::collections::{BTreeMap, BTreeSet};

use rustred::{CoefficientContext, MasterProduct, ProductLinearCombination};
use rustred_legacy_oracles::Integral;
use rustred_legacy_oracles::{
    FOUR_LOOP_T1S2_CLOSURE_COEFFICIENT_DEGREE, FOUR_LOOP_T1S2_CLOSURE_COEFFICIENT_OPERATIONS,
    FOUR_LOOP_T1S2_CLOSURE_COLLECTED_TERMS, FOUR_LOOP_T1S2_CLOSURE_COMPONENT_CALLS,
    FOUR_LOOP_T1S2_CLOSURE_COMPONENTS, FOUR_LOOP_T1S2_CLOSURE_CONVOLUTION_PAIRS,
    FOUR_LOOP_T1S2_CLOSURE_LOCAL_SLOTS, FOUR_LOOP_T1S2_CLOSURE_MASS_POWER_STEPS,
    FOUR_LOOP_T1S2_CLOSURE_OCCURRENCES, FOUR_LOOP_T1S2_CLOSURE_OPEN_PLANS,
    FOUR_LOOP_T1S2_CLOSURE_PLANS, FOUR_LOOP_T1S2_CLOSURE_PRECOLLECTION_TERMS,
    FOUR_LOOP_T1S2_CLOSURE_RETAINED_COEFFICIENT_BYTES, FOUR_LOOP_T1S2_CLOSURE_SCALAR_BRANCHES,
    FOUR_LOOP_T1S2_CLOSURE_UNIQUE_TARGETS, FourLoopComponentScalarBranchKind,
    FourLoopComponentTransport, FourLoopComponentTransportConfig, FourLoopNextInventory,
    FourLoopNextInventoryConfig, FourLoopT1S2Closure, FourLoopT1S2ClosureConfig,
    FourLoopT1S2ClosureError, FourLoopT1S2ClosureStatus, FourLoopT1S2ParentStatus,
    FourLoopT1S2ProductClass, MassiveVacuumMaster, OneLoopTadpoleReducer, TwoLoopTopDotConfig,
    TwoLoopTopDotReducer, equal_mass_two_loop_vacuum_in_context,
};

fn assert_preflight_resource(config: FourLoopT1S2ClosureConfig, expected_resource: &'static str) {
    assert!(matches!(
        FourLoopT1S2Closure::preflight_config(config),
        Err(FourLoopT1S2ClosureError::ResourceLimit { resource, .. })
            if resource == expected_resource
    ));
}

fn product_loop_weight(product: &MasterProduct<MassiveVacuumMaster>) -> usize {
    product
        .factors()
        .iter()
        .map(|(master, multiplicity)| master.loops() * (*multiplicity as usize))
        .sum()
}

fn allowed_product(product: &MasterProduct<MassiveVacuumMaster>) -> bool {
    let t1 = product.multiplicity(&MassiveVacuumMaster::T1);
    let s2 = product.multiplicity(&MassiveVacuumMaster::S2);
    product
        .factors()
        .keys()
        .all(|master| matches!(master, MassiveVacuumMaster::T1 | MassiveVacuumMaster::S2))
        && matches!((t1, s2), (4, 0) | (2, 1) | (0, 2))
}

#[test]
fn frozen_t1s2_closure_resources_fail_before_inventory_work() {
    let defaults = FourLoopT1S2ClosureConfig::default();

    let mut config = defaults;
    config.max_plans = FOUR_LOOP_T1S2_CLOSURE_PLANS - 1;
    assert_preflight_resource(config, "T1/S2 completed plans");
    let mut config = defaults;
    config.max_open_plans = FOUR_LOOP_T1S2_CLOSURE_OPEN_PLANS - 1;
    assert_preflight_resource(config, "T1/S2 open plans");
    let mut config = defaults;
    config.max_occurrences = FOUR_LOOP_T1S2_CLOSURE_OCCURRENCES - 1;
    assert_preflight_resource(config, "T1/S2 occurrence partition");
    let mut config = defaults;
    config.max_components = FOUR_LOOP_T1S2_CLOSURE_COMPONENTS - 1;
    assert_preflight_resource(config, "T1/S2 components");
    let mut config = defaults;
    config.max_local_slots = FOUR_LOOP_T1S2_CLOSURE_LOCAL_SLOTS - 1;
    assert_preflight_resource(config, "T1/S2 local slots");
    let mut config = defaults;
    config.max_scalar_branches = FOUR_LOOP_T1S2_CLOSURE_SCALAR_BRANCHES - 1;
    assert_preflight_resource(config, "T1/S2 scalar branches");
    let mut config = defaults;
    config.max_component_calls = FOUR_LOOP_T1S2_CLOSURE_COMPONENT_CALLS - 1;
    assert_preflight_resource(config, "T1/S2 component calls");
    let mut config = defaults;
    config.max_unique_targets = FOUR_LOOP_T1S2_CLOSURE_UNIQUE_TARGETS - 1;
    assert_preflight_resource(config, "T1/S2 unique targets");
    let mut config = defaults;
    config.max_convolution_pair_operations = FOUR_LOOP_T1S2_CLOSURE_CONVOLUTION_PAIRS - 1;
    assert_preflight_resource(config, "T1/S2 convolution pairs");
    let mut config = defaults;
    config.max_precollection_terms = FOUR_LOOP_T1S2_CLOSURE_PRECOLLECTION_TERMS - 1;
    assert_preflight_resource(config, "T1/S2 precollection terms");
    let mut config = defaults;
    config.max_collected_terms = FOUR_LOOP_T1S2_CLOSURE_COLLECTED_TERMS - 1;
    assert_preflight_resource(config, "T1/S2 collected terms");
    let mut config = defaults;
    config.max_mass_power_steps = FOUR_LOOP_T1S2_CLOSURE_MASS_POWER_STEPS - 1;
    assert_preflight_resource(config, "T1/S2 mass-power steps");
    let mut config = defaults;
    config.max_coefficient_operations = FOUR_LOOP_T1S2_CLOSURE_COEFFICIENT_OPERATIONS - 1;
    assert_preflight_resource(config, "T1/S2 coefficient operations");
    let mut config = defaults;
    config.max_coefficient_degree = FOUR_LOOP_T1S2_CLOSURE_COEFFICIENT_DEGREE - 1;
    assert_preflight_resource(config, "T1/S2 coefficient degree");
    let mut config = defaults;
    config.max_retained_coefficient_bytes = FOUR_LOOP_T1S2_CLOSURE_RETAINED_COEFFICIENT_BYTES - 1;
    assert_preflight_resource(config, "T1/S2 retained coefficient bytes");
    let mut config = defaults;
    config.max_coefficient_degree = 65_536;
    assert_preflight_resource(config, "configured T1/S2 coefficient degree");

    let mut config = defaults;
    config.one_loop.max_recurrence_steps = 1;
    assert_preflight_resource(config, "nested T1 recurrence steps");
    let mut config = defaults;
    config.one_loop.max_coefficient_operations = 7;
    assert_preflight_resource(config, "nested T1 coefficient operations");
    let mut config = defaults;
    config.one_loop.max_dense_term_operations = 23;
    assert_preflight_resource(config, "nested T1 dense term operations");
    let mut config = defaults;
    config.one_loop.max_coefficient_degree = 1;
    assert_preflight_resource(config, "nested T1 coefficient degree");
    let mut config = defaults;
    config.one_loop.max_coefficient_degree = 65_536;
    assert_preflight_resource(config, "configured nested T1 coefficient degree");
    let mut config = defaults;
    config.two_loop.max_explicit_terms = 5;
    assert_preflight_resource(config, "nested S2 explicit recurrence terms");
    let mut config = defaults;
    config.two_loop.max_raw_terms = 14;
    assert_preflight_resource(config, "nested S2 native provenance terms");
    let mut config = defaults;
    config.two_loop.max_states = 27;
    assert_preflight_resource(config, "nested S2 normal-form states");
    let mut config = defaults;
    config.two_loop.max_coefficient_operations = 199;
    assert_preflight_resource(config, "nested S2 coefficient operations");
    let mut config = defaults;
    config.two_loop.max_boundary_formula_iterations = 4;
    assert_preflight_resource(config, "nested S2 boundary iterations");
    let mut config = defaults;
    config.two_loop.max_coefficient_degree = 5;
    assert_preflight_resource(config, "nested S2 coefficient degree");
    let mut config = defaults;
    config.two_loop.max_coefficient_degree = 65_536;
    assert_preflight_resource(config, "configured nested S2 coefficient degree");
}

// Keep the exact inventory, component transport, local-service composition,
// full replay, and tamper checks in one serial Symbolica test process.
#[test]
fn exact_t1s2_slice_composes_and_replays_all_authenticated_plans() {
    let inventory = FourLoopNextInventory::build(FourLoopNextInventoryConfig::default()).unwrap();
    let transport =
        FourLoopComponentTransport::build(&inventory, FourLoopComponentTransportConfig::default())
            .unwrap();
    let mut closure_config = FourLoopT1S2ClosureConfig::default();
    closure_config.one_loop.max_recurrence_steps = 2;
    closure_config.one_loop.max_coefficient_operations = 8;
    closure_config.one_loop.max_dense_term_operations = 24;
    closure_config.one_loop.max_coefficient_degree = 2;
    closure_config.two_loop.max_explicit_terms = 6;
    closure_config.two_loop.max_raw_terms = 15;
    closure_config.two_loop.max_states = 28;
    closure_config.two_loop.max_coefficient_operations = 200;
    closure_config.two_loop.max_coefficient_degree = 6;
    closure_config.two_loop.max_boundary_formula_iterations = 5;
    let closure = FourLoopT1S2Closure::build(&transport, closure_config).unwrap();
    let stats = closure.stats();
    eprintln!(
        "four-loop T1/S2 closure stats: {stats:#?}\nchecksum: {:#018x}",
        closure.checksum()
    );
    eprintln!(
        "four-loop T1/S2 targets: {:#?}",
        closure
            .targets()
            .iter()
            .map(|reduction| (
                reduction.target().master(),
                reduction.target().powers().to_vec()
            ))
            .collect::<Vec<_>>()
    );

    assert_eq!(closure.status(), FourLoopT1S2ClosureStatus::ExactT1S2Slice);
    assert_eq!(closure.plans().len(), FOUR_LOOP_T1S2_CLOSURE_PLANS);
    assert_eq!(
        closure.open_leaf_ids().len(),
        FOUR_LOOP_T1S2_CLOSURE_OPEN_PLANS
    );
    assert_eq!(
        closure.occurrences().len(),
        FOUR_LOOP_T1S2_CLOSURE_OCCURRENCES
    );
    assert_eq!(stats.completed_plans(), FOUR_LOOP_T1S2_CLOSURE_PLANS);
    assert_eq!(stats.open_plans(), FOUR_LOOP_T1S2_CLOSURE_OPEN_PLANS);
    assert_eq!(stats.components(), FOUR_LOOP_T1S2_CLOSURE_COMPONENTS);
    assert_eq!(stats.local_slots(), FOUR_LOOP_T1S2_CLOSURE_LOCAL_SLOTS);
    assert!(stats.scalar_branches() <= FOUR_LOOP_T1S2_CLOSURE_SCALAR_BRANCHES);
    assert!(stats.component_calls() <= FOUR_LOOP_T1S2_CLOSURE_COMPONENT_CALLS);
    assert!(stats.unique_targets() <= FOUR_LOOP_T1S2_CLOSURE_UNIQUE_TARGETS);
    assert!(stats.convolution_pair_operations() <= FOUR_LOOP_T1S2_CLOSURE_CONVOLUTION_PAIRS);
    assert!(stats.precollection_terms() <= FOUR_LOOP_T1S2_CLOSURE_PRECOLLECTION_TERMS);
    assert!(stats.collected_terms() <= FOUR_LOOP_T1S2_CLOSURE_COLLECTED_TERMS);
    assert!(stats.mass_power_steps() <= FOUR_LOOP_T1S2_CLOSURE_MASS_POWER_STEPS);
    assert!(stats.coefficient_operations() <= FOUR_LOOP_T1S2_CLOSURE_COEFFICIENT_OPERATIONS);
    assert!(
        stats.retained_coefficient_bytes() <= FOUR_LOOP_T1S2_CLOSURE_RETAINED_COEFFICIENT_BYTES
    );
    assert_eq!(stats.completed_occurrences(), 1_134);
    assert_eq!(stats.open_occurrences(), 3_096);
    assert_eq!(stats.completed_rows(), 511);
    assert_eq!(stats.open_rows(), 969);
    assert_eq!(stats.mixed_rows(), 191);
    assert_eq!(stats.scalar_branches(), 454);
    assert_eq!(stats.base_branches(), 134);
    assert_eq!(stats.constant_branches(), 98);
    assert_eq!(stats.local_t1_branches(), 133);
    assert_eq!(stats.local_s2_branches(), 89);
    assert_eq!(stats.component_calls(), 1_442);
    assert_eq!(stats.t1_component_calls(), 1_068);
    assert_eq!(stats.s2_component_calls(), 374);
    assert_eq!(stats.unique_targets(), 25);
    assert_eq!(stats.t1_targets(), 4);
    assert_eq!(stats.s2_targets(), 21);
    assert_eq!(stats.cache_hits(), 1_417);
    assert_eq!(stats.convolution_pair_operations(), 1_252);
    assert_eq!(stats.precollection_terms(), 392);
    assert_eq!(stats.collected_terms(), 309);
    assert_eq!(stats.mass_power_steps(), 396);
    assert_eq!(stats.coefficient_operations(), 2_363);
    assert_eq!(stats.retained_coefficient_bytes(), 20_581);
    assert_eq!(stats.n0_plans(), 134);
    assert_eq!(stats.n1_plans(), 109);
    assert_eq!(closure.checksum(), 0xa2b9_2a62_c988_d2cb);
    assert_eq!(
        stats.scalar_branches(),
        stats.base_branches()
            + stats.constant_branches()
            + stats.local_t1_branches()
            + stats.local_s2_branches()
    );
    assert_eq!(
        stats.component_calls(),
        stats.t1_component_calls() + stats.s2_component_calls()
    );
    assert_eq!(
        stats.unique_targets(),
        stats.t1_targets() + stats.s2_targets()
    );
    assert_eq!(
        stats.cache_hits(),
        stats.component_calls() - stats.unique_targets()
    );
    assert_eq!(stats.base_branches(), stats.n0_plans());
    assert_eq!(stats.n0_plans() + stats.n1_plans(), stats.completed_plans());

    assert_eq!(
        closure.parent_status(),
        FourLoopT1S2ParentStatus::Partial {
            completed_plans: stats.completed_plans(),
            open_plans: stats.open_plans(),
            completed_occurrences: stats.completed_occurrences(),
            open_occurrences: stats.open_occurrences(),
        }
    );

    let mut class_counts = BTreeMap::new();
    let mut independently_counted_pairs = 0_usize;
    let independent_context = CoefficientContext::new(["d", "m2"]);
    let independent_m2 = independent_context.parameter("m2").unwrap();
    for plan in closure.plans() {
        *class_counts.entry(plan.product_class()).or_insert(0_usize) += 1;
        let source = transport
            .plans()
            .iter()
            .find(|source| source.leaf_id() == plan.leaf_id())
            .unwrap();
        assert_eq!(plan.branches().len(), source.scalar_branches().len());
        let mut independently_collected = ProductLinearCombination::new();
        for (branch_position, branch) in plan.branches().iter().enumerate() {
            let source_branch = &source.scalar_branches()[branch_position];
            assert_eq!(branch.branch_index(), branch_position);
            assert_eq!(branch.kind(), source_branch.kind());
            assert_eq!(branch.coefficient(), source_branch.coefficient());
            assert_eq!(branch.component_uses().len(), source.components().len());
            let lowered_owner = match source_branch.kind() {
                FourLoopComponentScalarBranchKind::Local {
                    component_index, ..
                } => Some(component_index),
                FourLoopComponentScalarBranchKind::Base
                | FourLoopComponentScalarBranchKind::Constant => None,
            };
            let mut independently_unscaled = ProductLinearCombination::from_term(
                MasterProduct::identity(),
                independent_context.one(),
            );
            let mut branch_pairs = 0_usize;
            for (position, component_use) in branch.component_uses().iter().enumerate() {
                assert_eq!(component_use.witness_index(), position);
                let reduction = &closure.targets()[component_use.target_index() as usize];
                let target = reduction.target();
                assert_eq!(target.master(), source.components()[position].master());
                let expected_powers = if lowered_owner == Some(position) {
                    source_branch.lowered_component_powers().unwrap()
                } else {
                    source.components()[position].local_powers()
                };
                assert_eq!(target.powers(), expected_powers);
                branch_pairs += independently_unscaled.len() * reduction.ordinary().len();
                independently_unscaled = independently_unscaled
                    .checked_convolve_with_limits(reduction.ordinary(), 3, u128::MAX)
                    .unwrap();
            }
            assert_eq!(&independently_unscaled, branch.ordinary_unscaled());
            assert_eq!(
                branch.ordinary_scaled(),
                &independently_unscaled.scaled(branch.coefficient())
            );
            assert_eq!(branch.convolution_pair_operations(), branch_pairs);
            independently_counted_pairs += branch_pairs;
            independently_collected.add_scaled(&independently_unscaled, branch.coefficient());
            for combination in [branch.ordinary_unscaled(), branch.ordinary_scaled()] {
                assert!(
                    combination.terms().keys().all(
                        |product| allowed_product(product) && product_loop_weight(product) == 4
                    )
                );
            }
        }
        assert_eq!(&independently_collected, plan.ordinary());
        let input_weight = source
            .key()
            .powers()
            .iter()
            .map(|&power| i64::from(power))
            .sum::<i64>();
        for product in plan.ordinary().terms().keys() {
            assert!(allowed_product(product));
            assert_eq!(product_loop_weight(product), 4);
        }
        assert_eq!(
            plan.ordinary().terms().keys().collect::<Vec<_>>(),
            plan.mass_normalized().terms().keys().collect::<Vec<_>>()
        );
        for (product, coefficient) in plan.mass_normalized().terms() {
            assert!(allowed_product(product));
            assert_eq!(product_loop_weight(product), 4);
            assert_eq!(coefficient.numerator.degree(1), 0);
            assert_eq!(coefficient.denominator.degree(1), 0);
            let ordinary_coefficient = plan.ordinary().coefficient(product).unwrap();
            let mass_weight = product
                .factors()
                .iter()
                .map(|(master, multiplicity)| {
                    i64::from(*multiplicity) * master.physical_lines() as i64
                })
                .sum::<i64>();
            let exponent = input_weight - mass_weight;
            let mut independently_normalized = ordinary_coefficient.clone();
            for _ in 0..exponent.unsigned_abs() {
                independently_normalized = if exponent >= 0 {
                    &independently_normalized * &independent_m2
                } else {
                    &independently_normalized / &independent_m2
                };
            }
            assert_eq!(&independently_normalized, coefficient);
        }
    }
    assert_eq!(
        independently_counted_pairs,
        stats.convolution_pair_operations()
    );
    assert_eq!(
        class_counts,
        BTreeMap::from([
            (FourLoopT1S2ProductClass::T1Fourth, 100),
            (FourLoopT1S2ProductClass::T1SquaredS2, 91),
            (FourLoopT1S2ProductClass::S2Squared, 52),
        ])
    );

    let independent_sunset = TwoLoopTopDotReducer::new(
        equal_mass_two_loop_vacuum_in_context(independent_context.clone()).unwrap(),
        TwoLoopTopDotConfig::default(),
    )
    .unwrap();
    for target in closure.targets() {
        assert!(matches!(
            target.target().master(),
            MassiveVacuumMaster::T1 | MassiveVacuumMaster::S2
        ));
        let expected_weight = target.target().master().loops();
        assert!(
            target
                .ordinary()
                .terms()
                .keys()
                .all(|product| product_loop_weight(product) == expected_weight)
        );
        let mut independent_local = ProductLinearCombination::new();
        match target.target().master() {
            MassiveVacuumMaster::T1 => {
                assert_eq!(target.service_schema(), OneLoopTadpoleReducer::SCHEMA);
                let power = target.target().powers()[0];
                if power > 0 {
                    let dimension = independent_context.parameter("d").unwrap();
                    let mut coefficient = independent_context.one();
                    for n in 1..i64::from(power) {
                        let two_n = independent_context.integer(2 * n);
                        coefficient =
                            &(&coefficient * &(&two_n - &dimension)) / &(&two_n * &independent_m2);
                    }
                    independent_local.add_term(
                        MasterProduct::from_factor(MassiveVacuumMaster::T1),
                        coefficient,
                    );
                }
            }
            MassiveVacuumMaster::S2 => {
                assert_eq!(
                    target.service_schema(),
                    "rustred-two-loop-top-dot-semantic-adapter-v1"
                );
                let output = independent_sunset
                    .reduce_integral(&Integral::new(target.target().powers().to_vec()))
                    .unwrap();
                for (integral, coefficient) in output.terms() {
                    let product = if integral == independent_sunset.sunset_master() {
                        MasterProduct::from_factor(MassiveVacuumMaster::S2)
                    } else {
                        assert_eq!(integral, independent_sunset.product_master());
                        MasterProduct::try_from_multiplicities([(MassiveVacuumMaster::T1, 2)])
                            .unwrap()
                    };
                    independent_local.add_term(product, coefficient.clone());
                }
            }
            _ => unreachable!(),
        }
        assert_eq!(&independent_local, target.ordinary());
    }
    assert_eq!(
        closure
            .targets()
            .iter()
            .map(|reduction| (
                reduction.target().master(),
                reduction.target().powers().to_vec()
            ))
            .collect::<Vec<_>>(),
        vec![
            (MassiveVacuumMaster::T1, vec![0]),
            (MassiveVacuumMaster::T1, vec![1]),
            (MassiveVacuumMaster::T1, vec![2]),
            (MassiveVacuumMaster::T1, vec![3]),
            (MassiveVacuumMaster::S2, vec![0, 1, 1]),
            (MassiveVacuumMaster::S2, vec![0, 1, 2]),
            (MassiveVacuumMaster::S2, vec![0, 1, 3]),
            (MassiveVacuumMaster::S2, vec![0, 2, 1]),
            (MassiveVacuumMaster::S2, vec![0, 2, 2]),
            (MassiveVacuumMaster::S2, vec![1, 0, 1]),
            (MassiveVacuumMaster::S2, vec![1, 0, 2]),
            (MassiveVacuumMaster::S2, vec![1, 1, 0]),
            (MassiveVacuumMaster::S2, vec![1, 1, 1]),
            (MassiveVacuumMaster::S2, vec![1, 1, 2]),
            (MassiveVacuumMaster::S2, vec![1, 1, 3]),
            (MassiveVacuumMaster::S2, vec![1, 2, 0]),
            (MassiveVacuumMaster::S2, vec![1, 2, 1]),
            (MassiveVacuumMaster::S2, vec![1, 2, 2]),
            (MassiveVacuumMaster::S2, vec![1, 3, 1]),
            (MassiveVacuumMaster::S2, vec![2, 1, 0]),
            (MassiveVacuumMaster::S2, vec![2, 1, 1]),
            (MassiveVacuumMaster::S2, vec![2, 1, 2]),
            (MassiveVacuumMaster::S2, vec![2, 2, 0]),
            (MassiveVacuumMaster::S2, vec![2, 2, 1]),
            (MassiveVacuumMaster::S2, vec![3, 1, 1]),
        ]
    );

    let open = closure
        .open_leaf_ids()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(open.len(), FOUR_LOOP_T1S2_CLOSURE_OPEN_PLANS);
    let completed = closure
        .plans()
        .iter()
        .map(|plan| plan.leaf_id())
        .collect::<BTreeSet<_>>();
    assert!(completed.is_disjoint(&open));
    assert_eq!(completed.len() + open.len(), transport.plans().len());
    assert_eq!(
        completed.union(&open).copied().collect::<BTreeSet<_>>(),
        transport
            .plans()
            .iter()
            .map(|plan| plan.leaf_id())
            .collect::<BTreeSet<_>>()
    );
    let mut completed_rows = BTreeSet::new();
    let mut open_rows = BTreeSet::new();
    for (occurrence, source_occurrence) in closure.occurrences().iter().zip(transport.occurrences())
    {
        assert_eq!(occurrence.row_index(), source_occurrence.row_index());
        assert_eq!(occurrence.path_index(), source_occurrence.path_index());
        assert_eq!(occurrence.leaf_id(), source_occurrence.leaf_id());
        if let Some(index) = occurrence.completed_plan_index() {
            assert_eq!(
                closure.plans()[index as usize].leaf_id(),
                occurrence.leaf_id()
            );
            completed_rows.insert(occurrence.row_index());
        } else {
            assert!(open.contains(&occurrence.leaf_id()));
            open_rows.insert(occurrence.row_index());
        }
    }
    assert_eq!(completed_rows.len(), stats.completed_rows());
    assert_eq!(open_rows.len(), stats.open_rows());
    assert_eq!(
        completed_rows.intersection(&open_rows).count(),
        stats.mixed_rows()
    );

    closure.replay().unwrap();

    let local = closure
        .targets()
        .iter()
        .find(|target| !target.ordinary().is_zero())
        .unwrap();
    let (local_product, local_coefficient) = local.ordinary().terms().first_key_value().unwrap();
    let context_one = local_coefficient / local_coefficient;
    let tampered_local =
        local.with_output_coefficient_for_replay(local_product, local_coefficient + &context_one);
    assert!(closure.replay_target_candidate(&tampered_local).is_err());
    let zero_local = closure
        .targets()
        .iter()
        .find(|target| target.ordinary().is_zero())
        .unwrap();
    let inserted_zero = zero_local.with_output_coefficient_for_replay(
        &MasterProduct::from_factor(MassiveVacuumMaster::T1),
        context_one.clone(),
    );
    assert!(closure.replay_target_candidate(&inserted_zero).is_err());
    let unexpected_local = local.with_output_coefficient_for_replay(
        &MasterProduct::from_factor(MassiveVacuumMaster::B4),
        context_one.clone(),
    );
    assert!(closure.replay_target_candidate(&unexpected_local).is_err());

    let plan = closure
        .plans()
        .iter()
        .find(|plan| {
            !plan.mass_normalized().is_zero()
                && plan
                    .branches()
                    .iter()
                    .any(|branch| !branch.component_uses().is_empty())
        })
        .unwrap();
    let branch_index = plan
        .branches()
        .iter()
        .position(|branch| !branch.component_uses().is_empty())
        .unwrap();
    let bad_branch = plan.with_branch_coefficient_for_replay(
        branch_index,
        plan.branches()[branch_index].coefficient() + &context_one,
    );
    assert!(closure.replay_plan_candidate(&bad_branch).is_err());
    let original_target = plan.branches()[branch_index].component_uses()[0].target_index();
    let replacement_target = (usize::from(original_target) + 1) % closure.targets().len();
    let bad_component =
        plan.with_component_target_for_replay(branch_index, 0, replacement_target as u16);
    assert!(closure.replay_plan_candidate(&bad_component).is_err());
    let (product, coefficient) = plan.mass_normalized().terms().first_key_value().unwrap();
    let bad_normalized =
        plan.with_mass_normalized_coefficient_for_replay(product, coefficient + &context_one);
    assert!(closure.replay_plan_candidate(&bad_normalized).is_err());
    let inserted_normalized = plan.with_mass_normalized_coefficient_for_replay(
        &MasterProduct::from_factor(MassiveVacuumMaster::B4),
        context_one.clone(),
    );
    assert!(closure.replay_plan_candidate(&inserted_normalized).is_err());

    let (occurrence_index, occurrence) = closure
        .occurrences()
        .iter()
        .copied()
        .enumerate()
        .find(|(_, occurrence)| occurrence.completed_plan_index().is_some())
        .unwrap();
    let bad_occurrence = occurrence.with_completed_plan_index_for_replay(None);
    assert!(
        closure
            .replay_occurrence_candidate(occurrence_index, bad_occurrence)
            .is_err()
    );
    let (open_occurrence_index, open_occurrence) = closure
        .occurrences()
        .iter()
        .copied()
        .enumerate()
        .find(|(_, occurrence)| occurrence.completed_plan_index().is_none())
        .unwrap();
    assert!(
        closure
            .replay_occurrence_candidate(
                open_occurrence_index,
                open_occurrence.with_completed_plan_index_for_replay(Some(0)),
            )
            .is_err()
    );
}
