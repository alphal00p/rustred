//! Consuming seal for one exact, executable, geometrically closed owner cover.

use crate::foundry::completion::frame::admission::ExactOwnerCoverStatus;
use crate::foundry::completion::stratum::ImmutableOwnerSnapshot;

use super::{ExactExecutableOwnerCover, ExactExecutableOwnerError};

/// Strongly owned publication boundary for a closed executable owner cover.
///
/// The proof cover and every semantic/executable pairing are moved in place;
/// sealing neither recompiles nor clones them.  The exact predecessor snapshot
/// is cloned from the common epoch authority so later sector orchestration
/// never has to trust or rejoin a snapshot identity string.
#[derive(Debug)]
pub(crate) struct ClosedExactExecutableOwnerCover {
    cover: ExactExecutableOwnerCover,
    predecessor_snapshot: ImmutableOwnerSnapshot,
}

impl ClosedExactExecutableOwnerCover {
    /// Consume a cover only after rechecking closure and the complete common
    /// execution scope of every retained owner.
    pub(crate) fn try_seal(
        cover: ExactExecutableOwnerCover,
    ) -> Result<Self, ExactExecutableOwnerError> {
        match cover.proof_cover().status() {
            ExactOwnerCoverStatus::Closed => {}
            ExactOwnerCoverStatus::Incomplete(obstruction) => {
                return Err(ExactExecutableOwnerError::CoverNotClosed { obstruction });
            }
        }

        let first = cover
            .owners()
            .first()
            .ok_or(ExactExecutableOwnerError::EmptyOwners)?;
        let predecessor_snapshot = first.epoch().predecessor_snapshot().clone();
        Self::try_seal_against_predecessor(cover, predecessor_snapshot)
    }

    /// Seal a compiler-closed cover against the exact retained predecessor.
    /// Unlike [`Self::try_seal`], this authority-bearing boundary also accepts
    /// a zero-cell cover whose closure was proved directly by that predecessor.
    pub(crate) fn try_seal_against_predecessor(
        cover: ExactExecutableOwnerCover,
        predecessor_snapshot: ImmutableOwnerSnapshot,
    ) -> Result<Self, ExactExecutableOwnerError> {
        match cover.proof_cover().status() {
            ExactOwnerCoverStatus::Closed => {}
            ExactOwnerCoverStatus::Incomplete(obstruction) => {
                return Err(ExactExecutableOwnerError::CoverNotClosed { obstruction });
            }
        }
        let proof = cover.proof_cover();

        validate_predecessor_scope(proof, &predecessor_snapshot)?;
        for (owner, executable) in cover.owners().iter().enumerate() {
            let epoch = executable.epoch();
            let plan = epoch.plan();
            let detail = if plan.family_fingerprint() != proof.family_fingerprint() {
                Some("family fingerprint differs")
            } else if plan.context_fingerprint() != proof.context_fingerprint() {
                Some("coefficient-context fingerprint differs")
            } else if plan.sector() != proof.sector() {
                Some("sector differs")
            } else if epoch.fixed_ordering() != proof.ordering() {
                Some("ordering policy differs")
            } else if epoch.fixed_snapshot_id() != proof.owner_snapshot_id() {
                Some("predecessor snapshot identity differs")
            } else if !epoch
                .predecessor_snapshot()
                .same_authority_as(&predecessor_snapshot)
            {
                Some("exact predecessor snapshot authority differs")
            } else {
                None
            };
            if let Some(detail) = detail {
                return Err(ExactExecutableOwnerError::ClosedCoverScopeMismatch { owner, detail });
            }
        }

        Ok(Self {
            cover,
            predecessor_snapshot,
        })
    }

    /// The immutable executable cover, preserving every retained `Arc` and
    /// `RuleCell` address from before sealing.
    pub(crate) const fn executable_cover(&self) -> &ExactExecutableOwnerCover {
        &self.cover
    }

    /// Strong predecessor authority for all proper-subsector images used by
    /// the sealed cover.
    pub(crate) const fn predecessor_snapshot(&self) -> &ImmutableOwnerSnapshot {
        &self.predecessor_snapshot
    }
}

fn validate_predecessor_scope(
    proof: &crate::foundry::completion::frame::admission::ExactCircuitOwnerCover,
    predecessor: &ImmutableOwnerSnapshot,
) -> Result<(), ExactExecutableOwnerError> {
    let detail = if predecessor.family_fingerprint() != proof.family_fingerprint() {
        Some("predecessor family fingerprint differs")
    } else if predecessor.context_fingerprint() != proof.context_fingerprint() {
        Some("predecessor coefficient-context fingerprint differs")
    } else if predecessor.arity() != proof.sector().arity() {
        Some("predecessor arity differs")
    } else if predecessor.id() != proof.owner_snapshot_id() {
        Some("predecessor snapshot identity differs")
    } else {
        None
    };
    if let Some(detail) = detail {
        Err(ExactExecutableOwnerError::ClosedCoverScopeMismatch { owner: 0, detail })
    } else {
        Ok(())
    }
}
