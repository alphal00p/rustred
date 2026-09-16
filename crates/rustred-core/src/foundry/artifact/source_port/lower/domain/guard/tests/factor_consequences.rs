use super::conjunction::{exact_domain, expression};
use super::*;

const FG: &str = "6*n8^2+2*n8^3-4*n2*n8+2*n2*n8^2-2*n2^2*n8-3*d*n8^2+2*d*n2*n8";
const H: &str = "-24*n7^2-44*n7^3-12*n7^4+16*n0*n7+4*n0*n7^2-16*n0*n7^3+16*n0^2*n7+8*n0^2*n7^2+4*n0^3*n7+24*d*n7^2+22*d*n7^3-16*d*n0*n7-2*d*n0*n7^2-8*d*n0^2*n7-6*d^2*n7^2+4*d^2*n0*n7";
const BMW: &str = "3+n9-3*n9^2-n9^3-4*n4-6*n4*n9-2*n4*n9^2+5*n3+12*n3*n9+3*n3*n9^2+2*n3*n4-5*n3^2+n3^2*n9+2*n3^2*n4-3*n3^3-d+d*n9^2+2*d*n4+2*d*n4*n9-2*d*n3-4*d*n3*n9-2*d*n3*n4+3*d*n3^2";

fn captured_context() -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["d"]),
        "factor-consequence-captures",
        10,
    )
    .unwrap()
}

fn check_capture(
    context: &IndexedCoefficientContext,
    source: &str,
    sector: &[bool; 10],
    piece: &LatticeBox,
    target: &AffineApplicationDomain,
    exclusions: &[Arc<AffineApplicationDomain>],
) -> Result<(), SourcePortAuditError> {
    let chart = target.prepare_restriction().unwrap();
    validate_guard_on_domain_with_limits(
        context,
        &polynomial(context, &expression(context, source)),
        piece,
        sector,
        Some((target, &chart)),
        exclusions,
        Default::default(),
    )
}

#[test]
fn captured_h58_factor_consequence_retains_all_coefficients_and_real_zero() {
    let context = captured_context();
    let sector = [
        false, true, false, true, true, true, false, false, false, false,
    ];
    let fixed = [
        None,
        Some(1),
        Some(0),
        Some(1),
        Some(1),
        Some(1),
        None,
        None,
        Some(0),
        Some(0),
    ];
    let target = exact_domain(&context, &sector, fixed, &["n6-n7"]);
    let upper = [
        None,
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        None,
        None,
        Some(0),
        Some(0),
    ];
    let piece = LatticeBox::try_new([2, 0, 0, 0, 0, 0, 1, 1, 0, 0], upper).unwrap();
    check_capture(&context, H, &sector, &piece, &target, &[]).unwrap();
    // n7=0 is an actual common coefficient zero after this widening.
    let widened = LatticeBox::try_new([2, 0, 0, 0, 0, 0, 0, 0, 0, 0], upper).unwrap();
    assert!(check_capture(&context, H, &sector, &widened, &target, &[]).is_err());
}

#[test]
fn captured_fg83_factor_consequence_retains_all_coefficients_and_real_zero() {
    let context = captured_context();
    let sector = [
        true, true, false, false, true, false, true, false, false, false,
    ];
    let fixed = [
        Some(1),
        Some(1),
        None,
        Some(0),
        Some(1),
        Some(0),
        Some(1),
        None,
        None,
        Some(0),
    ];
    let target = exact_domain(&context, &sector, fixed, &["n7-n8"]);
    let upper = [
        Some(0),
        Some(0),
        None,
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        None,
        None,
        Some(0),
    ];
    let piece = LatticeBox::try_new([0, 0, 2, 0, 0, 0, 0, 1, 1, 0], upper).unwrap();
    check_capture(&context, FG, &sector, &piece, &target, &[]).unwrap();
    let widened = LatticeBox::try_new([0, 0, 2, 0, 0, 0, 0, 0, 0, 0], upper).unwrap();
    assert!(check_capture(&context, FG, &sector, &widened, &target, &[]).is_err());
}

#[test]
fn captured_bmw230_and_broader_piece_keep_whole_exclusions_in_each_chart() {
    let context = captured_context();
    let sector = [
        false, true, true, false, false, true, true, true, false, false,
    ];
    let fixed = [
        None,
        Some(1),
        Some(1),
        None,
        None,
        Some(1),
        Some(1),
        Some(1),
        Some(0),
        None,
    ];
    let target = exact_domain(&context, &sector, fixed, &["2*n0-n3-n9-1"]);
    let first = exact_domain(&context, &sector, fixed, &["n0-n9-1", "n3-n9-1"]);
    let second = exact_domain(&context, &sector, fixed, &["n0-n9", "n3-n9+1", "n4-n9+1"]);
    let actual = LatticeBox::try_new(
        [2, 0, 0, 1, 2, 0, 0, 0, 0, 0],
        [
            None,
            Some(0),
            Some(0),
            None,
            None,
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            Some(0),
        ],
    )
    .unwrap();
    check_capture(
        &context,
        BMW,
        &sector,
        &actual,
        &target,
        &[first.clone(), second.clone()],
    )
    .unwrap();
    // This larger domain contains both distinct zero components. It needs
    // aligned factor classification after the first strict-rank refinement.
    let broad = LatticeBox::try_new(
        [0; 10],
        [
            None,
            Some(0),
            Some(0),
            None,
            None,
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            None,
        ],
    )
    .unwrap();
    check_capture(
        &context,
        BMW,
        &sector,
        &broad,
        &target,
        &[first.clone(), second.clone()],
    )
    .unwrap();
    for exclusions in [vec![], vec![first], vec![second]] {
        assert!(check_capture(&context, BMW, &sector, &broad, &target, &exclusions).is_err());
    }
}

#[test]
fn affine_factor_consequence_is_generic_under_order_scaling_and_multiplicity() {
    let context = context();
    let source = "6*n1^2+2*n1^3-4*n0*n1+2*n0*n1^2-2*n0^2*n1-3*d*n1^2+2*d*n0*n1";
    for order in [[0, 1, 2], [2, 0, 1], [1, 2, 0]] {
        let mut renamed = source.to_owned();
        for axis in 0..3 {
            renamed = renamed.replace(&format!("n{axis}"), &format!("{{{axis}}}"));
        }
        for (axis, replacement) in order.into_iter().enumerate() {
            renamed = renamed.replace(&format!("{{{axis}}}"), &format!("n{replacement}"));
        }
        let mut lower = [0; 3];
        lower[order[0]] = 2;
        lower[order[1]] = 1;
        let guard = expression(&context, &format!("-7*n{}^2*({renamed})", order[1]));
        check(
            &context,
            &guard,
            &LatticeBox::try_new(lower, [None; 3]).unwrap(),
            None,
            &[],
            Default::default(),
        )
        .unwrap();
    }
}

#[test]
fn unresolved_factor_alternatives_and_mixed_exclusions_never_become_an_and() {
    let context = context();
    let piece = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    let domain = super::super::conjunction::GuardDomain {
        piece: &piece,
        sector: &[false; 3],
        target: None,
    };
    assert!(matches!(
        super::super::factors::classify(
            &context,
            &polynomial(&context, &expression(&context, "n0*n1")),
            &[],
            &domain,
            Default::default(),
            &mut Work::default(),
            &mut 0,
        )
        .unwrap(),
        super::super::factors::Consequence::Unresolved
    ));
    let guard = expression(&context, "(n0-n1)*(n1-n2)+d*(n0-n1)*(n1-n2+1)");
    let first = exact_domain(&context, &[false; 3], [None; 3], &["n0-n1", "n0+n1+n2+2"]);
    let second = exact_domain(&context, &[false; 3], [None; 3], &["n1-n2", "n0+n1+n2+2"]);
    // (n0,n1,n2)=(0,0,-1) is a genuine common zero outside both conjunctions.
    assert!(
        check(
            &context,
            &guard,
            &piece,
            None,
            &[first, second],
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn factor_work_and_guard_work_remain_shared_and_context_authenticated() {
    let context = context();
    let piece = LatticeBox::try_new([0, 1, 0], [None; 3]).unwrap();
    let domain = super::super::conjunction::GuardDomain {
        piece: &piece,
        sector: &[false; 3],
        target: None,
    };
    let coefficient = polynomial(&context, &expression(&context, "n0*n1"));
    let mut work = Work::default();
    let mut factor_work = 0;
    assert!(matches!(
        super::super::factors::classify(
            &context,
            &coefficient,
            &[],
            &domain,
            Default::default(),
            &mut work,
            &mut factor_work
        )
        .unwrap(),
        super::super::factors::Consequence::Affine(_)
    ));
    assert!(factor_work > 0 && work.operations > 0);
    let mut limits = RuleCellLimits::default();
    limits.guard_algebra.max_gcd_factor_work = factor_work;
    assert!(
        super::super::factors::classify(
            &context,
            &coefficient,
            &[],
            &domain,
            limits,
            &mut Work::default(),
            &mut factor_work
        )
        .is_err()
    );
    limits = RuleCellLimits::default();
    limits.guard_algebra.max_exact_hyperplane_replay_work = work.operations;
    assert!(
        super::super::factors::classify(
            &context,
            &coefficient,
            &[],
            &domain,
            limits,
            &mut work,
            &mut 0
        )
        .is_err()
    );
    let mut exhausted_factor_work = usize::MAX;
    assert!(
        super::super::factors::classify(
            &context,
            &coefficient,
            &[],
            &domain,
            Default::default(),
            &mut Work::default(),
            &mut exhausted_factor_work
        )
        .is_err()
    );
    let foreign = IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["d"]),
        "foreign-factor-consequence",
        3,
    )
    .unwrap();
    assert!(
        super::super::factors::classify(
            &foreign,
            &coefficient,
            &[],
            &domain,
            Default::default(),
            &mut Work::default(),
            &mut 0
        )
        .is_err()
    );
}

#[test]
fn derived_rows_share_the_original_equation_quota_before_retention() {
    let context = context();
    let system = context
        .base_coefficient_system(
            &polynomial(&context, &expression(&context, "n0*n1+d*n1*(n0+1)")),
            Default::default(),
            Default::default(),
        )
        .unwrap();
    assert_eq!(system.equations().len(), 2);
    let mut consequences = Vec::new();
    let mut limits = RuleCellLimits::default();
    limits.guard_algebra.max_coefficient_equations = 2;
    assert!(
        super::super::conjunction::retain_consequence(
            &system,
            &[],
            &mut consequences,
            polynomial(&context, &expression(&context, "n0")),
            limits,
            &mut Work::default(),
        )
        .is_err()
    );
    assert!(consequences.is_empty());
    limits.guard_algebra.max_coefficient_equations = 3;
    super::super::conjunction::retain_consequence(
        &system,
        &[],
        &mut consequences,
        polynomial(&context, &expression(&context, "n0")),
        limits,
        &mut Work::default(),
    )
    .unwrap();
    assert!(
        super::super::conjunction::retain_consequence(
            &system,
            &[],
            &mut consequences,
            polynomial(&context, &expression(&context, "n1")),
            limits,
            &mut Work::default(),
        )
        .is_err()
    );
    assert_eq!(consequences.len(), 1);
}
