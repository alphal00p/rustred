//! Borrowed, non-authoritative observations of the existing installation path.

use std::time::Duration;

use super::SourcePortSectorAudit;

/// Progress from `SourcePortAudit::install_complete_with_observer`.
///
/// Like the solver's `SectorEvent`, these events borrow existing results and
/// introduce no worker pool, synchronization, or coefficient copies. The
/// callback runs serially on the calling thread. It cannot change rules or
/// authorize publication; a callback panic aborts the invocation normally.
/// `elapsed` is wall time since installation entry, including observer work,
/// and is never persisted in an artifact or used for a mathematical choice.
#[derive(Debug)]
pub enum SourcePortInstallEvent<'a, const N: usize> {
    /// `ordinal` follows the caller's input iterator, not the sector bitmask.
    CheckingSector {
        ordinal: usize,
        sector: [bool; N],
        rules: usize,
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
