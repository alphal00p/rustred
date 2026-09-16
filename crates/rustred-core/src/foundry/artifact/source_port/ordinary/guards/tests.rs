use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::{CoordinateCase, ExceptionalConditions};

fn fixture() -> (CoefficientContext, SectorRule<2>, Vec<LatticeBox>) {
    let context = CoefficientContext::new(["n0", "n1", "d"]);
    let case = CoordinateCase::generic();
    let rule = SectorRule {
        candidate: RuleCandidate {
            case: case.into(),
            target: case.integral(),
            rhs: vec![],
            sources: vec![],
            stats: Default::default(),
        },
        exceptions: ExceptionalConditions::default(),
    };
    (
        context,
        rule,
        vec![LatticeBox::try_new([0, 0], [None, None]).unwrap()],
    )
}

#[test]
fn newly_admitted_index_pole_is_rejected_but_parameter_poles_are_retained_downstream() {
    let (context, rule, boxes) = fixture();
    assert!(
        !preserves_domain(
            &[context.coefficient_fixture("1/n0")],
            &rule,
            &boxes,
            &[0, 1],
            &[false; 2]
        )
        .unwrap()
    );
    assert!(
        preserves_domain(
            &[context.coefficient_fixture("1/(d-4)")],
            &rule,
            &boxes,
            &[0, 1],
            &[false; 2]
        )
        .unwrap()
    );
}

#[test]
fn existing_exact_exception_and_already_tightened_boxes_allow_the_same_pole() {
    let (context, mut rule, boxes) = fixture();
    rule.exceptions
        .branches
        .push(vec![context.coefficient_fixture("n0").numerator]);
    assert!(
        preserves_domain(
            &[context.coefficient_fixture("1/n0")],
            &rule,
            &boxes,
            &[0, 1],
            &[false; 2]
        )
        .unwrap()
    );
    rule.exceptions.branches.clear();
    let tightened = vec![LatticeBox::try_new([1, 0], [None, None]).unwrap()];
    assert!(
        preserves_domain(
            &[context.coefficient_fixture("1/n0")],
            &rule,
            &tightened,
            &[0, 1],
            &[false; 2]
        )
        .unwrap()
    );
    assert!(
        !preserves_domain(
            &[context.coefficient_fixture("1/(n0+1)")],
            &rule,
            &tightened,
            &[0, 1],
            &[false; 2]
        )
        .unwrap()
    );
}

#[test]
fn coupled_new_pole_is_not_approximated_by_a_coordinate_prefilter() {
    let (context, mut rule, boxes) = fixture();
    let denominator = context.coefficient_fixture("n0-n1").numerator;
    let weight = context.coefficient_fixture("1/(n0-n1)");
    assert!(
        !preserves_domain(
            std::slice::from_ref(&weight),
            &rule,
            &boxes,
            &[0, 1],
            &[false; 2]
        )
        .unwrap()
    );
    rule.exceptions.branches.push(vec![denominator]);
    assert!(preserves_domain(&[weight], &rule, &boxes, &[0, 1], &[false; 2]).unwrap());
}

#[test]
fn source_condition_index_dependence_disables_support_changes() {
    let context = CoefficientContext::new(["n0", "n1", "d"]);
    assert!(conditions_are_index_free(
        &[context.coefficient_fixture("d-4").numerator],
        &[0, 1]
    ));
    assert!(!conditions_are_index_free(
        &[context.coefficient_fixture("d+n0").numerator],
        &[0, 1]
    ));
    assert!(!conditions_are_index_free(
        &[context.one().numerator],
        &[0, 9]
    ));
}
