use super::*;
use crate::reduction::ReductionLimits;
use crate::sector::Mask;
use crate::sector::symmetry::{self, CoefficientMatrix, MomentumMap, integral_transport};
use crate::solver::CandidateOwnerRoute;
use crate::solver::candidate_reduction::owner_test_support::*;
use std::sync::Arc;

fn weighted_route<const N: usize>(limits: ReductionLimits) -> RoutedCandidateReducer<N> {
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
    let pad = |powers: [i16; 3]| crate::storage_array::<_, N>(&powers, 0).unwrap();
    let owners = programs(
        family.clone(),
        None,
        vec![input(
            crate::storage_array(&[false, true, false], false).unwrap(),
            None,
            vec![
                rule(
                    &family,
                    pad([-1, 1, 0]),
                    &[(pad([0, 1, 0]), 1), (pad([0, 1, 0]), 1)],
                ),
                rule(
                    &family,
                    pad([0, 1, -1]),
                    &[(pad([0, 1, 0]), 1), (pad([0, 1, 0]), 1)],
                ),
            ],
            &[pad([0, 1, 0])],
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
    let admitted = weighted_route::<3>(ReductionLimits {
        max_rule_applications: 2,
        max_coalescing_additions: 2,
        ..Default::default()
    });
    // One routing equation and two endpoint rules, each merging two RHS terms.
    assert_eq!(
        admitted.terminal_identity_equations(&target).unwrap().len(),
        3
    );

    let rule_limited = weighted_route::<3>(ReductionLimits {
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

    let addition_limited = weighted_route::<3>(ReductionLimits {
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

#[test]
#[cfg(feature = "capacity-dispatch")]
fn padded_capacity_routing_equations_equal_exact_arity_rows() {
    let exact = weighted_route::<3>(Default::default());
    let padded = weighted_route::<4>(Default::default());
    for target in [key([1, 0, -1]), key([-1, 1, 0])] {
        let expected = exact.terminal_identity_equations(&target).unwrap();
        let actual = padded.terminal_identity_equations(&target).unwrap();
        assert_eq!(actual.len(), expected.len());
        for (left, right) in actual.iter().zip(&expected) {
            assert_eq!(left.terms, right.terms);
            assert_eq!(left.nonzero_conditions, right.nonzero_conditions);
            assert!(left.terms.keys().all(|key| key.powers().len() == 3));
        }
    }
    assert!(matches!(
        padded.terminal_identity_equations(&key([1, 0, -1, 0])),
        Err(CandidateRoutedError::Candidate(
            CandidateReductionError::Application(ReductionError::WrongArity {
                expected: 3,
                actual: 4
            })
        ))
    ));
}
