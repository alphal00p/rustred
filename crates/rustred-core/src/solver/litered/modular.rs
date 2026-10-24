//! Forward (memoized) and adjoint sweeps of a Zp application DAG.
//!
//! Forward mode gives every node's decomposition onto the candidate
//! terminals; adjoint mode propagates target weights from the roots down and
//! gives all target decompositions at once. Both must agree exactly; the
//! tests check this.

use super::LiteRedError;
use super::dag::{DagStructure, DagValues, NodeKind};
use super::zp::PrimeField;

/// Sparse vector over terminal (column) ids, sorted by id, no zero entries.
pub type SparseVector = Vec<(u32, u64)>;

/// Decomposition of every node onto terminals. Zero nodes map to the empty
/// vector; an uncovered node reachable from a computed node is an error.
pub fn forward<const N: usize>(
    structure: &DagStructure<N>,
    values: &DagValues,
    field: &PrimeField,
) -> Result<Vec<Option<SparseVector>>, LiteRedError> {
    let mut result: Vec<Option<SparseVector>> = vec![None; structure.len()];
    let mut dense = vec![0u64; structure.terminal_count()];
    let mut touched: Vec<u32> = Vec::new();
    for &id in &structure.postorder {
        let node = &structure.nodes[id as usize];
        let vector = match node.kind {
            NodeKind::Terminal(terminal) => vec![(terminal, 1)],
            NodeKind::Zero => Vec::new(),
            NodeKind::Uncovered => continue,
            NodeKind::Rule { .. } => {
                let mut uncovered = None;
                for (edge, &weight) in node.edges.iter().zip(&values.values[id as usize]) {
                    let Some(child) = result[edge.child as usize].as_ref() else {
                        uncovered = Some(edge.child);
                        break;
                    };
                    if weight == 0 {
                        continue;
                    }
                    for &(terminal, value) in child {
                        let slot = &mut dense[terminal as usize];
                        if *slot == 0 {
                            touched.push(terminal);
                        }
                        *slot = field.mul_add(*slot, weight, value);
                    }
                }
                if uncovered.is_some() {
                    for terminal in touched.drain(..) {
                        dense[terminal as usize] = 0;
                    }
                    // An uncovered descendant leaves this node undefined.
                    continue;
                }
                touched.sort_unstable();
                touched.dedup();
                let mut vector = Vec::with_capacity(touched.len());
                for terminal in touched.drain(..) {
                    let value = std::mem::take(&mut dense[terminal as usize]);
                    if value != 0 {
                        vector.push((terminal, value));
                    }
                }
                vector
            }
        };
        result[id as usize] = Some(vector);
    }
    Ok(result)
}

/// Decomposition onto terminals of a node computed by [`forward`].
pub fn require<'a>(
    forward: &'a [Option<SparseVector>],
    node: u32,
) -> Result<&'a SparseVector, LiteRedError> {
    forward[node as usize]
        .as_ref()
        .ok_or(LiteRedError::UncoveredNode { node })
}

/// Dense `targets x terminals` matrix by adjoint (reverse) accumulation.
pub fn adjoint<const N: usize>(
    structure: &DagStructure<N>,
    values: &DagValues,
    field: &PrimeField,
    targets: &[u32],
) -> Result<Vec<Vec<u64>>, LiteRedError> {
    let width = targets.len();
    let mut weights: Vec<Option<Vec<u64>>> = vec![None; structure.len()];
    for (column, &target) in targets.iter().enumerate() {
        let w = weights[target as usize].get_or_insert_with(|| vec![0; width]);
        w[column] = field.add(w[column], 1);
    }
    let mut result = vec![vec![0u64; structure.terminal_count()]; width];
    for &id in structure.postorder.iter().rev() {
        let Some(weight) = weights[id as usize].take() else {
            continue;
        };
        let node = &structure.nodes[id as usize];
        match node.kind {
            NodeKind::Terminal(terminal) => {
                for (column, &w) in weight.iter().enumerate() {
                    result[column][terminal as usize] =
                        field.add(result[column][terminal as usize], w);
                }
            }
            NodeKind::Zero => {}
            NodeKind::Uncovered => {
                if weight.iter().any(|&w| w != 0) {
                    return Err(LiteRedError::UncoveredNode { node: id });
                }
            }
            NodeKind::Rule { .. } => {
                for (edge, &value) in node.edges.iter().zip(&values.values[id as usize]) {
                    if value == 0 {
                        continue;
                    }
                    let child = weights[edge.child as usize].get_or_insert_with(|| vec![0; width]);
                    for (slot, &w) in child.iter_mut().zip(&weight) {
                        if w != 0 {
                            *slot = field.mul_add(*slot, value, w);
                        }
                    }
                }
            }
        }
    }
    Ok(result)
}
