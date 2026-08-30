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
    let one_below = exact.replay().exact_operations() - 1;
    limits = ParametricRuleLimits {
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
