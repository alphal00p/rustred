use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::{AffineCase, AffineIntersection, ExceptionalConditions, RuleCandidate, Term};

#[test]
fn descent_uses_entire_excluded_conjunction_without_cancelling_zero_over_zero() {
    let context = CoefficientContext::new(["a", "b", "c"]);
    let sector = [false; 3];
    let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
        &CoordinateCase::generic(),
        &[context.coefficient_fixture("a-b").numerator],
        &[0, 1, 2],
        &sector,
    )
    .unwrap() else {
        panic!("expected affine exception")
    };
    let excluded = Arc::new(AffineApplicationDomain::from_case(&case, &sector).unwrap());
    let make_rule = |coefficient| SectorRule {
        candidate: RuleCandidate {
            case: CoordinateCase::generic().into(),
            target: Integral::symbolic([0; 3]).unwrap(),
            rhs: vec![Term {
                integral: Integral::symbolic([1, 1, 0]).unwrap(),
                coefficient,
            }],
            sources: Vec::new(),
            stats: Default::default(),
        },
        exceptions: ExceptionalConditions::default(),
    };
    let rule = make_rule(context.coefficient_fixture("a*b/(b-a)"));
    let boxes = [LatticeBox::try_new([0, 0, 1], [Some(0), Some(0), None]).unwrap()];
    assert!(
        prove_descent(
            &rule,
            &boxes,
            &[],
            &sector,
            OrderingPolicy::SpiredUncutV1,
            &[0, 1, 2]
        )
        .is_err()
    );
    prove_descent(
        &rule,
        &boxes,
        &[Arc::clone(&excluded)],
        &sector,
        OrderingPolicy::SpiredUncutV1,
        &[0, 1, 2],
    )
    .unwrap();
    let ray = [LatticeBox::try_new([0, 1, 1], [Some(0), Some(1), None]).unwrap()];
    assert!(
        prove_descent(
            &make_rule(context.one()),
            &ray,
            &[Arc::clone(&excluded)],
            &sector,
            OrderingPolicy::SpiredUncutV1,
            &[0, 1, 2]
        )
        .is_err()
    );
    assert!(
        prove_descent(
            &rule,
            &boxes,
            &[Arc::clone(&excluded)],
            &sector,
            OrderingPolicy::SpiredUncutV1,
            &[1, 0, 2]
        )
        .is_err()
    );
    let foreign = CoefficientContext::new(["x", "y", "z"]);
    assert!(
        prove_descent(
            &make_rule(foreign.one()),
            &boxes,
            &[excluded],
            &sector,
            OrderingPolicy::SpiredUncutV1,
            &[0, 1, 2]
        )
        .is_err()
    );
}
