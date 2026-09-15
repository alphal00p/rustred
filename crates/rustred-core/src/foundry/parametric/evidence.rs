//! The actual proof used by a parametric rule, without invented search history.
//!
//! Existing elimination-produced rules retain their indexed replay and one
//! concrete specialization. A source-directed coordinate program instead owns
//! a full original-source identity proved on an exact domain. Neither variant
//! by itself claims that the surrounding family has a complete rule cover.

use std::sync::Arc;

use crate::foundry::cell::FixedIndexRestriction;
use crate::foundry::completion::LatticeBox;
use crate::sector::Mask;

use super::affine::AffineApplicationDomain;

use super::model::{ConcreteSpecializationReplayWitness, ParametricExactReplayWitness};

/// The proof-bearing replay convention of one exact parametric identity.
///
/// There is deliberately no universal concrete anchor: a combined identity
/// proved on a coordinate domain does not acquire an elimination transcript
/// or a specially sampled point merely to fit the other producer's metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricReplayEvidence {
    Anchored {
        indexed: ParametricExactReplayWitness,
        concrete: ConcreteSpecializationReplayWitness,
    },
    CombinedOriginalDomain(Arc<CombinedOriginalDomainEvidence>),
}

impl ParametricReplayEvidence {
    pub fn indexed(&self) -> Option<ParametricExactReplayWitness> {
        match self {
            Self::Anchored { indexed, .. } => Some(*indexed),
            Self::CombinedOriginalDomain(_) => None,
        }
    }

    pub fn concrete(&self) -> Option<&ConcreteSpecializationReplayWitness> {
        match self {
            Self::Anchored { concrete, .. } => Some(concrete),
            Self::CombinedOriginalDomain(_) => None,
        }
    }

    pub fn combined_original_domain(&self) -> Option<&CombinedOriginalDomainEvidence> {
        match self {
            Self::Anchored { .. } => None,
            Self::CombinedOriginalDomain(evidence) => Some(evidence),
        }
    }
}

/// Exact coordinate quotient used for full weighted original-source replay.
///
/// The owning rule supplies the unchanged source combination, original source
/// views, normalized target and RHS, and complete nonzero conditions. Its cold
/// verifier multiplies the full translated original rows before testing the
/// combined remainder on this domain. Individual source-term deletions are
/// not asserted here: cancellations may occur only after rows are combined.
///
/// Coordinates are x=n-1 for positive powers and x=-n otherwise. Unbounded
/// endpoints remain `None`; they are never replaced by an i64 endpoint in the
/// proof. Runtime cell representability is a separate restriction.
///
/// Fields and construction remain private to the exact producer boundary.
/// A domain description supplied by a caller is not replay authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CombinedOriginalDomainEvidence {
    pub(super) sector: Mask,
    pub(super) fixed: Box<[FixedIndexRestriction]>,
    pub(super) application: Arc<[LatticeBox]>,
    pub(super) source_rows_used: usize,
    pub(super) shift_columns_checked: usize,
    /// Exact coupled-domain membership, when the producer has retained an
    /// affine case.  This is deliberately an in-memory carrier only: the
    /// current artifact codecs and coverage compiler remain box-only and must
    /// reject a populated value until their authenticated schema is extended.
    pub(super) affine: Option<Arc<AffineApplicationDomain>>,
    /// Exact affine exceptional branches removed from the rectangular
    /// prefilter.  These are retained as predicates, never approximated by
    /// boxes.  Publication remains fail-closed until the durable predicate
    /// partition codec/coverage compiler is enabled.
    pub(super) affine_exclusions: Arc<[Arc<AffineApplicationDomain>]>,
}

impl CombinedOriginalDomainEvidence {
    pub fn sector(&self) -> &Mask {
        &self.sector
    }

    pub fn fixed_restrictions(&self) -> &[FixedIndexRestriction] {
        &self.fixed
    }

    pub(crate) fn application_boxes(&self) -> &[LatticeBox] {
        &self.application
    }
    pub(crate) fn source_rows_used(&self) -> usize {
        self.source_rows_used
    }
    pub(crate) fn shift_columns_checked(&self) -> usize {
        self.shift_columns_checked
    }

    /// Exact original-power predicate for a coupled replay domain.  A
    /// rectangular hull is never substituted for the affine equations.
    pub(crate) fn affine_application_domain(&self) -> Option<&AffineApplicationDomain> {
        self.affine.as_deref()
    }

    /// Retain the immutable affine carrier without cloning its Symbolica
    /// equations.  Owner compilation uses this to keep candidate metadata
    /// pointer-shared with the replay evidence.
    pub(crate) fn affine_application_domain_owner(
        &self,
    ) -> Option<Arc<AffineApplicationDomain>> {
        self.affine.clone()
    }

    /// Affine exceptional predicates which must be excluded after the
    /// rectangular prefilter.  The slice is immutable and pointer-shared
    /// across cells; callers must test original integral powers.
    pub(crate) fn affine_exclusions(&self) -> &[Arc<AffineApplicationDomain>] {
        &self.affine_exclusions
    }
}

#[cfg(test)]
#[path = "evidence/tests.rs"]
mod tests;
