use crate::foundry::completion::stratum::{
    DecoratedStratum, DecoratedStratumId, GuardBranchIdentity,
};

/// One unique primitive guard predicate and every exact-circuit guard ordinal
/// which reduced to that associate. The first ordinal fixes chronology; no
/// origin is discarded from the circuit itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RequiredGuardPredicate {
    nonzero: GuardBranchIdentity,
    circuit_guard_ordinals: Box<[usize]>,
}

impl RequiredGuardPredicate {
    pub(crate) const fn nonzero_branch(&self) -> &GuardBranchIdentity {
        &self.nonzero
    }

    pub(crate) fn circuit_guard_ordinals(&self) -> &[usize] {
        &self.circuit_guard_ordinals
    }

    pub(super) fn new(nonzero: GuardBranchIdentity, circuit_guard_ordinals: Vec<usize>) -> Self {
        Self {
            nonzero,
            circuit_guard_ordinals: circuit_guard_ordinals.into_boxed_slice(),
        }
    }
}

/// One disjoint first-zero child. It deliberately retains no target partition
/// or circuit owner; discovery must restart on this exact lower-dimensional
/// branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExceptionalGuardStratum {
    required_predicate_ordinal: usize,
    stratum: DecoratedStratum,
}

impl ExceptionalGuardStratum {
    pub(crate) const fn required_predicate_ordinal(&self) -> usize {
        self.required_predicate_ordinal
    }

    pub(crate) const fn stratum(&self) -> &DecoratedStratum {
        &self.stratum
    }

    pub(super) const fn new(required_predicate_ordinal: usize, stratum: DecoratedStratum) -> Self {
        Self {
            required_predicate_ordinal,
            stratum,
        }
    }
}

/// Exhaustive first-zero refinement of one exact circuit's parent stratum.
///
/// `admitted` is the sole all-required-nonzero child. `exceptional` is ordered
/// by first newly split predicate and is disjoint by construction. This value
/// is one-step evidence only and never an owner-cover certificate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactGuardRefinement {
    parent_stratum_id: DecoratedStratumId,
    required: Box<[RequiredGuardPredicate]>,
    newly_split_predicate_ordinals: Box<[usize]>,
    admitted: DecoratedStratum,
    exceptional: Box<[ExceptionalGuardStratum]>,
}

impl ExactGuardRefinement {
    pub(crate) const fn parent_stratum_id(&self) -> &DecoratedStratumId {
        &self.parent_stratum_id
    }

    pub(crate) fn required_predicates(&self) -> &[RequiredGuardPredicate] {
        &self.required
    }

    pub(crate) fn newly_split_predicate_ordinals(&self) -> &[usize] {
        &self.newly_split_predicate_ordinals
    }

    pub(crate) const fn admitted_stratum(&self) -> &DecoratedStratum {
        &self.admitted
    }

    pub(crate) fn exceptional_strata(&self) -> &[ExceptionalGuardStratum] {
        &self.exceptional
    }

    pub(super) fn from_parts(
        parent_stratum_id: DecoratedStratumId,
        required: Vec<RequiredGuardPredicate>,
        newly_split_predicate_ordinals: Vec<usize>,
        admitted: DecoratedStratum,
        exceptional: Vec<ExceptionalGuardStratum>,
    ) -> Self {
        Self {
            parent_stratum_id,
            required: required.into_boxed_slice(),
            newly_split_predicate_ordinals: newly_split_predicate_ordinals.into_boxed_slice(),
            admitted,
            exceptional: exceptional.into_boxed_slice(),
        }
    }
}

/// A known-zero exact predicate makes this circuit inapplicable on the parent;
/// this is a normal discovery outcome, not evidence that the branch closes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExactGuardRefinementOutcome {
    Admitted(ExactGuardRefinement),
    BlockedByKnownZero {
        required_predicate_ordinal: usize,
        first_circuit_guard_ordinal: usize,
        zero_branch: GuardBranchIdentity,
    },
}
