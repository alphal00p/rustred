use crate::algebra::CoefficientContext;
use crate::solver::{CoordinateCase, Integral, Power, RuleCandidate, SearchStats, Term};

use super::{ExceptionError, ExceptionalConditions, extract_exceptions};

fn candidate<const N: usize>(
    context: &CoefficientContext,
    terms: &[([i16; N], &str)],
) -> RuleCandidate<N> {
    RuleCandidate {
        case: CoordinateCase::generic(),
        target: Integral::symbolic([0; N]).unwrap(),
        rhs: terms
            .iter()
            .map(|(powers, coefficient)| Term {
                integral: Integral::symbolic(*powers).unwrap(),
                coefficient: context.coefficient_fixture(coefficient),
            })
            .collect(),
        sources: Vec::new(),
        stats: SearchStats::default(),
    }
}

fn expected(context: &CoefficientContext, branches: &[&[&str]]) -> ExceptionalConditions {
    use symbolica::domains::InternalOrdering;

    let mut branches: Vec<Vec<_>> = branches
        .iter()
        .map(|branch| {
            let mut equations: Vec<_> = branch
                .iter()
                .map(|equation| context.coefficient_fixture(equation).numerator)
                .collect();
            equations.sort_unstable_by(InternalOrdering::internal_cmp);
            equations
        })
        .collect();
    branches.sort_unstable_by(InternalOrdering::internal_cmp);
    ExceptionalConditions { branches }
}

#[test]
fn parameter_collection_conjoins_all_index_coefficients() {
    let context = CoefficientContext::new(["d", "n0", "n1"]);
    let rule = candidate(&context, &[([-1, 0], "1/(d*n0+n1)")]);
    assert_eq!(
        extract_exceptions(&rule, &[1, 2], &[true; 2]).unwrap(),
        expected(&context, &[&["n0", "n1"]])
    );
}

#[test]
fn parameter_only_denominators_and_nonzero_parameter_coefficients_have_no_exception() {
    let context = CoefficientContext::new(["d", "n0"]);
    for coefficient in ["1/(d-4)", "1/(d+n0)", "1/(d^2+d*n0)"] {
        let rule = candidate(&context, &[([-1], coefficient)]);
        assert_eq!(
            extract_exceptions(&rule, &[1], &[true]).unwrap(),
            ExceptionalConditions::default()
        );
    }
}

#[test]
fn denominator_factors_produce_coordinate_or_affine_branches() {
    let context = CoefficientContext::new(["d", "n0", "n1"]);
    let rule = candidate(&context, &[([-1, 0], "1/((n0-1)*(n0-n1))")]);
    assert_eq!(
        extract_exceptions(&rule, &[1, 2], &[true; 2]).unwrap(),
        expected(&context, &[&["n0-1"], &["n0-n1"]])
    );
}

#[test]
fn nonlinear_equations_remain_exact_without_geometry_claims() {
    let context = CoefficientContext::new(["d", "n0", "n1"]);
    let rule = candidate(&context, &[([-1, 0], "1/(n0*n1+1)")]);
    assert_eq!(
        extract_exceptions(&rule, &[1, 2], &[true; 2]).unwrap(),
        expected(&context, &[&["n0*n1+1"]])
    );
}

#[test]
fn inactive_activation_is_skipped_exactly_when_the_numerator_vanishes_on_the_face() {
    let context = CoefficientContext::new(["n0"]);
    let vanished = candidate(&context, &[([1], "n0")]);
    assert_eq!(
        extract_exceptions(&vanished, &[0], &[false]).unwrap(),
        ExceptionalConditions::default()
    );
    let active = candidate(&context, &[([1], "1")]);
    assert_eq!(
        extract_exceptions(&active, &[0], &[false]).unwrap(),
        expected(&context, &[&["n0"]])
    );
    let three_faces = candidate(&context, &[([3], "n0*(n0+2)")]);
    assert_eq!(
        extract_exceptions(&three_faces, &[0], &[false]).unwrap(),
        expected(&context, &[&["n0+1"]])
    );
}

#[test]
fn skipped_activation_does_not_remove_the_denominator_exception() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let rule = candidate(&context, &[([1, 0], "n0/(n0+n1)")]);
    assert_eq!(
        extract_exceptions(&rule, &[0, 1], &[false, true]).unwrap(),
        expected(&context, &[&["n0+n1"]])
    );
}

#[test]
fn explicit_index_positions_allow_interleaved_and_reversed_variable_maps() {
    let context = CoefficientContext::new(["n1", "x", "n0", "d"]);
    let rule = candidate(&context, &[([-1, 0], "1/(d*n0+x*n1)")]);
    assert_eq!(
        extract_exceptions(&rule, &[2, 0], &[true; 2]).unwrap(),
        expected(&context, &[&["n0", "n1"]])
    );
    let activating = candidate(&context, &[([2, 0], "n0")]);
    assert_eq!(
        extract_exceptions(&activating, &[2, 0], &[false, true]).unwrap(),
        expected(&context, &[&["n0+1"]])
    );
}

#[test]
fn scalar_content_repeated_factors_and_repeated_branches_are_deduplicated() {
    let context = CoefficientContext::new(["n0"]);
    let rule = candidate(
        &context,
        &[
            ([-1], "1/(2*(n0-1))"),
            ([-2], "1/(3*(1-n0))"),
            ([-3], "1/(n0-1)^2"),
        ],
    );
    assert_eq!(
        extract_exceptions(&rule, &[0], &[true]).unwrap(),
        expected(&context, &[&["n0-1"]])
    );
}

#[test]
fn invalid_canonical_arrays_are_rejected_before_native_algebra() {
    let context = CoefficientContext::new(["d", "n0", "n1"]);
    let mut rule = candidate(&context, &[([-1, 0], "1/(n0+n1)")]);
    assert!(matches!(
        extract_exceptions(&rule, &[1, 1], &[true; 2]),
        Err(ExceptionError::InvalidInput(_))
    ));
    assert!(matches!(
        extract_exceptions(&rule, &[1, 3], &[true; 2]),
        Err(ExceptionError::InvalidInput(_))
    ));
    rule.target = Integral::symbolic([1, 0]).unwrap();
    assert!(matches!(
        extract_exceptions(&rule, &[1, 2], &[true; 2]),
        Err(ExceptionError::InvalidInput(_))
    ));
    rule.target = Integral::symbolic([0, 0]).unwrap();
    rule.rhs[0].coefficient.denominator = rule.rhs[0].coefficient.denominator.zero();
    assert!(matches!(
        extract_exceptions(&rule, &[1, 2], &[true; 2]),
        Err(ExceptionError::InvalidInput(_))
    ));
}

#[test]
fn an_empty_rhs_has_no_exceptions() {
    let context = CoefficientContext::new(["n0"]);
    let rule = candidate::<1>(&context, &[]);
    assert_eq!(
        extract_exceptions(&rule, &[0], &[true]).unwrap(),
        ExceptionalConditions::default()
    );
}

#[test]
fn an_already_specialized_fixed_case_retains_only_free_coordinate_equations() {
    let context = CoefficientContext::new(["d", "n0", "n1"]);
    // Restricting 1/(n0+n1-5) to n0=2 already produced this coefficient.
    let mut rule = candidate(&context, &[([0, -1], "1/(n1-3)")]);
    rule.case = CoordinateCase::new([Some(2), None]).unwrap();
    rule.target = rule.case.integral();
    rule.rhs[0].integral =
        Integral::new([Power::new(false, 1).unwrap(), Power::new(true, -1).unwrap()]);
    let exceptions = extract_exceptions(&rule, &[1, 2], &[true; 2]).unwrap();
    assert_eq!(exceptions, expected(&context, &[&["n1-3"]]));
    assert!(
        exceptions
            .branches
            .iter()
            .flatten()
            .all(|equation| equation.degree(1) == 0)
    );
}

#[test]
fn zero_indices_and_zero_variables_with_a_constant_rhs_have_no_exceptions() {
    let context = CoefficientContext::new(std::iter::empty::<&str>());
    let rule = candidate::<0>(&context, &[([], "2")]);
    assert_eq!(
        extract_exceptions(&rule, &[], &[]).unwrap(),
        ExceptionalConditions::default()
    );
}
