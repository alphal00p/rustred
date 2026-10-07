use super::*;
use crate::reduction::ReductionLimits;
use crate::sector::Mask;
use crate::sector::symmetry::{self, CoefficientMatrix, MomentumMap, integral_transport};
use crate::solver::CandidateOwnerRoute;
use crate::solver::candidate_reduction::owner_test_support::*;
use std::sync::Arc;

fn weighted_route(limits: ReductionLimits) -> RoutedCandidateReducer<3> {
    let family = Arc::new(crate::solver::tests::sunset());
    let c = family.coefficient_context();
    let map = symmetry::verify(
        &family,
        &family,
        MomentumMap::new(
            CoefficientMatrix::try_new(2, 2, [c.zero(), c.one(), c.one(), c.one()]).unwrap(),
            CoefficientMatrix::try_new(2, 0, []).unwrap(),
            CoefficientMatrix::try_new(0, 0, []).unwrap(),
        ),
        Default::default(),
    )
    .unwrap();
    let owner_sector = Mask::try_new([false, true, false]).unwrap();
    let transport = integral_transport::compile(
        &family,
        family.clone(),
        Arc::new(map),
        Mask::try_new([true, false, false]).unwrap(),
        owner_sector.clone(),
        Default::default(),
    )
    .unwrap();
    let owners = programs(
        family.clone(),
        None,
        vec![input(
            [false, true, false],
            None,
            vec![
                rule(&family, [-1, 1, 0], &[([0, 1, 0], 1), ([0, 1, 0], 1)]),
                rule(&family, [0, 1, -1], &[([0, 1, 0], 1), ([0, 1, 0], 1)]),
            ],
            &[[0, 1, 0]],
        )],
        limits,
    );
    RoutedCandidateReducer::try_new(
        owners,
        [CandidateOwnerRoute {
            owner_sector,
            transport: Arc::new(transport),
        }],
        Default::default(),
    )
    .unwrap()
}

#[test]
fn weighted_endpoint_identities_share_rule_and_coalescing_budgets() {
    let target = key([1, 0, -1]);
    let admitted = weighted_route(ReductionLimits {
        max_rule_applications: 2,
        max_coalescing_additions: 2,
        ..Default::default()
    });
    // One routing equation and two endpoint rules, each merging two RHS terms.
    assert_eq!(
        admitted.terminal_identity_equations(&target).unwrap().len(),
        3
    );

    let rule_limited = weighted_route(ReductionLimits {
        max_rule_applications: 1,
        ..Default::default()
    });
    assert!(matches!(
        rule_limited.terminal_identity_equations(&target),
        Err(CandidateRoutedError::Candidate(
            CandidateReductionError::Application(ReductionError::RuleApplicationLimit {
                requested: 2,
                limit: 1
            })
        ))
    ));

    let addition_limited = weighted_route(ReductionLimits {
        max_coalescing_additions: 1,
        ..Default::default()
    });
    assert!(matches!(
        addition_limited.terminal_identity_equations(&target),
        Err(CandidateRoutedError::Candidate(
            CandidateReductionError::Application(ReductionError::CoalescingAdditionLimit {
                requested: 2,
                limit: 1
            })
        ))
    ));
}
