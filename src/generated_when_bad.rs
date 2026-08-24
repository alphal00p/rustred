//! Canonical generated-source authentication for symbolic `WhenBad` cases.
//!
//! [`crate::WhenBadCompiler`] deliberately accepts any replayable algebraic
//! elimination candidate.  That is useful as a low-level admissibility
//! compiler, but it is not evidence that the source equations are IBP/LI
//! identities of the supplied family.  This module is the production
//! provenance boundary: it regenerates the complete parametric `IBPLI` list
//! from [`crate::IntegralFamily`] and proves that every retained elimination
//! row is either one canonical generated row, a verified whole-row symmetry
//! transport of one, or an exact Symbolica translation of either.  Only after
//! that proof does it invoke the low-level compiler.
//!
//! No row label is trusted.  Translation witnesses are inferred from exact
//! lattice supports and replayed through [`crate::ParametricRelation::translated`],
//! including all coefficient substitutions and exceptional-domain origins.

use std::fmt;
use std::sync::Arc;

use crate::{
    GeneratedSymbolicRowSpanCertificate, GeneratedSymbolicRowSpanCompiler,
    GeneratedSymbolicRowSpanConfig, GeneratedSymbolicRowSpanError, GeneratedSymbolicRowSpanLineage,
    GeneratedSymbolicRowSpanStrategy, IndexShift, IndexSpace, IntegralFamily,
    ParametricCoefficientContext, ParametricIbpConfig, ParametricIbpError,
    ParametricReductionRuleCandidate, ParametricRelation, ParametricRelationError, ParametricRowId,
    VerifiedInternalFamilyPermutationSymmetry, WhenBadCertificate, WhenBadCompilation,
    WhenBadCompiler, WhenBadCompilerError, WhenBadCompilerLimits, WhenBadUnsupported,
};

pub const GENERATED_SOURCE_AUTHENTICATION_V1_SCHEMA: &str =
    "rustred-generated-source-authentication-v1";
pub const GENERATED_SOURCE_AUTHENTICATION_V2_SCHEMA: &str =
    "rustred-generated-source-authentication-v2";
pub const GENERATED_WHEN_BAD_V1_SCHEMA: &str = "rustred-generated-when-bad-v1";
pub const GENERATED_WHEN_BAD_V2_SCHEMA: &str = "rustred-generated-when-bad-v2";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedWhenBadSourceAuthentication {
    CanonicalIbpLiAndExactTranslations,
    CanonicalIbpLiExactTranslationsAndVerifiedWholeRowSymmetryTransports,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedWhenBadLimits {
    pub ibp: ParametricIbpConfig,
    /// Optional symbolic row-span augmentation.  This remains separate from
    /// LiteRed's concrete/numeric `SR` quotient.
    pub row_span: GeneratedSymbolicRowSpanConfig,
    pub when_bad: WhenBadCompilerLimits,
    pub max_canonical_rows: usize,
    pub max_retained_rows: usize,
    pub max_canonical_terms: usize,
    pub max_retained_terms: usize,
    pub max_match_attempts: usize,
    pub max_translation_components: usize,
    pub max_symmetry_witness_components: usize,
    pub max_source_manifest_bytes: usize,
}

impl Default for GeneratedWhenBadLimits {
    fn default() -> Self {
        Self {
            ibp: ParametricIbpConfig::default(),
            row_span: GeneratedSymbolicRowSpanConfig::default(),
            when_bad: WhenBadCompilerLimits::default(),
            max_canonical_rows: 100_000,
            max_retained_rows: 100_000,
            max_canonical_terms: 16_000_000,
            max_retained_terms: 16_000_000,
            max_match_attempts: 10_000_000,
            max_translation_components: 16_000_000,
            max_symmetry_witness_components: 16_000_000,
            max_source_manifest_bytes: 512 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedSourceRowMode {
    CanonicalOriginal,
    ExactTranslation,
    VerifiedWholeRowSymmetryTransport,
    ExactTranslationOfVerifiedWholeRowSymmetryTransport,
}

/// Exact lineage of one row retained by parametric elimination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedSourceRowWitness {
    retained_ordinal: usize,
    basis_ordinal: usize,
    canonical_ordinal: usize,
    symmetry_ordinal: Option<usize>,
    symmetry_permutation: Box<[usize]>,
    mode: GeneratedSourceRowMode,
    translation: IndexShift,
    retained_row_id: ParametricRowId,
    basis_row_id: ParametricRowId,
    canonical_row_id: ParametricRowId,
}

impl GeneratedSourceRowWitness {
    pub const fn retained_ordinal(&self) -> usize {
        self.retained_ordinal
    }

    pub const fn canonical_ordinal(&self) -> usize {
        self.canonical_ordinal
    }

    pub const fn basis_ordinal(&self) -> usize {
        self.basis_ordinal
    }

    pub const fn symmetry_ordinal(&self) -> Option<usize> {
        self.symmetry_ordinal
    }

    pub fn symmetry_permutation(&self) -> Option<&[usize]> {
        self.symmetry_ordinal
            .map(|_| self.symmetry_permutation.as_ref())
    }

    pub const fn mode(&self) -> GeneratedSourceRowMode {
        self.mode
    }

    pub const fn translation(&self) -> &IndexShift {
        &self.translation
    }

    pub const fn retained_row_id(&self) -> &ParametricRowId {
        &self.retained_row_id
    }

    pub const fn basis_row_id(&self) -> &ParametricRowId {
        &self.basis_row_id
    }

    pub const fn canonical_row_id(&self) -> &ParametricRowId {
        &self.canonical_row_id
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedSourceAuthenticationStats {
    canonical_rows: usize,
    authenticated_basis_rows: usize,
    verified_symmetries: usize,
    transported_basis_rows: usize,
    retained_rows: usize,
    match_attempts: usize,
    original_rows: usize,
    translated_rows: usize,
    transported_rows: usize,
    translated_transported_rows: usize,
    translation_components: usize,
    symmetry_witness_components: usize,
    canonical_terms: usize,
    retained_terms: usize,
    source_manifest_bytes: usize,
}

impl GeneratedSourceAuthenticationStats {
    pub const fn canonical_rows(self) -> usize {
        self.canonical_rows
    }
    pub const fn retained_rows(self) -> usize {
        self.retained_rows
    }
    pub const fn authenticated_basis_rows(self) -> usize {
        self.authenticated_basis_rows
    }
    pub const fn verified_symmetries(self) -> usize {
        self.verified_symmetries
    }
    pub const fn transported_basis_rows(self) -> usize {
        self.transported_basis_rows
    }
    pub const fn match_attempts(self) -> usize {
        self.match_attempts
    }
    pub const fn original_rows(self) -> usize {
        self.original_rows
    }
    pub const fn translated_rows(self) -> usize {
        self.translated_rows
    }
    pub const fn transported_rows(self) -> usize {
        self.transported_rows
    }
    pub const fn translated_transported_rows(self) -> usize {
        self.translated_transported_rows
    }
    pub const fn translation_components(self) -> usize {
        self.translation_components
    }
    pub const fn symmetry_witness_components(self) -> usize {
        self.symmetry_witness_components
    }
    pub const fn canonical_terms(self) -> usize {
        self.canonical_terms
    }
    pub const fn retained_terms(self) -> usize {
        self.retained_terms
    }
    pub const fn source_manifest_bytes(self) -> usize {
        self.source_manifest_bytes
    }
}

/// Replayable proof that all elimination roots descend from fresh generated
/// parametric IBP/LI identities.
#[derive(Clone, Debug)]
pub struct GeneratedSourceAuthenticationCertificate {
    schema: &'static str,
    source_authentication: GeneratedWhenBadSourceAuthentication,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    candidate_source_manifest: Arc<str>,
    row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    witnesses: Box<[GeneratedSourceRowWitness]>,
    stats: GeneratedSourceAuthenticationStats,
    limits: GeneratedWhenBadLimits,
}

impl GeneratedSourceAuthenticationCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn source_authentication(&self) -> GeneratedWhenBadSourceAuthentication {
        self.source_authentication
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub fn witnesses(&self) -> &[GeneratedSourceRowWitness] {
        &self.witnesses
    }

    pub fn row_span(&self) -> &GeneratedSymbolicRowSpanCertificate {
        self.row_span.as_ref()
    }

    /// Shared immutable generated row span used to authenticate this source.
    /// Coverage and family compilers retain the same `Arc` for every
    /// candidate/sector rather than deep-cloning the generated basis.
    pub fn row_span_arc(&self) -> &Arc<GeneratedSymbolicRowSpanCertificate> {
        &self.row_span
    }

    pub const fn stats(&self) -> GeneratedSourceAuthenticationStats {
        self.stats
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        candidate: &ParametricReductionRuleCandidate,
    ) -> Result<(), GeneratedWhenBadError> {
        if self.schema != GENERATED_SOURCE_AUTHENTICATION_V1_SCHEMA
            && self.schema != GENERATED_SOURCE_AUTHENTICATION_V2_SCHEMA
        {
            return Err(GeneratedWhenBadError::SchemaMismatch);
        }
        if self.family_fingerprint.as_ref() != family.fingerprint() {
            return Err(GeneratedWhenBadError::WrongFamily);
        }
        if self.context_fingerprint.as_ref() != context.fingerprint() {
            return Err(GeneratedWhenBadError::WrongContext);
        }
        self.row_span.replay(family, context)?;
        self.replay_with_replayed_row_span(family, context, candidate, self.row_span.clone())
    }

    /// Replay this candidate authentication against a caller-shared row span.
    /// The supplied row span is replayed exactly once by this entry point.
    pub fn replay_with_row_span(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        candidate: &ParametricReductionRuleCandidate,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    ) -> Result<(), GeneratedWhenBadError> {
        if self.schema != GENERATED_SOURCE_AUTHENTICATION_V1_SCHEMA
            && self.schema != GENERATED_SOURCE_AUTHENTICATION_V2_SCHEMA
        {
            return Err(GeneratedWhenBadError::SchemaMismatch);
        }
        if self.family_fingerprint.as_ref() != family.fingerprint() {
            return Err(GeneratedWhenBadError::WrongFamily);
        }
        if self.context_fingerprint.as_ref() != context.fingerprint() {
            return Err(GeneratedWhenBadError::WrongContext);
        }
        row_span.replay(family, context)?;
        self.replay_with_replayed_row_span(family, context, candidate, row_span)
    }

    pub(crate) fn replay_with_replayed_row_span(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        candidate: &ParametricReductionRuleCandidate,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    ) -> Result<(), GeneratedWhenBadError> {
        if self.schema != GENERATED_SOURCE_AUTHENTICATION_V1_SCHEMA
            && self.schema != GENERATED_SOURCE_AUTHENTICATION_V2_SCHEMA
        {
            return Err(GeneratedWhenBadError::SchemaMismatch);
        }
        let replayed = GeneratedSourceAuthenticator::authenticate_with_replayed_row_span(
            family,
            context,
            candidate,
            row_span,
            self.limits,
        )?;
        if self.payload_eq(&replayed) {
            Ok(())
        } else {
            Err(GeneratedWhenBadError::ReplayMismatch)
        }
    }

    fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.source_authentication == other.source_authentication
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.candidate_source_manifest == other.candidate_source_manifest
            && self.row_span.payload_eq(&other.row_span)
            && self.witnesses == other.witnesses
            && self.stats == other.stats
            && self.limits == other.limits
    }
}

impl PartialEq for GeneratedSourceAuthenticationCertificate {
    fn eq(&self, other: &Self) -> bool {
        self.payload_eq(other)
    }
}

impl Eq for GeneratedSourceAuthenticationCertificate {}

pub struct GeneratedSourceAuthenticator;

impl GeneratedSourceAuthenticator {
    pub fn authenticate(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        candidate: &ParametricReductionRuleCandidate,
        limits: GeneratedWhenBadLimits,
    ) -> Result<GeneratedSourceAuthenticationCertificate, GeneratedWhenBadError> {
        let row_span = Arc::new(GeneratedSymbolicRowSpanCompiler::compile(
            family,
            context,
            limits.ibp,
            limits.row_span,
        )?);
        Self::authenticate_with_replayed_row_span(family, context, candidate, row_span, limits)
    }

    /// Authenticate against canonical generated rows plus explicitly supplied
    /// proof-carrying whole-row symmetry transports.  Each supplied symmetry
    /// is replayed against its exact owned restrictions before use.
    pub fn authenticate_with_verified_symmetries(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        candidate: &ParametricReductionRuleCandidate,
        symmetries: &[VerifiedInternalFamilyPermutationSymmetry],
        mut limits: GeneratedWhenBadLimits,
    ) -> Result<GeneratedSourceAuthenticationCertificate, GeneratedWhenBadError> {
        limits.row_span.strategy = GeneratedSymbolicRowSpanStrategy::VerifiedInputs;
        let row_span = Arc::new(
            GeneratedSymbolicRowSpanCompiler::compile_with_verified_symmetries(
                family,
                context,
                limits.ibp,
                symmetries,
                limits.row_span.limits,
            )?,
        );
        Self::authenticate_with_replayed_row_span(family, context, candidate, row_span, limits)
    }

    /// Authenticate one candidate against an immutable, caller-supplied row
    /// span.  The row span is replayed once before use; callers authenticating
    /// a batch should use the coverage compiler, which performs that replay
    /// once for the whole batch.
    pub fn authenticate_with_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        candidate: &ParametricReductionRuleCandidate,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: GeneratedWhenBadLimits,
    ) -> Result<GeneratedSourceAuthenticationCertificate, GeneratedWhenBadError> {
        row_span.replay(family, context)?;
        Self::authenticate_with_replayed_row_span(family, context, candidate, row_span, limits)
    }

    pub(crate) fn authenticate_with_replayed_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        candidate: &ParametricReductionRuleCandidate,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: GeneratedWhenBadLimits,
    ) -> Result<GeneratedSourceAuthenticationCertificate, GeneratedWhenBadError> {
        if !limits.row_span.strategy.is_disabled()
            && limits.row_span.limits.transport.arithmetic != limits.ibp.arithmetic_limits
        {
            return Err(GeneratedWhenBadError::IncoherentLimits {
                detail: "IBP generation and whole-row symmetry transport arithmetic policies differ",
            });
        }
        if candidate.family_fingerprint() != family.fingerprint() {
            return Err(GeneratedWhenBadError::WrongFamily);
        }
        if candidate.context_fingerprint() != context.fingerprint() {
            return Err(GeneratedWhenBadError::WrongContext);
        }
        let retained = candidate.derivation().source_rows();
        check_limit(
            "retained parametric source rows",
            retained.len(),
            limits.max_retained_rows,
        )?;
        let retained_terms = aggregate_terms(
            "retained parametric source terms",
            retained.iter(),
            limits.max_retained_terms,
        )?;
        check_limit(
            "candidate source manifest bytes",
            candidate.source_manifest().len(),
            limits.max_source_manifest_bytes,
        )?;
        let expected_canonical_rows = generated_row_count(family)?;
        check_limit(
            "canonical generated IBP/LI rows",
            expected_canonical_rows,
            limits.max_canonical_rows,
        )?;

        // Every accepted source row carries one arity-sized translation
        // witness, including the all-zero witness for an original row. Bound
        // the aggregate transcript before any translation replay allocates.
        let translation_components = retained.len().checked_mul(context.index_count()).ok_or(
            GeneratedWhenBadError::ResourceCountOverflow {
                resource: "generated-source translation components",
            },
        )?;
        check_limit(
            "generated-source translation components",
            translation_components,
            limits.max_translation_components,
        )?;
        // Every retained row must be compared with at least the first basis
        // row.  Enforce that unavoidable lower bound before replaying a
        // potentially large elimination or rebuilding the generated span.
        check_limit(
            "generated-source match attempts",
            if expected_canonical_rows == 0 {
                0
            } else {
                retained.len()
            },
            limits.max_match_attempts,
        )?;
        // Cheap caller-controlled bounds have now been checked. Replaying the
        // retained elimination can be substantial, so it intentionally comes
        // after those preflights.
        candidate.replay_retained(context)?;

        validate_shared_row_span(family, context, &row_span, limits)?;
        let basis = row_span.rows();
        let row_span_stats = row_span.stats();
        if row_span_stats.canonical_rows() != expected_canonical_rows {
            return Err(GeneratedWhenBadError::GeneratedRowCountMismatch {
                expected: expected_canonical_rows,
                actual: row_span_stats.canonical_rows(),
            });
        }
        check_limit(
            "canonical generated IBP/LI terms",
            row_span_stats.canonical_terms(),
            limits.max_canonical_terms,
        )?;

        let mut attempts = 0usize;
        let mut original_rows = 0usize;
        let mut translated_rows = 0usize;
        let mut transported_rows = 0usize;
        let mut translated_transported_rows = 0usize;
        let mut symmetry_witness_components = 0usize;
        let mut witnesses = Vec::with_capacity(retained.len());
        for (retained_ordinal, actual) in retained.iter().enumerate() {
            let mut matched = None;
            for (basis_ordinal, source) in basis.iter().enumerate() {
                attempts = checked_add("generated-source match attempts", attempts, 1)?;
                check_limit(
                    "generated-source match attempts",
                    attempts,
                    limits.max_match_attempts,
                )?;
                let Some((basis_mode, translation)) =
                    match_generated_row(context, actual, source, limits.ibp)?
                else {
                    continue;
                };
                let lineage = &row_span.lineages()[basis_ordinal];
                let canonical_ordinal = lineage.canonical_ordinal();
                let mode = match (lineage, basis_mode) {
                    (
                        GeneratedSymbolicRowSpanLineage::Canonical { .. },
                        GeneratedSourceRowMode::CanonicalOriginal,
                    ) => GeneratedSourceRowMode::CanonicalOriginal,
                    (
                        GeneratedSymbolicRowSpanLineage::Canonical { .. },
                        GeneratedSourceRowMode::ExactTranslation,
                    ) => GeneratedSourceRowMode::ExactTranslation,
                    (
                        GeneratedSymbolicRowSpanLineage::VerifiedWholeRowSymmetryTransport {
                            ..
                        },
                        GeneratedSourceRowMode::CanonicalOriginal,
                    ) => GeneratedSourceRowMode::VerifiedWholeRowSymmetryTransport,
                    (
                        GeneratedSymbolicRowSpanLineage::VerifiedWholeRowSymmetryTransport {
                            ..
                        },
                        GeneratedSourceRowMode::ExactTranslation,
                    ) => {
                        GeneratedSourceRowMode::ExactTranslationOfVerifiedWholeRowSymmetryTransport
                    }
                    _ => return Err(GeneratedWhenBadError::ReplayMismatch),
                };
                match mode {
                    GeneratedSourceRowMode::CanonicalOriginal => {
                        original_rows = checked_add("canonical original rows", original_rows, 1)?;
                    }
                    GeneratedSourceRowMode::ExactTranslation => {
                        translated_rows = checked_add("exact translated rows", translated_rows, 1)?;
                    }
                    GeneratedSourceRowMode::VerifiedWholeRowSymmetryTransport => {
                        transported_rows = checked_add(
                            "verified whole-row symmetry transports",
                            transported_rows,
                            1,
                        )?;
                    }
                    GeneratedSourceRowMode::ExactTranslationOfVerifiedWholeRowSymmetryTransport => {
                        translated_transported_rows = checked_add(
                            "exact translations of verified whole-row symmetry transports",
                            translated_transported_rows,
                            1,
                        )?;
                    }
                }
                let symmetry_permutation = lineage.symmetry_permutation().unwrap_or(&[]);
                if lineage.symmetry_ordinal().is_some() {
                    symmetry_witness_components = checked_add(
                        "generated-source symmetry witness components",
                        symmetry_witness_components,
                        symmetry_permutation.len(),
                    )?;
                    check_limit(
                        "generated-source symmetry witness components",
                        symmetry_witness_components,
                        limits.max_symmetry_witness_components,
                    )?;
                }
                matched = Some(GeneratedSourceRowWitness {
                    retained_ordinal,
                    basis_ordinal,
                    canonical_ordinal,
                    symmetry_ordinal: lineage.symmetry_ordinal(),
                    symmetry_permutation: symmetry_permutation.to_vec().into_boxed_slice(),
                    mode,
                    translation,
                    retained_row_id: actual.row_id().clone(),
                    basis_row_id: source.row_id().clone(),
                    canonical_row_id: basis[canonical_ordinal].row_id().clone(),
                });
                break;
            }
            let Some(witness) = matched else {
                return Err(GeneratedWhenBadError::UnauthenticatedRetainedSourceRow {
                    retained_ordinal,
                });
            };
            witnesses.push(witness);
        }

        let stats = GeneratedSourceAuthenticationStats {
            canonical_rows: row_span_stats.canonical_rows(),
            authenticated_basis_rows: row_span_stats.augmented_rows(),
            verified_symmetries: row_span_stats.verified_symmetries(),
            transported_basis_rows: row_span_stats.retained_transports(),
            retained_rows: retained.len(),
            match_attempts: attempts,
            original_rows,
            translated_rows,
            transported_rows,
            translated_transported_rows,
            translation_components,
            symmetry_witness_components,
            canonical_terms: row_span_stats.canonical_terms(),
            retained_terms,
            source_manifest_bytes: candidate.source_manifest().len(),
        };
        let augmented = !limits.row_span.strategy.is_disabled();
        Ok(GeneratedSourceAuthenticationCertificate {
            schema: if augmented {
                GENERATED_SOURCE_AUTHENTICATION_V2_SCHEMA
            } else {
                GENERATED_SOURCE_AUTHENTICATION_V1_SCHEMA
            },
            source_authentication: if augmented {
                GeneratedWhenBadSourceAuthentication::CanonicalIbpLiExactTranslationsAndVerifiedWholeRowSymmetryTransports
            } else {
                GeneratedWhenBadSourceAuthentication::CanonicalIbpLiAndExactTranslations
            },
            family_fingerprint: family.fingerprint().into(),
            context_fingerprint: context.fingerprint().into(),
            candidate_source_manifest: candidate.source_manifest().into(),
            row_span,
            witnesses: witnesses.into_boxed_slice(),
            stats,
            limits,
        })
    }
}

#[derive(Clone, Debug)]
pub struct GeneratedWhenBadCertificate {
    schema: &'static str,
    source: GeneratedSourceAuthenticationCertificate,
    admissibility: WhenBadCertificate,
}

impl GeneratedWhenBadCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub const fn source_authentication(&self) -> &GeneratedSourceAuthenticationCertificate {
        &self.source
    }
    pub const fn admissibility(&self) -> &WhenBadCertificate {
        &self.admissibility
    }
    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedWhenBadError> {
        if self.schema != GENERATED_WHEN_BAD_V1_SCHEMA
            && self.schema != GENERATED_WHEN_BAD_V2_SCHEMA
        {
            return Err(GeneratedWhenBadError::SchemaMismatch);
        }
        if self.schema != generated_when_bad_schema_for_source(&self.source) {
            return Err(GeneratedWhenBadError::ReplayMismatch);
        }
        self.source
            .replay(family, context, self.admissibility.candidate())?;
        self.admissibility.replay(context)?;
        Ok(())
    }

    pub fn replay_with_row_span(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    ) -> Result<(), GeneratedWhenBadError> {
        row_span.replay(family, context)?;
        self.replay_with_replayed_row_span(family, context, row_span)
    }

    pub(crate) fn replay_with_replayed_row_span(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    ) -> Result<(), GeneratedWhenBadError> {
        if self.schema != GENERATED_WHEN_BAD_V1_SCHEMA
            && self.schema != GENERATED_WHEN_BAD_V2_SCHEMA
        {
            return Err(GeneratedWhenBadError::SchemaMismatch);
        }
        if self.schema != generated_when_bad_schema_for_source(&self.source) {
            return Err(GeneratedWhenBadError::ReplayMismatch);
        }
        self.source.replay_with_replayed_row_span(
            family,
            context,
            self.admissibility.candidate(),
            row_span,
        )?;
        self.admissibility.replay(context)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct GeneratedWhenBadUnsupported {
    schema: &'static str,
    candidate: Arc<ParametricReductionRuleCandidate>,
    source: GeneratedSourceAuthenticationCertificate,
    admissibility: WhenBadUnsupported,
}

impl GeneratedWhenBadUnsupported {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn candidate(&self) -> &ParametricReductionRuleCandidate {
        &self.candidate
    }
    pub const fn source_authentication(&self) -> &GeneratedSourceAuthenticationCertificate {
        &self.source
    }
    pub const fn admissibility(&self) -> &WhenBadUnsupported {
        &self.admissibility
    }
    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedWhenBadError> {
        if self.schema != GENERATED_WHEN_BAD_V1_SCHEMA
            && self.schema != GENERATED_WHEN_BAD_V2_SCHEMA
        {
            return Err(GeneratedWhenBadError::SchemaMismatch);
        }
        if self.schema != generated_when_bad_schema_for_source(&self.source) {
            return Err(GeneratedWhenBadError::ReplayMismatch);
        }
        self.source.replay(family, context, &self.candidate)?;
        verify_cross_binding(&self.candidate, self.admissibility.binding())?;
        self.admissibility.replay(context)?;
        Ok(())
    }

    pub fn replay_with_row_span(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    ) -> Result<(), GeneratedWhenBadError> {
        row_span.replay(family, context)?;
        self.replay_with_replayed_row_span(family, context, row_span)
    }

    pub(crate) fn replay_with_replayed_row_span(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    ) -> Result<(), GeneratedWhenBadError> {
        if self.schema != GENERATED_WHEN_BAD_V1_SCHEMA
            && self.schema != GENERATED_WHEN_BAD_V2_SCHEMA
        {
            return Err(GeneratedWhenBadError::SchemaMismatch);
        }
        if self.schema != generated_when_bad_schema_for_source(&self.source) {
            return Err(GeneratedWhenBadError::ReplayMismatch);
        }
        self.source
            .replay_with_replayed_row_span(family, context, &self.candidate, row_span)?;
        verify_cross_binding(&self.candidate, self.admissibility.binding())?;
        self.admissibility.replay(context)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum GeneratedWhenBadCompilation {
    Certified(GeneratedWhenBadCertificate),
    Unsupported(GeneratedWhenBadUnsupported),
}

impl GeneratedWhenBadCompilation {
    pub fn source_authentication(&self) -> &GeneratedSourceAuthenticationCertificate {
        match self {
            Self::Certified(certificate) => certificate.source_authentication(),
            Self::Unsupported(unsupported) => unsupported.source_authentication(),
        }
    }

    pub fn candidate(&self) -> &ParametricReductionRuleCandidate {
        match self {
            Self::Certified(certificate) => certificate.admissibility().candidate(),
            Self::Unsupported(unsupported) => unsupported.candidate(),
        }
    }

    /// Compare the complete externally meaningful proof payload.
    ///
    /// This is crate-visible for enclosing replay transcripts that regenerate
    /// an exact adaptive-search locator.  It deliberately compares more than
    /// the candidate's pivot ordinal or display form: generated-source
    /// authentication, the full `WhenBad` partition, classifications, and an
    /// unsupported reason are all part of the identity.
    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Certified(left), Self::Certified(right)) => {
                let left_admissibility = left.admissibility();
                let right_admissibility = right.admissibility();
                left.schema() == right.schema()
                    && left.source_authentication() == right.source_authentication()
                    && left_admissibility.schema() == right_admissibility.schema()
                    && left_admissibility.binding() == right_admissibility.binding()
                    && left_admissibility.domain_conditions()
                        == right_admissibility.domain_conditions()
                    && left_admissibility
                        .base_domain_guards()
                        .eq(right_admissibility.base_domain_guards())
                    && left_admissibility
                        .index_domain_guards()
                        .eq(right_admissibility.index_domain_guards())
                    && left_admissibility.leak_events() == right_admissibility.leak_events()
                    && left_admissibility.descent_witnesses()
                        == right_admissibility.descent_witnesses()
                    && left_admissibility.partition() == right_admissibility.partition()
                    && left_admissibility.classifications() == right_admissibility.classifications()
                    && left_admissibility.stats() == right_admissibility.stats()
            }
            (Self::Unsupported(left), Self::Unsupported(right)) => {
                left.schema() == right.schema()
                    && left.source_authentication() == right.source_authentication()
                    && left.admissibility().binding() == right.admissibility().binding()
                    && left.admissibility().reason() == right.admissibility().reason()
            }
            _ => false,
        }
    }

    /// Replay the complete generated-source and admissibility payload without
    /// exposing which supported/unsupported variant owns it.
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedWhenBadError> {
        match self {
            Self::Certified(certificate) => certificate.replay(family, context),
            Self::Unsupported(unsupported) => unsupported.replay(family, context),
        }
    }

    pub(crate) fn replay_with_replayed_row_span(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    ) -> Result<(), GeneratedWhenBadError> {
        match self {
            Self::Certified(certificate) => {
                certificate.replay_with_replayed_row_span(family, context, row_span)
            }
            Self::Unsupported(unsupported) => {
                unsupported.replay_with_replayed_row_span(family, context, row_span)
            }
        }
    }
}

pub struct GeneratedWhenBadCompiler;

impl GeneratedWhenBadCompiler {
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        candidate: &ParametricReductionRuleCandidate,
        limits: GeneratedWhenBadLimits,
    ) -> Result<GeneratedWhenBadCompilation, GeneratedWhenBadError> {
        let source =
            GeneratedSourceAuthenticator::authenticate(family, context, candidate, limits)?;
        Self::compile_authenticated(context, candidate, source, limits)
    }

    pub fn compile_with_verified_symmetries(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        candidate: &ParametricReductionRuleCandidate,
        symmetries: &[VerifiedInternalFamilyPermutationSymmetry],
        mut limits: GeneratedWhenBadLimits,
    ) -> Result<GeneratedWhenBadCompilation, GeneratedWhenBadError> {
        limits.row_span.strategy = GeneratedSymbolicRowSpanStrategy::VerifiedInputs;
        let source = GeneratedSourceAuthenticator::authenticate_with_verified_symmetries(
            family, context, candidate, symmetries, limits,
        )?;
        Self::compile_authenticated(context, candidate, source, limits)
    }

    /// Compile one candidate against a shared immutable generated row span.
    /// The supplied certificate is replayed exactly once by this entry point.
    pub fn compile_with_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        candidate: &ParametricReductionRuleCandidate,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: GeneratedWhenBadLimits,
    ) -> Result<GeneratedWhenBadCompilation, GeneratedWhenBadError> {
        row_span.replay(family, context)?;
        Self::compile_with_replayed_row_span(family, context, candidate, row_span, limits)
    }

    pub(crate) fn compile_with_replayed_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        candidate: &ParametricReductionRuleCandidate,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: GeneratedWhenBadLimits,
    ) -> Result<GeneratedWhenBadCompilation, GeneratedWhenBadError> {
        let source = GeneratedSourceAuthenticator::authenticate_with_replayed_row_span(
            family, context, candidate, row_span, limits,
        )?;
        Self::compile_authenticated(context, candidate, source, limits)
    }

    fn compile_authenticated(
        context: &ParametricCoefficientContext,
        candidate: &ParametricReductionRuleCandidate,
        source: GeneratedSourceAuthenticationCertificate,
        limits: GeneratedWhenBadLimits,
    ) -> Result<GeneratedWhenBadCompilation, GeneratedWhenBadError> {
        let schema = generated_when_bad_schema_for_source(&source);
        let candidate = Arc::new(candidate.clone());
        Ok(
            match WhenBadCompiler::compile_algebraic_candidate(
                context,
                &candidate,
                limits.when_bad,
            )? {
                WhenBadCompilation::Certified(admissibility) => {
                    GeneratedWhenBadCompilation::Certified(GeneratedWhenBadCertificate {
                        schema,
                        source,
                        admissibility,
                    })
                }
                WhenBadCompilation::Unsupported(admissibility) => {
                    GeneratedWhenBadCompilation::Unsupported(GeneratedWhenBadUnsupported {
                        schema,
                        candidate,
                        source,
                        admissibility,
                    })
                }
            },
        )
    }
}

fn generated_when_bad_schema_for_source(
    source: &GeneratedSourceAuthenticationCertificate,
) -> &'static str {
    if source.schema() == GENERATED_SOURCE_AUTHENTICATION_V2_SCHEMA {
        GENERATED_WHEN_BAD_V2_SCHEMA
    } else {
        GENERATED_WHEN_BAD_V1_SCHEMA
    }
}

fn match_generated_row(
    context: &ParametricCoefficientContext,
    actual: &ParametricRelation,
    canonical: &ParametricRelation,
    config: ParametricIbpConfig,
) -> Result<Option<(GeneratedSourceRowMode, IndexShift)>, GeneratedWhenBadError> {
    let zero = IndexSpace::try_new(context.index_count())?.zero();
    if actual.has_identical_guard_provenance(canonical) {
        return Ok(Some((GeneratedSourceRowMode::CanonicalOriginal, zero)));
    }
    if actual.terms().len() != canonical.terms().len() {
        return Ok(None);
    }

    // A nonempty exact support fixes the translation uniquely.  For an empty
    // generated identity only offset zero is provable from row content; this
    // conservative case never invents an unconstrained translation witness.
    let translation = match (
        actual.terms().keys().next(),
        canonical.terms().keys().next(),
    ) {
        (Some(actual_first), Some(canonical_first)) => {
            let values = actual_first
                .values()
                .iter()
                .zip(canonical_first.values())
                .map(|(&actual, &canonical)| actual.checked_sub(canonical))
                .collect::<Option<Vec<_>>>();
            let Some(values) = values else {
                return Ok(None);
            };
            IndexShift::try_new(values, context.index_count())?
        }
        (None, None) => zero,
        _ => return Ok(None),
    };

    // Compare shifted supports before invoking coefficient substitution.  A
    // mismatched canonical row therefore cannot consume exact-algebra work.
    for (canonical_shift, actual_shift) in canonical.terms().keys().zip(actual.terms().keys()) {
        match canonical_shift.checked_add(&translation) {
            Ok(shifted) if &shifted == actual_shift => {}
            _ => return Ok(None),
        }
    }

    let replayed = canonical.translated(
        context,
        &translation,
        actual.row_id().clone(),
        config.arithmetic_limits,
    )?;
    Ok(replayed
        .has_identical_guard_provenance(actual)
        .then_some((GeneratedSourceRowMode::ExactTranslation, translation)))
}

fn aggregate_terms<'a>(
    resource: &'static str,
    rows: impl Iterator<Item = &'a ParametricRelation>,
    limit: usize,
) -> Result<usize, GeneratedWhenBadError> {
    let mut total = 0usize;
    for row in rows {
        total = checked_add(resource, total, row.terms().len())?;
        check_limit(resource, total, limit)?;
    }
    Ok(total)
}

fn generated_row_count(family: &IntegralFamily) -> Result<usize, GeneratedWhenBadError> {
    let loops = family.loop_count();
    let externals = family.external_count();
    let contractions =
        loops
            .checked_add(externals)
            .ok_or(GeneratedWhenBadError::ResourceCountOverflow {
                resource: "canonical generated IBP/LI rows",
            })?;
    let ordinary =
        loops
            .checked_mul(contractions)
            .ok_or(GeneratedWhenBadError::ResourceCountOverflow {
                resource: "canonical generated IBP/LI rows",
            })?;
    let li = externals
        .checked_mul(externals.saturating_sub(1))
        .and_then(|count| count.checked_div(2))
        .ok_or(GeneratedWhenBadError::ResourceCountOverflow {
            resource: "canonical generated IBP/LI rows",
        })?;
    checked_add("canonical generated IBP/LI rows", ordinary, li)
}

fn validate_shared_row_span(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    row_span: &GeneratedSymbolicRowSpanCertificate,
    limits: GeneratedWhenBadLimits,
) -> Result<(), GeneratedWhenBadError> {
    if row_span.family_fingerprint() != family.fingerprint() {
        return Err(GeneratedWhenBadError::WrongFamily);
    }
    if row_span.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedWhenBadError::WrongContext);
    }
    if row_span.ibp_config() != limits.ibp {
        return Err(GeneratedWhenBadError::SharedRowSpanIbpConfigMismatch);
    }
    if row_span.config() != limits.row_span {
        return Err(GeneratedWhenBadError::SharedRowSpanConfigMismatch);
    }
    Ok(())
}

fn verify_cross_binding(
    candidate: &ParametricReductionRuleCandidate,
    binding: &crate::WhenBadCandidateBinding,
) -> Result<(), GeneratedWhenBadError> {
    if candidate.family_fingerprint() == binding.family_fingerprint()
        && candidate.context_fingerprint() == binding.context_fingerprint()
        && candidate.source_manifest() == binding.source_manifest()
        && candidate.pivot_ordinal() == binding.pivot_ordinal()
        && candidate.sector() == binding.sector()
        && candidate.ordering().stable_string() == binding.ordering()
    {
        Ok(())
    } else {
        Err(GeneratedWhenBadError::CrossBindingMismatch)
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedWhenBadError> {
    left.checked_add(right)
        .ok_or(GeneratedWhenBadError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedWhenBadError> {
    if requested > limit {
        Err(GeneratedWhenBadError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedWhenBadError {
    WrongFamily,
    WrongContext,
    SchemaMismatch,
    ReplayMismatch,
    CrossBindingMismatch,
    SharedRowSpanIbpConfigMismatch,
    SharedRowSpanConfigMismatch,
    GeneratedRowCountMismatch {
        expected: usize,
        actual: usize,
    },
    UnauthenticatedRetainedSourceRow {
        retained_ordinal: usize,
    },
    IncoherentLimits {
        detail: &'static str,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Ibp(ParametricIbpError),
    RowSpan(GeneratedSymbolicRowSpanError),
    Relation(ParametricRelationError),
    Rule(crate::ParametricRuleError),
    WhenBad(WhenBadCompilerError),
}

impl fmt::Display for GeneratedWhenBadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFamily => formatter
                .write_str("parametric candidate does not belong to the supplied integral family"),
            Self::WrongContext => formatter
                .write_str("parametric candidate does not use the supplied coefficient context"),
            Self::SchemaMismatch => {
                formatter.write_str("generated WhenBad certificate schema mismatch")
            }
            Self::ReplayMismatch => {
                formatter.write_str("generated-source authentication replay mismatch")
            }
            Self::CrossBindingMismatch => formatter.write_str(
                "generated-source and symbolic admissibility certificates bind different candidates",
            ),
            Self::SharedRowSpanIbpConfigMismatch => formatter.write_str(
                "shared generated row span uses a different IBP configuration",
            ),
            Self::SharedRowSpanConfigMismatch => formatter.write_str(
                "shared generated row span uses a different symmetry/configuration policy",
            ),
            Self::GeneratedRowCountMismatch { expected, actual } => write!(
                formatter,
                "generated IBP/LI row count is {actual}, expected preflighted count {expected}"
            ),
            Self::UnauthenticatedRetainedSourceRow { retained_ordinal } => write!(
                formatter,
                "retained elimination source row {retained_ordinal} is not a canonical generated IBP/LI row, a verified whole-row symmetry transport, or an exact translation of either"
            ),
            Self::IncoherentLimits { detail } => {
                write!(formatter, "incoherent generated-source limits: {detail}")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested {requested}, configured limit is {limit}"
            ),
            Self::Ibp(error) => error.fmt(formatter),
            Self::RowSpan(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Rule(error) => error.fmt(formatter),
            Self::WhenBad(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedWhenBadError {}

impl From<ParametricIbpError> for GeneratedWhenBadError {
    fn from(value: ParametricIbpError) -> Self {
        Self::Ibp(value)
    }
}

impl From<GeneratedSymbolicRowSpanError> for GeneratedWhenBadError {
    fn from(value: GeneratedSymbolicRowSpanError) -> Self {
        Self::RowSpan(value)
    }
}

impl From<ParametricRelationError> for GeneratedWhenBadError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

impl From<crate::ParametricRuleError> for GeneratedWhenBadError {
    fn from(value: crate::ParametricRuleError) -> Self {
        Self::Rule(value)
    }
}

impl From<WhenBadCompilerError> for GeneratedWhenBadError {
    fn from(value: WhenBadCompilerError) -> Self {
        Self::WhenBad(value)
    }
}
