use symbolica::poly::groebner::GroebnerBasis;
use symbolica::poly::{GrevLexOrder, LexOrder};
use symbolica::prelude::{Q, Rational};

use crate::algebra::{CoefficientContext, CoefficientPolynomial};
use crate::solver::{Case, CaseIntersectionLimits, CoordinateCase};

use super::{canonicalize, normalize};

fn equations(context: &CoefficientContext, expressions: &[&str]) -> Vec<CoefficientPolynomial> {
    expressions
        .iter()
        .map(|expression| context.coefficient_fixture(expression).numerator)
        .collect()
}

fn same_ideal(original: &[CoefficientPolynomial], normalized: &[CoefficientPolynomial]) {
    let original: Vec<_> = original
        .iter()
        .map(|p| {
            p.map_coeff(|v| Rational::from(v), Q)
                .reorder::<GrevLexOrder>()
        })
        .collect();
    let normalized: Vec<_> = normalized
        .iter()
        .map(|p| {
            p.map_coeff(|v| Rational::from(v), Q)
                .reorder::<GrevLexOrder>()
        })
        .collect();
    let original_basis = GroebnerBasis::new(&original, false);
    let normalized_basis = GroebnerBasis::new(&normalized, false);
    for p in &original {
        assert!(p.reduce(&normalized_basis.system).is_zero());
    }
    for p in &normalized {
        assert!(p.reduce(&original_basis.system).is_zero());
    }
}

#[test]
fn grevlex_generators_keep_lex_storage_without_claiming_a_lex_basis() {
    let context = CoefficientContext::new(["x", "y"]);
    let original = equations(&context, &["x*y-1", "y^2-x"]);
    let result = normalize(&original).unwrap();
    let mut expected = equations(&context, &["x^2-y", "x*y-1", "x-y^2"]);
    canonicalize(&mut expected);
    assert_eq!(result, expected);
    assert!(
        result
            .iter()
            .all(|p| p.variables() == original[0].variables())
    );
    let grevlex: Vec<_> = result
        .iter()
        .map(|p| {
            p.map_coeff(|v| Rational::from(v), Q)
                .reorder::<GrevLexOrder>()
        })
        .collect();
    assert!(GroebnerBasis::is_groebner_basis(&grevlex));
    let lex: Vec<_> = grevlex.iter().map(|p| p.reorder::<LexOrder>()).collect();
    assert!(!GroebnerBasis::is_groebner_basis(&lex));
    same_ideal(&original, &result);
}

#[test]
fn complete_conjunction_is_preserved_with_primitive_integer_content() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    for input in [
        vec!["-65537*(x-y)*(z-1)", "257*(x-y)*(z-2)"],
        vec!["2*x^2+2*y^2-10", "-3*x^2+3*y^2-9"],
        vec!["2*x-3*y", "x*y-6"],
    ] {
        let original = equations(&context, &input);
        let result = normalize(&original).unwrap();
        assert!(!result.is_empty());
        same_ideal(&original, &result);
        for p in &result {
            assert!(!p.lcoeff().is_negative());
            assert_eq!(p.clone().make_primitive(), *p);
            assert_eq!(p.variables(), original[0].variables());
        }
    }
}

#[test]
fn empty_zero_and_inconsistent_ideals_stay_exact() {
    let context = CoefficientContext::new(["x", "y"]);
    assert!(normalize(&[]).unwrap().is_empty());
    // The intersection engine discards zero equations before native F4.
    let zero = Case::<2>::generic()
        .intersect_many(
            &equations(&context, &["0", "0"]),
            &[0, 1],
            &[true; 2],
            CaseIntersectionLimits::default(),
        )
        .unwrap();
    assert_eq!(zero.cases, vec![Case::<2>::generic()]);
    for input in [vec!["-17"], vec!["x*y-1", "x"]] {
        let original = equations(&context, &input);
        let result = normalize(&original).unwrap();
        assert_eq!(result, equations(&context, &["1"]));
        same_ideal(&original, &result);
    }
}

#[test]
fn normalized_affine_consequence_keeps_parent_and_every_sibling() {
    let context = CoefficientContext::new(["x", "y", "z", "w"]);
    let parent: Case<4> = CoordinateCase::new([None, None, None, Some(3)])
        .unwrap()
        .into();
    let conjunction = equations(&context, &["(x-y)*(z-1)", "(x-y)*(z-2)", "z^2-w^2"]);
    let result = parent
        .intersect_many(
            &conjunction,
            &[0, 1, 2, 3],
            &[true; 4],
            CaseIntersectionLimits::default(),
        )
        .unwrap();
    let expected = Case::<4>::generic()
        .intersect(
            &equations(&context, &["x-y", "z-3", "w-3"]),
            &[0, 1, 2, 3],
            &[true; 4],
        )
        .unwrap()
        .unwrap();
    assert_eq!(result.cases, vec![expected]);
    assert!(result.stats.normalizations > 0);
}
