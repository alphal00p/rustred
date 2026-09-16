use crate::algebra::CoefficientContext;
use crate::solver::{Integral, Term};
use symbolica::prelude::Integer;

use super::*;

fn row(context: &CoefficientContext, terms: &[(i16, &str)]) -> PolynomialRow<1> {
    terms
        .iter()
        .map(|&(power, value)| Term {
            integral: Integral::numeric([power]).unwrap(),
            coefficient: context.coefficient_fixture(value).numerator,
        })
        .collect()
}

fn assert_identities(
    source: &[PolynomialRow<1>],
    reduced: &[PolynomialRow<1>],
    trace: &PreconditionProvenance,
    context: &CoefficientContext,
    transform: impl Fn(&CoefficientPolynomial) -> Coefficient,
) {
    let mut columns: Vec<_> = source
        .iter()
        .chain(reduced)
        .flatten()
        .map(|term| term.integral)
        .collect();
    columns.sort_by(|left, right| IntegralOrder::new([true], [false]).compare(left, right));
    columns.dedup();
    for (root, target) in reduced.iter().enumerate() {
        let weights = trace
            .compose(&[(root, context.one())], &context.one(), |scale| {
                Ok(transform(scale))
            })
            .unwrap();
        assert_eq!(weights.len(), source.len());
        for column in &columns {
            let mut actual = context.zero();
            for (row, weight) in source.iter().zip(&weights) {
                if let Some(term) = row.iter().find(|term| &term.integral == column) {
                    actual = &actual + &(weight * &transform(&term.coefficient));
                }
            }
            let expected = target
                .iter()
                .find(|term| &term.integral == column)
                .map_or_else(|| context.zero(), |term| transform(&term.coefficient));
            assert_eq!(actual, expected, "root {root}, column {column:?}");
        }
    }
}

#[test]
fn traced_output_matches_existing_forward_backward_and_gcd_unit_fixtures() {
    let context = CoefficientContext::new(["x", "y"]);
    let order = IntegralOrder::new([true], [false]);
    let fixtures = [
        vec![
            row(&context, &[(3, "2*x"), (1, "1")]),
            row(&context, &[(3, "x"), (2, "1")]),
        ],
        vec![
            row(&context, &[(3, "x-y^2"), (2, "1")]),
            row(&context, &[(3, "2*x-2*y^2"), (1, "1")]),
        ],
        vec![
            row(&context, &[(4, "x+y"), (3, "x"), (1, "1")]),
            row(&context, &[(4, "x-y"), (2, "y"), (1, "2")]),
            row(&context, &[(3, "x^2"), (2, "x*y"), (1, "y^2")]),
            row(&context, &[(4, "2*x"), (3, "x"), (2, "y"), (1, "3")]),
        ],
    ];
    for source in fixtures {
        for priority in [[0, 1], [1, 0]] {
            let expected =
                super::super::precondition_with_variable_order(source.clone(), &order, &priority);
            let (reduced, trace) = precondition_with_provenance(source.clone(), &order, &priority);
            assert_eq!(reduced, expected);
            assert_eq!(format!("{reduced:?}"), format!("{expected:?}"));
            assert_identities(&source, &reduced, &trace, &context, |p| p.clone().into());
            for (id, node) in trace.nodes.iter().enumerate() {
                if let Node::Subtract { left, right, .. } = node {
                    assert!(left.0 < id && right.0 < id, "immutable prior versions only");
                }
            }
        }
    }
}

#[test]
fn empty_duplicate_and_dependent_rows_replay_without_ambiguous_row_matching() {
    let context = CoefficientContext::new(["x"]);
    let order = IntegralOrder::new([true], [false]);
    let source = vec![
        vec![],
        row(&context, &[(3, "2*x"), (1, "2")]),
        row(&context, &[(3, "x"), (1, "1")]),
        row(&context, &[(3, "x"), (1, "1")]),
        vec![],
    ];
    let (reduced, trace) = precondition_with_provenance(source.clone(), &order, &[0]);
    assert_eq!(reduced, super::super::precondition(source.clone(), &order));
    assert_identities(&source, &reduced, &trace, &context, |p| p.clone().into());
    for source in [Vec::new(), vec![Vec::new(), Vec::new()]] {
        let (reduced, trace) = precondition_with_provenance(source.clone(), &order, &[]);
        assert_eq!(reduced, source);
        assert_eq!(trace.nodes.len(), source.len());
        assert_identities(&source, &reduced, &trace, &context, |p| p.clone().into());
    }
}

#[test]
fn specialized_rank_loss_does_not_invalidate_the_forward_polynomial_identity() {
    let context = CoefficientContext::new(["x", "y"]);
    let order = IntegralOrder::new([true], [false]);
    let source = vec![
        row(&context, &[(3, "x"), (2, "1")]),
        row(&context, &[(3, "y"), (2, "1")]),
    ];
    let (reduced, trace) = precondition_with_provenance(source.clone(), &order, &[0, 1]);
    let point = [Integer::from(1), Integer::from(1)];
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
    assert_identities(&source, &reduced, &trace, &context, |p| {
        p.constant(p.replace_all(&point)).into()
    });
}

#[test]
fn repeated_basis_weights_cancel_before_visiting_unneeded_edges() {
    let context = CoefficientContext::new(["x", "y"]);
    let order = IntegralOrder::new([true], [false]);
    let source = vec![
        row(&context, &[(3, "x"), (2, "1")]),
        row(&context, &[(3, "y"), (2, "1")]),
    ];
    let (_, trace) = precondition_with_provenance(source, &order, &[0, 1]);
    let value = context.coefficient_fixture("1/x");
    let result = trace.compose(&[(0, value.clone()), (0, -value)], &context.one(), |_| {
        panic!("cancelled adjoint")
    });
    assert_eq!(result.unwrap(), vec![context.zero(), context.zero()]);
}

#[test]
fn native_translation_composes_two_rationally_weighted_basis_roots() {
    let context = CoefficientContext::new(["x", "y"]);
    let order = IntegralOrder::new([true], [false]);
    let source = vec![
        row(&context, &[(3, "x"), (2, "1")]),
        row(&context, &[(3, "y"), (2, "1")]),
    ];
    let (basis, trace) = precondition_with_provenance(source.clone(), &order, &[0, 1]);
    let weights = [
        (0, context.coefficient_fixture("1/(x+y)")),
        (1, context.coefficient_fixture("(x-2)/(y+1)")),
    ];
    let transform = |polynomial: &CoefficientPolynomial| {
        crate::solver::translate_source_port(&polynomial.clone().into(), &[0], &[2])
    };
    let original = trace
        .compose(&weights, &context.one(), |scale| Ok(transform(scale)))
        .unwrap();
    for power in [3, 2] {
        let key = Integral::numeric([power]).unwrap();
        let coefficient = |row: &PolynomialRow<1>| {
            row.iter()
                .find(|term| term.integral == key)
                .map_or_else(|| context.zero(), |term| transform(&term.coefficient))
        };
        let actual = source
            .iter()
            .zip(&original)
            .fold(context.zero(), |sum, (row, weight)| {
                &sum + &(weight * &coefficient(row))
            });
        let expected = weights.iter().fold(context.zero(), |sum, (root, weight)| {
            &sum + &(weight * &coefficient(&basis[*root]))
        });
        assert_eq!(actual, expected);
    }
}

#[test]
fn invalid_roots_maps_and_failed_scale_transforms_are_rejected() {
    let context = CoefficientContext::new(["x", "y"]);
    let foreign = CoefficientContext::new(["foreign"]);
    let order = IntegralOrder::new([true], [false]);
    let source = vec![
        row(&context, &[(3, "x"), (2, "1")]),
        row(&context, &[(3, "y"), (2, "1")]),
    ];
    let (_, trace) = precondition_with_provenance(source, &order, &[0, 1]);
    assert!(
        trace
            .compose(&[(2, context.one())], &context.one(), |p| Ok(p
                .clone()
                .into()))
            .is_err()
    );
    assert!(
        trace
            .compose(&[(0, foreign.one())], &context.one(), |p| Ok(p
                .clone()
                .into()))
            .is_err()
    );
    assert!(
        trace
            .compose(&[(0, context.one())], &context.one(), |_| Ok(foreign.one()))
            .is_err()
    );
    assert!(
        trace
            .compose(&[(0, context.one())], &context.one(), |_| Err(
                SolverError::UnluckySample
            ))
            .is_err()
    );
}

#[test]
fn ordinary_recorder_is_zero_sized_and_leaves_rows_untagged() {
    use super::super::NoTrace;
    assert_eq!(std::mem::size_of::<NoTrace>(), 0);
    assert_eq!(
        std::mem::size_of::<<NoTrace as Recording<1>>::Entry>(),
        std::mem::size_of::<PolynomialRow<1>>()
    );
}
