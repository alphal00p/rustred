//! Lane decision per DAG node (owner: M2-T3 rr-lanes).
use super::error::PError;
use super::lane::LaneDecision;
use super::topology::NodeInfo;

pub fn decide(_node: &NodeInfo) -> Result<LaneDecision, PError> {
    Err(PError::NotImplemented("M2-T3"))
}
