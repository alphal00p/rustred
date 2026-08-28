use std::collections::{BTreeMap, BTreeSet};

use rustred::{MasterProduct, ProductLinearCombination};
use rustred_legacy_oracles::{
    FOUR_LOOP_THREE_LOOP_CLOSURE_COEFFICIENT_DEGREE,
    FOUR_LOOP_THREE_LOOP_CLOSURE_COEFFICIENT_OPERATION_BOUND,
    FOUR_LOOP_THREE_LOOP_CLOSURE_COLLECTED_TERM_BOUND,
    FOUR_LOOP_THREE_LOOP_CLOSURE_COMPONENT_CALLS, FOUR_LOOP_THREE_LOOP_CLOSURE_COMPONENTS,
    FOUR_LOOP_THREE_LOOP_CLOSURE_CONVOLUTION_PAIR_BOUND, FOUR_LOOP_THREE_LOOP_CLOSURE_LOCAL_SLOTS,
    FOUR_LOOP_THREE_LOOP_CLOSURE_MASS_POWER_STEP_BOUND, FOUR_LOOP_THREE_LOOP_CLOSURE_OCCURRENCES,
    FOUR_LOOP_THREE_LOOP_CLOSURE_OUTSIDE_PLANS, FOUR_LOOP_THREE_LOOP_CLOSURE_PLANS,
    FOUR_LOOP_THREE_LOOP_CLOSURE_PRECOLLECTION_TERM_BOUND,
    FOUR_LOOP_THREE_LOOP_CLOSURE_RETAINED_OUTPUT_COEFFICIENT_BYTES,
    FOUR_LOOP_THREE_LOOP_CLOSURE_SCALAR_BRANCHES, FOUR_LOOP_THREE_LOOP_CLOSURE_UNIQUE_TARGETS,
    FOUR_LOOP_THREE_LOOP_SERVICE_B4_TARGETS, FOUR_LOOP_THREE_LOOP_SERVICE_F5_TARGETS,
    FOUR_LOOP_THREE_LOOP_SERVICE_M6_TARGETS, FOUR_LOOP_THREE_LOOP_SERVICE_NATIVE_IDENTITIES,
    FOUR_LOOP_THREE_LOOP_SERVICE_OUTPUT_TERM_BOUND,
    FOUR_LOOP_THREE_LOOP_SERVICE_RETAINED_OUTPUT_COEFFICIENT_BYTE_BOUND,
    FOUR_LOOP_THREE_LOOP_SERVICE_T1_TARGETS, FOUR_LOOP_THREE_LOOP_SERVICE_TARGET_MANIFEST_CHECKSUM,
    FOUR_LOOP_THREE_LOOP_SERVICE_TARGETS, FourLoopComponentScalarBranchKind,
    FourLoopComponentTransport, FourLoopComponentTransportConfig, FourLoopNextInventory,
    FourLoopNextInventoryConfig, FourLoopThreeLoopClosure, FourLoopThreeLoopClosureConfig,
    FourLoopThreeLoopClosureError, FourLoopThreeLoopClosureStatus, FourLoopThreeLoopParentStatus,
    FourLoopThreeLoopProductClass, FourLoopThreeLoopService, FourLoopThreeLoopServiceError,
    FourLoopThreeLoopServiceStatus, MassiveVacuumMaster,
};

fn assert_closure_preflight(
    config: FourLoopThreeLoopClosureConfig,
    expected_resource: &'static str,
) {
    assert!(matches!(
        FourLoopThreeLoopClosure::preflight_config(config),
        Err(FourLoopThreeLoopClosureError::ResourceLimit { resource, .. })
            if resource == expected_resource
    ));
}

fn assert_service_preflight(
    config: rustred_legacy_oracles::FourLoopThreeLoopServiceConfig,
    expected_resource: &'static str,
) {
    assert!(matches!(
        FourLoopThreeLoopService::preflight_config(config),
        Err(FourLoopThreeLoopServiceError::ResourceLimit { resource, .. })
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

fn allowed_four_loop_product(product: &MasterProduct<MassiveVacuumMaster>) -> bool {
    let t1 = product.multiplicity(&MassiveVacuumMaster::T1);
    let s2 = product.multiplicity(&MassiveVacuumMaster::S2);
    let b4 = product.multiplicity(&MassiveVacuumMaster::B4);
    let f5 = product.multiplicity(&MassiveVacuumMaster::F5);
    let m6 = product.multiplicity(&MassiveVacuumMaster::M6);
    matches!(
        (t1, s2, b4, f5, m6),
        (4, 0, 0, 0, 0) | (2, 1, 0, 0, 0) | (1, 0, 1, 0, 0) | (1, 0, 0, 1, 0) | (1, 0, 0, 0, 1)
    )
}

fn allowed_three_loop_product(product: &MasterProduct<MassiveVacuumMaster>) -> bool {
    let t1 = product.multiplicity(&MassiveVacuumMaster::T1);
    let s2 = product.multiplicity(&MassiveVacuumMaster::S2);
    let b4 = product.multiplicity(&MassiveVacuumMaster::B4);
    let f5 = product.multiplicity(&MassiveVacuumMaster::F5);
    let m6 = product.multiplicity(&MassiveVacuumMaster::M6);
    matches!(
        (t1, s2, b4, f5, m6),
        (1, 0, 0, 0, 0)
            | (3, 0, 0, 0, 0)
            | (1, 1, 0, 0, 0)
            | (0, 0, 1, 0, 0)
            | (0, 0, 0, 1, 0)
            | (0, 0, 0, 0, 1)
    )
}

fn target_degrees(powers: &[i32]) -> (u64, u64) {
    powers.iter().fold((0, 0), |(dots, numerators), &power| {
        if power > 1 {
            (dots + u64::try_from(power - 1).unwrap(), numerators)
        } else if power < 0 {
            (dots, numerators + i64::from(power).unsigned_abs())
        } else {
            (dots, numerators)
        }
    })
}

#[test]
fn frozen_three_loop_component_resources_fail_before_inventory_work() {
    let defaults = FourLoopThreeLoopClosureConfig::default();
    for (config, resource) in [
        (
            {
                let mut c = defaults;
                c.max_plans = FOUR_LOOP_THREE_LOOP_CLOSURE_PLANS - 1;
                c
            },
            "three-loop plans",
        ),
        (
            {
                let mut c = defaults;
                c.max_outside_plans = FOUR_LOOP_THREE_LOOP_CLOSURE_OUTSIDE_PLANS - 1;
                c
            },
            "outside plans",
        ),
        (
            {
                let mut c = defaults;
                c.max_occurrences = FOUR_LOOP_THREE_LOOP_CLOSURE_OCCURRENCES - 1;
                c
            },
            "occurrences",
        ),
        (
            {
                let mut c = defaults;
                c.max_components = FOUR_LOOP_THREE_LOOP_CLOSURE_COMPONENTS - 1;
                c
            },
            "components",
        ),
        (
            {
                let mut c = defaults;
                c.max_local_slots = FOUR_LOOP_THREE_LOOP_CLOSURE_LOCAL_SLOTS - 1;
                c
            },
            "local slots",
        ),
        (
            {
                let mut c = defaults;
                c.max_scalar_branches = FOUR_LOOP_THREE_LOOP_CLOSURE_SCALAR_BRANCHES - 1;
                c
            },
            "scalar branches",
        ),
        (
            {
                let mut c = defaults;
                c.max_component_calls = FOUR_LOOP_THREE_LOOP_CLOSURE_COMPONENT_CALLS - 1;
                c
            },
            "component calls",
        ),
        (
            {
                let mut c = defaults;
                c.max_unique_targets = FOUR_LOOP_THREE_LOOP_CLOSURE_UNIQUE_TARGETS - 1;
                c
            },
            "unique targets",
        ),
        (
            {
                let mut c = defaults;
                c.max_convolution_pair_operations =
                    FOUR_LOOP_THREE_LOOP_CLOSURE_CONVOLUTION_PAIR_BOUND - 1;
                c
            },
            "convolution pairs",
        ),
        (
            {
                let mut c = defaults;
                c.max_precollection_terms =
                    FOUR_LOOP_THREE_LOOP_CLOSURE_PRECOLLECTION_TERM_BOUND - 1;
                c
            },
            "precollection terms",
        ),
        (
            {
                let mut c = defaults;
                c.max_collected_terms = FOUR_LOOP_THREE_LOOP_CLOSURE_COLLECTED_TERM_BOUND - 1;
                c
            },
            "collected terms",
        ),
        (
            {
                let mut c = defaults;
                c.max_mass_power_steps = FOUR_LOOP_THREE_LOOP_CLOSURE_MASS_POWER_STEP_BOUND - 1;
                c
            },
            "mass-power steps",
        ),
        (
            {
                let mut c = defaults;
                c.max_coefficient_operations =
                    FOUR_LOOP_THREE_LOOP_CLOSURE_COEFFICIENT_OPERATION_BOUND - 1;
                c
            },
            "coefficient operations",
        ),
        (
            {
                let mut c = defaults;
                c.max_retained_output_coefficient_bytes =
                    FOUR_LOOP_THREE_LOOP_CLOSURE_RETAINED_OUTPUT_COEFFICIENT_BYTES - 1;
                c
            },
            "retained coefficient bytes",
        ),
    ] {
        assert_closure_preflight(config, resource);
    }
    let mut config = defaults;
    config.max_coefficient_degree = FOUR_LOOP_THREE_LOOP_CLOSURE_COEFFICIENT_DEGREE - 1;
    assert_closure_preflight(config, "coefficient degree");
    let mut config = defaults;
    config.max_coefficient_degree = 65_536;
    assert_closure_preflight(config, "configured coefficient degree");

    let service = defaults.service;
    for (config, resource) in [
        (
            {
                let mut c = service;
                c.max_targets = FOUR_LOOP_THREE_LOOP_SERVICE_TARGETS - 1;
                c
            },
            "local target manifest",
        ),
        (
            {
                let mut c = service;
                c.max_t1_targets = FOUR_LOOP_THREE_LOOP_SERVICE_T1_TARGETS - 1;
                c
            },
            "T1 local targets",
        ),
        (
            {
                let mut c = service;
                c.max_b4_targets = FOUR_LOOP_THREE_LOOP_SERVICE_B4_TARGETS - 1;
                c
            },
            "B4-owner local targets",
        ),
        (
            {
                let mut c = service;
                c.max_f5_targets = FOUR_LOOP_THREE_LOOP_SERVICE_F5_TARGETS - 1;
                c
            },
            "F5-owner local targets",
        ),
        (
            {
                let mut c = service;
                c.max_m6_targets = FOUR_LOOP_THREE_LOOP_SERVICE_M6_TARGETS - 1;
                c
            },
            "M6-owner local targets",
        ),
        (
            {
                let mut c = service;
                c.max_output_terms = FOUR_LOOP_THREE_LOOP_SERVICE_OUTPUT_TERM_BOUND - 1;
                c
            },
            "semantic output terms",
        ),
        (
            {
                let mut c = service;
                c.max_retained_output_coefficient_bytes =
                    FOUR_LOOP_THREE_LOOP_SERVICE_RETAINED_OUTPUT_COEFFICIENT_BYTE_BOUND - 1;
                c
            },
            "retained semantic-output coefficient bytes",
        ),
    ] {
        assert_service_preflight(config, resource);
    }
    let mut config = service;
    config.one_loop.max_recurrence_steps = 1;
    assert_service_preflight(config, "nested T1 recurrence steps");
    let mut config = service;
    config.one_loop.max_coefficient_operations = 7;
    assert_service_preflight(config, "nested T1 coefficient operations");
    let mut config = service;
    config.one_loop.max_dense_term_operations = 23;
    assert_service_preflight(config, "nested T1 dense term operations");
    let mut config = service;
    config.one_loop.max_coefficient_degree = 1;
    assert_service_preflight(config, "nested T1 coefficient degree");
    let mut config = service;
    config.one_loop.max_coefficient_degree = 65_536;
    assert_service_preflight(config, "configured nested T1 coefficient degree");
}

// One process owns the expensive Symbolica pipeline build and all candidate
// replays; do not split these assertions into parallel integration tests.
#[test]
fn exact_three_loop_component_slice_composes_and_replays_retained_witnesses() {
    let inventory = FourLoopNextInventory::build(FourLoopNextInventoryConfig::default()).unwrap();
    let transport =
        FourLoopComponentTransport::build(&inventory, FourLoopComponentTransportConfig::default())
            .unwrap();
    let closure =
        FourLoopThreeLoopClosure::build(&transport, FourLoopThreeLoopClosureConfig::default())
            .unwrap();
    let stats = closure.stats();
    let service = closure.service();
    let service_stats = service.stats();

    assert_eq!(
        closure.status(),
        FourLoopThreeLoopClosureStatus::ExactThreeLoopComponentSliceGenericQ
    );
    assert_eq!(
        service.status(),
        FourLoopThreeLoopServiceStatus::ExactFiniteBoxGenericQ
    );
    assert!(service.generic_q_caveat().contains("Q(d,m2)"));
    assert_eq!(closure.plans().len(), 823);
    assert_eq!(closure.outside_leaf_ids().len(), 243);
    assert_eq!(closure.occurrences().len(), 4_230);
    assert_eq!(stats.completed_plans(), 823);
    assert_eq!(stats.outside_plans(), 243);
    assert_eq!(stats.completed_occurrences(), 3_096);
    assert_eq!(stats.outside_occurrences(), 1_134);
    assert_eq!(stats.completed_rows(), 969);
    assert_eq!(stats.outside_rows(), 511);
    assert_eq!(stats.mixed_rows(), 191);
    assert_eq!(stats.components(), 1_646);
    assert_eq!(stats.local_slots(), 5_761);
    assert_eq!(stats.scalar_branches(), 1_884);
    assert_eq!(stats.base_branches(), 443);
    assert_eq!(stats.constant_branches(), 323);
    assert_eq!(stats.local_t1_branches(), 186);
    assert_eq!(stats.local_b4_branches(), 220);
    assert_eq!(stats.local_f5_branches(), 656);
    assert_eq!(stats.local_m6_branches(), 56);
    assert_eq!(stats.component_calls(), 3_768);
    assert_eq!(stats.t1_component_calls(), 1_884);
    assert_eq!(stats.b4_component_calls(), 444);
    assert_eq!(stats.f5_component_calls(), 1_260);
    assert_eq!(stats.m6_component_calls(), 180);
    assert_eq!(stats.unique_targets(), 204);
    assert_eq!(stats.cache_hits(), 3_564);
    assert_eq!(stats.convolution_pair_operations(), 7_356);
    assert_eq!(stats.precollection_terms(), 3_598);
    assert_eq!(stats.collected_terms(), 2_159);
    assert_eq!(stats.mass_power_steps(), 4_279);
    assert_eq!(stats.coefficient_operations(), 17_456);
    assert_eq!(stats.retained_output_coefficient_bytes(), 256_603);
    assert_eq!(stats.n0_plans(), 443);
    assert_eq!(stats.n1_plans(), 380);
    assert_eq!(closure.checksum(), 0xda3c_250b_95b1_0976);
    assert_eq!(
        closure.parent_status(),
        FourLoopThreeLoopParentStatus::Partial {
            completed_plans: 823,
            outside_plans: 243,
            completed_occurrences: 3_096,
            outside_occurrences: 1_134,
        }
    );

    assert_eq!(service.targets().len(), 204);
    assert_eq!(service.reductions().len(), 204);
    assert_eq!(service_stats.targets(), 204);
    assert_eq!(service_stats.t1_targets(), 4);
    assert_eq!(service_stats.b4_targets(), 41);
    assert_eq!(service_stats.f5_targets(), 89);
    assert_eq!(service_stats.m6_targets(), 70);
    assert_eq!(
        service_stats.native_target_identities(),
        FOUR_LOOP_THREE_LOOP_SERVICE_NATIVE_IDENTITIES
    );
    assert_eq!(service_stats.output_terms(), 502);
    assert_eq!(service_stats.retained_output_coefficient_bytes(), 12_555);
    assert_eq!(service.manifest_checksum(), 0x9bb3_c1a6_d4ea_7bdd);
    assert_eq!(
        service.manifest_checksum(),
        FOUR_LOOP_THREE_LOOP_SERVICE_TARGET_MANIFEST_CHECKSUM
    );
    assert_eq!(service.checksum(), 0x6a1b_52dd_b449_d5bb);
    let pipeline = service.pipeline_stats();
    assert_eq!(pipeline.input_equations, 306);
    assert_eq!(pipeline.rules, 149);
    assert_eq!(pipeline.dependent_equations, 157);
    assert_eq!(pipeline.maximum_terms, 30);

    assert!(
        service
            .coefficient_context()
            .parameter_names()
            .iter()
            .map(String::as_str)
            .eq(["d", "m2"])
    );
    assert!(
        service
            .coefficient_context()
            .has_same_variable_map(service.family().coefficients())
    );

    let mut degree_census = BTreeMap::new();
    let mut observed_service_products = BTreeSet::new();
    for (target, reduction) in service.targets().iter().zip(service.reductions()) {
        assert_eq!(target, reduction.target());
        *degree_census
            .entry((target.owner(), target_degrees(target.powers())))
            .or_insert(0usize) += 1;
        let expected_weight = target.owner().loops();
        for product in reduction.ordinary().terms().keys() {
            assert!(allowed_three_loop_product(product));
            assert_eq!(product_loop_weight(product), expected_weight);
            observed_service_products.insert(product.clone());
        }
    }
    assert_eq!(observed_service_products.len(), 6);
    assert_eq!(
        degree_census,
        rustred_legacy_oracles::FOUR_LOOP_THREE_LOOP_SERVICE_DEGREE_CENSUS
            .into_iter()
            .map(|(owner, dots, numerators, count)| ((owner, (dots, numerators)), count))
            .collect()
    );

    let mut class_census = BTreeMap::new();
    for plan in closure.plans() {
        *class_census.entry(plan.product_class()).or_insert(0usize) += 1;
        let source = transport
            .plans()
            .iter()
            .find(|source| source.leaf_id() == plan.leaf_id())
            .unwrap();
        assert_eq!(plan.branches().len(), source.scalar_branches().len());
        let mut independently_collected = ProductLinearCombination::new();
        for (branch_index, branch) in plan.branches().iter().enumerate() {
            let source_branch = &source.scalar_branches()[branch_index];
            assert_eq!(branch.branch_index(), branch_index);
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
            let mut independently_convolved = ProductLinearCombination::from_term(
                MasterProduct::identity(),
                service.coefficient_context().one(),
            );
            let mut convolution_pairs = 0usize;
            for (component_index, component_use) in branch.component_uses().iter().enumerate() {
                assert_eq!(
                    component_use.witness_index(),
                    source.components()[component_index].witness_index()
                );
                let target = service.targets()[component_use.target_index() as usize].clone();
                assert_eq!(
                    target.owner(),
                    source.components()[component_index].master()
                );
                let expected_powers = if lowered_owner == Some(component_index) {
                    source_branch.lowered_component_powers().unwrap()
                } else {
                    source.components()[component_index].local_powers()
                };
                assert_eq!(target.powers(), expected_powers);
                let reduction = &service.reductions()[component_use.target_index() as usize];
                convolution_pairs += independently_convolved.len() * reduction.ordinary().len();
                independently_convolved = independently_convolved
                    .checked_convolve_with_limits(reduction.ordinary(), usize::MAX, u128::MAX)
                    .unwrap();
            }
            assert_eq!(branch.convolution_pair_operations(), convolution_pairs);
            assert_eq!(branch.ordinary_unscaled(), &independently_convolved);
            assert_eq!(
                branch.ordinary_scaled(),
                &independently_convolved.scaled(branch.coefficient())
            );
            independently_collected.add_scaled(branch.ordinary_unscaled(), branch.coefficient());
            for combination in [branch.ordinary_unscaled(), branch.ordinary_scaled()] {
                assert!(combination.terms().keys().all(|product| {
                    allowed_four_loop_product(product) && product_loop_weight(product) == 4
                }));
            }
        }
        assert_eq!(plan.ordinary(), &independently_collected);
        for combination in [plan.ordinary(), plan.mass_normalized()] {
            assert!(combination.terms().keys().all(|product| {
                allowed_four_loop_product(product) && product_loop_weight(product) == 4
            }));
        }
        assert_eq!(
            plan.ordinary().terms().keys().collect::<Vec<_>>(),
            plan.mass_normalized().terms().keys().collect::<Vec<_>>()
        );
        assert!(plan.mass_normalized().terms().values().all(|coefficient| {
            coefficient.numerator.degree(1) == 0 && coefficient.denominator.degree(1) == 0
        }));
    }
    assert_eq!(
        class_census,
        BTreeMap::from([
            (FourLoopThreeLoopProductClass::T1B4, 223),
            (FourLoopThreeLoopProductClass::T1F5, 494),
            (FourLoopThreeLoopProductClass::T1M6, 106),
        ])
    );

    let completed = closure
        .plans()
        .iter()
        .map(|plan| plan.leaf_id())
        .collect::<BTreeSet<_>>();
    let outside = closure
        .outside_leaf_ids()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    assert!(completed.is_disjoint(&outside));
    assert_eq!(completed.len() + outside.len(), transport.plans().len());
    assert_eq!(
        completed.union(&outside).copied().collect::<BTreeSet<_>>(),
        transport
            .plans()
            .iter()
            .map(|plan| plan.leaf_id())
            .collect()
    );
    let mut completed_rows = BTreeSet::new();
    let mut outside_rows = BTreeSet::new();
    let mut completed_occurrences = 0usize;
    let mut outside_occurrences = 0usize;
    assert_eq!(closure.occurrences().len(), transport.occurrences().len());
    for (occurrence, source) in closure.occurrences().iter().zip(transport.occurrences()) {
        assert_eq!(occurrence.row_index(), source.row_index());
        assert_eq!(occurrence.path_index(), source.path_index());
        assert_eq!(occurrence.leaf_id(), source.leaf_id());
        if let Some(index) = occurrence.completed_plan_index() {
            assert_eq!(closure.plans()[index as usize].leaf_id(), source.leaf_id());
            completed_rows.insert(source.row_index());
            completed_occurrences += 1;
        } else {
            assert!(outside.contains(&source.leaf_id()));
            outside_rows.insert(source.row_index());
            outside_occurrences += 1;
        }
    }
    assert_eq!(completed_occurrences, 3_096);
    assert_eq!(outside_occurrences, 1_134);
    assert_eq!(completed_rows.len(), 969);
    assert_eq!(outside_rows.len(), 511);
    assert_eq!(completed_rows.intersection(&outside_rows).count(), 191);

    service.validate_retained_reductions().unwrap();
    let target_index = service
        .reductions()
        .iter()
        .position(|reduction| !reduction.ordinary().is_zero())
        .unwrap();
    let target = &service.reductions()[target_index];
    service
        .replay_target_candidate(target_index, target)
        .unwrap();
    let (product, coefficient) = target.ordinary().terms().first_key_value().unwrap();
    let one = service.coefficient_context().one();
    let tampered_target = target.with_output_coefficient_for_replay(product, coefficient + &one);
    assert!(
        service
            .replay_target_candidate(target_index, &tampered_target)
            .is_err()
    );
    let swapped_index = (target_index + 1) % service.targets().len();
    let swapped_target = target.with_target_for_replay(service.targets()[swapped_index].clone());
    assert!(
        service
            .replay_target_candidate(target_index, &swapped_target)
            .is_err()
    );

    let plan = closure
        .plans()
        .iter()
        .find(|plan| !plan.mass_normalized().is_zero())
        .unwrap();
    closure.replay_plan_candidate(plan).unwrap();
    let bad_branch =
        plan.with_branch_coefficient_for_replay(0, plan.branches()[0].coefficient() + &one);
    assert!(closure.replay_plan_candidate(&bad_branch).is_err());
    let original_target = plan.branches()[0].component_uses()[0].target_index();
    let replacement = (usize::from(original_target) + 1) % service.targets().len();
    let bad_component = plan.with_component_target_for_replay(0, 0, replacement as u16);
    assert!(closure.replay_plan_candidate(&bad_component).is_err());
    let (product, coefficient) = plan.mass_normalized().terms().first_key_value().unwrap();
    let bad_normalized =
        plan.with_mass_normalized_coefficient_for_replay(product, coefficient + &one);
    assert!(closure.replay_plan_candidate(&bad_normalized).is_err());

    let (completed_index, completed_occurrence) = closure
        .occurrences()
        .iter()
        .copied()
        .enumerate()
        .find(|(_, occurrence)| occurrence.completed_plan_index().is_some())
        .unwrap();
    closure
        .replay_occurrence_candidate(completed_index, completed_occurrence)
        .unwrap();
    assert!(
        closure
            .replay_occurrence_candidate(
                completed_index,
                completed_occurrence.with_completed_plan_index_for_replay(None),
            )
            .is_err()
    );
    let (outside_index, outside_occurrence) = closure
        .occurrences()
        .iter()
        .copied()
        .enumerate()
        .find(|(_, occurrence)| occurrence.completed_plan_index().is_none())
        .unwrap();
    assert!(
        closure
            .replay_occurrence_candidate(
                outside_index,
                outside_occurrence.with_completed_plan_index_for_replay(Some(0)),
            )
            .is_err()
    );
}
