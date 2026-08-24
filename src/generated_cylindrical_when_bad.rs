//! Persistent generated-cylindrical `WhenBad` authority.
//!
//! The low-level [`crate::WhenBadCompiler`] deliberately returns a
//! candidate-independent core for generated cylindrical input.  This module
//! closes that persistence boundary by retaining the exact authenticated
//! [`GeneratedCylindricalGlobalCandidateAuthority`] next to the core.  It
//! accepts neither the candidate umbrella nor its locus-bound arm, fabricates
//! no discovery anchor, and publishes no legacy generated-source certificate.
//!
//! Replay is intentionally complete: it replays the retained cylindrical
//! source through the candidate, invokes the shared `WhenBad` compiler again,
//! requires the same certified/unsupported variant, and compares the entire
//! candidate and core payloads.

use std::fmt;

use crate::generated_cylindrical_candidate_authority::{
    GeneratedCylindricalGlobalCandidateAuthority, GeneratedCylindricalReplaySession,
    ReplayedGeneratedCylindricalGlobalCandidate,
};
use crate::when_bad::{
    WhenBadCandidateBinding, WhenBadCertifiedCore, WhenBadCompiler, WhenBadCompilerError,
    WhenBadCompilerLimits, WhenBadCompilerStats, WhenBadCoreCompilation, WhenBadDomainCondition,
    WhenBadLeafClassification, WhenBadLeakEvent, WhenBadUniformDescentWitness,
    WhenBadUnsupportedCore, WhenBadUnsupportedReason,
};
use crate::{IntegralFamily, ParametricCoefficientContext, SymbolicSectorCasePartitionCertificate};

/// Schema for a retained global cylindrical candidate and its shared
/// `WhenBad` core.
pub const GENERATED_CYLINDRICAL_WHEN_BAD_V1_SCHEMA: &str =
    "rustred-generated-cylindrical-when-bad-v1";

/// A replayable generated-cylindrical candidate with a certified symbolic
/// admissibility partition.
#[derive(Clone)]
pub struct GeneratedCylindricalWhenBadCertificate {
    schema: &'static str,
    candidate: GeneratedCylindricalGlobalCandidateAuthority,
    /// Independent copy of the compiler input.  Replay must not learn its
    /// limits exclusively from the core it is checking: otherwise a relaxed
    /// but nonbinding core limit could authorize itself during recompilation.
    compile_limits: WhenBadCompilerLimits,
    core: WhenBadCertifiedCore,
}

impl GeneratedCylindricalWhenBadCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    /// The exact Global authority retained by this proof.  A locus-bound
    /// candidate cannot inhabit this field.
    pub const fn candidate(&self) -> &GeneratedCylindricalGlobalCandidateAuthority {
        &self.candidate
    }

    pub const fn binding(&self) -> &WhenBadCandidateBinding {
        self.core.binding()
    }

    pub fn domain_conditions(&self) -> &[WhenBadDomainCondition] {
        self.core.domain_conditions()
    }

    pub fn base_domain_guards(&self) -> impl Iterator<Item = &WhenBadDomainCondition> {
        self.core.base_domain_guards()
    }

    pub fn index_domain_guards(&self) -> impl Iterator<Item = &WhenBadDomainCondition> {
        self.core.index_domain_guards()
    }

    pub(crate) fn index_domain_guards_with_ordinals(
        &self,
    ) -> impl Iterator<Item = (usize, &WhenBadDomainCondition)> {
        self.core.index_domain_guards_with_ordinals()
    }

    pub fn leak_events(&self) -> &[WhenBadLeakEvent] {
        self.core.leak_events()
    }

    pub fn descent_witnesses(&self) -> &[WhenBadUniformDescentWitness] {
        self.core.descent_witnesses()
    }

    pub const fn partition(&self) -> &SymbolicSectorCasePartitionCertificate {
        self.core.partition()
    }

    pub fn classifications(&self) -> &[WhenBadLeafClassification] {
        self.core.classifications()
    }

    pub const fn limits(&self) -> WhenBadCompilerLimits {
        self.compile_limits
    }

    pub const fn stats(&self) -> WhenBadCompilerStats {
        self.core.stats()
    }

    /// Conservative capacity-aware bytes owned by the shared `WhenBad` core.
    /// The generated candidate authority is charged independently.
    pub const fn retained_core_bytes(&self) -> usize {
        self.core.retained_core_bytes()
    }

    /// Locate the unique retained symbolic leaf for concrete integral
    /// indices.  Base parameters remain formal.
    pub fn classification_for_indices(
        &self,
        context: &ParametricCoefficientContext,
        indices: &[i64],
    ) -> Result<Option<&WhenBadLeafClassification>, WhenBadCompilerError> {
        self.core.classification_for_indices(context, indices)
    }

    /// Rebuild the complete shared proof from the retained generated source
    /// and compare the whole candidate/core payload.
    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), WhenBadCompilerError> {
        let mut session = GeneratedCylindricalReplaySession::new(family, context);
        self.replay_with_replay_session(&mut session)
    }

    pub(crate) fn replay_with_replay_session(
        &self,
        session: &mut GeneratedCylindricalReplaySession<'_>,
    ) -> Result<(), WhenBadCompilerError> {
        self.preflight_replay(session.family(), session.context())?;
        session.authenticate_source(self.candidate.source())?;
        self.replay_with_authenticated_session(session)
    }

    pub(crate) fn replay_with_authenticated_session(
        &self,
        session: &GeneratedCylindricalReplaySession<'_>,
    ) -> Result<(), WhenBadCompilerError> {
        self.preflight_replay(session.family(), session.context())?;
        let replayed_candidate = self.candidate.replay_with_authenticated_session(session)?;
        let replayed_core = WhenBadCompiler::compile_replayed_cylindrical_global_candidate(
            replayed_candidate,
            self.compile_limits,
        )?;
        let WhenBadCoreCompilation::Certified(core) = replayed_core else {
            return Err(WhenBadCompilerError::ReplayMismatch);
        };
        let replayed = Self {
            schema: GENERATED_CYLINDRICAL_WHEN_BAD_V1_SCHEMA,
            candidate: self.candidate.clone(),
            compile_limits: self.compile_limits,
            core,
        };
        if self.payload_eq_with_authenticated_candidate(&replayed) {
            Ok(())
        } else {
            Err(WhenBadCompilerError::ReplayMismatch)
        }
    }

    pub(crate) fn preflight_replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), WhenBadCompilerError> {
        if self.schema != GENERATED_CYLINDRICAL_WHEN_BAD_V1_SCHEMA {
            return Err(WhenBadCompilerError::SchemaMismatch);
        }
        if self.core.limits() != self.compile_limits {
            return Err(WhenBadCompilerError::ReplayMismatch);
        }
        if self.candidate.family_fingerprint() != family.fingerprint_ref() {
            return Err(WhenBadCompilerError::FamilyMismatch);
        }
        if self.candidate.context_fingerprint() != context.fingerprint() {
            return Err(WhenBadCompilerError::ContextMismatch);
        }
        self.core.replay_capacity_census()?;
        self.candidate.preflight_replay(family, context)?;
        Ok(())
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.candidate.payload_eq(&other.candidate)
            && self.compile_limits == other.compile_limits
            && self.core.payload_eq(&other.core)
    }

    fn payload_eq_with_authenticated_candidate(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.candidate.shares_binding_allocation(&other.candidate)
            && self.compile_limits == other.compile_limits
            && self.core.payload_eq(&other.core)
    }

    pub(crate) fn payload_eq_with_replayed_source(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self
                .candidate
                .payload_eq_with_replayed_source(&other.candidate)
            && self.compile_limits == other.compile_limits
            && self.core.payload_eq(&other.core)
    }
}

impl fmt::Debug for GeneratedCylindricalWhenBadCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedCylindricalWhenBadCertificate")
            .field("schema", &self.schema)
            .field("candidate", &"<redacted>")
            .field(
                "domain_condition_count",
                &self.core.domain_conditions().len(),
            )
            .field("leak_event_count", &self.core.leak_events().len())
            .field(
                "descent_witness_count",
                &self.core.descent_witnesses().len(),
            )
            .field("leaf_count", &self.core.classifications().len())
            .field("stats", &self.core.stats())
            .finish_non_exhaustive()
    }
}

/// A replayable generated-cylindrical result for which the shared `WhenBad`
/// algorithm proved no uniform recurrence.  This is not a rule, a master, or
/// a zero-sector claim.
#[derive(Clone)]
pub struct GeneratedCylindricalWhenBadUnsupported {
    schema: &'static str,
    candidate: GeneratedCylindricalGlobalCandidateAuthority,
    /// Independent compiler-input binding; see the certified arm.
    compile_limits: WhenBadCompilerLimits,
    core: WhenBadUnsupportedCore,
}

impl GeneratedCylindricalWhenBadUnsupported {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn candidate(&self) -> &GeneratedCylindricalGlobalCandidateAuthority {
        &self.candidate
    }

    pub const fn binding(&self) -> &WhenBadCandidateBinding {
        self.core.binding()
    }

    pub const fn reason(&self) -> &WhenBadUnsupportedReason {
        self.core.reason()
    }

    pub const fn limits(&self) -> WhenBadCompilerLimits {
        self.compile_limits
    }

    /// Conservative capacity-aware bytes owned by the unsupported shared
    /// `WhenBad` core. The generated candidate authority is charged
    /// independently.
    pub const fn retained_core_bytes(&self) -> usize {
        self.core.retained_core_bytes()
    }

    /// Fully regenerate the unsupported result and require both the same
    /// variant and the same complete payload.
    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), WhenBadCompilerError> {
        let mut session = GeneratedCylindricalReplaySession::new(family, context);
        self.replay_with_replay_session(&mut session)
    }

    pub(crate) fn replay_with_replay_session(
        &self,
        session: &mut GeneratedCylindricalReplaySession<'_>,
    ) -> Result<(), WhenBadCompilerError> {
        self.preflight_replay(session.family(), session.context())?;
        session.authenticate_source(self.candidate.source())?;
        self.replay_with_authenticated_session(session)
    }

    pub(crate) fn replay_with_authenticated_session(
        &self,
        session: &GeneratedCylindricalReplaySession<'_>,
    ) -> Result<(), WhenBadCompilerError> {
        self.preflight_replay(session.family(), session.context())?;
        let replayed_candidate = self.candidate.replay_with_authenticated_session(session)?;
        let replayed_core = WhenBadCompiler::compile_replayed_cylindrical_global_candidate(
            replayed_candidate,
            self.compile_limits,
        )?;
        let WhenBadCoreCompilation::Unsupported(core) = replayed_core else {
            return Err(WhenBadCompilerError::ReplayMismatch);
        };
        let replayed = Self {
            schema: GENERATED_CYLINDRICAL_WHEN_BAD_V1_SCHEMA,
            candidate: self.candidate.clone(),
            compile_limits: self.compile_limits,
            core,
        };
        if self.payload_eq_with_authenticated_candidate(&replayed) {
            Ok(())
        } else {
            Err(WhenBadCompilerError::ReplayMismatch)
        }
    }

    pub(crate) fn preflight_replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), WhenBadCompilerError> {
        if self.schema != GENERATED_CYLINDRICAL_WHEN_BAD_V1_SCHEMA {
            return Err(WhenBadCompilerError::SchemaMismatch);
        }
        if self.core.limits() != self.compile_limits {
            return Err(WhenBadCompilerError::ReplayMismatch);
        }
        if self.candidate.family_fingerprint() != family.fingerprint_ref() {
            return Err(WhenBadCompilerError::FamilyMismatch);
        }
        if self.candidate.context_fingerprint() != context.fingerprint() {
            return Err(WhenBadCompilerError::ContextMismatch);
        }
        self.core.replay_capacity_census()?;
        self.candidate.preflight_replay(family, context)?;
        Ok(())
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.candidate.payload_eq(&other.candidate)
            && self.compile_limits == other.compile_limits
            && self.core.payload_eq(&other.core)
    }

    fn payload_eq_with_authenticated_candidate(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.candidate.shares_binding_allocation(&other.candidate)
            && self.compile_limits == other.compile_limits
            && self.core.payload_eq(&other.core)
    }

    pub(crate) fn payload_eq_with_replayed_source(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self
                .candidate
                .payload_eq_with_replayed_source(&other.candidate)
            && self.compile_limits == other.compile_limits
            && self.core.payload_eq(&other.core)
    }
}

impl fmt::Debug for GeneratedCylindricalWhenBadUnsupported {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason_kind = match self.core.reason() {
            WhenBadUnsupportedReason::NonUniformSameSectorDescent { .. } => {
                "non-uniform-same-sector-descent"
            }
            WhenBadUnsupportedReason::ZeroSameSectorComplexityDelta { .. } => {
                "zero-same-sector-complexity-delta"
            }
            WhenBadUnsupportedReason::UnboundedIndexAddition { .. } => "unbounded-index-addition",
        };
        formatter
            .debug_struct("GeneratedCylindricalWhenBadUnsupported")
            .field("schema", &self.schema)
            .field("candidate", &"<redacted>")
            .field("reason_kind", &reason_kind)
            .finish_non_exhaustive()
    }
}

/// Typed generated-cylindrical `WhenBad` result.  Unsupported output remains
/// explicit and cannot be mistaken for an applicable recurrence.
#[derive(Clone, Debug)]
pub enum GeneratedCylindricalWhenBadCompilation {
    Certified(GeneratedCylindricalWhenBadCertificate),
    Unsupported(GeneratedCylindricalWhenBadUnsupported),
}

impl GeneratedCylindricalWhenBadCompilation {
    pub const fn schema(&self) -> &'static str {
        match self {
            Self::Certified(certificate) => certificate.schema(),
            Self::Unsupported(unsupported) => unsupported.schema(),
        }
    }

    pub const fn candidate(&self) -> &GeneratedCylindricalGlobalCandidateAuthority {
        match self {
            Self::Certified(certificate) => certificate.candidate(),
            Self::Unsupported(unsupported) => unsupported.candidate(),
        }
    }

    pub const fn binding(&self) -> &WhenBadCandidateBinding {
        match self {
            Self::Certified(certificate) => certificate.binding(),
            Self::Unsupported(unsupported) => unsupported.binding(),
        }
    }

    pub const fn is_certified(&self) -> bool {
        matches!(self, Self::Certified(_))
    }

    pub const fn is_unsupported(&self) -> bool {
        matches!(self, Self::Unsupported(_))
    }

    pub fn retained_core_bytes(&self) -> usize {
        match self {
            Self::Certified(certificate) => certificate.retained_core_bytes(),
            Self::Unsupported(unsupported) => unsupported.retained_core_bytes(),
        }
    }

    pub(crate) fn preflight_replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), WhenBadCompilerError> {
        match self {
            Self::Certified(certificate) => certificate.preflight_replay(family, context),
            Self::Unsupported(unsupported) => unsupported.preflight_replay(family, context),
        }
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), WhenBadCompilerError> {
        match self {
            Self::Certified(certificate) => certificate.replay(family, context),
            Self::Unsupported(unsupported) => unsupported.replay(family, context),
        }
    }

    pub(crate) fn replay_with_replay_session(
        &self,
        session: &mut GeneratedCylindricalReplaySession<'_>,
    ) -> Result<(), WhenBadCompilerError> {
        match self {
            Self::Certified(certificate) => certificate.replay_with_replay_session(session),
            Self::Unsupported(unsupported) => unsupported.replay_with_replay_session(session),
        }
    }

    pub(crate) fn replay_with_authenticated_session(
        &self,
        session: &GeneratedCylindricalReplaySession<'_>,
    ) -> Result<(), WhenBadCompilerError> {
        match self {
            Self::Certified(certificate) => certificate.replay_with_authenticated_session(session),
            Self::Unsupported(unsupported) => {
                unsupported.replay_with_authenticated_session(session)
            }
        }
    }

    /// Compare every externally meaningful retained field, including the
    /// exact Global candidate and same-variant shared core.
    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Certified(left), Self::Certified(right)) => left.payload_eq(right),
            (Self::Unsupported(left), Self::Unsupported(right)) => left.payload_eq(right),
            _ => false,
        }
    }

    /// Compare two independently compiled cores while requiring both
    /// wrappers to retain the same candidate-binding allocation. This is the
    /// local W1/W2 check used after a sealed candidate replay capability has
    /// already established C1/C2 from the exact persistent source.
    fn payload_eq_with_same_candidate_binding(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Certified(left), Self::Certified(right)) => {
                left.payload_eq_with_authenticated_candidate(right)
            }
            (Self::Unsupported(left), Self::Unsupported(right)) => {
                left.payload_eq_with_authenticated_candidate(right)
            }
            _ => false,
        }
    }

    #[cfg(test)]
    pub(crate) fn corrupt_schema_for_test(&mut self) {
        match self {
            Self::Certified(certificate) => certificate.schema = "rustred-test-corrupt-schema",
            Self::Unsupported(unsupported) => unsupported.schema = "rustred-test-corrupt-schema",
        }
    }

    #[cfg(test)]
    pub(crate) fn corrupt_limits_for_test(&mut self) {
        let limits = match self {
            Self::Certified(certificate) => &mut certificate.compile_limits,
            Self::Unsupported(unsupported) => &mut unsupported.compile_limits,
        };
        limits.max_rhs_terms = limits.max_rhs_terms.saturating_add(1);
    }
}

pub struct GeneratedCylindricalWhenBadCompiler;

impl GeneratedCylindricalWhenBadCompiler {
    /// Compile only an authenticated Global cylindrical candidate.  The exact
    /// argument type is the authority boundary: callers cannot pass the
    /// umbrella or convert a locus-bound candidate through this API.
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        candidate: &GeneratedCylindricalGlobalCandidateAuthority,
        limits: WhenBadCompilerLimits,
    ) -> Result<GeneratedCylindricalWhenBadCompilation, WhenBadCompilerError> {
        let mut session = GeneratedCylindricalReplaySession::new(family, context);
        Self::compile_with_replay_session(candidate, limits, &mut session)
    }

    pub(crate) fn compile_with_replay_session(
        candidate: &GeneratedCylindricalGlobalCandidateAuthority,
        limits: WhenBadCompilerLimits,
        session: &mut GeneratedCylindricalReplaySession<'_>,
    ) -> Result<GeneratedCylindricalWhenBadCompilation, WhenBadCompilerError> {
        let replayed_candidate = candidate.replay_with_replay_session(session)?;
        Self::compile_replayed_candidate(replayed_candidate, limits)
    }

    pub(crate) fn compile_with_authenticated_session(
        candidate: &GeneratedCylindricalGlobalCandidateAuthority,
        limits: WhenBadCompilerLimits,
        session: &GeneratedCylindricalReplaySession<'_>,
    ) -> Result<GeneratedCylindricalWhenBadCompilation, WhenBadCompilerError> {
        let replayed_candidate = candidate.replay_with_authenticated_session(session)?;
        Self::compile_replayed_candidate(replayed_candidate, limits)
    }

    pub(crate) fn compile_replayed_candidate(
        replayed_candidate: ReplayedGeneratedCylindricalGlobalCandidate<'_, '_, '_>,
        limits: WhenBadCompilerLimits,
    ) -> Result<GeneratedCylindricalWhenBadCompilation, WhenBadCompilerError> {
        let first = Self::compile_replayed_candidate_once(replayed_candidate, limits)?;
        let second = Self::compile_replayed_candidate_once(replayed_candidate, limits)?;
        if first.payload_eq_with_same_candidate_binding(&second) {
            Ok(first)
        } else {
            Err(WhenBadCompilerError::ReplayMismatch)
        }
    }

    fn compile_replayed_candidate_once(
        replayed_candidate: ReplayedGeneratedCylindricalGlobalCandidate<'_, '_, '_>,
        limits: WhenBadCompilerLimits,
    ) -> Result<GeneratedCylindricalWhenBadCompilation, WhenBadCompilerError> {
        let candidate = replayed_candidate.candidate().clone();
        let core = WhenBadCompiler::compile_replayed_cylindrical_global_candidate(
            replayed_candidate,
            limits,
        )?;
        Ok(match core {
            WhenBadCoreCompilation::Certified(core) => {
                GeneratedCylindricalWhenBadCompilation::Certified(
                    GeneratedCylindricalWhenBadCertificate {
                        schema: GENERATED_CYLINDRICAL_WHEN_BAD_V1_SCHEMA,
                        candidate,
                        compile_limits: limits,
                        core,
                    },
                )
            }
            WhenBadCoreCompilation::Unsupported(core) => {
                GeneratedCylindricalWhenBadCompilation::Unsupported(
                    GeneratedCylindricalWhenBadUnsupported {
                        schema: GENERATED_CYLINDRICAL_WHEN_BAD_V1_SCHEMA,
                        candidate,
                        compile_limits: limits,
                        core,
                    },
                )
            }
        })
    }
}
