//! Generated IBP/LI re-elimination on a sparse integer equality locus.
//!
//! LiteRed revisits exceptional `WhenBad` loci by substituting the equality
//! defining the locus into freshly generated equations and eliminating the
//! resulting conditional system again.  This module is the corresponding
//! provenance boundary for RustRed.  It accepts no caller-authored equations:
//! every source row is regenerated from [`crate::IntegralFamily`], translated
//! by an authenticated finite stencil, partially specialized, and only then
//! passed to exact [`crate::ParametricElimination`].
//!
//! The specialized rows and the elimination object deliberately remain
//! private.  In particular, this certificate is not an unconditioned
//! `K(n)` identity and cannot be converted into an ordinary parametric rule
//! candidate.  Public pivot witnesses expose only the equality locus on which
//! a subsequently centered pivot could be valid.

use std::fmt;
use std::fmt::Write as _;
use std::sync::Arc;

use crate::{
    IndexShift, IntegralFamily, ParametricArithmeticLimits, ParametricCoefficientContext,
    ParametricElimination, ParametricEliminationError, ParametricEliminationLimits,
    ParametricEliminationOrdering, ParametricEliminationStats, ParametricIbpConfig,
    ParametricIbpError, ParametricIbpGenerator, ParametricRelation, ParametricRelationError,
    ParametricRowId, PartialIndexAssignment, PartialParametricRelationSpecialization,
    PartialParametricRelationSpecializationLimits,
};

/// Legacy identifier for the Atom-rendered base-assumption transcript.
pub const GENERATED_PARTIAL_REELIMINATION_V1_SCHEMA: &str =
    "rustred-generated-partial-reelimination-v1";
/// Current typed-sparse, bounded transcript schema.
pub const GENERATED_PARTIAL_REELIMINATION_V2_SCHEMA: &str =
    "rustred-generated-partial-reelimination-v2";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedPartialSourceAuthentication {
    CanonicalIbpLiExactTranslationsAndSparseSpecialization,
}

/// Aggregate and per-operation proof budgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedPartialReeliminationLimits {
    pub ibp: ParametricIbpConfig,
    pub specialization: PartialParametricRelationSpecializationLimits,
    pub elimination: ParametricEliminationLimits,
    pub max_translations: usize,
    pub max_translation_components: usize,
    pub max_canonical_rows: usize,
    pub max_expanded_rows: usize,
    pub max_canonical_terms: usize,
    pub max_translated_terms: usize,
    pub max_retained_rows: usize,
    pub max_specialized_guards: usize,
    pub max_specialized_guard_origins: usize,
    pub max_base_assumptions: usize,
    pub max_base_assumption_origins: usize,
    pub max_aggregate_source_terms: usize,
    pub max_aggregate_output_terms: usize,
    pub max_aggregate_power_operations: usize,
    pub max_aggregate_integer_bit_work: usize,
    pub max_aggregate_retained_terms: usize,
    pub max_aggregate_retained_bytes: usize,
    pub max_transcript_bytes: usize,
}

impl Default for GeneratedPartialReeliminationLimits {
    fn default() -> Self {
        Self {
            ibp: ParametricIbpConfig::default(),
            specialization: PartialParametricRelationSpecializationLimits::default(),
            elimination: ParametricEliminationLimits::default(),
            max_translations: 100_000,
            max_translation_components: 10_000_000,
            max_canonical_rows: 100_000,
            max_expanded_rows: 10_000_000,
            max_canonical_terms: 16_000_000,
            max_translated_terms: 1_000_000_000,
            max_retained_rows: 10_000_000,
            max_specialized_guards: 100_000_000,
            max_specialized_guard_origins: 1_000_000_000,
            max_base_assumptions: 100_000_000,
            max_base_assumption_origins: 1_000_000_000,
            max_aggregate_source_terms: 1_000_000_000,
            max_aggregate_output_terms: 1_000_000_000,
            max_aggregate_power_operations: 4_000_000_000,
            max_aggregate_integer_bit_work: 4_000_000_000,
            max_aggregate_retained_terms: 1_000_000_000,
            max_aggregate_retained_bytes: 8 * 1024 * 1024 * 1024,
            max_transcript_bytes: 8 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedPartialReeliminationStats {
    canonical_rows: usize,
    translations: usize,
    translation_components: usize,
    expanded_rows: usize,
    retained_rows: usize,
    unsatisfiable_rows: usize,
    canonical_terms: usize,
    translated_terms: usize,
    specialized_guards: usize,
    specialized_guard_origins: usize,
    base_assumptions: usize,
    base_assumption_origins: usize,
    aggregate_source_terms: usize,
    aggregate_output_terms: usize,
    aggregate_power_operations: usize,
    aggregate_integer_bit_work: usize,
    aggregate_retained_terms: usize,
    aggregate_retained_bytes: usize,
    transcript_bytes: usize,
}

impl GeneratedPartialReeliminationStats {
    pub const fn canonical_rows(self) -> usize {
        self.canonical_rows
    }
    pub const fn translations(self) -> usize {
        self.translations
    }
    pub const fn translation_components(self) -> usize {
        self.translation_components
    }
    pub const fn expanded_rows(self) -> usize {
        self.expanded_rows
    }
    pub const fn retained_rows(self) -> usize {
        self.retained_rows
    }
    pub const fn unsatisfiable_rows(self) -> usize {
        self.unsatisfiable_rows
    }
    pub const fn canonical_terms(self) -> usize {
        self.canonical_terms
    }
    pub const fn translated_terms(self) -> usize {
        self.translated_terms
    }
    pub const fn specialized_guards(self) -> usize {
        self.specialized_guards
    }
    pub const fn specialized_guard_origins(self) -> usize {
        self.specialized_guard_origins
    }
    pub const fn base_assumptions(self) -> usize {
        self.base_assumptions
    }
    pub const fn base_assumption_origins(self) -> usize {
        self.base_assumption_origins
    }
    pub const fn aggregate_source_terms(self) -> usize {
        self.aggregate_source_terms
    }
    pub const fn aggregate_output_terms(self) -> usize {
        self.aggregate_output_terms
    }
    pub const fn aggregate_power_operations(self) -> usize {
        self.aggregate_power_operations
    }
    pub const fn aggregate_integer_bit_work(self) -> usize {
        self.aggregate_integer_bit_work
    }
    pub const fn aggregate_retained_terms(self) -> usize {
        self.aggregate_retained_terms
    }
    pub const fn aggregate_retained_bytes(self) -> usize {
        self.aggregate_retained_bytes
    }
    pub const fn transcript_bytes(self) -> usize {
        self.transcript_bytes
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedPartialSourceRowOutcome {
    Retained {
        specialized_manifest: Arc<String>,
        base_assumptions: usize,
    },
    UnsatisfiableDomain,
}

/// Public, immutable provenance for one row in the generated stencil.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedPartialSourceRowWitness {
    expanded_ordinal: usize,
    canonical_ordinal: usize,
    canonical_row_id: ParametricRowId,
    translation: IndexShift,
    canonical_manifest: Arc<String>,
    translated_manifest: Arc<String>,
    outcome: GeneratedPartialSourceRowOutcome,
}

impl GeneratedPartialSourceRowWitness {
    pub const fn expanded_ordinal(&self) -> usize {
        self.expanded_ordinal
    }
    pub const fn canonical_ordinal(&self) -> usize {
        self.canonical_ordinal
    }
    pub const fn canonical_row_id(&self) -> &ParametricRowId {
        &self.canonical_row_id
    }
    pub const fn translation(&self) -> &IndexShift {
        &self.translation
    }
    pub fn canonical_manifest(&self) -> &str {
        self.canonical_manifest.as_str()
    }
    pub fn translated_manifest(&self) -> &str {
        self.translated_manifest.as_str()
    }
    pub const fn outcome(&self) -> &GeneratedPartialSourceRowOutcome {
        &self.outcome
    }
}

/// A base-field nonzero assumption carried by one retained conditional row.
///
/// The complete polynomial and all origins are encoded with typed,
/// length-prefixed sparse payloads.  Polynomial variable ordinals are
/// lossless relative to the enclosing certificate's context fingerprint; the
/// manifest intentionally does not repeat context-owned Symbolica names.  The
/// exact condition below remains the mathematical proof payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedPartialBaseAssumptionWitness {
    expanded_row_ordinal: usize,
    assumption_ordinal: usize,
    condition: crate::ParametricNonZeroCondition,
    manifest: Arc<String>,
    origin_count: usize,
}

impl GeneratedPartialBaseAssumptionWitness {
    pub const fn expanded_row_ordinal(&self) -> usize {
        self.expanded_row_ordinal
    }
    pub const fn assumption_ordinal(&self) -> usize {
        self.assumption_ordinal
    }
    /// Exact base-field polynomial and complete typed origin set retained by
    /// partial specialization.  The canonical manifest is context-relative
    /// serialization metadata; it is not the mathematical proof payload.
    pub const fn condition(&self) -> &crate::ParametricNonZeroCondition {
        &self.condition
    }
    pub fn manifest(&self) -> &str {
        self.manifest.as_str()
    }
    pub const fn origin_count(&self) -> usize {
        self.origin_count
    }
}

/// Equality locus of one normalized pivot after centering its pivot at zero.
///
/// `ParametricPivotEquation::centered_relation` translates by `-pivot`, so
/// `n_source = n_center - pivot`.  Therefore a source equality
/// `n_source[position] = a` becomes
/// `n_center[position] = a + pivot[position]`.  The checked sum below is the
/// only locus claimed by this witness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConditionalCenteredPivotLocus {
    pivot_ordinal: usize,
    original_pivot: IndexShift,
    centered_assignment: PartialIndexAssignment,
}

impl ConditionalCenteredPivotLocus {
    pub const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }
    pub const fn original_pivot(&self) -> &IndexShift {
        &self.original_pivot
    }
    pub const fn centered_assignment(&self) -> &PartialIndexAssignment {
        &self.centered_assignment
    }
}

#[derive(Clone)]
struct RetainedConditionalRow {
    specialization: Arc<PartialParametricRelationSpecialization>,
}

/// Successful exact re-elimination of at least one retained conditional row.
#[derive(Clone)]
pub struct GeneratedPartialReeliminationCertificate {
    schema: &'static str,
    authentication: GeneratedPartialSourceAuthentication,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    assignment: PartialIndexAssignment,
    translations: Vec<IndexShift>,
    ordering: ParametricEliminationOrdering,
    witnesses: Vec<GeneratedPartialSourceRowWitness>,
    base_assumptions: Vec<GeneratedPartialBaseAssumptionWitness>,
    centered_pivot_loci: Vec<ConditionalCenteredPivotLocus>,
    retained_rows: Vec<RetainedConditionalRow>,
    elimination: ParametricElimination,
    limits: GeneratedPartialReeliminationLimits,
    stats: GeneratedPartialReeliminationStats,
}

// Deliberately omit the crate-private conditional rows and elimination
// object.  `Debug` must not become a back door around the proof boundary.
impl fmt::Debug for GeneratedPartialReeliminationCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedPartialReeliminationCertificate")
            .field("schema", &self.schema)
            .field("authentication", &self.authentication)
            .field("family_fingerprint", &self.family_fingerprint)
            .field("context_fingerprint", &self.context_fingerprint)
            .field("assignment", &self.assignment)
            .field("translations", &self.translations)
            .field("ordering", &self.ordering)
            .field("witnesses", &self.witnesses)
            .field("base_assumptions", &self.base_assumptions)
            .field("centered_pivot_loci", &self.centered_pivot_loci)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl GeneratedPartialReeliminationCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub const fn authentication(&self) -> GeneratedPartialSourceAuthentication {
        self.authentication
    }
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }
    pub const fn assignment(&self) -> &PartialIndexAssignment {
        &self.assignment
    }
    pub fn translations(&self) -> &[IndexShift] {
        &self.translations
    }
    pub const fn ordering(&self) -> &ParametricEliminationOrdering {
        &self.ordering
    }
    pub fn witnesses(&self) -> &[GeneratedPartialSourceRowWitness] {
        &self.witnesses
    }
    pub fn base_assumptions(&self) -> &[GeneratedPartialBaseAssumptionWitness] {
        &self.base_assumptions
    }
    pub fn centered_pivot_loci(&self) -> &[ConditionalCenteredPivotLocus] {
        &self.centered_pivot_loci
    }
    pub const fn elimination_stats(&self) -> ParametricEliminationStats {
        self.elimination.stats()
    }
    pub const fn stats(&self) -> GeneratedPartialReeliminationStats {
        self.stats
    }

    /// Materialize one centered conditional pivot for another proof-bearing
    /// crate layer.  This deliberately stays crate-private: the returned row
    /// is valid only on the matching [`ConditionalCenteredPivotLocus`] and
    /// must never escape as an ordinary global parametric rule candidate.
    pub(crate) fn centered_pivot_relation_for_bound_rule(
        &self,
        context: &ParametricCoefficientContext,
        pivot_ordinal: usize,
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricRelation, GeneratedPartialReeliminationError> {
        if context.fingerprint() != self.context_fingerprint.as_ref() {
            return Err(GeneratedPartialReeliminationError::WrongContext);
        }
        let pivot = self.elimination.pivots().get(pivot_ordinal).ok_or(
            GeneratedPartialReeliminationError::PivotOutOfRange {
                pivot: pivot_ordinal,
                available: self.elimination.pivots().len(),
            },
        )?;
        Ok(pivot.centered_relation(context, limits)?)
    }

    /// Regenerate the complete source stencil, repeat sparse specialization
    /// and exact elimination, and compare the full retained transcript.
    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedPartialReeliminationError> {
        validate_replay_scope(
            self.schema,
            &self.family_fingerprint,
            &self.context_fingerprint,
            family,
            context,
        )?;
        let mut rows = Vec::new();
        try_reserve_conditional_entries(
            "conditional elimination replay rows",
            &mut rows,
            self.retained_rows.len(),
        )?;
        for row in &self.retained_rows {
            row.specialization.replay(context)?;
            // ParametricElimination::replay currently authenticates an owned
            // `[ParametricRelation]`; this is the remaining downstream clone
            // seam after the row-vector allocation has been made fallible.
            rows.push(
                row.specialization
                    .relation_for_bound_reelimination()
                    .clone(),
            );
        }
        self.elimination.replay(context, &rows)?;
        let replayed = GeneratedPartialReeliminationCompiler::compile(
            family,
            context,
            &self.translations,
            self.assignment.clone(),
            self.ordering.clone(),
            self.limits,
        )?;
        let GeneratedPartialReeliminationCompilation::Certified(replayed) = replayed else {
            return Err(GeneratedPartialReeliminationError::ReplayMismatch);
        };
        if certificate_payload_eq(self, &replayed) {
            Ok(())
        } else {
            Err(GeneratedPartialReeliminationError::ReplayMismatch)
        }
    }
}

/// Complete transcript for a locus on which every generated translated row
/// had an unsatisfiable inherited guard.  This is explicitly not a master or
/// a zero-sector conclusion.
#[derive(Clone, Debug)]
pub struct GeneratedPartialReeliminationEmptySystem {
    schema: &'static str,
    authentication: GeneratedPartialSourceAuthentication,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    assignment: PartialIndexAssignment,
    translations: Vec<IndexShift>,
    ordering: ParametricEliminationOrdering,
    witnesses: Vec<GeneratedPartialSourceRowWitness>,
    limits: GeneratedPartialReeliminationLimits,
    stats: GeneratedPartialReeliminationStats,
}

impl GeneratedPartialReeliminationEmptySystem {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub const fn authentication(&self) -> GeneratedPartialSourceAuthentication {
        self.authentication
    }
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }
    pub const fn assignment(&self) -> &PartialIndexAssignment {
        &self.assignment
    }
    pub fn translations(&self) -> &[IndexShift] {
        &self.translations
    }
    pub const fn ordering(&self) -> &ParametricEliminationOrdering {
        &self.ordering
    }
    pub fn witnesses(&self) -> &[GeneratedPartialSourceRowWitness] {
        &self.witnesses
    }
    pub const fn stats(&self) -> GeneratedPartialReeliminationStats {
        self.stats
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedPartialReeliminationError> {
        validate_replay_scope(
            self.schema,
            &self.family_fingerprint,
            &self.context_fingerprint,
            family,
            context,
        )?;
        let replayed = GeneratedPartialReeliminationCompiler::compile(
            family,
            context,
            &self.translations,
            self.assignment.clone(),
            self.ordering.clone(),
            self.limits,
        )?;
        let GeneratedPartialReeliminationCompilation::EmptySystem(replayed) = replayed else {
            return Err(GeneratedPartialReeliminationError::ReplayMismatch);
        };
        if self.schema == replayed.schema
            && self.authentication == replayed.authentication
            && self.family_fingerprint == replayed.family_fingerprint
            && self.context_fingerprint == replayed.context_fingerprint
            && self.assignment == replayed.assignment
            && self.translations == replayed.translations
            && self.ordering == replayed.ordering
            && self.witnesses == replayed.witnesses
            && self.stats == replayed.stats
        {
            Ok(())
        } else {
            Err(GeneratedPartialReeliminationError::ReplayMismatch)
        }
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.authentication == other.authentication
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.assignment == other.assignment
            && self.translations == other.translations
            && self.ordering == other.ordering
            && self.witnesses == other.witnesses
            && self.limits == other.limits
            && self.stats == other.stats
    }
}

#[derive(Clone, Debug)]
pub enum GeneratedPartialReeliminationCompilation {
    Certified(GeneratedPartialReeliminationCertificate),
    EmptySystem(GeneratedPartialReeliminationEmptySystem),
}

pub struct GeneratedPartialReeliminationCompiler;

impl GeneratedPartialReeliminationCompiler {
    #[allow(clippy::too_many_arguments)]
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        translations: &[IndexShift],
        assignment: PartialIndexAssignment,
        ordering: ParametricEliminationOrdering,
        limits: GeneratedPartialReeliminationLimits,
    ) -> Result<GeneratedPartialReeliminationCompilation, GeneratedPartialReeliminationError> {
        if !family
            .coefficient_context()
            .has_same_variable_map(context.base())
            || family.denominator_count() != context.index_count()
        {
            return Err(GeneratedPartialReeliminationError::WrongContext);
        }
        if assignment.arity() != context.index_count() {
            return Err(GeneratedPartialReeliminationError::WrongArity {
                expected: context.index_count(),
                actual: assignment.arity(),
            });
        }
        if ordering.anchor().len() != context.index_count() {
            return Err(GeneratedPartialReeliminationError::WrongArity {
                expected: context.index_count(),
                actual: ordering.anchor().len(),
            });
        }
        let (ordinary_count, li_count) = crate::parametric_ibp::checked_generated_row_counts(
            family.loop_count(),
            family.external_count(),
        )?;
        let canonical_count = checked_add("canonical generated rows", ordinary_count, li_count)?;
        check_limit(
            "canonical generated rows",
            canonical_count,
            limits.max_canonical_rows,
        )?;

        let translations = canonical_translations(translations, context.index_count(), limits)?;

        let generated =
            ParametricIbpGenerator::try_with_context(family, context.clone(), limits.ibp)?
                .generate()?;
        let canonical_terms = sum_counts(
            "canonical generated terms",
            generated.ibp_li().map(|row| row.terms().len()),
        )?;
        check_limit(
            "canonical generated terms",
            canonical_terms,
            limits.max_canonical_terms,
        )?;
        let expanded_count = canonical_count.checked_mul(translations.len()).ok_or(
            GeneratedPartialReeliminationError::ResourceCountOverflow {
                resource: "expanded generated rows",
            },
        )?;
        check_limit(
            "expanded generated rows",
            expanded_count,
            limits.max_expanded_rows,
        )?;

        let mut stats = GeneratedPartialReeliminationStats {
            canonical_rows: canonical_count,
            translations: translations.len(),
            translation_components: translations
                .len()
                .checked_mul(context.index_count())
                .ok_or(GeneratedPartialReeliminationError::ResourceCountOverflow {
                    resource: "translation components",
                })?,
            expanded_rows: expanded_count,
            canonical_terms,
            ..Default::default()
        };
        let mut witnesses = Vec::new();
        try_reserve_conditional_entries(
            "generated partial source witnesses",
            &mut witnesses,
            expanded_count,
        )?;
        let mut retained_rows = Vec::new();
        let mut base_assumptions = Vec::new();

        for (canonical_ordinal, source) in generated.ibp_li().enumerate() {
            let canonical_manifest = retain_relation_manifest(
                source,
                &mut stats.transcript_bytes,
                limits.max_transcript_bytes,
            )?;
            for (translation_ordinal, translation) in translations.iter().enumerate() {
                let expanded_ordinal = canonical_ordinal
                    .checked_mul(translations.len())
                    .and_then(|value| value.checked_add(translation_ordinal))
                    .ok_or(GeneratedPartialReeliminationError::ResourceCountOverflow {
                        resource: "expanded row ordinals",
                    })?;
                let translation_label = shift_string(translation)?;
                let translated = source.translated(
                    context,
                    translation,
                    ParametricRowId::Derived {
                        label: Arc::from(format!(
                            "generated-partial-c{canonical_ordinal}-t{translation_ordinal}-[{}]",
                            translation_label
                        )),
                    },
                    limits.specialization.arithmetic,
                )?;
                stats.translated_terms = checked_add(
                    "translated generated terms",
                    stats.translated_terms,
                    translated.terms().len(),
                )?;
                check_limit(
                    "translated generated terms",
                    stats.translated_terms,
                    limits.max_translated_terms,
                )?;
                let translated_manifest = retain_relation_manifest(
                    &translated,
                    &mut stats.transcript_bytes,
                    limits.max_transcript_bytes,
                )?;

                let translated = Arc::new(translated);
                match translated.partially_specialized_on(
                    context,
                    assignment.clone(),
                    limits.specialization,
                ) {
                    Ok(specialization) => {
                        let relation = specialization.relation_for_bound_reelimination();
                        let specialized_manifest = retain_relation_manifest(
                            relation,
                            &mut stats.transcript_bytes,
                            limits.max_transcript_bytes,
                        )?;
                        accumulate_specialization_stats(&mut stats, &specialization, limits)?;
                        for (assumption_ordinal, assumption) in
                            specialization.base_assumptions().iter().enumerate()
                        {
                            let condition = assumption.condition();
                            let next_base_assumptions =
                                checked_add("partial base assumptions", stats.base_assumptions, 1)?;
                            check_limit(
                                "partial base assumptions",
                                next_base_assumptions,
                                limits.max_base_assumptions,
                            )?;
                            let next_base_assumption_origins = checked_add(
                                "partial base assumption origins",
                                stats.base_assumption_origins,
                                condition.origins().len(),
                            )?;
                            check_limit(
                                "partial base assumption origins",
                                next_base_assumption_origins,
                                limits.max_base_assumption_origins,
                            )?;
                            try_reserve_conditional_entries(
                                "generated partial base assumptions",
                                &mut base_assumptions,
                                1,
                            )?;
                            let manifest = retain_assumption_manifest(
                                condition,
                                &mut stats.transcript_bytes,
                                limits.max_transcript_bytes,
                            )?;
                            stats.base_assumptions = next_base_assumptions;
                            stats.base_assumption_origins = next_base_assumption_origins;
                            base_assumptions.push(GeneratedPartialBaseAssumptionWitness {
                                expanded_row_ordinal: expanded_ordinal,
                                assumption_ordinal,
                                condition: condition.clone(),
                                manifest,
                                origin_count: condition.origins().len(),
                            });
                        }
                        let assumption_count = specialization.base_assumptions().len();
                        stats.retained_rows =
                            checked_add("retained conditional rows", stats.retained_rows, 1)?;
                        check_limit(
                            "retained conditional rows",
                            stats.retained_rows,
                            limits.max_retained_rows,
                        )?;
                        try_reserve_conditional_entries(
                            "retained conditional rows",
                            &mut retained_rows,
                            1,
                        )?;
                        witnesses.push(GeneratedPartialSourceRowWitness {
                            expanded_ordinal,
                            canonical_ordinal,
                            canonical_row_id: source.row_id().clone(),
                            translation: IndexShift::try_new(
                                translation.values().iter().copied(),
                                translation.arity(),
                            )?,
                            canonical_manifest: canonical_manifest.clone(),
                            translated_manifest,
                            outcome: GeneratedPartialSourceRowOutcome::Retained {
                                specialized_manifest,
                                base_assumptions: assumption_count,
                            },
                        });
                        retained_rows.push(RetainedConditionalRow {
                            specialization: Arc::new(specialization),
                        });
                    }
                    Err(ParametricRelationError::UnsatisfiableDomain) => {
                        stats.unsatisfiable_rows = checked_add(
                            "unsatisfiable conditional rows",
                            stats.unsatisfiable_rows,
                            1,
                        )?;
                        witnesses.push(GeneratedPartialSourceRowWitness {
                            expanded_ordinal,
                            canonical_ordinal,
                            canonical_row_id: source.row_id().clone(),
                            translation: IndexShift::try_new(
                                translation.values().iter().copied(),
                                translation.arity(),
                            )?,
                            canonical_manifest: canonical_manifest.clone(),
                            translated_manifest,
                            outcome: GeneratedPartialSourceRowOutcome::UnsatisfiableDomain,
                        });
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        }

        let family_fingerprint: Arc<str> = family.fingerprint().into();
        let context_fingerprint: Arc<str> = context.fingerprint().into();
        if retained_rows.is_empty() {
            return Ok(GeneratedPartialReeliminationCompilation::EmptySystem(
                GeneratedPartialReeliminationEmptySystem {
                    schema: GENERATED_PARTIAL_REELIMINATION_V2_SCHEMA,
                    authentication: GeneratedPartialSourceAuthentication::CanonicalIbpLiExactTranslationsAndSparseSpecialization,
                    family_fingerprint,
                    context_fingerprint,
                    assignment,
                    translations,
                    ordering,
                    witnesses,
                    limits,
                    stats,
                },
            ));
        }

        let source_rows = clone_retained_conditional_relations(&retained_rows)?;
        let elimination = ParametricElimination::build(
            context,
            &source_rows,
            ordering.clone(),
            limits.elimination,
        )?;
        let mut centered_pivot_loci = Vec::new();
        try_reserve_conditional_entries(
            "conditional centered pivot loci",
            &mut centered_pivot_loci,
            elimination.pivots().len(),
        )?;
        for pivot in elimination.pivots() {
            let mut entries = Vec::new();
            try_reserve_conditional_entries(
                "conditional centered assignment entries",
                &mut entries,
                assignment.entries().len(),
            )?;
            for &(position, value) in assignment.entries() {
                let centered = value.checked_add(pivot.pivot().values()[position]).ok_or(
                    GeneratedPartialReeliminationError::CenteredAssignmentOverflow {
                        pivot: pivot.ordinal(),
                        position,
                    },
                )?;
                entries.push((position, centered));
            }
            let centered_assignment = PartialIndexAssignment::try_new(
                entries,
                assignment.arity(),
                limits.specialization.max_assignments,
            )?;
            centered_pivot_loci.push(ConditionalCenteredPivotLocus {
                pivot_ordinal: pivot.ordinal(),
                original_pivot: IndexShift::try_new(
                    pivot.pivot().values().iter().copied(),
                    pivot.pivot().arity(),
                )?,
                centered_assignment,
            });
        }

        Ok(GeneratedPartialReeliminationCompilation::Certified(
            GeneratedPartialReeliminationCertificate {
                schema: GENERATED_PARTIAL_REELIMINATION_V2_SCHEMA,
                authentication: GeneratedPartialSourceAuthentication::CanonicalIbpLiExactTranslationsAndSparseSpecialization,
                family_fingerprint,
                context_fingerprint,
                assignment,
                translations,
                ordering,
                witnesses,
                base_assumptions,
                centered_pivot_loci,
                retained_rows,
                elimination,
                limits,
                stats,
            },
        ))
    }
}

fn canonical_translations(
    translations: &[IndexShift],
    arity: usize,
    limits: GeneratedPartialReeliminationLimits,
) -> Result<Vec<IndexShift>, GeneratedPartialReeliminationError> {
    check_limit(
        "partial source translations",
        translations.len(),
        limits.max_translations,
    )?;
    let components = translations.len().checked_mul(arity).ok_or(
        GeneratedPartialReeliminationError::ResourceCountOverflow {
            resource: "translation components",
        },
    )?;
    check_limit(
        "translation components",
        components,
        limits.max_translation_components,
    )?;
    for translation in translations {
        if translation.arity() != arity {
            return Err(GeneratedPartialReeliminationError::WrongArity {
                expected: arity,
                actual: translation.arity(),
            });
        }
    }
    if !translations
        .iter()
        .any(|translation| translation.values().iter().all(|&value| value == 0))
    {
        return Err(GeneratedPartialReeliminationError::MissingZeroTranslation);
    }
    let mut canonical = Vec::new();
    try_reserve_conditional_entries(
        "canonical partial source translations",
        &mut canonical,
        translations.len(),
    )?;
    for translation in translations {
        canonical.push(IndexShift::try_new(
            translation.values().iter().copied(),
            arity,
        )?);
    }
    canonical.sort();
    for pair in canonical.windows(2) {
        if pair[0] == pair[1] {
            let mut values = Vec::new();
            try_reserve_conditional_entries(
                "duplicate translation components",
                &mut values,
                pair[0].arity(),
            )?;
            values.extend_from_slice(pair[0].values());
            return Err(GeneratedPartialReeliminationError::DuplicateTranslation { values });
        }
    }
    Ok(canonical)
}

fn accumulate_specialization_stats(
    aggregate: &mut GeneratedPartialReeliminationStats,
    specialization: &PartialParametricRelationSpecialization,
    limits: GeneratedPartialReeliminationLimits,
) -> Result<(), GeneratedPartialReeliminationError> {
    let stats = specialization.stats();
    macro_rules! add_limited {
        ($field:ident, $value:expr, $name:literal, $limit:expr) => {{
            aggregate.$field = checked_add($name, aggregate.$field, $value)?;
            check_limit($name, aggregate.$field, $limit)?;
        }};
    }
    add_limited!(
        aggregate_source_terms,
        stats.source_terms(),
        "aggregate partial source terms",
        limits.max_aggregate_source_terms
    );
    add_limited!(
        aggregate_output_terms,
        stats.output_terms(),
        "aggregate partial output terms",
        limits.max_aggregate_output_terms
    );
    add_limited!(
        aggregate_power_operations,
        stats.power_operations(),
        "aggregate partial power operations",
        limits.max_aggregate_power_operations
    );
    add_limited!(
        aggregate_integer_bit_work,
        stats.integer_bit_work(),
        "aggregate partial integer bit work",
        limits.max_aggregate_integer_bit_work
    );
    add_limited!(
        aggregate_retained_terms,
        stats.retained_terms(),
        "aggregate partial retained terms",
        limits.max_aggregate_retained_terms
    );
    add_limited!(
        aggregate_retained_bytes,
        stats.retained_bytes(),
        "aggregate partial retained bytes",
        limits.max_aggregate_retained_bytes
    );

    let relation = specialization.relation_for_bound_reelimination();
    aggregate.specialized_guards = checked_add(
        "aggregate specialized guards",
        aggregate.specialized_guards,
        relation.guarded_nonzero_conditions().len(),
    )?;
    check_limit(
        "aggregate specialized guards",
        aggregate.specialized_guards,
        limits.max_specialized_guards,
    )?;
    let origins = sum_counts(
        "aggregate specialized guard origins",
        relation
            .guarded_nonzero_conditions()
            .iter()
            .map(|condition| condition.origins().len()),
    )?;
    aggregate.specialized_guard_origins = checked_add(
        "aggregate specialized guard origins",
        aggregate.specialized_guard_origins,
        origins,
    )?;
    check_limit(
        "aggregate specialized guard origins",
        aggregate.specialized_guard_origins,
        limits.max_specialized_guard_origins,
    )?;
    Ok(())
}

const GENERATED_PARTIAL_BASE_ASSUMPTION_V2_SCHEMA: &str =
    "rustred-generated-partial-base-assumption-v2";

struct ConditionalTranscriptCounter {
    bytes: usize,
    byte_offset: usize,
    max_bytes: usize,
    error: Option<GeneratedPartialReeliminationError>,
}

impl ConditionalTranscriptCounter {
    fn new_window(max_bytes: usize, byte_offset: usize) -> Self {
        Self {
            bytes: 0,
            byte_offset,
            max_bytes,
            error: None,
        }
    }
}

impl fmt::Write for ConditionalTranscriptCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(local_requested) = self.bytes.checked_add(value.len()) else {
            self.error = Some(GeneratedPartialReeliminationError::ResourceCountOverflow {
                resource: "conditional transcript bytes",
            });
            return Err(fmt::Error);
        };
        let Some(requested) = self.byte_offset.checked_add(local_requested) else {
            self.error = Some(GeneratedPartialReeliminationError::ResourceCountOverflow {
                resource: "conditional transcript bytes",
            });
            return Err(fmt::Error);
        };
        if requested > self.max_bytes {
            self.error = Some(GeneratedPartialReeliminationError::ResourceLimit {
                resource: "conditional transcript bytes",
                requested,
                limit: self.max_bytes,
            });
            return Err(fmt::Error);
        }
        self.bytes = local_requested;
        Ok(())
    }
}

struct ConditionalTranscriptBuilder {
    output: String,
    expected_bytes: usize,
    error: Option<GeneratedPartialReeliminationError>,
}

impl ConditionalTranscriptBuilder {
    fn try_new(expected_bytes: usize) -> Result<Self, GeneratedPartialReeliminationError> {
        let mut output = String::new();
        output.try_reserve_exact(expected_bytes).map_err(|_| {
            GeneratedPartialReeliminationError::AllocationFailure {
                resource: "conditional transcript bytes",
                requested: expected_bytes,
            }
        })?;
        Ok(Self {
            output,
            expected_bytes,
            error: None,
        })
    }
}

impl fmt::Write for ConditionalTranscriptBuilder {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(requested) = self.output.len().checked_add(value.len()) else {
            self.error = Some(GeneratedPartialReeliminationError::ResourceCountOverflow {
                resource: "conditional transcript bytes",
            });
            return Err(fmt::Error);
        };
        if requested > self.expected_bytes {
            self.error = Some(GeneratedPartialReeliminationError::ReplayMismatch);
            return Err(fmt::Error);
        }
        self.output.push_str(value);
        Ok(())
    }
}

enum ConditionalTranscriptSink<'a> {
    Counter(&'a mut ConditionalTranscriptCounter),
    Output(&'a mut ConditionalTranscriptBuilder),
}

impl ConditionalTranscriptSink<'_> {
    fn bytes_written(&self) -> usize {
        match self {
            Self::Counter(counter) => counter.byte_offset.saturating_add(counter.bytes),
            Self::Output(output) => output.output.len(),
        }
    }

    fn byte_ceiling(&self) -> usize {
        match self {
            Self::Counter(counter) => counter.max_bytes,
            Self::Output(output) => output.expected_bytes,
        }
    }

    fn take_error(&mut self) -> GeneratedPartialReeliminationError {
        match self {
            Self::Counter(counter) => counter.error.take(),
            Self::Output(output) => output.error.take(),
        }
        .unwrap_or(GeneratedPartialReeliminationError::ResourceCountOverflow {
            resource: "conditional transcript bytes",
        })
    }

    fn finish_write(
        &mut self,
        result: fmt::Result,
    ) -> Result<(), GeneratedPartialReeliminationError> {
        result.map_err(|_| self.take_error())
    }

    fn write_text(&mut self, value: &str) -> Result<(), GeneratedPartialReeliminationError> {
        let result = fmt::Write::write_str(self, value);
        self.finish_write(result)
    }

    fn write_arguments(
        &mut self,
        value: fmt::Arguments<'_>,
    ) -> Result<(), GeneratedPartialReeliminationError> {
        let result = fmt::Write::write_fmt(self, value);
        self.finish_write(result)
    }
}

impl fmt::Write for ConditionalTranscriptSink<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        match self {
            Self::Counter(counter) => counter.write_str(value),
            Self::Output(output) => output.write_str(value),
        }
    }
}

fn write_length_prefixed_conditional_payload<F>(
    sink: &mut ConditionalTranscriptSink<'_>,
    write_payload: F,
) -> Result<(), GeneratedPartialReeliminationError>
where
    F: for<'a> Fn(
        &mut ConditionalTranscriptSink<'a>,
    ) -> Result<(), GeneratedPartialReeliminationError>,
{
    let mut counter =
        ConditionalTranscriptCounter::new_window(sink.byte_ceiling(), sink.bytes_written());
    {
        let mut count_sink = ConditionalTranscriptSink::Counter(&mut counter);
        write_payload(&mut count_sink)?;
    }
    sink.write_arguments(format_args!("{}:", counter.bytes))?;
    write_payload(sink)
}

fn write_assumption_manifest(
    sink: &mut ConditionalTranscriptSink<'_>,
    condition: &crate::ParametricNonZeroCondition,
) -> Result<(), GeneratedPartialReeliminationError> {
    sink.write_text(GENERATED_PARTIAL_BASE_ASSUMPTION_V2_SCHEMA)?;
    sink.write_text("|polynomial=")?;
    write_length_prefixed_conditional_payload(sink, |payload| {
        let result = crate::parametric_relation::write_typed_polynomial(
            payload,
            condition.polynomial().raw(),
        );
        payload.finish_write(result)
    })?;
    sink.write_text("|origins=")?;
    sink.write_arguments(format_args!("{}", condition.origins().len()))?;
    for origin in condition.origins() {
        sink.write_text("|origin=")?;
        write_length_prefixed_conditional_payload(sink, |payload| {
            let result = origin.write_stable(payload);
            payload.finish_write(result)
        })?;
    }
    Ok(())
}

fn retain_assumption_manifest(
    condition: &crate::ParametricNonZeroCondition,
    retained_bytes: &mut usize,
    limit: usize,
) -> Result<Arc<String>, GeneratedPartialReeliminationError> {
    let mut counter = ConditionalTranscriptCounter::new_window(limit, *retained_bytes);
    {
        let mut sink = ConditionalTranscriptSink::Counter(&mut counter);
        write_assumption_manifest(&mut sink, condition)?;
    }
    let exact_bytes = counter.bytes;
    let mut output = ConditionalTranscriptBuilder::try_new(exact_bytes)?;
    {
        let mut sink = ConditionalTranscriptSink::Output(&mut output);
        write_assumption_manifest(&mut sink, condition)?;
    }
    if output.output.len() != exact_bytes {
        return Err(GeneratedPartialReeliminationError::ReplayMismatch);
    }
    *retained_bytes = checked_add("conditional transcript bytes", *retained_bytes, exact_bytes)?;
    debug_assert!(*retained_bytes <= limit);
    Ok(Arc::new(output.output))
}

pub(crate) fn certificate_payload_eq(
    left: &GeneratedPartialReeliminationCertificate,
    right: &GeneratedPartialReeliminationCertificate,
) -> bool {
    left.schema == right.schema
        && left.authentication == right.authentication
        && left.family_fingerprint == right.family_fingerprint
        && left.context_fingerprint == right.context_fingerprint
        && left.assignment == right.assignment
        && left.translations == right.translations
        && left.ordering == right.ordering
        && left.witnesses == right.witnesses
        && left.base_assumptions == right.base_assumptions
        && left.centered_pivot_loci == right.centered_pivot_loci
        && left.stats == right.stats
        && elimination_payload_eq(&left.elimination, &right.elimination)
}

fn elimination_payload_eq(left: &ParametricElimination, right: &ParametricElimination) -> bool {
    left.family_fingerprint() == right.family_fingerprint()
        && left.context_fingerprint() == right.context_fingerprint()
        && left.source_manifest() == right.source_manifest()
        && left.ordering() == right.ordering()
        && left.limits() == right.limits()
        && left.columns_easiest_first() == right.columns_easiest_first()
        && left.free_columns() == right.free_columns()
        && left.stats() == right.stats()
        && left.pivots().len() == right.pivots().len()
        && left
            .pivots()
            .iter()
            .zip(right.pivots())
            .all(|(left, right)| {
                left.ordinal() == right.ordinal()
                    && left.pivot() == right.pivot()
                    && left.trace() == right.trace()
                    && left
                        .unit_relation()
                        .has_identical_guard_provenance(right.unit_relation())
            })
}

fn validate_replay_scope(
    schema: &str,
    family_fingerprint: &str,
    context_fingerprint: &str,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
) -> Result<(), GeneratedPartialReeliminationError> {
    if schema != GENERATED_PARTIAL_REELIMINATION_V2_SCHEMA {
        return Err(GeneratedPartialReeliminationError::SchemaMismatch);
    }
    if family_fingerprint != family.fingerprint() {
        return Err(GeneratedPartialReeliminationError::WrongFamily);
    }
    if context_fingerprint != context.fingerprint() {
        return Err(GeneratedPartialReeliminationError::WrongContext);
    }
    Ok(())
}

fn shift_string(shift: &IndexShift) -> Result<String, GeneratedPartialReeliminationError> {
    let value_bytes = shift.values().len().checked_mul(20).ok_or(
        GeneratedPartialReeliminationError::ResourceCountOverflow {
            resource: "translation diagnostic bytes",
        },
    )?;
    let separator_bytes = shift.values().len().saturating_sub(1);
    let requested = checked_add("translation diagnostic bytes", value_bytes, separator_bytes)?;
    let mut output = String::new();
    output.try_reserve_exact(requested).map_err(|_| {
        GeneratedPartialReeliminationError::AllocationFailure {
            resource: "translation diagnostic bytes",
            requested,
        }
    })?;
    for (ordinal, value) in shift.values().iter().enumerate() {
        if ordinal != 0 {
            output.write_str(",").map_err(|_| {
                GeneratedPartialReeliminationError::ResourceCountOverflow {
                    resource: "translation diagnostic bytes",
                }
            })?;
        }
        write!(&mut output, "{value}").map_err(|_| {
            GeneratedPartialReeliminationError::ResourceCountOverflow {
                resource: "translation diagnostic bytes",
            }
        })?;
    }
    Ok(output)
}

fn sum_counts(
    resource: &'static str,
    values: impl IntoIterator<Item = usize>,
) -> Result<usize, GeneratedPartialReeliminationError> {
    values
        .into_iter()
        .try_fold(0usize, |total, value| checked_add(resource, total, value))
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedPartialReeliminationError> {
    left.checked_add(right)
        .ok_or(GeneratedPartialReeliminationError::ResourceCountOverflow { resource })
}

fn try_reserve_conditional_entries<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), GeneratedPartialReeliminationError> {
    let requested = checked_add(resource, values.len(), additional)?;
    values.try_reserve_exact(additional).map_err(|_| {
        GeneratedPartialReeliminationError::AllocationFailure {
            resource,
            requested,
        }
    })
}

fn clone_retained_conditional_relations(
    retained_rows: &[RetainedConditionalRow],
) -> Result<Vec<ParametricRelation>, GeneratedPartialReeliminationError> {
    let mut source_rows = Vec::new();
    try_reserve_conditional_entries(
        "conditional elimination source rows",
        &mut source_rows,
        retained_rows.len(),
    )?;
    for row in retained_rows {
        // ParametricElimination::build currently consumes an owned relation
        // slice and clones each row again during reduction.  Keep this one
        // unavoidable downstream API clone explicit and bounded by the
        // already checked retained-row count.
        source_rows.push(
            row.specialization
                .relation_for_bound_reelimination()
                .clone(),
        );
    }
    Ok(source_rows)
}

fn retain_relation_manifest(
    relation: &ParametricRelation,
    retained_bytes: &mut usize,
    limit: usize,
) -> Result<Arc<String>, GeneratedPartialReeliminationError> {
    let remaining = limit.checked_sub(*retained_bytes).ok_or(
        GeneratedPartialReeliminationError::ResourceLimit {
            resource: "conditional transcript bytes",
            requested: *retained_bytes,
            limit,
        },
    )?;
    let manifest = match relation.stable_manifest_with_limit(remaining) {
        Ok(manifest) => manifest,
        Err(ParametricRelationError::ResourceLimit { requested, .. }) => {
            return Err(GeneratedPartialReeliminationError::ResourceLimit {
                resource: "conditional transcript bytes",
                requested: checked_add("conditional transcript bytes", *retained_bytes, requested)?,
                limit,
            });
        }
        Err(ParametricRelationError::ResourceCountOverflow { .. }) => {
            return Err(GeneratedPartialReeliminationError::ResourceCountOverflow {
                resource: "conditional transcript bytes",
            });
        }
        Err(error @ ParametricRelationError::AllocationFailure { .. }) => {
            // Preserve the typed allocation request. The family-layer
            // classifier follows nested relation resource failures.
            return Err(GeneratedPartialReeliminationError::Relation(error));
        }
        Err(error) => return Err(GeneratedPartialReeliminationError::Relation(error)),
    };
    *retained_bytes = checked_add(
        "conditional transcript bytes",
        *retained_bytes,
        manifest.len(),
    )?;
    debug_assert!(*retained_bytes <= limit);

    // Moving the String into Arc keeps its already fallibly reserved payload
    // buffer; converting String -> Arc<str> would allocate and copy it again.
    Ok(Arc::new(manifest))
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedPartialReeliminationError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(GeneratedPartialReeliminationError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedPartialReeliminationError {
    WrongFamily,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    MissingZeroTranslation,
    DuplicateTranslation {
        values: Vec<i64>,
    },
    CenteredAssignmentOverflow {
        pivot: usize,
        position: usize,
    },
    PivotOutOfRange {
        pivot: usize,
        available: usize,
    },
    SchemaMismatch,
    ReplayMismatch,
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Ibp(ParametricIbpError),
    Relation(ParametricRelationError),
    Elimination(ParametricEliminationError),
    Coefficient(crate::ParametricCoefficientError),
}

impl fmt::Display for GeneratedPartialReeliminationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFamily => formatter
                .write_str("the replay family differs from the authenticated generated family"),
            Self::WrongContext => formatter
                .write_str("the K(n) context differs from the authenticated generated context"),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "conditional source arity is {actual}, expected {expected}"
            ),
            Self::MissingZeroTranslation => formatter
                .write_str("the generated conditional stencil must contain the zero translation"),
            Self::DuplicateTranslation { values } => write!(
                formatter,
                "the generated conditional stencil repeats translation {values:?}"
            ),
            Self::CenteredAssignmentOverflow { pivot, position } => write!(
                formatter,
                "source assignment plus pivot {pivot} overflowed at index {position}"
            ),
            Self::PivotOutOfRange { pivot, available } => write!(
                formatter,
                "conditional pivot {pivot} is outside {available} available pivots"
            ),
            Self::SchemaMismatch => {
                formatter.write_str("the generated partial re-elimination schema differs")
            }
            Self::ReplayMismatch => formatter
                .write_str("the generated partial re-elimination transcript differs on replay"),
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
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve capacity {requested} for {resource}"
            ),
            Self::Ibp(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Elimination(error) => error.fmt(formatter),
            Self::Coefficient(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedPartialReeliminationError {}

impl From<ParametricIbpError> for GeneratedPartialReeliminationError {
    fn from(value: ParametricIbpError) -> Self {
        Self::Ibp(value)
    }
}
impl From<ParametricRelationError> for GeneratedPartialReeliminationError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}
impl From<ParametricEliminationError> for GeneratedPartialReeliminationError {
    fn from(value: ParametricEliminationError) -> Self {
        Self::Elimination(value)
    }
}
impl From<crate::ParametricCoefficientError> for GeneratedPartialReeliminationError {
    fn from(value: crate::ParametricCoefficientError) -> Self {
        Self::Coefficient(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AffineDenominator, IndexSpace, IntegralOrderingPolicy, algebra::CoefficientContext,
    };

    fn family() -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        IntegralFamily::new(
            "conditional-reelimination-provenance-tamper",
            vec!["k".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                coefficients.parse("-m2").unwrap(),
                vec![coefficients.one()],
            )],
            Vec::new(),
            vec![coefficients.zero()],
        )
        .unwrap()
    }

    #[test]
    fn transcript_retention_honors_zero_and_one_byte_remaining() {
        let coefficients = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&coefficients, "conditional-manifest-limit", 1)
                .unwrap();
        let relation = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: "bounded-manifest".into(),
            },
            &context,
        );

        for remaining in [0usize, 1] {
            let mut retained = 7usize;
            let limit = retained + remaining;
            assert!(matches!(
                retain_relation_manifest(&relation, &mut retained, limit),
                Err(GeneratedPartialReeliminationError::ResourceLimit {
                    resource: "conditional transcript bytes",
                    requested,
                    limit: actual_limit,
                }) if requested > actual_limit && actual_limit == limit
            ));
            assert_eq!(retained, 7, "a rejected manifest must not commit bytes");
        }

        let polynomial = context
            .numerator_condition(&context.index(0).unwrap())
            .unwrap();
        let condition = context
            .nonzero_condition(polynomial, crate::GuardOrigin::ExplicitRelationCondition)
            .unwrap();
        for remaining in [0usize, 1] {
            let mut retained = 11usize;
            let limit = retained + remaining;
            assert!(matches!(
                retain_assumption_manifest(&condition, &mut retained, limit),
                Err(GeneratedPartialReeliminationError::ResourceLimit {
                    resource: "conditional transcript bytes",
                    requested,
                    limit: actual_limit,
                }) if requested > actual_limit && actual_limit == limit
            ));
            assert_eq!(retained, 11, "a rejected assumption must not commit bytes");
        }

        let mut retained = 0;
        let manifest = retain_assumption_manifest(&condition, &mut retained, usize::MAX).unwrap();
        assert!(manifest.starts_with(GENERATED_PARTIAL_BASE_ASSUMPTION_V2_SCHEMA));
        assert_eq!(retained, manifest.len());
    }

    #[test]
    fn conditional_entry_reservation_reports_capacity_failure_without_growth() {
        let mut values = Vec::<u8>::new();
        assert!(matches!(
            try_reserve_conditional_entries(
                "conditional allocation-path test entries",
                &mut values,
                usize::MAX,
            ),
            Err(GeneratedPartialReeliminationError::AllocationFailure {
                resource: "conditional allocation-path test entries",
                requested: usize::MAX,
            })
        ));
        assert!(values.is_empty());
    }

    #[test]
    fn certificate_clones_share_retained_conditional_specializations() {
        let family = family();
        let generated = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .generate()
            .unwrap();
        let context = generated.context();
        let compilation = GeneratedPartialReeliminationCompiler::compile(
            &family,
            context,
            &[IndexSpace::try_new(1).unwrap().zero()],
            PartialIndexAssignment::try_new([(0, 1)], 1, 1).unwrap(),
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
                .unwrap(),
            GeneratedPartialReeliminationLimits::default(),
        )
        .unwrap();
        let GeneratedPartialReeliminationCompilation::Certified(certificate) = compilation else {
            panic!("expected a retained generated conditional system");
        };
        let cloned = certificate.clone();
        assert!(!certificate.retained_rows.is_empty());
        assert_eq!(certificate.retained_rows.len(), cloned.retained_rows.len());
        for (retained, cloned_retained) in
            certificate.retained_rows.iter().zip(&cloned.retained_rows)
        {
            assert!(Arc::ptr_eq(
                &retained.specialization,
                &cloned_retained.specialization,
            ));
        }
    }

    #[test]
    fn provenance_tampering_is_detected_by_fresh_generated_replay() {
        let family = family();
        let generated = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .generate()
            .unwrap();
        let context = generated.context();
        let compilation = GeneratedPartialReeliminationCompiler::compile(
            &family,
            context,
            &[IndexSpace::try_new(1).unwrap().zero()],
            PartialIndexAssignment::try_new([(0, 1)], 1, 1).unwrap(),
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [1])
                .unwrap(),
            GeneratedPartialReeliminationLimits::default(),
        )
        .unwrap();
        let GeneratedPartialReeliminationCompilation::Certified(mut certificate) = compilation
        else {
            panic!("expected a retained generated conditional system");
        };
        certificate.witnesses[0].translated_manifest = Arc::new("tampered".to_owned());
        assert!(matches!(
            certificate.replay(&family, context),
            Err(GeneratedPartialReeliminationError::ReplayMismatch)
        ));
    }
}
