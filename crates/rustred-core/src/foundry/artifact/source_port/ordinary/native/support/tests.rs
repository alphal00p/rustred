use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::{Integral, Term};

fn row(context: &CoefficientContext, terms: &[(i16, &str)]) -> ExactRow<1> {
    let mut row: ExactRow<1> = terms
        .iter()
        .map(|(shift, coefficient)| Term {
            integral: Integral::symbolic([*shift]).unwrap(),
            coefficient: context.coefficient_fixture(coefficient),
        })
        .collect();
    let order = IntegralOrder::new([true], [false]);
    row.sort_unstable_by(|a, b| order.compare(&a.integral, &b.integral));
    row
}

fn at(rows: &[ExactRow<1>], prime: u64, point: u64) -> Option<Vec<usize>> {
    let order = IntegralOrder::new([true], [false]);
    let mut columns: Vec<_> = rows.iter().flatten().map(|term| term.integral).collect();
    columns.sort_unstable_by(|a, b| order.compare(a, b));
    columns.dedup();
    let field = Zp64::new(prime);
    select(
        rows,
        &order,
        &columns,
        columns.len() as u32 + 1,
        &field,
        &[field.to_element(point)],
    )
}

#[test]
fn empty_and_dependent_inputs_preserve_original_source_mapping_and_transitive_trace() {
    let context = CoefficientContext::new(["n"]);
    let rows = vec![
        vec![],
        row(&context, &[(3, "1"), (2, "1")]),
        row(&context, &[(3, "2"), (2, "2")]),
        row(&context, &[(2, "1")]),
        row(&context, &[(1, "1")]),
        row(&context, &[(3, "1")]),
    ];
    assert_eq!(at(&rows, 101, 7), Some(vec![1, 3]));
    assert_eq!(
        candidates(&rows, &IntegralOrder::new([true], [false])),
        vec![vec![1, 3]]
    );
}

#[test]
fn complete_physical_rank_still_records_dependent_desired_row() {
    let context = CoefficientContext::new(["n"]);
    let rows = vec![
        row(&context, &[(1, "1")]),
        row(&context, &[(1, "2")]),
        row(&context, &[(1, "1")]),
    ];
    assert_eq!(at(&rows, 101, 7), Some(vec![0]));
}

#[test]
fn empty_or_sampled_zero_desired_image_is_inconclusive_not_a_stale_trace() {
    let context = CoefficientContext::new(["n"]);
    let mut rows = vec![
        row(&context, &[(1, "1")]),
        row(&context, &[(1, "2")]),
        row(&context, &[(1, "n")]),
    ];
    assert_eq!(at(&rows, 101, 0), None);
    rows[2].clear();
    assert_eq!(at(&rows, 101, 7), None);
}

#[test]
fn sampled_zero_source_and_rank_drop_do_not_supply_missing_pivots() {
    let context = CoefficientContext::new(["n"]);
    let mut rows = vec![
        row(&context, &[(3, "n")]),
        row(&context, &[(2, "1")]),
        row(&context, &[(3, "1")]),
    ];
    assert_eq!(at(&rows, 101, 0), None);
    assert_eq!(at(&rows, 101, 7), Some(vec![0]));
    rows[2] = row(&context, &[(2, "1")]);
    assert_eq!(at(&rows, 101, 0), Some(vec![1]));
}

#[test]
fn sampled_coefficient_pole_and_bad_prime_are_inconclusive() {
    let context = CoefficientContext::new(["n"]);
    let mut rows = vec![
        row(&context, &[(1, "1/n")]),
        vec![],
        row(&context, &[(1, "1")]),
    ];
    assert_eq!(at(&rows, 101, 0), None);
    assert_eq!(at(&rows, 101, 7), Some(vec![0]));
    rows[0] = row(&context, &[(1, "1/5")]);
    assert_eq!(at(&rows, 5, 1), None);
    assert_eq!(at(&rows, 101, 1), Some(vec![0]));
}

#[test]
fn duplicate_columns_and_foreign_variable_maps_disable_modular_support() {
    let context = CoefficientContext::new(["n"]);
    let order = IntegralOrder::new([true], [false]);
    let mut rows = vec![
        row(&context, &[(1, "1"), (1, "2")]),
        vec![],
        row(&context, &[(1, "1")]),
    ];
    assert!(candidates(&rows, &order).is_empty());
    let foreign = CoefficientContext::new(["foreign"]);
    rows[0] = row(&foreign, &[(1, "1")]);
    assert!(candidates(&rows, &order).is_empty());
}

#[test]
fn bounded_schedule_is_deterministic_and_oversized_inputs_fall_back() {
    let context = CoefficientContext::new(["n"]);
    let order = IntegralOrder::new([true], [false]);
    let rows = vec![
        row(&context, &[(1, "n-1")]),
        row(&context, &[(1, "1")]),
        row(&context, &[(1, "1")]),
    ];
    assert_eq!(candidates(&rows, &order), candidates(&rows, &order));
    assert!(candidates(&vec![vec![]; MAX_ROWS + 1], &order).is_empty());
}
