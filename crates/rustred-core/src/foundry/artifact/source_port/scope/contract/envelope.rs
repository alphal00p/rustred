//! One caller-proposed domain, with entry containment but no rule authority.

use crate::family::IntegralFamily;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::completion::{CompletionGeometryLimits, LatticeBox};
use crate::sector::Mask;

use super::super::successor::{DestinationScope, PreparedSuccessorScope};
use super::{EntryScope, invalid};

/// Immutable box union and one cumulative geometry allowance. Entry inclusion
/// is checked at construction; every selected executable rule must separately
/// establish exact replay, predicates/guards, descent, coverage and RHS closure.
/// This owner cannot publish an artifact or certify itself.
pub(in crate::foundry::artifact) struct ProposedProofEnvelope {
    entry: EntryScope,
    prepared: PreparedSuccessorScope,
}

impl ProposedProofEnvelope {
    pub(in crate::foundry::artifact) fn try_new(
        entry: &EntryScope,
        destinations: &[DestinationScope<'_>],
        limits: CompletionGeometryLimits,
    ) -> Result<Self, ArtifactError> {
        for destination in destinations {
            entry.validate_sector(destination.sector)?;
        }
        let mut prepared =
            PreparedSuccessorScope::try_new(entry.root().arity(), destinations, limits)?;
        if !prepared.entry_is_contained(entry)? {
            return Err(invalid(
                "entry domain is not contained in the proposed proof envelope",
            ));
        }
        Ok(Self {
            entry: entry.clone(),
            prepared,
        })
    }

    pub(in crate::foundry::artifact) fn entry_scope(&self) -> &EntryScope {
        &self.entry
    }

    pub(in crate::foundry::artifact) fn validate_binding(
        &self,
        family: &IntegralFamily,
        root: &Mask,
    ) -> Result<(), ArtifactError> {
        self.entry.validate_binding(family, root)
    }

    /// Borrow admitted, canonically ordered domains for predicate-aware rule
    /// clipping. Intersecting a rectangular prefilter must retain all affine
    /// equations and exceptional conjunctions in the caller's rule domain.
    pub(in crate::foundry::artifact) fn destinations(
        &self,
    ) -> impl ExactSizeIterator<Item = DestinationScope<'_>> {
        self.prepared.destinations()
    }

    pub(in crate::foundry::artifact) fn sector_boxes(
        &self,
        sector: &[bool],
    ) -> Result<&[LatticeBox], ArtifactError> {
        self.entry.validate_sector(sector)?;
        Ok(self.prepared.sector_boxes(sector).unwrap_or(&[]))
    }

    /// Conservative whole-rectangle check after exact RHS pruning and clipping.
    /// A failure can be inconclusive for a tighter affine/guarded cell; it is
    /// never permission to omit the escaped image or silently widen the scope.
    pub(in crate::foundry::artifact) fn check_rule_images(
        &mut self,
        source: &LatticeBox,
        sector: &[bool],
        rhs_shifts: &[&[i64]],
    ) -> Result<(), ArtifactError> {
        self.entry.validate_sector(sector)?;
        if !self.prepared.contains_box(source, sector)? {
            return Err(invalid(
                "rule source is outside the proposed proof envelope",
            ));
        }
        for shift in rhs_shifts {
            if !self.prepared.contains(source, sector, shift)? {
                return Err(invalid("RHS image escapes the proposed proof envelope"));
            }
        }
        Ok(())
    }
}
