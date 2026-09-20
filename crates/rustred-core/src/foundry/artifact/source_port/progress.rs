//! Borrowed, non-authoritative observations of exact checking and installation.

use std::time::Duration;

use super::SourcePortSectorAudit;

/// Independent successor traversals; neither observation grants authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourcePortSuccessorStage {
    Retained,
    ActualCells,
}

/// Structural policy units, not native allocation bytes or wall-clock work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourcePortSuccessorCounts {
    pub boxes: usize,
    pub coordinate_cells: usize,
    pub work: usize,
}

/// The most recent failed charge, recorded before any consumed total changes.
/// Array positions are boxes, coordinate cells, work. `None` denotes checked
/// arithmetic overflow, never a saturated value presented as an exact count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourcePortSuccessorAttempt {
    pub partition_policy: bool,
    pub increment: [Option<usize>; 3],
    pub attempted: [Option<usize>; 3],
    pub limits: SourcePortSuccessorCounts,
    pub exceeded: [bool; 3],
    pub overflow: [bool; 3],
}

/// Fixed-size non-authoritative telemetry. No coefficient, sector vector or
/// event history is allocated. Ordinals are zero-based; in ActualCells,
/// `rule_ordinal` is the global executable-cell ordinal and sector ordinal is
/// absent. Completed sectors count propagation, not merely local audit reports.
/// `traversal_complete` refers only to this successor pass, never publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourcePortSuccessorSnapshot {
    pub stage: SourcePortSuccessorStage,
    pub sector_ordinal: Option<usize>,
    pub rule_ordinal: Option<usize>,
    pub rhs_ordinal: Option<usize>,
    pub application_ordinal: Option<usize>,
    pub completed_sectors: usize,
    pub completed_cells: usize,
    pub consumed: SourcePortSuccessorCounts,
    pub limits: SourcePortSuccessorCounts,
    pub max_arity: usize,
    pub max_uncovered_boxes: usize,
    pub max_uncovered_coordinate_cells: usize,
    pub failed_attempt: Option<SourcePortSuccessorAttempt>,
    /// Requests/count classifications include attempts rejected by policy;
    /// they are not a count of successfully completed mathematical checks.
    pub partition_requests: usize,
    pub singleton_partitions: usize,
    pub split_partitions: usize,
    pub total_pieces: usize,
    pub maximum_pieces: usize,
    pub source_degree_probes: usize,
    pub piece_degree_probes: usize,
    pub skipped_singleton_probes: usize,
    pub source_key_builds: usize,
    pub child_key_builds: usize,
    /// Observational counters stop at their last exact value on overflow.
    pub telemetry_overflow: bool,
    /// A proof-count/degree calculation overflowed; no wrapped value is used.
    pub arithmetic_overflow: bool,
    pub failed: bool,
    pub traversal_complete: bool,
}

/// Progress from installation or the complete total-excess audit.
///
/// Like the solver's `SectorEvent`, these events borrow existing results and
/// introduce no worker pool, synchronization, or coefficient copies. The
/// callback runs serially on the calling thread. It cannot change rules or
/// authorize publication; a callback panic aborts the invocation normally.
/// `elapsed` is wall time since that path's checking entry, including observer work,
/// and is never persisted in an artifact or used for a mathematical choice.
#[derive(Debug)]
pub enum SourcePortInstallEvent<'a, const N: usize> {
    /// Low-frequency successor checkpoint or failure. The sector is borrowed
    /// separately so core telemetry imposes no fixed-width mask or arity cap.
    SuccessorGeometry {
        sector: Option<&'a [bool]>,
        snapshot: &'a SourcePortSuccessorSnapshot,
        elapsed: Duration,
    },
    /// Installation follows the caller's input iterator. The total-excess
    /// diagnostic instead follows its checked harder-to-easier sector order.
    /// `ordinal` is the zero-based position in that path's traversal.
    CheckingSector {
        ordinal: usize,
        sector: [bool; N],
        rules: usize,
        elapsed: Duration,
    },
    /// Start checking one proposed rule, before guard geometry and exact
    /// replay. `ordinal` is zero-based in the sector's original rule order.
    /// This observation does not imply that the rule will be admitted.
    CheckingRule {
        sector: [bool; N],
        ordinal: usize,
        total: usize,
        elapsed: Duration,
    },
    /// A diagnostic report, possibly incomplete. Receiving it does not mean
    /// that the sector passed the subsequent admission gate.
    CheckedSector {
        ordinal: usize,
        report: &'a SourcePortSectorAudit<N>,
        elapsed: Duration,
    },
    /// Rules keep their original precedence. Sectors are lowered in the
    /// existing canonical map order; `ordinal` indexes this sector's rules.
    LoweringRule {
        sector: [bool; N],
        ordinal: usize,
        total: usize,
        elapsed: Duration,
    },
    LoweredSector {
        sector: [bool; N],
        cells: usize,
        elapsed: Duration,
    },
    /// Start the existing final binding and exact whole-family cover checks.
    Installing {
        sectors: usize,
        rule_cells: usize,
        terminals: usize,
        elapsed: Duration,
    },
    /// The in-memory installer returned successfully. Durable encoding and
    /// independent cold loading are outside this observer's scope.
    Installed { elapsed: Duration },
}
