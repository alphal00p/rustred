use std::fmt;
use std::sync::Arc;

/// Monotonic mutation revision of one canonical exact-owner ledger.
///
/// Revision zero is the owner-free state. A revision advances exactly once
/// after a proposal has transactionally changed the retained canonical owner
/// set, regardless of whether that change also shrank the exact cover.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ExactOwnerLedgerRevision(u64);

impl ExactOwnerLedgerRevision {
    pub(crate) const ZERO: Self = Self(0);

    pub(crate) const fn get(self) -> u64 {
        self.0
    }

    pub(super) const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(next) => Some(Self(next)),
            None => None,
        }
    }

    #[cfg(test)]
    pub(super) const fn overflow_boundary_for_test() -> Self {
        Self(u64::MAX)
    }
}

/// Process-local authority for one concrete ledger instance.
///
/// The private zero-sized payload is deliberately opaque. Pointer identity,
/// not value equality or a structural predecessor digest, prevents snapshots
/// from independently installed authorities (or independently constructed
/// ledgers over one authority) from aliasing.
#[derive(Debug)]
struct ExactOwnerLedgerNonce;

/// Immutable planner binding for one exact ledger revision.
///
/// Workers may retain this value beside tasks and ask the owning ledger to
/// revalidate it before applying a delayed result. Cloning preserves the
/// ledger nonce through `Arc`; callers cannot mint or edit either component.
#[derive(Clone)]
pub(crate) struct ExactOwnerLedgerSnapshotIdentity {
    ledger_nonce: Arc<ExactOwnerLedgerNonce>,
    revision: ExactOwnerLedgerRevision,
}

impl ExactOwnerLedgerSnapshotIdentity {
    pub(crate) const fn revision(&self) -> ExactOwnerLedgerRevision {
        self.revision
    }

    pub(crate) fn same_ledger_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.ledger_nonce, &other.ledger_nonce)
    }

    pub(crate) fn same_snapshot_as(&self, other: &Self) -> bool {
        self.revision == other.revision && self.same_ledger_as(other)
    }

    pub(super) fn fresh(revision: ExactOwnerLedgerRevision) -> Self {
        Self {
            ledger_nonce: Arc::new(ExactOwnerLedgerNonce),
            revision,
        }
    }

    pub(super) fn at_revision(&self, revision: ExactOwnerLedgerRevision) -> Self {
        Self {
            ledger_nonce: Arc::clone(&self.ledger_nonce),
            revision,
        }
    }
}

impl fmt::Debug for ExactOwnerLedgerSnapshotIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactOwnerLedgerSnapshotIdentity")
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}

impl PartialEq for ExactOwnerLedgerSnapshotIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.same_snapshot_as(other)
    }
}

impl Eq for ExactOwnerLedgerSnapshotIdentity {}
