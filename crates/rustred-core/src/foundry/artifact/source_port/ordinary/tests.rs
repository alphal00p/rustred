use crate::algebra::CoefficientContext;
use crate::solver::{CoordinateCase, ExceptionalConditions, Integral, Power, RuleCandidate};

use super::*;

fn fixture() -> (CoefficientContext, SectorRule<2>, IntegralOrder<2>) {
    let context = CoefficientContext::new(["n0", "n1", "d"]);
    let case = CoordinateCase::new([None, Some(1)]).unwrap();
    let rule = SectorRule {
        candidate: RuleCandidate {
            case: case.into(),
            target: case.integral(),
            rhs: Vec::new(),
            sources: Vec::new(),
            stats: Default::default(),
        },
        exceptions: ExceptionalConditions::default(),
    };
    (
        context,
        rule,
        IntegralOrder::new([false, true], [false, false]),
    )
}

fn term(
    context: &CoefficientContext,
    shift: i16,
    fixed: i16,
    coefficient: &str,
) -> Term<2, Coefficient> {
    Term {
        integral: Integral::new([
            Power::new(true, shift).unwrap(),
            Power::new(false, fixed).unwrap(),
        ]),
        coefficient: context.coefficient_fixture(coefficient),
    }
}

#[test]
fn weighted_original_replay_proves_two_point_activation_only_after_combination() {
    let (context, rule, order) = fixture();
    let boxes = geometry::application_boxes(&rule, &[0, 1], order.sector(), &[]).unwrap();
    let desired = vec![term(&context, 0, 1, "1")];
    let rows = vec![
        vec![term(&context, 0, 1, "1"), term(&context, -1, 1, "n0")],
        vec![term(&context, -1, 1, "1"), term(&context, 2, 0, "n0+1")],
    ];
    let zero_product = |term: &Term<2, Coefficient>| {
        geometry::uniformly_zero_term(
            &rule,
            term,
            &boxes,
            order.sector(),
            &[[false, false]],
            &[0, 1],
        )
    };
    // Neither raw activating term is zero over n0 <= 0. The strict projected
    // frame therefore has no exact membership certificate for this target.
    assert!(
        native::propose(&rows, &desired, &order, zero_product)
            .unwrap()
            .is_none()
    );
    let mut weights = native::propose(&rows, &desired, &order, |term| {
        Ok(term.integral[1].value() == 0)
    })
    .unwrap()
    .unwrap();
    assert_eq!(
        weights,
        vec![context.one(), -context.parameter("n0").unwrap()]
    );
    // Full unprojected replay leaves -n0*(n0+1)*I(n0+2,0): the bulk is
    // independently zero-sector, and BOTH points {-1,0} are exactly zero.
    native::verify(&rows, &desired, &weights, &order, zero_product).unwrap();

    weights[1] = &weights[1] + &context.one();
    assert!(native::verify(&rows, &desired, &weights, &order, zero_product).is_err());
    weights[1] = -context.parameter("n0").unwrap();
    let mut corrupt_sources = rows.clone();
    corrupt_sources[0][0].coefficient = context.integer(2);
    assert!(native::verify(&corrupt_sources, &desired, &weights, &order, zero_product).is_err());
    assert!(native::verify(&rows, &desired, &weights[..1], &order, zero_product).is_err());
}

#[test]
fn finite_activation_checks_every_integer_and_rejects_poles_and_infinite_sampling() {
    let (context, rule, order) = fixture();
    let boxes = geometry::application_boxes(&rule, &[0, 1], order.sector(), &[]).unwrap();
    let check = |shift, coefficient| {
        geometry::uniformly_zero_term(
            &rule,
            &term(&context, shift, 0, coefficient),
            &boxes,
            order.sector(),
            &[[false, false]],
            &[0, 1],
        )
    };
    assert!(check(2, "n0*(n0+1)").unwrap());
    assert!(
        !check(2, "n0+1").unwrap(),
        "zero at -1 is insufficient at 0"
    );
    assert!(!check(2, "n0").unwrap(), "zero at 0 is insufficient at -1");
    assert!(!check(2, "1").unwrap());
    assert!(
        !check(2, "n0*(n0+1)/(n0+n1)").unwrap(),
        "0/0 at n0=-1,n1=1 is not zero"
    );
    // With no zero census, the unbounded negative tail remains a symbolic
    // polynomial, even though the finite activation boundary vanishes.
    assert!(
        !geometry::uniformly_zero_term(
            &rule,
            &term(&context, 2, 0, "n0*(n0+1)"),
            &boxes,
            order.sector(),
            &[],
            &[0, 1],
        )
        .unwrap()
    );
}

#[test]
fn finite_zero_proof_budget_is_fail_closed_not_partial_coverage() {
    let context = CoefficientContext::new(["n0", "n1", "n2", "n3", "d"]);
    let sector = [false, true, false, true];
    let case = CoordinateCase::generic();
    let rule = SectorRule {
        candidate: RuleCandidate {
            case: case.into(),
            target: case.integral(),
            rhs: Vec::new(),
            sources: Vec::new(),
            stats: Default::default(),
        },
        exceptions: ExceptionalConditions::default(),
    };
    let boxes = geometry::application_boxes(&rule, &[0, 1, 2, 3], &sector, &[]).unwrap();
    let contribution = Term {
        // Every shift is compact-representable, but the first physical sign
        // cell has 32^4 points, exceeding the default proof budget.
        integral: Integral::symbolic([32, -32, 32, -32]).unwrap(),
        coefficient: context.coefficient_fixture("n0*n1*n2*n3"),
    };
    let error =
        geometry::uniformly_zero_term(&rule, &contribution, &boxes, &sector, &[], &[0, 1, 2, 3])
            .unwrap_err();
    assert!(error.to_string().contains("budget"));
}
