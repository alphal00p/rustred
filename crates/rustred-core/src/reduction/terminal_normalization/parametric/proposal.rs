use symbolica::graph::Graph;
use symbolica::prelude::Integer;

use crate::family::{IntegralFamily, IntegralKey};

use super::super::{TerminalAliasError as Error, products};
use super::model::{Support, VacuumParametricLimits};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum Color {
    Parameter(i64),
    Monomial(Integer),
}

pub(super) type ProposalGraph = Graph<Color, u16>;

pub(super) struct Proposal {
    pub graph: ProposalGraph,
    /// Input-local parameter position → native canonical graph vertex.
    pub parameters: Vec<usize>,
}

pub(super) fn canonicalize(
    family: &IntegralFamily,
    key: &IntegralKey,
    support: &Support,
    limits: VacuumParametricLimits,
) -> Result<Option<Proposal>, Error> {
    let Some(vertices) = support.slots.len().checked_add(support.u.nterms()) else {
        return Ok(None);
    };
    let edges = support
        .u
        .exponents_iter()
        .flatten()
        .filter(|&&power| power != 0)
        .count();
    if vertices > limits.max_graph_vertices || edges > limits.max_graph_edges {
        return Ok(None);
    }
    let mut graph = Graph::new();
    for &slot in &support.slots {
        graph.add_node(Color::Parameter(key.powers()[slot]));
    }
    for term in 0..support.u.nterms() {
        let coefficient =
            products::integer(&support.u.coefficients[term], family.coefficient_context())?
                .ok_or(Error::InvalidParametricWitness)?;
        let node = graph.add_node(Color::Monomial(coefficient));
        for (variable, &power) in support.u.exponents(term).iter().enumerate() {
            if power != 0 {
                graph
                    .add_edge(variable, node, false, power)
                    .map_err(|error| Error::ExactAlgebra(error.to_string()))?;
            }
        }
    }
    let canonical = graph.canonize();
    Ok(Some(Proposal {
        parameters: canonical.vertex_map[..support.slots.len()].to_vec(),
        graph: canonical.graph,
    }))
}
