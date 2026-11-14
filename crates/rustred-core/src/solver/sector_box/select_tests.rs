//! Property tests of the selection kernel against a dense reference
//! elimination over Zp: closure, reduced rows and masters of every column;
//! the second-sample replay of the L-selected rows; and the insertion-order
//! independence of the global modular system.

use std::sync::Arc;

use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::{Field, RingOps};
use symbolica::prelude::Ring;

use super::super::discovery::{ColumnComparator, Discovery};
use super::super::{Integral, SolverError, Term};
use super::modular::GlobalDiscovery;
use super::order::BoxOrder;
use super::rows::{Provenance, Zp, ZpRow};
use super::select::{Echelon, Replay, closure, nonzero_masters, reduced_rows};
use super::{BoxPhase, ColumnOrder};

const N: usize = 3;
const PRIME: u64 = 1_000_003;

fn order() -> Arc<BoxOrder<N>> {
    Arc::new(BoxOrder::new(&ColumnOrder::LinesDotsRank, N, &[]).unwrap())
}

/// Random rows over integrals with powers in `0..=2`, harder first, distinct.
fn random_rows(
    order: &BoxOrder<N>,
    field: &Zp64,
    count: usize,
    seed: u64,
) -> Vec<Vec<Term<N, Zp>>> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..count)
        .map(|_| {
            let mut integrals: Vec<Integral<N>> = (0..rng.random_range(1..6))
                .map(|_| {
                    Integral::numeric(std::array::from_fn(|_| rng.random_range(0..=2))).unwrap()
                })
                .collect();
            integrals.sort_unstable_by(|left, right| order.compare(left, right));
            integrals.dedup();
            integrals
                .into_iter()
                .map(|integral| Term {
                    integral,
                    coefficient: field.to_element(rng.random_range(1..PRIME)),
                })
                .collect()
        })
        .collect()
}

/// Dense reference: reduced row echelon form of all rows over the sorted
/// column list; the relation of a pivot column `t` is
/// `t = -sum_j R[t][j] j` over the non-pivot columns `j`.
fn dense_reduced(
    field: &Zp64,
    columns: &[Integral<N>],
    rows: &[Vec<Term<N, Zp>>],
) -> (Vec<Option<usize>>, Vec<Vec<Zp>>) {
    let n = columns.len();
    let index = |integral: &Integral<N>| columns.iter().position(|c| c == integral).unwrap();
    let mut matrix: Vec<Vec<Zp>> = rows
        .iter()
        .map(|row| {
            let mut dense = vec![field.zero(); n];
            for term in row {
                dense[index(&term.integral)] = term.coefficient;
            }
            dense
        })
        .collect();
    let mut pivot_of = vec![None; n];
    let mut rank = 0;
    for column in 0..n {
        let Some(found) = (rank..matrix.len()).find(|&r| !field.is_zero(&matrix[r][column])) else {
            continue;
        };
        matrix.swap(rank, found);
        let inverse = field.inv(&matrix[rank][column]);
        for value in matrix[rank].iter_mut() {
            *value = field.mul(&*value, &inverse);
        }
        for r in 0..matrix.len() {
            if r != rank && !field.is_zero(&matrix[r][column]) {
                let factor = matrix[r][column];
                let pivot_row = matrix[rank].clone();
                for (value, pivot_value) in matrix[r].iter_mut().zip(&pivot_row) {
                    let delta = field.mul(&factor, pivot_value);
                    *value = field.sub(&*value, &delta);
                }
            }
        }
        pivot_of[column] = Some(rank);
        rank += 1;
    }
    matrix.truncate(rank);
    (pivot_of, matrix)
}

#[test]
fn reduced_rows_and_masters_equal_a_dense_reference() {
    let field = Zp64::new(PRIME);
    let order = order();
    for seed in 0..12 {
        let rows = random_rows(&order, &field, 18, seed);
        let mut discovery = Discovery::new(order.clone(), field.clone());
        discovery.add_rows(&rows);
        let columns = discovery.columns().to_vec();
        let (pivot_of, dense) = dense_reduced(&field, &columns, &rows);
        for target in 0..columns.len() {
            assert_eq!(
                discovery.pivot_row(target).is_some(),
                pivot_of[target].is_some()
            );
            let reach = closure(&discovery, &[target]);
            let reduced = reduced_rows(&discovery, &reach, &[target]).remove(0);
            let expected: Vec<(usize, Zp)> = match pivot_of[target] {
                None => vec![(target, field.one())],
                Some(r) => (0..columns.len())
                    .filter(|&j| pivot_of[j].is_none() && !field.is_zero(&dense[r][j]))
                    .map(|j| (j, field.neg(&dense[r][j])))
                    .collect(),
            };
            assert_eq!(reduced, expected, "seed {seed}, column {target}");
            let masters = nonzero_masters(&discovery, &reach, &[target]);
            let expected_masters: Vec<usize> = expected.iter().map(|(j, _)| *j).collect();
            assert_eq!(masters, expected_masters, "seed {seed}, column {target}");
            assert!(reach.candidates.iter().all(|&c| pivot_of[c].is_none()));
            assert!(reach.columns.contains(&target));
        }
    }
}

/// The L-closure of the pivot rows of a demand, replayed alone in trace
/// order, reproduces every pivot and the same reduced rows; a replay in
/// another order is an unlucky sample.
#[test]
fn replay_of_the_selected_rows_reproduces_pivots_and_reduced_rows() {
    let field = Zp64::new(PRIME);
    let order = order();
    for seed in 20..30 {
        let rows = random_rows(&order, &field, 18, seed);
        let mut discovery = Discovery::new(order.clone(), field.clone());
        let mut accepted = Vec::new();
        for (row, pivot) in rows.iter().zip(discovery.add_rows(&rows)) {
            if pivot.is_some() {
                accepted.push(row.clone());
            }
        }
        let demand: Vec<usize> = (0..discovery.columns().len()).step_by(3).collect();
        let reach = closure(&discovery, &demand);
        let roots: Vec<usize> = reach.pivots.iter().map(|&(_, row)| row).collect();
        let selected = discovery.trace_union(&roots);
        let selected_rows: Vec<Vec<Term<N, Zp>>> =
            selected.iter().map(|&row| accepted[row].clone()).collect();
        let expected: Vec<Integral<N>> = selected
            .iter()
            .map(|&row| {
                let (column, _) = discovery
                    .pivot_columns()
                    .find(|&(_, basis)| basis == row)
                    .unwrap();
                discovery.columns()[column]
            })
            .collect();
        let replay = Replay::new(order.clone(), field.clone(), &selected_rows, &expected).unwrap();
        // A demanded column that no selected row touches is not a replay
        // column; it is a candidate of the full system with the trivial
        // reduced row (its own master).
        let (demand, untouched): (Vec<usize>, Vec<usize>) = demand
            .iter()
            .partition(|&&c| replay.column_index(&discovery.columns()[c]).is_some());
        for row in reduced_rows(&discovery, &reach, &untouched) {
            assert_eq!(row.len(), 1);
        }
        let demanded: Vec<Integral<N>> = demand.iter().map(|&c| discovery.columns()[c]).collect();
        let replay_demand: Vec<usize> = demanded
            .iter()
            .map(|integral| replay.column_index(integral).unwrap())
            .collect();
        let replay_reach = closure(&replay, &replay_demand);
        let in_replay = reduced_rows(&replay, &replay_reach, &replay_demand);
        let in_full = reduced_rows(&discovery, &reach, &demand);
        for (a, b) in in_replay.iter().zip(&in_full) {
            let a: Vec<(Integral<N>, Zp)> = a.iter().map(|&(c, v)| (replay.column(c), v)).collect();
            let b: Vec<(Integral<N>, Zp)> = b
                .iter()
                .map(|&(c, v)| (discovery.columns()[c], v))
                .collect();
            assert_eq!(a, b, "seed {seed}");
        }
        if selected_rows.len() > 1 {
            // Pivots are distinct, so a permuted expectation fails at row 0.
            let mut wrong = expected.clone();
            let last = wrong.len() - 1;
            wrong.swap(0, last);
            assert!(matches!(
                Replay::new(order.clone(), field.clone(), &selected_rows, &wrong),
                Err(SolverError::UnluckyBoxSample {
                    phase: BoxPhase::Replay
                })
            ));
        }
    }
}

/// `insert_batch` sorts its rows, so the system is a function of the row set
/// alone: any permutation of a batch gives the same pivots and provenance.
#[test]
fn global_system_does_not_depend_on_row_order() {
    let field = Zp64::new(PRIME);
    let order = order();
    let rows: Vec<ZpRow<N>> = random_rows(&order, &field, 40, 7)
        .into_iter()
        .enumerate()
        .map(|(index, terms)| ZpRow {
            provenance: Provenance {
                seed: index as u32 / 4,
                source: (index % 4) as u16,
            },
            terms,
        })
        .collect();
    let mut first = GlobalDiscovery::new(order.clone(), field.clone());
    let outcome = first.insert_batch(rows.clone()).unwrap();
    assert_eq!(outcome.rows, rows.len());
    assert_eq!(outcome.accepted, first.rank());
    let mut shuffled = rows.clone();
    let mut rng = StdRng::seed_from_u64(99);
    for i in (1..shuffled.len()).rev() {
        shuffled.swap(i, rng.random_range(0..=i));
    }
    let mut second = GlobalDiscovery::new(order.clone(), field.clone());
    second.insert_batch(shuffled).unwrap();
    let pivots = |system: &GlobalDiscovery<N>| -> Vec<Integral<N>> {
        system
            .discovery
            .pivot_columns()
            .map(|(column, _)| system.discovery.columns()[column])
            .collect()
    };
    assert_eq!(pivots(&first), pivots(&second));
    assert_eq!(first.provenance, second.provenance);
    assert!(first.bytes() > 0);
}
