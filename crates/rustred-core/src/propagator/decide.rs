//! Lane decision per DAG node (owner: M2-T3 rr-lanes).
//!
//! The lane is a function of the node alone: Forcer's action of the topology
//! crate, split into SeriesParallel and Insertion. Roles name slots and loops of
//! the presentation that `present` builds for the node: line slot `i` is
//! canonical line `i` and the loop the rule acts on is [`ADAPTED_LOOP`]; a
//! Diamond acts on its first `L + S` loops ([`DiamondRoles`]). The decision
//! reads the graph only; a contract that the presentation then cannot meet is
//! `present_node`'s fallback.
use super::error::PError;
use super::lane::{
    BubbleRoles, CarpetRoles, DIAMOND_MAX_PATHS, DiamondRoles, LaneDecision, LaneRoles, OwnerLane,
    TriangleRoles,
};
use super::topology::{Action, NodeInfo, Normalised, normalise};

/// The loop an adapted presentation integrates out first: the bubble loop of
/// an Insertion, the outer loop `p` of a Carpet, `k = mom(b)` of a Triangle.
pub const ADAPTED_LOOP: u8 = 0;

pub(super) const CUSTOM: LaneDecision = LaneDecision {
    lane: OwnerLane::Custom,
    roles: LaneRoles::None,
    trivial: false,
};

/// The lane of `node`. `Zero` is reserved for a scaleless topology, which no
/// DAG holds; the trivial node has `trivial = true` and `Zero` as its lane,
/// since it is a terminal and owns no program.
pub fn decide(node: &NodeInfo) -> Result<LaneDecision, PError> {
    let terminal = |trivial| LaneDecision {
        lane: OwnerLane::Zero,
        roles: LaneRoles::None,
        trivial,
    };
    if normalise(&node.topo) == Normalised::Zero {
        return Ok(terminal(false));
    }
    let (lane, roles) = match &node.action {
        Action::Trivial => return Ok(terminal(true)),
        Action::Custom => match super::scheme::scheme_for(node) {
            Some(roles) => (OwnerLane::Scheme, LaneRoles::Scheme(roles)),
            None => return Ok(CUSTOM),
        },
        Action::Diamond(diamond) => {
            let lines = |lines: &[u8]| {
                lines
                    .iter()
                    .map(|&line| u16::from(line))
                    .collect::<Vec<_>>()
            };
            let spectator = match diamond.spect.as_slice() {
                [] => None,
                &[line] => Some(u16::from(line)),
                _ => {
                    return Err(PError::Unsupported(format!(
                        "node {}: a diamond with {} spectator lines",
                        node.index,
                        diamond.spect.len()
                    )));
                }
            };
            let roles = DiamondRoles::new(
                &lines(&diamond.a),
                &lines(&diamond.b),
                &lines(&diamond.c),
                spectator,
            )
            .ok_or_else(|| {
                PError::Unsupported(format!(
                    "node {}: a diamond of {} paths (at most {DIAMOND_MAX_PATHS})",
                    node.index,
                    diamond.a.len()
                ))
            })?;
            (OwnerLane::Diamond, LaneRoles::Diamond(roles))
        }
        Action::Insertion(pair) => {
            let lane = if node.series_parallel {
                OwnerLane::SeriesParallel
            } else {
                OwnerLane::Insertion
            };
            let roles = BubbleRoles {
                lines: [u16::from(pair.keep), u16::from(pair.drop)],
                loop_index: ADAPTED_LOOP,
            };
            (lane, LaneRoles::Insertion(roles))
        }
        Action::Carpet(carpet) => {
            let roles = CarpetRoles {
                r: u16::from(carpet.r),
                third: carpet.third.map(u16::from),
                outer_loop: ADAPTED_LOOP,
            };
            (OwnerLane::Carpet, LaneRoles::Carpet(roles))
        }
        Action::Triangle(triangle) => {
            let roles = TriangleRoles {
                b: u16::from(triangle.b),
                a: [u16::from(triangle.a1), u16::from(triangle.a2)],
                c: [u16::from(triangle.c1), u16::from(triangle.c2)],
                k_loop: ADAPTED_LOOP,
            };
            (OwnerLane::Triangle, LaneRoles::Triangle(roles))
        }
    };
    Ok(LaneDecision {
        lane,
        roles,
        trivial: false,
    })
}
