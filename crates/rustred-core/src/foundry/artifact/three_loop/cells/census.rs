//! Finite, test-only reachability census for the current K=6 rule slices.
//!
//! The roots deliberately sample bounded diamonds around the top, five-line,
//! and four-line graph corners, plus independently compiled product and zero
//! terminals.  A clean report is evidence only for this finite set and its
//! concrete descendants; it is never a sector-complete artifact witness.

use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::RuleCell;
use crate::foundry::search::{
    ReachabilityError, ReachabilityFrontier, ReachabilityLimits, ReachabilityPlanner,
    SectorSearchDiamond, SectorSearchLimits,
};
use crate::identity::IntegralShift;
use crate::sector::OrderingPolicy;

use super::super::K6ReachabilityTerminals;
use super::five_line::{FiveLineCellSet, derive_five_line_cells};
use super::four_line::{FourLineCellSet, derive_all_four_line_cells};
use super::three_line::{ThreeLineCellSet, derive_three_line_cells};
use super::top::derive_top_cell;

const TOP_CORNER: [i64; 6] = [1; 6];
const FIVE_LINE_CORNER: [i64; 6] = [0, 1, 1, 1, 1, 1];
const FIVE_LINE_OVERLAP_PROBE: [i64; 6] = [0, 1, 1, 1, 2, 2];
const FIVE_LINE_SCALAR_NUMERATOR_BULK_PROBE: [i64; 6] = [-2, 1, 1, 1, 1, 1];
const FIVE_LINE_ADJACENT_NUMERATOR_ENDPOINT_PROBE: [i64; 6] = [-1, 1, 1, 1, 2, 1];
const FIVE_LINE_ADJACENT_NUMERATOR_BULK_PROBE: [i64; 6] = [-2, 1, 1, 1, 2, 1];
const FIVE_LINE_OPPOSITE_NUMERATOR_ENDPOINT_PROBE: [i64; 6] = [-1, 1, 1, 1, 1, 2];
const FIVE_LINE_OPPOSITE_NUMERATOR_BULK_PROBE: [i64; 6] = [-2, 1, 1, 1, 1, 2];
const FIVE_LINE_NUMERATOR_OVERLAP_PROBE: [i64; 6] = [-1, 1, 1, 1, 2, 2];
const FOUR_LINE_CORNER: [i64; 6] = [0, 1, 1, 1, 1, 0];
const FOUR_LINE_BULK_DOT_PROBE: [i64; 6] = [0, 2, 2, 2, 3, 0];
const FOUR_LINE_COMPLEMENTARY_MIXED_DOT_PROBE: [i64; 6] = [0, 1, 2, 3, 2, 0];
const FOUR_LINE_SCALAR_NUMERATOR_ENDPOINT_PROBE: [i64; 6] = [0, 1, 1, 1, 1, -1];
const FOUR_LINE_SCALAR_NUMERATOR_BULK_PROBE: [i64; 6] = [0, 1, 1, 1, 1, -2];
const FOUR_LINE_INCIDENT_TWO_DOT_NUMERATOR_ENDPOINT_PROBE: [i64; 6] = [0, 1, 2, 2, 1, -1];
const FOUR_LINE_OPPOSITE_INACTIVE_NUMERATOR_PAIR_ENDPOINT_PROBE: [i64; 6] = [-1, 1, 1, 1, 1, -1];
const FOUR_LINE_OPPOSITE_INACTIVE_NUMERATOR_PAIR_DOT_ENDPOINT_PROBE: [i64; 6] =
    [-1, 1, 1, 1, 2, -1];
const FOUR_LINE_FACTORIZED_BRIDGE_DOT_NUMERATOR_ENDPOINT_PROBE: [i64; 6] = [0, -1, 2, 1, 1, 1];
const FOUR_LINE_FACTORIZED_BRIDGE_DOT_NUMERATOR_BULK_PROBE: [i64; 6] = [0, -2, 2, 1, 1, 1];
const FOUR_LINE_FACTORIZED_FACE_NUMERATOR_ENDPOINT_PROBE: [i64; 6] = [0, -1, 1, 1, 1, 1];
const FOUR_LINE_FACTORIZED_TWO_DOT_NUMERATOR_ENDPOINT_PROBE: [i64; 6] = [0, -1, 2, 2, 1, 1];
const FOUR_LINE_FACTORIZED_BRIDGE_OPPOSITE_TRIANGLE_DOT_NUMERATOR_RAY_PROBE: [i64; 6] =
    [0, -1, 1, 1, 2, 1];
const FOUR_LINE_FACTORIZED_OPPOSITE_EDGE_DOT_NUMERATOR_ENDPOINT_PROBE: [i64; 6] =
    [0, -1, 1, 2, 1, 1];
const FOUR_LINE_FACTORIZED_OPPOSITE_EDGE_REPEATED_DOT_NUMERATOR_ENDPOINT_PROBE: [i64; 6] =
    [0, -1, 1, 3, 1, 1];
const FOUR_LINE_DOTTED_NEGATIVE_NUMERATOR_BULK_PROBE: [i64; 6] = [0, 1, 1, 1, 2, -2];
const THREE_LINE_DECORATED_PATH_NUMERATOR_ENDPOINT_PROBE: [i64; 6] = [0, 0, 2, -1, 1, 1];
const THREE_LINE_DECORATED_PATH_NUMERATOR_BULK_PROBE: [i64; 6] = [0, 0, 2, -2, 1, 1];
const THREE_LINE_BRIDGE_DESCENDANT_DOT_NUMERATOR_ENDPOINT_PROBE: [i64; 6] = [-1, 0, 1, 0, 2, 1];
const THREE_LINE_INCIDENT_PATH_DOT_NUMERATOR_ENDPOINT_PROBE: [i64; 6] = [0, 0, 1, -1, 2, 1];
const THREE_LINE_FACTORIZED_PATH_MIDDLE_DOT_NUMERATOR_RAY_PROBE: [i64; 6] = [0, 0, 1, -1, 1, 2];
const THREE_LINE_FACTORIZED_STAR_SPOKE_DOT_NUMERATOR_RAY_PROBE: [i64; 6] = [0, 0, 1, 1, -1, 2];
const THREE_LINE_UNDOTTED_PATH_NUMERATOR_ENDPOINT_PROBE: [i64; 6] = [0, 0, 1, -1, 1, 1];
const THREE_LINE_UNDOTTED_PATH_NUMERATOR_BULK_PROBE: [i64; 6] = [0, 0, 1, -2, 1, 1];
const FACTORIZATION_SECTORS: [[i64; 6]; 3] =
    [[0, 0, 1, 1, 1, 1], [0, 0, 1, 1, 0, 1], [0, 0, 1, 0, 1, 1]];
const ZERO_PROBE: [i64; 6] = [0, 0, 0, 1, 1, 1];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum K6CellKind {
    Top,
    FiveLineScalarNumeratorEndpoint,
    FiveLineScalarNumeratorBulk,
    FiveLineAdjacentNumeratorEndpoint,
    FiveLineAdjacentNumeratorBulk,
    FiveLineOppositeNumeratorEndpoint,
    FiveLineOppositeNumeratorBulk,
    FiveLineAdjacent,
    FiveLineOpposite,
    FourLineIsolated,
    FourLineOppositePair,
    FourLineAdjacentPair,
    FourLineTriple,
    FourLineThreeDistinct,
    FourLineAdjacentMixedDot,
    FourLineOppositeMixedDot,
    FourLineComplementaryMixedDot,
    FourLineMixedDotRay,
    FourLineRepeatedDotRay,
    FourLineScalarNumeratorEndpoint,
    FourLineScalarNumeratorBulk,
    FourLineFactorizedBridgeDotNumeratorEndpoint,
    FourLineFactorizedBridgeDotNumeratorBulk,
    FourLineFactorizedFaceNumeratorEndpoint,
    FourLineFactorizedTwoDotNumeratorEndpoint,
    FourLineFactorizedBridgeOppositeTriangleDotNumeratorRay,
    FourLineFactorizedOppositeEdgeDotNumeratorEndpoint,
    FourLineFactorizedOppositeEdgeRepeatedDotNumeratorEndpoint,
    FourLineIncidentTwoDotNumeratorEndpoint,
    FourLineOppositeInactiveNumeratorPairEndpoint,
    FourLineOppositeInactiveNumeratorPairDotEndpoint,
    FourLineDottedNegativeNumeratorBulk,
    ThreeLineBridgeDescendantDotNumeratorEndpoint,
    ThreeLineIncidentPathDotNumeratorEndpoint,
    ThreeLineFactorizedPathMiddleDotNumeratorRay,
    ThreeLineFactorizedStarSpokeDotNumeratorRay,
    ThreeLineDecoratedPathNumeratorEndpoint,
    ThreeLineDecoratedPathNumeratorBulk,
    ThreeLineUndottedPathNumeratorEndpoint,
    ThreeLineUndottedPathNumeratorBulk,
    FourLineDotBulk,
    FourLineMixedNumerator,
}

struct OwnedCell {
    kind: K6CellKind,
    cell: RuleCell,
}

/// Cohesive owner of one finite K=6 discovery experiment.
struct K6ReachabilityCensus {
    terminals: K6ReachabilityTerminals,
    cells: Box<[OwnedCell]>,
    roots: Box<[IntegralKey]>,
}

impl K6ReachabilityCensus {
    fn try_new() -> Result<Self, ArtifactError> {
        let terminals = K6ReachabilityTerminals::try_new()?;
        let (_top_context, top) = derive_top_cell()?;
        let (
            _five_line_context,
            FiveLineCellSet {
                adjacent_dot: five_adjacent,
                opposite_dot: five_opposite,
                scalar_numerator_endpoint,
                scalar_numerator_bulk,
                adjacent_numerator_endpoint,
                adjacent_numerator_bulk,
                opposite_numerator_endpoint,
                opposite_numerator_bulk,
            },
        ) = derive_five_line_cells()?;
        let FourLineCellSet {
            isolated,
            opposite,
            adjacent,
            triple,
            three_distinct,
            adjacent_mixed_dot,
            opposite_mixed_dot,
            complementary_mixed_dot,
            mixed_dot_ray,
            repeated_dot_ray,
            scalar_numerator_endpoint: four_line_scalar_numerator_endpoint,
            scalar_numerator_bulk: four_line_scalar_numerator_bulk,
            factorized_bridge_dot_numerator_endpoint,
            factorized_bridge_dot_numerator_bulk,
            factorized_face_numerator_endpoint,
            factorized_two_dot_numerator_endpoint,
            factorized_bridge_opposite_triangle_dot_numerator_ray,
            factorized_opposite_edge_dot_numerator_endpoint,
            factorized_opposite_edge_repeated_dot_numerator_endpoint,
            incident_two_dot_numerator_endpoint,
            opposite_inactive_numerator_pair_endpoint,
            opposite_inactive_numerator_pair_dot_endpoint,
            dotted_negative_numerator_bulk,
            canonical_dot,
            mixed_numerator,
        } = derive_all_four_line_cells()?;
        let ThreeLineCellSet {
            bridge_descendant_dot_numerator_endpoint,
            incident_path_dot_numerator_endpoint,
            factorized_path_middle_dot_numerator_ray,
            factorized_star_spoke_dot_numerator_ray,
            decorated_path_numerator_endpoint,
            decorated_path_numerator_bulk,
            undotted_path_numerator_endpoint,
            undotted_path_numerator_bulk,
        } = derive_three_line_cells()?;

        // First-applicable order is explicit. Five-line numerator endpoints
        // precede their disjoint bulk rays, and the adjacent active-dot lane
        // owns its genuine overlap with the opposite lane. Exact four-line
        // corner exceptions and selected-source rays likewise precede broad
        // positive boxes.  Each three-line endpoint precedes its disjoint
        // full-i64 bulk lane; none of these numerator cells is itself a
        // factorization owner.
        let cells = vec![
            OwnedCell {
                kind: K6CellKind::Top,
                cell: top,
            },
            OwnedCell {
                kind: K6CellKind::FiveLineScalarNumeratorEndpoint,
                cell: scalar_numerator_endpoint,
            },
            OwnedCell {
                kind: K6CellKind::FiveLineScalarNumeratorBulk,
                cell: scalar_numerator_bulk,
            },
            OwnedCell {
                kind: K6CellKind::FiveLineAdjacentNumeratorEndpoint,
                cell: adjacent_numerator_endpoint,
            },
            OwnedCell {
                kind: K6CellKind::FiveLineAdjacentNumeratorBulk,
                cell: adjacent_numerator_bulk,
            },
            OwnedCell {
                kind: K6CellKind::FiveLineOppositeNumeratorEndpoint,
                cell: opposite_numerator_endpoint,
            },
            OwnedCell {
                kind: K6CellKind::FiveLineOppositeNumeratorBulk,
                cell: opposite_numerator_bulk,
            },
            OwnedCell {
                kind: K6CellKind::FiveLineAdjacent,
                cell: five_adjacent,
            },
            OwnedCell {
                kind: K6CellKind::FiveLineOpposite,
                cell: five_opposite,
            },
            OwnedCell {
                kind: K6CellKind::FourLineIsolated,
                cell: isolated,
            },
            OwnedCell {
                kind: K6CellKind::FourLineOppositePair,
                cell: opposite,
            },
            OwnedCell {
                kind: K6CellKind::FourLineAdjacentPair,
                cell: adjacent,
            },
            OwnedCell {
                kind: K6CellKind::FourLineTriple,
                cell: triple,
            },
            OwnedCell {
                kind: K6CellKind::FourLineThreeDistinct,
                cell: three_distinct,
            },
            OwnedCell {
                kind: K6CellKind::FourLineAdjacentMixedDot,
                cell: adjacent_mixed_dot,
            },
            OwnedCell {
                kind: K6CellKind::FourLineOppositeMixedDot,
                cell: opposite_mixed_dot,
            },
            OwnedCell {
                kind: K6CellKind::FourLineComplementaryMixedDot,
                cell: complementary_mixed_dot,
            },
            OwnedCell {
                kind: K6CellKind::FourLineMixedDotRay,
                cell: mixed_dot_ray,
            },
            OwnedCell {
                kind: K6CellKind::FourLineRepeatedDotRay,
                cell: repeated_dot_ray,
            },
            OwnedCell {
                kind: K6CellKind::FourLineScalarNumeratorEndpoint,
                cell: four_line_scalar_numerator_endpoint,
            },
            OwnedCell {
                kind: K6CellKind::FourLineScalarNumeratorBulk,
                cell: four_line_scalar_numerator_bulk,
            },
            OwnedCell {
                kind: K6CellKind::FourLineFactorizedBridgeDotNumeratorEndpoint,
                cell: factorized_bridge_dot_numerator_endpoint,
            },
            OwnedCell {
                kind: K6CellKind::FourLineFactorizedBridgeDotNumeratorBulk,
                cell: factorized_bridge_dot_numerator_bulk,
            },
            OwnedCell {
                kind: K6CellKind::FourLineFactorizedFaceNumeratorEndpoint,
                cell: factorized_face_numerator_endpoint,
            },
            OwnedCell {
                kind: K6CellKind::FourLineFactorizedTwoDotNumeratorEndpoint,
                cell: factorized_two_dot_numerator_endpoint,
            },
            OwnedCell {
                kind: K6CellKind::FourLineFactorizedBridgeOppositeTriangleDotNumeratorRay,
                cell: factorized_bridge_opposite_triangle_dot_numerator_ray,
            },
            OwnedCell {
                kind: K6CellKind::FourLineFactorizedOppositeEdgeDotNumeratorEndpoint,
                cell: factorized_opposite_edge_dot_numerator_endpoint,
            },
            OwnedCell {
                kind: K6CellKind::FourLineFactorizedOppositeEdgeRepeatedDotNumeratorEndpoint,
                cell: factorized_opposite_edge_repeated_dot_numerator_endpoint,
            },
            OwnedCell {
                kind: K6CellKind::FourLineIncidentTwoDotNumeratorEndpoint,
                cell: incident_two_dot_numerator_endpoint,
            },
            OwnedCell {
                kind: K6CellKind::FourLineOppositeInactiveNumeratorPairEndpoint,
                cell: opposite_inactive_numerator_pair_endpoint,
            },
            OwnedCell {
                kind: K6CellKind::FourLineOppositeInactiveNumeratorPairDotEndpoint,
                cell: opposite_inactive_numerator_pair_dot_endpoint,
            },
            OwnedCell {
                kind: K6CellKind::FourLineDottedNegativeNumeratorBulk,
                cell: dotted_negative_numerator_bulk,
            },
            OwnedCell {
                kind: K6CellKind::ThreeLineBridgeDescendantDotNumeratorEndpoint,
                cell: bridge_descendant_dot_numerator_endpoint,
            },
            OwnedCell {
                kind: K6CellKind::ThreeLineIncidentPathDotNumeratorEndpoint,
                cell: incident_path_dot_numerator_endpoint,
            },
            OwnedCell {
                kind: K6CellKind::ThreeLineFactorizedPathMiddleDotNumeratorRay,
                cell: factorized_path_middle_dot_numerator_ray,
            },
            OwnedCell {
                kind: K6CellKind::ThreeLineFactorizedStarSpokeDotNumeratorRay,
                cell: factorized_star_spoke_dot_numerator_ray,
            },
            OwnedCell {
                kind: K6CellKind::ThreeLineDecoratedPathNumeratorEndpoint,
                cell: decorated_path_numerator_endpoint,
            },
            OwnedCell {
                kind: K6CellKind::ThreeLineDecoratedPathNumeratorBulk,
                cell: decorated_path_numerator_bulk,
            },
            OwnedCell {
                kind: K6CellKind::ThreeLineUndottedPathNumeratorEndpoint,
                cell: undotted_path_numerator_endpoint,
            },
            OwnedCell {
                kind: K6CellKind::ThreeLineUndottedPathNumeratorBulk,
                cell: undotted_path_numerator_bulk,
            },
            OwnedCell {
                kind: K6CellKind::FourLineDotBulk,
                cell: canonical_dot,
            },
            OwnedCell {
                kind: K6CellKind::FourLineMixedNumerator,
                cell: mixed_numerator,
            },
        ]
        .into_boxed_slice();
        if cells
            .iter()
            .any(|owned| owned.cell.rule().family_fingerprint() != terminals.family_fingerprint())
        {
            return Err(ArtifactError::WrongFamily);
        }
        let roots = bounded_roots()?.into_boxed_slice();
        Ok(Self {
            terminals,
            cells,
            roots,
        })
    }

    fn discover(
        &self,
        limits: ReachabilityLimits,
    ) -> Result<ReachabilityFrontier, ReachabilityError> {
        let planner = ReachabilityPlanner::try_new(
            self.terminals.context(),
            OrderingPolicy::default(),
            Some(self.terminals.canonicalizer()),
            self.cells.iter().map(|owned| &owned.cell),
            limits,
        )?;
        planner.discover(&self.roots, &self.terminals)
    }

    fn cell_kinds(&self) -> impl Iterator<Item = K6CellKind> + '_ {
        self.cells.iter().map(|owned| owned.kind)
    }

    fn cell_kind(&self, ordinal: usize) -> K6CellKind {
        self.cells[ordinal].kind
    }

    fn canonical_key(&self, powers: [i64; 6]) -> IntegralKey {
        let raw = IntegralKey::try_new(powers).unwrap();
        IntegralKey::try_new(
            self.terminals
                .canonicalizer()
                .canonicalize(&raw)
                .unwrap()
                .canonical()
                .powers()
                .iter()
                .copied(),
        )
        .unwrap()
    }
}

fn bounded_roots() -> Result<Vec<IntegralKey>, ArtifactError> {
    let mut roots = Vec::new();
    append_diamond(&mut roots, TOP_CORNER, 1)?;
    append_diamond(&mut roots, FIVE_LINE_CORNER, 1)?;
    roots.push(IntegralKey::try_new(FIVE_LINE_OVERLAP_PROBE)?);
    for probe in [
        FIVE_LINE_SCALAR_NUMERATOR_BULK_PROBE,
        FIVE_LINE_ADJACENT_NUMERATOR_ENDPOINT_PROBE,
        FIVE_LINE_ADJACENT_NUMERATOR_BULK_PROBE,
        FIVE_LINE_OPPOSITE_NUMERATOR_ENDPOINT_PROBE,
        FIVE_LINE_OPPOSITE_NUMERATOR_BULK_PROBE,
        FIVE_LINE_NUMERATOR_OVERLAP_PROBE,
    ] {
        roots.push(IntegralKey::try_new(probe)?);
    }
    append_diamond(&mut roots, FOUR_LINE_CORNER, 3)?;
    // This complete diamond already includes the scalar numerator endpoint
    // and the first two bulk-ray targets. Keep them as named probes in the
    // assertions below without duplicating submitted roots here.
    // The broad positive-box recurrence starts beyond the depth-three corner
    // diamond; retain one explicit interior representative so every installed
    // discovery cell is exercised.
    roots.push(IntegralKey::try_new(FOUR_LINE_BULK_DOT_PROBE)?);
    roots.push(IntegralKey::try_new(
        FOUR_LINE_COMPLEMENTARY_MIXED_DOT_PROBE,
    )?);
    // The decorated path descendants exercise the undotted endpoint.  Keep a
    // separate bulk representative so every installed three-line cell is
    // covered by this finite census.
    roots.push(IntegralKey::try_new(
        THREE_LINE_UNDOTTED_PATH_NUMERATOR_BULK_PROBE,
    )?);
    for sector in FACTORIZATION_SECTORS {
        roots.push(IntegralKey::try_new(sector)?);
        let dotted: [i64; 6] = std::array::from_fn(|slot| {
            if slot == first_active_slot(&sector) {
                sector[slot] + 1
            } else {
                sector[slot]
            }
        });
        roots.push(IntegralKey::try_new(dotted)?);
    }
    roots.push(IntegralKey::try_new(ZERO_PROBE)?);
    Ok(roots)
}

fn append_diamond(
    roots: &mut Vec<IntegralKey>,
    anchor: [i64; 6],
    depth: usize,
) -> Result<(), ArtifactError> {
    let diamond = SectorSearchDiamond::try_new(
        IntegralKey::try_new(anchor)?,
        depth,
        SectorSearchLimits {
            max_depth: 3,
            max_offsets: 128,
            max_offset_coordinate_cells: 768,
        },
    )?;
    for offset in diamond.offsets() {
        roots.push(shifted_key(&anchor, offset)?);
    }
    Ok(())
}

fn shifted_key(anchor: &[i64; 6], offset: &IntegralShift) -> Result<IntegralKey, ArtifactError> {
    let mut powers = Vec::with_capacity(anchor.len());
    for (&power, &shift) in anchor.iter().zip(offset.values()) {
        powers.push(
            power
                .checked_add(shift)
                .ok_or(ArtifactError::InvalidReplayEvidence {
                    detail: "bounded K=6 census root arithmetic overflowed",
                })?,
        );
    }
    Ok(IntegralKey::try_new(powers)?)
}

fn first_active_slot(sector: &[i64; 6]) -> usize {
    sector
        .iter()
        .position(|&power| power >= 1)
        .expect("each factorization probe has an active denominator")
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use crate::foundry::search::{ReachabilityDisposition, ReachabilityTerminalKind};

    use super::*;

    fn census_limits() -> ReachabilityLimits {
        ReachabilityLimits {
            max_rule_cells: 42,
            max_roots: 115,
            max_discovered_nodes: 2_048,
            max_pending_nodes: 1_024,
            max_retained_lattice_coordinate_cells: 100_000,
            max_dependency_edges: 4_096,
            max_rule_cell_probes: 20_000,
            max_guard_specializations: 20_000,
            max_coefficient_specializations: 20_000,
            ..ReachabilityLimits::default()
        }
    }

    #[test]
    fn finite_k6_census_reports_current_coverage_without_promoting_corners() {
        let census = K6ReachabilityCensus::try_new().unwrap();
        assert_eq!(census.roots.len(), 115);
        assert_eq!(
            census.cell_kinds().collect::<Vec<_>>(),
            [
                K6CellKind::Top,
                K6CellKind::FiveLineScalarNumeratorEndpoint,
                K6CellKind::FiveLineScalarNumeratorBulk,
                K6CellKind::FiveLineAdjacentNumeratorEndpoint,
                K6CellKind::FiveLineAdjacentNumeratorBulk,
                K6CellKind::FiveLineOppositeNumeratorEndpoint,
                K6CellKind::FiveLineOppositeNumeratorBulk,
                K6CellKind::FiveLineAdjacent,
                K6CellKind::FiveLineOpposite,
                K6CellKind::FourLineIsolated,
                K6CellKind::FourLineOppositePair,
                K6CellKind::FourLineAdjacentPair,
                K6CellKind::FourLineTriple,
                K6CellKind::FourLineThreeDistinct,
                K6CellKind::FourLineAdjacentMixedDot,
                K6CellKind::FourLineOppositeMixedDot,
                K6CellKind::FourLineComplementaryMixedDot,
                K6CellKind::FourLineMixedDotRay,
                K6CellKind::FourLineRepeatedDotRay,
                K6CellKind::FourLineScalarNumeratorEndpoint,
                K6CellKind::FourLineScalarNumeratorBulk,
                K6CellKind::FourLineFactorizedBridgeDotNumeratorEndpoint,
                K6CellKind::FourLineFactorizedBridgeDotNumeratorBulk,
                K6CellKind::FourLineFactorizedFaceNumeratorEndpoint,
                K6CellKind::FourLineFactorizedTwoDotNumeratorEndpoint,
                K6CellKind::FourLineFactorizedBridgeOppositeTriangleDotNumeratorRay,
                K6CellKind::FourLineFactorizedOppositeEdgeDotNumeratorEndpoint,
                K6CellKind::FourLineFactorizedOppositeEdgeRepeatedDotNumeratorEndpoint,
                K6CellKind::FourLineIncidentTwoDotNumeratorEndpoint,
                K6CellKind::FourLineOppositeInactiveNumeratorPairEndpoint,
                K6CellKind::FourLineOppositeInactiveNumeratorPairDotEndpoint,
                K6CellKind::FourLineDottedNegativeNumeratorBulk,
                K6CellKind::ThreeLineBridgeDescendantDotNumeratorEndpoint,
                K6CellKind::ThreeLineIncidentPathDotNumeratorEndpoint,
                K6CellKind::ThreeLineFactorizedPathMiddleDotNumeratorRay,
                K6CellKind::ThreeLineFactorizedStarSpokeDotNumeratorRay,
                K6CellKind::ThreeLineDecoratedPathNumeratorEndpoint,
                K6CellKind::ThreeLineDecoratedPathNumeratorBulk,
                K6CellKind::ThreeLineUndottedPathNumeratorEndpoint,
                K6CellKind::ThreeLineUndottedPathNumeratorBulk,
                K6CellKind::FourLineDotBulk,
                K6CellKind::FourLineMixedNumerator,
            ]
        );
        assert_eq!(census.terminals.factorization_rule_count(), 3);
        assert_eq!(census.terminals.zero_sectors().len(), 26);

        let first = census.discover(census_limits()).unwrap();
        let second = census.discover(census_limits()).unwrap();
        assert_eq!(first, second);
        let mut owners = BTreeMap::<K6CellKind, usize>::new();
        let mut terminals = BTreeMap::<ReachabilityTerminalKind, usize>::new();
        for node in first.nodes() {
            match node.disposition() {
                ReachabilityDisposition::Rule(application) => {
                    *owners
                        .entry(census.cell_kind(application.cell_ordinal()))
                        .or_default() += 1;
                }
                ReachabilityDisposition::Terminal(terminal) => {
                    *terminals.entry(terminal.kind()).or_default() += 1;
                }
                ReachabilityDisposition::Uncovered => {}
            }
        }
        assert_eq!(
            owners,
            BTreeMap::from([
                (K6CellKind::Top, 1),
                (K6CellKind::FiveLineScalarNumeratorEndpoint, 1),
                (K6CellKind::FiveLineScalarNumeratorBulk, 1),
                (K6CellKind::FiveLineAdjacentNumeratorEndpoint, 2),
                (K6CellKind::FiveLineAdjacentNumeratorBulk, 1),
                (K6CellKind::FiveLineOppositeNumeratorEndpoint, 1),
                (K6CellKind::FiveLineOppositeNumeratorBulk, 1),
                (K6CellKind::FiveLineAdjacent, 2),
                (K6CellKind::FiveLineOpposite, 2),
                (K6CellKind::FourLineIsolated, 1),
                (K6CellKind::FourLineOppositePair, 1),
                (K6CellKind::FourLineAdjacentPair, 1),
                (K6CellKind::FourLineTriple, 1),
                (K6CellKind::FourLineThreeDistinct, 1),
                (K6CellKind::FourLineAdjacentMixedDot, 1),
                (K6CellKind::FourLineOppositeMixedDot, 1),
                (K6CellKind::FourLineComplementaryMixedDot, 1),
                (K6CellKind::FourLineMixedDotRay, 2),
                (K6CellKind::FourLineRepeatedDotRay, 2),
                (K6CellKind::FourLineScalarNumeratorEndpoint, 1),
                (K6CellKind::FourLineScalarNumeratorBulk, 2),
                (K6CellKind::FourLineFactorizedBridgeDotNumeratorEndpoint, 1,),
                (K6CellKind::FourLineFactorizedBridgeDotNumeratorBulk, 1,),
                (K6CellKind::FourLineFactorizedFaceNumeratorEndpoint, 1,),
                (K6CellKind::FourLineFactorizedTwoDotNumeratorEndpoint, 1,),
                (
                    K6CellKind::FourLineFactorizedBridgeOppositeTriangleDotNumeratorRay,
                    1,
                ),
                (
                    K6CellKind::FourLineFactorizedOppositeEdgeDotNumeratorEndpoint,
                    1,
                ),
                (
                    K6CellKind::FourLineFactorizedOppositeEdgeRepeatedDotNumeratorEndpoint,
                    1,
                ),
                (K6CellKind::FourLineIncidentTwoDotNumeratorEndpoint, 1,),
                (K6CellKind::FourLineOppositeInactiveNumeratorPairEndpoint, 1,),
                (
                    K6CellKind::FourLineOppositeInactiveNumeratorPairDotEndpoint,
                    1,
                ),
                (K6CellKind::FourLineDottedNegativeNumeratorBulk, 1,),
                (K6CellKind::ThreeLineBridgeDescendantDotNumeratorEndpoint, 1,),
                (K6CellKind::ThreeLineIncidentPathDotNumeratorEndpoint, 1,),
                (K6CellKind::ThreeLineFactorizedPathMiddleDotNumeratorRay, 1,),
                (K6CellKind::ThreeLineFactorizedStarSpokeDotNumeratorRay, 1,),
                (K6CellKind::ThreeLineDecoratedPathNumeratorEndpoint, 1),
                (K6CellKind::ThreeLineDecoratedPathNumeratorBulk, 1),
                (K6CellKind::ThreeLineUndottedPathNumeratorEndpoint, 1),
                (K6CellKind::ThreeLineUndottedPathNumeratorBulk, 1),
                (K6CellKind::FourLineDotBulk, 1),
                (K6CellKind::FourLineMixedNumerator, 4),
            ])
        );
        assert_eq!(
            terminals,
            BTreeMap::from([
                (ReachabilityTerminalKind::ZeroSector, 1),
                (ReachabilityTerminalKind::Factorization, 26),
            ])
        );

        let statistics = first.statistics();
        assert_eq!(statistics.submitted_roots(), 115);
        assert_eq!(statistics.canonical_roots(), 44);
        assert_eq!(statistics.discovered_nodes(), 88);
        assert_eq!(statistics.terminal_nodes(), 27);
        assert_eq!(statistics.rule_applications(), 51);
        assert_eq!(statistics.uncovered_nodes(), 10);

        for (powers, kind) in [
            ([1, 1, 1, 1, 1, 2], K6CellKind::Top),
            (
                [-1, 1, 1, 1, 1, 1],
                K6CellKind::FiveLineScalarNumeratorEndpoint,
            ),
            (
                FIVE_LINE_SCALAR_NUMERATOR_BULK_PROBE,
                K6CellKind::FiveLineScalarNumeratorBulk,
            ),
            (
                FIVE_LINE_ADJACENT_NUMERATOR_ENDPOINT_PROBE,
                K6CellKind::FiveLineAdjacentNumeratorEndpoint,
            ),
            (
                FIVE_LINE_ADJACENT_NUMERATOR_BULK_PROBE,
                K6CellKind::FiveLineAdjacentNumeratorBulk,
            ),
            (
                FIVE_LINE_OPPOSITE_NUMERATOR_ENDPOINT_PROBE,
                K6CellKind::FiveLineOppositeNumeratorEndpoint,
            ),
            (
                FIVE_LINE_OPPOSITE_NUMERATOR_BULK_PROBE,
                K6CellKind::FiveLineOppositeNumeratorBulk,
            ),
            (
                FIVE_LINE_NUMERATOR_OVERLAP_PROBE,
                K6CellKind::FiveLineAdjacentNumeratorEndpoint,
            ),
            ([0, 1, 1, 1, 2, 1], K6CellKind::FiveLineAdjacent),
            (FIVE_LINE_OVERLAP_PROBE, K6CellKind::FiveLineAdjacent),
            ([0, 1, 1, 1, 1, 2], K6CellKind::FiveLineOpposite),
            ([0, 1, 1, 1, 2, 0], K6CellKind::FourLineIsolated),
            ([0, 1, 2, 1, 2, 0], K6CellKind::FourLineOppositePair),
            ([0, 1, 1, 2, 2, 0], K6CellKind::FourLineAdjacentPair),
            ([0, 1, 1, 1, 3, 0], K6CellKind::FourLineTriple),
            ([0, 1, 2, 2, 2, 0], K6CellKind::FourLineThreeDistinct),
            ([0, 1, 1, 2, 3, 0], K6CellKind::FourLineAdjacentMixedDot),
            ([0, 1, 2, 1, 3, 0], K6CellKind::FourLineOppositeMixedDot),
            (
                FOUR_LINE_COMPLEMENTARY_MIXED_DOT_PROBE,
                K6CellKind::FourLineComplementaryMixedDot,
            ),
            ([0, 1, 2, 2, 3, 0], K6CellKind::FourLineMixedDotRay),
            ([0, 1, 2, 2, 4, 0], K6CellKind::FourLineMixedDotRay),
            ([0, 1, 1, 1, 4, 0], K6CellKind::FourLineRepeatedDotRay),
            (
                FOUR_LINE_SCALAR_NUMERATOR_ENDPOINT_PROBE,
                K6CellKind::FourLineScalarNumeratorEndpoint,
            ),
            (
                FOUR_LINE_SCALAR_NUMERATOR_BULK_PROBE,
                K6CellKind::FourLineScalarNumeratorBulk,
            ),
            ([0, 1, 1, 1, 1, -3], K6CellKind::FourLineScalarNumeratorBulk),
            (
                FOUR_LINE_FACTORIZED_BRIDGE_DOT_NUMERATOR_ENDPOINT_PROBE,
                K6CellKind::FourLineFactorizedBridgeDotNumeratorEndpoint,
            ),
            (
                FOUR_LINE_FACTORIZED_BRIDGE_DOT_NUMERATOR_BULK_PROBE,
                K6CellKind::FourLineFactorizedBridgeDotNumeratorBulk,
            ),
            (
                FOUR_LINE_FACTORIZED_FACE_NUMERATOR_ENDPOINT_PROBE,
                K6CellKind::FourLineFactorizedFaceNumeratorEndpoint,
            ),
            (
                FOUR_LINE_FACTORIZED_TWO_DOT_NUMERATOR_ENDPOINT_PROBE,
                K6CellKind::FourLineFactorizedTwoDotNumeratorEndpoint,
            ),
            (
                FOUR_LINE_FACTORIZED_BRIDGE_OPPOSITE_TRIANGLE_DOT_NUMERATOR_RAY_PROBE,
                K6CellKind::FourLineFactorizedBridgeOppositeTriangleDotNumeratorRay,
            ),
            (
                FOUR_LINE_FACTORIZED_OPPOSITE_EDGE_DOT_NUMERATOR_ENDPOINT_PROBE,
                K6CellKind::FourLineFactorizedOppositeEdgeDotNumeratorEndpoint,
            ),
            (
                FOUR_LINE_FACTORIZED_OPPOSITE_EDGE_REPEATED_DOT_NUMERATOR_ENDPOINT_PROBE,
                K6CellKind::FourLineFactorizedOppositeEdgeRepeatedDotNumeratorEndpoint,
            ),
            (
                FOUR_LINE_INCIDENT_TWO_DOT_NUMERATOR_ENDPOINT_PROBE,
                K6CellKind::FourLineIncidentTwoDotNumeratorEndpoint,
            ),
            (
                FOUR_LINE_OPPOSITE_INACTIVE_NUMERATOR_PAIR_ENDPOINT_PROBE,
                K6CellKind::FourLineOppositeInactiveNumeratorPairEndpoint,
            ),
            (
                FOUR_LINE_OPPOSITE_INACTIVE_NUMERATOR_PAIR_DOT_ENDPOINT_PROBE,
                K6CellKind::FourLineOppositeInactiveNumeratorPairDotEndpoint,
            ),
            (
                FOUR_LINE_DOTTED_NEGATIVE_NUMERATOR_BULK_PROBE,
                K6CellKind::FourLineDottedNegativeNumeratorBulk,
            ),
            (
                THREE_LINE_BRIDGE_DESCENDANT_DOT_NUMERATOR_ENDPOINT_PROBE,
                K6CellKind::ThreeLineBridgeDescendantDotNumeratorEndpoint,
            ),
            (
                THREE_LINE_INCIDENT_PATH_DOT_NUMERATOR_ENDPOINT_PROBE,
                K6CellKind::ThreeLineIncidentPathDotNumeratorEndpoint,
            ),
            (
                THREE_LINE_FACTORIZED_PATH_MIDDLE_DOT_NUMERATOR_RAY_PROBE,
                K6CellKind::ThreeLineFactorizedPathMiddleDotNumeratorRay,
            ),
            (
                THREE_LINE_FACTORIZED_STAR_SPOKE_DOT_NUMERATOR_RAY_PROBE,
                K6CellKind::ThreeLineFactorizedStarSpokeDotNumeratorRay,
            ),
            (
                THREE_LINE_DECORATED_PATH_NUMERATOR_ENDPOINT_PROBE,
                K6CellKind::ThreeLineDecoratedPathNumeratorEndpoint,
            ),
            (
                THREE_LINE_DECORATED_PATH_NUMERATOR_BULK_PROBE,
                K6CellKind::ThreeLineDecoratedPathNumeratorBulk,
            ),
            (
                THREE_LINE_UNDOTTED_PATH_NUMERATOR_ENDPOINT_PROBE,
                K6CellKind::ThreeLineUndottedPathNumeratorEndpoint,
            ),
            (
                THREE_LINE_UNDOTTED_PATH_NUMERATOR_BULK_PROBE,
                K6CellKind::ThreeLineUndottedPathNumeratorBulk,
            ),
            (FOUR_LINE_BULK_DOT_PROBE, K6CellKind::FourLineDotBulk),
            ([0, 1, 1, 1, 2, -1], K6CellKind::FourLineMixedNumerator),
        ] {
            assert_rule_kind(&census, &first, powers, kind);
        }

        // Both newly installed singleton owners expose exactly the certified
        // path-factorization child and the still-unresolved scalar four-line
        // corner.  First-applicable ordering must not silently route either
        // target through a broader discovery slice.
        for (powers, kind) in [
            ([0, 1, 1, 2, 3, 0], K6CellKind::FourLineAdjacentMixedDot),
            ([0, 1, 2, 1, 3, 0], K6CellKind::FourLineOppositeMixedDot),
        ] {
            let ReachabilityDisposition::Rule(application) = disposition(&census, &first, powers)
            else {
                panic!("expected mixed-dot singleton application for {powers:?}")
            };
            assert_eq!(census.cell_kind(application.cell_ordinal()), kind);
            assert_eq!(application.assignment(), FOUR_LINE_CORNER);
            assert_eq!(application.dependencies().len(), 2);
            assert_eq!(
                application.dependencies()[0].canonical_child().powers(),
                [0, 0, 2, 0, 2, 2]
            );
            assert_eq!(
                application.dependencies()[1].canonical_child().powers(),
                FOUR_LINE_CORNER
            );
        }
        assert!(matches!(
            disposition(&census, &first, [0, 0, 2, 0, 2, 2]),
            ReachabilityDisposition::Terminal(terminal)
                if terminal.kind() == ReachabilityTerminalKind::Factorization
        ));

        // The generated selected-source ray owns both finite-census points
        // with one parametric cell.  Its exact first child exposes the next
        // same-sector ray honestly; the remaining children descend into the
        // existing repeated-dot and factorization owners.
        for (target_power, free_power) in [(3, 1), (4, 2)] {
            let powers = [0, 1, 2, 2, target_power, 0];
            let ReachabilityDisposition::Rule(application) = disposition(&census, &first, powers)
            else {
                panic!("expected selected-source mixed-dot ray at {powers:?}")
            };
            assert_eq!(
                census.cell_kind(application.cell_ordinal()),
                K6CellKind::FourLineMixedDotRay
            );
            assert_eq!(application.assignment(), [0, 1, 1, 1, free_power, 0]);
            assert_eq!(application.dependencies().len(), 5);
            assert_eq!(
                application.dependencies()[0].canonical_child().powers(),
                [0, 1, 1, 2, free_power + 3, 0]
            );
        }

        // The scalar inactive-numerator endpoint precedes its disjoint bulk
        // ray. Their exact children recurse toward the unresolved scalar
        // corner and expose the pinched numerator face honestly.
        let ReachabilityDisposition::Rule(endpoint) =
            disposition(&census, &first, FOUR_LINE_SCALAR_NUMERATOR_ENDPOINT_PROBE)
        else {
            panic!("expected scalar four-line numerator endpoint")
        };
        assert_eq!(
            census.cell_kind(endpoint.cell_ordinal()),
            K6CellKind::FourLineScalarNumeratorEndpoint
        );
        assert_eq!(endpoint.assignment(), FOUR_LINE_CORNER);
        assert_eq!(endpoint.dependencies().len(), 2);
        assert_eq!(
            endpoint.dependencies()[0].canonical_child().powers(),
            FOUR_LINE_CORNER
        );
        assert_eq!(
            endpoint.dependencies()[1].canonical_child().powers(),
            [0, 0, 1, 0, 2, 1]
        );
        assert!(matches!(
            disposition(&census, &first, [0, 0, 1, 0, 2, 1]),
            ReachabilityDisposition::Terminal(terminal)
                if terminal.kind() == ReachabilityTerminalKind::Factorization
                    && terminal.owner_ordinal() == 2
        ));

        for (target_power, assignment_power) in [(-2, -1), (-3, -2)] {
            let target = [0, 1, 1, 1, 1, target_power];
            let ReachabilityDisposition::Rule(application) = disposition(&census, &first, target)
            else {
                panic!("expected scalar four-line numerator bulk at {target:?}")
            };
            assert_eq!(
                census.cell_kind(application.cell_ordinal()),
                K6CellKind::FourLineScalarNumeratorBulk
            );
            assert_eq!(application.assignment(), [0, 1, 1, 1, 1, assignment_power]);
            assert_eq!(application.dependencies().len(), 3);
            assert_eq!(
                application.dependencies()[0].canonical_child().powers(),
                [0, 1, 1, 1, 1, assignment_power]
            );
            assert_eq!(
                application.dependencies()[1].canonical_child().powers(),
                [0, 0, 2, assignment_power, 1, 1]
            );
            assert_eq!(
                application.dependencies()[2].canonical_child().powers(),
                [0, 1, 1, 1, 1, assignment_power + 1]
            );
            assert!(matches!(
                disposition(&census, &first, [0, 0, 2, assignment_power, 1, 1]),
                ReachabilityDisposition::Rule(_)
            ));
        }

        // The one-dot/deeper-numerator ray is disjoint from the existing
        // N=-1 mixed-numerator boundary.  At its first target all children
        // were already present: the scalar numerator endpoint, the decorated
        // path endpoint, and the still-unresolved scalar four-line corner.
        // The new owner therefore shrinks this finite frontier without
        // manufacturing another obligation.
        let ReachabilityDisposition::Rule(dotted_negative_bulk) = disposition(
            &census,
            &first,
            FOUR_LINE_DOTTED_NEGATIVE_NUMERATOR_BULK_PROBE,
        ) else {
            panic!("expected dotted negative-numerator bulk")
        };
        assert_eq!(
            census.cell_kind(dotted_negative_bulk.cell_ordinal()),
            K6CellKind::FourLineDottedNegativeNumeratorBulk
        );
        assert_eq!(dotted_negative_bulk.assignment(), [0, 1, 1, 1, 1, -1]);
        assert_eq!(
            dotted_negative_bulk
                .dependencies()
                .iter()
                .map(|dependency| dependency.canonical_child().powers())
                .collect::<Vec<_>>(),
            [[0, 1, 1, 1, 1, -1], [0, 0, 2, -1, 1, 1], FOUR_LINE_CORNER,]
        );
        assert_rule_kind(
            &census,
            &first,
            FOUR_LINE_SCALAR_NUMERATOR_ENDPOINT_PROBE,
            K6CellKind::FourLineScalarNumeratorEndpoint,
        );
        assert_rule_kind(
            &census,
            &first,
            THREE_LINE_DECORATED_PATH_NUMERATOR_ENDPOINT_PROBE,
            K6CellKind::ThreeLineDecoratedPathNumeratorEndpoint,
        );
        assert!(matches!(
            disposition(&census, &first, FOUR_LINE_CORNER),
            ReachabilityDisposition::Uncovered
        ));

        // The factorized bridge-dot lane owns exactly its endpoint and bulk
        // orbit.  Its endpoint closes into authenticated factorization
        // terminals; the bulk exposes the adjacent decorated-path and
        // undotted factorized-face obligations without claiming either.
        let ReachabilityDisposition::Rule(bridge_endpoint) = disposition(
            &census,
            &first,
            FOUR_LINE_FACTORIZED_BRIDGE_DOT_NUMERATOR_ENDPOINT_PROBE,
        ) else {
            panic!("expected factorized bridge-dot numerator endpoint")
        };
        assert_eq!(
            census.cell_kind(bridge_endpoint.cell_ordinal()),
            K6CellKind::FourLineFactorizedBridgeDotNumeratorEndpoint
        );
        assert_eq!(bridge_endpoint.assignment(), [0, 0, 1, 1, 1, 1]);
        assert_eq!(bridge_endpoint.dependencies().len(), 2);
        assert_eq!(
            bridge_endpoint.dependencies()[0].canonical_child().powers(),
            [0, 0, 1, 0, 2, 1]
        );
        assert_eq!(
            bridge_endpoint.dependencies()[1].canonical_child().powers(),
            [0, 0, 1, 1, 1, 1]
        );
        for (powers, owner) in [([0, 0, 1, 0, 2, 1], 2), ([0, 0, 1, 1, 1, 1], 0)] {
            assert!(matches!(
                disposition(&census, &first, powers),
                ReachabilityDisposition::Terminal(terminal)
                    if terminal.kind() == ReachabilityTerminalKind::Factorization
                        && terminal.owner_ordinal() == owner
            ));
        }

        let ReachabilityDisposition::Rule(bridge_bulk) = disposition(
            &census,
            &first,
            FOUR_LINE_FACTORIZED_BRIDGE_DOT_NUMERATOR_BULK_PROBE,
        ) else {
            panic!("expected factorized bridge-dot numerator bulk")
        };
        assert_eq!(
            census.cell_kind(bridge_bulk.cell_ordinal()),
            K6CellKind::FourLineFactorizedBridgeDotNumeratorBulk
        );
        assert_eq!(bridge_bulk.assignment(), [0, -1, 1, 1, 1, 1]);
        assert_eq!(bridge_bulk.dependencies().len(), 4);
        assert_eq!(
            bridge_bulk
                .dependencies()
                .iter()
                .map(|dependency| dependency.canonical_child().powers())
                .collect::<Vec<_>>(),
            [
                [-1, 0, 1, 0, 2, 1],
                [0, -1, 1, 1, 1, 1],
                [0, 0, 1, 1, 1, 1],
                [0, 0, 1, 0, 1, 1],
            ]
        );
        assert_rule_kind(
            &census,
            &first,
            THREE_LINE_BRIDGE_DESCENDANT_DOT_NUMERATOR_ENDPOINT_PROBE,
            K6CellKind::ThreeLineBridgeDescendantDotNumeratorEndpoint,
        );
        assert_rule_kind(
            &census,
            &first,
            FOUR_LINE_FACTORIZED_FACE_NUMERATOR_ENDPOINT_PROBE,
            K6CellKind::FourLineFactorizedFaceNumeratorEndpoint,
        );
        for (powers, owner) in [([0, 0, 1, 1, 1, 1], 0), ([0, 0, 1, 0, 1, 1], 2)] {
            assert!(matches!(
                disposition(&census, &first, powers),
                ReachabilityDisposition::Terminal(terminal)
                    if terminal.kind() == ReachabilityTerminalKind::Factorization
                        && terminal.owner_ordinal() == owner
            ));
        }

        // The second bridge-bulk obligation is a separate four-line
        // singleton.  Its three children terminate in already authenticated
        // product sectors, so it also shrinks the frontier without exposing
        // a new numerator lane.
        let ReachabilityDisposition::Rule(factorized_face_endpoint) = disposition(
            &census,
            &first,
            FOUR_LINE_FACTORIZED_FACE_NUMERATOR_ENDPOINT_PROBE,
        ) else {
            panic!("expected factorized-face numerator endpoint")
        };
        assert_eq!(
            census.cell_kind(factorized_face_endpoint.cell_ordinal()),
            K6CellKind::FourLineFactorizedFaceNumeratorEndpoint
        );
        assert_eq!(factorized_face_endpoint.assignment(), [0, 0, 1, 1, 1, 1]);
        assert_eq!(
            factorized_face_endpoint
                .dependencies()
                .iter()
                .map(|dependency| dependency.canonical_child().powers())
                .collect::<Vec<_>>(),
            [[0, 0, 1, 0, 2, 1], [0, 0, 1, 1, 1, 1], [0, 0, 1, 0, 1, 1],]
        );
        for (powers, owner) in [
            ([0, 0, 1, 0, 2, 1], 2),
            ([0, 0, 1, 1, 1, 1], 0),
            ([0, 0, 1, 0, 1, 1], 2),
        ] {
            assert!(matches!(
                disposition(&census, &first, powers),
                ReachabilityDisposition::Terminal(terminal)
                    if terminal.kind() == ReachabilityTerminalKind::Factorization
                        && terminal.owner_ordinal() == owner
            ));
        }

        // This exact-corner numerator cell owns only the S4 placement in
        // which the inactive edge is incident to both active dots. Its four
        // children route through two existing positive-dot cells, one
        // authenticated product, and the still-unresolved scalar corner.
        let ReachabilityDisposition::Rule(incident_two_dot_endpoint) = disposition(
            &census,
            &first,
            FOUR_LINE_INCIDENT_TWO_DOT_NUMERATOR_ENDPOINT_PROBE,
        ) else {
            panic!("expected incident two-dot numerator endpoint")
        };
        assert_eq!(
            census.cell_kind(incident_two_dot_endpoint.cell_ordinal()),
            K6CellKind::FourLineIncidentTwoDotNumeratorEndpoint
        );
        assert_eq!(incident_two_dot_endpoint.assignment(), FOUR_LINE_CORNER);
        assert_eq!(
            incident_two_dot_endpoint
                .dependencies()
                .iter()
                .map(|dependency| dependency.canonical_child().powers())
                .collect::<Vec<_>>(),
            [
                [0, 1, 1, 2, 2, 0],
                [0, 1, 1, 1, 3, 0],
                [0, 0, 1, 0, 2, 2],
                FOUR_LINE_CORNER,
            ]
        );
        assert_rule_kind(
            &census,
            &first,
            [0, 1, 1, 2, 2, 0],
            K6CellKind::FourLineAdjacentPair,
        );
        assert_rule_kind(
            &census,
            &first,
            [0, 1, 1, 1, 3, 0],
            K6CellKind::FourLineTriple,
        );
        assert!(matches!(
            disposition(&census, &first, [0, 0, 1, 0, 2, 2]),
            ReachabilityDisposition::Terminal(terminal)
                if terminal.kind() == ReachabilityTerminalKind::Factorization
                    && terminal.owner_ordinal() == 2
        ));
        assert!(matches!(
            disposition(&census, &first, FOUR_LINE_CORNER),
            ReachabilityDisposition::Uncovered
        ));

        // The two opposite-inactive-numerator endpoints share one exact
        // three-line descendant. Installing that descendant closes the whole
        // three-cell cluster without adding another frontier obligation.
        for (powers, kind, expected_children) in [
            (
                FOUR_LINE_OPPOSITE_INACTIVE_NUMERATOR_PAIR_ENDPOINT_PROBE,
                K6CellKind::FourLineOppositeInactiveNumeratorPairEndpoint,
                vec![
                    FOUR_LINE_SCALAR_NUMERATOR_ENDPOINT_PROBE,
                    FOUR_LINE_SCALAR_NUMERATOR_ENDPOINT_PROBE,
                    THREE_LINE_INCIDENT_PATH_DOT_NUMERATOR_ENDPOINT_PROBE,
                    FOUR_LINE_CORNER,
                ],
            ),
            (
                FOUR_LINE_OPPOSITE_INACTIVE_NUMERATOR_PAIR_DOT_ENDPOINT_PROBE,
                K6CellKind::FourLineOppositeInactiveNumeratorPairDotEndpoint,
                vec![
                    FOUR_LINE_SCALAR_NUMERATOR_ENDPOINT_PROBE,
                    THREE_LINE_INCIDENT_PATH_DOT_NUMERATOR_ENDPOINT_PROBE,
                    FOUR_LINE_CORNER,
                ],
            ),
        ] {
            let ReachabilityDisposition::Rule(application) = disposition(&census, &first, powers)
            else {
                panic!("expected opposite inactive-numerator-pair endpoint at {powers:?}")
            };
            assert_eq!(census.cell_kind(application.cell_ordinal()), kind);
            assert_eq!(
                application.assignment(),
                FOUR_LINE_SCALAR_NUMERATOR_ENDPOINT_PROBE
            );
            assert_eq!(
                application
                    .dependencies()
                    .iter()
                    .map(|dependency| dependency.canonical_child().powers())
                    .collect::<Vec<_>>(),
                expected_children
            );
        }
        assert_rule_kind(
            &census,
            &first,
            FOUR_LINE_SCALAR_NUMERATOR_ENDPOINT_PROBE,
            K6CellKind::FourLineScalarNumeratorEndpoint,
        );

        let ReachabilityDisposition::Rule(incident_path_endpoint) = disposition(
            &census,
            &first,
            THREE_LINE_INCIDENT_PATH_DOT_NUMERATOR_ENDPOINT_PROBE,
        ) else {
            panic!("expected incident path dot/numerator endpoint")
        };
        assert_eq!(
            census.cell_kind(incident_path_endpoint.cell_ordinal()),
            K6CellKind::ThreeLineIncidentPathDotNumeratorEndpoint
        );
        assert_eq!(incident_path_endpoint.assignment(), [0, 0, 1, -1, 1, 1]);
        assert_eq!(
            incident_path_endpoint
                .dependencies()
                .iter()
                .map(|dependency| dependency.canonical_child().powers())
                .collect::<Vec<_>>(),
            [[0, 0, 1, -1, 1, 1], [0, 0, 1, 0, 1, 1]]
        );
        assert_rule_kind(
            &census,
            &first,
            [0, 0, 1, -1, 1, 1],
            K6CellKind::ThreeLineUndottedPathNumeratorEndpoint,
        );
        assert!(matches!(
            disposition(&census, &first, [0, 0, 1, 0, 1, 1]),
            ReachabilityDisposition::Terminal(terminal)
                if terminal.kind() == ReachabilityTerminalKind::Factorization
                    && terminal.owner_ordinal() == 2
        ));

        // The two-dot inactive-numerator placement is a deliberately narrow
        // singleton.  Its compact selected-source replay removes the
        // complete-system's spurious d-1 guard, and every child is discharged
        // by an immutable factorization owner.
        let ReachabilityDisposition::Rule(two_dot_endpoint) = disposition(
            &census,
            &first,
            FOUR_LINE_FACTORIZED_TWO_DOT_NUMERATOR_ENDPOINT_PROBE,
        ) else {
            panic!("expected factorized two-dot numerator endpoint")
        };
        assert_eq!(
            census.cell_kind(two_dot_endpoint.cell_ordinal()),
            K6CellKind::FourLineFactorizedTwoDotNumeratorEndpoint
        );
        assert_eq!(two_dot_endpoint.assignment(), [0, 0, 1, 1, 1, 1]);
        assert_eq!(
            two_dot_endpoint
                .dependencies()
                .iter()
                .map(|dependency| dependency.canonical_child().powers())
                .collect::<Vec<_>>(),
            [
                [0, 0, 1, 0, 2, 2],
                [0, 0, 1, 1, 1, 1],
                [0, 0, 1, 1, 0, 2],
                [0, 0, 1, 0, 1, 2],
            ]
        );
        for (powers, owner) in [
            ([0, 0, 1, 0, 2, 2], 2),
            ([0, 0, 1, 1, 1, 1], 0),
            ([0, 0, 1, 1, 0, 2], 1),
            ([0, 0, 1, 0, 1, 2], 2),
        ] {
            assert!(matches!(
                disposition(&census, &first, powers),
                ReachabilityDisposition::Terminal(terminal)
                    if terminal.kind() == ReachabilityTerminalKind::Factorization
                        && terminal.owner_ordinal() == owner
            ));
        }

        // Three factorized four-line owners share the same exact lower-sector
        // boundary. Two generated positive-dot rays discharge its only
        // nonterminal children directly into authenticated products.
        let factorized_triangle_children = BTreeSet::from([
            THREE_LINE_FACTORIZED_PATH_MIDDLE_DOT_NUMERATOR_RAY_PROBE.to_vec(),
            THREE_LINE_FACTORIZED_STAR_SPOKE_DOT_NUMERATOR_RAY_PROBE.to_vec(),
            vec![0, 0, 1, 0, 2, 1],
            vec![0, 0, 1, 1, 1, 1],
            vec![0, 0, 1, 1, 0, 2],
            vec![0, 0, 1, 0, 1, 2],
            vec![0, 0, 1, 1, 0, 1],
            vec![0, 0, 1, 0, 1, 1],
        ]);
        for (powers, kind) in [
            (
                FOUR_LINE_FACTORIZED_BRIDGE_OPPOSITE_TRIANGLE_DOT_NUMERATOR_RAY_PROBE,
                K6CellKind::FourLineFactorizedBridgeOppositeTriangleDotNumeratorRay,
            ),
            (
                FOUR_LINE_FACTORIZED_OPPOSITE_EDGE_DOT_NUMERATOR_ENDPOINT_PROBE,
                K6CellKind::FourLineFactorizedOppositeEdgeDotNumeratorEndpoint,
            ),
            (
                FOUR_LINE_FACTORIZED_OPPOSITE_EDGE_REPEATED_DOT_NUMERATOR_ENDPOINT_PROBE,
                K6CellKind::FourLineFactorizedOppositeEdgeRepeatedDotNumeratorEndpoint,
            ),
        ] {
            let ReachabilityDisposition::Rule(application) = disposition(&census, &first, powers)
            else {
                panic!("expected factorized triangle dot/numerator owner at {powers:?}")
            };
            assert_eq!(census.cell_kind(application.cell_ordinal()), kind);
            assert_eq!(application.assignment(), FACTORIZATION_SECTORS[0]);
            assert_eq!(
                application
                    .dependencies()
                    .iter()
                    .map(|dependency| dependency.canonical_child().powers().to_vec())
                    .collect::<BTreeSet<_>>(),
                factorized_triangle_children
            );
        }

        for (powers, kind, assignment, owner) in [
            (
                THREE_LINE_FACTORIZED_PATH_MIDDLE_DOT_NUMERATOR_RAY_PROBE,
                K6CellKind::ThreeLineFactorizedPathMiddleDotNumeratorRay,
                [0, 0, 1, 0, 1, 1],
                2,
            ),
            (
                THREE_LINE_FACTORIZED_STAR_SPOKE_DOT_NUMERATOR_RAY_PROBE,
                K6CellKind::ThreeLineFactorizedStarSpokeDotNumeratorRay,
                [0, 0, 1, 1, 0, 1],
                1,
            ),
        ] {
            let ReachabilityDisposition::Rule(application) = disposition(&census, &first, powers)
            else {
                panic!("expected factorized three-line dot/numerator ray at {powers:?}")
            };
            assert_eq!(census.cell_kind(application.cell_ordinal()), kind);
            assert_eq!(application.assignment(), assignment);
            assert_eq!(application.dependencies().len(), 1);
            assert_eq!(
                application.dependencies()[0].canonical_child().powers(),
                assignment
            );
            assert!(matches!(
                disposition(&census, &first, assignment),
                ReachabilityDisposition::Terminal(terminal)
                    if terminal.kind() == ReachabilityTerminalKind::Factorization
                        && terminal.owner_ordinal() == owner
            ));
        }

        // The first bridge-bulk obligation now has its own exact singleton
        // owner.  Both children were already reachable and owned, so this
        // cell removes one uncovered node without growing the frontier.
        let ReachabilityDisposition::Rule(bridge_descendant) = disposition(
            &census,
            &first,
            THREE_LINE_BRIDGE_DESCENDANT_DOT_NUMERATOR_ENDPOINT_PROBE,
        ) else {
            panic!("expected bridge-descendant three-line endpoint")
        };
        assert_eq!(
            census.cell_kind(bridge_descendant.cell_ordinal()),
            K6CellKind::ThreeLineBridgeDescendantDotNumeratorEndpoint
        );
        assert_eq!(bridge_descendant.assignment(), [0, 0, 1, 0, 1, 1]);
        assert_eq!(
            bridge_descendant
                .dependencies()
                .iter()
                .map(|dependency| dependency.canonical_child().powers())
                .collect::<Vec<_>>(),
            [[0, 0, 2, -1, 1, 1], [0, 0, 1, 0, 1, 1]]
        );
        assert_rule_kind(
            &census,
            &first,
            [0, 0, 2, -1, 1, 1],
            K6CellKind::ThreeLineDecoratedPathNumeratorEndpoint,
        );
        assert!(matches!(
            disposition(&census, &first, [0, 0, 1, 0, 1, 1]),
            ReachabilityDisposition::Terminal(terminal)
                if terminal.kind() == ReachabilityTerminalKind::Factorization
                    && terminal.owner_ordinal() == 2
        ));

        // The decorated three-line cells own one certified S4 orbit of the
        // negative dotted path.  They descend into the undotted recurrence,
        // whose scalar n=0 endpoint is an authenticated factorization
        // terminal.  The other decorated-path orbits remain obligations.
        let ReachabilityDisposition::Rule(path_endpoint) = disposition(
            &census,
            &first,
            THREE_LINE_DECORATED_PATH_NUMERATOR_ENDPOINT_PROBE,
        ) else {
            panic!("expected decorated three-line numerator endpoint")
        };
        assert_eq!(
            census.cell_kind(path_endpoint.cell_ordinal()),
            K6CellKind::ThreeLineDecoratedPathNumeratorEndpoint
        );
        assert_eq!(path_endpoint.assignment(), [0, 0, 1, 0, 1, 1]);
        assert_eq!(path_endpoint.dependencies().len(), 1);
        assert_eq!(
            path_endpoint.dependencies()[0].canonical_child().powers(),
            [0, 0, 1, 0, 1, 1]
        );
        assert!(matches!(
            disposition(&census, &first, [0, 0, 1, 0, 1, 1]),
            ReachabilityDisposition::Terminal(terminal)
                if terminal.kind() == ReachabilityTerminalKind::Factorization
                    && terminal.owner_ordinal() == 2
        ));

        let ReachabilityDisposition::Rule(path_bulk) = disposition(
            &census,
            &first,
            THREE_LINE_DECORATED_PATH_NUMERATOR_BULK_PROBE,
        ) else {
            panic!("expected decorated three-line numerator bulk")
        };
        assert_eq!(
            census.cell_kind(path_bulk.cell_ordinal()),
            K6CellKind::ThreeLineDecoratedPathNumeratorBulk
        );
        assert_eq!(path_bulk.assignment(), [0, 0, 1, -1, 1, 1]);
        assert_eq!(path_bulk.dependencies().len(), 2);
        assert_eq!(
            path_bulk.dependencies()[0].canonical_child().powers(),
            [0, 0, 1, -1, 1, 1]
        );
        assert_eq!(
            path_bulk.dependencies()[1].canonical_child().powers(),
            [0, 0, 1, 0, 1, 1]
        );
        assert_rule_kind(
            &census,
            &first,
            [0, 0, 1, -1, 1, 1],
            K6CellKind::ThreeLineUndottedPathNumeratorEndpoint,
        );

        let ReachabilityDisposition::Rule(undotted_bulk) = disposition(
            &census,
            &first,
            THREE_LINE_UNDOTTED_PATH_NUMERATOR_BULK_PROBE,
        ) else {
            panic!("expected undotted three-line numerator bulk")
        };
        assert_eq!(
            census.cell_kind(undotted_bulk.cell_ordinal()),
            K6CellKind::ThreeLineUndottedPathNumeratorBulk
        );
        assert_eq!(undotted_bulk.assignment(), [0, 0, 1, -1, 1, 1]);
        assert_eq!(undotted_bulk.dependencies().len(), 2);
        assert_eq!(
            undotted_bulk.dependencies()[0].canonical_child().powers(),
            [0, 0, 1, -1, 1, 1]
        );
        assert_eq!(
            undotted_bulk.dependencies()[1].canonical_child().powers(),
            [0, 0, 1, 0, 1, 1]
        );

        // Scalar graph corners are obligations, never assumed masters.
        // The additional representatives pin the distinct deeper-dot and
        // inactive-numerator holes exposed by this bounded root set.
        for powers in [
            TOP_CORNER,
            FIVE_LINE_CORNER,
            FOUR_LINE_CORNER,
            [0, 1, 1, 2, 4, 0],
            [0, 1, 1, 2, 5, 0],
        ] {
            assert!(matches!(
                disposition(&census, &first, powers),
                ReachabilityDisposition::Uncovered
            ));
        }
        assert_eq!(
            first
                .uncovered()
                .map(|key| key.powers().to_vec())
                .collect::<Vec<_>>(),
            [
                [0, -1, 1, 2, 2, 1],
                [0, -2, 2, 2, 1, 1],
                [0, 1, 1, 1, 1, 0],
                [-1, 1, 1, 1, 1, -2],
                [0, 1, 1, 2, 4, 0],
                [0, 1, 1, 2, 5, 0],
                [0, 1, 2, 3, 3, 0],
                [0, 1, 3, 2, 3, 0],
                FIVE_LINE_CORNER,
                TOP_CORNER,
            ]
            .map(|powers| powers.to_vec())
        );

        for node in first.nodes() {
            if let ReachabilityDisposition::Terminal(terminal) = node.disposition() {
                assert!(matches!(
                    terminal.kind(),
                    ReachabilityTerminalKind::ZeroSector | ReachabilityTerminalKind::Factorization
                ));
            }
        }

        for (owner_ordinal, sector) in FACTORIZATION_SECTORS.into_iter().enumerate() {
            assert!(matches!(
                disposition(&census, &first, sector),
                ReachabilityDisposition::Terminal(terminal)
                    if terminal.kind() == ReachabilityTerminalKind::Factorization
                        && terminal.owner_ordinal() == owner_ordinal
            ));
        }
        // The newly covered fourth-power ray point produces four distinct
        // canonical lower-sector children, all discharged by the existing
        // exact factorization registry rather than assumed to vanish or to be
        // masters.
        for powers in [
            [0, 0, 2, 2, 3, 0],
            [0, 1, 0, 2, 3, 0],
            [0, 1, 0, 2, 2, 0],
            [0, 0, 1, 1, 3, 0],
        ] {
            assert!(matches!(
                disposition(&census, &first, powers),
                ReachabilityDisposition::Terminal(terminal)
                    if terminal.kind() == ReachabilityTerminalKind::Factorization
            ));
        }
        assert!(matches!(
            disposition(&census, &first, ZERO_PROBE),
            ReachabilityDisposition::Terminal(terminal)
                if terminal.kind() == ReachabilityTerminalKind::ZeroSector
        ));

        let mut exact_node_limit = census_limits();
        exact_node_limit.max_discovered_nodes = statistics.discovered_nodes();
        assert_eq!(census.discover(exact_node_limit).unwrap(), first);
        exact_node_limit.max_discovered_nodes -= 1;
        assert_eq!(
            census.discover(exact_node_limit),
            Err(ReachabilityError::ResourceLimit {
                resource: "discovered nodes",
                requested: statistics.discovered_nodes(),
                limit: statistics.discovered_nodes() - 1,
            })
        );
    }

    fn assert_rule_kind(
        census: &K6ReachabilityCensus,
        frontier: &ReachabilityFrontier,
        powers: [i64; 6],
        expected: K6CellKind,
    ) {
        let ReachabilityDisposition::Rule(application) = disposition(census, frontier, powers)
        else {
            panic!("expected {:?} to be owned by {expected:?}", powers)
        };
        assert_eq!(census.cell_kind(application.cell_ordinal()), expected);
    }

    fn disposition<'frontier>(
        census: &K6ReachabilityCensus,
        frontier: &'frontier ReachabilityFrontier,
        powers: [i64; 6],
    ) -> &'frontier ReachabilityDisposition {
        let target = census.canonical_key(powers);
        frontier
            .nodes()
            .iter()
            .find(|node| node.target() == &target)
            .unwrap_or_else(|| panic!("expected census key {:?}", target.powers()))
            .disposition()
    }
}
