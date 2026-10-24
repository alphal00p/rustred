use crate::sector::OrderingPolicy;

use super::super::{ParametricRuleError, ParametricRuleLimits, derive_sector_interior_rule};
use super::support::tadpole_sources;

#[test]
fn empty_wrong_arity_and_boundary_anchor_fail_typed() {
    let (_, context, relations, _) = tadpole_sources();
    assert_eq!(
        derive_sector_interior_rule(
            &context,
            &[],
            &[1],
            OrderingPolicy::default(),
            ParametricRuleLimits::default(),
        ),
        Err(ParametricRuleError::EmptySourceRows)
    );
    assert_eq!(
        derive_sector_interior_rule(
            &context,
            &relations,
            &[1, 2],
            OrderingPolicy::default(),
            ParametricRuleLimits::default(),
        ),
        Err(ParametricRuleError::WrongAnchorArity {
            expected: 1,
            actual: 2,
        })
    );
    assert_eq!(
        derive_sector_interior_rule(
            &context,
            &relations,
            &[0],
            OrderingPolicy::default(),
            ParametricRuleLimits::default(),
        ),
        Err(ParametricRuleError::AnchorOutsideInterior)
    );
}

#[test]
fn domain_ordering_and_replay_limits_are_exact_and_typed() {
    let (_, context, relations, _) = tadpole_sources();
    let exact_structural = ParametricRuleLimits {
        diagnostics_enabled: true,
        max_index_coordinate_cells: 3,
        max_sector_mask_cells: 1,
        max_domain_bound_endpoint_cells: 2,
        max_ordering_key_coordinate_cells: 4,
        ..ParametricRuleLimits::default()
    };
    derive_sector_interior_rule(
        &context,
        &relations,
        &[1],
        OrderingPolicy::default(),
        exact_structural,
    )
    .unwrap();

    let mut limits = ParametricRuleLimits {
        diagnostics_enabled: true,
        max_index_coordinate_cells: 1,
        ..ParametricRuleLimits::default()
    };
    assert_eq!(
        derive_sector_interior_rule(
            &context,
            &relations,
            &[1],
            OrderingPolicy::default(),
            limits,
        ),
        Err(ParametricRuleError::ResourceLimit {
            resource: "prospective parametric shift coordinate cells",
            requested: 2,
            limit: 1,
        })
    );

    limits = ParametricRuleLimits {
        diagnostics_enabled: true,
        max_index_coordinate_cells: 2,
        ..ParametricRuleLimits::default()
    };
    assert_eq!(
        derive_sector_interior_rule(
            &context,
            &relations,
            &[1],
            OrderingPolicy::default(),
            limits,
        ),
        Err(ParametricRuleError::ResourceLimit {
            resource: "live parametric index-coordinate cells",
            requested: 3,
            limit: 2,
        })
    );

    limits = ParametricRuleLimits {
        diagnostics_enabled: true,
        max_sector_mask_cells: 0,
        ..ParametricRuleLimits::default()
    };
    assert_eq!(
        derive_sector_interior_rule(
            &context,
            &relations,
            &[1],
            OrderingPolicy::default(),
            limits,
        ),
        Err(ParametricRuleError::ResourceLimit {
            resource: "parametric sector mask cells",
            requested: 1,
            limit: 0,
        })
    );

    limits = ParametricRuleLimits {
        diagnostics_enabled: true,
        max_domain_bound_endpoint_cells: 1,
        ..ParametricRuleLimits::default()
    };
    assert_eq!(
        derive_sector_interior_rule(
            &context,
            &relations,
            &[1],
            OrderingPolicy::default(),
            limits,
        ),
        Err(ParametricRuleError::ResourceLimit {
            resource: "parametric domain bound endpoint cells",
            requested: 2,
            limit: 1,
        })
    );

    limits = ParametricRuleLimits {
        diagnostics_enabled: true,
        max_ordering_key_coordinate_cells: 1,
        ..ParametricRuleLimits::default()
    };
    assert_eq!(
        derive_sector_interior_rule(
            &context,
            &relations,
            &[1],
            OrderingPolicy::default(),
            limits,
        ),
        Err(ParametricRuleError::ResourceLimit {
            resource: "live parametric ordering-key coordinate cells",
            requested: 2,
            limit: 1,
        })
    );

    limits = ParametricRuleLimits {
        diagnostics_enabled: true,
        max_ordering_key_coordinate_cells: 3,
        ..ParametricRuleLimits::default()
    };
    assert_eq!(
        derive_sector_interior_rule(
            &context,
            &relations,
            &[1],
            OrderingPolicy::default(),
            limits,
        ),
        Err(ParametricRuleError::ResourceLimit {
            resource: "live parametric ordering-key coordinate cells",
            requested: 4,
            limit: 3,
        })
    );

    limits = ParametricRuleLimits {
        diagnostics_enabled: true,
        max_concrete_replay_integral_key_power_cells: 3,
        ..ParametricRuleLimits::default()
    };
    assert_eq!(
        derive_sector_interior_rule(
            &context,
            &relations,
            &[1],
            OrderingPolicy::default(),
            limits,
        ),
        Err(ParametricRuleError::ResourceLimit {
            resource: "concrete specialization replay integral-key power cells",
            requested: 5,
            limit: 3,
        })
    );

    let exact = derive_sector_interior_rule(
        &context,
        &relations,
        &[1],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();
    let one_below = exact.replay().expect("anchored fixture").exact_operations() - 1;
    limits = ParametricRuleLimits {
        diagnostics_enabled: true,
        max_replay_exact_operations: one_below,
        ..ParametricRuleLimits::default()
    };
    assert_eq!(
        derive_sector_interior_rule(
            &context,
            &relations,
            &[1],
            OrderingPolicy::default(),
            limits,
        ),
        Err(ParametricRuleError::ResourceLimit {
            resource: "parametric replay exact operations",
            requested: one_below + 1,
            limit: one_below,
        })
    );
}

#[test]
fn ordinary_zero_templates_keep_exact_replay_and_shared_resources() {
    use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};
    let (_, context, relations, _) = tadpole_sources();
    let limits = ParametricRuleLimits {
        max_source_rows: 0,
        max_shift_columns: 0,
        max_augmented_columns: 0,
        max_input_nonzero_entries: 0,
        max_index_coordinate_cells: 0,
        max_ordering_key_coordinate_cells: 0,
        max_domain_bound_endpoint_cells: 0,
        max_sector_monotone_thresholds: 0,
        max_sector_mask_cells: 0,
        max_guard_provenance_index_cells: 0,
        max_native_decomposition_nonzero_entries: 0,
        max_back_substitution_output_nonzero_entries: 0,
        max_back_substitution_live_nonzero_entries: 0,
        max_rule_guards: 0,
        max_guard_origins: 0,
        max_guard_provenance_sources: 0,
        max_elimination_pivots: 0,
        max_elimination_pivot_dependency_entries: 0,
        max_source_combination_terms: 0,
        max_replay_exact_operations: 0,
        max_concrete_replay_terms: 0,
        max_concrete_replay_integral_keys: 0,
        max_concrete_replay_retained_coefficient_terms: 0,
        max_concrete_replay_retained_coefficient_clone_owned_bytes: 0,
        max_concrete_replay_integral_key_power_cells: 0,
        max_concrete_replay_exact_operations: 0,
        ..Default::default()
    };
    let derive = || {
        derive_sector_interior_rule(
            &context,
            &relations,
            &[1],
            OrderingPolicy::default(),
            limits,
        )
    };
    assert!(derive().is_ok());
    assert!(
        derive_sector_interior_rule(
            &context,
            &relations,
            &[1],
            OrderingPolicy::default(),
            limits.with_diagnostics()
        )
        .is_err()
    );
    let budget = ResourceBudget::with_memory_monitor(
        ResourceLimits {
            max_process_rss_bytes: Some(1),
            ..Default::default()
        },
        || Ok(2),
    );
    let error = budget.run(derive).unwrap_err();
    assert!(matches!(
        crate::reduction::resource_limit_error(&error),
        Some(ResourceLimitError::Memory { .. })
    ));
    assert!(derive().is_ok());
}
