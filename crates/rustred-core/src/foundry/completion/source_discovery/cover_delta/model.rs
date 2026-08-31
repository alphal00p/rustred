use crate::foundry::completion::frame::admission::ExactOwnerCoverStatus;

use super::ExactOwnerLedgerRevision;

/// Whether a ledger has no compiled owner yet or retains an exact compiler
/// verdict. Only `Compiled(Closed)` carries closure evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExactOwnerLedgerCoverStatus {
    OwnerFree,
    Compiled(ExactOwnerCoverStatus),
}

impl ExactOwnerLedgerCoverStatus {
    pub(crate) const fn is_compiler_closed(self) -> bool {
        matches!(self, Self::Compiled(ExactOwnerCoverStatus::Closed))
    }
}

/// Allocation-free structural telemetry for one exact ledger state.
///
/// The revision makes before/after observations auditable, but this copyable
/// value deliberately omits the opaque ledger nonce. It is not delayed-task
/// authority; only `ExactOwnerLedgerSnapshotIdentity` can be rejoined to one
/// concrete live ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactOwnerCoverSnapshot {
    revision: ExactOwnerLedgerRevision,
    status: ExactOwnerLedgerCoverStatus,
    owner_count: usize,
    terminal_count: usize,
    uncovered_box_count: usize,
    uncovered_is_finite: bool,
    missing_terminal_count: usize,
    guard_incomplete_owner_count: usize,
}

impl ExactOwnerCoverSnapshot {
    pub(crate) const fn revision(self) -> ExactOwnerLedgerRevision {
        self.revision
    }

    pub(crate) const fn status(self) -> ExactOwnerLedgerCoverStatus {
        self.status
    }

    pub(crate) const fn owner_count(self) -> usize {
        self.owner_count
    }

    pub(crate) const fn terminal_count(self) -> usize {
        self.terminal_count
    }

    pub(crate) const fn uncovered_box_count(self) -> usize {
        self.uncovered_box_count
    }

    pub(crate) const fn uncovered_is_finite(self) -> bool {
        self.uncovered_is_finite
    }

    pub(crate) const fn missing_terminal_count(self) -> usize {
        self.missing_terminal_count
    }

    pub(crate) const fn guard_incomplete_owner_count(self) -> usize {
        self.guard_incomplete_owner_count
    }

    pub(super) const fn new(
        revision: ExactOwnerLedgerRevision,
        status: ExactOwnerLedgerCoverStatus,
        owner_count: usize,
        terminal_count: usize,
        uncovered_box_count: usize,
        uncovered_is_finite: bool,
        missing_terminal_count: usize,
        guard_incomplete_owner_count: usize,
    ) -> Self {
        Self {
            revision,
            status,
            owner_count,
            terminal_count,
            uncovered_box_count,
            uncovered_is_finite,
            missing_terminal_count,
            guard_incomplete_owner_count,
        }
    }
}

/// Exact geometric effect of one canonical owner proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExactOwnerCoverDeltaKind {
    /// The coordinator retained its existing canonical representative.
    Duplicate,
    /// The canonical owner ledger changed, but its uncovered box union did not.
    ChangedWithoutGeometricShrink,
    /// The exact uncovered box union became a proper subset of its baseline.
    StrictGeometricShrink,
}

/// Structural scalar before/after telemetry. Closure is exposed only through
/// the exact compiler status in `updated`. A delta contains no ledger nonce
/// and cannot authorize a delayed task or owner application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactOwnerCoverDelta {
    kind: ExactOwnerCoverDeltaKind,
    baseline: ExactOwnerCoverSnapshot,
    updated: ExactOwnerCoverSnapshot,
}

impl ExactOwnerCoverDelta {
    pub(crate) const fn kind(self) -> ExactOwnerCoverDeltaKind {
        self.kind
    }

    pub(crate) const fn baseline(self) -> ExactOwnerCoverSnapshot {
        self.baseline
    }

    pub(crate) const fn updated(self) -> ExactOwnerCoverSnapshot {
        self.updated
    }

    pub(crate) const fn strictly_shrank(self) -> bool {
        matches!(self.kind, ExactOwnerCoverDeltaKind::StrictGeometricShrink)
    }

    pub(super) const fn new(
        kind: ExactOwnerCoverDeltaKind,
        baseline: ExactOwnerCoverSnapshot,
        updated: ExactOwnerCoverSnapshot,
    ) -> Self {
        Self {
            kind,
            baseline,
            updated,
        }
    }
}
