use crate::algebra::CoefficientContext;
use crate::solver::{CoordinateCase, Integral, Power, RuleCandidate, SearchStats, Term};

use super::{
    ExceptionError, ExceptionalConditions, extract_exceptions, extract_exceptions_two_sided,
};

fn candidate<const N: usize>(
    context: &CoefficientContext,
    terms: &[([i16; N], &str)],
) -> RuleCandidate<N> {
    RuleCandidate {
        case: CoordinateCase::generic().into(),
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
        hidden_poles: Vec::new(),
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
    rule.case = CoordinateCase::new([Some(2), None]).unwrap().into();
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

#[test]
fn a_shift_down_on_an_active_two_sided_axis_creates_the_mirror_branches() {
    let context = CoefficientContext::new(["n0"]);
    let two_sided = |rule: &RuleCandidate<1>| {
        extract_exceptions_two_sided(rule, &[0], &[true], &[true]).unwrap()
    };
    let rule = candidate(&context, &[([-2], "1")]);
    // One-sided, a symbolic shift has no exception in the chart n >= 1 ...
    assert_eq!(
        extract_exceptions(&rule, &[0], &[true]).unwrap(),
        ExceptionalConditions::default()
    );
    // ... two-sided, n = 1, 2 cross into the chart n <= 0 and are solved apart.
    assert_eq!(
        two_sided(&rule),
        expected(&context, &[&["n0-1"], &["n0-2"]])
    );
    // A shift up stays in the chart.
    assert_eq!(
        two_sided(&candidate(&context, &[([2], "1")])),
        ExceptionalConditions::default()
    );
}

#[test]
fn a_mirror_branch_is_skipped_exactly_when_the_term_vanishes_on_it() {
    let context = CoefficientContext::new(["n0"]);
    let two_sided = |terms: &[([i16; 1], &str)]| {
        extract_exceptions_two_sided(&candidate(&context, terms), &[0], &[true], &[true]).unwrap()
    };
    assert_eq!(
        two_sided(&[([-2], "n0-1")]),
        expected(&context, &[&["n0-2"]])
    );
    assert_eq!(
        two_sided(&[([-3], "(n0-1)*(n0-3)")]),
        expected(&context, &[&["n0-2"]])
    );
    assert_eq!(
        two_sided(&[([-2], "(n0-1)*(n0-2)")]),
        ExceptionalConditions::default()
    );
    // The skip does not remove a denominator exception (the factor n0+3 here),
    // and another term keeps the branch its neighbour vanishes on.
    assert_eq!(
        two_sided(&[([-1], "(n0-1)/(n0+3)")]),
        expected(&context, &[&["n0+3"]])
    );
    assert_eq!(
        two_sided(&[([-1], "n0-1"), ([-2], "1")]),
        expected(&context, &[&["n0-1"], &["n0-2"]])
    );
}

#[test]
fn mirror_branches_belong_to_the_active_two_sided_symbolic_axes_only() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let rule = candidate(&context, &[([-1, -1], "1")]);
    assert_eq!(
        extract_exceptions_two_sided(&rule, &[0, 1], &[true, true], &[true, false]).unwrap(),
        expected(&context, &[&["n0-1"]])
    );
    // A fixed power on a two-sided axis has one chart: no branch to split off.
    let mut fixed = candidate(&context, &[([0, -1], "1")]);
    fixed.case = CoordinateCase::new([Some(2), None]).unwrap().into();
    fixed.target = fixed.case.integral();
    fixed.rhs[0].integral =
        Integral::new([Power::new(false, 1).unwrap(), Power::new(true, -1).unwrap()]);
    assert_eq!(
        extract_exceptions_two_sided(&fixed, &[0, 1], &[true, true], &[true, false]).unwrap(),
        ExceptionalConditions::default()
    );
}

#[test]
fn an_inactive_two_sided_axis_keeps_the_activation_branches() {
    let context = CoefficientContext::new(["n0"]);
    let rule = candidate(&context, &[([2], "n0")]);
    let one_sided = extract_exceptions(&rule, &[0], &[false]).unwrap();
    assert_eq!(one_sided, expected(&context, &[&["n0+1"]]));
    assert_eq!(
        extract_exceptions_two_sided(&rule, &[0], &[false], &[true]).unwrap(),
        one_sided
    );
    // No mirror on this side: a shift down stays in the chart n <= 0.
    let down = candidate(&context, &[([-2], "1")]);
    assert_eq!(
        extract_exceptions_two_sided(&down, &[0], &[false], &[true]).unwrap(),
        ExceptionalConditions::default()
    );
}

#[test]
fn a_numeric_activation_is_a_legal_chart_crossing_only_on_a_two_sided_axis() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let mut rule = candidate(&context, &[([0, -1], "1/(n1-3)")]);
    rule.case = CoordinateCase::new([Some(0), None]).unwrap().into();
    rule.target = rule.case.integral();
    rule.rhs[0].integral =
        Integral::new([Power::new(false, 1).unwrap(), Power::new(true, -1).unwrap()]);
    let sector = [false, true];
    let refused =
        ExceptionError::InvalidInput("numeric RHS index activates an inactive sector coordinate");
    assert_eq!(extract_exceptions(&rule, &[0, 1], &sector), Err(refused));
    // Another axis being two-sided does not admit it.
    assert_eq!(
        extract_exceptions_two_sided(&rule, &[0, 1], &sector, &[false, true]),
        Err(refused)
    );
    assert_eq!(
        extract_exceptions_two_sided(&rule, &[0, 1], &sector, &[true, false]).unwrap(),
        expected(&context, &[&["n1-3"]])
    );
}

// ---- rr-5l-api: the exact content route and the routed (timed) guard extraction ----

use std::time::Duration;

use super::{
    GuardRoute, GuardRouteTaken, NativeGuardTry, child, extract_exceptions_content,
    extract_exceptions_routed,
};

/// Integer value of a polynomial at an index point (parameters must be absent).
fn evaluate(
    polynomial: &crate::algebra::CoefficientPolynomial,
    indices: &[usize],
    point: &[i64],
) -> i128 {
    let mut value = polynomial.clone();
    for (axis, &position) in indices.iter().enumerate() {
        value = value.replace(position, &symbolica::prelude::Integer::from(point[axis]));
    }
    assert!(
        value.is_constant(),
        "an exception equation is parameter-free"
    );
    if value.is_zero() {
        0
    } else {
        value.lcoeff().to_string().parse().unwrap()
    }
}

/// The exceptional point set of `conditions` (OR of ANDs) on a box of index points.
fn zero_set(
    conditions: &ExceptionalConditions,
    indices: &[usize],
    range: std::ops::RangeInclusive<i64>,
) -> Vec<Vec<i64>> {
    let mut points = vec![Vec::new()];
    for _ in indices {
        points = points
            .into_iter()
            .flat_map(|p: Vec<i64>| range.clone().map(move |v| [p.clone(), vec![v]].concat()))
            .collect();
    }
    points
        .into_iter()
        .filter(|point| {
            conditions
                .branches
                .iter()
                .any(|branch| branch.iter().all(|e| evaluate(e, indices, point) == 0))
        })
        .collect()
}

#[test]
fn content_route_matches_native_on_single_factor_inputs() {
    let context = CoefficientContext::new(["d", "n0", "n1"]);
    for coefficient in [
        "1/(d*n0+n1)",
        "1/((n0-1)*(n0-n1))",
        "1/((n0-1)*(d*n1+n0))",
        "1/(d*(n0^2-1)+(n0-1)*n1)",
        "1/(d-4)",
        "1/(n0*n1+1)",
    ] {
        let rule = candidate(&context, &[([-1, 0], coefficient)]);
        assert_eq!(
            extract_exceptions_content(&rule, &[1, 2], &[true; 2], &[false; 2]).unwrap(),
            extract_exceptions(&rule, &[1, 2], &[true; 2]).unwrap(),
            "{coefficient}"
        );
    }
}

#[test]
fn content_route_keeps_the_zero_set_of_several_parameter_dependent_factors() {
    let context = CoefficientContext::new(["d", "n0", "n1", "n2", "n3"]);
    let rule = candidate(
        &context,
        &[([-1, 0, 0, 0], "1/((d*n0+n1)*(d*n2+n3)*(n0-n3-2))")],
    );
    let indices = [1, 2, 3, 4];
    let native = extract_exceptions(&rule, &indices, &[true; 4]).unwrap();
    let content = extract_exceptions_content(&rule, &indices, &[true; 4], &[false; 4]).unwrap();
    // Native: one branch per irreducible factor; content: the d-free factor plus ONE coarse
    // cofactor branch (the d-coefficients of the product).
    assert_eq!(
        native,
        expected(&context, &[&["n0", "n1"], &["n2", "n3"], &["n0-n3-2"]])
    );
    assert_eq!(
        content,
        expected(
            &context,
            &[&["n0*n2", "n0*n3+n1*n2", "n1*n3"], &["n0-n3-2"]]
        )
    );
    assert_eq!(
        zero_set(&native, &indices, -2..=2),
        zero_set(&content, &indices, -2..=2)
    );
}

#[test]
fn content_route_keeps_activation_and_two_sided_mirror_branches() {
    let context = CoefficientContext::new(["d", "n0", "n1"]);
    let rule = candidate(
        &context,
        &[
            ([2, 0], "1/((d*n0+n1)*(n1+5))"),
            ([0, -2], "1"),
            ([-1, 0], "1/(d*n1+1)"),
        ],
    );
    let sector = [false, true];
    let two_sided = [false, true];
    let native = extract_exceptions_two_sided(&rule, &[1, 2], &sector, &two_sided).unwrap();
    let content = extract_exceptions_content(&rule, &[1, 2], &sector, &two_sided).unwrap();
    assert_eq!(native, content);
    assert_eq!(
        native,
        expected(
            &context,
            &[
                &["n0", "n1"],
                &["n1+5"],
                &["n0"],
                &["n0+1"],
                &["n1-1"],
                &["n1-2"]
            ]
        )
    );
}

#[test]
fn routed_extraction_reports_its_route() {
    let context = CoefficientContext::new(["d", "n0", "n1", "n2", "n3"]);
    let rule = candidate(&context, &[([-1, 0, 0, 0], "1/((d*n0+n1)*(d*n2+n3))")]);
    let indices = [1, 2, 3, 4];
    let (sector, two_sided) = ([true; 4], [false; 4]);
    let native = extract_exceptions(&rule, &indices, &sector).unwrap();
    let content = extract_exceptions_content(&rule, &indices, &sector, &two_sided).unwrap();
    assert_ne!(native, content);

    let (got, report) =
        extract_exceptions_routed(&rule, &indices, &sector, &two_sided, GuardRoute::Native)
            .unwrap();
    assert_eq!(
        (got, report.route, report.native_try),
        (
            native.clone(),
            GuardRouteTaken::Native,
            NativeGuardTry::NotTried
        )
    );
    let (got, report) =
        extract_exceptions_routed(&rule, &indices, &sector, &two_sided, GuardRoute::Content)
            .unwrap();
    assert_eq!(
        (got, report.route),
        (content.clone(), GuardRouteTaken::Content)
    );
    let generous = GuardRoute::NativeThenContent {
        native_timeout: Duration::from_secs(120),
    };
    let (got, report) =
        extract_exceptions_routed(&rule, &indices, &sector, &two_sided, generous).unwrap();
    if cfg!(unix) {
        assert_eq!(report.native_try, NativeGuardTry::Finished);
    }
    assert_eq!((got, report.route), (native, GuardRouteTaken::Native));
    // Invalid input fails before any child.
    let mut invalid = candidate(&context, &[([-1, 0, 0, 0], "1/((d*n0+n1)*(d*n2+n3))")]);
    invalid.target = Integral::symbolic([1, 0, 0, 0]).unwrap();
    assert!(matches!(
        extract_exceptions_routed(&invalid, &indices, &sector, &two_sided, generous),
        Err(ExceptionError::InvalidInput(_))
    ));
}

#[cfg(unix)]
#[test]
fn a_timed_child_is_killed_at_its_timeout_and_failures_are_reported() {
    let start = std::time::Instant::now();
    let outcome = child::finishes_within(Duration::from_millis(200), || {
        std::thread::sleep(Duration::from_secs(30));
        true
    });
    assert_eq!(outcome, NativeGuardTry::TimedOut);
    assert!(start.elapsed() < Duration::from_secs(10));
    assert_eq!(
        child::finishes_within(Duration::from_secs(30), || true),
        NativeGuardTry::Finished
    );
    assert_eq!(
        child::finishes_within(Duration::from_secs(30), || false),
        NativeGuardTry::ChildFailed
    );
    assert_eq!(
        child::finishes_within(Duration::from_secs(30), || panic!("child panic")),
        NativeGuardTry::ChildFailed
    );
}

/// guard-fix: witness poles are guards like RHS denominators (index factors
/// only; parameter-only factors stay excluded), on both routes, and a rule
/// with an EMPTY RHS keeps the guards of its witness.
#[test]
fn witness_poles_join_the_guards_on_both_routes_and_for_empty_rhs() {
    use super::extract_exceptions_content;
    let context = CoefficientContext::new(["d", "n0", "n1"]);
    let mut rule = candidate(&context, &[([-1, 0], "1/(n0-1)")]);
    rule.hidden_poles = vec![
        context.coefficient_fixture("(n0-n1)*(d-4)").numerator,
        context.coefficient_fixture("n0-1").numerator,
    ];
    let want = expected(&context, &[&["n0-1"], &["n0-n1"]]);
    assert_eq!(
        extract_exceptions(&rule, &[1, 2], &[true; 2]).unwrap(),
        want
    );
    let content = extract_exceptions_content(&rule, &[1, 2], &[true; 2], &[false; 2]).unwrap();
    assert_eq!(content, want);
    let mut empty = candidate::<2>(&context, &[]);
    empty.hidden_poles = vec![context.coefficient_fixture("n1+2").numerator];
    assert_eq!(
        extract_exceptions(&empty, &[1, 2], &[true; 2]).unwrap(),
        expected(&context, &[&["n1+2"]])
    );
}
