use symbolica::{
    domains::rational_polynomial::RationalPolynomialField,
    prelude::{Integer, Z},
    tensors::sparse::{LuLMode, SparseRowReducer},
};

use crate::{
    algebra::{Coefficient, CoefficientContext},
    solver::Integral,
};

use super::*;

// The fixed forward/back, exceptional-rank, GCD-sign, and cascade outputs were
// also obtained by compiling and calling the vendored C++ solver::rowReduce
// directly with native ibp<1> rows and a FLINT DEGLEX [x, y] context.

fn order() -> IntegralOrder<1> {
    IntegralOrder::new([true], [false])
}

fn row(context: &CoefficientContext, terms: &[(i16, &str)]) -> PolynomialRow<1> {
    terms
        .iter()
        .map(|&(power, coefficient)| Term {
            integral: Integral::numeric([power]).unwrap(),
            coefficient: context.coefficient_fixture(coefficient).numerator,
        })
        .collect()
}

// Mutual containment in Symbolica's native exact-field reducer independently
// verifies generic span; the production path never constructs these fractions.
fn assert_same_exact_span(left: &[PolynomialRow<1>], right: &[PolynomialRow<1>]) {
    let mut integrals: Vec<_> = left
        .iter()
        .chain(right)
        .flatten()
        .map(|term| term.integral)
        .collect();
    integrals.sort_by(|left, right| order().compare(left, right));
    integrals.dedup();
    for (source, targets) in [(left, right), (right, left)] {
        let mut reducer = SparseRowReducer::new(
            integrals.len() as u32,
            RationalPolynomialField::new(Z),
            LuLMode::None,
        );
        for (rows, must_be_dependent) in [(source, false), (targets, true)] {
            for row in rows {
                let (columns, coefficients): (Vec<_>, Vec<_>) = row
                    .iter()
                    .map(|term| {
                        (
                            integrals
                                .iter()
                                .position(|key| key == &term.integral)
                                .unwrap() as u32,
                            Coefficient {
                                numerator: term.coefficient.clone(),
                                denominator: term.coefficient.one(),
                            },
                        )
                    })
                    .unzip();
                let pivot = reducer.add_row(&coefficients, &columns);
                if must_be_dependent {
                    assert!(
                        pivot.is_none(),
                        "preconditioning changed the exact row span"
                    );
                }
            }
        }
    }
}

#[test]
fn forward_and_backward_cancellation_preserve_polynomial_factors_and_signs() {
    let context = CoefficientContext::new(["x"]);
    let source = vec![
        row(&context, &[(3, "2*x"), (1, "1")]),
        row(&context, &[(3, "x"), (2, "1")]),
    ];
    let reduced = precondition(source.clone(), &order());
    assert_eq!(
        reduced,
        vec![
            row(&context, &[(3, "2*x"), (1, "1")]),
            row(&context, &[(2, "2"), (1, "-1")]),
        ]
    );
    assert_same_exact_span(&source, &reduced);
    // Native parsing may allocate an equal map before preconditioning even
    // starts. The solver preserves variable identities and order, not the
    // allocation identity of the coefficient fixture's external context.
    for term in source.iter().chain(&reduced).flatten() {
        assert_eq!(term.coefficient.variables(), context.variables());
    }
}

#[test]
fn zero_and_dependent_rows_are_retained_after_nonzero_rows() {
    let context = CoefficientContext::new(["x"]);
    let source = vec![
        Vec::new(),
        row(&context, &[(3, "2*x"), (1, "2")]),
        row(&context, &[(3, "x"), (1, "1")]),
        Vec::new(),
    ];
    let reduced = precondition(source.clone(), &order());
    assert_eq!(reduced.len(), source.len());
    assert_eq!(reduced[0], row(&context, &[(3, "x"), (1, "1")]));
    assert!(reduced[1..].iter().all(Vec::is_empty));
    assert_same_exact_span(&source, &reduced);
    assert_eq!(
        precondition::<1>(vec![vec![], vec![]], &order()),
        vec![vec![], vec![]]
    );
    assert!(precondition::<1>(Vec::new(), &order()).is_empty());
}

#[test]
fn native_span_check_covers_cascading_heads_and_exceptional_rank_loss() {
    let context = CoefficientContext::new(["x", "y"]);
    let source = vec![
        row(&context, &[(3, "x"), (2, "1")]),
        row(&context, &[(3, "y"), (2, "1")]),
    ];
    // y sorts before x in FLINT DEGLEX, hence the forward sign is x-y.
    let reduced = precondition(source.clone(), &order());
    assert_eq!(
        reduced,
        vec![
            row(&context, &[(3, "x*y-y^2")]),
            row(&context, &[(2, "x-y")]),
        ]
    );
    assert_same_exact_span(&source, &reduced);
    let point = [1.into(), 1.into()];
    assert!(
        source
            .iter()
            .flatten()
            .all(|term| term.coefficient.replace_all(&point) == Integer::from(1))
    );
    assert!(
        reduced
            .iter()
            .flatten()
            .all(|term| term.coefficient.replace_all(&point) == Integer::from(0))
    );

    let source = vec![
        row(&context, &[(4, "x+y"), (3, "x"), (1, "1")]),
        row(&context, &[(4, "x-y"), (2, "y"), (1, "2")]),
        row(&context, &[(3, "x^2"), (2, "x*y"), (1, "y^2")]),
        row(&context, &[(4, "2*x"), (3, "x"), (2, "y"), (1, "3")]),
    ];
    let reduced = precondition(source.clone(), &order());
    assert_same_exact_span(&source, &reduced);
    assert_eq!(
        reduced,
        vec![
            row(
                &context,
                &[(4, "2*x^3+2*x^2*y"), (1, "-x*y^2-y^3+3*x^2+3*x*y")]
            ),
            row(&context, &[(3, "2*x^3"), (1, "x*y^2+y^3-x^2-3*x*y")]),
            row(&context, &[(2, "2*x^2*y"), (1, "x*y^2-y^3+x^2+3*x*y")]),
            Vec::new(),
        ]
    );
    for (index, pivot) in reduced
        .iter()
        .enumerate()
        .filter(|(_, row)| !row.is_empty())
    {
        for (other_index, other) in reduced.iter().enumerate() {
            if index != other_index {
                assert!(other.iter().all(|term| term.integral != pivot[0].integral));
            }
        }
    }
}

#[test]
fn coefficient_ties_match_flint_term_count_monomials_then_integer_coefficients() {
    let context = CoefficientContext::new(["x", "y"]);
    let polynomials = PolynomialOrder { variables: &[0, 1] };
    let polynomial = |value| context.coefficient_fixture(value).numerator;
    assert_eq!(
        polynomials.compare(&polynomial("x^5"), &polynomial("1+x")),
        Ordering::Less
    );
    assert_eq!(
        polynomials.compare(&polynomial("x"), &polynomial("y^2")),
        Ordering::Less
    );
    assert_eq!(
        polynomials.compare(&polynomial("x+y"), &polynomial("2*x+1")),
        Ordering::Greater
    );
    assert_eq!(
        polynomials.compare(&polynomial("x+y"), &polynomial("2*x+y")),
        Ordering::Less
    );
    assert_eq!(
        polynomials.compare(&polynomial("-x"), &polynomial("x")),
        Ordering::Less
    );

    let source = vec![row(&context, &[(3, "x")]), row(&context, &[(3, "y")])];
    assert_eq!(
        precondition(source.clone(), &order())[0],
        row(&context, &[(3, "y")])
    );
    assert_eq!(
        precondition_with_variable_order(source, &order(), &[1, 0])[0],
        row(&context, &[(3, "x")])
    );
}

#[test]
fn gcd_unit_matches_flint_degree_lexicographic_leading_sign() {
    let context = CoefficientContext::new(["x", "y"]);
    let source = vec![
        row(&context, &[(3, "x-y^2"), (2, "1")]),
        row(&context, &[(3, "2*x-2*y^2"), (1, "1")]),
    ];
    let reduced = precondition(source.clone(), &order());
    assert_eq!(
        reduced,
        vec![
            row(&context, &[(3, "-2*x+2*y^2"), (1, "-1")]),
            row(&context, &[(2, "-2"), (1, "1")]),
        ]
    );
    assert_same_exact_span(&source, &reduced);

    let source = vec![
        row(&context, &[(3, "x-y"), (2, "1")]),
        row(&context, &[(3, "2*x-2*y"), (1, "1")]),
    ];
    // Changing variable priority also changes the GCD's positive-leading
    // unit, even though arithmetic retains the original native [x, y] map.
    let reduced = precondition_with_variable_order(source.clone(), &order(), &[1, 0]);
    assert_eq!(
        reduced,
        vec![
            row(&context, &[(3, "-2*x+2*y"), (1, "-1")]),
            row(&context, &[(2, "-2"), (1, "1")]),
        ]
    );
    assert_same_exact_span(&source, &reduced);
}

#[test]
#[should_panic(expected = "precondition variable order must be a permutation")]
fn rejects_an_invalid_variable_priority() {
    let context = CoefficientContext::new(["x", "y"]);
    precondition_with_variable_order(vec![row(&context, &[(1, "x")])], &order(), &[0, 0]);
}

#[test]
#[should_panic(expected = "precondition coefficients must share one ordered variable map")]
fn rejects_mixed_coefficient_maps() {
    let left = CoefficientContext::new(["x", "y"]);
    let right = CoefficientContext::new(["y", "x"]);
    precondition(
        vec![row(&left, &[(2, "x")]), row(&right, &[(1, "x")])],
        &order(),
    );
}
