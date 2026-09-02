//! Authority-minimal ingress for parent-lattice support proposals.
//!
//! A coordinate chart or another cold search heuristic may nominate raw
//! support points in the authoritative parent index lattice.  This module
//! expands that support through the sealed parent ordinary-source incidence
//! index and retains only canonical [`TranslatedSourceRequest`] identities.
//! It deliberately cannot carry translated rows, coefficients, guards,
//! circuits, owners, terminals, or artifacts.

use std::sync::Arc;

use crate::identity::{CompletedIbpSourceRows, IntegralShift, RowId, TranslatedSourceRequest};

use super::incidence::{DISTINCT_SHIFTS, SOURCE_ROWS, SOURCE_TERMS};
use super::nominate::{
    CANDIDATE_COORDINATES, INCIDENCE_VISITS, RAW_REQUESTS, SUPPORT_ENTRIES, UNIQUE_REQUESTS,
    check_limit, checked_mul, nominate_requests, try_vec,
};
use super::{OrdinarySourceIncidenceIndex, SourceDiscoveryError, SourceDiscoveryLimits};

const FAMILY_SCOPE: &str = "initial-parent proposal family fingerprint";
const CONTEXT_SCOPE: &str = "initial-parent proposal context fingerprint";
const SOURCE_CHRONOLOGY: &str = "initial-parent proposal ordinary-source chronology";

/// Deterministic scalar census retained with one initial parent proposal.
///
/// These values describe bounded inverse-incidence work only.  They are not
/// evidence that any translated row is useful, nonzero, or part of a closing
/// relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InitialParentSourceProposalTelemetry {
    arity: usize,
    ordinary_source_rows: usize,
    source_term_occurrences: usize,
    distinct_source_shifts: usize,
    parent_support_entries: usize,
    raw_incidence_visits: usize,
    unique_before_existing_exclusion: usize,
    request_count: usize,
    request_coordinate_cells: usize,
}

impl InitialParentSourceProposalTelemetry {
    pub(crate) const fn arity(self) -> usize {
        self.arity
    }

    pub(crate) const fn ordinary_source_rows(self) -> usize {
        self.ordinary_source_rows
    }

    pub(crate) const fn source_term_occurrences(self) -> usize {
        self.source_term_occurrences
    }

    pub(crate) const fn distinct_source_shifts(self) -> usize {
        self.distinct_source_shifts
    }

    pub(crate) const fn parent_support_entries(self) -> usize {
        self.parent_support_entries
    }

    pub(crate) const fn raw_incidence_visits(self) -> usize {
        self.raw_incidence_visits
    }

    pub(crate) const fn unique_before_existing_exclusion(self) -> usize {
        self.unique_before_existing_exclusion
    }

    pub(crate) const fn request_count(self) -> usize {
        self.request_count
    }

    pub(crate) const fn request_coordinate_cells(self) -> usize {
        self.request_coordinate_cells
    }
}

/// Immutable canonical parent-source proposal for an epoch-zero bootstrap.
///
/// Construction is possible only through a complete parent ordinary-source
/// incidence index.  The proposal carries no source row or mathematical
/// authority: a scheduler must regenerate every request from its own sealed
/// parent [`crate::identity::CompletedIbpSourceRows`] before querying it.
#[derive(Debug)]
pub(crate) struct InitialParentSourceProposal {
    family_fingerprint: String,
    context_fingerprint: String,
    completed_identity: Arc<()>,
    source_chronology: Box<[RowId]>,
    telemetry: InitialParentSourceProposalTelemetry,
    requests: Box<[TranslatedSourceRequest]>,
}

impl OrdinarySourceIncidenceIndex<'_> {
    /// Expand a canonical set of parent-lattice support points into exact
    /// parent translated-source requests.
    ///
    /// `parent_support` must be nonempty, strictly ordered, duplicate-free,
    /// and have this incidence index's arity.  Requiring a canonical slice
    /// keeps coordinate-chart enumeration policy outside generic discovery.
    pub(crate) fn try_nominate_initial_parent_support(
        &self,
        completed: &CompletedIbpSourceRows,
        parent_support: &[IntegralShift],
        limits: SourceDiscoveryLimits,
    ) -> Result<InitialParentSourceProposal, SourceDiscoveryError> {
        if parent_support.is_empty() {
            return Err(SourceDiscoveryError::Invariant {
                detail: "initial parent support is empty",
            });
        }
        if parent_support.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(SourceDiscoveryError::Invariant {
                detail: "initial parent support is not canonical and unique",
            });
        }
        check_limit(
            SUPPORT_ENTRIES,
            parent_support.len(),
            limits.max_obstruction_support,
        )?;
        self.try_verify_limits(limits)?;
        for shift in parent_support {
            if shift.len() != self.arity() {
                return Err(SourceDiscoveryError::WrongArity {
                    object: "initial parent support shift",
                    expected: self.arity(),
                    actual: shift.len(),
                });
            }
        }
        let raw_incidence_visits = checked_mul(
            INCIDENCE_VISITS,
            parent_support.len(),
            self.term_occurrences(),
        )?;
        check_limit(
            INCIDENCE_VISITS,
            raw_incidence_visits,
            limits.max_incidence_visits,
        )?;
        check_limit(RAW_REQUESTS, raw_incidence_visits, limits.max_raw_requests)?;
        let raw_coordinate_cells =
            checked_mul(CANDIDATE_COORDINATES, raw_incidence_visits, self.arity())?;
        check_limit(
            CANDIDATE_COORDINATES,
            raw_coordinate_cells,
            limits.max_candidate_coordinate_cells,
        )?;
        if !self.exactly_replays_completed(completed) {
            return Err(SourceDiscoveryError::CompletedSourceChronologyMismatch);
        }
        let mut support = try_vec(SUPPORT_ENTRIES, parent_support.len())?;
        support.extend(parent_support.iter());
        let nominations = nominate_requests(self, &support, &[], limits)?;
        if nominations.excluded_existing_requests != 0 {
            return Err(SourceDiscoveryError::Invariant {
                detail: "initial parent proposal unexpectedly excluded materialized requests",
            });
        }
        let request_coordinate_cells = checked_mul(
            CANDIDATE_COORDINATES,
            nominations.requests.len(),
            self.arity(),
        )?;
        let telemetry = InitialParentSourceProposalTelemetry {
            arity: self.arity(),
            ordinary_source_rows: self.source_count(),
            source_term_occurrences: self.term_occurrences(),
            distinct_source_shifts: self.distinct_shift_count(),
            parent_support_entries: parent_support.len(),
            raw_incidence_visits: nominations.raw_incidence_visits,
            unique_before_existing_exclusion: nominations.unique_before_existing_exclusion,
            request_count: nominations.requests.len(),
            request_coordinate_cells,
        };
        let mut source_chronology = try_vec(SOURCE_CHRONOLOGY, self.source_count())?;
        for ordinal in 0..self.source_count() {
            source_chronology.push(
                completed
                    .source_row_id(ordinal)
                    .ok_or(SourceDiscoveryError::CompletedSourceChronologyMismatch)?
                    .clone(),
            );
        }
        Ok(InitialParentSourceProposal {
            family_fingerprint: try_copy_scope(FAMILY_SCOPE, self.family_fingerprint())?,
            context_fingerprint: try_copy_scope(CONTEXT_SCOPE, self.context_fingerprint())?,
            completed_identity: completed.identity_owner(),
            source_chronology: source_chronology.into_boxed_slice(),
            telemetry,
            requests: nominations.requests.into_boxed_slice(),
        })
    }
}

impl InitialParentSourceProposal {
    pub(crate) fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub(crate) const fn telemetry(&self) -> InitialParentSourceProposalTelemetry {
        self.telemetry
    }

    pub(crate) fn requests(&self) -> &[TranslatedSourceRequest] {
        &self.requests
    }

    /// Reapply the current cold-boundary policy and bind this immutable
    /// proposal to the scheduler's authoritative parent source chronology.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn try_verify_for_parent(
        &self,
        family_fingerprint: &str,
        context_fingerprint: &str,
        arity: usize,
        completed: &CompletedIbpSourceRows,
        limits: SourceDiscoveryLimits,
    ) -> Result<(), SourceDiscoveryError> {
        if self.family_fingerprint() != family_fingerprint {
            return Err(SourceDiscoveryError::ScopeMismatch {
                detail: "initial parent proposal belongs to a different integral family",
            });
        }
        if self.context_fingerprint() != context_fingerprint {
            return Err(SourceDiscoveryError::ScopeMismatch {
                detail: "initial parent proposal belongs to a different coefficient context",
            });
        }
        if completed.family_fingerprint() != family_fingerprint {
            return Err(SourceDiscoveryError::ScopeMismatch {
                detail: "completed ordinary-source barrier belongs to a different integral family",
            });
        }
        if completed.context_fingerprint() != context_fingerprint {
            return Err(SourceDiscoveryError::ScopeMismatch {
                detail: "completed ordinary-source barrier belongs to a different coefficient context",
            });
        }
        if self.telemetry.arity != arity {
            return Err(SourceDiscoveryError::WrongArity {
                object: "initial parent proposal",
                expected: arity,
                actual: self.telemetry.arity,
            });
        }
        if !completed.owns_identity(&self.completed_identity) {
            return Err(SourceDiscoveryError::ScopeMismatch {
                detail: "initial parent proposal belongs to a different completed ordinary-source barrier",
            });
        }
        let ordinary_source_rows = completed.source_row_count();
        if self.telemetry.ordinary_source_rows != ordinary_source_rows
            || self.source_chronology.len() != ordinary_source_rows
        {
            return Err(SourceDiscoveryError::ScopeMismatch {
                detail: "initial parent proposal belongs to a different ordinary-source chronology",
            });
        }
        for (ordinal, expected) in self.source_chronology.iter().enumerate() {
            if completed.source_row_id(ordinal) != Some(expected) {
                return Err(SourceDiscoveryError::ScopeMismatch {
                    detail: "initial parent proposal ordinary-source chronology changed",
                });
            }
        }
        if self.requests.is_empty()
            || self.requests.windows(2).any(|pair| pair[0] >= pair[1])
            || self.telemetry.request_count != self.requests.len()
            || self.telemetry.parent_support_entries == 0
            || self.telemetry.source_term_occurrences == 0
            || self.telemetry.distinct_source_shifts == 0
            || self.telemetry.unique_before_existing_exclusion != self.requests.len()
        {
            return Err(SourceDiscoveryError::Invariant {
                detail: "initial parent proposal payload is not canonical and census-complete",
            });
        }
        let expected_incidence_visits = checked_mul(
            INCIDENCE_VISITS,
            self.telemetry.parent_support_entries,
            self.telemetry.source_term_occurrences,
        )?;
        if self.telemetry.raw_incidence_visits != expected_incidence_visits {
            return Err(SourceDiscoveryError::Invariant {
                detail: "initial parent proposal inverse-incidence census changed",
            });
        }
        for request in self.requests() {
            if request.source_ordinal() >= ordinary_source_rows {
                return Err(SourceDiscoveryError::ScopeMismatch {
                    detail: "initial parent proposal names a foreign ordinary source",
                });
            }
            if request.offset().len() != arity {
                return Err(SourceDiscoveryError::WrongArity {
                    object: "initial parent translated-source request",
                    expected: arity,
                    actual: request.offset().len(),
                });
            }
        }

        check_limit("source-discovery arity", arity, limits.max_arity)?;
        check_limit(SOURCE_ROWS, ordinary_source_rows, limits.max_source_rows)?;
        check_limit(
            SOURCE_TERMS,
            self.telemetry.source_term_occurrences,
            limits.max_source_term_occurrences,
        )?;
        check_limit(
            DISTINCT_SHIFTS,
            self.telemetry.distinct_source_shifts,
            limits.max_distinct_source_shifts,
        )?;
        check_limit(
            SUPPORT_ENTRIES,
            self.telemetry.parent_support_entries,
            limits.max_obstruction_support,
        )?;
        check_limit(
            INCIDENCE_VISITS,
            self.telemetry.raw_incidence_visits,
            limits.max_incidence_visits,
        )?;
        check_limit(
            RAW_REQUESTS,
            self.telemetry.raw_incidence_visits,
            limits.max_raw_requests,
        )?;
        check_limit(
            UNIQUE_REQUESTS,
            self.telemetry.unique_before_existing_exclusion,
            limits.max_unique_requests,
        )?;
        let raw_coordinate_cells = checked_mul(
            CANDIDATE_COORDINATES,
            self.telemetry.raw_incidence_visits,
            arity,
        )?;
        check_limit(
            CANDIDATE_COORDINATES,
            raw_coordinate_cells,
            limits.max_candidate_coordinate_cells,
        )?;
        let request_coordinate_cells =
            checked_mul(CANDIDATE_COORDINATES, self.requests.len(), arity)?;
        if request_coordinate_cells != self.telemetry.request_coordinate_cells {
            return Err(SourceDiscoveryError::Invariant {
                detail: "initial parent proposal request-coordinate census changed",
            });
        }
        Ok(())
    }
}

fn try_copy_scope(resource: &'static str, value: &str) -> Result<String, SourceDiscoveryError> {
    let mut retained = String::new();
    retained.try_reserve_exact(value.len()).map_err(|_| {
        SourceDiscoveryError::AllocationFailure {
            resource,
            requested: value.len(),
        }
    })?;
    retained.push_str(value);
    Ok(retained)
}

#[cfg(test)]
mod payload_shape_test {
    use super::*;

    /// This exhaustive destructure is a compile-time tripwire: adding any
    /// authority-bearing field to the proposal requires intentionally
    /// changing this test and reviewing the boundary.
    #[test]
    fn proposal_payload_is_only_scope_admission_seals_census_and_requests() {
        let proposal = InitialParentSourceProposal {
            family_fingerprint: "family".to_owned(),
            context_fingerprint: "context".to_owned(),
            completed_identity: Arc::new(()),
            source_chronology: vec![RowId::OrdinaryIbp {
                contraction_momentum: 0,
                differentiated_loop: 0,
            }]
            .into_boxed_slice(),
            telemetry: InitialParentSourceProposalTelemetry {
                arity: 1,
                ordinary_source_rows: 1,
                source_term_occurrences: 1,
                distinct_source_shifts: 1,
                parent_support_entries: 1,
                raw_incidence_visits: 1,
                unique_before_existing_exclusion: 1,
                request_count: 1,
                request_coordinate_cells: 1,
            },
            requests: vec![TranslatedSourceRequest::new(
                0,
                IntegralShift::try_new([0]).unwrap(),
            )]
            .into_boxed_slice(),
        };
        let InitialParentSourceProposal {
            family_fingerprint,
            context_fingerprint,
            completed_identity,
            source_chronology,
            telemetry,
            requests,
        } = proposal;
        assert_eq!(family_fingerprint, "family");
        assert_eq!(context_fingerprint, "context");
        assert_eq!(Arc::strong_count(&completed_identity), 1);
        assert_eq!(source_chronology.len(), 1);
        assert_eq!(telemetry.request_count(), requests.len());
    }
}
