use std::sync::Arc;

use crate::foundry::completion::frame::PhysicalFramePlanIdentity;
use crate::foundry::completion::frame::modular::{
    ModularRightObstruction, ModularRightObstructionIdentity, ModularSampleFingerprint,
};
use crate::identity::TranslatedSourceRequest;

use super::residual::ResidualConstructionSeal;

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
    identity: Arc<()>,
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
/// This payload deliberately retains no finite-field coefficient. Its
/// requests belong exclusively to the probe-local accumulator whose checked
/// sample produced them; they must never be unioned across independent
/// modular states as scheduling or evidence input. A cross-probe identity
/// union may be computed only as detached telemetry. It carries no
/// exact-relation, owner, terminal, or closure authority.
#[derive(Clone, Debug)]
pub(crate) struct NonzeroIncidentTranslationResiduals {
    census: ResidualCensusProvenance,
    requests: Box<[TranslatedSourceRequest]>,
    evaluated_candidates: usize,
    evaluated_source_terms: usize,
    paired_source_terms: usize,
    obstruction_support_entries: usize,
}

/// Private admission seal retained by one complete residual evaluation.
///
/// None of these in-memory identities is mathematical payload.  Together
/// they prevent an empty request slice from being detached from the exact
/// nominations, incidence index, physical plan, checked obstruction, or
/// modular sample which were joined before every candidate row was
/// evaluated.
#[derive(Clone, Debug)]
pub(super) struct ResidualCensusProvenance {
    nomination_identity: Arc<()>,
    incidence_identity: Arc<()>,
    plan_identity: PhysicalFramePlanIdentity,
    obstruction_identity: ModularRightObstructionIdentity,
    sample: Arc<ModularSampleFingerprint>,
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

    pub(super) fn identity_owner(&self) -> Arc<()> {
        self.identity.clone()
    }

    pub(super) fn owns_identity(&self, identity: &Arc<()>) -> bool {
        Arc::ptr_eq(&self.identity, identity)
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
            identity: Arc::new(()),
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

impl ResidualCensusProvenance {
    pub(super) fn new(
        _seal: ResidualConstructionSeal,
        nominations: &IncidentTranslationNominations,
        incidence_identity: Arc<()>,
        plan_identity: PhysicalFramePlanIdentity,
        obstruction_identity: ModularRightObstructionIdentity,
        sample: Arc<ModularSampleFingerprint>,
    ) -> Self {
        Self {
            nomination_identity: nominations.identity_owner(),
            incidence_identity,
            plan_identity,
            obstruction_identity,
            sample,
        }
    }

    pub(super) fn belongs_to_nominations(
        &self,
        nominations: &IncidentTranslationNominations,
    ) -> bool {
        nominations.owns_identity(&self.nomination_identity)
    }

    pub(super) fn belongs_to_incidence(&self, incidence: &Arc<()>) -> bool {
        Arc::ptr_eq(&self.incidence_identity, incidence)
    }

    pub(super) fn belongs_to_plan(
        &self,
        plan: &crate::foundry::completion::frame::PhysicalFramePlan,
    ) -> bool {
        self.plan_identity.belongs_to(plan)
    }

    pub(super) fn belongs_to_obstruction(&self, obstruction: &ModularRightObstruction<'_>) -> bool {
        self.obstruction_identity.belongs_to(obstruction)
    }

    pub(super) fn belongs_to_sample(&self, sample: &Arc<ModularSampleFingerprint>) -> bool {
        Arc::ptr_eq(&self.sample, sample)
    }
}

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

    pub(super) const fn census(&self) -> &ResidualCensusProvenance {
        &self.census
    }

    #[cfg(test)]
    pub(super) fn set_paired_source_terms_for_test(&mut self, paired_source_terms: usize) {
        self.paired_source_terms = paired_source_terms;
    }

    pub(super) fn from_parts(
        _seal: ResidualConstructionSeal,
        census: ResidualCensusProvenance,
        requests: Vec<TranslatedSourceRequest>,
        evaluated_candidates: usize,
        evaluated_source_terms: usize,
        paired_source_terms: usize,
        obstruction_support_entries: usize,
    ) -> Self {
        Self {
            census,
            requests: requests.into_boxed_slice(),
            evaluated_candidates,
            evaluated_source_terms,
            paired_source_terms,
            obstruction_support_entries,
        }
    }
}

// Provenance is an admission seal and deliberately not structural payload.
// Preserve the existing deterministic equality used by residual regressions.
impl PartialEq for NonzeroIncidentTranslationResiduals {
    fn eq(&self, other: &Self) -> bool {
        self.requests == other.requests
            && self.evaluated_candidates == other.evaluated_candidates
            && self.evaluated_source_terms == other.evaluated_source_terms
            && self.paired_source_terms == other.paired_source_terms
            && self.obstruction_support_entries == other.obstruction_support_entries
    }
}

impl Eq for NonzeroIncidentTranslationResiduals {}
