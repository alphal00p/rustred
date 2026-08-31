use crate::identity::TranslatedSourceRequest;

/// Canonical exact source requests nominated by one structural incidence pass.
///
/// The counters are deterministic telemetry.  This value carries no sampled
/// residual, exact-circuit, terminal, owner, or closure authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IncidentTranslationNominations {
    requests: Box<[TranslatedSourceRequest]>,
    raw_incidence_visits: usize,
    unique_before_existing_exclusion: usize,
    excluded_existing_requests: usize,
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

    pub(super) fn from_parts(
        requests: Vec<TranslatedSourceRequest>,
        raw_incidence_visits: usize,
        unique_before_existing_exclusion: usize,
        excluded_existing_requests: usize,
    ) -> Self {
        Self {
            requests: requests.into_boxed_slice(),
            raw_incidence_visits,
            unique_before_existing_exclusion,
            excluded_existing_requests,
        }
    }
}
