use std::sync::Arc;

use crate::foundry::completion::frame::modular::ModularRightObstructionIdentity;
use crate::identity::TranslatedSourceRequest;

#[derive(Clone, Debug)]
pub(super) enum IncidentNominationOrigin {
    TargetUnit,
    CheckedObstruction(ModularRightObstructionIdentity),
}

/// Canonical exact source requests nominated by one structural incidence pass.
///
/// The counters are deterministic telemetry.  This value carries no sampled
/// residual, exact-circuit, terminal, owner, or closure authority.
#[derive(Clone, Debug)]
pub(crate) struct IncidentTranslationNominations {
    incidence_identity: Arc<()>,
    origin: IncidentNominationOrigin,
    requests: Box<[TranslatedSourceRequest]>,
    raw_incidence_visits: usize,
    unique_before_existing_exclusion: usize,
    excluded_existing_requests: usize,
}

/// Canonical nominated translations whose complete sampled rows pair
/// nontrivially with one checked right obstruction.
///
/// This payload deliberately retains no finite-field coefficient: its
/// requests may be unioned deterministically across independent modular
/// states, while every numeric residual remains local to the checked sample
/// which produced it.  It carries no exact-relation, owner, terminal, or
/// closure authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NonzeroIncidentTranslationResiduals {
    requests: Box<[TranslatedSourceRequest]>,
    evaluated_candidates: usize,
    evaluated_source_terms: usize,
    paired_source_terms: usize,
    obstruction_support_entries: usize,
}

impl IncidentTranslationNominations {
    pub(crate) fn requests(&self) -> &[TranslatedSourceRequest] {
        &self.requests
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

    pub(super) fn incidence_identity(&self) -> &Arc<()> {
        &self.incidence_identity
    }

    pub(super) const fn origin(&self) -> &IncidentNominationOrigin {
        &self.origin
    }

    pub(super) fn from_parts(
        incidence_identity: Arc<()>,
        origin: IncidentNominationOrigin,
        requests: Vec<TranslatedSourceRequest>,
        raw_incidence_visits: usize,
        unique_before_existing_exclusion: usize,
        excluded_existing_requests: usize,
    ) -> Self {
        Self {
            incidence_identity,
            origin,
            requests: requests.into_boxed_slice(),
            raw_incidence_visits,
            unique_before_existing_exclusion,
            excluded_existing_requests,
        }
    }
}

// Provenance tokens and origin kind are admission seals, not mathematical
// payload. Preserve the preexisting structural equality used by deterministic
// nomination regressions and telemetry comparisons.
impl PartialEq for IncidentTranslationNominations {
    fn eq(&self, other: &Self) -> bool {
        self.requests == other.requests
            && self.raw_incidence_visits == other.raw_incidence_visits
            && self.unique_before_existing_exclusion == other.unique_before_existing_exclusion
            && self.excluded_existing_requests == other.excluded_existing_requests
    }
}

impl Eq for IncidentTranslationNominations {}

impl NonzeroIncidentTranslationResiduals {
    pub(crate) fn requests(&self) -> &[TranslatedSourceRequest] {
        &self.requests
    }

    pub(crate) const fn evaluated_candidates(&self) -> usize {
        self.evaluated_candidates
    }

    pub(crate) const fn evaluated_source_terms(&self) -> usize {
        self.evaluated_source_terms
    }

    pub(crate) const fn paired_source_terms(&self) -> usize {
        self.paired_source_terms
    }

    pub(crate) const fn obstruction_support_entries(&self) -> usize {
        self.obstruction_support_entries
    }

    pub(super) fn from_parts(
        requests: Vec<TranslatedSourceRequest>,
        evaluated_candidates: usize,
        evaluated_source_terms: usize,
        paired_source_terms: usize,
        obstruction_support_entries: usize,
    ) -> Self {
        Self {
            requests: requests.into_boxed_slice(),
            evaluated_candidates,
            evaluated_source_terms,
            paired_source_terms,
            obstruction_support_entries,
        }
    }
}
