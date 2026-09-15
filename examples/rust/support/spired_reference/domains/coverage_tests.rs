//! Coverage-only comparisons reuse the exact-domain/RHS fixtures next door.
use super::*;
use rustred::solver::CaseIntersectionLimits;

fn generic_reference(excluded: &str) -> String {
    format!("int[n1_?Positive,n2_?Positive]/;!({excluded})->(1)*int[-1+n1,n2]")
}

fn fixed_rule(context: &CoefficientContext, a: Option<i16>, b: Option<i16>) -> SectorRule<2> {
    let case: Case<2> = CoordinateCase::new([a, b]).unwrap().into();
    let mut candidate = rule(case, context.one(), Vec::new());
    candidate.candidate.rhs[0].integral = Integral::new([
        Power::new(a.is_none(), a.map_or(-1, |a| a - 1)).unwrap(),
        Power::new(b.is_none(), b.unwrap_or(0)).unwrap(),
    ]);
    candidate
}

#[test]
fn redundant_child_is_covered_by_a_broader_already_matched_rule() {
    let (context, system) = fixture();
    let a = context.parameter("a").unwrap();
    let b = context.parameter("b").unwrap();
    let candidate = rule(
        Case::generic(),
        context.one(),
        vec![vec![(&(&a + &b) - &context.one()).numerator]],
    );
    let reference = format!(
        "{{{},int[n1_?Positive,1]/;!(n1==1)->(1)*int[-1+n1,1]}}",
        generic_reference("n1+n2==1")
    );
    let report =
        compare_sector_with_aliases(&reference, &[candidate], &system, &[true; 2], None, &[])
            .unwrap();
    assert_eq!(report.matched_rules, 1);
    assert_eq!(report.covered_reference_rules, 1);
    assert_eq!(report.integer_empty_rules, 0);
}

#[test]
fn every_exceptional_intersection_must_be_covered() {
    let (context, system) = fixture();
    let a = context.parameter("a").unwrap();
    let b = context.parameter("b").unwrap();
    let generic = rule(
        Case::generic(),
        context.one(),
        vec![
            vec![(&a - &context.one()).numerator],
            vec![(&b - &context.one()).numerator],
        ],
    );
    let diagonal = entry("n1==n2", "False", "1");
    let reference = format!(
        "{{{},{diagonal},int[1,1]->(1)*int[0,1]}}",
        generic_reference("(n1==1)||(n2==1)")
    );
    // The two OR faces both intersect the required diagonal at the same corner.
    // Either candidate order must find the generic rule and then the corner.
    let corner = fixed_rule(&context, Some(1), Some(1));
    let mut candidates = [generic, corner];
    for _ in 0..2 {
        let report =
            compare_sector_with_aliases(&reference, &candidates, &system, &[true; 2], None, &[])
                .unwrap();
        assert_eq!(report.matched_rules, 2);
        assert_eq!(report.covered_reference_rules, 1);
        candidates.reverse();
    }
    let missing = format!("{{{},{diagonal}}}", generic_reference("(n1==1)||(n2==1)"));
    assert!(
        compare_sector_with_aliases(&missing, &candidates[..1], &system, &[true; 2], None, &[])
            .is_err()
    );
}

#[test]
fn reference_exclusions_remove_only_their_exact_covered_holes() {
    let (context, system) = fixture();
    let a = context.parameter("a").unwrap();
    let generic = rule(
        Case::generic(),
        context.one(),
        vec![vec![(&a - &context.one()).numerator]],
    );
    for (excluded, expected) in [("n2==1", true), ("n2==2", false)] {
        let reference = format!(
            "{{{},{}}}",
            generic_reference("n1==1"),
            entry("n1==n2", excluded, "1")
        );
        assert_eq!(
            compare_sector_with_aliases(
                &reference,
                std::slice::from_ref(&generic),
                &system,
                &[true; 2],
                None,
                &[]
            )
            .is_ok(),
            expected
        );
    }
}

#[test]
fn distinct_exceptional_siblings_are_not_dropped_after_one_is_covered() {
    let (context, system) = fixture();
    let a = context.parameter("a").unwrap();
    let b = context.parameter("b").unwrap();
    let generic = rule(
        Case::generic(),
        context.one(),
        vec![
            vec![(&a - &context.one()).numerator],
            vec![(&b - &context.one()).numerator],
        ],
    );
    let reference = format!(
        "{{{},{},int[1,2]->(1)*int[0,2],int[2,1]->(1)*int[1,1]}}",
        generic_reference("(n1==1)||(n2==1)"),
        entry("n1+n2==3", "False", "1"),
    );
    let first = fixed_rule(&context, Some(1), Some(2));
    let second = fixed_rule(&context, Some(2), Some(1));
    let candidates = [first, generic, second];
    let report =
        compare_sector_with_aliases(&reference, &candidates, &system, &[true; 2], None, &[])
            .unwrap();
    assert_eq!(report.covered_reference_rules, 1);
    assert_eq!(report.matched_rules, 3);
    let missing = reference.replace(",int[2,1]->(1)*int[1,1]", "");
    assert!(
        compare_sector_with_aliases(&missing, &candidates[..2], &system, &[true; 2], None, &[],)
            .is_err()
    );
}

#[test]
fn coverage_cannot_rescue_wrong_matched_rhs_guards_or_sector_annotations() {
    let (context, system) = fixture();
    let reference = format!(
        "{{{},int[n1_?Positive,1]->(1)*int[-1+n1,1]}}",
        generic_reference("False")
    );
    let wrong_rhs = rule(Case::generic(), context.integer(2), Vec::new());
    assert!(
        compare_sector_with_aliases(&reference, &[wrong_rhs], &system, &[true; 2], None, &[])
            .is_err()
    );
    let a = context.parameter("a").unwrap();
    let wrong_guard = rule(
        Case::generic(),
        context.one(),
        vec![vec![(&a - &context.one()).numerator]],
    );
    assert!(
        compare_sector_with_aliases(&reference, &[wrong_guard], &system, &[true; 2], None, &[])
            .is_err()
    );
    let wrong_sign = reference.replace("n1_?Positive,1", "n1_?NonPositive,1");
    let candidate = rule(Case::generic(), context.one(), Vec::new());
    assert!(
        compare_sector_with_aliases(&wrong_sign, &[candidate], &system, &[true; 2], None, &[])
            .is_err()
    );
}

#[test]
fn full_self_exclusion_makes_no_progress_and_malformed_geometry_fails_closed() {
    let (context, system) = fixture();
    let generic = rule(Case::generic(), context.one(), vec![Vec::new()]);
    let reference = format!(
        "{{{},int[n1_?Positive,1]->(1)*int[-1+n1,1]}}",
        generic_reference("True")
    );
    assert!(
        compare_sector_with_aliases(&reference, &[generic], &system, &[true; 2], None, &[])
            .is_err()
    );
    let candidate = rule(Case::generic(), context.one(), Vec::new());
    for excluded in ["n1<2", "n1^2+n2^2==6"] {
        let reference = format!(
            "{{{},{}}}",
            generic_reference("False"),
            entry("n1==n2+1", excluded, "1")
        );
        assert!(
            compare_sector_with_aliases(
                &reference,
                std::slice::from_ref(&candidate),
                &system,
                &[true; 2],
                None,
                &[]
            )
            .is_err()
        );
    }
}

#[test]
fn only_proper_subcases_do_not_prove_a_finite_affine_domain_covered() {
    let (context, system) = fixture();
    let reference = format!(
        "{{int[1,2]->(1)*int[0,2],int[2,1]->(1)*int[1,1],{}}}",
        entry("n1+n2==3", "False", "1")
    );
    let candidates = [
        fixed_rule(&context, Some(1), Some(2)),
        fixed_rule(&context, Some(2), Some(1)),
    ];
    // This finite integer line really is covered, but proving that would need
    // a stronger sector-inequality service. The narrow oracle must reject it.
    assert!(
        compare_sector_with_aliases(&reference, &candidates, &system, &[true; 2], None, &[])
            .is_err()
    );
}

#[test]
fn coverage_work_budget_never_turns_unprocessed_cases_into_success() {
    let (context, system) = fixture();
    let lhs = "int[n1_?Positive,1]";
    let reference = parse_entry(lhs, "(1)*int[-1+n1,1]", &system, Some(&[true; 2]))
        .unwrap()
        .unwrap();
    let candidates = [rule(Case::generic(), context.one(), Vec::new())];
    let error = super::super::coverage::covered_by_matched_rules(
        &reference,
        &candidates,
        &system,
        &[true; 2],
        CaseIntersectionLimits {
            max_work_items: 0,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("budget"));
}
