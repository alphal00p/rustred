//! Optional forward polynomial identities for the existing preconditioner.
//!
//! This is row-operation bookkeeping, not another elimination algorithm.
//! Symbolica owns every scale and all coefficient arithmetic. A forward
//! identity remains true where a scale vanishes; it does not assert that the
//! preconditioned rows retain the original specialized rank.

use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::solver::{IntegralOrder, PolynomialRow, SolverError};

use super::{Recording, RowEntry, precondition_recorded};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NodeId(usize);

#[derive(Debug)]
enum Node {
    Source(usize),
    Subtract {
        left: NodeId,
        left_scale: CoefficientPolynomial,
        right: NodeId,
        right_scale: CoefficientPolynomial,
    },
}

/// Immutable versioned DAG; roots follow the returned basis ordering.
///
/// It is private runtime derivation data, not a serialized authority claim.
/// Search need not expand any transitive source combination.
#[derive(Debug)]
pub(crate) struct PreconditionProvenance {
    nodes: Vec<Node>,
    roots: Box<[NodeId]>,
    source_count: usize,
}

impl PreconditionProvenance {
    /// Compose selected basis weights back to original source rows.
    ///
    /// `scale` supplies the caller's exact seed/chart transformation in the
    /// correct order, using the same ring homomorphism as the source rows.
    /// This accessor does not authenticate that caller-supplied transform.
    /// It is evaluated only for reachable DAG edges. This
    /// forward polynomial identity introduces no division; any poles in the
    /// supplied weights or transformed scales remain the caller's guard/replay
    /// obligations on the exact application domain.
    pub(crate) fn compose(
        &self,
        weights: &[(usize, Coefficient)],
        template: &Coefficient,
        mut scale: impl FnMut(&CoefficientPolynomial) -> Result<Coefficient, SolverError>,
    ) -> Result<Vec<Coefficient>, SolverError> {
        let variables = template.get_variables();
        let same_map = |value: &Coefficient| {
            value.numerator.variables() == variables && value.denominator.variables() == variables
        };
        if !same_map(template) {
            return Err(SolverError::InvalidInput(
                "precondition template variable maps differ".into(),
            ));
        }
        let mut adjoints = vec![None; self.nodes.len()];
        for (basis_row, weight) in weights {
            let Some(&node) = self.roots.get(*basis_row) else {
                return Err(SolverError::InvalidInput(
                    "precondition basis row is out of range".into(),
                ));
            };
            if !same_map(weight) {
                return Err(SolverError::InvalidInput(
                    "precondition weight variable map differs".into(),
                ));
            }
            accumulate(&mut adjoints[node.0], weight.clone());
        }
        let zero: Coefficient = template.numerator.zero().into();
        let mut original = vec![zero; self.source_count];
        for ordinal in (0..self.nodes.len()).rev() {
            let Some(weight) = adjoints[ordinal].take() else {
                continue;
            };
            if weight.is_zero() {
                continue;
            }
            match &self.nodes[ordinal] {
                Node::Source(source) => original[*source] = &original[*source] + &weight,
                Node::Subtract {
                    left,
                    left_scale,
                    right,
                    right_scale,
                } => {
                    let left_scale = scale(left_scale)?;
                    let right_scale = scale(right_scale)?;
                    if !same_map(&left_scale) || !same_map(&right_scale) {
                        return Err(SolverError::InvalidInput(
                            "precondition scale variable map differs".into(),
                        ));
                    }
                    accumulate(&mut adjoints[left.0], &weight * &left_scale);
                    accumulate(&mut adjoints[right.0], -(&weight * &right_scale));
                }
            }
        }
        Ok(original)
    }
}

fn accumulate(slot: &mut Option<Coefficient>, value: Coefficient) {
    if value.is_zero() {
        return;
    }
    *slot = Some(match slot.take() {
        Some(previous) => &previous + &value,
        None => value,
    });
}

struct TrackedRow<const N: usize> {
    terms: PolynomialRow<N>,
    node: NodeId,
}

impl<const N: usize> RowEntry<N> for TrackedRow<N> {
    fn terms(&self) -> &PolynomialRow<N> {
        &self.terms
    }
}

struct Builder {
    nodes: Vec<Node>,
}

impl<const N: usize> Recording<N> for Builder {
    type Entry = TrackedRow<N>;

    fn derived(
        &mut self,
        left: &Self::Entry,
        left_scale: &CoefficientPolynomial,
        right: &Self::Entry,
        right_scale: &CoefficientPolynomial,
        result: PolynomialRow<N>,
    ) -> Self::Entry {
        let node = NodeId(self.nodes.len());
        debug_assert!(left.node.0 < node.0 && right.node.0 < node.0);
        self.nodes.push(Node::Subtract {
            left: left.node,
            left_scale: left_scale.clone(),
            right: right.node,
            right_scale: right_scale.clone(),
        });
        TrackedRow {
            terms: result,
            node,
        }
    }
}

pub(crate) fn precondition_with_provenance<const N: usize>(
    rows: Vec<PolynomialRow<N>>,
    order: &IntegralOrder<N>,
    variables: &[usize],
) -> (Vec<PolynomialRow<N>>, PreconditionProvenance) {
    let source_count = rows.len();
    let mut builder = Builder {
        nodes: (0..source_count).map(Node::Source).collect(),
    };
    let tagged = rows
        .into_iter()
        .enumerate()
        .map(|(source, terms)| TrackedRow {
            terms,
            node: NodeId(source),
        })
        .collect();
    let reduced = precondition_recorded(tagged, order, variables, &mut builder);
    let roots = reduced.iter().map(|row| row.node).collect();
    let provenance = PreconditionProvenance {
        nodes: builder.nodes,
        roots,
        source_count,
    };
    (
        reduced.into_iter().map(|row| row.terms).collect(),
        provenance,
    )
}

#[cfg(test)]
mod tests;
