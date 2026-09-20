//! Independent soundness checks for definite-quadratic case refinement.
//! Inputs are generic polynomials and cases, never production family dispatch.

use crate::algebra::CoefficientContext;
use crate::solver::{Case, CoordinateCase};

use super::super::{CaseIntersectionBudget, CaseIntersectionFailure, CaseIntersectionLimits};
use super::{QuadraticProof, classify, proves_empty};

#[test]
fn physical_axis_permutation_and_unused_variables_preserve_the_zero_set() {
    let context = CoefficientContext::new(["parameter", "b", "x", "a", "z"]);
    let equation = context.coefficient_fixture("(a-1)^2+(b+2)^2+z^2").numerator;
    let parent = Case::<4>::generic();
    assert!(!proves_empty(
        &parent,
        &equation,
        &[3, 1, 4, 2],
        &[true, false, false, true],
    ));
    assert!(!proves_empty(
        &parent,
        &equation,
        &[1, 3, 2, 4],
        &[false, true, true, false],
    ));
    assert!(proves_empty(
        &parent,
        &equation,
        &[3, 1, 4, 2],
        &[false, false, false, true],
    ));
}

#[test]
fn negative_orientation_keeps_exact_minimum_and_integer_conditions() {
    let context = CoefficientContext::new(["x", "y"]);
    let parent = Case::<2>::generic();
    for (input, expected) in [
        ("-7*((x-3)^2+(y+2)^2+1)", true),
        ("-7*((x-3)^2+(y+2)^2)", false),
        ("-7*((2*x-1)^2+(3*y+1)^2)", true),
        ("-7*(x^2+y^2-100)", false),
    ] {
        assert_eq!(
            proves_empty(
                &parent,
                &context.coefficient_fixture(input).numerator,
                &[0, 1],
                &[true, false],
            ),
            expected,
            "{input}",
        );
    }
}

#[test]
fn zero_index_is_admitted_only_on_the_nonpositive_sector_side() {
    let context = CoefficientContext::new(["x", "y"]);
    let equation = context.coefficient_fixture("x^2+y^2").numerator;
    let parent = Case::<2>::generic();
    assert!(!proves_empty(&parent, &equation, &[0, 1], &[false; 2]));
    for sector in [[true, false], [false, true], [true, true]] {
        assert!(proves_empty(&parent, &equation, &[0, 1], &sector));
    }
    let matching: Case<2> = CoordinateCase::new([Some(0), None]).unwrap().into();
    assert!(!proves_empty(&matching, &equation, &[0, 1], &[false; 2]));
    let conflicting: Case<2> = CoordinateCase::new([Some(-1), None]).unwrap().into();
    assert!(proves_empty(&conflicting, &equation, &[0, 1], &[false; 2],));
}

#[test]
fn a_negative_minimum_is_never_an_empty_domain_certificate() {
    let context = CoefficientContext::new(["x", "y"]);
    let parent = Case::<2>::generic();
    for input in ["x^2+y^2-100", "(2*x-1)^2+(2*y-1)^2-2"] {
        let equation = context.coefficient_fixture(input).numerator;
        for sector in [[false, false], [true, false], [false, true], [true, true]] {
            assert!(matches!(
                classify(&parent, &equation, &[0, 1], &sector),
                QuadraticProof::Unknown
            ));
        }
    }
}

#[test]
fn singular_indefinite_parameter_dependent_and_nonquadratic_inputs_stay_unknown() {
    let context = CoefficientContext::new(["d", "x", "y"]);
    let parent = Case::<2>::generic();
    for input in [
        "(x+y)^2+1",
        "x^2-y^2+1",
        "2*x*y+1",
        "x^2+y+1",
        "d*(x^2+y^2+1)",
        "x^2+y^2+d",
        "x^4+y^2+1",
        "x+y+1",
        "0",
    ] {
        assert!(
            matches!(
                classify(
                    &parent,
                    &context.coefficient_fixture(input).numerator,
                    &[1, 2],
                    &[true, false],
                ),
                QuadraticProof::Unknown
            ),
            "{input}",
        );
    }
}

#[test]
fn malformed_index_maps_do_not_create_a_proof() {
    let context = CoefficientContext::new(["x", "y"]);
    let equation = context.coefficient_fixture("x^2+y^2+1").numerator;
    let parent = Case::<2>::generic();
    for indices in [[0, 0], [0, 2], [usize::MAX, 1]] {
        assert!(matches!(
            classify(&parent, &equation, &indices, &[false; 2]),
            QuadraticProof::Unknown
        ));
    }
}

#[test]
fn unrelated_arity_and_quadratic_storage_ceiling_are_conservative() {
    const LARGE: usize = 65;
    let names = (0..LARGE)
        .map(|axis| format!("x{axis}"))
        .collect::<Vec<_>>();
    let context = CoefficientContext::new(names.iter().map(String::as_str));
    let input = names
        .iter()
        .map(|name| format!("{name}^2"))
        .chain(std::iter::once("1".to_owned()))
        .collect::<Vec<_>>()
        .join("+");
    let equation = context.coefficient_fixture(&input).numerator;
    assert!(!proves_empty(
        &Case::<LARGE>::generic(),
        &equation,
        &std::array::from_fn(|axis| axis),
        &[false; LARGE],
    ));
    // The limit counts supported coordinates, not family arity. A large
    // family with a small quadratic support is still eligible.
    let small_support = context.coefficient_fixture("x3^2+x61^2+1").numerator;
    assert!(proves_empty(
        &Case::<LARGE>::generic(),
        &small_support,
        &std::array::from_fn(|axis| axis),
        &[false; LARGE],
    ));
}

#[test]
fn empty_factor_cannot_hide_an_unresolved_or_sibling() {
    let context = CoefficientContext::new(["x", "y"]);
    let equation = context
        .coefficient_fixture("(x^2+y^2+1)*(x^2+y^2-1)")
        .numerator;
    let error = Case::<2>::generic()
        .intersect_many(
            &[equation.clone()],
            &[0, 1],
            &[true, false],
            Default::default(),
        )
        .unwrap_err();
    assert_eq!(error.failure, CaseIntersectionFailure::UnsupportedGeometry);
    assert_eq!(error.original_conjunction.as_ref(), &[equation]);
}

#[test]
fn existing_work_factor_and_term_budgets_cannot_be_bypassed() {
    let context = CoefficientContext::new(["x", "y"]);
    let equation = context.coefficient_fixture("x^2+y^2+1").numerator;
    for (limits, kind, limit) in [
        (
            CaseIntersectionLimits {
                max_work_items: 0,
                ..Default::default()
            },
            CaseIntersectionBudget::WorkItems,
            0,
        ),
        (
            CaseIntersectionLimits {
                max_factorizations: 0,
                ..Default::default()
            },
            CaseIntersectionBudget::Factorizations,
            0,
        ),
        (
            CaseIntersectionLimits {
                max_terms_per_conjunction: 1,
                ..Default::default()
            },
            CaseIntersectionBudget::ConjunctionTerms,
            1,
        ),
    ] {
        let error = Case::<2>::generic()
            .intersect_many(&[equation.clone()], &[0, 1], &[false; 2], limits)
            .unwrap_err();
        assert_eq!(
            error.failure,
            CaseIntersectionFailure::Budget { kind, limit }
        );
    }
}

#[test]
fn admissible_unique_minimum_propagates_into_every_nonlinear_and_sibling() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    let parse = |input: &str| context.coefficient_fixture(input).numerator;
    for reverse in [false, true] {
        for (sibling, expected) in [
            ("(x-z)^2+(y+1)^2-1", Some([Some(0); 3])),
            ("(x-z)^2+(y+1)^2", None),
        ] {
            let mut input = vec![parse("x^2+y^2"), parse(sibling)];
            if reverse {
                input.reverse();
            }
            let result = Case::<3>::generic()
                .intersect_many(&input, &[0, 1, 2], &[false; 3], Default::default())
                .unwrap();
            match expected {
                Some(fixed) => assert_eq!(
                    result.cases,
                    vec![CoordinateCase::new(fixed).unwrap().into()],
                ),
                None => assert!(result.cases.is_empty()),
            }
        }
    }
}

#[test]
fn propagated_minimum_preserves_remapped_axes_affine_parent_and_free_dimensions() {
    let context = CoefficientContext::new(["d", "w", "x", "u", "y", "v"]);
    let parse = |input: &str| context.coefficient_fixture(input).numerator;
    let indices = [2, 4, 3, 5, 1];
    let sector = [false, false, false, false, true];
    let parent = Case::<5>::generic()
        .intersect(&[parse("u-v")], &indices, &sector)
        .unwrap()
        .unwrap();
    let expected = parent
        .intersect(&[parse("x+2"), parse("y+3")], &indices, &sector)
        .unwrap()
        .unwrap();
    assert!(parent.affine().is_some());
    assert!(expected.affine().is_some());
    assert_eq!(expected.fixed()[4], None, "unrelated w must remain free");
    for input in ["(x+2)^2+7*(y+3)^2", "-13*((x+2)^2+7*(y+3)^2)"] {
        let result = parent
            .intersect_many(&[parse(input)], &indices, &sector, Default::default())
            .unwrap();
        assert_eq!(result.cases, vec![expected.clone()]);
    }
}

#[test]
fn admissible_quadratic_factor_does_not_discard_an_independent_or_sibling() {
    let context = CoefficientContext::new(["x", "y", "z"]);
    let input = context.coefficient_fixture("(x^2+y^2)*(z+1)").numerator;
    let result = Case::<3>::generic()
        .intersect_many(&[input], &[0, 1, 2], &[false; 3], Default::default())
        .unwrap();
    let expected: [Case<3>; 2] = [
        CoordinateCase::new([Some(0), Some(0), None])
            .unwrap()
            .into(),
        CoordinateCase::new([None, None, Some(-1)]).unwrap().into(),
    ];
    assert_eq!(result.cases.len(), expected.len());
    for case in expected {
        assert!(result.cases.contains(&case), "missing {case:?}");
    }
}

#[test]
fn empty_propagated_and_branch_keeps_the_other_nonempty_or_branch() {
    let context = CoefficientContext::new(["x", "y", "z", "free"]);
    let input = ["(x^2+y^2)*(z+1)", "(x-z)^2+(y+1)^2"]
        .map(|expression| context.coefficient_fixture(expression).numerator);
    let result = Case::<4>::generic()
        .intersect_many(
            &input,
            &[0, 1, 2, 3],
            &[false, false, false, true],
            Default::default(),
        )
        .unwrap();
    assert_eq!(
        result.cases,
        vec![
            CoordinateCase::new([Some(-1), Some(-1), Some(-1), None])
                .unwrap()
                .into()
        ],
    );
}

#[test]
fn zero_minimum_affine_refinement_is_charged_to_the_work_budget() {
    let context = CoefficientContext::new(["x", "y"]);
    let equation = context.coefficient_fixture("x^2+y^2").numerator;
    let parent = Case::<2>::generic();
    let error = parent
        .intersect_many(
            &[equation.clone()],
            &[0, 1],
            &[false; 2],
            CaseIntersectionLimits {
                max_work_items: 1,
                ..Default::default()
            },
        )
        .unwrap_err();
    assert_eq!(
        error.failure,
        CaseIntersectionFailure::Budget {
            kind: CaseIntersectionBudget::WorkItems,
            limit: 1,
        },
    );
    assert_eq!(error.original_parent, parent);
    assert_eq!(error.original_conjunction.as_ref(), &[equation]);
}

#[test]
fn original_captured_cubic_conjunction_keeps_its_nonempty_unsupported_sibling() {
    // External diagnostic data, not production dispatch. One normalized
    // factor branch is empty, but that is not the whole original zero locus:
    // n3=-1, n10=n14=0 admits inactive n7<=0. Check exact witnesses before
    // requiring atomic failure on the remaining unsupported OR sibling.
    use symbolica::prelude::Integer;
    let names = ["d".to_owned()]
        .into_iter()
        .chain((0..15).map(|axis| format!("n{axis}")))
        .collect::<Vec<_>>();
    let context = CoefficientContext::new(names.iter().map(String::as_str));
    let indices = std::array::from_fn(|axis| axis + 1);
    let sector = std::array::from_fn(|axis| b"111000000001110"[axis] == b'1');
    let parent: Case<15> = CoordinateCase::new([
        Some(2),
        Some(1),
        Some(1),
        None,
        Some(0),
        Some(0),
        Some(0),
        None,
        Some(0),
        Some(0),
        None,
        Some(1),
        Some(1),
        Some(1),
        None,
    ])
    .unwrap()
    .into();
    let mut input = [
        "24+6*n14-2*n10-2*n10*n14-5*n10^2-n10^2*n14-11*n7-3*n7*n14+n7*n10+n7*n10*n14+43*n3+9*n3*n14+3*n3*n10-n3*n10*n14-3*n3*n10^2-14*n3*n7-3*n3*n7*n14-n3*n7*n10+22*n3^2+3*n3^2*n14+3*n3^2*n10-3*n3^2*n7+3*n3^3",
        "9-3*n10-n10^2-4*n7+n7*n10+14*n3-2*n3*n10-4*n3*n7+5*n3^2",
    ]
    .map(|expression| context.coefficient_fixture(expression).numerator);
    for n7 in [0, -1, -37] {
        let mut point = vec![Integer::zero(); names.len()];
        for (axis, fixed) in parent.fixed().iter().enumerate() {
            point[indices[axis]] = Integer::from(fixed.unwrap_or(0));
        }
        point[indices[3]] = Integer::from(-1);
        point[indices[7]] = Integer::from(n7);
        for (axis, &variable) in indices.iter().enumerate() {
            assert_eq!(point[variable] > Integer::zero(), sector[axis]);
        }
        for equation in &input {
            assert!(equation.replace_all(&point).is_zero());
        }
    }
    for reverse in [false, true] {
        if reverse {
            input.reverse();
        }
        let error = parent
            .intersect_many(&input, &indices, &sector, Default::default())
            .unwrap_err();
        assert_eq!(error.failure, CaseIntersectionFailure::UnsupportedGeometry);
        assert_eq!(error.original_parent, parent);
        assert_eq!(error.original_conjunction.as_ref(), &input);
        assert!(error.stats.normalizations > 0);
    }
}
