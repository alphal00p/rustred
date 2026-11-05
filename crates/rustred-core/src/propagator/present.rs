//! Lane-adapted node presentations and edge maps (owner: M2-T3 rr-lanes).
use std::sync::Arc;

use crate::algebra::CoefficientContext;

use super::PresentationId;
use super::error::PError;
use super::lane::LaneDecision;
use super::presentation::{EdgeMap, Presentation};
use super::topology::{DagEdge, NodeInfo};

pub fn present(
    _node: &NodeInfo,
    _decision: &LaneDecision,
    _tops: &[Arc<Presentation>],
    _id: PresentationId,
    _context: &CoefficientContext,
) -> Result<Presentation, PError> {
    Err(PError::NotImplemented("M2-T3"))
}

pub fn edge_map(
    _parent: &Presentation,
    _child: &Presentation,
    _edge: &DagEdge,
) -> Result<EdgeMap, PError> {
    Err(PError::NotImplemented("M2-T3"))
}
