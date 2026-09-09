use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::tensors::sparse::{LuLMode, SparseMatrix, SparseRowReducer};

use crate::identity::{IntegralShift, TranslatedSourceRequest};

use super::{
    FORBIDDEN_COLUMNS, SpiredForbiddenTerm, SpiredModularError, SpiredModularKernel,
    SpiredModularLimits, SpiredModularRow, SpiredModularStreamOutcome,
};

const PRIME: u64 = 101;

#[derive(Debug)]
struct CountedColumn {
    value: u32,
    clone_count: Arc<AtomicUsize>,
}

impl Clone for CountedColumn {
    fn clone(&self) -> Self {
        self.clone_count.fetch_add(1, AtomicOrdering::Relaxed);
        Self {
            value: self.value,
            clone_count: Arc::clone(&self.clone_count),
        }
    }
}

impl PartialEq for CountedColumn {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for CountedColumn {}

impl PartialOrd for CountedColumn {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CountedColumn {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

fn request(source: usize) -> TranslatedSourceRequest {
    TranslatedSourceRequest::new(source, IntegralShift::try_new([0]).unwrap())
}

fn term(column: u32, residue: u64) -> SpiredForbiddenTerm<u32> {
    SpiredForbiddenTerm::new(column, residue)
}

fn row(
    source: usize,
    terms: impl IntoIterator<Item = SpiredForbiddenTerm<u32>>,
    target: u64,
) -> SpiredModularRow<u32> {
    SpiredModularRow::new(request(source), terms.into_iter().collect(), target)
}

fn batch_ranks(rows: &[(Vec<(u32, u64)>, u64)]) -> (usize, usize) {
    let columns = rows
        .iter()
        .flat_map(|(terms, _)| terms.iter().map(|&(column, _)| column))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let field = Zp64::new(PRIME);
    let mut forbidden_triplets = Vec::new();
    let mut augmented_triplets = Vec::new();
    for (row_ordinal, (terms, target)) in rows.iter().enumerate() {
        let row_ordinal = u32::try_from(row_ordinal).unwrap();
        let mut terms = terms.clone();
        terms.sort_unstable_by_key(|&(column, _)| column);
        for (column, residue) in terms {
            if residue == 0 {
                continue;
            }
            let logical = columns.binary_search(&column).unwrap() as u32;
            let entry = (row_ordinal, logical, field.to_element(residue));
            forbidden_triplets.push(entry);
            augmented_triplets.push(entry);
        }
        if *target != 0 {
            augmented_triplets.push((row_ordinal, columns.len() as u32, field.to_element(*target)));
        }
    }
    let row_count = rows.len() as u32;
    let forbidden = SparseMatrix::from_triplets(
        row_count,
        columns.len() as u32,
        forbidden_triplets,
        field.clone(),
    );
    let augmented = SparseMatrix::from_triplets(
        row_count,
        columns.len() as u32 + 1,
        augmented_triplets,
        field,
    );
    (
        SparseRowReducer::from_matrix(&forbidden, LuLMode::None)
            .u()
            .nrows() as usize,
        SparseRowReducer::from_matrix(&augmented, LuLMode::None)
            .u()
            .nrows() as usize,
    )
}

#[test]
fn dynamic_columns_insert_before_pivots_and_the_logical_target() {
    let mut kernel = SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
    assert!(
        kernel
            .try_push_row(row(0, [term(20, 1)], 0))
            .unwrap()
            .is_none()
    );
    assert_eq!(kernel.forbidden_columns(), &[20]);
    assert_eq!(kernel.target_logical_column(), 1);
    assert_eq!(kernel.forbidden_pivots(), &[Some(0)]);

    assert!(
        kernel
            .try_push_row(row(1, [term(10, 1)], 0))
            .unwrap()
            .is_none()
    );
    assert_eq!(kernel.forbidden_columns(), &[10, 20]);
    assert_eq!(kernel.target_logical_column(), 2);
    assert_eq!(kernel.forbidden_pivots(), &[Some(1), Some(0)]);

    // A sampled zero is nevertheless structural and inserts between both
    // existing pivots. Its later nonzero use must not add another column.
    assert!(
        kernel
            .try_push_row(row(2, [term(15, 0)], 0))
            .unwrap()
            .is_none()
    );
    assert_eq!(kernel.forbidden_columns(), &[10, 15, 20]);
    assert_eq!(kernel.target_logical_column(), 3);
    assert_eq!(kernel.forbidden_rank(), 2);
    assert!(
        kernel
            .try_push_row(row(3, [term(15, 1)], 0))
            .unwrap()
            .is_none()
    );
    assert_eq!(kernel.forbidden_columns(), &[10, 15, 20]);
    assert_eq!(kernel.forbidden_rank(), 3);

    let hit = kernel
        .try_push_row(row(4, [term(20, 1)], 1))
        .unwrap()
        .unwrap();
    assert_eq!(hit.target_logical_column(), 3);
    assert_eq!(hit.forbidden_rank(), 3);
    assert_eq!(hit.augmented_rank(), 4);
    assert_eq!(hit.support(), &[request(0), request(4)]);
}

#[test]
fn simultaneous_column_insertions_use_old_positions_and_keep_target_before_sentinel() {
    let mut kernel = SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
    kernel
        .try_push_row(row(0, [term(30, 1), term(10, 1)], 0))
        .unwrap();

    // Relative to the old [10, 30] registry these insert at positions
    // [0, 1, 1, 2]. The repeated middle and end positions exercise the
    // exact convention required by Symbolica's add_cols API.
    kernel
        .try_push_row(row(
            1,
            [term(40, 0), term(25, 0), term(5, 0), term(20, 0)],
            0,
        ))
        .unwrap();
    assert_eq!(kernel.forbidden_columns(), &[5, 10, 20, 25, 30, 40]);
    assert_eq!(kernel.target_logical_column(), 6);
    assert_eq!(
        kernel.forbidden_pivots(),
        &[None, Some(0), None, None, None, None]
    );

    for (source, column) in [5, 20, 25, 40, 30].into_iter().enumerate() {
        kernel
            .try_push_row(row(source + 2, [term(column, 1)], 0))
            .unwrap();
    }
    assert_eq!(kernel.forbidden_rank(), 6);
    let hit = kernel
        .try_push_row(row(7, [term(10, 1), term(30, 1)], 1))
        .unwrap()
        .unwrap();
    assert_eq!(hit.target_logical_column(), 6);
    assert_eq!(hit.forbidden_rank(), 6);
    assert_eq!(hit.augmented_rank(), 7);
    assert_eq!(hit.support(), &[request(0), request(7)]);
}

#[test]
fn canonical_preregistration_unions_existing_and_historical_zero_columns_once() {
    let mut kernel = SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
    kernel.try_push_row(row(0, [term(20, 1)], 0)).unwrap();
    assert_eq!(kernel.forbidden_pivots(), &[Some(0)]);

    // Relative to the old [20] registry, the new columns use insertion
    // positions [0, 1, 1]. Column 20 is already present and participates
    // only in the requested union. All three missing columns are known to
    // have been structurally zero in row zero.
    assert_eq!(
        kernel
            .try_preregister_historical_zero_columns(&[5, 20, 25, 40])
            .unwrap(),
        3
    );
    assert_eq!(kernel.forbidden_columns(), &[5, 20, 25, 40]);
    assert_eq!(kernel.forbidden_pivots(), &[None, Some(0), None, None]);
    assert_eq!(kernel.target_logical_column(), 4);
    assert_eq!((kernel.forbidden_rank(), kernel.augmented_rank()), (1, 1));
    assert_eq!(kernel.rows_consumed(), 1);

    // An all-existing union is a true no-op. A later structural column
    // exercises the ordinary dynamic-registration fallback.
    assert_eq!(
        kernel
            .try_preregister_historical_zero_columns(&[5, 20, 25, 40])
            .unwrap(),
        0
    );
    kernel.try_push_row(row(1, [term(30, 0)], 0)).unwrap();
    assert_eq!(kernel.forbidden_columns(), &[5, 20, 25, 30, 40]);
    assert_eq!(kernel.target_logical_column(), 5);

    for (source, column) in [5, 25, 30, 40].into_iter().enumerate() {
        kernel
            .try_push_row(row(source + 2, [term(column, 1)], 0))
            .unwrap();
    }
    assert_eq!(kernel.forbidden_rank(), 5);
    assert!(
        kernel
            .try_push_row(row(6, [term(20, 1)], 1))
            .unwrap()
            .is_some()
    );
}

#[test]
fn preregistration_rejects_noncanonical_or_over_limit_batches_transactionally() {
    let mut kernel = SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
    assert_eq!(
        kernel
            .try_preregister_historical_zero_columns(&[10, 10])
            .unwrap_err(),
        SpiredModularError::NonCanonicalForbiddenColumnBatch {
            first_column: 0,
            second_column: 1,
        }
    );
    assert_eq!(
        kernel
            .try_preregister_historical_zero_columns(&[20, 10])
            .unwrap_err(),
        SpiredModularError::NonCanonicalForbiddenColumnBatch {
            first_column: 0,
            second_column: 1,
        }
    );
    assert!(kernel.forbidden_columns().is_empty());
    assert_eq!((kernel.forbidden_rank(), kernel.augmented_rank()), (0, 0));

    let limits = SpiredModularLimits {
        max_forbidden_columns: 2,
        ..SpiredModularLimits::default()
    };
    let mut bounded = SpiredModularKernel::try_new(PRIME, limits).unwrap();
    assert_eq!(
        bounded
            .try_preregister_historical_zero_columns(&[1, 2, 3])
            .unwrap_err(),
        SpiredModularError::ResourceLimit {
            resource: FORBIDDEN_COLUMNS,
            requested: 3,
            limit: 2,
        }
    );
    assert!(!bounded.is_poisoned());
    assert!(bounded.forbidden_columns().is_empty());
    assert_eq!((bounded.forbidden_rank(), bounded.augmented_rank()), (0, 0));
}

#[test]
fn existing_column_fast_path_does_not_clone_the_registry() {
    let clone_count = Arc::new(AtomicUsize::new(0));
    let column = || CountedColumn {
        value: 7,
        clone_count: Arc::clone(&clone_count),
    };
    let mut kernel = SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
    kernel
        .try_push_row(SpiredModularRow::new(
            request(0),
            vec![SpiredForbiddenTerm::new(column(), 1)],
            0,
        ))
        .unwrap();

    clone_count.store(0, AtomicOrdering::Relaxed);
    kernel
        .try_push_row(SpiredModularRow::new(
            request(1),
            vec![SpiredForbiddenTerm::new(column(), 2)],
            0,
        ))
        .unwrap();
    assert_eq!(clone_count.load(AtomicOrdering::Relaxed), 0);
}

#[test]
fn dense_scan_work_charges_each_row_at_its_actual_streamed_width() {
    let limits = SpiredModularLimits {
        max_reducer_scratch_cells: 12,
        max_reducer_dense_scan_work: 16,
        ..SpiredModularLimits::default()
    };
    let mut kernel = SpiredModularKernel::try_new(PRIME, limits).unwrap();
    // Width 1 + 3 = 4 (the augmented width includes target + sentinel).
    kernel.try_push_row(row(0, [term(0, 1)], 0)).unwrap();
    // Four late structural-zero columns widen the reducers to 5 + 7 = 12.
    // The cumulative charge is 4 + 12 = 16, not 2 * 12.
    kernel
        .try_push_row(row(1, [term(1, 0), term(2, 0), term(3, 0), term(4, 0)], 0))
        .unwrap();
    assert_eq!(kernel.forbidden_columns(), &[0, 1, 2, 3, 4]);
    assert_eq!(
        kernel.try_push_row(row(2, [term(4, 0)], 0)).unwrap_err(),
        SpiredModularError::ResourceLimit {
            resource: super::REDUCER_DENSE_SCAN_WORK,
            requested: 28,
            limit: 16,
        }
    );
    assert!(!kernel.is_poisoned());
    assert_eq!(kernel.rows_consumed(), 2);
}

#[test]
fn randomized_streaming_first_hit_matches_canonical_batch_rank() {
    fn next(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }

    for trial in 0..24u64 {
        let mut state = 0x9e37_79b9_7f4a_7c15 ^ trial;
        let mut kernel =
            SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
        let mut prefix = Vec::new();
        for row_ordinal in 0..16usize {
            let available = usize::min(8, row_ordinal / 2 + 2);
            let mut terms = Vec::new();
            for column in 0..available as u32 {
                if next(&mut state).is_multiple_of(3) {
                    let draw = next(&mut state);
                    let residue = if draw.is_multiple_of(5) {
                        0
                    } else {
                        draw % PRIME
                    };
                    terms.push((column, residue));
                }
            }
            if terms.is_empty() {
                terms.push(((next(&mut state) as usize % available) as u32, 0));
            }
            if next(&mut state).is_multiple_of(2) {
                terms.reverse();
            }
            let target = if row_ordinal < 3 || !next(&mut state).is_multiple_of(4) {
                0
            } else {
                next(&mut state) % PRIME
            };
            prefix.push((terms.clone(), target));
            let outcome = kernel
                .try_push_row(row(
                    row_ordinal,
                    terms.iter().map(|&(column, residue)| term(column, residue)),
                    target,
                ))
                .unwrap();
            let (forbidden_rank, augmented_rank) = batch_ranks(&prefix);
            assert_eq!(
                (kernel.forbidden_rank(), kernel.augmented_rank()),
                (forbidden_rank, augmented_rank),
                "rank mismatch in trial {trial}, prefix {}",
                row_ordinal + 1
            );
            assert_eq!(
                outcome.is_some(),
                augmented_rank == forbidden_rank + 1,
                "first-hit mismatch in trial {trial}, prefix {}",
                row_ordinal + 1
            );
            if outcome.is_some() {
                break;
            }
        }
    }
}

#[test]
fn randomized_hit_support_reduces_to_a_rank_gain_with_late_sampled_zero_columns() {
    fn next(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }

    fn nonzero(state: &mut u64) -> u64 {
        next(state) % (PRIME - 1) + 1
    }

    fn maybe_reverse(state: &mut u64, mut terms: Vec<(u32, u64)>) -> Vec<(u32, u64)> {
        if next(state).is_multiple_of(2) {
            terms.reverse();
        }
        terms
    }

    for trial in 0..64u64 {
        let mut state = 0xd1b5_4a32_d192_ed03 ^ trial;
        let base = (next(&mut state) % 1_000) as u32 * 8;
        let a = base + 1;
        let b = base + 3;
        let c = base + 5;
        let unrelated = base + 7;
        let a_seed = nonzero(&mut state);
        let unrelated_seed = nonzero(&mut state);
        let a_to_b = nonzero(&mut state);
        let b_seed = nonzero(&mut state);
        let b_to_c = nonzero(&mut state);
        let c_seed = nonzero(&mut state);
        let unrelated_dependent = nonzero(&mut state);
        let c_target = nonzero(&mut state);
        let target = nonzero(&mut state);
        let a_b_row = maybe_reverse(&mut state, vec![(a, a_to_b), (b, b_seed)]);
        let b_c_row = maybe_reverse(&mut state, vec![(b, b_to_c), (c, c_seed)]);
        let rows = vec![
            (vec![(a, a_seed)], 0),
            (vec![(unrelated, unrelated_seed)], 0),
            // These two rows introduce late structural columns at sampled
            // zero without entering the augmented basis trace.
            (vec![(b, 0)], 0),
            (a_b_row, 0),
            (vec![(c, 0)], 0),
            (b_c_row, 0),
            (vec![(unrelated, unrelated_dependent)], 0),
            (vec![(c, c_target)], target),
        ];

        let mut kernel =
            SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
        let mut hit = None;
        for (source_ordinal, (terms, target)) in rows.iter().enumerate() {
            let outcome = kernel
                .try_push_row(row(
                    source_ordinal,
                    terms.iter().map(|&(column, residue)| term(column, residue)),
                    *target,
                ))
                .unwrap();
            if outcome.is_some() {
                assert_eq!(source_ordinal, rows.len() - 1);
                hit = outcome;
            }
        }
        let hit = hit.expect("the randomized dependency chain must hit");
        let support_ordinals = hit
            .support()
            .iter()
            .map(TranslatedSourceRequest::source_ordinal)
            .collect::<BTreeSet<_>>();
        assert_eq!(support_ordinals, BTreeSet::from([0, 3, 5, 7]));
        assert!(rows[2].0.iter().all(|(_, residue)| *residue == 0));
        assert!(rows[4].0.iter().all(|(_, residue)| *residue == 0));

        let supported_rows = rows
            .iter()
            .enumerate()
            .filter(|(ordinal, _)| support_ordinals.contains(ordinal))
            .map(|(_, row)| row.clone())
            .collect::<Vec<_>>();
        let (forbidden_rank, augmented_rank) = batch_ranks(&supported_rows);
        assert_eq!(augmented_rank, forbidden_rank + 1);

        // Re-stream only the reported support in chronology order. The
        // omitted sampled-zero registration rows force B and C through
        // the safe dynamic late-column path during this independent run.
        let mut replay =
            SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
        let mut replay_hit = None;
        for (source_ordinal, (terms, target)) in rows.iter().enumerate() {
            if !support_ordinals.contains(&source_ordinal) {
                continue;
            }
            replay_hit = replay
                .try_push_row(row(
                    source_ordinal,
                    terms.iter().map(|&(column, residue)| term(column, residue)),
                    *target,
                ))
                .unwrap();
        }
        assert!(replay_hit.is_some(), "support failed in trial {trial}");
        assert_eq!(
            (replay.forbidden_rank(), replay.augmented_rank()),
            (forbidden_rank, augmented_rank)
        );
    }
}

#[test]
fn dependent_rows_do_not_pollute_the_basis_trace() {
    let mut kernel = SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
    kernel.try_push_row(row(0, [term(0, 1)], 0)).unwrap();
    kernel.try_push_row(row(1, [term(0, 2)], 0)).unwrap();
    assert_eq!(kernel.trace_node_count(), 1);
    let hit = kernel
        .try_push_row(row(2, [term(0, 1)], 1))
        .unwrap()
        .unwrap();
    assert_eq!(hit.direct_dependencies(), &[request(0)]);
    assert_eq!(hit.support(), &[request(0), request(2)]);
    assert!(!hit.support().contains(&request(1)));
}

#[test]
fn dfs_keeps_transitive_dependencies_and_prunes_unrelated_rows() {
    let mut kernel = SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
    kernel.try_push_row(row(0, [term(0, 1)], 0)).unwrap();
    kernel
        .try_push_row(row(1, [term(1, 1), term(0, 1)], 0))
        .unwrap();
    kernel.try_push_row(row(2, [term(2, 1)], 0)).unwrap();
    let hit = kernel
        .try_push_row(row(3, [term(1, 1)], 1))
        .unwrap()
        .unwrap();
    assert_eq!(hit.direct_dependencies(), &[request(1)]);
    assert_eq!(hit.support(), &[request(0), request(1), request(3)]);
    assert_eq!(
        hit.dependency_order(),
        &[request(0), request(1), request(3)]
    );
    let trace = hit.dependency_trace();
    assert_eq!(trace.root(), 2);
    assert_eq!(trace.edge_count(), 2);
    assert_eq!(trace.nodes().len(), 3);
    assert_eq!(trace.nodes()[0].source(), &request(0));
    assert!(trace.nodes()[0].direct_predecessors().is_empty());
    assert_eq!(trace.nodes()[1].source(), &request(1));
    assert_eq!(trace.nodes()[1].direct_predecessors(), &[0]);
    assert_eq!(trace.nodes()[2].source(), &request(3));
    assert_eq!(trace.nodes()[2].direct_predecessors(), &[1]);
    assert!(!hit.support().contains(&request(2)));
}

#[test]
fn dependency_trace_preserves_gplu_topology_separately_from_canonical_support() {
    let mut kernel = SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
    kernel.try_push_row(row(9, [term(0, 1)], 0)).unwrap();
    kernel.try_push_row(row(8, [term(1, 1)], 0)).unwrap();
    kernel
        .try_push_row(row(4, [term(0, 1), term(2, 1)], 0))
        .unwrap();
    kernel
        .try_push_row(row(3, [term(1, 1), term(3, 1)], 0))
        .unwrap();
    let hit = kernel
        .try_push_row(row(1, [term(2, 1), term(3, 1)], 1))
        .unwrap()
        .unwrap();

    assert_eq!(
        hit.support(),
        &[request(1), request(3), request(4), request(8), request(9)]
    );
    assert_eq!(
        hit.dependency_order(),
        &[request(9), request(8), request(4), request(3), request(1)]
    );
    assert_eq!(hit.direct_dependencies(), &[request(3), request(4)]);
    let trace = hit.dependency_trace();
    assert_eq!(trace.root(), 4);
    assert_eq!(trace.edge_count(), 4);
    assert_eq!(
        trace
            .nodes()
            .iter()
            .map(|node| node.source().source_ordinal())
            .collect::<Vec<_>>(),
        [9, 8, 4, 3, 1]
    );
    assert!(trace.nodes()[0].direct_predecessors().is_empty());
    assert!(trace.nodes()[1].direct_predecessors().is_empty());
    assert_eq!(trace.nodes()[2].direct_predecessors(), &[0]);
    assert_eq!(trace.nodes()[3].direct_predecessors(), &[1]);
    assert_eq!(trace.nodes()[4].direct_predecessors(), &[2, 3]);
}

#[test]
fn dependency_trace_output_caps_fail_closed_after_a_hit() {
    let limits = SpiredModularLimits {
        max_dependency_order_requests: 2,
        ..SpiredModularLimits::default()
    };
    let mut kernel = SpiredModularKernel::try_new(PRIME, limits).unwrap();
    kernel.try_push_row(row(9, [term(0, 1)], 0)).unwrap();
    kernel
        .try_push_row(row(4, [term(0, 1), term(1, 1)], 0))
        .unwrap();
    assert_eq!(
        kernel.try_push_row(row(1, [term(1, 1)], 1)).unwrap_err(),
        SpiredModularError::ResourceLimit {
            resource: super::DEPENDENCY_ORDER_REQUESTS,
            requested: 3,
            limit: 2,
        }
    );
    assert!(kernel.is_poisoned());
    assert_eq!(kernel.trace_node_count(), 0);

    let limits = SpiredModularLimits {
        max_hit_trace_edges: 1,
        ..SpiredModularLimits::default()
    };
    let mut kernel = SpiredModularKernel::try_new(PRIME, limits).unwrap();
    kernel.try_push_row(row(9, [term(0, 1)], 0)).unwrap();
    kernel
        .try_push_row(row(4, [term(0, 1), term(1, 1)], 0))
        .unwrap();
    assert_eq!(
        kernel.try_push_row(row(1, [term(1, 1)], 1)).unwrap_err(),
        SpiredModularError::ResourceLimit {
            resource: super::HIT_TRACE_EDGES,
            requested: 2,
            limit: 1,
        }
    );
    assert!(kernel.is_poisoned());
    assert_eq!(kernel.trace_node_count(), 0);

    let limits = SpiredModularLimits {
        // Direct owns one request, while canonical support, dependency order,
        // and the request-bearing DAG each own all three: ten components.
        max_hit_request_shift_components: 9,
        ..SpiredModularLimits::default()
    };
    let mut kernel = SpiredModularKernel::try_new(PRIME, limits).unwrap();
    kernel.try_push_row(row(9, [term(0, 1)], 0)).unwrap();
    kernel
        .try_push_row(row(4, [term(0, 1), term(1, 1)], 0))
        .unwrap();
    assert_eq!(
        kernel.try_push_row(row(1, [term(1, 1)], 1)).unwrap_err(),
        SpiredModularError::ResourceLimit {
            resource: super::HIT_REQUEST_SHIFT_COMPONENTS,
            requested: 10,
            limit: 9,
        }
    );
    assert!(kernel.is_poisoned());
}

#[test]
fn aggregate_trace_edge_cap_precedes_dependency_retention() {
    let limits = SpiredModularLimits {
        max_trace_edges: 0,
        ..SpiredModularLimits::default()
    };
    let mut kernel = SpiredModularKernel::try_new(PRIME, limits).unwrap();
    kernel.try_push_row(row(9, [term(0, 1)], 0)).unwrap();
    assert_eq!(
        kernel
            .try_push_row(row(4, [term(0, 1), term(1, 1)], 0))
            .unwrap_err(),
        SpiredModularError::ResourceLimit {
            resource: super::TRACE_EDGES,
            requested: 1,
            limit: 0,
        }
    );
    assert!(kernel.is_poisoned());
    assert_eq!(kernel.trace_node_count(), 0);
}

#[test]
fn support_is_deterministic_under_term_arrival_order() {
    fn run(reverse: bool) -> Vec<TranslatedSourceRequest> {
        let mut kernel =
            SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
        kernel.try_push_row(row(0, [term(8, 1)], 0)).unwrap();
        let middle = if reverse {
            vec![term(9, 1), term(8, 1)]
        } else {
            vec![term(8, 1), term(9, 1)]
        };
        kernel.try_push_row(row(1, middle, 0)).unwrap();
        kernel
            .try_push_row(row(2, [term(9, 1)], 1))
            .unwrap()
            .unwrap()
            .support()
            .to_vec()
    }
    assert_eq!(run(false), run(true));
}

#[test]
fn streaming_ranks_match_canonical_batch_matrices() {
    let mut kernel = SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
    kernel.try_push_row(row(0, [term(20, 1)], 1)).unwrap();
    kernel.try_push_row(row(1, [term(10, 1)], 1)).unwrap();
    kernel
        .try_push_row(row(2, [term(20, 1), term(10, 1)], 2))
        .unwrap();
    kernel.try_push_row(row(3, [term(15, 0)], 0)).unwrap();
    assert!(!kernel.has_hit());
    assert_eq!(kernel.forbidden_columns(), &[10, 15, 20]);

    let field = Zp64::new(PRIME);
    let forbidden = SparseMatrix::from_triplets(
        4,
        3,
        vec![
            (0, 2, field.to_element(1)),
            (1, 0, field.to_element(1)),
            (2, 0, field.to_element(1)),
            (2, 2, field.to_element(1)),
        ],
        field.clone(),
    );
    let augmented = SparseMatrix::from_triplets(
        4,
        4,
        vec![
            (0, 2, field.to_element(1)),
            (0, 3, field.to_element(1)),
            (1, 0, field.to_element(1)),
            (1, 3, field.to_element(1)),
            (2, 0, field.to_element(1)),
            (2, 2, field.to_element(1)),
            (2, 3, field.to_element(2)),
        ],
        field,
    );
    let forbidden_batch = SparseRowReducer::from_matrix(&forbidden, LuLMode::Pattern);
    let augmented_batch = SparseRowReducer::from_matrix(&augmented, LuLMode::Pattern);
    assert_eq!(
        kernel.forbidden_rank(),
        forbidden_batch.u().nrows() as usize
    );
    assert_eq!(
        kernel.augmented_rank(),
        augmented_batch.u().nrows() as usize
    );
    assert_eq!(kernel.forbidden_pivots(), forbidden_batch.pivots());
}

#[test]
fn aggregate_trace_shift_budget_fails_closed_and_discards_native_state() {
    let limits = SpiredModularLimits {
        max_retained_request_shift_components: 1,
        ..SpiredModularLimits::default()
    };
    let mut kernel = SpiredModularKernel::try_new(PRIME, limits).unwrap();
    kernel.try_push_row(row(0, [term(0, 1)], 0)).unwrap();
    let error = kernel.try_push_row(row(1, [term(1, 1)], 0)).unwrap_err();
    assert_eq!(
        error,
        SpiredModularError::ResourceLimit {
            resource: super::TRACE_SHIFT_COMPONENTS,
            requested: 2,
            limit: 1,
        }
    );
    assert!(kernel.is_poisoned());
    assert_eq!(kernel.rows_consumed(), 0);
    assert!(kernel.forbidden_columns().is_empty());
    assert_eq!((kernel.forbidden_rank(), kernel.augmented_rank()), (0, 0));
    assert_eq!(kernel.trace_node_count(), 0);
    assert_eq!(
        kernel.try_push_row(row(2, [], 0)).unwrap_err(),
        SpiredModularError::Poisoned
    );
}

#[test]
fn post_native_fill_exhaustion_discards_all_observable_state() {
    let mut kernel = SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
    kernel.try_push_row(row(0, [term(0, 1)], 0)).unwrap();
    let retained = kernel.native_entry_count().unwrap();
    assert!(retained > 0);

    // Simulate a stricter owner boundary being installed after native
    // work but before its mandatory post-check.
    kernel.limits.max_reducer_entries = retained - 1;
    let error = kernel.enforce_native_fill_after_mutation().unwrap_err();
    assert_eq!(
        error,
        SpiredModularError::ResourceLimit {
            resource: super::REDUCER_ENTRIES,
            requested: retained,
            limit: retained - 1,
        }
    );
    assert!(kernel.is_poisoned());
    assert_eq!(kernel.rows_consumed(), 0);
    assert!(kernel.forbidden_columns().is_empty());
    assert_eq!((kernel.forbidden_rank(), kernel.augmented_rank()), (0, 0));
    assert_eq!(kernel.trace_node_count(), 0);
}

#[test]
fn malformed_inputs_and_exhaustion_fail_closed() {
    assert_eq!(
        SpiredModularKernel::<u32>::try_new(2, SpiredModularLimits::default()).unwrap_err(),
        SpiredModularError::UnsupportedEvenModulus { modulus: 2 }
    );
    assert_eq!(
        SpiredModularKernel::<u32>::try_new(9, SpiredModularLimits::default()).unwrap_err(),
        SpiredModularError::NonPrimeModulus { modulus: 9 }
    );

    let mut kernel = SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
    assert_eq!(
        kernel
            .try_push_row(row(0, [term(1, 1), term(1, 2)], 0))
            .unwrap_err(),
        SpiredModularError::DuplicateForbiddenTerm {
            row: 0,
            first_term: 0,
            second_term: 1,
        }
    );
    assert_eq!(
        kernel
            .try_push_row(row(0, [term(1, PRIME)], 0))
            .unwrap_err(),
        SpiredModularError::NonCanonicalResidue {
            row: 0,
            term: Some(0),
            residue: PRIME,
            modulus: PRIME,
        }
    );
    assert_eq!(
        kernel.try_push_row(row(0, [], PRIME)).unwrap_err(),
        SpiredModularError::NonCanonicalResidue {
            row: 0,
            term: None,
            residue: PRIME,
            modulus: PRIME,
        }
    );

    let limits = SpiredModularLimits {
        max_forbidden_columns: 0,
        ..SpiredModularLimits::default()
    };
    let mut bounded = SpiredModularKernel::try_new(PRIME, limits).unwrap();
    assert_eq!(
        bounded.try_push_row(row(0, [term(7, 0)], 0)).unwrap_err(),
        SpiredModularError::ResourceLimit {
            resource: FORBIDDEN_COLUMNS,
            requested: 1,
            limit: 0,
        }
    );

    let limits = SpiredModularLimits {
        max_reducer_entries: 0,
        ..SpiredModularLimits::default()
    };
    let mut fill_bounded = SpiredModularKernel::try_new(PRIME, limits).unwrap();
    assert_eq!(
        fill_bounded
            .try_push_row(row(0, [term(7, 1)], 0))
            .unwrap_err(),
        SpiredModularError::ResourceLimit {
            resource: super::REDUCER_ENTRIES,
            requested: 6,
            limit: 0,
        }
    );
    assert!(!fill_bounded.is_poisoned());
    assert_eq!(fill_bounded.rows_consumed(), 0);
    assert!(fill_bounded.forbidden_columns().is_empty());

    let mut closed = SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
    assert!(closed.try_push_row(row(0, [], 1)).unwrap().is_some());
    assert_eq!(
        closed.try_push_row(row(1, [], 0)).unwrap_err(),
        SpiredModularError::AlreadyHit
    );
}

#[test]
fn post_hit_stream_skips_unrelated_dependence_then_emits_a_later_root_trace() {
    let mut kernel = SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
    assert!(matches!(
        kernel
            .try_push_row_continuing(row(0, [term(0, 1)], 0))
            .unwrap(),
        SpiredModularStreamOutcome::Pending
    ));
    let first = kernel
        .try_push_row_continuing(row(1, [term(0, 1)], 1))
        .unwrap();
    assert!(matches!(first, SpiredModularStreamOutcome::FirstHit(_)));
    assert_eq!(kernel.post_hit_rows_consumed(), 0);

    // This row is dependent in both systems but its augmented dependency
    // closure never touches the target-producing basis row.
    assert!(matches!(
        kernel
            .try_push_row_continuing(row(2, [term(0, 2)], 0))
            .unwrap(),
        SpiredModularStreamOutcome::Pending
    ));
    assert_eq!(kernel.post_hit_rows_consumed(), 1);

    // The later row closes through all prior S rows and the immutable first
    // target pivot. It, not the first hit, is the designated trace root.
    let candidate = match kernel
        .try_push_row_continuing(row(3, [term(0, 3)], 1))
        .unwrap()
    {
        SpiredModularStreamOutcome::PostHitCandidate(candidate) => candidate,
        other => panic!("expected a later post-hit candidate, got {other:?}"),
    };
    assert_eq!(candidate.rows_consumed(), 4);
    assert_eq!(candidate.forbidden_rank(), 1);
    assert_eq!(candidate.augmented_rank(), 2);
    assert_eq!(candidate.target_logical_column(), 1);
    assert_eq!(candidate.root_source(), &request(3));
    let trace = candidate.dependency_trace();
    assert_eq!(trace.root(), trace.nodes().len() - 1);
    assert_eq!(
        trace
            .nodes()
            .iter()
            .map(|node| node.source().source_ordinal())
            .collect::<Vec<_>>(),
        [0, 1, 3]
    );
    assert!(trace.nodes()[0].direct_predecessors().is_empty());
    assert_eq!(trace.nodes()[1].direct_predecessors(), &[0]);
    assert_eq!(trace.nodes()[2].direct_predecessors(), &[0, 1]);
    assert_eq!(trace.edge_count(), 3);
    assert_eq!(kernel.post_hit_rows_consumed(), 2);
}

#[test]
fn zero_sentinel_keeps_post_hit_trace_alive_when_forbidden_block_is_full() {
    let mut kernel = SpiredModularKernel::try_new(PRIME, SpiredModularLimits::default()).unwrap();
    kernel
        .try_push_row_continuing(row(0, [term(7, 1)], 0))
        .unwrap();
    assert_eq!(kernel.forbidden_rank(), kernel.forbidden_columns().len());
    assert!(matches!(
        kernel
            .try_push_row_continuing(row(1, [term(7, 1)], 1))
            .unwrap(),
        SpiredModularStreamOutcome::FirstHit(_)
    ));
    assert!(matches!(
        kernel
            .try_push_row_continuing(row(2, [term(7, 4)], 1))
            .unwrap(),
        SpiredModularStreamOutcome::PostHitCandidate(_)
    ));
    assert!(!kernel.is_poisoned());
}

#[test]
fn post_hit_window_limit_precedes_mutation_and_candidate_output_failure_poisons() {
    let limits = SpiredModularLimits {
        max_post_hit_rows: 1,
        ..SpiredModularLimits::default()
    };
    let mut kernel = SpiredModularKernel::try_new(PRIME, limits).unwrap();
    kernel
        .try_push_row_continuing(row(0, [term(0, 1)], 0))
        .unwrap();
    kernel
        .try_push_row_continuing(row(1, [term(0, 1)], 1))
        .unwrap();
    kernel
        .try_push_row_continuing(row(2, [term(0, 2)], 0))
        .unwrap();
    assert_eq!(kernel.rows_consumed(), 3);
    assert_eq!(
        kernel
            .try_push_row_continuing(row(3, [term(0, 3)], 1))
            .unwrap_err(),
        SpiredModularError::ResourceLimit {
            resource: super::POST_HIT_ROWS,
            requested: 2,
            limit: 1,
        }
    );
    assert_eq!(kernel.rows_consumed(), 3);
    assert_eq!(kernel.post_hit_rows_consumed(), 1);
    assert!(!kernel.is_poisoned());

    let limits = SpiredModularLimits {
        max_hit_trace_edges: 2,
        ..SpiredModularLimits::default()
    };
    let mut kernel = SpiredModularKernel::try_new(PRIME, limits).unwrap();
    kernel
        .try_push_row_continuing(row(0, [term(0, 1)], 0))
        .unwrap();
    kernel
        .try_push_row_continuing(row(1, [term(0, 1)], 1))
        .unwrap();
    assert_eq!(
        kernel
            .try_push_row_continuing(row(2, [term(0, 3)], 1))
            .unwrap_err(),
        SpiredModularError::ResourceLimit {
            resource: super::HIT_TRACE_EDGES,
            requested: 3,
            limit: 2,
        }
    );
    assert!(kernel.is_poisoned());
    assert_eq!(kernel.rows_consumed(), 0);
    assert_eq!(kernel.post_hit_rows_consumed(), 0);
}
