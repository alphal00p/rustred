use std::collections::{BTreeMap, BTreeSet};

use rustred::SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT;
use rustred_legacy_oracles::four_loop_next_corner_cross_auth as corner_cross;
use rustred_legacy_oracles::four_loop_next_elimination as elimination;
use rustred_legacy_oracles::four_loop_next_modular_rank as modular;
use rustred_legacy_oracles::{
    FOUR_LOOP_NEXT_CLOSED_ROWS, FOUR_LOOP_NEXT_CLOSED_ROWS_CHECKSUM,
    FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES, FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS,
    FourLoopComponentTransport, FourLoopComponentTransportConfig, FourLoopCornerShellCertificate,
    FourLoopCornerShellConfig, FourLoopNextClosedRows, FourLoopNextClosedRowsConfig,
    FourLoopNextCornerCrossAuth, FourLoopNextCornerCrossAuthStatus, FourLoopNextElimination,
    FourLoopNextEliminationConditionStatus, FourLoopNextEliminationConfig,
    FourLoopNextEliminationError, FourLoopNextEliminationStatus, FourLoopNextInventory,
    FourLoopNextInventoryConfig, FourLoopT1S2Closure, FourLoopT1S2ClosureConfig,
    FourLoopThreeLoopClosure, FourLoopThreeLoopClosureConfig,
};

fn assert_preflight_resource(
    config: FourLoopNextEliminationConfig,
    expected_resource: &'static str,
    expected_requested: usize,
    expected_limit: usize,
) {
    assert!(matches!(
        FourLoopNextElimination::preflight_config(config),
        Err(FourLoopNextEliminationError::ResourceLimit {
            resource,
            requested,
            limit,
        }) if resource == expected_resource
            && requested == expected_requested as u128
            && limit == expected_limit as u128
    ));
}

#[test]
fn frozen_elimination_resources_fail_in_cheap_preflight() {
    let defaults = FourLoopNextEliminationConfig::default();
    FourLoopNextElimination::preflight_config(defaults).unwrap();

    let mut config = defaults;
    config.exact.max_rows = FOUR_LOOP_NEXT_CLOSED_ROWS - 1;
    assert_preflight_resource(
        config,
        "configured exact source rows",
        FOUR_LOOP_NEXT_CLOSED_ROWS,
        FOUR_LOOP_NEXT_CLOSED_ROWS - 1,
    );

    let mut config = defaults;
    config.exact.max_columns = FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS - 1;
    assert_preflight_resource(
        config,
        "configured exact columns",
        FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS,
        FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS - 1,
    );

    let mut config = defaults;
    config.exact.max_input_entries = FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES - 1;
    assert_preflight_resource(
        config,
        "configured exact input entries",
        FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES,
        FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES - 1,
    );

    let mut config = defaults;
    config.exact.max_input_coefficient_bytes = FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES - 1;
    assert_preflight_resource(
        config,
        "configured exact input coefficient bytes",
        FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES,
        FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES - 1,
    );

    let mut config = defaults;
    config.modular.max_images = modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_IMAGES.len() - 1;
    assert_preflight_resource(
        config,
        "configured modular images",
        modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_IMAGES.len(),
        modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_IMAGES.len() - 1,
    );

    let mut config = defaults;
    config.modular.max_initial_nonzeros = FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES - 1;
    assert_preflight_resource(
        config,
        "configured modular input entries",
        FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES,
        FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES - 1,
    );

    let mut config = defaults;
    config.exact.max_coefficient_degree =
        usize::try_from(SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT).unwrap() + 1;
    assert_preflight_resource(
        config,
        "configured exact coefficient exponent degree",
        config.exact.max_coefficient_degree,
        usize::try_from(SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT).unwrap(),
    );

    let cheap_limit_failures: [(
        &'static str,
        fn(&mut FourLoopNextEliminationConfig),
        usize,
        usize,
    ); 11] = [
        (
            "configured exact arithmetic updates",
            |config: &mut FourLoopNextEliminationConfig| config.exact.max_updates = 0,
            1,
            0,
        ),
        (
            "configured exact retained entries",
            |config: &mut FourLoopNextEliminationConfig| config.exact.max_retained_entries = 1,
            2,
            1,
        ),
        (
            "configured exact retained coefficient terms",
            |config: &mut FourLoopNextEliminationConfig| {
                config.exact.max_retained_coefficient_terms = 3
            },
            4,
            3,
        ),
        (
            "configured exact retained coefficient bytes",
            |config: &mut FourLoopNextEliminationConfig| {
                config.exact.max_retained_coefficient_bytes = 1
            },
            2,
            1,
        ),
        (
            "configured exact coefficient operation terms",
            |config: &mut FourLoopNextEliminationConfig| {
                config.exact.max_coefficient_operation_terms = 0
            },
            1,
            0,
        ),
        (
            "configured exact coefficient dense terms",
            |config: &mut FourLoopNextEliminationConfig| {
                config.exact.max_coefficient_dense_terms = 0
            },
            1,
            0,
        ),
        (
            "configured exact replay reductions",
            |config: &mut FourLoopNextEliminationConfig| {
                config.exact.max_replay_reductions = FOUR_LOOP_NEXT_CLOSED_ROWS - 1
            },
            FOUR_LOOP_NEXT_CLOSED_ROWS,
            FOUR_LOOP_NEXT_CLOSED_ROWS - 1,
        ),
        (
            "configured exact replay updates",
            |config: &mut FourLoopNextEliminationConfig| {
                config.exact.max_replay_updates = FOUR_LOOP_NEXT_CLOSED_ROWS - 1
            },
            FOUR_LOOP_NEXT_CLOSED_ROWS,
            FOUR_LOOP_NEXT_CLOSED_ROWS - 1,
        ),
        (
            "configured projected pivots",
            |config: &mut FourLoopNextEliminationConfig| config.max_projected_pivots = 0,
            1,
            0,
        ),
        (
            "configured projected coefficient terms",
            |config: &mut FourLoopNextEliminationConfig| config.max_projected_coefficient_terms = 1,
            2,
            1,
        ),
        (
            "configured projected coefficient bytes",
            |config: &mut FourLoopNextEliminationConfig| config.max_projected_coefficient_bytes = 0,
            1,
            0,
        ),
    ];
    for (resource, configure, requested, limit) in cheap_limit_failures {
        let mut config = defaults;
        configure(&mut config);
        assert_preflight_resource(config, resource, requested, limit);
    }
}

// Keep construction of the complete dependency graph, native corner
// certificate, all frozen assertions, and the one public composed replay in a
// single expensive acceptance test. The licensed suite may execute this test
// binary concurrently with other binaries; this test itself deliberately
// performs one composed replay rather than rebuilding every component.
#[test]
fn exact_fixed_seed_shell_is_frozen_and_composed_replay_succeeds() {
    let inventory = FourLoopNextInventory::build(FourLoopNextInventoryConfig::default()).unwrap();
    let transport =
        FourLoopComponentTransport::build(&inventory, FourLoopComponentTransportConfig::default())
            .unwrap();
    let t1s2 =
        FourLoopT1S2Closure::build(&transport, FourLoopT1S2ClosureConfig::default()).unwrap();
    let three_loop =
        FourLoopThreeLoopClosure::build(&transport, FourLoopThreeLoopClosureConfig::default())
            .unwrap();
    let closed = FourLoopNextClosedRows::build(
        &inventory,
        &transport,
        &t1s2,
        &three_loop,
        FourLoopNextClosedRowsConfig::default(),
    )
    .unwrap();
    let default_config = FourLoopNextEliminationConfig::default();
    let certificate = FourLoopNextElimination::build(&closed, default_config).unwrap();
    let corner =
        FourLoopCornerShellCertificate::build(FourLoopCornerShellConfig::default()).unwrap();
    let cross_auth = FourLoopNextCornerCrossAuth::compose(&corner, &certificate).unwrap();

    assert_eq!(
        certificate.status(),
        FourLoopNextEliminationStatus::CompleteFixedSeedShell
    );
    assert_eq!(certificate.config(), &default_config);
    assert_eq!(certificate.exact_engine().config(), &default_config.exact);
    assert_eq!(certificate.proof_domain(), "Q(d)");
    assert_eq!(certificate.coefficient_context().parameter_names(), ["d"]);
    assert_eq!(
        certificate.source_checksum(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_CHECKSUM
    );
    assert_eq!(
        certificate.source_checksum(),
        certificate.closed_rows().checksum()
    );
    assert_eq!(
        certificate.columns().len(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_COLUMNS
    );
    assert_eq!(
        certificate.rank(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_RANK
    );
    assert_eq!(
        certificate.free_unresolved_columns().len(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_FREE_UNRESOLVED_COLUMNS
    );
    assert_eq!(
        certificate.rank() + certificate.free_unresolved_columns().len(),
        certificate.columns().len()
    );
    assert_eq!(
        certificate.exact_engine().source_checksum(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_PROJECTED_SOURCE_CHECKSUM
    );
    assert_eq!(
        certificate.exact_engine().checksum(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_CHECKSUM
    );
    assert_eq!(
        certificate.checksum(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_CHECKSUM
    );

    let modular_report = certificate.modular_discovery();
    assert_eq!(
        modular_report.source_checksum(),
        FOUR_LOOP_NEXT_CLOSED_ROWS_CHECKSUM
    );
    assert_eq!(
        modular_report.column_catalog_checksum(),
        modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_COLUMN_CATALOG_CHECKSUM
    );
    assert_eq!(
        modular_report.checksum(),
        modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_CHECKSUM
    );
    assert_eq!(
        modular_report.common_modular_rank(),
        Some(modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_RANK)
    );
    assert!(modular_report.ranks_agree());
    assert!(modular_report.pivot_columns_agree());
    assert!(modular_report.pivot_skeletons_agree());
    assert_eq!(
        modular_report.images().len(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_MODULAR_IMAGES
    );
    let first_modular_pivots = modular_report.images()[0].pivots();
    for (index, image) in modular_report.images().iter().enumerate() {
        assert_eq!(
            image.image(),
            modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_IMAGES[index]
        );
        assert_eq!(image.rank(), modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_RANK);
        assert_eq!(
            image.free_columns(),
            modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_FREE_COLUMNS
        );
        assert_eq!(
            image.matrix_checksum(),
            modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_MATRIX_CHECKSUMS[index]
        );
        assert_eq!(
            image.pivot_checksum(),
            modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_PIVOT_CHECKSUMS[index]
        );
        assert_eq!(image.pivots().len(), first_modular_pivots.len());
        assert!(
            image
                .pivots()
                .iter()
                .zip(first_modular_pivots)
                .all(|(actual, expected)| {
                    (
                        actual.step(),
                        actual.source_row_index(),
                        actual.column_index(),
                    ) == (
                        expected.step(),
                        expected.source_row_index(),
                        expected.column_index(),
                    )
                })
        );
        let fill = image.fill();
        assert_eq!(
            fill.source_nonzeros(),
            elimination::FOUR_LOOP_NEXT_ELIMINATION_INPUT_ENTRIES
        );
        assert_eq!(
            fill.initial_nonzeros(),
            elimination::FOUR_LOOP_NEXT_ELIMINATION_INPUT_ENTRIES
        );
        assert_eq!(fill.evaluated_zero_coefficients(), 0);
        assert_eq!(
            fill.peak_live_nonzeros(),
            modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_PEAK_LIVE_NONZEROS
        );
        assert_eq!(
            fill.peak_row_nonzeros(),
            modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_PEAK_ROW_NONZEROS
        );
        assert_eq!(
            fill.cumulative_fill_in(),
            modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_FILL_IN
        );
        assert_eq!(
            fill.cancellations(),
            modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_CANCELLATIONS
        );
        assert_eq!(
            fill.cleared_pivot_entries(),
            modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_CLEARED_PIVOTS
        );
        assert_eq!(
            fill.elimination_updates(),
            modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_FIELD_WORK_UNITS
        );
        assert_eq!(
            fill.dependent_rows(),
            modular::FOUR_LOOP_NEXT_MODULAR_DISCOVERY_DEPENDENT_ROWS
        );
    }

    let exact = certificate.exact_engine();
    assert_eq!(exact.coefficient_context().parameter_names(), ["d"]);
    assert_eq!(
        exact.source_row_count(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_SOURCE_ROWS
    );
    assert_eq!(
        exact.column_count(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_COLUMNS
    );
    assert_eq!(exact.rank(), elimination::FOUR_LOOP_NEXT_ELIMINATION_RANK);
    assert_eq!(
        exact.free_columns().len(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_FREE_UNRESOLVED_COLUMNS
    );
    let exact_stats = exact.stats();
    assert_eq!(
        exact_stats.source_rows(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_SOURCE_ROWS
    );
    assert_eq!(
        exact_stats.columns(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_COLUMNS
    );
    assert_eq!(
        exact_stats.input_entries(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_INPUT_ENTRIES
    );
    assert_eq!(
        exact_stats.rank(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_RANK
    );
    assert_eq!(
        exact_stats.free_columns(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_FREE_UNRESOLVED_COLUMNS
    );
    assert_eq!(
        exact_stats.pivot_reductions(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_PIVOT_REDUCTIONS
    );
    assert_eq!(
        exact_stats.verification_reductions(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_VERIFICATION_REDUCTIONS
    );
    assert_eq!(
        exact_stats.arithmetic_updates(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_ARITHMETIC_UPDATES
    );
    assert_eq!(
        exact_stats.retained_entries(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_RETAINED_ENTRIES
    );
    assert_eq!(
        exact_stats.retained_coefficient_terms(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_RETAINED_COEFFICIENT_TERMS
    );
    assert_eq!(
        exact_stats.retained_coefficient_bytes(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_RETAINED_COEFFICIENT_BYTES
    );
    assert_eq!(
        exact_stats.maximum_row_width(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_MAXIMUM_ROW_WIDTH
    );
    assert_eq!(
        exact_stats.maximum_coefficient_degree(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_MAXIMUM_COEFFICIENT_DEGREE
    );
    assert_eq!(
        exact_stats.replay_reductions(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_REPLAY_REDUCTIONS
    );
    assert_eq!(
        exact_stats.replay_updates(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_REPLAY_UPDATES
    );

    let stats = certificate.stats();
    assert_eq!(
        stats.source_rows(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_SOURCE_ROWS
    );
    assert_eq!(
        stats.columns(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_COLUMNS
    );
    assert_eq!(
        stats.input_entries(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_INPUT_ENTRIES
    );
    assert_eq!(
        stats.maximum_input_row_width(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_MAXIMUM_INPUT_ROW_WIDTH
    );
    assert_eq!(
        stats.modular_images(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_MODULAR_IMAGES
    );
    assert_eq!(
        stats.modular_candidate_rank(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_MODULAR_CANDIDATE_RANK
    );
    assert_eq!(
        stats.exact_rank(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_RANK
    );
    assert_eq!(
        stats.pivot_rules(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_PIVOT_RULES
    );
    assert_eq!(
        stats.free_unresolved_columns(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_FREE_UNRESOLVED_COLUMNS
    );
    assert_eq!(
        stats.projected_rhs_entries(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_PROJECTED_RHS_ENTRIES
    );
    assert_eq!(
        stats.trace_reductions(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_TRACE_REDUCTIONS
    );
    assert_eq!(
        stats.maximum_trace_reductions(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_MAXIMUM_TRACE_REDUCTIONS
    );
    assert_eq!(
        stats.projected_coefficient_terms(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_PROJECTED_COEFFICIENT_TERMS
    );
    assert_eq!(
        stats.projected_coefficient_bytes(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_PROJECTED_COEFFICIENT_BYTES
    );
    assert_eq!(
        stats.exact_pivot_reductions(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_PIVOT_REDUCTIONS
    );
    assert_eq!(
        stats.exact_verification_reductions(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_VERIFICATION_REDUCTIONS
    );
    assert_eq!(
        stats.exact_arithmetic_updates(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_ARITHMETIC_UPDATES
    );
    assert_eq!(
        stats.exact_retained_entries(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_RETAINED_ENTRIES
    );
    assert_eq!(
        stats.exact_retained_coefficient_terms(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_RETAINED_COEFFICIENT_TERMS
    );
    assert_eq!(
        stats.exact_retained_coefficient_bytes(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_RETAINED_COEFFICIENT_BYTES
    );
    assert_eq!(
        stats.exact_maximum_row_width(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_MAXIMUM_ROW_WIDTH
    );
    assert_eq!(
        stats.exact_maximum_coefficient_degree(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_MAXIMUM_COEFFICIENT_DEGREE
    );
    assert_eq!(
        stats.exact_replay_reductions(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_REPLAY_REDUCTIONS
    );
    assert_eq!(
        stats.exact_replay_updates(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_EXACT_REPLAY_UPDATES
    );
    assert_eq!(
        stats.conservative_condition_slots(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_CONSERVATIVE_CONDITION_SLOTS
    );

    let conditions = certificate.conditions();
    assert_eq!(
        conditions.status(),
        FourLoopNextEliminationConditionStatus::ConservativeUnfactoredInversionSlotCensusOnly
    );
    assert!(!conditions.is_complete_exceptional_dimension_inventory());
    assert_eq!(
        conditions.parent_row_scale_slots(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_PARENT_ROW_SCALE_SLOTS
    );
    assert_eq!(
        conditions.parent_coefficient_denominator_slots(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_PARENT_COEFFICIENT_DENOMINATOR_SLOTS
    );
    assert_eq!(
        conditions.trace_divisor_slots(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_TRACE_DIVISOR_SLOTS
    );
    assert_eq!(
        conditions.trace_factor_denominator_slots(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_TRACE_FACTOR_DENOMINATOR_SLOTS
    );
    assert_eq!(
        conditions.rule_rhs_denominator_slots(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_RULE_RHS_DENOMINATOR_SLOTS
    );
    assert_eq!(
        conditions.total_slots(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_CONSERVATIVE_CONDITION_SLOTS
    );

    assert!(
        certificate
            .columns()
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
    assert!(
        exact
            .pivot_rules()
            .windows(2)
            .all(|pair| pair[0].pivot_column() > pair[1].pivot_column())
    );
    assert!(
        certificate
            .pivots()
            .windows(2)
            .all(|pair| pair[0].pivot() > pair[1].pivot())
    );
    assert!(
        exact
            .free_columns()
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );

    let one = certificate.coefficient_context().one();
    for (ordinal, ((typed, indexed), proposed)) in certificate
        .pivots()
        .iter()
        .zip(exact.pivot_rules())
        .zip(first_modular_pivots)
        .enumerate()
    {
        assert_eq!(typed.ordinal(), ordinal);
        assert_eq!(indexed.ordinal(), ordinal);
        assert_eq!(proposed.step(), ordinal);
        assert_eq!(typed.source_row_index(), proposed.source_row_index());
        assert_eq!(indexed.source_row_index(), proposed.source_row_index());
        assert_eq!(indexed.pivot_column(), proposed.column_index());
        assert_eq!(
            typed.pivot(),
            &certificate.columns()[proposed.column_index()]
        );
        assert_eq!(
            typed.source_raw_id(),
            closed.rows()[typed.source_row_index()].raw_id()
        );
        assert_eq!(
            typed.trace().base_source_row_index(),
            typed.source_row_index()
        );
        assert_eq!(
            typed.trace().base_source_row_index(),
            indexed.source_row_index()
        );
        assert_eq!(
            indexed.trace().base_source_row_index(),
            indexed.source_row_index()
        );
        assert_eq!(
            typed.trace().base_source_raw_id(),
            closed.rows()[typed.trace().base_source_row_index()].raw_id()
        );
        assert_eq!(certificate.pivot_rule(typed.pivot()), Some(typed));
        assert_eq!(indexed.row().get(&indexed.pivot_column()), Some(&one));
        assert_eq!(
            typed.trace().base_source_row_index(),
            indexed.trace().base_source_row_index()
        );
        assert_eq!(typed.trace().divisor(), indexed.trace().divisor());
        assert!(!typed.trace().divisor().is_zero());
        assert!(!indexed.trace().divisor().is_zero());
        assert_eq!(
            typed.trace().reductions().len(),
            indexed.trace().reductions().len()
        );
        for (typed_reduction, indexed_reduction) in typed
            .trace()
            .reductions()
            .iter()
            .zip(indexed.trace().reductions())
        {
            let prior = indexed_reduction.prior_pivot_ordinal();
            assert!(prior < ordinal);
            assert_eq!(typed_reduction.prior_pivot_ordinal(), prior);
            assert_eq!(
                typed_reduction.prior_pivot(),
                certificate.pivots()[prior].pivot()
            );
            assert_eq!(typed_reduction.factor(), indexed_reduction.factor());
            assert!(!typed_reduction.factor().is_zero());
            assert!(!indexed_reduction.factor().is_zero());
        }

        let expected_rhs = indexed
            .row()
            .iter()
            .filter(|(column, _)| **column != indexed.pivot_column())
            .map(|(&column, coefficient)| {
                (certificate.columns()[column].clone(), -coefficient.clone())
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(typed.rhs(), &expected_rhs);
        assert!(typed.rhs().keys().all(|column| column < typed.pivot()));
    }

    let pivot_indices = exact
        .pivot_rules()
        .iter()
        .map(|rule| rule.pivot_column())
        .collect::<BTreeSet<_>>();
    let expected_free_indices = (0..certificate.columns().len())
        .filter(|column| !pivot_indices.contains(column))
        .collect::<Vec<_>>();
    assert_eq!(exact.free_columns(), expected_free_indices);
    let expected_typed_free = exact
        .free_columns()
        .iter()
        .map(|&column| certificate.columns()[column].clone())
        .collect::<Vec<_>>();
    assert_eq!(certificate.free_unresolved_columns(), expected_typed_free);

    assert_eq!(
        cross_auth.status(),
        FourLoopNextCornerCrossAuthStatus::CompleteInheritedCornerDispositionFixedSeedShell
    );
    assert!(std::ptr::eq(cross_auth.corner(), &corner));
    assert!(std::ptr::eq(cross_auth.elimination(), &certificate));
    assert_eq!(
        corner_cross::FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_CHECKSUM,
        0xa359_ccf8_3fd1_eb5c
    );
    assert_eq!(
        cross_auth.checksum(),
        corner_cross::FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_CHECKSUM
    );

    let cross_stats = cross_auth.stats();
    assert_eq!(
        cross_stats.base_rows(),
        corner_cross::FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_ROWS
    );
    assert_eq!(
        cross_stats.base_columns(),
        corner_cross::FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_COLUMNS
    );
    assert_eq!(
        cross_stats.base_entries(),
        corner_cross::FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_ENTRIES
    );
    assert_eq!(
        cross_stats.base_rank(),
        corner_cross::FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_RANK
    );
    assert_eq!(
        cross_stats.inherited_columns(),
        corner_cross::FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_INHERITED_COLUMNS
    );
    assert_eq!(
        cross_stats.embedded_rows(),
        corner_cross::FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_ROWS
    );
    assert_eq!(
        cross_stats.embedded_entries(),
        corner_cross::FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_EMBEDDED_ENTRIES
    );
    assert_eq!(
        cross_stats.coefficient_projections(),
        corner_cross::FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_COEFFICIENT_PROJECTIONS
    );
    assert_eq!(
        cross_stats.next_columns(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_COLUMNS
    );
    assert_eq!(
        cross_stats.next_rank(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_RANK
    );
    assert_eq!(
        cross_stats.next_free_columns(),
        elimination::FOUR_LOOP_NEXT_ELIMINATION_FREE_UNRESOLVED_COLUMNS
    );
    assert_eq!(
        cross_stats.pivoted_nonterminals(),
        corner_cross::FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_PIVOTED_NONTERMINALS
    );
    assert_eq!(
        cross_stats.retained_terminals(),
        corner_cross::FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_RETAINED_TERMINALS
    );
    assert_eq!(
        cross_stats.retained_scalars(),
        corner_cross::FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_RETAINED_SCALARS
    );
    assert_eq!(
        cross_stats.retained_products(),
        corner_cross::FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_RETAINED_PRODUCTS
    );
    assert_eq!(
        cross_stats.pivoted_d1_n0(),
        corner_cross::FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_PIVOTED_D1_N0
    );
    assert_eq!(
        cross_stats.pivoted_d1_n1(),
        corner_cross::FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_PIVOTED_D1_N1
    );

    let base_set = corner
        .normalized_rows()
        .iter()
        .flat_map(|row| row.entries().keys().cloned())
        .collect::<BTreeSet<_>>();
    let ordered_base = base_set.iter().cloned().collect::<Vec<_>>();
    assert_eq!(cross_auth.base_columns(), ordered_base);
    assert_eq!(
        cross_auth.base_columns().len(),
        corner_cross::FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_COLUMNS
    );
    assert!(
        cross_auth
            .base_columns()
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
    assert_eq!(
        cross_auth.inherited_columns(),
        corner.free_unresolved_columns()
    );
    for columns in [
        cross_auth.inherited_columns(),
        cross_auth.pivoted_nonterminals(),
        cross_auth.retained_terminals(),
    ] {
        assert!(columns.windows(2).all(|pair| pair[0] < pair[1]));
    }

    let inherited_set = cross_auth
        .inherited_columns()
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let pivoted_nonterminal_set = cross_auth
        .pivoted_nonterminals()
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let retained_terminal_set = cross_auth
        .retained_terminals()
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    assert!(pivoted_nonterminal_set.is_disjoint(&retained_terminal_set));
    assert_eq!(
        pivoted_nonterminal_set
            .union(&retained_terminal_set)
            .cloned()
            .collect::<BTreeSet<_>>(),
        inherited_set
    );

    let next_pivot_set = certificate
        .pivots()
        .iter()
        .map(|rule| rule.pivot().clone())
        .collect::<BTreeSet<_>>();
    let next_free_set = certificate
        .free_unresolved_columns()
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        inherited_set
            .intersection(&next_pivot_set)
            .cloned()
            .collect::<BTreeSet<_>>(),
        pivoted_nonterminal_set
    );
    assert_eq!(
        inherited_set
            .intersection(&next_free_set)
            .cloned()
            .collect::<BTreeSet<_>>(),
        retained_terminal_set
    );

    // Across all 223 native base columns, the larger exact shell pivots 207
    // and retains precisely the same sixteen inherited terminals as free.
    let pivoted_base_set = base_set
        .intersection(&next_pivot_set)
        .cloned()
        .collect::<BTreeSet<_>>();
    let free_base_set = base_set
        .intersection(&next_free_set)
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(pivoted_base_set.len(), 207);
    assert_eq!(free_base_set.len(), 16);
    assert!(pivoted_base_set.is_disjoint(&free_base_set));
    assert_eq!(free_base_set, retained_terminal_set);
    assert_eq!(
        pivoted_base_set
            .union(&free_base_set)
            .cloned()
            .collect::<BTreeSet<_>>(),
        base_set
    );

    eprintln!(
        "four-loop next exact elimination: rank={}, free={}, exact_stats={exact_stats:#?}, adapter_stats={stats:#?}, conditions={conditions:#?}, cross_auth_stats={cross_stats:#?}, projected_source_checksum=0x{:016x}, exact_checksum=0x{:016x}, adapter_checksum=0x{:016x}, cross_auth_checksum=0x{:016x}",
        certificate.rank(),
        certificate.free_unresolved_columns().len(),
        exact.source_checksum(),
        exact.checksum(),
        certificate.checksum(),
        cross_auth.checksum(),
    );

    cross_auth.replay().unwrap();
}
