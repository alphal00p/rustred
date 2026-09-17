use super::*;
use crate::algebra::{CoefficientContext, IndexedCoefficient};
use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};

mod conjunction;
mod factor_consequences;
mod joint_bounds;
mod singleton;
mod support;

fn context() -> IndexedCoefficientContext {
    IndexedCoefficientContext::try_new(&CoefficientContext::new(["d"]), "affine-guard-tests", 3)
        .unwrap()
}

fn polynomial(
    context: &IndexedCoefficientContext,
    value: &IndexedCoefficient,
) -> IndexedPolynomial {
    context
        .numerator_condition_with_limits(value, Default::default())
        .unwrap()
}

fn g(context: &IndexedCoefficientContext) -> IndexedCoefficient {
    context
        .sub(
            &context
                .add(&context.one(), &context.index(0).unwrap())
                .unwrap(),
            &context
                .mul(&context.integer(2), &context.index(1).unwrap())
                .unwrap(),
        )
        .unwrap()
}

fn domain(
    _context: &IndexedCoefficientContext,
    fixed: [Option<i16>; 3],
    equations: &[IndexedCoefficient],
    indices: [usize; 3],
) -> Arc<AffineApplicationDomain> {
    let equations = equations
        .iter()
        .map(|e| e.raw().numerator.clone())
        .collect::<Vec<_>>();
    let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
        &CoordinateCase::new(fixed).unwrap(),
        &equations,
        &indices,
        &[false; 3],
    )
    .unwrap() else {
        panic!("expected affine test domain")
    };
    Arc::new(AffineApplicationDomain::from_case(&case, &[false; 3]).unwrap())
}

fn standard_domain(
    context: &IndexedCoefficientContext,
    fixed: [Option<i16>; 3],
    equations: &[IndexedCoefficient],
) -> Arc<AffineApplicationDomain> {
    let first = context.base().variables().len();
    domain(context, fixed, equations, [first, first + 1, first + 2])
}

fn check(
    context: &IndexedCoefficientContext,
    guard: &IndexedCoefficient,
    piece: &LatticeBox,
    target: Option<&AffineApplicationDomain>,
    exclusions: &[Arc<AffineApplicationDomain>],
    limits: RuleCellLimits,
) -> Result<(), SourcePortAuditError> {
    let prepared = target.map(|t| t.prepare_restriction().unwrap());
    validate_guard_on_domain_with_limits(
        context,
        &polynomial(context, guard),
        piece,
        &[false; 3],
        target.zip(prepared.as_ref()),
        exclusions,
        limits,
    )
}

#[test]
fn product_guard_is_proved_off_complete_excluded_affine_locus() {
    let context = context();
    let g = g(&context);
    let guard = context
        .mul(
            &g,
            &context
                .sub(&context.index(2).unwrap(), &context.one())
                .unwrap(),
        )
        .unwrap();
    let excluded = standard_domain(&context, [None; 3], &[g]);
    assert!(
        check(
            &context,
            &guard,
            &LatticeBox::try_new([0; 3], [None; 3]).unwrap(),
            None,
            &[excluded],
            Default::default()
        )
        .is_ok()
    );
}

#[test]
fn product_guard_needs_all_factors_and_exclusion_needs_all_equations() {
    let context = context();
    let g = g(&context);
    let h = context
        .sub(&context.index(0).unwrap(), &context.index(2).unwrap())
        .unwrap();
    let guard = context.mul(&g, &h).unwrap();
    let excluded_g = standard_domain(&context, [None; 3], &[g.clone()]);
    assert!(
        check(
            &context,
            &guard,
            &LatticeBox::try_new([0; 3], [None; 3]).unwrap(),
            None,
            &[excluded_g],
            Default::default()
        )
        .is_err()
    );
    let conjunction = standard_domain(&context, [None; 3], &[g.clone(), h]);
    assert!(
        check(
            &context,
            &g,
            &LatticeBox::try_new([0; 3], [None; 3]).unwrap(),
            None,
            &[conjunction],
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn exclusion_fixed_face_cannot_be_ignored() {
    let context = context();
    let g = g(&context);
    let exclusion = standard_domain(&context, [None, None, Some(0)], &[g.clone()]);
    assert!(
        check(
            &context,
            &g,
            &LatticeBox::try_new([0; 3], [None; 3]).unwrap(),
            None,
            &[Arc::clone(&exclusion)],
            Default::default()
        )
        .is_err()
    );
    let fixed = LatticeBox::try_new([0; 3], [None, None, Some(0)]).unwrap();
    assert!(check(&context, &g, &fixed, None, &[exclusion], Default::default()).is_ok());
}

#[test]
fn identically_zero_guard_on_target_is_rejected() {
    let context = context();
    let g = g(&context);
    let target = standard_domain(&context, [None; 3], &[g.clone()]);
    assert!(
        check(
            &context,
            &g,
            &LatticeBox::try_new([0; 3], [None; 3]).unwrap(),
            Some(&target),
            &[],
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn affine_guard_root_outside_another_axis_bound_is_not_an_exception() {
    let context = context();
    let target = standard_domain(&context, [None; 3], &[g(&context)]);
    let guard = context.index(1).unwrap();
    // b=0 implies a=-1. The box a<=-2 excludes that coordinate child
    // while retaining the infinite target ray b<=-1, a=2b-1.
    let excludes_root = LatticeBox::try_new([2, 0, 0], [None; 3]).unwrap();
    assert!(
        check(
            &context,
            &guard,
            &excludes_root,
            Some(&target),
            &[],
            Default::default()
        )
        .is_ok()
    );
    let contains_root = LatticeBox::try_new([1, 0, 0], [None; 3]).unwrap();
    assert!(
        check(
            &context,
            &guard,
            &contains_root,
            Some(&target),
            &[],
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn rational_chart_clears_only_nonzero_content_and_preserves_roots() {
    let context = context();
    let equation = context
        .sub(
            &context
                .mul(&context.integer(2), &context.index(0).unwrap())
                .unwrap(),
            &context.index(1).unwrap(),
        )
        .unwrap();
    let target = standard_domain(&context, [None; 3], &[equation]);
    assert_eq!(target.has_integral_chart(), Some(false));
    let guard = context
        .add(&context.index(0).unwrap(), &context.one())
        .unwrap();
    // n0+1 restricts to (n1+2)/2. n1=-1 cannot be a guard zero;
    // n1=-2 can, and must not be hidden by primitive normalization.
    let miss = LatticeBox::try_new([0, 1, 0], [None, Some(1), None]).unwrap();
    let hit = LatticeBox::try_new([0, 2, 0], [None, Some(2), None]).unwrap();
    assert!(
        check(
            &context,
            &guard,
            &miss,
            Some(&target),
            &[],
            Default::default()
        )
        .is_ok()
    );
    assert!(
        check(
            &context,
            &guard,
            &hit,
            Some(&target),
            &[],
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn wrong_axis_and_native_variable_maps_fail_closed() {
    let context = context();
    let g = g(&context);
    let permuted = domain(&context, [None; 3], &[g.clone()], [2, 1, 3]);
    assert!(
        check(
            &context,
            &g,
            &LatticeBox::try_new([0; 3], [None; 3]).unwrap(),
            None,
            &[permuted],
            Default::default()
        )
        .is_err()
    );
    let foreign = IndexedCoefficientContext::try_new(
        &CoefficientContext::new(["foreign_d"]),
        "foreign-guard",
        3,
    )
    .unwrap();
    let foreign_g = super::tests::g(&foreign);
    let excluded = standard_domain(&foreign, [None; 3], &[foreign_g]);
    assert!(
        check(
            &context,
            &g,
            &LatticeBox::try_new([0; 3], [None; 3]).unwrap(),
            None,
            &[excluded],
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn affine_implication_restricts_exclusion_equations_on_target() {
    let context = context();
    // Target x=y; exclusion x=z. Guard y-z vanishes only in that
    // exclusion, but implication requires restricting x-z to y-z.
    let target_eq = context
        .sub(&context.index(0).unwrap(), &context.index(1).unwrap())
        .unwrap();
    let exclusion_eq = context
        .sub(&context.index(0).unwrap(), &context.index(2).unwrap())
        .unwrap();
    let guard = context
        .sub(&context.index(1).unwrap(), &context.index(2).unwrap())
        .unwrap();
    let target = standard_domain(&context, [None; 3], &[target_eq]);
    let exclusion = standard_domain(&context, [None; 3], &[exclusion_eq]);
    assert!(
        check(
            &context,
            &guard,
            &LatticeBox::try_new([0; 3], [None; 3]).unwrap(),
            Some(&target),
            &[exclusion],
            Default::default()
        )
        .is_ok()
    );
}

#[test]
fn affine_guard_factor_and_chart_work_budgets_are_enforced() {
    let context = context();
    let g = g(&context);
    let exclusion = standard_domain(&context, [None; 3], &[g.clone()]);
    let mut limits = RuleCellLimits::default();
    limits.guard_algebra.max_gcd_factor_work = 0;
    assert!(
        check(
            &context,
            &g,
            &LatticeBox::try_new([0; 3], [None; 3]).unwrap(),
            None,
            &[exclusion],
            limits
        )
        .is_err()
    );
    let target_eq = context
        .sub(&context.index(0).unwrap(), &context.index(1).unwrap())
        .unwrap();
    let target = standard_domain(&context, [None; 3], &[target_eq]);
    let mut limits = RuleCellLimits::default();
    limits.guard_algebra.max_exact_hyperplane_replay_work = 0;
    assert!(
        check(
            &context,
            &g,
            &LatticeBox::try_new([0; 3], [None; 3]).unwrap(),
            Some(&target),
            &[],
            limits
        )
        .is_err()
    );
}

#[test]
fn one_safe_base_coefficient_suffices_without_excluding_every_coefficient() {
    let context = context();
    let g = g(&context);
    let dimension = context
        .lift(&context.base().parameter("d").unwrap())
        .unwrap();
    let guard = context
        .add(
            &context.mul(&dimension, &g).unwrap(),
            &context.index(2).unwrap(),
        )
        .unwrap();
    let exclusion = standard_domain(&context, [None; 3], &[g]);
    assert!(
        check(
            &context,
            &guard,
            &LatticeBox::try_new([0; 3], [None; 3]).unwrap(),
            None,
            &[exclusion],
            Default::default()
        )
        .is_ok()
    );
}
