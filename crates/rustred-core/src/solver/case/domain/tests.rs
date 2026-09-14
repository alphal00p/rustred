use crate::algebra::CoefficientContext;
use crate::solver::Power;

use super::*;

fn polynomials(context: &CoefficientContext, expressions: &[&str]) -> Vec<CoefficientPolynomial> {
    expressions
        .iter()
        .map(|expression| context.coefficient_fixture(expression).numerator)
        .collect()
}

fn equality<const N: usize>(context: &CoefficientContext, expressions: &[&str]) -> Case<N> {
    Case::generic()
        .intersect(
            &polynomials(context, expressions),
            &std::array::from_fn(|axis| axis),
            &[true; N],
        )
        .unwrap()
        .unwrap()
}

#[test]
fn coordinate_cases_stay_inline_and_affine_clones_share_the_native_chart() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let coordinate = Case::from(CoordinateCase::new([Some(1), None]).unwrap());
    assert!(coordinate.affine().is_none());
    assert!(coordinate.coordinate().is_some());
    let case = equality::<2>(&context, &["n0-n1"]);
    let clone = case.clone();
    let (Case::Affine(first), Case::Affine(second)) = (&case, &clone) else {
        panic!("expected shared affine cases");
    };
    assert!(Arc::ptr_eq(first, second));
    let unchanged = case.intersect(&[], &[0, 1], &[true; 2]).unwrap().unwrap();
    let Case::Affine(unchanged) = unchanged else {
        panic!("expected affine")
    };
    assert!(Arc::ptr_eq(first, &unchanged));
}

#[test]
fn intersections_promote_and_demote_exactly_without_changing_integral_axes() {
    let context = CoefficientContext::new(["n0", "n1", "n2"]);
    let parent = Case::from(CoordinateCase::new([None, None, Some(1)]).unwrap());
    let case = parent
        .intersect(
            &polynomials(&context, &["n0-2*n1-1"]),
            &[0, 1, 2],
            &[true; 3],
        )
        .unwrap()
        .unwrap();
    assert_eq!(case.fixed(), &[None, None, Some(1)]);
    assert_eq!(
        case.integral(),
        CoordinateCase::new([None, None, Some(1)])
            .unwrap()
            .integral()
    );
    assert!(case.matches(&Integral::new([
        Power::new(true, 2).unwrap(),
        Power::new(true, 1).unwrap(),
        Power::new(false, 1).unwrap(),
    ])));
    assert!(!case.matches(&Integral::symbolic([2, 1, 0]).unwrap()));
    let fixed = case
        .intersect(&polynomials(&context, &["n1-2"]), &[0, 1, 2], &[true; 3])
        .unwrap()
        .unwrap();
    assert_eq!(
        fixed.coordinate().unwrap().fixed(),
        &[Some(5), Some(2), Some(1)]
    );
    assert!(fixed.is_numerical());
    assert!(
        case.intersect(&polynomials(&context, &["n1"]), &[0, 1, 2], &[true; 3])
            .unwrap()
            .is_none()
    );
}

#[test]
fn mixed_domain_containment_is_exact_and_checks_affine_native_maps() {
    let context = CoefficientContext::new(["n0", "n1", "n2"]);
    let broad = equality::<3>(&context, &["n0-n1"]);
    let narrow = equality::<3>(&context, &["n0-n1", "n2-1"]);
    let corner: Case<3> = CoordinateCase::new([Some(2), Some(2), Some(1)])
        .unwrap()
        .into();
    let wrong: Case<3> = CoordinateCase::new([Some(2), Some(1), Some(1)])
        .unwrap()
        .into();
    let face: Case<3> = CoordinateCase::new([None, None, Some(1)]).unwrap().into();
    assert!(Case::generic().contains(&broad).unwrap());
    assert!(broad.contains(&narrow).unwrap());
    assert!(!narrow.contains(&broad).unwrap());
    assert!(narrow.contains(&corner).unwrap());
    assert!(!narrow.contains(&wrong).unwrap());
    assert!(face.contains(&narrow).unwrap());
    assert!(!face.contains(&broad).unwrap());
    assert!(matches!(
        broad.intersect(&[], &[1, 0, 2], &[true; 3]),
        Err(AffineGeometryError::InvalidInput(_))
    ));
}

#[test]
fn queue_order_matches_reference_constraint_and_coupled_equation_priorities() {
    let context = CoefficientContext::new(["n0", "n1", "n2"]);
    let generic = Case::<3>::generic();
    let equal = equality::<3>(&context, &["n0-n1"]);
    let coordinate: Case<3> = CoordinateCase::new([None, None, Some(1)]).unwrap().into();
    assert_eq!(generic.queue_cmp(&equal), Ordering::Less);
    // Total constraints tie: fewer simple equations precede more simple ones.
    assert_eq!(equal.queue_cmp(&coordinate), Ordering::Less);
    let later_pivot = equality::<3>(&context, &["n1-n2"]);
    assert_eq!(equal.queue_cmp(&later_pivot), Ordering::Less);
    let zero_at_second_axis = equality::<3>(&context, &["n0-n2"]);
    assert_eq!(equal.queue_cmp(&zero_at_second_axis), Ordering::Less);
    let more_negative = equality::<3>(&context, &["n0-2*n1"]);
    assert_eq!(more_negative.queue_cmp(&equal), Ordering::Less);
    // The reference's constants are on the LHS and nonzero precedes zero.
    let negative_constant = equality::<3>(&context, &["n0-n1-1"]);
    let positive_constant = equality::<3>(&context, &["n0-n1+1"]);
    assert_eq!(negative_constant.queue_cmp(&equal), Ordering::Less);
    assert_eq!(positive_constant.queue_cmp(&equal), Ordering::Less);
    assert_eq!(
        negative_constant.queue_cmp(&positive_constant),
        Ordering::Less
    );
}

#[test]
fn queue_order_includes_fixed_pivots_and_reverses_fixed_values_last() {
    let context = CoefficientContext::new(["n0", "n1", "n2"]);
    let first = equality::<3>(&context, &["n0-n2", "n1-1"]);
    let second = equality::<3>(&context, &["n0-n1", "n2-1"]);
    assert_eq!(first.queue_cmp(&second), Ordering::Less);
    let higher_fixed = equality::<3>(&context, &["n0-n1", "n2-2"]);
    assert_eq!(higher_fixed.queue_cmp(&second), Ordering::Less);
    let equivalent = equality::<3>(&context, &["2*n2-2", "-3*n0+3*n1"]);
    assert_eq!(second.queue_cmp(&equivalent), Ordering::Equal);
    assert_eq!(second, equivalent);
}
