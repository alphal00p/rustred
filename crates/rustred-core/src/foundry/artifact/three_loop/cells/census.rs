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
use super::five_line::derive_five_line_cells;
use super::four_line::{FourLineCellSet, derive_all_four_line_cells};
use super::top::derive_top_cell;

const TOP_CORNER: [i64; 6] = [1; 6];
const FIVE_LINE_CORNER: [i64; 6] = [0, 1, 1, 1, 1, 1];
const FIVE_LINE_OVERLAP_PROBE: [i64; 6] = [0, 1, 1, 1, 2, 2];
const FOUR_LINE_CORNER: [i64; 6] = [0, 1, 1, 1, 1, 0];
const FOUR_LINE_BULK_DOT_PROBE: [i64; 6] = [0, 2, 2, 2, 3, 0];
const FACTORIZATION_SECTORS: [[i64; 6]; 3] =
    [[0, 0, 1, 1, 1, 1], [0, 0, 1, 1, 0, 1], [0, 0, 1, 0, 1, 1]];
const ZERO_PROBE: [i64; 6] = [0, 0, 0, 1, 1, 1];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum K6CellKind {
    Top,
    FiveLineAdjacent,
    FiveLineOpposite,
    FourLineIsolated,
    FourLineOppositePair,
    FourLineAdjacentPair,
    FourLineTriple,
    FourLineThreeDistinct,
    FourLineAdjacentMixedDot,
    FourLineOppositeMixedDot,
    FourLineRepeatedDotRay,
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
        let (_five_line_context, five_adjacent, five_opposite) = derive_five_line_cells()?;
        let FourLineCellSet {
            isolated,
            opposite,
            adjacent,
            triple,
            three_distinct,
            adjacent_mixed_dot,
            opposite_mixed_dot,
            repeated_dot_ray,
            canonical_dot,
            mixed_numerator,
        } = derive_all_four_line_cells()?;

        // First-applicable order is explicit.  Exact corner exceptions own
        // their singleton endpoints before the broad four-line cells.
        let cells = vec![
            OwnedCell {
                kind: K6CellKind::Top,
                cell: top,
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
                kind: K6CellKind::FourLineRepeatedDotRay,
                cell: repeated_dot_ray,
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
    append_diamond(&mut roots, FOUR_LINE_CORNER, 3)?;
    // The broad positive-box recurrence starts beyond the depth-three corner
    // diamond; retain one explicit interior representative so every installed
    // discovery cell is exercised.
    roots.push(IntegralKey::try_new(FOUR_LINE_BULK_DOT_PROBE)?);
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
    use std::collections::BTreeMap;

    use crate::foundry::search::{ReachabilityDisposition, ReachabilityTerminalKind};

    use super::*;

    fn census_limits() -> ReachabilityLimits {
        ReachabilityLimits {
            max_rule_cells: 13,
            max_roots: 107,
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
        assert_eq!(census.roots.len(), 107);
        assert_eq!(
            census.cell_kinds().collect::<Vec<_>>(),
            [
                K6CellKind::Top,
                K6CellKind::FiveLineAdjacent,
                K6CellKind::FiveLineOpposite,
                K6CellKind::FourLineIsolated,
                K6CellKind::FourLineOppositePair,
                K6CellKind::FourLineAdjacentPair,
                K6CellKind::FourLineTriple,
                K6CellKind::FourLineThreeDistinct,
                K6CellKind::FourLineAdjacentMixedDot,
                K6CellKind::FourLineOppositeMixedDot,
                K6CellKind::FourLineRepeatedDotRay,
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
                (K6CellKind::FiveLineAdjacent, 2),
                (K6CellKind::FiveLineOpposite, 1),
                (K6CellKind::FourLineIsolated, 1),
                (K6CellKind::FourLineOppositePair, 1),
                (K6CellKind::FourLineAdjacentPair, 1),
                (K6CellKind::FourLineTriple, 1),
                (K6CellKind::FourLineThreeDistinct, 1),
                (K6CellKind::FourLineAdjacentMixedDot, 1),
                (K6CellKind::FourLineOppositeMixedDot, 1),
                (K6CellKind::FourLineRepeatedDotRay, 1),
                (K6CellKind::FourLineDotBulk, 1),
                (K6CellKind::FourLineMixedNumerator, 4),
            ])
        );
        assert_eq!(
            terminals,
            BTreeMap::from([
                (ReachabilityTerminalKind::ZeroSector, 1),
                (ReachabilityTerminalKind::Factorization, 14),
            ])
        );

        let statistics = first.statistics();
        assert_eq!(statistics.submitted_roots(), 107);
        assert_eq!(statistics.canonical_roots(), 36);
        assert_eq!(statistics.discovered_nodes(), 48);
        assert_eq!(statistics.terminal_nodes(), 15);
        assert_eq!(statistics.rule_applications(), 17);
        assert_eq!(statistics.uncovered_nodes(), 16);

        for (powers, kind) in [
            ([1, 1, 1, 1, 1, 2], K6CellKind::Top),
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
            ([0, 1, 1, 1, 4, 0], K6CellKind::FourLineRepeatedDotRay),
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

        // Scalar graph corners are obligations, never assumed masters.
        // The additional representatives pin the distinct deeper-dot and
        // inactive-numerator holes exposed by this bounded root set.
        for powers in [
            TOP_CORNER,
            FIVE_LINE_CORNER,
            FOUR_LINE_CORNER,
            [0, 1, 1, 1, 1, -1],
            [-1, 1, 1, 1, 1, 1],
            [0, 1, 2, 2, 1, -1],
        ] {
            assert!(matches!(
                disposition(&census, &first, powers),
                ReachabilityDisposition::Uncovered
            ));
        }

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
