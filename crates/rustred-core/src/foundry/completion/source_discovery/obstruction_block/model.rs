use std::sync::Arc;

use symbolica::domains::finite_field::FiniteFieldElement;

use crate::foundry::completion::frame::modular::ModularRightObstructionIdentity;
use crate::identity::{IntegralShift, TranslatedSourceRequest};

use super::super::IncidentTranslationNominations;

/// Conservative, checked work envelope for one proposal-only union
/// nomination. It is scheduling telemetry only and carries no residual seal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ObstructionBlockNominationUpperBound {
    raw_block_entries: usize,
    raw_request_visits: usize,
    coordinate_cells: usize,
    dense_coefficient_cells: usize,
    canonicalization_logical_work_reservation: usize,
    subset_comparisons: usize,
}

impl ObstructionBlockNominationUpperBound {
    pub(crate) const fn raw_block_entries(self) -> usize {
        self.raw_block_entries
    }

    pub(crate) const fn raw_request_visits(self) -> usize {
        self.raw_request_visits
    }

    pub(crate) const fn coordinate_cells(self) -> usize {
        self.coordinate_cells
    }

    pub(crate) const fn dense_coefficient_cells(self) -> usize {
        self.dense_coefficient_cells
    }

    pub(crate) const fn canonicalization_logical_work_reservation(self) -> usize {
        self.canonicalization_logical_work_reservation
    }

    pub(crate) const fn subset_comparisons(self) -> usize {
        self.subset_comparisons
    }

    pub(super) const fn from_parts(
        raw_block_entries: usize,
        raw_request_visits: usize,
        coordinate_cells: usize,
        dense_coefficient_cells: usize,
        canonicalization_logical_work_reservation: usize,
        subset_comparisons: usize,
    ) -> Self {
        Self {
            raw_block_entries,
            raw_request_visits,
            coordinate_cells,
            dense_coefficient_cells,
            canonicalization_logical_work_reservation,
            subset_comparisons,
        }
    }
}

/// Scope-bound two-phase plan admitted by the outer scheduler before union
/// support/request allocation or sorting starts.
#[derive(Clone, Debug)]
pub(crate) struct ObstructionBlockNominationPlan {
    incidence_identity: Arc<()>,
    obstruction_identity: ModularRightObstructionIdentity,
    primary_identity: Arc<()>,
    upper_bound: ObstructionBlockNominationUpperBound,
}

impl ObstructionBlockNominationPlan {
    pub(crate) const fn upper_bound(&self) -> ObstructionBlockNominationUpperBound {
        self.upper_bound
    }

    pub(super) fn belongs_to_incidence(&self, identity: &Arc<()>) -> bool {
        Arc::ptr_eq(&self.incidence_identity, identity)
    }

    pub(super) const fn obstruction_identity(&self) -> &ModularRightObstructionIdentity {
        &self.obstruction_identity
    }

    pub(super) const fn primary_identity(&self) -> &Arc<()> {
        &self.primary_identity
    }

    pub(super) fn from_parts(
        incidence_identity: Arc<()>,
        obstruction_identity: ModularRightObstructionIdentity,
        primary_identity: Arc<()>,
        upper_bound: ObstructionBlockNominationUpperBound,
    ) -> Self {
        Self {
            incidence_identity,
            obstruction_identity,
            primary_identity,
            upper_bound,
        }
    }
}

/// One exact raw shift in the union support of a checked obstruction block.
/// Coefficients are dense in stable block-direction order, including zeros.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnionObstructionSupportEntry {
    shift: IntegralShift,
    coefficients: Box<[FiniteFieldElement<u64>]>,
}

impl UnionObstructionSupportEntry {
    pub(crate) const fn shift(&self) -> &IntegralShift {
        &self.shift
    }

    pub(crate) fn coefficients(&self) -> &[FiniteFieldElement<u64>] {
        &self.coefficients
    }

    pub(super) fn from_parts(
        shift: IntegralShift,
        coefficients: Vec<FiniteFieldElement<u64>>,
    ) -> Self {
        Self {
            shift,
            coefficients: coefficients.into_boxed_slice(),
        }
    }
}

/// Canonical requests incident to the union support of one checked block.
///
/// This value carries no incidence identity, obstruction identity, sample,
/// plan identity, or residual-construction capability. Its retained primary
/// request slice is only an exact-subset audit witness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnionSupportNominations {
    requests: Box<[TranslatedSourceRequest]>,
    primary_requests: Box<[TranslatedSourceRequest]>,
    support: Box<[UnionObstructionSupportEntry]>,
    raw_incidence_visits: usize,
    unique_before_existing_exclusion: usize,
    excluded_existing_requests: usize,
    nomination_upper_bound: ObstructionBlockNominationUpperBound,
}

impl UnionSupportNominations {
    pub(crate) fn requests(&self) -> &[TranslatedSourceRequest] {
        &self.requests
    }

    pub(crate) fn primary_requests(&self) -> &[TranslatedSourceRequest] {
        &self.primary_requests
    }

    pub(crate) fn support(&self) -> &[UnionObstructionSupportEntry] {
        &self.support
    }

    pub(crate) fn direction_count(&self) -> usize {
        self.support
            .first()
            .map_or(0, |entry| entry.coefficients.len())
    }

    pub(crate) const fn raw_incidence_visits(&self) -> usize {
        self.raw_incidence_visits
    }

    pub(crate) const fn unique_before_existing_exclusion(&self) -> usize {
        self.unique_before_existing_exclusion
    }

    pub(crate) const fn excluded_existing_requests(&self) -> usize {
        self.excluded_existing_requests
    }

    pub(crate) const fn nomination_upper_bound(&self) -> ObstructionBlockNominationUpperBound {
        self.nomination_upper_bound
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_parts(
        requests: Vec<TranslatedSourceRequest>,
        primary_requests: Vec<TranslatedSourceRequest>,
        support: Vec<UnionObstructionSupportEntry>,
        raw_incidence_visits: usize,
        unique_before_existing_exclusion: usize,
        excluded_existing_requests: usize,
        nomination_upper_bound: ObstructionBlockNominationUpperBound,
    ) -> Self {
        Self {
            requests: requests.into_boxed_slice(),
            primary_requests: primary_requests.into_boxed_slice(),
            support: support.into_boxed_slice(),
            raw_incidence_visits,
            unique_before_existing_exclusion,
            excluded_existing_requests,
            nomination_upper_bound,
        }
    }
}

/// Joined result of primary-authoritative and block-proposal nominations.
/// Only `primary` can enter the existing residual census and sampled dual.
#[derive(Clone, Debug)]
pub(crate) struct ObstructionBlockNominations<'primary> {
    primary: &'primary IncidentTranslationNominations,
    union: UnionSupportNominations,
}

impl<'primary> ObstructionBlockNominations<'primary> {
    pub(crate) const fn primary(&self) -> &IncidentTranslationNominations {
        &self.primary
    }

    pub(crate) const fn union(&self) -> &UnionSupportNominations {
        &self.union
    }

    pub(super) const fn from_parts(
        primary: &'primary IncidentTranslationNominations,
        union: UnionSupportNominations,
    ) -> Self {
        Self { primary, union }
    }
}

impl PartialEq for ObstructionBlockNominations<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.primary == other.primary && self.union == other.union
    }
}

impl Eq for ObstructionBlockNominations<'_> {}
