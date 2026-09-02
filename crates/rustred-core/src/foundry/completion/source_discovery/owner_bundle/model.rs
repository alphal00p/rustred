use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::cell::{RuleCell, RuleCellGuardDomainSplit};
use crate::foundry::completion::frame::admission::{
    ExactCircuitOwnerCover, ExactCircuitSemanticDag, ExactGuardRefinement,
};
use crate::foundry::completion::frame::exact::{ClearedExactCircuit, ExactTargetCircuit};
use crate::foundry::completion::stratum::GuardBranchIdentity;

use super::super::{
    AdmittedExactRuleCandidate, CanonicalReplayBatch, ExactRuleCellGuardObstruction, FreshTaskEpoch,
};

/// A normal reason why one canonical replay batch cannot yet become an
/// ordinary globally executable owner.
#[derive(Debug)]
pub(crate) enum ExactExecutableOwnerObstruction {
    BlockedByKnownZero {
        required_predicate_ordinal: usize,
        first_circuit_guard_ordinal: usize,
        zero_branch: GuardBranchIdentity,
    },
    NeedsGuardedStratum {
        refinement: ExactGuardRefinement,
        obstruction: ExactRuleCellGuardObstruction,
    },
    AnchorOnGuardWall {
        refinement: ExactGuardRefinement,
        guard_ordinal: usize,
    },
    /// The replay owns one guard-free rectangular component. The exact root
    /// singleton (and an optional second guard-free component) remain
    /// explicit alternate-support work.
    ExceptionalGuardDomain {
        refinement: ExactGuardRefinement,
        split: RuleCellGuardDomainSplit,
    },
}

/// One exact candidate retained with the authority needed to revisit its
/// normal promotion obstruction.
#[derive(Debug)]
pub(crate) struct ExactExecutableCandidateObstruction {
    candidate_ordinal: usize,
    epoch: Arc<FreshTaskEpoch>,
    circuit: Arc<ExactTargetCircuit>,
    cleared: Arc<ClearedExactCircuit>,
    obstruction: ExactExecutableOwnerObstruction,
}

impl ExactExecutableCandidateObstruction {
    pub(crate) const fn candidate_ordinal(&self) -> usize {
        self.candidate_ordinal
    }

    pub(crate) const fn epoch(&self) -> &Arc<FreshTaskEpoch> {
        &self.epoch
    }

    pub(crate) const fn circuit(&self) -> &Arc<ExactTargetCircuit> {
        &self.circuit
    }

    pub(crate) const fn cleared(&self) -> &Arc<ClearedExactCircuit> {
        &self.cleared
    }

    pub(crate) const fn obstruction(&self) -> &ExactExecutableOwnerObstruction {
        &self.obstruction
    }

    pub(super) const fn new(
        candidate_ordinal: usize,
        epoch: Arc<FreshTaskEpoch>,
        circuit: Arc<ExactTargetCircuit>,
        cleared: Arc<ClearedExactCircuit>,
        obstruction: ExactExecutableOwnerObstruction,
    ) -> Self {
        Self {
            candidate_ordinal,
            epoch,
            circuit,
            cleared,
            obstruction,
        }
    }
}

/// The original non-authoritative batch retained beside its first ordinary
/// owner obstruction so a later stratum/anchor policy can retry it exactly.
#[derive(Debug)]
pub(crate) struct UnpublishedCanonicalOwnerProposal {
    batch: CanonicalReplayBatch,
    obstructions: Box<[ExactExecutableCandidateObstruction]>,
}

impl UnpublishedCanonicalOwnerProposal {
    pub(crate) const fn batch(&self) -> &CanonicalReplayBatch {
        &self.batch
    }

    pub(crate) fn obstructions(&self) -> &[ExactExecutableCandidateObstruction] {
        &self.obstructions
    }

    pub(super) const fn new(
        batch: CanonicalReplayBatch,
        obstructions: Box<[ExactExecutableCandidateObstruction]>,
    ) -> Self {
        Self {
            batch,
            obstructions,
        }
    }
}

/// Transactional result of compiling one canonical replay batch.
#[derive(Debug)]
pub(crate) enum ExactExecutableOwnerProposal {
    Compiled {
        owner: Arc<ExactSemanticExecutableOwner>,
        obstructions: Box<[ExactExecutableCandidateObstruction]>,
    },
    Incomplete(UnpublishedCanonicalOwnerProposal),
}

/// Compact resident order key for one complete canonical owner encoding.
///
/// The digest supplies a deterministic cold-path order and the encoded length
/// bounds an exact replay when both compact fields coincide. A matching key is
/// never itself treated as proof of equal owner content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ExactOwnerContentOrderKey {
    digest: [u8; blake3::OUT_LEN],
    encoded_len: usize,
}

impl ExactOwnerContentOrderKey {
    pub(super) const fn from_digest_and_len(
        digest: [u8; blake3::OUT_LEN],
        encoded_len: usize,
    ) -> Self {
        Self {
            digest,
            encoded_len,
        }
    }

    /// Exact canonical byte length committed by the digest.
    pub(crate) const fn encoded_len(self) -> usize {
        self.encoded_len
    }

    /// Actual fixed-size owner payload retained in memory for ordering.
    pub(crate) const fn retained_bytes(self) -> usize {
        std::mem::size_of::<Self>()
    }

    #[cfg(test)]
    pub(crate) const fn with_encoded_len_for_test(self, encoded_len: usize) -> Self {
        Self {
            digest: self.digest,
            encoded_len,
        }
    }
}

/// One target owner whose exact semantic candidates remain positionally
/// paired with their executable cells after canonical exact-content sorting.
#[derive(Debug)]
pub(crate) struct ExactSemanticExecutableOwner {
    pub(super) epoch: Arc<FreshTaskEpoch>,
    pub(super) semantic: Arc<ExactCircuitSemanticDag>,
    pub(super) executable: Box<[AdmittedExactRuleCandidate]>,
    pub(super) content_order_key: ExactOwnerContentOrderKey,
}

impl ExactSemanticExecutableOwner {
    pub(crate) const fn epoch(&self) -> &Arc<FreshTaskEpoch> {
        &self.epoch
    }

    pub(crate) const fn semantic(&self) -> &Arc<ExactCircuitSemanticDag> {
        &self.semantic
    }

    pub(crate) fn executable_candidates(&self) -> &[AdmittedExactRuleCandidate] {
        &self.executable
    }

    /// Compact deterministic order key for the complete canonical owner
    /// encoding. Equal digest/length pairs are never assumed equal: the
    /// comparator re-encodes both owners and compares their exact bytes.
    pub(crate) const fn content_order_key(&self) -> ExactOwnerContentOrderKey {
        self.content_order_key
    }

    #[cfg(test)]
    pub(crate) fn replace_content_order_key_for_test(
        &mut self,
        content_order_key: ExactOwnerContentOrderKey,
    ) {
        self.content_order_key = content_order_key;
    }
}

/// Exact selection with the semantic proof circuit and executable cell joined
/// by the compiler rather than rediscovered by the caller.
#[derive(Debug)]
pub(crate) enum ExactExecutableOwnerSelection<'a> {
    Descending {
        owner_ordinal: usize,
        candidate_ordinal: usize,
        circuit: &'a Arc<ExactTargetCircuit>,
        cell: &'a RuleCell,
    },
    Terminal(&'a IntegralKey),
    Incomplete,
}

/// Immutable logical owner set plus its most recently successful whole-cover
/// compilation. `try_insert` replaces neither field unless every cold join,
/// exact cover check, and semantic/executable pairing succeeds.
#[derive(Debug)]
pub(crate) struct ExactExecutableOwnerCover {
    pub(super) owners: Box<[Arc<ExactSemanticExecutableOwner>]>,
    pub(super) terminals: Box<[IntegralKey]>,
    pub(super) cover: ExactCircuitOwnerCover,
}

impl ExactExecutableOwnerCover {
    pub(crate) fn owners(&self) -> &[Arc<ExactSemanticExecutableOwner>] {
        &self.owners
    }

    pub(crate) fn terminals(&self) -> &[IntegralKey] {
        &self.terminals
    }

    pub(crate) const fn proof_cover(&self) -> &ExactCircuitOwnerCover {
        &self.cover
    }
}
