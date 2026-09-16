use super::*;
use crate::algebra::indexed::BaseCoefficientSystem;
use crate::solver::AffineGeometryError;

pub(super) fn expression(context: &IndexedCoefficientContext, source: &str) -> IndexedCoefficient {
    // Fixtures use the indexed field's stable private symbol namespace.
    context
        .parse_expression_with_limits(
            &source.replace('n', "rustred_indexed_coefficient_v1::n"),
            Default::default(),
        )
        .unwrap()
}

fn system(context: &IndexedCoefficientContext, source: &str) -> BaseCoefficientSystem {
    context
        .base_coefficient_system(
            &polynomial(context, &expression(context, source)),
            Default::default(),
            Default::default(),
        )
        .unwrap()
}

fn predicates(
    context: &IndexedCoefficientContext,
    source: &[&[&str]],
) -> Vec<Vec<IndexedPolynomial>> {
    source
        .iter()
        .map(|equations| {
            equations
                .iter()
                .map(|equation| polynomial(context, &expression(context, equation)))
                .collect()
        })
        .collect()
}

pub(super) fn exact_domain<const N: usize>(
    context: &IndexedCoefficientContext,
    sector: &[bool; N],
    fixed: [Option<i16>; N],
    equations: &[&str],
) -> Arc<AffineApplicationDomain> {
    let equations: Vec<_> = equations
        .iter()
        .map(|equation| expression(context, equation).raw().numerator.clone())
        .collect();
    let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
        &CoordinateCase::new(fixed).unwrap(),
        &equations,
        &std::array::from_fn(|axis| context.base().variables().len() + axis),
        sector,
    )
    .unwrap() else {
        panic!("fixture must retain a coupled affine domain")
    };
    Arc::new(AffineApplicationDomain::from_case(&case, sector).unwrap())
}

const CAPTURED: &str = "-3+2*n9+n9^2+4*n4+2*n4*n9-8*n3-2*n3*n9+2*n3*n4-3*n3^2+d-d*n9-2*d*n4+3*d*n3";
const RENAMED: &str = "-3+2*n2+n2^2+4*n1+2*n1*n2-8*n0-2*n0*n2+2*n0*n1-3*n0^2+d-d*n2-2*d*n1+3*d*n0";

#[test]
fn captured_joint_guard_implies_every_equation_of_retained_exclusion() {
    let context = IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["d"]),
        "joint-guard-captured",
        10,
    )
    .unwrap();
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
    let target = exact_domain(&context, &sector, fixed, &["-1-n9-n3+2*n0"]);
    let excluded = exact_domain(&context, &sector, fixed, &["-n9+n0", "1-n9+n3", "1-n9+n4"]);
    let chart = target.prepare_restriction().unwrap();
    let piece = LatticeBox::try_new(
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
    validate_guard_on_domain_with_limits(
        &context,
        &polynomial(&context, &expression(&context, CAPTURED)),
        &piece,
        &sector,
        Some((&target, &chart)),
        &[excluded],
        Default::default(),
    )
    .unwrap();
}

#[test]
fn conjunction_is_generic_under_coordinate_order_and_nonzero_scaling() {
    let context = context();
    for order in [[0, 1, 2], [2, 0, 1], [1, 2, 0]] {
        let rename = |source: &str| {
            let mut source = source.to_owned();
            for axis in 0..3 {
                source = source.replace(&format!("n{axis}"), &format!("{{{axis}}}"));
            }
            for (axis, replacement) in order.into_iter().enumerate() {
                source = source.replace(&format!("{{{axis}}}"), &format!("n{replacement}"));
            }
            source
        };
        let guard = expression(&context, &format!("7*({})", rename(RENAMED)));
        let first = rename("1-n2+n0");
        let second = rename("1-n2+n1");
        let excluded = exact_domain(&context, &[false; 3], [None; 3], &[&second, &first]);
        let piece = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
        check(
            &context,
            &guard,
            &piece,
            None,
            &[excluded],
            Default::default(),
        )
        .unwrap();
        assert!(check(&context, &guard, &piece, None, &[], Default::default()).is_err());
        let wrong = exact_domain(
            &context,
            &[false; 3],
            [None; 3],
            &[&first, &rename("2-n2+n1")],
        );
        assert!(check(&context, &guard, &piece, None, &[wrong], Default::default()).is_err());
    }
}

#[test]
fn affine_superset_proof_never_mixes_exclusion_conjunctions() {
    let context = context();
    let equations = system(&context, "n0+d*n1+d^2*n0*n2");
    let wrong = predicates(&context, &[&["n0", "n2"], &["n1", "n2"]]);
    assert!(
        !super::super::conjunction::proves_excluded(
            &context,
            &equations,
            &wrong,
            Default::default(),
            &mut Work::default(),
        )
        .unwrap()
    );
    let complete = predicates(&context, &[&["n0", "n1"]]);
    assert!(
        super::super::conjunction::proves_excluded(
            &context,
            &equations,
            &complete,
            Default::default(),
            &mut Work::default(),
        )
        .unwrap()
    );
}

#[test]
fn newly_fixed_rows_restrict_nonlinear_siblings_without_losing_values() {
    let context = context();
    let equations = system(&context, "n0+d*(n0*n1+n1)");
    let excluded = predicates(&context, &[&["n0", "n1"]]);
    assert!(
        super::super::conjunction::proves_excluded(
            &context,
            &equations,
            &excluded,
            Default::default(),
            &mut Work::default(),
        )
        .unwrap()
    );
    let large = system(&context, "n0+40000+d*(n0*n1+n1)");
    assert!(
        super::super::conjunction::proves_excluded(
            &context,
            &large,
            &excluded,
            Default::default(),
            &mut Work::default(),
        )
        .is_err()
    );
}

#[test]
fn integer_contradictions_and_unresolved_nonlinear_siblings_stay_distinct() {
    let context = context();
    let impossible = system(&context, "n0+n1+d*(n0-n1-1)");
    assert!(
        super::super::conjunction::proves_excluded(
            &context,
            &impossible,
            &[],
            Default::default(),
            &mut Work::default(),
        )
        .unwrap()
    );
    let unresolved = system(&context, "n0+d*(n1*n2-1)");
    let excluded = predicates(&context, &[&["n0", "n1"]]);
    assert!(
        !super::super::conjunction::proves_excluded(
            &context,
            &unresolved,
            &excluded,
            Default::default(),
            &mut Work::default(),
        )
        .unwrap()
    );
    let no_affine = system(&context, "n0*n1+d*n1*n2");
    let mut work = Work::default();
    assert!(
        !super::super::conjunction::proves_excluded(
            &context,
            &no_affine,
            &excluded,
            Default::default(),
            &mut work,
        )
        .unwrap()
    );
    assert_eq!(work.operations, 0);
}

#[test]
fn conjunction_work_is_shared_and_every_limit_fails_closed() {
    let context = context();
    let equations = system(&context, "n0+d*(n0*n1+n1)");
    let excluded = predicates(&context, &[&["n0", "n1"]]);
    let mut baseline = Work::default();
    assert!(
        super::super::conjunction::proves_excluded(
            &context,
            &equations,
            &excluded,
            Default::default(),
            &mut baseline,
        )
        .unwrap()
    );
    let mut limits = RuleCellLimits::default();
    limits.guard_algebra.max_exact_hyperplane_replay_work = baseline.operations;
    let mut shared = Work::default();
    assert!(
        super::super::conjunction::proves_excluded(
            &context,
            &equations,
            &excluded,
            limits,
            &mut shared,
        )
        .unwrap()
    );
    assert!(
        super::super::conjunction::proves_excluded(
            &context,
            &equations,
            &excluded,
            limits,
            &mut shared,
        )
        .is_err()
    );
    for selector in 0..6 {
        let mut limits = RuleCellLimits::default();
        match selector {
            0 => limits.guard_algebra.max_exact_hyperplane_replay_work = baseline.operations - 1,
            1 => {
                limits
                    .guard_algebra
                    .max_exact_hyperplane_replay_substitutions = 0
            }
            2 => limits.guard_algebra.max_exact_hyperplane_replay_terms = 0,
            3 => limits.guard_algebra.max_coefficient_equations = 1,
            4 => limits.guard_algebra.max_total_integer_bits = 0,
            _ => limits.indexed_algebra.max_specialization_integer_bits = 0,
        }
        assert!(
            super::super::conjunction::proves_excluded(
                &context,
                &equations,
                &excluded,
                limits,
                &mut Work::default(),
            )
            .is_err(),
            "selector {selector}"
        );
    }
    for overflow in [true, false] {
        let mut work = Work::default();
        if overflow {
            work.operations = usize::MAX;
        } else {
            work.input_terms = usize::MAX;
        }
        assert!(
            super::super::conjunction::proves_excluded(
                &context,
                &equations,
                &excluded,
                Default::default(),
                &mut work,
            )
            .is_err()
        );
    }
}

#[test]
fn native_errors_panics_and_foreign_context_never_certify_a_conjunction() {
    let context = context();
    let equations = system(&context, "n0+d*n1");
    let excluded = predicates(&context, &[&["n0", "n1"]]);
    for panic in [false, true] {
        assert!(
            super::super::conjunction::proves_excluded_using(
                &context,
                &equations,
                &excluded,
                &[],
                None,
                Default::default(),
                &mut Work::default(),
                &mut 0,
                |_, _, _| {
                    if panic {
                        panic!("injected native conjunction failure");
                    }
                    Err(AffineGeometryError::NativeAlgebra)
                },
            )
            .is_err()
        );
    }
    let foreign = IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["d"]),
        "foreign-conjunction",
        3,
    )
    .unwrap();
    assert!(
        super::super::conjunction::proves_excluded(
            &foreign,
            &equations,
            &excluded,
            Default::default(),
            &mut Work::default(),
        )
        .is_err()
    );
}

#[test]
fn captured_affine_refinement_exposes_a_root_outside_the_complete_piece() {
    let context = IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["d"]),
        "joint-guard-refined-root",
        10,
    )
    .unwrap();
    let guard = polynomial(
        &context,
        &expression(
            &context,
            "-16*n9-12*n9^2-2*n9^3+4*n7-n7*n9^2+2*n7^2+n7^2*n9-16*n0*n9-6*n0*n9^2+4*n0*n7+n0*n7^2-4*n0^2*n9+n0^2*n7+16*d*n9+6*d*n9^2-4*d*n7-d*n7^2+8*d*n0*n9-2*d*n0*n7-4*d^2*n9+d^2*n7",
        ),
    );
    let sector = [
        false, true, false, true, true, false, false, false, true, false,
    ];
    let upper = [
        Some(1),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        None,
        Some(0),
        None,
    ];
    let piece = LatticeBox::try_new([1, 0, 0, 0, 0, 0, 0, 1, 0, 1], upper).unwrap();
    validate_guard_on_domain_with_limits(
        &context,
        &guard,
        &piece,
        &sector,
        None,
        &[],
        Default::default(),
    )
    .unwrap();
    // The same guard really vanishes at n7=n9=0. Broadening only those two
    // bounds must not turn this certificate into a claim of universal safety.
    let hit = LatticeBox::try_new([1, 0, 0, 0, 0, 0, 0, 0, 0, 0], upper).unwrap();
    assert!(
        validate_guard_on_domain_with_limits(
            &context,
            &guard,
            &hit,
            &sector,
            None,
            &[],
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn captured_affine_factors_miss_the_box_but_real_factor_zeros_are_retained() {
    let context = IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["d"]),
        "joint-guard-affine-factor",
        10,
    )
    .unwrap();
    let guard = polynomial(
        &context,
        &expression(
            &context,
            "-2+4*n9+2*n9^2+8*n8-2*n8*n9-2*n8*n9^2-6*n8^2-2*n8^2*n9-2*n1+2*n1*n9+6*n1*n8-2*n1*n8*n9-4*n1*n8^2+2*d-2*d*n9-6*d*n8+2*d*n8*n9+4*d*n8^2",
        ),
    );
    let mut sector = [
        false, true, true, true, true, false, false, true, false, false,
    ];
    let piece = LatticeBox::try_new(
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 2],
        [
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            None,
            None,
        ],
    )
    .unwrap();
    validate_guard_on_domain_with_limits(
        &context,
        &guard,
        &piece,
        &sector,
        None,
        &[],
        Default::default(),
    )
    .unwrap();
    // In the adjacent positive n8 sector, n8=1 is a genuine guard zero.
    sector[8] = true;
    assert!(
        validate_guard_on_domain_with_limits(
            &context,
            &guard,
            &piece,
            &sector,
            None,
            &[],
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn sparse_native_support_bound_keeps_term_and_bit_limits() {
    let context = context();
    let equations = vec![
        expression(&context, "3*n0-2*n1-n2+1")
            .raw()
            .numerator
            .clone(),
    ];
    let (chart, primitive) =
        AffineDomainRestriction::from_equalities(&[None; 3], &equations, &[1, 2, 3])
            .unwrap()
            .unwrap();
    let input = polynomial(&context, &expression(&context, "n0^2"));
    assert_eq!(chart.restriction_term_bound(input.raw()).unwrap(), (9, 9));
    let restriction = || RestrictionTarget {
        chart: &chart,
        primitive: Some(&primitive),
        diagnostic_domain: None,
    };
    let mut limits = RuleCellLimits::default();
    limits.guard_algebra.max_input_terms = 8;
    assert!(
        restrict_prepared(
            &context,
            input.raw(),
            Some(restriction()),
            limits,
            &mut Work::default()
        )
        .unwrap_err()
        .to_string()
        .contains("prospective term budget")
    );
    limits.guard_algebra.max_input_terms = 9;
    restrict_prepared(
        &context,
        input.raw(),
        Some(restriction()),
        limits,
        &mut Work::default(),
    )
    .unwrap();
    limits.guard_algebra.max_total_integer_bits = 1;
    assert!(
        restrict_prepared(
            &context,
            input.raw(),
            Some(restriction()),
            limits,
            &mut Work::default()
        )
        .is_err()
    );
}

#[test]
fn monomial_support_counts_free_powers_and_fixed_zero_before_native_expansion() {
    let context = context();
    for leading in ["n0", "3*n0"] {
        let equations = vec![
            expression(&context, &format!("{leading}-2*n1-n2+1"))
                .raw()
                .numerator
                .clone(),
        ];
        let (chart, primitive) =
            AffineDomainRestriction::from_equalities(&[None; 3], &equations, &[1, 2, 3])
                .unwrap()
                .unwrap();
        let input = polynomial(&context, &expression(&context, "n0^2+n2^6"));
        assert_eq!(chart.restriction_term_bound(input.raw()).unwrap(), (10, 9));
        assert!(chart.restrict_equation(input.raw()).unwrap().nterms() <= 10);
        let restriction = || RestrictionTarget {
            chart: &chart,
            primitive: Some(&primitive),
            diagnostic_domain: None,
        };
        let mut limits = RuleCellLimits::default();
        limits.guard_algebra.max_input_terms = 9;
        assert!(
            restrict_prepared(
                &context,
                input.raw(),
                Some(restriction()),
                limits,
                &mut Work::default()
            )
            .unwrap_err()
            .to_string()
            .contains("prospective term budget")
        );
        limits.guard_algebra.max_input_terms = 10;
        restrict_prepared(
            &context,
            input.raw(),
            Some(restriction()),
            limits,
            &mut Work::default(),
        )
        .unwrap();
    }
    let equations = ["2*n0-n2-1", "3*n1-2*n2-1"]
        .map(|source| expression(&context, source).raw().numerator.clone());
    let (chart, _) = AffineDomainRestriction::from_equalities(&[None; 3], &equations, &[1, 2, 3])
        .unwrap()
        .unwrap();
    let input = polynomial(&context, &expression(&context, "n0^2*n1^3"));
    assert_eq!(chart.restriction_term_bound(input.raw()).unwrap(), (32, 32));
    assert!(chart.restrict_equation(input.raw()).unwrap().nterms() <= 32);
    for fixed in [0, 2] {
        let equations = vec![expression(&context, "2*n0-n1-1").raw().numerator.clone()];
        let (chart, _) = AffineDomainRestriction::from_equalities(
            &[None, None, Some(fixed)],
            &equations,
            &[1, 2, 3],
        )
        .unwrap()
        .unwrap();
        for (source, zero_bound, nonzero_bound) in [
            ("n0^3*n2^5+n1", 2, 9),
            ("n0^2+n1^6", 5, 5),
            ("n2^6", 1, 1),
            ("n0^3*n2", 1, 8),
        ] {
            let input = polynomial(&context, &expression(&context, source));
            let (bound, _) = chart.restriction_term_bound(input.raw()).unwrap();
            assert_eq!(
                bound,
                if fixed == 0 {
                    zero_bound
                } else {
                    nonzero_bound
                }
            );
            assert!(chart.restrict_equation(input.raw()).unwrap().nterms() <= bound);
        }
        let input = polynomial(&context, &expression(&context, "n0^64"));
        assert!(chart.restriction_term_bound(input.raw()).is_err());
        let foreign = IndexedCoefficientContext::try_new(
            &CoefficientContext::new(["x"]),
            "foreign-term-bound",
            3,
        )
        .unwrap();
        assert!(
            chart
                .restriction_term_bound(polynomial(&foreign, &expression(&foreign, "1")).raw())
                .is_err()
        );
        let mut malformed = input.raw().clone();
        malformed.exponents.pop();
        assert!(chart.restriction_term_bound(&malformed).is_err());
    }
}

#[test]
fn local_caps_and_exhausted_work_reject_before_native_reduction() {
    let context = context();
    let equations = system(
        &context,
        &(0..33)
            .map(|power| format!("d^{power}*n0"))
            .collect::<Vec<_>>()
            .join("+"),
    );
    let mut calls = 0;
    assert!(
        super::super::conjunction::proves_excluded_using(
            &context,
            &equations,
            &[],
            &[],
            None,
            Default::default(),
            &mut Work::default(),
            &mut 0,
            |_, _, _| {
                calls += 1;
                Err(AffineGeometryError::NativeAlgebra)
            },
        )
        .is_err()
    );
    assert_eq!(calls, 0);
    let equations = system(&context, "n0+d*n1");
    let mut limits = RuleCellLimits::default();
    limits.guard_algebra.max_exact_hyperplane_replay_work = 0;
    assert!(
        super::super::conjunction::proves_excluded_using(
            &context,
            &equations,
            &[],
            &[],
            None,
            limits,
            &mut Work::default(),
            &mut 0,
            |_, _, _| {
                calls += 1;
                Err(AffineGeometryError::NativeAlgebra)
            },
        )
        .is_err()
    );
    assert_eq!(calls, 0);
    assert!(
        super::super::conjunction::precharge_matrix(
            &[expression(&context, "n0").raw().numerator.clone()],
            usize::MAX,
            Default::default(),
            &mut Work::default(),
        )
        .is_err()
    );
}

#[test]
fn affine_box_preflight_counts_supported_finite_faces_and_wide_endpoints() {
    let context = context();
    let equation = expression(&context, "n0+2*n1-3*n2").raw().numerator.clone();
    let infinite = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    let finite = LatticeBox::try_new([0; 3], [Some(1), None, None]).unwrap();
    let mut baseline = Work::default();
    super::super::conjunction::precharge_box_equation(
        &equation,
        1,
        &infinite,
        &[false; 3],
        Default::default(),
        &mut baseline,
    )
    .unwrap();
    let mut expanded = Work::default();
    super::super::conjunction::precharge_box_equation(
        &equation,
        1,
        &finite,
        &[false; 3],
        Default::default(),
        &mut expanded,
    )
    .unwrap();
    assert!(expanded.operations > baseline.operations);
    assert!(expanded.substitutions > baseline.substitutions);
    let mut limits = RuleCellLimits::default();
    limits.guard_algebra.max_exact_hyperplane_replay_work = baseline.operations;
    assert!(
        super::super::conjunction::precharge_box_equation(
            &equation,
            1,
            &finite,
            &[false; 3],
            limits,
            &mut Work::default(),
        )
        .is_err()
    );
    let unrelated = expression(&context, "n1-n2").raw().numerator.clone();
    let mut unrelated_finite = Work::default();
    super::super::conjunction::precharge_box_equation(
        &unrelated,
        1,
        &finite,
        &[false; 3],
        Default::default(),
        &mut unrelated_finite,
    )
    .unwrap();
    // No finite supported variable: only the original pass is charged.
    assert_eq!(unrelated_finite.substitutions, 1);
    let wide = LatticeBox::try_new([u64::MAX, 0, 0], [Some(u64::MAX), None, None]).unwrap();
    let mut limits = RuleCellLimits::default();
    limits.indexed_algebra.max_specialization_integer_bits = 64;
    assert!(
        super::super::conjunction::precharge_box_equation(
            &equation,
            1,
            &wide,
            &[true, false, false],
            limits,
            &mut Work::default(),
        )
        .is_err()
    );
}
