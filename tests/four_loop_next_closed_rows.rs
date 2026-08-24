#![cfg(feature = "legacy-authored-oracles")]

use std::collections::{BTreeMap, BTreeSet};

use rustred::{
    Coefficient, FOUR_LOOP_NEXT_CLOSED_ROWS, FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES,
    FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_PLANS, FOUR_LOOP_NEXT_CLOSED_ROWS_CANCELED_BOUNDARY_GROUPS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_CHECKSUM, FOUR_LOOP_NEXT_CLOSED_ROWS_COEFFICIENT_ADDITIONS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_COEFFICIENT_DIVISIONS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_COEFFICIENT_MULTIPLICATIONS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES, FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRY_BOUND,
    FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_COLUMNS, FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_GROUPS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_PATHS, FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMN_BOUND,
    FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS, FOUR_LOOP_NEXT_CLOSED_ROWS_MASS_POWER_STEPS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_BOUNDARY_GROUPS, FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_PATHS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_WIDTH,
    FOUR_LOOP_NEXT_CLOSED_ROWS_NONZERO_BOUNDARY_CONTRIBUTORS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_NONZERO_BOUNDARY_GROUPS, FOUR_LOOP_NEXT_CLOSED_ROWS_PATHS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_PRIMARY_CONTRIBUTION_BOUND,
    FOUR_LOOP_NEXT_CLOSED_ROWS_PRIMARY_CONTRIBUTIONS, FOUR_LOOP_NEXT_CLOSED_ROWS_PRODUCT_COLUMNS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_AUDIT_CONTRIBUTION_BOUND,
    FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_AUDIT_CONTRIBUTIONS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_BOUNDARY_GROUPS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_BOUNDARY_GROUPS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_CANCELED_BOUNDARY_GROUPS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_SURVIVING_BOUNDARY_GROUPS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_RETAINED_COEFFICIENT_BYTES,
    FOUR_LOOP_NEXT_CLOSED_ROWS_RETAINED_COEFFICIENT_TERMS, FOUR_LOOP_NEXT_CLOSED_ROWS_ZERO_ROWS,
    FOUR_LOOP_T1S2_CLOSURE_PLANS, FOUR_LOOP_THREE_LOOP_CLOSURE_PLANS, FourLoopComponentTransport,
    FourLoopComponentTransportConfig, FourLoopCornerColumnId, FourLoopNextClosedRows,
    FourLoopNextClosedRowsConfig, FourLoopNextClosedRowsError, FourLoopNextClosedRowsStatus,
    FourLoopNextClosureSlice, FourLoopNextInventory, FourLoopNextInventoryConfig, FourLoopNextLeaf,
    FourLoopNextPathDisposition, FourLoopT1S2Closure, FourLoopT1S2ClosureConfig,
    FourLoopThreeLoopClosure, FourLoopThreeLoopClosureConfig, MassiveVacuumMaster, MasterProduct,
    SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
};

fn assert_preflight_resource(
    config: FourLoopNextClosedRowsConfig,
    expected_resource: &'static str,
) {
    assert!(matches!(
        FourLoopNextClosedRows::preflight_config(config),
        Err(FourLoopNextClosedRowsError::ResourceLimit { resource, .. })
            if resource == expected_resource
    ));
}

fn master_product(factors: &[MassiveVacuumMaster]) -> MasterProduct<MassiveVacuumMaster> {
    MasterProduct::try_from_factors(factors.iter().copied()).unwrap()
}

fn product_mass_weight(product: &MasterProduct<MassiveVacuumMaster>) -> i64 {
    product
        .factors()
        .iter()
        .map(|(master, multiplicity)| {
            i64::from(*multiplicity) * i64::try_from(master.physical_lines()).unwrap()
        })
        .sum()
}

fn apply_mass_power(
    context: &rustred::CoefficientContext,
    coefficient: &Coefficient,
    exponent: i64,
) -> Coefficient {
    let mass = context.parameter("m2").unwrap();
    let mut output = coefficient.clone();
    for _ in 0..exponent.unsigned_abs() {
        output = if exponent >= 0 {
            &output * &mass
        } else {
            &output / &mass
        };
    }
    output
}

fn add_product_term(
    output: &mut BTreeMap<MasterProduct<MassiveVacuumMaster>, Coefficient>,
    product: MasterProduct<MassiveVacuumMaster>,
    coefficient: Coefficient,
) {
    if coefficient.is_zero() {
        return;
    }
    if let Some(current) = output.remove(&product) {
        let sum = &current + &coefficient;
        if !sum.is_zero() {
            output.insert(product, sum);
        }
    } else {
        output.insert(product, coefficient);
    }
}

#[test]
fn frozen_closed_row_resources_fail_in_cheap_preflight() {
    let defaults = FourLoopNextClosedRowsConfig::default();
    for (config, resource) in [
        (
            {
                let mut config = defaults;
                config.max_rows = FOUR_LOOP_NEXT_CLOSED_ROWS - 1;
                config
            },
            "closed parent rows",
        ),
        (
            {
                let mut config = defaults;
                config.max_paths = FOUR_LOOP_NEXT_CLOSED_ROWS_PATHS - 1;
                config
            },
            "path dispositions",
        ),
        (
            {
                let mut config = defaults;
                config.max_plan_bindings = FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_PLANS - 1;
                config
            },
            "plan bindings",
        ),
        (
            {
                let mut config = defaults;
                config.max_occurrence_bindings =
                    FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES - 1;
                config
            },
            "occurrence bindings",
        ),
        (
            {
                let mut config = defaults;
                config.max_boundary_groups = FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_BOUNDARY_GROUPS - 1;
                config
            },
            "raw boundary groups",
        ),
        (
            {
                let mut config = defaults;
                config.max_boundary_group_contributors =
                    FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES - 1;
                config
            },
            "boundary group contributors",
        ),
        (
            {
                let mut config = defaults;
                config.max_genuine_groups = FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_GROUPS - 1;
                config
            },
            "genuine row groups",
        ),
        (
            {
                let mut config = defaults;
                config.max_global_columns = FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMN_BOUND - 1;
                config
            },
            "global columns",
        ),
        (
            {
                let mut config = defaults;
                config.max_primary_contributions =
                    FOUR_LOOP_NEXT_CLOSED_ROWS_PRIMARY_CONTRIBUTION_BOUND - 1;
                config
            },
            "primary contributions",
        ),
        (
            {
                let mut config = defaults;
                config.max_raw_audit_contributions =
                    FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_AUDIT_CONTRIBUTION_BOUND - 1;
                config
            },
            "raw-audit contributions",
        ),
        (
            {
                let mut config = defaults;
                config.max_collected_entries = FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRY_BOUND - 1;
                config
            },
            "collected row entries",
        ),
        (
            {
                let mut config = defaults;
                config.max_row_width = FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMN_BOUND - 1;
                config
            },
            "closed row width",
        ),
    ] {
        assert_preflight_resource(config, resource);
    }

    let mut unrepresentable_degree = defaults;
    unrepresentable_degree.max_coefficient_degree =
        usize::try_from(SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT).unwrap() + 1;
    assert_preflight_resource(
        unrepresentable_degree,
        "configured coefficient exponent degree",
    );
}

// Keep the inventory, transport, both lower-loop slices, parent assembly,
// exact replay, and bounded tamper checks in one Symbolica-backed test process.
#[test]
fn exact_parent_rows_close_every_path_and_remain_replayable_over_qd() {
    let inventory = FourLoopNextInventory::build(FourLoopNextInventoryConfig::default()).unwrap();
    let transport =
        FourLoopComponentTransport::build(&inventory, FourLoopComponentTransportConfig::default())
            .unwrap();
    let t1s2 =
        FourLoopT1S2Closure::build(&transport, FourLoopT1S2ClosureConfig::default()).unwrap();
    let three_loop =
        FourLoopThreeLoopClosure::build(&transport, FourLoopThreeLoopClosureConfig::default())
            .unwrap();
    let retained_limit_failures: [(&str, fn(&mut FourLoopNextClosedRowsConfig)); 2] = [
        (
            "retained coefficient terms",
            |config: &mut FourLoopNextClosedRowsConfig| {
                config.max_retained_coefficient_terms = 0;
            },
        ),
        (
            "retained coefficient bytes",
            |config: &mut FourLoopNextClosedRowsConfig| {
                config.max_retained_coefficient_bytes = 0;
            },
        ),
    ];
    for (resource, configure) in retained_limit_failures {
        let mut config = FourLoopNextClosedRowsConfig::default();
        configure(&mut config);
        assert!(matches!(
            FourLoopNextClosedRows::build(&inventory, &transport, &t1s2, &three_loop, config),
            Err(FourLoopNextClosedRowsError::ResourceLimit { resource: actual, .. })
                if actual == resource
        ));
    }
    let closed = FourLoopNextClosedRows::build(
        &inventory,
        &transport,
        &t1s2,
        &three_loop,
        FourLoopNextClosedRowsConfig::default(),
    )
    .unwrap();
    let context = closed.coefficient_context();
    let one = context.one();
    assert_eq!(context.parameter_names(), ["d", "m2"]);
    assert_eq!(
        closed.status(),
        FourLoopNextClosedRowsStatus::ExactFixedSeedParentRowsGenericQdEliminationPending
    );
    assert_eq!(closed.inventory_schema(), FourLoopNextInventory::SCHEMA);
    assert_eq!(
        closed.transport_schema(),
        FourLoopComponentTransport::SCHEMA
    );
    assert_eq!(closed.t1s2_schema(), FourLoopT1S2Closure::SCHEMA);
    assert_eq!(closed.three_loop_schema(), FourLoopThreeLoopClosure::SCHEMA);
    assert_eq!(
        closed.inventory_seed_checksum(),
        inventory.manifest().seed_checksum()
    );
    assert_eq!(
        closed.transport_source_seed_checksum(),
        transport.source_seed_checksum()
    );
    assert_eq!(closed.t1s2_checksum(), t1s2.checksum());
    assert_eq!(closed.three_loop_checksum(), three_loop.checksum());

    // The two plan certificates are a disjoint, exhaustive partition of the
    // transport plans, and every retained binding resolves within its slice.
    let t1s2_leaf_ids = t1s2
        .plans()
        .iter()
        .map(|plan| plan.leaf_id())
        .collect::<BTreeSet<_>>();
    let three_loop_leaf_ids = three_loop
        .plans()
        .iter()
        .map(|plan| plan.leaf_id())
        .collect::<BTreeSet<_>>();
    let transport_leaf_ids = transport
        .plans()
        .iter()
        .map(|plan| plan.leaf_id())
        .collect::<BTreeSet<_>>();
    assert_eq!(t1s2_leaf_ids.len(), FOUR_LOOP_T1S2_CLOSURE_PLANS);
    assert_eq!(
        three_loop_leaf_ids.len(),
        FOUR_LOOP_THREE_LOOP_CLOSURE_PLANS
    );
    assert!(t1s2_leaf_ids.is_disjoint(&three_loop_leaf_ids));
    assert_eq!(
        t1s2_leaf_ids
            .union(&three_loop_leaf_ids)
            .copied()
            .collect::<BTreeSet<_>>(),
        transport_leaf_ids
    );
    assert_eq!(
        closed.plan_bindings().len(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_PLANS
    );
    let mut slice_plan_counts = BTreeMap::new();
    for (binding_index, binding) in closed.plan_bindings().iter().enumerate() {
        assert_eq!(binding.transport_plan_index() as usize, binding_index);
        assert_eq!(
            transport.plans()[binding.transport_plan_index() as usize].leaf_id(),
            binding.leaf_id()
        );
        let closure_leaf = match binding.slice() {
            FourLoopNextClosureSlice::T1S2 => {
                t1s2.plans()[binding.closure_plan_index() as usize].leaf_id()
            }
            FourLoopNextClosureSlice::ThreeLoop => {
                three_loop.plans()[binding.closure_plan_index() as usize].leaf_id()
            }
        };
        assert_eq!(closure_leaf, binding.leaf_id());
        *slice_plan_counts.entry(binding.slice()).or_insert(0_usize) += 1;
    }
    assert_eq!(
        slice_plan_counts[&FourLoopNextClosureSlice::T1S2],
        FOUR_LOOP_T1S2_CLOSURE_PLANS
    );
    assert_eq!(
        slice_plan_counts[&FourLoopNextClosureSlice::ThreeLoop],
        FOUR_LOOP_THREE_LOOP_CLOSURE_PLANS
    );

    // Both sibling occurrence arrays describe the same 4,230 sources. Zip
    // them with transport order and require an XOR completion at every slot.
    assert_eq!(
        closed.occurrence_bindings().len(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES
    );
    assert_eq!(t1s2.occurrences().len(), transport.occurrences().len());
    assert_eq!(
        three_loop.occurrences().len(),
        transport.occurrences().len()
    );
    let mut canceled_occurrences = 0_usize;
    for (index, ((source, t1), three)) in transport
        .occurrences()
        .iter()
        .zip(t1s2.occurrences())
        .zip(three_loop.occurrences())
        .enumerate()
    {
        assert_eq!(
            (source.row_index(), source.path_index(), source.leaf_id()),
            (t1.row_index(), t1.path_index(), t1.leaf_id())
        );
        assert_eq!(
            (source.row_index(), source.path_index(), source.leaf_id()),
            (three.row_index(), three.path_index(), three.leaf_id())
        );
        assert!(t1.completed_plan_index().is_some() ^ three.completed_plan_index().is_some());

        let occurrence = &closed.occurrence_bindings()[index];
        assert_eq!(occurrence.closure_occurrence_index() as usize, index);
        assert_eq!(occurrence.row_index(), source.row_index());
        assert_eq!(occurrence.path_index(), source.path_index());
        assert_eq!(occurrence.leaf_id(), source.leaf_id());
        assert_eq!(occurrence.transport_plan_index(), source.plan_index());
        assert_eq!(occurrence.plan_binding_index(), source.plan_index());
        let plan = &closed.plan_bindings()[occurrence.plan_binding_index() as usize];
        assert_eq!(plan.leaf_id(), source.leaf_id());
        assert_eq!(plan.slice(), occurrence.slice());
        match (t1.completed_plan_index(), three.completed_plan_index()) {
            (Some(completed), None) => {
                assert_eq!(occurrence.slice(), FourLoopNextClosureSlice::T1S2);
                assert_eq!(plan.closure_plan_index(), completed);
            }
            (None, Some(completed)) => {
                assert_eq!(occurrence.slice(), FourLoopNextClosureSlice::ThreeLoop);
                assert_eq!(plan.closure_plan_index(), completed);
            }
            _ => unreachable!(),
        }
        let group = &closed.boundary_groups()[occurrence.boundary_group_index() as usize];
        assert_eq!(group.row_index(), occurrence.row_index());
        assert_eq!(group.leaf_id(), occurrence.leaf_id());
        assert_eq!(group.plan_binding_index(), occurrence.plan_binding_index());
        assert!(
            group
                .contributor_path_indices()
                .contains(&occurrence.path_index())
        );
        canceled_occurrences += usize::from(group.canceled());
    }
    // Canceled paths still bind to real retained groups; no sentinel index is
    // permitted for the 16 paths in the eight canceled two-path groups.
    assert_eq!(canceled_occurrences, 16);

    // Independently recover all raw row/leaf groups without coefficient
    // replay and compare the exact retained group projection.
    let mut raw_boundary_groups = BTreeMap::<(u16, u32), Vec<u32>>::new();
    for (row_index, row) in inventory.rows().iter().enumerate() {
        for (path_index, path) in row.paths().iter().copied().enumerate() {
            if matches!(
                inventory.leaves()[path.leaf_id() as usize],
                FourLoopNextLeaf::Boundary(_)
            ) {
                raw_boundary_groups
                    .entry((row_index as u16, path.leaf_id()))
                    .or_default()
                    .push(path_index as u32);
            }
        }
    }
    assert_eq!(
        raw_boundary_groups.len(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_BOUNDARY_GROUPS
    );
    assert_eq!(
        closed
            .boundary_groups()
            .iter()
            .map(|group| (group.row_index(), group.leaf_id()))
            .collect::<BTreeSet<_>>(),
        raw_boundary_groups.keys().copied().collect::<BTreeSet<_>>()
    );
    let mut repeated_groups = 0_usize;
    let mut repeated_surviving_groups = 0_usize;
    let mut repeated_canceled_groups = 0_usize;
    let mut nonzero_groups = 0_usize;
    let mut canceled_groups = 0_usize;
    let mut nonzero_contributors = 0_usize;
    let mut all_group_contributors = 0_usize;
    for (group_index, group) in closed.boundary_groups().iter().enumerate() {
        let contributors = &raw_boundary_groups[&(group.row_index(), group.leaf_id())];
        assert_eq!(group.contributor_path_indices(), contributors);
        all_group_contributors += contributors.len();
        let row = &inventory.rows()[usize::from(group.row_index())];
        let seed_weight = row
            .raw_id()
            .seed()
            .powers()
            .iter()
            .map(|power| i64::from(*power))
            .sum::<i64>();
        let boundary_weight = inventory
            .boundary_key(group.leaf_id())
            .unwrap()
            .powers()
            .iter()
            .map(|power| i64::from(*power))
            .sum::<i64>();
        assert_eq!(group.seed_mass_weight(), seed_weight);
        assert_eq!(group.boundary_mass_weight(), boundary_weight);
        assert_eq!(group.mass_bridge_exponent(), seed_weight - boundary_weight);
        assert_eq!(
            group.seed_to_boundary_coefficient(),
            &apply_mass_power(
                context,
                group.collected_coefficient(),
                group.mass_bridge_exponent(),
            )
        );
        let retained = row
            .collected_boundaries()
            .iter()
            .find(|entry| entry.leaf_id() == group.leaf_id());
        if group.canceled() {
            canceled_groups += 1;
            assert!(group.collected_coefficient().is_zero());
            assert!(retained.is_none());
        } else {
            nonzero_groups += 1;
            nonzero_contributors += contributors.len();
            let retained = retained.unwrap();
            assert_eq!(retained.coefficient(), group.collected_coefficient());
            assert_eq!(retained.contributor_path_indices(), contributors);
        }
        if contributors.len() > 1 {
            repeated_groups += 1;
            assert_eq!(contributors.len(), 2);
            if group.canceled() {
                repeated_canceled_groups += 1;
            } else {
                repeated_surviving_groups += 1;
            }
        }
        assert_eq!(
            closed.plan_bindings()[group.plan_binding_index() as usize].leaf_id(),
            group.leaf_id()
        );
        for &path_index in contributors {
            let disposition = &closed.rows()[usize::from(group.row_index())].path_dispositions()
                [path_index as usize];
            assert!(matches!(
                disposition,
                FourLoopNextPathDisposition::Boundary {
                    boundary_group_index,
                    ..
                } if *boundary_group_index as usize == group_index
            ));
        }
    }
    assert_eq!(
        all_group_contributors,
        FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES
    );
    assert_eq!(
        nonzero_groups,
        FOUR_LOOP_NEXT_CLOSED_ROWS_NONZERO_BOUNDARY_GROUPS
    );
    assert_eq!(
        canceled_groups,
        FOUR_LOOP_NEXT_CLOSED_ROWS_CANCELED_BOUNDARY_GROUPS
    );
    assert_eq!(
        repeated_groups,
        FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_BOUNDARY_GROUPS
    );
    assert_eq!(
        repeated_surviving_groups,
        FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_SURVIVING_BOUNDARY_GROUPS
    );
    assert_eq!(
        repeated_canceled_groups,
        FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_CANCELED_BOUNDARY_GROUPS
    );
    assert_eq!(
        nonzero_contributors,
        FOUR_LOOP_NEXT_CLOSED_ROWS_NONZERO_BOUNDARY_CONTRIBUTORS
    );

    // Every inventory path has exactly one aligned typed disposition. Genuine
    // paths resolve to an authenticated inventory column; boundary paths
    // resolve to one real occurrence and raw-group binding.
    assert_eq!(closed.rows().len(), FOUR_LOOP_NEXT_CLOSED_ROWS);
    let mut disposition_counts = [0_usize; 4];
    let mut seen_occurrences = BTreeSet::new();
    for (row_index, (source, row)) in inventory.rows().iter().zip(closed.rows()).enumerate() {
        assert_eq!(row.raw_id(), source.raw_id());
        assert_eq!(row.path_dispositions().len(), source.paths().len());
        assert_eq!(
            row.seed_mass_weight(),
            source
                .raw_id()
                .seed()
                .powers()
                .iter()
                .map(|power| i64::from(*power))
                .sum::<i64>()
        );
        let expected_group_indices = closed
            .boundary_groups()
            .iter()
            .enumerate()
            .filter_map(|(index, group)| {
                (usize::from(group.row_index()) == row_index).then_some(index as u32)
            })
            .collect::<Vec<_>>();
        assert_eq!(row.boundary_group_indices(), expected_group_indices);
        for (path_index, (path, disposition)) in source
            .paths()
            .iter()
            .zip(row.path_dispositions())
            .enumerate()
        {
            assert_eq!(disposition.leaf_id(), path.leaf_id());
            match (&inventory.leaves()[path.leaf_id() as usize], disposition) {
                (
                    FourLoopNextLeaf::FamilyScaleless { .. },
                    FourLoopNextPathDisposition::FamilyScaleless { .. },
                ) => disposition_counts[0] += 1,
                (
                    FourLoopNextLeaf::ScalarCornerScaleless { .. },
                    FourLoopNextPathDisposition::ScalarCornerScaleless { .. },
                ) => disposition_counts[1] += 1,
                (
                    FourLoopNextLeaf::Genuine(genuine),
                    FourLoopNextPathDisposition::Genuine { column_index, .. },
                ) => {
                    disposition_counts[2] += 1;
                    assert_eq!(
                        closed.columns()[*column_index as usize],
                        FourLoopCornerColumnId::Genuine {
                            corner_type: genuine.corner_type(),
                            powers: *genuine.powers(),
                        }
                    );
                }
                (
                    FourLoopNextLeaf::Boundary(_),
                    FourLoopNextPathDisposition::Boundary {
                        occurrence_binding_index,
                        boundary_group_index,
                        ..
                    },
                ) => {
                    disposition_counts[3] += 1;
                    assert!(seen_occurrences.insert(*occurrence_binding_index));
                    let occurrence =
                        &closed.occurrence_bindings()[*occurrence_binding_index as usize];
                    assert_eq!(usize::from(occurrence.row_index()), row_index);
                    assert_eq!(occurrence.path_index() as usize, path_index);
                    assert_eq!(occurrence.leaf_id(), path.leaf_id());
                    assert_eq!(occurrence.boundary_group_index(), *boundary_group_index);
                }
                other => panic!("mismatched leaf/disposition pair: {other:?}"),
            }
        }
    }
    assert_eq!(disposition_counts[0] + disposition_counts[1], 0);
    assert_eq!(
        disposition_counts[2],
        FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_PATHS
    );
    assert_eq!(
        disposition_counts[3],
        FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES
    );
    assert_eq!(
        disposition_counts.iter().sum::<usize>(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_PATHS
    );
    assert_eq!(
        seen_occurrences.len(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES
    );

    // Exercise a genuine column reached by more than one raw path. The
    // production route groups raw coefficients before normalization; the
    // audit route normalizes each path independently. Both must reproduce the
    // stored pre-canonical coefficient after restoring the row scale.
    let mut repeated_genuine = None;
    'rows: for (row_index, row) in closed.rows().iter().enumerate() {
        let mut paths_by_column = BTreeMap::<u32, Vec<usize>>::new();
        for (path_index, disposition) in row.path_dispositions().iter().enumerate() {
            if let FourLoopNextPathDisposition::Genuine { column_index, .. } = disposition {
                paths_by_column
                    .entry(*column_index)
                    .or_default()
                    .push(path_index);
            }
        }
        for (column_index, path_indices) in paths_by_column {
            let column = &closed.columns()[column_index as usize];
            if path_indices.len() > 1 && row.coefficient(column).is_some() {
                repeated_genuine = Some((row_index, column_index, path_indices));
                break 'rows;
            }
        }
    }
    let (genuine_row_index, genuine_column_index, genuine_path_indices) = repeated_genuine
        .expect("the frozen fixture must include a surviving repeated genuine row/column group");
    let genuine_row = &closed.rows()[genuine_row_index];
    let genuine_column = &closed.columns()[genuine_column_index as usize];
    let genuine_exponent = genuine_row.seed_mass_weight() - genuine_column.mass_weight();
    let mut unnormalized_genuine_sum = context.zero();
    let mut direct_genuine_sum = context.zero();
    for path_index in genuine_path_indices {
        let coefficient = inventory
            .replay_path(genuine_row_index, path_index)
            .unwrap()
            .final_coefficient()
            .clone();
        unnormalized_genuine_sum = &unnormalized_genuine_sum + &coefficient;
        direct_genuine_sum =
            &direct_genuine_sum + &apply_mass_power(context, &coefficient, genuine_exponent);
    }
    let grouped_genuine_sum =
        apply_mass_power(context, &unnormalized_genuine_sum, genuine_exponent);
    assert_eq!(grouped_genuine_sum, direct_genuine_sum);
    assert_eq!(
        grouped_genuine_sum,
        genuine_row.coefficient(genuine_column).unwrap() * genuine_row.row_scale()
    );

    // The global column space is exactly the six allowed products plus every
    // authenticated genuine inventory column; no fingerprint-free synthetic
    // genuine column can enter through the public assembly surface.
    assert_eq!(
        closed.columns().len(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS
    );
    let actual_products = closed
        .columns()
        .iter()
        .filter_map(|column| match column {
            FourLoopCornerColumnId::Product(product) => Some(product.clone()),
            FourLoopCornerColumnId::Genuine { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    let expected_products = BTreeSet::from([
        master_product(&[
            MassiveVacuumMaster::T1,
            MassiveVacuumMaster::T1,
            MassiveVacuumMaster::T1,
            MassiveVacuumMaster::T1,
        ]),
        master_product(&[
            MassiveVacuumMaster::T1,
            MassiveVacuumMaster::T1,
            MassiveVacuumMaster::S2,
        ]),
        master_product(&[MassiveVacuumMaster::S2, MassiveVacuumMaster::S2]),
        master_product(&[MassiveVacuumMaster::T1, MassiveVacuumMaster::B4]),
        master_product(&[MassiveVacuumMaster::T1, MassiveVacuumMaster::F5]),
        master_product(&[MassiveVacuumMaster::T1, MassiveVacuumMaster::M6]),
    ]);
    assert_eq!(actual_products, expected_products);
    assert_eq!(
        actual_products.len(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_PRODUCT_COLUMNS
    );
    let actual_genuine = closed
        .columns()
        .iter()
        .filter(|column| matches!(column, FourLoopCornerColumnId::Genuine { .. }))
        .cloned()
        .collect::<BTreeSet<_>>();
    let inventory_genuine = inventory
        .leaves()
        .iter()
        .filter_map(|leaf| match leaf {
            FourLoopNextLeaf::Genuine(genuine) => {
                assert_eq!(
                    genuine.topology(),
                    genuine.corner_type().reference_topology()
                );
                assert!(!genuine.family_fingerprint().is_empty());
                Some(FourLoopCornerColumnId::Genuine {
                    corner_type: genuine.corner_type(),
                    powers: *genuine.powers(),
                })
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_genuine, inventory_genuine);
    assert_eq!(
        actual_genuine.len(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_COLUMNS
    );

    // All stored coefficients are literal rational functions of d. Nonzero
    // rows have a unit coefficient in their hardest retained column.
    let column_positions = closed
        .columns()
        .iter()
        .enumerate()
        .map(|(index, column)| (column, index))
        .collect::<BTreeMap<_, _>>();
    for row in closed.rows() {
        assert_eq!(row.row_scale().numerator.degree(1), 0);
        assert_eq!(row.row_scale().denominator.degree(1), 0);
        assert!(row.entries().values().all(|coefficient| {
            !coefficient.is_zero()
                && coefficient.numerator.degree(1) == 0
                && coefficient.denominator.degree(1) == 0
        }));
        if let Some((hardest, coefficient)) = row.entries().last_key_value() {
            assert_eq!(coefficient, &one);
            assert_eq!(row.pivot(), Some(hardest));
            let pivot_index = row.pivot_column_index().unwrap() as usize;
            assert_eq!(&closed.columns()[pivot_index], hardest);
            assert_eq!(column_positions[hardest], pivot_index);
            assert_eq!(row.coefficient(hardest), Some(&one));
            assert!(!row.row_scale().is_zero());
        } else {
            assert_eq!(row.pivot(), None);
            assert_eq!(row.pivot_column_index(), None);
            assert_eq!(row.row_scale(), &one);
        }
    }

    let stats = closed.stats();
    assert_eq!(stats.rows(), FOUR_LOOP_NEXT_CLOSED_ROWS);
    assert_eq!(stats.paths(), FOUR_LOOP_NEXT_CLOSED_ROWS_PATHS);
    assert_eq!(
        stats.boundary_paths(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES
    );
    assert_eq!(
        stats.genuine_paths(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_PATHS
    );
    assert_eq!(stats.scaleless_paths(), 0);
    assert_eq!(
        stats.plan_bindings(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_PLANS
    );
    assert_eq!(
        stats.occurrence_bindings(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES
    );
    assert_eq!(
        stats.raw_boundary_groups(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_BOUNDARY_GROUPS
    );
    assert_eq!(
        stats.nonzero_boundary_groups(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_NONZERO_BOUNDARY_GROUPS
    );
    assert_eq!(
        stats.canceled_boundary_groups(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_CANCELED_BOUNDARY_GROUPS
    );
    assert_eq!(
        stats.repeated_boundary_groups(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_BOUNDARY_GROUPS
    );
    assert_eq!(
        stats.repeated_surviving_boundary_groups(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_SURVIVING_BOUNDARY_GROUPS
    );
    assert_eq!(
        stats.repeated_canceled_boundary_groups(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_CANCELED_BOUNDARY_GROUPS
    );
    assert_eq!(
        stats.nonzero_boundary_contributors(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_NONZERO_BOUNDARY_CONTRIBUTORS
    );
    assert_eq!(
        stats.genuine_groups(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_GROUPS
    );
    assert_eq!(
        stats.genuine_columns(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_COLUMNS
    );
    assert_eq!(
        stats.product_columns(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_PRODUCT_COLUMNS
    );
    assert_eq!(
        stats.global_columns(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS
    );
    assert_eq!(
        stats.max_row_paths(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_PATHS
    );
    assert_eq!(
        stats.max_row_boundary_groups(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_BOUNDARY_GROUPS
    );
    assert_eq!(
        stats.primary_contributions(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_PRIMARY_CONTRIBUTIONS
    );
    assert_eq!(
        stats.raw_audit_contributions(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_AUDIT_CONTRIBUTIONS
    );
    assert_eq!(
        stats.collected_entries(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES
    );
    assert_eq!(stats.zero_rows(), FOUR_LOOP_NEXT_CLOSED_ROWS_ZERO_ROWS);
    assert_eq!(
        stats.max_row_width(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_WIDTH
    );
    assert_eq!(
        stats.mass_power_steps(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_MASS_POWER_STEPS
    );
    assert_eq!(
        stats.coefficient_multiplications(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_COEFFICIENT_MULTIPLICATIONS
    );
    assert_eq!(
        stats.coefficient_additions(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_COEFFICIENT_ADDITIONS
    );
    assert_eq!(
        stats.coefficient_divisions(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_COEFFICIENT_DIVISIONS
    );
    assert_eq!(
        stats.coefficient_operations(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_COEFFICIENT_MULTIPLICATIONS
            + FOUR_LOOP_NEXT_CLOSED_ROWS_COEFFICIENT_ADDITIONS
            + FOUR_LOOP_NEXT_CLOSED_ROWS_COEFFICIENT_DIVISIONS
    );
    assert_eq!(
        stats.retained_coefficient_terms(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_RETAINED_COEFFICIENT_TERMS
    );
    assert_eq!(
        stats.retained_coefficient_bytes(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_RETAINED_COEFFICIENT_BYTES
    );
    assert_eq!(closed.checksum(), FOUR_LOOP_NEXT_CLOSED_ROWS_CHECKSUM);
    assert!(stats.primary_contributions() <= FOUR_LOOP_NEXT_CLOSED_ROWS_PRIMARY_CONTRIBUTION_BOUND);
    assert!(
        stats.raw_audit_contributions() <= FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_AUDIT_CONTRIBUTION_BOUND
    );
    assert!(stats.collected_entries() <= FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRY_BOUND);
    let limits = closed.config();
    assert!(stats.max_row_width() <= limits.max_row_width);
    assert!(stats.mass_power_steps() <= limits.max_mass_power_steps);
    assert!(stats.coefficient_operations() <= limits.max_coefficient_operations);
    assert!(stats.coefficient_multiplications() <= limits.max_coefficient_multiplications);
    assert!(stats.coefficient_additions() <= limits.max_coefficient_additions);
    assert!(stats.coefficient_divisions() <= limits.max_coefficient_divisions);
    assert!(stats.retained_coefficient_terms() <= limits.max_retained_coefficient_terms);
    assert!(stats.retained_coefficient_bytes() <= limits.max_retained_coefficient_bytes);
    eprintln!(
        "four-loop next closed rows stats: {stats:#?}\nchecksum: {:#018x}",
        closed.checksum()
    );

    // Replay only 18 raw paths outside construction: one repeated survivor
    // proves grouped/normalized equation (2.1), while all 16 paths in the
    // eight canceled groups independently prove exact row-local zero sums.
    let repeated_survivor_index = closed
        .boundary_groups()
        .iter()
        .position(|group| !group.canceled() && group.contributor_path_indices().len() == 2)
        .unwrap();
    let canceled_indices = closed
        .boundary_groups()
        .iter()
        .enumerate()
        .filter_map(|(index, group)| group.canceled().then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(
        canceled_indices.len(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_CANCELED_BOUNDARY_GROUPS
    );
    let repeated_survivor = &closed.boundary_groups()[repeated_survivor_index];
    let survivor_coefficients = repeated_survivor
        .contributor_path_indices()
        .iter()
        .map(|&path_index| {
            inventory
                .replay_path(
                    usize::from(repeated_survivor.row_index()),
                    path_index as usize,
                )
                .unwrap()
                .final_coefficient()
                .clone()
        })
        .collect::<Vec<_>>();
    let mut raw_sum = context.zero();
    for coefficient in &survivor_coefficients {
        raw_sum = &raw_sum + coefficient;
    }
    assert_eq!(&raw_sum, repeated_survivor.collected_coefficient());
    let binding = &closed.plan_bindings()[repeated_survivor.plan_binding_index() as usize];
    let (ordinary, normalized) = match binding.slice() {
        FourLoopNextClosureSlice::T1S2 => {
            let plan = &t1s2.plans()[binding.closure_plan_index() as usize];
            (plan.ordinary(), plan.mass_normalized())
        }
        FourLoopNextClosureSlice::ThreeLoop => {
            let plan = &three_loop.plans()[binding.closure_plan_index() as usize];
            (plan.ordinary(), plan.mass_normalized())
        }
    };
    let mut raw_route = BTreeMap::new();
    for path_coefficient in &survivor_coefficients {
        for (product, closure_coefficient) in ordinary.terms() {
            let multiplied = path_coefficient * closure_coefficient;
            let coefficient = apply_mass_power(
                context,
                &multiplied,
                repeated_survivor.seed_mass_weight() - product_mass_weight(product),
            );
            add_product_term(&mut raw_route, product.clone(), coefficient);
        }
    }
    let mut grouped_route = BTreeMap::new();
    for (product, closure_coefficient) in normalized.terms() {
        add_product_term(
            &mut grouped_route,
            product.clone(),
            repeated_survivor.seed_to_boundary_coefficient() * closure_coefficient,
        );
    }
    assert_eq!(grouped_route, raw_route);
    assert!(grouped_route.values().all(|coefficient| {
        coefficient.numerator.degree(1) == 0 && coefficient.denominator.degree(1) == 0
    }));

    for &canceled_index in &canceled_indices {
        let canceled = &closed.boundary_groups()[canceled_index];
        let mut canceled_sum = context.zero();
        for &path_index in canceled.contributor_path_indices() {
            let replayed = inventory
                .replay_path(usize::from(canceled.row_index()), path_index as usize)
                .unwrap();
            canceled_sum = &canceled_sum + replayed.final_coefficient();
        }
        assert!(canceled_sum.is_zero());
        assert_eq!(&canceled_sum, canceled.collected_coefficient());
    }
    let canceled_index = canceled_indices[0];
    let canceled = &closed.boundary_groups()[canceled_index];

    // Public replay is deliberately invoked only once: it authenticates the
    // exhaustive raw-vs-group reconstruction already performed by the owner.
    closed.replay().unwrap();

    // Bounded candidate helpers first accept independently reconstructed source
    // records, then reject representative provenance and exact coefficient
    // corruption without cloning the full certificate per case.
    let plan = &closed.plan_bindings()[0];
    let occurrence = &closed.occurrence_bindings()[0];
    let (row_index, row) = closed
        .rows()
        .iter()
        .enumerate()
        .find(|(_, row)| !row.entries().is_empty())
        .unwrap();
    closed.replay_plan_binding_candidate(0, plan).unwrap();
    closed
        .replay_occurrence_binding_candidate(0, occurrence)
        .unwrap();
    closed
        .replay_boundary_group_candidate(repeated_survivor_index, repeated_survivor)
        .unwrap();
    closed
        .replay_boundary_group_candidate(canceled_index, canceled)
        .unwrap();
    closed.replay_row_candidate(row_index, row).unwrap();

    let wrong_slice = match plan.slice() {
        FourLoopNextClosureSlice::T1S2 => FourLoopNextClosureSlice::ThreeLoop,
        FourLoopNextClosureSlice::ThreeLoop => FourLoopNextClosureSlice::T1S2,
    };
    assert!(
        closed
            .replay_plan_binding_candidate(0, &plan.with_slice_for_replay(wrong_slice))
            .is_err()
    );
    assert!(
        closed
            .replay_occurrence_binding_candidate(0, &occurrence.with_leaf_id_for_replay(u32::MAX),)
            .is_err()
    );
    assert!(
        closed
            .replay_occurrence_binding_candidate(
                0,
                &occurrence.with_boundary_group_index_for_replay(u32::MAX),
            )
            .is_err()
    );

    let mut deleted = repeated_survivor.contributor_path_indices().to_vec();
    deleted.pop();
    assert!(
        closed
            .replay_boundary_group_candidate(
                repeated_survivor_index,
                &repeated_survivor.with_contributor_path_indices_for_replay(deleted),
            )
            .is_err()
    );
    let mut duplicated = repeated_survivor.contributor_path_indices().to_vec();
    duplicated.push(duplicated[0]);
    assert!(
        closed
            .replay_boundary_group_candidate(
                repeated_survivor_index,
                &repeated_survivor.with_contributor_path_indices_for_replay(duplicated),
            )
            .is_err()
    );
    assert!(
        closed
            .replay_boundary_group_candidate(
                canceled_index,
                &canceled.with_collected_coefficient_for_replay(one.clone()),
            )
            .is_err()
    );
    assert!(
        closed
            .replay_boundary_group_candidate(
                repeated_survivor_index,
                &repeated_survivor.with_mass_bridge_exponent_for_replay(
                    repeated_survivor.mass_bridge_exponent() + 1,
                ),
            )
            .is_err()
    );
    assert!(
        closed
            .replay_boundary_group_candidate(
                repeated_survivor_index,
                &repeated_survivor.with_seed_to_boundary_coefficient_for_replay(
                    repeated_survivor.seed_to_boundary_coefficient() + &one,
                ),
            )
            .is_err()
    );

    let changed_scale = row.row_scale() + &one;
    assert!(
        closed
            .replay_row_candidate(row_index, &row.with_row_scale_for_replay(changed_scale))
            .is_err()
    );
    let (column, coefficient) = row.entries().first_key_value().unwrap();
    let residual_mass = coefficient * &context.parameter("m2").unwrap();
    assert!(
        closed
            .replay_row_candidate(
                row_index,
                &row.with_coefficient_for_replay(column.clone(), residual_mass),
            )
            .is_err()
    );
    let seventh_product =
        FourLoopCornerColumnId::Product(MasterProduct::from_factor(MassiveVacuumMaster::B4));
    assert!(
        closed
            .replay_row_candidate(
                row_index,
                &row.with_coefficient_for_replay(seventh_product, one),
            )
            .is_err()
    );
}
