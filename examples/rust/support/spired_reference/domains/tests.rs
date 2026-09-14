use super::*;
use rustred::algebra::CoefficientContext;
use rustred::solver::Term;

fn fixture() -> (CoefficientContext, SourceSystem<2>) {
    let context = CoefficientContext::try_new(["d", "a", "b"]).unwrap();
    let system = SourceSystem::new(
        vec![vec![Term {
            integral: Integral::symbolic([0, 0]).unwrap(),
            coefficient: context.one().numerator,
        }]],
        [1, 2],
    )
    .unwrap();
    (context, system)
}

fn diagonal(context: &CoefficientContext, system: &SourceSystem<2>) -> Case<2> {
    let equation = (&context.parameter("a").unwrap() - &context.parameter("b").unwrap()).numerator;
    Case::from(CoordinateCase::generic())
        .intersect(&[equation], system.index_variables(), &[true; 2])
        .unwrap()
        .unwrap()
}

fn rule(
    case: Case<2>,
    coefficient: Coefficient,
    branches: Vec<Vec<CoefficientPolynomial>>,
) -> SectorRule<2> {
    SectorRule {
        candidate: RuleCandidate {
            target: case.integral(),
            case,
            rhs: vec![Term {
                integral: Integral::symbolic([-1, 0]).unwrap(),
                coefficient,
            }],
            sources: Vec::new(),
            stats: SearchStats::default(),
        },
        exceptions: ExceptionalConditions { branches },
    }
}

fn entry(required: &str, excluded: &str, coefficient: &str) -> String {
    format!(
        "int[n1_?Positive,n2_?Positive]/;{required}&&!({excluded})->({coefficient})*int[-1+n1,n2]"
    )
}

#[test]
fn affine_coefficients_compare_after_exact_required_case_restriction() {
    let (context, system) = fixture();
    let a = context.parameter("a").unwrap();
    let b = context.parameter("b").unwrap();
    let candidate = rule(
        diagonal(&context, &system),
        &b + &context.one(),
        vec![vec![(&a - &context.one()).numerator]],
    );
    let reference = format!("{{{}}}", entry("n1-n2==0", "n2==1", "n1+1"));
    compare_rule(&reference, &candidate, &system, &[true; 2], None).unwrap();
    compare(&reference, &candidate.candidate, &system, None).unwrap();
    for (required, coefficient) in [("n1-n2==1", "n1+1"), ("n1-n2==0", "n1+2")] {
        let wrong = format!("{{{}}}", entry(required, "n2==1", coefficient));
        assert!(compare_rule(&wrong, &candidate, &system, &[true; 2], None).is_err());
    }
}

#[test]
fn affine_guards_compare_domains_after_intersection_not_raw_expressions() {
    let (context, system) = fixture();
    let b = context.parameter("b").unwrap();
    let candidate = rule(
        diagonal(&context, &system),
        context.one(),
        vec![vec![(&b - &context.one()).numerator]],
    );
    for excluded in ["n1==1", "n2==1", "n1+n2==2", "(n1==1)||(n2==1)"] {
        let reference = format!("{{{}}}", entry("2*n1-2*n2==0", excluded, "1"));
        compare_rule(&reference, &candidate, &system, &[true; 2], None).unwrap();
    }
    for excluded in ["n2==2", "False", "n1-n2==0"] {
        let reference = format!("{{{}}}", entry("n1==n2", excluded, "1"));
        assert!(compare_rule(&reference, &candidate, &system, &[true; 2], None).is_err());
    }
}

#[test]
fn full_sector_distinguishes_affine_cases_with_identical_fixed_patterns() {
    let (context, system) = fixture();
    let a = context.parameter("a").unwrap();
    let b = context.parameter("b").unwrap();
    let generic = rule(
        CoordinateCase::generic().into(),
        &a + &context.one(),
        vec![vec![(&a - &b).numerator]],
    );
    let affine = rule(
        diagonal(&context, &system),
        &b + &context.one(),
        vec![vec![(&b - &context.one()).numerator]],
    );
    let reference = format!(
        "{{int[n1_?Positive,n2_?Positive]/;!(n1==n2)->(n1+1)*int[-1+n1,n2],{}}}",
        entry("n1-n2==0", "n2==1", "n1+1")
    );
    let report = compare_sector_with_aliases(
        &reference,
        &[generic, affine],
        &system,
        &[true; 2],
        None,
        &[],
    )
    .unwrap();
    assert_eq!(
        report,
        SectorComparison {
            matched_rules: 2,
            integer_empty_rules: 0
        }
    );
}

#[test]
fn integer_empty_affine_reference_rule_is_omitted_with_exact_proof() {
    let (context, system) = fixture();
    let candidate = rule(CoordinateCase::generic().into(), context.one(), Vec::new());
    let reference = format!(
        "{{int[n1_?Positive,n2_?Positive]->(1)*int[-1+n1,n2],{}}}",
        entry("2*n1-2*n2==1", "n2==1", "1")
    );
    let report =
        compare_sector_with_aliases(&reference, &[candidate], &system, &[true; 2], None, &[])
            .unwrap();
    assert_eq!(
        report,
        SectorComparison {
            matched_rules: 1,
            integer_empty_rules: 1
        }
    );
    let nonempty = reference.replace("2*n1-2*n2==1", "n1-n2==1");
    let candidate = rule(CoordinateCase::generic().into(), context.one(), Vec::new());
    assert!(
        compare_sector_with_aliases(&nonempty, &[candidate], &system, &[true; 2], None, &[])
            .is_err()
    );
}

#[test]
fn duplicate_and_missing_nonempty_cases_fail_whole_sector_validation() {
    let (context, system) = fixture();
    let item = entry("n1==n2", "False", "1");
    let duplicate = format!("{{{item},{item}}}");
    let candidate = rule(diagonal(&context, &system), context.one(), Vec::new());
    assert!(
        compare_sector_with_aliases(&duplicate, &[candidate], &system, &[true; 2], None, &[])
            .is_err()
    );
    assert!(
        compare_sector_with_aliases(&format!("{{{item}}}"), &[], &system, &[true; 2], None, &[])
            .is_err()
    );
}

#[test]
fn coefficient_poles_on_required_affine_case_are_not_silently_cancelled() {
    let (context, system) = fixture();
    let candidate = rule(diagonal(&context, &system), context.one(), Vec::new());
    let reference = format!("{{{}}}", entry("n1==n2", "False", "1/(n1-n2)"));
    assert!(compare_rule(&reference, &candidate, &system, &[true; 2], None).is_err());
}

#[test]
fn required_equalities_may_fix_coordinates_without_rewriting_free_integral_axes() {
    let (context, system) = fixture();
    let case: Case<2> = CoordinateCase::new([Some(2), None]).unwrap().into();
    let mut candidate = rule(case, context.integer(3), Vec::new());
    candidate.candidate.rhs[0].integral =
        Integral::new([Power::new(false, 1).unwrap(), Power::new(true, 0).unwrap()]);
    let reference = format!("{{{}}}", entry("n1==2", "False", "n1+1"));
    compare_rule(&reference, &candidate, &system, &[true; 2], None).unwrap();
}

#[test]
fn affine_equality_does_not_identify_different_physical_integral_axes() {
    let (context, system) = fixture();
    let mut candidate = rule(diagonal(&context, &system), context.one(), Vec::new());
    let reference = format!("{{{}}}", entry("n1==n2", "False", "1"));
    compare_rule(&reference, &candidate, &system, &[true; 2], None).unwrap();
    candidate.candidate.rhs[0].integral = Integral::symbolic([0, -1]).unwrap();
    assert!(compare_rule(&reference, &candidate, &system, &[true; 2], None).is_err());
}

#[test]
fn separate_exception_negations_are_not_a_negated_conjunction() {
    let (context, system) = fixture();
    let a = context.parameter("a").unwrap();
    let b = context.parameter("b").unwrap();
    let candidate = rule(
        CoordinateCase::generic().into(),
        context.one(),
        vec![
            vec![(&a - &context.one()).numerator],
            vec![(&b - &context.one()).numerator],
        ],
    );
    let separate = "{int[n1_?Positive,n2_?Positive]/;!(n1==1)&&!(n2==1)->(1)*int[-1+n1,n2]}";
    compare_rule(separate, &candidate, &system, &[true; 2], None).unwrap();
    let conjunction = "{int[n1_?Positive,n2_?Positive]/;!((n1==1)&&(n2==1))->(1)*int[-1+n1,n2]}";
    assert!(compare_rule(conjunction, &candidate, &system, &[true; 2], None).is_err());
}

#[test]
fn duplicate_candidate_cases_cannot_replace_a_missing_reference_case() {
    let (context, system) = fixture();
    let reference = format!(
        "{{int[n1_?Positive,n2_?Positive]->(1)*int[-1+n1,n2],{}}}",
        entry("n1==n2", "False", "1")
    );
    let candidates = [
        rule(CoordinateCase::generic().into(), context.one(), Vec::new()),
        rule(CoordinateCase::generic().into(), context.one(), Vec::new()),
    ];
    let error =
        compare_sector_with_aliases(&reference, &candidates, &system, &[true; 2], None, &[])
            .unwrap_err();
    assert!(error.to_string().contains("duplicate candidate"));
}
