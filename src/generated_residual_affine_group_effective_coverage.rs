//! Sequential effective coverage for one exact generated residual-affine
//! geometry group.
//!
//! This layer owns the only mutable `consumed_targets` state in the generated
//! affine pipeline. It iterates the retained matcher outcome order exactly
//! once, invokes target-local affine `WhenBad` only for a pending pivot's
//! persisted first available target, and consumes that target only after a
//! complete `Certified` local result. No concrete family-specific metadata
//! enters this transition.

use std::fmt;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use crate::generated_residual_affine_pivot_target_matching::{
    GeneratedResidualAffineEffectiveTargetSelection,
    GeneratedResidualAffineEffectiveTargetSelectionError,
};
use crate::generated_residual_affine_when_bad_compilation::{
    GeneratedResidualAffineWhenBadCompilation, GeneratedResidualAffineWhenBadCompiler,
    GeneratedResidualAffineWhenBadError, GeneratedResidualAffineWhenBadGroupResourceUsage,
    GeneratedResidualAffineWhenBadLimits,
};
use crate::{
    AffineWhenBadRelativeCaseId, AffineWhenBadRelativeLeafDisposition,
    GeneratedResidualAffineCaseInventoryCertificate, GeneratedResidualAffineCaseLocator,
    GeneratedResidualAffineContiguousCaseGroup,
    GeneratedResidualAffinePivotTargetMatchingCertificate,
    GeneratedResidualAffinePivotTargetMatchingError, GeneratedResidualAffinePivotTargetOutcome,
    IntegralFamily, ParametricCoefficientContext,
};

pub(crate) const GENERATED_RESIDUAL_AFFINE_GROUP_EFFECTIVE_COVERAGE_V1_SCHEMA: &str =
    "rustred-generated-residual-affine-group-effective-coverage-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineGroupEffectiveCoverageLimits {
    pub(crate) local_when_bad: GeneratedResidualAffineWhenBadLimits,
    pub(crate) max_matcher_outcomes_inspected: usize,
    pub(crate) max_pending_target_selections: usize,
    pub(crate) max_checked_target_references: usize,
    pub(crate) max_matching_target_references: usize,
    pub(crate) max_selection_target_references_inspected: usize,
    pub(crate) max_group_cases_inspected: usize,
    pub(crate) max_local_when_bad_compilations: usize,
    pub(crate) max_accepted_attempts: usize,
    pub(crate) max_rejected_attempts: usize,
    pub(crate) max_consumed_targets: usize,
    pub(crate) max_rejected_attempt_references_per_target: usize,
    pub(crate) max_rejected_attempt_references: usize,
    pub(crate) max_child_source_terms: usize,
    pub(crate) max_child_source_exponent_entries: usize,
    pub(crate) max_child_source_integer_bits: usize,
    pub(crate) max_child_output_terms: usize,
    pub(crate) max_child_output_exponent_entries: usize,
    pub(crate) max_child_native_integer_bit_work: usize,
    pub(crate) max_child_total_integer_bit_work: usize,
    pub(crate) max_child_payload_comparison_units: usize,
    pub(crate) max_child_payload_comparison_bytes: usize,
    pub(crate) max_child_payload_comparison_integer_bits: usize,
    pub(crate) max_child_payload_comparison_private_manifest_bytes: usize,
    pub(crate) max_child_structural_loci: usize,
    pub(crate) max_child_bad_clauses: usize,
    pub(crate) max_child_relative_leaves: usize,
    pub(crate) max_child_retained_bytes: usize,
    pub(crate) max_group_target_dispositions: usize,
    pub(crate) max_sealed_conditional_rule_handles: usize,
    pub(crate) max_residual_work_leaves: usize,
    pub(crate) max_outer_retained_bytes: usize,
    pub(crate) max_outer_payload_comparison_units: usize,
    pub(crate) max_outer_payload_comparison_bytes: usize,
    pub(crate) max_outer_payload_comparison_integer_bits: usize,
}

impl Default for GeneratedResidualAffineGroupEffectiveCoverageLimits {
    fn default() -> Self {
        Self {
            local_when_bad: GeneratedResidualAffineWhenBadLimits::default(),
            max_matcher_outcomes_inspected: 256_000_000,
            max_pending_target_selections: 256_000_000,
            max_checked_target_references: 1_000_000_000,
            max_matching_target_references: 1_000_000_000,
            max_selection_target_references_inspected: 1_000_000_000,
            max_group_cases_inspected: 256_000_000,
            max_local_when_bad_compilations: 256_000_000,
            max_accepted_attempts: 256_000_000,
            max_rejected_attempts: 256_000_000,
            max_consumed_targets: 256_000_000,
            max_rejected_attempt_references_per_target: 256_000_000,
            max_rejected_attempt_references: 1_000_000_000,
            max_child_source_terms: 2_000_000_000,
            max_child_source_exponent_entries: portable_usize(128_000_000_000),
            max_child_source_integer_bits: portable_usize(32_000_000_000_000_000),
            max_child_output_terms: 2_000_000_000,
            max_child_output_exponent_entries: portable_usize(128_000_000_000),
            max_child_native_integer_bit_work: portable_usize(64_000_000_000_000_000),
            max_child_total_integer_bit_work: portable_usize(64_000_000_000_000_000),
            max_child_payload_comparison_units: portable_usize(512_000_000_000),
            max_child_payload_comparison_bytes: portable_usize(512 * 1024 * 1024 * 1024),
            max_child_payload_comparison_integer_bits: portable_usize(64_000_000_000_000_000),
            max_child_payload_comparison_private_manifest_bytes: portable_usize(
                16 * 1024 * 1024 * 1024,
            ),
            max_child_structural_loci: 1_000_000_000,
            max_child_bad_clauses: 1_000_000_000,
            max_child_relative_leaves: 1_000_000_000,
            max_child_retained_bytes: portable_usize(256 * 1024 * 1024 * 1024),
            max_group_target_dispositions: 256_000_000,
            max_sealed_conditional_rule_handles: 1_000_000_000,
            max_residual_work_leaves: 1_000_000_000,
            max_outer_retained_bytes: portable_usize(64 * 1024 * 1024 * 1024),
            max_outer_payload_comparison_units: portable_usize(64_000_000_000),
            max_outer_payload_comparison_bytes: portable_usize(64 * 1024 * 1024 * 1024),
            max_outer_payload_comparison_integer_bits: portable_usize(4_000_000_000_000),
        }
    }
}

const fn portable_usize(value: u64) -> usize {
    if value > usize::MAX as u64 {
        usize::MAX
    } else {
        value as usize
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineGroupEffectiveCoverageStats {
    matcher_outcomes_inspected: usize,
    pending_target_selections: usize,
    checked_target_references: usize,
    matching_target_references: usize,
    selection_target_references_inspected: usize,
    group_cases_inspected: usize,
    local_when_bad_compilations: usize,
    accepted_attempts: usize,
    rejected_attempts: usize,
    consumed_targets: usize,
    rejected_attempt_references: usize,
    maximum_rejected_attempt_references_per_target: usize,
    child_source_terms: usize,
    child_source_exponent_entries: usize,
    child_source_integer_bits: usize,
    child_output_terms: usize,
    child_output_exponent_entries: usize,
    child_native_integer_bit_work: usize,
    child_total_integer_bit_work: usize,
    child_payload_comparison_units: usize,
    child_payload_comparison_bytes: usize,
    child_payload_comparison_integer_bits: usize,
    child_payload_comparison_private_manifest_bytes: usize,
    child_assembly_payload_comparison_units: usize,
    child_assembly_payload_comparison_bytes: usize,
    child_structural_loci: usize,
    child_bad_clauses: usize,
    child_applicable_leaves: usize,
    child_exceptional_leaves: usize,
    child_retained_bytes: usize,
    group_target_dispositions: usize,
    sealed_conditional_rule_handles: usize,
    unconsumed_residual_roots: usize,
    exceptional_residual_leaves: usize,
    residual_work_leaves: usize,
    outer_retained_bytes: usize,
    outer_payload_comparison_units: usize,
    outer_payload_comparison_bytes: usize,
    outer_payload_comparison_integer_bits: usize,
}

macro_rules! group_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedResidualAffineGroupEffectiveCoverageStats {
    group_stats_getters!(
        matcher_outcomes_inspected,
        pending_target_selections,
        checked_target_references,
        matching_target_references,
        selection_target_references_inspected,
        group_cases_inspected,
        local_when_bad_compilations,
        accepted_attempts,
        rejected_attempts,
        consumed_targets,
        rejected_attempt_references,
        maximum_rejected_attempt_references_per_target,
        child_source_terms,
        child_source_exponent_entries,
        child_source_integer_bits,
        child_output_terms,
        child_output_exponent_entries,
        child_native_integer_bit_work,
        child_total_integer_bit_work,
        child_payload_comparison_units,
        child_payload_comparison_bytes,
        child_payload_comparison_integer_bits,
        child_payload_comparison_private_manifest_bytes,
        child_assembly_payload_comparison_units,
        child_assembly_payload_comparison_bytes,
        child_structural_loci,
        child_bad_clauses,
        child_applicable_leaves,
        child_exceptional_leaves,
        child_retained_bytes,
        group_target_dispositions,
        sealed_conditional_rule_handles,
        unconsumed_residual_roots,
        exceptional_residual_leaves,
        residual_work_leaves,
        outer_retained_bytes,
        outer_payload_comparison_units,
        outer_payload_comparison_bytes,
        outer_payload_comparison_integer_bits,
    );
}

pub(crate) struct GeneratedResidualAffineTargetAttempt {
    attempt_ordinal: usize,
    pivot_ordinal: usize,
    selected_target_case_ordinal: Option<usize>,
    selected_target_position: Option<usize>,
    outcome: GeneratedResidualAffineTargetAttemptOutcome,
}

impl GeneratedResidualAffineTargetAttempt {
    pub(crate) const fn attempt_ordinal(&self) -> usize {
        self.attempt_ordinal
    }

    pub(crate) const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }

    pub(crate) const fn selected_target_case_ordinal(&self) -> Option<usize> {
        self.selected_target_case_ordinal
    }

    pub(crate) const fn selected_target_position(&self) -> Option<usize> {
        self.selected_target_position
    }

    pub(crate) const fn outcome(&self) -> &GeneratedResidualAffineTargetAttemptOutcome {
        &self.outcome
    }
}

impl fmt::Debug for GeneratedResidualAffineTargetAttempt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineTargetAttempt")
            .field("attempt_ordinal", &self.attempt_ordinal)
            .field("pivot_ordinal", &self.pivot_ordinal)
            .field(
                "selected_target_case_ordinal",
                &self.selected_target_case_ordinal,
            )
            .field("selected_target_position", &self.selected_target_position)
            .field("outcome", &self.outcome)
            .finish()
    }
}

pub(crate) enum GeneratedResidualAffineTargetAttemptOutcome {
    MatcherRejectedNoTarget,
    MatcherRejectedRecenteringBoundary,
    NoRemainingTargetCase,
    WhenBadUnsupported(Arc<GeneratedResidualAffineWhenBadCompilation>),
    WhenBadIdenticallyBad(Arc<GeneratedResidualAffineWhenBadCompilation>),
    Accepted(Arc<GeneratedResidualAffineWhenBadCompilation>),
}

impl GeneratedResidualAffineTargetAttemptOutcome {
    pub(crate) const fn local_compilation(
        &self,
    ) -> Option<&Arc<GeneratedResidualAffineWhenBadCompilation>> {
        match self {
            Self::WhenBadUnsupported(value)
            | Self::WhenBadIdenticallyBad(value)
            | Self::Accepted(value) => Some(value),
            Self::MatcherRejectedNoTarget
            | Self::MatcherRejectedRecenteringBoundary
            | Self::NoRemainingTargetCase => None,
        }
    }
}

impl fmt::Debug for GeneratedResidualAffineTargetAttemptOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MatcherRejectedNoTarget => formatter.write_str("MatcherRejectedNoTarget"),
            Self::MatcherRejectedRecenteringBoundary => {
                formatter.write_str("MatcherRejectedRecenteringBoundary")
            }
            Self::NoRemainingTargetCase => formatter.write_str("NoRemainingTargetCase"),
            Self::WhenBadUnsupported(value) => formatter
                .debug_struct("WhenBadUnsupported")
                .field("binding", value.binding())
                .field("stats", &value.stats())
                .field("private_payload", &"<redacted>")
                .finish(),
            Self::WhenBadIdenticallyBad(value) => formatter
                .debug_struct("WhenBadIdenticallyBad")
                .field("binding", value.binding())
                .field("stats", &value.stats())
                .field("private_payload", &"<redacted>")
                .finish(),
            Self::Accepted(value) => formatter
                .debug_struct("Accepted")
                .field("binding", value.binding())
                .field("stats", &value.stats())
                .field("private_payload", &"<redacted>")
                .finish(),
        }
    }
}

pub(crate) struct GeneratedResidualAffineGroupTargetDispositionRecord {
    target_case_ordinal: usize,
    target_locator: GeneratedResidualAffineCaseLocator,
    disposition: GeneratedResidualAffineGroupTargetDisposition,
}

impl GeneratedResidualAffineGroupTargetDispositionRecord {
    pub(crate) const fn target_case_ordinal(&self) -> usize {
        self.target_case_ordinal
    }

    pub(crate) const fn target_locator(&self) -> GeneratedResidualAffineCaseLocator {
        self.target_locator
    }

    pub(crate) const fn disposition(&self) -> &GeneratedResidualAffineGroupTargetDisposition {
        &self.disposition
    }
}

impl fmt::Debug for GeneratedResidualAffineGroupTargetDispositionRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineGroupTargetDispositionRecord")
            .field("target_case_ordinal", &self.target_case_ordinal)
            .field("target_locator", &self.target_locator)
            .field("disposition", &self.disposition)
            .finish()
    }
}

pub(crate) enum GeneratedResidualAffineGroupTargetDisposition {
    Consumed {
        accepted_attempt_ordinal: usize,
        when_bad: Arc<GeneratedResidualAffineWhenBadCompilation>,
    },
    Unconsumed {
        rejected_attempt_ordinals: Vec<usize>,
    },
}

impl fmt::Debug for GeneratedResidualAffineGroupTargetDisposition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Consumed {
                accepted_attempt_ordinal,
                when_bad,
            } => formatter
                .debug_struct("Consumed")
                .field("accepted_attempt_ordinal", accepted_attempt_ordinal)
                .field("binding", when_bad.binding())
                .field("private_payload", &"<redacted>")
                .finish(),
            Self::Unconsumed {
                rejected_attempt_ordinals,
            } => formatter
                .debug_struct("Unconsumed")
                .field("rejected_attempt_ordinals", rejected_attempt_ordinals)
                .finish(),
        }
    }
}

pub(crate) struct GeneratedResidualAffineSealedConditionalRuleHandle {
    accepted_attempt_ordinal: usize,
    pivot_ordinal: usize,
    target_case_ordinal: usize,
    target_locator: GeneratedResidualAffineCaseLocator,
    leaf_ordinal: usize,
    relative_case: AffineWhenBadRelativeCaseId,
    when_bad: Arc<GeneratedResidualAffineWhenBadCompilation>,
}

impl GeneratedResidualAffineSealedConditionalRuleHandle {
    pub(crate) const fn accepted_attempt_ordinal(&self) -> usize {
        self.accepted_attempt_ordinal
    }

    pub(crate) const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }

    pub(crate) const fn target_case_ordinal(&self) -> usize {
        self.target_case_ordinal
    }

    pub(crate) const fn target_locator(&self) -> GeneratedResidualAffineCaseLocator {
        self.target_locator
    }

    pub(crate) const fn leaf_ordinal(&self) -> usize {
        self.leaf_ordinal
    }

    pub(crate) const fn relative_case(&self) -> AffineWhenBadRelativeCaseId {
        self.relative_case
    }

    /// Exact private compilation allocation which authorized this sealed
    /// handle.  This crate-private seam exposes no relation or predicate
    /// payload; it exists solely so a higher-level owner can authenticate a
    /// concrete point result by allocation identity.
    pub(crate) const fn when_bad(&self) -> &Arc<GeneratedResidualAffineWhenBadCompilation> {
        &self.when_bad
    }

    pub(crate) fn rhs_term_count(&self) -> usize {
        self.when_bad.binding().rhs_terms()
    }
}

impl fmt::Debug for GeneratedResidualAffineSealedConditionalRuleHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineSealedConditionalRuleHandle")
            .field("accepted_attempt_ordinal", &self.accepted_attempt_ordinal)
            .field("pivot_ordinal", &self.pivot_ordinal)
            .field("target_case_ordinal", &self.target_case_ordinal)
            .field("target_locator", &self.target_locator)
            .field("leaf_ordinal", &self.leaf_ordinal)
            .field("relative_case", &self.relative_case)
            .field("rhs_term_count", &self.rhs_term_count())
            .field("private_rule", &"<sealed>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedResidualAffineResidualWorkKind {
    CompleteTargetRoot,
    ExceptionalDomain { condition_ordinal: usize },
    ExceptionalLeak { pullback_ordinal: usize },
}

pub(crate) struct GeneratedResidualAffineResidualWorkLeaf {
    target_case_ordinal: usize,
    target_locator: GeneratedResidualAffineCaseLocator,
    accepted_attempt_ordinal: Option<usize>,
    leaf_ordinal: Option<usize>,
    relative_case: Option<AffineWhenBadRelativeCaseId>,
    kind: GeneratedResidualAffineResidualWorkKind,
    when_bad: Option<Arc<GeneratedResidualAffineWhenBadCompilation>>,
}

impl GeneratedResidualAffineResidualWorkLeaf {
    pub(crate) const fn target_case_ordinal(&self) -> usize {
        self.target_case_ordinal
    }

    pub(crate) const fn target_locator(&self) -> GeneratedResidualAffineCaseLocator {
        self.target_locator
    }

    pub(crate) const fn accepted_attempt_ordinal(&self) -> Option<usize> {
        self.accepted_attempt_ordinal
    }

    pub(crate) const fn leaf_ordinal(&self) -> Option<usize> {
        self.leaf_ordinal
    }

    pub(crate) const fn relative_case(&self) -> Option<AffineWhenBadRelativeCaseId> {
        self.relative_case
    }

    pub(crate) const fn kind(&self) -> GeneratedResidualAffineResidualWorkKind {
        self.kind
    }

    /// Exact private compilation allocation which authorized an exceptional
    /// child. Complete-target roots deliberately return `None`.
    pub(crate) const fn when_bad(&self) -> Option<&Arc<GeneratedResidualAffineWhenBadCompilation>> {
        self.when_bad.as_ref()
    }
}

impl fmt::Debug for GeneratedResidualAffineResidualWorkLeaf {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineResidualWorkLeaf")
            .field("target_case_ordinal", &self.target_case_ordinal)
            .field("target_locator", &self.target_locator)
            .field("accepted_attempt_ordinal", &self.accepted_attempt_ordinal)
            .field("leaf_ordinal", &self.leaf_ordinal)
            .field("relative_case", &self.relative_case)
            .field("kind", &self.kind)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

pub(crate) struct GeneratedResidualAffineGroupEffectiveCoverageCertificate {
    schema: &'static str,
    matcher: Arc<GeneratedResidualAffinePivotTargetMatchingCertificate>,
    attempts: Vec<GeneratedResidualAffineTargetAttempt>,
    target_dispositions: Vec<GeneratedResidualAffineGroupTargetDispositionRecord>,
    sealed_rules: Vec<GeneratedResidualAffineSealedConditionalRuleHandle>,
    residual_work: Vec<GeneratedResidualAffineResidualWorkLeaf>,
    limits: GeneratedResidualAffineGroupEffectiveCoverageLimits,
    stats: GeneratedResidualAffineGroupEffectiveCoverageStats,
}

impl GeneratedResidualAffineGroupEffectiveCoverageCertificate {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn matcher(
        &self,
    ) -> &Arc<GeneratedResidualAffinePivotTargetMatchingCertificate> {
        &self.matcher
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.matcher.inventory().family_fingerprint()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.matcher.inventory().context_fingerprint()
    }

    pub(crate) fn attempts(&self) -> &[GeneratedResidualAffineTargetAttempt] {
        &self.attempts
    }

    pub(crate) fn target_dispositions(
        &self,
    ) -> &[GeneratedResidualAffineGroupTargetDispositionRecord] {
        &self.target_dispositions
    }

    pub(crate) fn sealed_rules(&self) -> &[GeneratedResidualAffineSealedConditionalRuleHandle] {
        &self.sealed_rules
    }

    pub(crate) fn residual_work(&self) -> &[GeneratedResidualAffineResidualWorkLeaf] {
        &self.residual_work
    }

    pub(crate) const fn limits(&self) -> GeneratedResidualAffineGroupEffectiveCoverageLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> GeneratedResidualAffineGroupEffectiveCoverageStats {
        self.stats
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedResidualAffineGroupEffectiveCoverageError> {
        if self.schema != GENERATED_RESIDUAL_AFFINE_GROUP_EFFECTIVE_COVERAGE_V1_SCHEMA {
            return Err(GeneratedResidualAffineGroupEffectiveCoverageError::SchemaMismatch);
        }
        let rebuilt = GeneratedResidualAffineGroupEffectiveCoverageCompiler::compile(
            family,
            context,
            self.matcher.clone(),
            self.limits,
        )?;
        if payload_eq_checked(self, &rebuilt)? {
            Ok(())
        } else {
            Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch)
        }
    }

    #[cfg(test)]
    pub(crate) fn test_only_corrupt_first_residual_authority_shape(&mut self) -> bool {
        let Some(leaf) = self.residual_work.first_mut() else {
            return false;
        };
        match leaf.kind {
            GeneratedResidualAffineResidualWorkKind::CompleteTargetRoot => {
                leaf.accepted_attempt_ordinal = Some(usize::MAX);
            }
            GeneratedResidualAffineResidualWorkKind::ExceptionalDomain { .. }
            | GeneratedResidualAffineResidualWorkKind::ExceptionalLeak { .. } => {
                leaf.when_bad = None;
            }
        }
        true
    }

    #[cfg(test)]
    pub(crate) fn test_only_validate_private_authorities(
        &self,
    ) -> Result<(), GeneratedResidualAffineGroupEffectiveCoverageError> {
        validate_arc_authorities(self)
    }
}

impl fmt::Debug for GeneratedResidualAffineGroupEffectiveCoverageCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineGroupEffectiveCoverageCertificate")
            .field("schema", &self.schema)
            .field("source_case_ordinal", &self.matcher.source_case_ordinal())
            .field("source_group_ordinal", &self.matcher.source_group_ordinal())
            .field("attempt_count", &self.attempts.len())
            .field("target_disposition_count", &self.target_dispositions.len())
            .field("sealed_rule_count", &self.sealed_rules.len())
            .field("residual_work_count", &self.residual_work.len())
            .field("private_matcher", &"<redacted>")
            .field("stats", &self.stats)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedResidualAffineLocalTransitionKind {
    Unsupported,
    IdenticallyBad,
    Certified,
}

pub(crate) struct GeneratedResidualAffineSequentialTargetState {
    consumed_by_group_position: Vec<bool>,
    consumed_count: usize,
}

impl GeneratedResidualAffineSequentialTargetState {
    pub(crate) fn try_with_group_size(
        group_size: usize,
    ) -> Result<Self, GeneratedResidualAffineGroupEffectiveCoverageError> {
        let mut consumed_by_group_position = Vec::new();
        try_reserve_exact(
            "group effective consumed target state",
            &mut consumed_by_group_position,
            group_size,
        )?;
        consumed_by_group_position.resize(group_size, false);
        Ok(Self {
            consumed_by_group_position,
            consumed_count: 0,
        })
    }

    pub(crate) const fn consumed_count(&self) -> usize {
        self.consumed_count
    }

    pub(crate) fn is_consumed_position(&self, target_position: usize) -> bool {
        self.consumed_by_group_position
            .get(target_position)
            .copied()
            .unwrap_or(false)
    }

    pub(crate) fn commit_selected(
        &mut self,
        target_case_ordinal: usize,
        target_position: usize,
        kind: GeneratedResidualAffineLocalTransitionKind,
    ) -> Result<bool, GeneratedResidualAffineGroupEffectiveCoverageError> {
        match kind {
            GeneratedResidualAffineLocalTransitionKind::Unsupported
            | GeneratedResidualAffineLocalTransitionKind::IdenticallyBad => Ok(false),
            GeneratedResidualAffineLocalTransitionKind::Certified => {
                let consumed = self
                    .consumed_by_group_position
                    .get_mut(target_position)
                    .ok_or(
                    GeneratedResidualAffineGroupEffectiveCoverageError::GroupCasePositionMismatch {
                        case_ordinal: target_case_ordinal,
                    },
                )?;
                if *consumed {
                    return Err(
                        GeneratedResidualAffineGroupEffectiveCoverageError::TargetAcceptedTwice {
                            target_case_ordinal,
                        },
                    );
                }
                let prospective_count = checked_add(
                    "group effective consumed target state",
                    self.consumed_count,
                    1,
                )?;
                *consumed = true;
                self.consumed_count = prospective_count;
                Ok(true)
            }
        }
    }
}

pub(crate) struct GeneratedResidualAffineGroupEffectiveCoverageCompiler;

impl GeneratedResidualAffineGroupEffectiveCoverageCompiler {
    pub(crate) fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        matcher: Arc<GeneratedResidualAffinePivotTargetMatchingCertificate>,
        limits: GeneratedResidualAffineGroupEffectiveCoverageLimits,
    ) -> Result<
        GeneratedResidualAffineGroupEffectiveCoverageCertificate,
        GeneratedResidualAffineGroupEffectiveCoverageError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            compile_inner(family, context, matcher, limits)
        }))
        .map_err(|_| {
            GeneratedResidualAffineGroupEffectiveCoverageError::SymbolicaPanic {
                stage: "transactional group compilation",
            }
        })?
    }
}

#[derive(Debug)]
pub(crate) enum GeneratedResidualAffineGroupEffectiveCoverageError {
    SchemaMismatch,
    SourceGroupOutOfRange {
        group_ordinal: usize,
    },
    SourceGroupOrdinalMismatch {
        expected: usize,
        actual: usize,
    },
    GroupCaseOutOfRange {
        case_ordinal: usize,
    },
    GroupCaseOrdinalMismatch {
        expected: usize,
        actual: usize,
    },
    GroupCaseMembershipMismatch {
        case_ordinal: usize,
    },
    GroupCasePositionMismatch {
        case_ordinal: usize,
    },
    PivotOrdinalMismatch {
        expected: usize,
        actual: usize,
    },
    RetainedTargetOutsideGroup {
        target_case_ordinal: usize,
    },
    SelectedTargetPositionMismatch {
        target_case_ordinal: usize,
    },
    LocalBindingMismatch {
        pivot_ordinal: usize,
        target_case_ordinal: usize,
    },
    LocalVariantMismatch {
        pivot_ordinal: usize,
    },
    TargetAcceptedTwice {
        target_case_ordinal: usize,
    },
    AcceptedStateMismatch {
        target_case_ordinal: usize,
    },
    StructuralLeafCensusMismatch {
        target_case_ordinal: usize,
    },
    ArcAuthorityMismatch {
        attempt_ordinal: usize,
    },
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
    ReplayMismatch,
    SymbolicaPanic {
        stage: &'static str,
    },
    Matcher(GeneratedResidualAffinePivotTargetMatchingError),
    Local(GeneratedResidualAffineWhenBadError),
}

impl fmt::Display for GeneratedResidualAffineGroupEffectiveCoverageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("group effective-coverage schema mismatch"),
            Self::SourceGroupOutOfRange { group_ordinal } => {
                write!(formatter, "source group {group_ordinal} is out of range")
            }
            Self::SourceGroupOrdinalMismatch { expected, actual } => write!(
                formatter,
                "source group ordinal mismatch: expected {expected}, got {actual}"
            ),
            Self::GroupCaseOutOfRange { case_ordinal } => {
                write!(formatter, "group case {case_ordinal} is out of range")
            }
            Self::GroupCaseOrdinalMismatch { expected, actual } => write!(
                formatter,
                "group case ordinal mismatch: expected {expected}, got {actual}"
            ),
            Self::GroupCaseMembershipMismatch { case_ordinal } => write!(
                formatter,
                "case {case_ordinal} is not in the matcher's exact group"
            ),
            Self::GroupCasePositionMismatch { case_ordinal } => write!(
                formatter,
                "case {case_ordinal} has inconsistent within-group position"
            ),
            Self::PivotOrdinalMismatch { expected, actual } => write!(
                formatter,
                "matcher pivot order mismatch: expected {expected}, got {actual}"
            ),
            Self::RetainedTargetOutsideGroup {
                target_case_ordinal,
            } => write!(
                formatter,
                "retained target {target_case_ordinal} lies outside the source group"
            ),
            Self::SelectedTargetPositionMismatch {
                target_case_ordinal,
            } => write!(
                formatter,
                "selected target {target_case_ordinal} has a mismatched retained list position"
            ),
            Self::LocalBindingMismatch {
                pivot_ordinal,
                target_case_ordinal,
            } => write!(
                formatter,
                "local binding mismatch for pivot {pivot_ordinal}, target {target_case_ordinal}"
            ),
            Self::LocalVariantMismatch { pivot_ordinal } => {
                write!(
                    formatter,
                    "local outcome variant mismatch at pivot {pivot_ordinal}"
                )
            }
            Self::TargetAcceptedTwice {
                target_case_ordinal,
            } => {
                write!(formatter, "target {target_case_ordinal} was accepted twice")
            }
            Self::AcceptedStateMismatch {
                target_case_ordinal,
            } => write!(
                formatter,
                "accepted state mismatch for target {target_case_ordinal}"
            ),
            Self::StructuralLeafCensusMismatch {
                target_case_ordinal,
            } => write!(
                formatter,
                "relative leaf census mismatch for target {target_case_ordinal}"
            ),
            Self::ArcAuthorityMismatch { attempt_ordinal } => write!(
                formatter,
                "shared local authority mismatch for attempt {attempt_ordinal}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "resource count overflow for {resource}")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "resource limit for {resource}: requested {requested}, limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "allocation failure for {resource}: requested {requested} elements"
            ),
            Self::ReplayMismatch => formatter.write_str("group effective-coverage replay mismatch"),
            Self::SymbolicaPanic { stage } => {
                write!(formatter, "Symbolica panicked during {stage}")
            }
            Self::Matcher(error) => write!(formatter, "matcher error: {error}"),
            Self::Local(error) => write!(formatter, "local affine WhenBad error: {error}"),
        }
    }
}

impl std::error::Error for GeneratedResidualAffineGroupEffectiveCoverageError {}

impl From<GeneratedResidualAffinePivotTargetMatchingError>
    for GeneratedResidualAffineGroupEffectiveCoverageError
{
    fn from(value: GeneratedResidualAffinePivotTargetMatchingError) -> Self {
        Self::Matcher(value)
    }
}

impl From<GeneratedResidualAffineWhenBadError>
    for GeneratedResidualAffineGroupEffectiveCoverageError
{
    fn from(value: GeneratedResidualAffineWhenBadError) -> Self {
        Self::Local(value)
    }
}

struct AcceptedTargetState {
    attempt_ordinal: usize,
    local: Arc<GeneratedResidualAffineWhenBadCompilation>,
}

#[derive(Clone, Copy)]
struct RejectedTargetTransition {
    target_references: usize,
    aggregate_references: usize,
    rejected_attempts: usize,
}

fn compile_inner(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    matcher: Arc<GeneratedResidualAffinePivotTargetMatchingCertificate>,
    limits: GeneratedResidualAffineGroupEffectiveCoverageLimits,
) -> Result<
    GeneratedResidualAffineGroupEffectiveCoverageCertificate,
    GeneratedResidualAffineGroupEffectiveCoverageError,
> {
    matcher.replay(family, context)?;
    let inventory = matcher.inventory();
    let group = inventory
        .groups()
        .get(matcher.source_group_ordinal())
        .ok_or(
            GeneratedResidualAffineGroupEffectiveCoverageError::SourceGroupOutOfRange {
                group_ordinal: matcher.source_group_ordinal(),
            },
        )?;
    if group.ordinal() != matcher.source_group_ordinal() {
        return Err(
            GeneratedResidualAffineGroupEffectiveCoverageError::SourceGroupOrdinalMismatch {
                expected: matcher.source_group_ordinal(),
                actual: group.ordinal(),
            },
        );
    }

    let mut stats = GeneratedResidualAffineGroupEffectiveCoverageStats::default();
    check_limit(
        "group effective matcher outcomes",
        matcher.outcomes().len(),
        limits.max_matcher_outcomes_inspected,
    )?;
    check_limit(
        "group effective target dispositions",
        group.case_ordinals().len(),
        limits.max_group_target_dispositions,
    )?;
    check_limit(
        "group effective group cases",
        group.case_ordinals().len(),
        limits.max_group_cases_inspected,
    )?;
    stats.group_cases_inspected = group.case_ordinals().len();
    for (position, &case_ordinal) in group.case_ordinals().iter().enumerate() {
        let case = inventory.cases().get(case_ordinal).ok_or(
            GeneratedResidualAffineGroupEffectiveCoverageError::GroupCaseOutOfRange {
                case_ordinal,
            },
        )?;
        if case.ordinal() != case_ordinal {
            return Err(
                GeneratedResidualAffineGroupEffectiveCoverageError::GroupCaseOrdinalMismatch {
                    expected: case_ordinal,
                    actual: case.ordinal(),
                },
            );
        }
        if case.group_ordinal() != group.ordinal() {
            return Err(
                GeneratedResidualAffineGroupEffectiveCoverageError::GroupCaseMembershipMismatch {
                    case_ordinal,
                },
            );
        }
        if case.ordinal_within_group() != position {
            return Err(
                GeneratedResidualAffineGroupEffectiveCoverageError::GroupCasePositionMismatch {
                    case_ordinal,
                },
            );
        }
    }

    let mut outer_retained_bytes =
        size_of::<GeneratedResidualAffineGroupEffectiveCoverageCertificate>();
    check_limit(
        "group effective outer retained bytes",
        outer_retained_bytes,
        limits.max_outer_retained_bytes,
    )?;
    let mut attempts = Vec::new();
    preflight_capacity_bytes::<GeneratedResidualAffineTargetAttempt>(
        outer_retained_bytes,
        matcher.outcomes().len(),
        limits.max_outer_retained_bytes,
    )?;
    try_reserve_exact(
        "group effective attempts",
        &mut attempts,
        matcher.outcomes().len(),
    )?;
    outer_retained_bytes = charge_capacity_bytes::<GeneratedResidualAffineTargetAttempt>(
        outer_retained_bytes,
        attempts.capacity(),
        limits.max_outer_retained_bytes,
    )?;

    let mut accepted_by_position = Vec::new();
    try_reserve_exact(
        "group effective accepted target state",
        &mut accepted_by_position,
        group.case_ordinals().len(),
    )?;
    accepted_by_position.resize_with(group.case_ordinals().len(), || None);
    let mut rejected_counts = Vec::new();
    try_reserve_exact(
        "group effective rejected target counts",
        &mut rejected_counts,
        group.case_ordinals().len(),
    )?;
    rejected_counts.resize(group.case_ordinals().len(), 0usize);
    let mut rejected_target_attempts = Vec::new();
    try_reserve_exact(
        "group effective rejected target references",
        &mut rejected_target_attempts,
        matcher.outcomes().len(),
    )?;
    let mut state = GeneratedResidualAffineSequentialTargetState::try_with_group_size(
        group.case_ordinals().len(),
    )?;

    for (attempt_ordinal, outcome) in matcher.outcomes().iter().enumerate() {
        if outcome.pivot_ordinal() != attempt_ordinal {
            return Err(
                GeneratedResidualAffineGroupEffectiveCoverageError::PivotOrdinalMismatch {
                    expected: attempt_ordinal,
                    actual: outcome.pivot_ordinal(),
                },
            );
        }
        stats.matcher_outcomes_inspected = bounded_add(
            "group effective matcher outcomes",
            stats.matcher_outcomes_inspected,
            1,
            limits.max_matcher_outcomes_inspected,
        )?;

        let (checked, matching) = match outcome {
            GeneratedResidualAffinePivotTargetOutcome::RejectedNoTargetCase(value) => {
                (value.checked_target_case_ordinals(), &[][..])
            }
            GeneratedResidualAffinePivotTargetOutcome::RejectedRecenteringBoundary(value) => (
                value.checked_target_case_ordinals(),
                value.matching_target_case_ordinals(),
            ),
            GeneratedResidualAffinePivotTargetOutcome::PendingAffineWhenBad(value) => (
                value.checked_target_case_ordinals(),
                value.matching_target_case_ordinals(),
            ),
        };
        validate_target_references(
            checked,
            inventory,
            group,
            &mut stats.checked_target_references,
            limits.max_checked_target_references,
        )?;
        validate_target_references(
            matching,
            inventory,
            group,
            &mut stats.matching_target_references,
            limits.max_matching_target_references,
        )?;

        let attempt = match outcome {
            GeneratedResidualAffinePivotTargetOutcome::RejectedNoTargetCase(_) => {
                stats.rejected_attempts = bounded_add(
                    "group effective rejected attempts",
                    stats.rejected_attempts,
                    1,
                    limits.max_rejected_attempts,
                )?;
                GeneratedResidualAffineTargetAttempt {
                    attempt_ordinal,
                    pivot_ordinal: attempt_ordinal,
                    selected_target_case_ordinal: None,
                    selected_target_position: None,
                    outcome: GeneratedResidualAffineTargetAttemptOutcome::MatcherRejectedNoTarget,
                }
            }
            GeneratedResidualAffinePivotTargetOutcome::RejectedRecenteringBoundary(_) => {
                stats.rejected_attempts = bounded_add(
                    "group effective rejected attempts",
                    stats.rejected_attempts,
                    1,
                    limits.max_rejected_attempts,
                )?;
                GeneratedResidualAffineTargetAttempt {
                    attempt_ordinal,
                    pivot_ordinal: attempt_ordinal,
                    selected_target_case_ordinal: None,
                    selected_target_position: None,
                    outcome:
                        GeneratedResidualAffineTargetAttemptOutcome::MatcherRejectedRecenteringBoundary,
                }
            }
            GeneratedResidualAffinePivotTargetOutcome::PendingAffineWhenBad(pending) => {
                stats.pending_target_selections = bounded_add(
                    "group effective pending target selections",
                    stats.pending_target_selections,
                    1,
                    limits.max_pending_target_selections,
                )?;
                let remaining_selection_references = remaining(
                    "group effective selection target references",
                    limits.max_selection_target_references_inspected,
                    stats.selection_target_references_inspected,
                )?;
                let selection = pending
                    .first_available_target_for_effective_coverage(
                        |case_ordinal| {
                            let position = inventory.cases()[case_ordinal].ordinal_within_group();
                            state.is_consumed_position(position)
                        },
                        remaining_selection_references,
                    )
                    .map_err(map_selection_error)?;
                let (target_case_ordinal, selected_target_position, references_inspected) =
                    match selection {
                        GeneratedResidualAffineEffectiveTargetSelection::Exhausted {
                            references_inspected,
                        } => {
                            stats.selection_target_references_inspected = bounded_add(
                                "group effective selection target references",
                                stats.selection_target_references_inspected,
                                references_inspected,
                                limits.max_selection_target_references_inspected,
                            )?;
                            stats.rejected_attempts = bounded_add(
                                "group effective rejected attempts",
                                stats.rejected_attempts,
                                1,
                                limits.max_rejected_attempts,
                            )?;
                            attempts.push(GeneratedResidualAffineTargetAttempt {
                                attempt_ordinal,
                                pivot_ordinal: attempt_ordinal,
                                selected_target_case_ordinal: None,
                                selected_target_position: None,
                                outcome:
                                    GeneratedResidualAffineTargetAttemptOutcome::NoRemainingTargetCase,
                            });
                            continue;
                        }
                        GeneratedResidualAffineEffectiveTargetSelection::Selected {
                            case_ordinal,
                            position,
                            references_inspected,
                        } => (case_ordinal, position, references_inspected),
                    };
                stats.selection_target_references_inspected = bounded_add(
                    "group effective selection target references",
                    stats.selection_target_references_inspected,
                    references_inspected,
                    limits.max_selection_target_references_inspected,
                )?;
                let target_position = target_position(target_case_ordinal, inventory, group)?;
                if pending
                    .matching_target_case_ordinals()
                    .get(selected_target_position)
                    != Some(&target_case_ordinal)
                {
                    return Err(
                        GeneratedResidualAffineGroupEffectiveCoverageError::SelectedTargetPositionMismatch {
                            target_case_ordinal,
                        },
                    );
                }
                let prospective_outer_retained_bytes = bounded_add(
                    "group effective outer retained bytes",
                    outer_retained_bytes,
                    arc_control_and_padding_bytes::<GeneratedResidualAffineWhenBadCompilation>()?,
                    limits.max_outer_retained_bytes,
                )?;
                let prospective_local_compilations = bounded_add(
                    "group effective local WhenBad compilations",
                    stats.local_when_bad_compilations,
                    1,
                    limits.max_local_when_bad_compilations,
                )?;
                let child_limits = projected_child_limits(limits, stats)?;
                let local = GeneratedResidualAffineWhenBadCompiler::compile(
                    family,
                    context,
                    matcher.clone(),
                    attempt_ordinal,
                    target_case_ordinal,
                    child_limits,
                )?;
                if local.binding().pivot_ordinal() != attempt_ordinal
                    || local.binding().target_case_ordinal() != target_case_ordinal
                    || local.binding().target_position_in_matching_list()
                        != selected_target_position
                {
                    return Err(
                        GeneratedResidualAffineGroupEffectiveCoverageError::LocalBindingMismatch {
                            pivot_ordinal: attempt_ordinal,
                            target_case_ordinal,
                        },
                    );
                }
                charge_child_usage(&mut stats, local.group_resource_usage(), limits)?;
                stats.local_when_bad_compilations = prospective_local_compilations;
                let kind = match local {
                    GeneratedResidualAffineWhenBadCompilation::Certified(_) => {
                        GeneratedResidualAffineLocalTransitionKind::Certified
                    }
                    GeneratedResidualAffineWhenBadCompilation::IdenticallyBad(_) => {
                        GeneratedResidualAffineLocalTransitionKind::IdenticallyBad
                    }
                    GeneratedResidualAffineWhenBadCompilation::Unsupported(_) => {
                        GeneratedResidualAffineLocalTransitionKind::Unsupported
                    }
                };
                let mut prospective_accepted_attempts = None;
                let mut prospective_consumed_targets = None;
                let mut rejected_transition = None;
                match kind {
                    GeneratedResidualAffineLocalTransitionKind::Certified => {
                        if state.is_consumed_position(target_position)
                            || accepted_by_position[target_position].is_some()
                        {
                            return Err(
                                GeneratedResidualAffineGroupEffectiveCoverageError::AcceptedStateMismatch {
                                    target_case_ordinal,
                                },
                            );
                        }
                        prospective_accepted_attempts = Some(bounded_add(
                            "group effective accepted attempts",
                            stats.accepted_attempts,
                            1,
                            limits.max_accepted_attempts,
                        )?);
                        prospective_consumed_targets = Some(bounded_add(
                            "group effective consumed targets",
                            stats.consumed_targets,
                            1,
                            limits.max_consumed_targets,
                        )?);
                    }
                    GeneratedResidualAffineLocalTransitionKind::IdenticallyBad
                    | GeneratedResidualAffineLocalTransitionKind::Unsupported => {
                        rejected_transition = Some(preflight_rejected_target(
                            target_position,
                            &rejected_counts,
                            stats,
                            limits,
                        )?);
                    }
                }
                let local = Arc::new(local);
                outer_retained_bytes = prospective_outer_retained_bytes;
                let consumed = state.commit_selected(target_case_ordinal, target_position, kind)?;
                let attempt_outcome = match kind {
                    GeneratedResidualAffineLocalTransitionKind::Certified => {
                        if !consumed || accepted_by_position[target_position].is_some() {
                            return Err(
                                GeneratedResidualAffineGroupEffectiveCoverageError::AcceptedStateMismatch {
                                    target_case_ordinal,
                                },
                            );
                        }
                        stats.accepted_attempts = prospective_accepted_attempts.ok_or(
                            GeneratedResidualAffineGroupEffectiveCoverageError::AcceptedStateMismatch {
                                target_case_ordinal,
                            },
                        )?;
                        stats.consumed_targets = prospective_consumed_targets.ok_or(
                            GeneratedResidualAffineGroupEffectiveCoverageError::AcceptedStateMismatch {
                                target_case_ordinal,
                            },
                        )?;
                        accepted_by_position[target_position] = Some(AcceptedTargetState {
                            attempt_ordinal,
                            local: local.clone(),
                        });
                        GeneratedResidualAffineTargetAttemptOutcome::Accepted(local)
                    }
                    GeneratedResidualAffineLocalTransitionKind::IdenticallyBad => {
                        if consumed {
                            return Err(
                                GeneratedResidualAffineGroupEffectiveCoverageError::AcceptedStateMismatch {
                                    target_case_ordinal,
                                },
                            );
                        }
                        commit_rejected_target(
                            target_position,
                            attempt_ordinal,
                            &mut rejected_counts,
                            &mut rejected_target_attempts,
                            &mut stats,
                            rejected_transition.ok_or(
                                GeneratedResidualAffineGroupEffectiveCoverageError::AcceptedStateMismatch {
                                    target_case_ordinal,
                                },
                            )?,
                        )?;
                        GeneratedResidualAffineTargetAttemptOutcome::WhenBadIdenticallyBad(local)
                    }
                    GeneratedResidualAffineLocalTransitionKind::Unsupported => {
                        if consumed {
                            return Err(
                                GeneratedResidualAffineGroupEffectiveCoverageError::AcceptedStateMismatch {
                                    target_case_ordinal,
                                },
                            );
                        }
                        commit_rejected_target(
                            target_position,
                            attempt_ordinal,
                            &mut rejected_counts,
                            &mut rejected_target_attempts,
                            &mut stats,
                            rejected_transition.ok_or(
                                GeneratedResidualAffineGroupEffectiveCoverageError::AcceptedStateMismatch {
                                    target_case_ordinal,
                                },
                            )?,
                        )?;
                        GeneratedResidualAffineTargetAttemptOutcome::WhenBadUnsupported(local)
                    }
                };
                GeneratedResidualAffineTargetAttempt {
                    attempt_ordinal,
                    pivot_ordinal: attempt_ordinal,
                    selected_target_case_ordinal: Some(target_case_ordinal),
                    selected_target_position: Some(selected_target_position),
                    outcome: attempt_outcome,
                }
            }
        };
        attempts.push(attempt);
    }

    if attempts.len() != matcher.outcomes().len()
        || checked_add(
            "group effective attempts",
            stats.accepted_attempts,
            stats.rejected_attempts,
        )? != attempts.len()
        || stats.consumed_targets != state.consumed_count()
    {
        return Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch);
    }

    let (sealed_count, exceptional_count) = census_accepted_leaves(&accepted_by_position)?;
    let unconsumed_roots = accepted_by_position
        .iter()
        .filter(|entry| entry.is_none())
        .count();
    let residual_count = checked_add(
        "group effective residual work leaves",
        unconsumed_roots,
        exceptional_count,
    )?;
    check_limit(
        "group effective sealed conditional-rule handles",
        sealed_count,
        limits.max_sealed_conditional_rule_handles,
    )?;
    check_limit(
        "group effective residual work leaves",
        residual_count,
        limits.max_residual_work_leaves,
    )?;

    let mut target_dispositions = Vec::new();
    preflight_capacity_bytes::<GeneratedResidualAffineGroupTargetDispositionRecord>(
        outer_retained_bytes,
        group.case_ordinals().len(),
        limits.max_outer_retained_bytes,
    )?;
    try_reserve_exact(
        "group effective target dispositions",
        &mut target_dispositions,
        group.case_ordinals().len(),
    )?;
    outer_retained_bytes =
        charge_capacity_bytes::<GeneratedResidualAffineGroupTargetDispositionRecord>(
            outer_retained_bytes,
            target_dispositions.capacity(),
            limits.max_outer_retained_bytes,
        )?;
    let mut sealed_rules = Vec::new();
    preflight_capacity_bytes::<GeneratedResidualAffineSealedConditionalRuleHandle>(
        outer_retained_bytes,
        sealed_count,
        limits.max_outer_retained_bytes,
    )?;
    try_reserve_exact(
        "group effective sealed conditional-rule handles",
        &mut sealed_rules,
        sealed_count,
    )?;
    outer_retained_bytes =
        charge_capacity_bytes::<GeneratedResidualAffineSealedConditionalRuleHandle>(
            outer_retained_bytes,
            sealed_rules.capacity(),
            limits.max_outer_retained_bytes,
        )?;
    let mut residual_work = Vec::new();
    preflight_capacity_bytes::<GeneratedResidualAffineResidualWorkLeaf>(
        outer_retained_bytes,
        residual_count,
        limits.max_outer_retained_bytes,
    )?;
    try_reserve_exact(
        "group effective residual work leaves",
        &mut residual_work,
        residual_count,
    )?;
    outer_retained_bytes = charge_capacity_bytes::<GeneratedResidualAffineResidualWorkLeaf>(
        outer_retained_bytes,
        residual_work.capacity(),
        limits.max_outer_retained_bytes,
    )?;
    let mut rejected_attempt_ordinals_by_position = distribute_unconsumed_rejected_attempts(
        &state.consumed_by_group_position,
        &rejected_counts,
        &rejected_target_attempts,
        &mut outer_retained_bytes,
        limits.max_outer_retained_bytes,
    )?;

    for (target_position, &target_case_ordinal) in group.case_ordinals().iter().enumerate() {
        let target = &inventory.cases()[target_case_ordinal];
        let target_locator = target.locator();
        if let Some(accepted) = &accepted_by_position[target_position] {
            let GeneratedResidualAffineWhenBadCompilation::Certified(certificate) =
                accepted.local.as_ref()
            else {
                return Err(
                    GeneratedResidualAffineGroupEffectiveCoverageError::LocalVariantMismatch {
                        pivot_ordinal: accepted.attempt_ordinal,
                    },
                );
            };
            target_dispositions.push(GeneratedResidualAffineGroupTargetDispositionRecord {
                target_case_ordinal,
                target_locator,
                disposition: GeneratedResidualAffineGroupTargetDisposition::Consumed {
                    accepted_attempt_ordinal: accepted.attempt_ordinal,
                    when_bad: accepted.local.clone(),
                },
            });
            for (leaf_ordinal, leaf) in certificate.leaf_classifications().iter().enumerate() {
                match leaf.disposition() {
                    AffineWhenBadRelativeLeafDisposition::Applicable => {
                        sealed_rules.push(GeneratedResidualAffineSealedConditionalRuleHandle {
                            accepted_attempt_ordinal: accepted.attempt_ordinal,
                            pivot_ordinal: certificate.binding().pivot_ordinal(),
                            target_case_ordinal,
                            target_locator,
                            leaf_ordinal,
                            relative_case: leaf.case(),
                            when_bad: accepted.local.clone(),
                        });
                    }
                    AffineWhenBadRelativeLeafDisposition::ExceptionalDomain {
                        condition_ordinal,
                    } => {
                        residual_work.push(GeneratedResidualAffineResidualWorkLeaf {
                            target_case_ordinal,
                            target_locator,
                            accepted_attempt_ordinal: Some(accepted.attempt_ordinal),
                            leaf_ordinal: Some(leaf_ordinal),
                            relative_case: Some(leaf.case()),
                            kind: GeneratedResidualAffineResidualWorkKind::ExceptionalDomain {
                                condition_ordinal,
                            },
                            when_bad: Some(accepted.local.clone()),
                        });
                    }
                    AffineWhenBadRelativeLeafDisposition::ExceptionalLeak { pullback_ordinal } => {
                        residual_work.push(GeneratedResidualAffineResidualWorkLeaf {
                            target_case_ordinal,
                            target_locator,
                            accepted_attempt_ordinal: Some(accepted.attempt_ordinal),
                            leaf_ordinal: Some(leaf_ordinal),
                            relative_case: Some(leaf.case()),
                            kind: GeneratedResidualAffineResidualWorkKind::ExceptionalLeak {
                                pullback_ordinal,
                            },
                            when_bad: Some(accepted.local.clone()),
                        });
                    }
                }
            }
        } else {
            let rejected_count = rejected_counts[target_position];
            let rejected_attempt_ordinals =
                std::mem::take(&mut rejected_attempt_ordinals_by_position[target_position]);
            if rejected_attempt_ordinals.len() != rejected_count {
                return Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch);
            }
            target_dispositions.push(GeneratedResidualAffineGroupTargetDispositionRecord {
                target_case_ordinal,
                target_locator,
                disposition: GeneratedResidualAffineGroupTargetDisposition::Unconsumed {
                    rejected_attempt_ordinals,
                },
            });
            residual_work.push(GeneratedResidualAffineResidualWorkLeaf {
                target_case_ordinal,
                target_locator,
                accepted_attempt_ordinal: None,
                leaf_ordinal: None,
                relative_case: None,
                kind: GeneratedResidualAffineResidualWorkKind::CompleteTargetRoot,
                when_bad: None,
            });
        }
    }

    if target_dispositions.len() != group.case_ordinals().len()
        || sealed_rules.len() != sealed_count
        || residual_work.len() != residual_count
    {
        return Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch);
    }
    stats.group_target_dispositions = target_dispositions.len();
    stats.sealed_conditional_rule_handles = sealed_rules.len();
    stats.unconsumed_residual_roots = unconsumed_roots;
    stats.exceptional_residual_leaves = exceptional_count;
    stats.residual_work_leaves = residual_work.len();
    stats.outer_retained_bytes = outer_retained_bytes;
    let outer_payload = outer_payload_census(
        &attempts,
        &target_dispositions,
        &sealed_rules,
        &residual_work,
        outer_retained_bytes,
    )?;
    check_outer_payload_limits(outer_payload, limits)?;
    stats.outer_payload_comparison_units = outer_payload.units;
    stats.outer_payload_comparison_bytes = outer_payload.bytes;
    stats.outer_payload_comparison_integer_bits = outer_payload.integer_bits;

    let certificate = GeneratedResidualAffineGroupEffectiveCoverageCertificate {
        schema: GENERATED_RESIDUAL_AFFINE_GROUP_EFFECTIVE_COVERAGE_V1_SCHEMA,
        matcher,
        attempts,
        target_dispositions,
        sealed_rules,
        residual_work,
        limits,
        stats,
    };
    validate_arc_authorities(&certificate)?;
    Ok(certificate)
}

fn validate_target_references(
    references: &[usize],
    inventory: &GeneratedResidualAffineCaseInventoryCertificate,
    group: &GeneratedResidualAffineContiguousCaseGroup,
    observed: &mut usize,
    limit: usize,
) -> Result<(), GeneratedResidualAffineGroupEffectiveCoverageError> {
    for &target_case_ordinal in references {
        let prospective = checked_add("group effective target references", *observed, 1)?;
        check_limit("group effective target references", prospective, limit)?;
        target_position(target_case_ordinal, inventory, group)?;
        *observed = prospective;
    }
    Ok(())
}

fn target_position(
    target_case_ordinal: usize,
    inventory: &GeneratedResidualAffineCaseInventoryCertificate,
    group: &GeneratedResidualAffineContiguousCaseGroup,
) -> Result<usize, GeneratedResidualAffineGroupEffectiveCoverageError> {
    let target = inventory.cases().get(target_case_ordinal).ok_or(
        GeneratedResidualAffineGroupEffectiveCoverageError::RetainedTargetOutsideGroup {
            target_case_ordinal,
        },
    )?;
    if target.ordinal() != target_case_ordinal || target.group_ordinal() != group.ordinal() {
        return Err(
            GeneratedResidualAffineGroupEffectiveCoverageError::RetainedTargetOutsideGroup {
                target_case_ordinal,
            },
        );
    }
    let position = target.ordinal_within_group();
    if group.case_ordinals().get(position) != Some(&target_case_ordinal) {
        return Err(
            GeneratedResidualAffineGroupEffectiveCoverageError::GroupCasePositionMismatch {
                case_ordinal: target_case_ordinal,
            },
        );
    }
    Ok(position)
}

fn map_selection_error(
    error: GeneratedResidualAffineEffectiveTargetSelectionError,
) -> GeneratedResidualAffineGroupEffectiveCoverageError {
    match error {
        GeneratedResidualAffineEffectiveTargetSelectionError::ResourceCountOverflow => {
            GeneratedResidualAffineGroupEffectiveCoverageError::ResourceCountOverflow {
                resource: "group effective selection target references",
            }
        }
        GeneratedResidualAffineEffectiveTargetSelectionError::ResourceLimit {
            requested,
            limit,
        } => GeneratedResidualAffineGroupEffectiveCoverageError::ResourceLimit {
            resource: "group effective selection target references",
            requested,
            limit,
        },
    }
}

fn projected_child_limits(
    group: GeneratedResidualAffineGroupEffectiveCoverageLimits,
    stats: GeneratedResidualAffineGroupEffectiveCoverageStats,
) -> Result<GeneratedResidualAffineWhenBadLimits, GeneratedResidualAffineGroupEffectiveCoverageError>
{
    let mut child = group.local_when_bad;
    child.max_total_source_terms = child.max_total_source_terms.min(remaining(
        "group effective child source terms",
        group.max_child_source_terms,
        stats.child_source_terms,
    )?);
    child.max_total_source_exponent_entries =
        child.max_total_source_exponent_entries.min(remaining(
            "group effective child source exponent entries",
            group.max_child_source_exponent_entries,
            stats.child_source_exponent_entries,
        )?);
    child.max_total_source_integer_bits = child.max_total_source_integer_bits.min(remaining(
        "group effective child source integer bits",
        group.max_child_source_integer_bits,
        stats.child_source_integer_bits,
    )?);
    child.max_total_output_terms = child.max_total_output_terms.min(remaining(
        "group effective child output terms",
        group.max_child_output_terms,
        stats.child_output_terms,
    )?);
    child.max_total_output_exponent_entries =
        child.max_total_output_exponent_entries.min(remaining(
            "group effective child output exponent entries",
            group.max_child_output_exponent_entries,
            stats.child_output_exponent_entries,
        )?);
    child.max_total_native_integer_bit_work =
        child.max_total_native_integer_bit_work.min(remaining(
            "group effective child native integer-bit work",
            group.max_child_native_integer_bit_work,
            stats.child_native_integer_bit_work,
        )?);
    child.max_total_integer_bit_work = child.max_total_integer_bit_work.min(remaining(
        "group effective child total integer-bit work",
        group.max_child_total_integer_bit_work,
        stats.child_total_integer_bit_work,
    )?);
    child.max_payload_comparison_units = child.max_payload_comparison_units.min(remaining(
        "group effective child payload comparison units",
        group.max_child_payload_comparison_units,
        stats.child_payload_comparison_units,
    )?);
    child.max_payload_comparison_bytes = child.max_payload_comparison_bytes.min(remaining(
        "group effective child payload comparison bytes",
        group.max_child_payload_comparison_bytes,
        stats.child_payload_comparison_bytes,
    )?);
    child.max_payload_comparison_integer_bits =
        child.max_payload_comparison_integer_bits.min(remaining(
            "group effective child payload comparison integer bits",
            group.max_child_payload_comparison_integer_bits,
            stats.child_payload_comparison_integer_bits,
        )?);
    child.max_payload_comparison_private_manifest_bytes = child
        .max_payload_comparison_private_manifest_bytes
        .min(remaining(
            "group effective child private-manifest comparison bytes",
            group.max_child_payload_comparison_private_manifest_bytes,
            stats.child_payload_comparison_private_manifest_bytes,
        )?);
    child.max_structural_loci = child.max_structural_loci.min(remaining(
        "group effective child structural loci",
        group.max_child_structural_loci,
        stats.child_structural_loci,
    )?);
    child.max_bad_clauses = child.max_bad_clauses.min(remaining(
        "group effective child bad clauses",
        group.max_child_bad_clauses,
        stats.child_bad_clauses,
    )?);
    child.relative_partition.max_leaf_classifications = child
        .relative_partition
        .max_leaf_classifications
        .min(remaining(
            "group effective child relative leaves",
            group.max_child_relative_leaves,
            checked_add(
                "group effective child relative leaves",
                stats.child_applicable_leaves,
                stats.child_exceptional_leaves,
            )?,
        )?);
    child.max_retained_bytes = child.max_retained_bytes.min(remaining(
        "group effective child retained bytes",
        group.max_child_retained_bytes,
        stats.child_retained_bytes,
    )?);
    Ok(child)
}

fn charge_child_usage(
    stats: &mut GeneratedResidualAffineGroupEffectiveCoverageStats,
    usage: GeneratedResidualAffineWhenBadGroupResourceUsage,
    limits: GeneratedResidualAffineGroupEffectiveCoverageLimits,
) -> Result<(), GeneratedResidualAffineGroupEffectiveCoverageError> {
    macro_rules! charge {
        ($field:ident, $usage:ident, $limit:ident, $resource:literal) => {
            stats.$field = bounded_add($resource, stats.$field, usage.$usage, limits.$limit)?;
        };
    }
    charge!(
        child_source_terms,
        source_terms,
        max_child_source_terms,
        "group effective child source terms"
    );
    charge!(
        child_source_exponent_entries,
        source_exponent_entries,
        max_child_source_exponent_entries,
        "group effective child source exponent entries"
    );
    charge!(
        child_source_integer_bits,
        source_integer_bits,
        max_child_source_integer_bits,
        "group effective child source integer bits"
    );
    charge!(
        child_output_terms,
        output_terms,
        max_child_output_terms,
        "group effective child output terms"
    );
    charge!(
        child_output_exponent_entries,
        output_exponent_entries,
        max_child_output_exponent_entries,
        "group effective child output exponent entries"
    );
    charge!(
        child_native_integer_bit_work,
        native_integer_bit_work,
        max_child_native_integer_bit_work,
        "group effective child native integer-bit work"
    );
    charge!(
        child_total_integer_bit_work,
        total_integer_bit_work,
        max_child_total_integer_bit_work,
        "group effective child total integer-bit work"
    );
    // The aggregate payload counters already include the assembly subset
    // exactly once. The separate assembly fields below are diagnostic only.
    charge!(
        child_payload_comparison_units,
        payload_comparison_units,
        max_child_payload_comparison_units,
        "group effective child payload comparison units"
    );
    charge!(
        child_payload_comparison_bytes,
        payload_comparison_bytes,
        max_child_payload_comparison_bytes,
        "group effective child payload comparison bytes"
    );
    charge!(
        child_payload_comparison_integer_bits,
        payload_comparison_integer_bits,
        max_child_payload_comparison_integer_bits,
        "group effective child payload comparison integer bits"
    );
    charge!(
        child_payload_comparison_private_manifest_bytes,
        payload_comparison_private_manifest_bytes,
        max_child_payload_comparison_private_manifest_bytes,
        "group effective child private-manifest comparison bytes"
    );
    stats.child_assembly_payload_comparison_units = checked_add(
        "group effective child assembly payload comparison units",
        stats.child_assembly_payload_comparison_units,
        usage.assembly_payload_comparison_units,
    )?;
    stats.child_assembly_payload_comparison_bytes = checked_add(
        "group effective child assembly payload comparison bytes",
        stats.child_assembly_payload_comparison_bytes,
        usage.assembly_payload_comparison_bytes,
    )?;
    charge!(
        child_structural_loci,
        structural_loci,
        max_child_structural_loci,
        "group effective child structural loci"
    );
    charge!(
        child_bad_clauses,
        bad_clauses,
        max_child_bad_clauses,
        "group effective child bad clauses"
    );
    let leaves_before = checked_add(
        "group effective child relative leaves",
        stats.child_applicable_leaves,
        stats.child_exceptional_leaves,
    )?;
    let leaves_added = checked_add(
        "group effective child relative leaves",
        usage.applicable_leaves,
        usage.exceptional_leaves,
    )?;
    check_limit(
        "group effective child relative leaves",
        checked_add(
            "group effective child relative leaves",
            leaves_before,
            leaves_added,
        )?,
        limits.max_child_relative_leaves,
    )?;
    stats.child_applicable_leaves = checked_add(
        "group effective child applicable leaves",
        stats.child_applicable_leaves,
        usage.applicable_leaves,
    )?;
    stats.child_exceptional_leaves = checked_add(
        "group effective child exceptional leaves",
        stats.child_exceptional_leaves,
        usage.exceptional_leaves,
    )?;
    charge!(
        child_retained_bytes,
        retained_bytes,
        max_child_retained_bytes,
        "group effective child retained bytes"
    );
    Ok(())
}

fn preflight_rejected_target(
    target_position: usize,
    rejected_counts: &[usize],
    stats: GeneratedResidualAffineGroupEffectiveCoverageStats,
    limits: GeneratedResidualAffineGroupEffectiveCoverageLimits,
) -> Result<RejectedTargetTransition, GeneratedResidualAffineGroupEffectiveCoverageError> {
    let target_references = checked_add(
        "group effective rejected-attempt references per target",
        rejected_counts[target_position],
        1,
    )?;
    check_limit(
        "group effective rejected-attempt references per target",
        target_references,
        limits.max_rejected_attempt_references_per_target,
    )?;
    let aggregate_references = bounded_add(
        "group effective rejected-attempt references",
        stats.rejected_attempt_references,
        1,
        limits.max_rejected_attempt_references,
    )?;
    let rejected_attempts = bounded_add(
        "group effective rejected attempts",
        stats.rejected_attempts,
        1,
        limits.max_rejected_attempts,
    )?;
    Ok(RejectedTargetTransition {
        target_references,
        aggregate_references,
        rejected_attempts,
    })
}

#[allow(clippy::too_many_arguments)]
fn commit_rejected_target(
    target_position: usize,
    attempt_ordinal: usize,
    rejected_counts: &mut [usize],
    rejected_target_attempts: &mut Vec<(usize, usize)>,
    stats: &mut GeneratedResidualAffineGroupEffectiveCoverageStats,
    transition: RejectedTargetTransition,
) -> Result<(), GeneratedResidualAffineGroupEffectiveCoverageError> {
    if rejected_target_attempts.len() == rejected_target_attempts.capacity() {
        return Err(
            GeneratedResidualAffineGroupEffectiveCoverageError::AllocationFailure {
                resource: "group effective rejected target references",
                requested: 1,
            },
        );
    }
    rejected_target_attempts.push((target_position, attempt_ordinal));
    rejected_counts[target_position] = transition.target_references;
    stats.rejected_attempt_references = transition.aggregate_references;
    stats.rejected_attempts = transition.rejected_attempts;
    stats.maximum_rejected_attempt_references_per_target = stats
        .maximum_rejected_attempt_references_per_target
        .max(transition.target_references);
    Ok(())
}

pub(crate) fn distribute_unconsumed_rejected_attempts(
    consumed_by_group_position: &[bool],
    rejected_counts: &[usize],
    rejected_target_attempts: &[(usize, usize)],
    outer_retained_bytes: &mut usize,
    max_outer_retained_bytes: usize,
) -> Result<Vec<Vec<usize>>, GeneratedResidualAffineGroupEffectiveCoverageError> {
    if consumed_by_group_position.len() != rejected_counts.len() {
        return Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch);
    }
    let retained_reference_count = consumed_by_group_position
        .iter()
        .zip(rejected_counts)
        .try_fold(0usize, |total, (&consumed, &count)| {
            if consumed {
                Ok(total)
            } else {
                checked_add(
                    "group effective unconsumed rejected-attempt references",
                    total,
                    count,
                )
            }
        })?;
    preflight_capacity_bytes::<usize>(
        *outer_retained_bytes,
        retained_reference_count,
        max_outer_retained_bytes,
    )?;

    let group_size = rejected_counts.len();
    let mut grouped = Vec::new();
    try_reserve_exact(
        "group effective rejected-attempt groups",
        &mut grouped,
        group_size,
    )?;
    grouped.resize_with(group_size, Vec::new);
    let mut seen_counts = Vec::new();
    try_reserve_exact(
        "group effective rejected-attempt seen counts",
        &mut seen_counts,
        group_size,
    )?;
    seen_counts.resize(group_size, 0usize);

    for (target_position, (&consumed, &count)) in consumed_by_group_position
        .iter()
        .zip(rejected_counts)
        .enumerate()
    {
        if consumed {
            continue;
        }
        preflight_capacity_bytes::<usize>(*outer_retained_bytes, count, max_outer_retained_bytes)?;
        try_reserve_exact(
            "group effective unconsumed rejected-attempt references",
            &mut grouped[target_position],
            count,
        )?;
        *outer_retained_bytes = charge_capacity_bytes::<usize>(
            *outer_retained_bytes,
            grouped[target_position].capacity(),
            max_outer_retained_bytes,
        )?;
    }

    for &(target_position, attempt_ordinal) in rejected_target_attempts {
        let seen = seen_counts
            .get_mut(target_position)
            .ok_or(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch)?;
        *seen = checked_add(
            "group effective rejected-attempt references per target",
            *seen,
            1,
        )?;
        if !consumed_by_group_position[target_position] {
            let target = &mut grouped[target_position];
            if target.len() == target.capacity() {
                return Err(
                    GeneratedResidualAffineGroupEffectiveCoverageError::AllocationFailure {
                        resource: "group effective unconsumed rejected-attempt references",
                        requested: 1,
                    },
                );
            }
            target.push(attempt_ordinal);
        }
    }

    for target_position in 0..group_size {
        if seen_counts[target_position] != rejected_counts[target_position]
            || (!consumed_by_group_position[target_position]
                && grouped[target_position].len() != rejected_counts[target_position])
            || (consumed_by_group_position[target_position] && !grouped[target_position].is_empty())
        {
            return Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch);
        }
    }
    Ok(grouped)
}

fn census_accepted_leaves(
    accepted: &[Option<AcceptedTargetState>],
) -> Result<(usize, usize), GeneratedResidualAffineGroupEffectiveCoverageError> {
    let mut applicable = 0usize;
    let mut exceptional = 0usize;
    for entry in accepted.iter().flatten() {
        let GeneratedResidualAffineWhenBadCompilation::Certified(certificate) =
            entry.local.as_ref()
        else {
            return Err(
                GeneratedResidualAffineGroupEffectiveCoverageError::LocalVariantMismatch {
                    pivot_ordinal: entry.attempt_ordinal,
                },
            );
        };
        let mut local_applicable = 0usize;
        let mut local_exceptional = 0usize;
        for leaf in certificate.leaf_classifications() {
            match leaf.disposition() {
                AffineWhenBadRelativeLeafDisposition::Applicable => {
                    local_applicable =
                        checked_add("group effective applicable leaves", local_applicable, 1)?;
                }
                AffineWhenBadRelativeLeafDisposition::ExceptionalDomain { .. }
                | AffineWhenBadRelativeLeafDisposition::ExceptionalLeak { .. } => {
                    local_exceptional =
                        checked_add("group effective exceptional leaves", local_exceptional, 1)?;
                }
            }
        }
        let usage = entry.local.group_resource_usage();
        if local_applicable != usage.applicable_leaves
            || local_exceptional != usage.exceptional_leaves
        {
            return Err(
                GeneratedResidualAffineGroupEffectiveCoverageError::StructuralLeafCensusMismatch {
                    target_case_ordinal: certificate.binding().target_case_ordinal(),
                },
            );
        }
        applicable = checked_add(
            "group effective applicable leaves",
            applicable,
            local_applicable,
        )?;
        exceptional = checked_add(
            "group effective exceptional leaves",
            exceptional,
            local_exceptional,
        )?;
    }
    Ok((applicable, exceptional))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct OuterPayloadCensus {
    units: usize,
    bytes: usize,
    integer_bits: usize,
}

fn outer_payload_census(
    attempts: &[GeneratedResidualAffineTargetAttempt],
    dispositions: &[GeneratedResidualAffineGroupTargetDispositionRecord],
    sealed_rules: &[GeneratedResidualAffineSealedConditionalRuleHandle],
    residual_work: &[GeneratedResidualAffineResidualWorkLeaf],
    retained_bytes: usize,
) -> Result<OuterPayloadCensus, GeneratedResidualAffineGroupEffectiveCoverageError> {
    let rejected_references = dispositions
        .iter()
        .map(|record| match &record.disposition {
            GeneratedResidualAffineGroupTargetDisposition::Consumed { .. } => 0,
            GeneratedResidualAffineGroupTargetDisposition::Unconsumed {
                rejected_attempt_ordinals,
            } => rejected_attempt_ordinals.len(),
        })
        .try_fold(0usize, |sum, value| {
            checked_add("group effective outer payload comparison units", sum, value)
        })?;
    let units = checked_add(
        "group effective outer payload comparison units",
        8,
        checked_add(
            "group effective outer payload comparison units",
            checked_mul(
                "group effective outer payload comparison units",
                attempts.len(),
                7,
            )?,
            checked_add(
                "group effective outer payload comparison units",
                checked_mul(
                    "group effective outer payload comparison units",
                    dispositions.len(),
                    6,
                )?,
                checked_add(
                    "group effective outer payload comparison units",
                    rejected_references,
                    checked_add(
                        "group effective outer payload comparison units",
                        checked_mul(
                            "group effective outer payload comparison units",
                            sealed_rules.len(),
                            7,
                        )?,
                        checked_mul(
                            "group effective outer payload comparison units",
                            residual_work.len(),
                            7,
                        )?,
                    )?,
                )?,
            )?,
        )?,
    )?;
    let bytes = checked_mul(
        "group effective outer payload comparison bytes",
        retained_bytes,
        2,
    )?;
    let integer_bits = checked_mul(
        "group effective outer payload comparison integer bits",
        units,
        usize::BITS as usize,
    )?;
    Ok(OuterPayloadCensus {
        units,
        bytes,
        integer_bits,
    })
}

fn check_outer_payload_limits(
    census: OuterPayloadCensus,
    limits: GeneratedResidualAffineGroupEffectiveCoverageLimits,
) -> Result<(), GeneratedResidualAffineGroupEffectiveCoverageError> {
    check_limit(
        "group effective outer payload comparison units",
        census.units,
        limits.max_outer_payload_comparison_units,
    )?;
    check_limit(
        "group effective outer payload comparison bytes",
        census.bytes,
        limits.max_outer_payload_comparison_bytes,
    )?;
    check_limit(
        "group effective outer payload comparison integer bits",
        census.integer_bits,
        limits.max_outer_payload_comparison_integer_bits,
    )
}

fn validate_arc_authorities(
    certificate: &GeneratedResidualAffineGroupEffectiveCoverageCertificate,
) -> Result<(), GeneratedResidualAffineGroupEffectiveCoverageError> {
    let inventory = certificate.matcher.inventory();
    let group = inventory
        .groups()
        .get(certificate.matcher.source_group_ordinal())
        .ok_or(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch)?;
    if certificate.attempts.len() != certificate.matcher.outcomes().len()
        || certificate.target_dispositions.len() != group.case_ordinals().len()
    {
        return Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch);
    }

    for (attempt_ordinal, (attempt, matcher_outcome)) in certificate
        .attempts
        .iter()
        .zip(certificate.matcher.outcomes())
        .enumerate()
    {
        if attempt.attempt_ordinal != attempt_ordinal || attempt.pivot_ordinal != attempt_ordinal {
            return Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch);
        }
        match (&attempt.outcome, matcher_outcome) {
            (
                GeneratedResidualAffineTargetAttemptOutcome::MatcherRejectedNoTarget,
                GeneratedResidualAffinePivotTargetOutcome::RejectedNoTargetCase(_),
            )
            | (
                GeneratedResidualAffineTargetAttemptOutcome::MatcherRejectedRecenteringBoundary,
                GeneratedResidualAffinePivotTargetOutcome::RejectedRecenteringBoundary(_),
            )
            | (
                GeneratedResidualAffineTargetAttemptOutcome::NoRemainingTargetCase,
                GeneratedResidualAffinePivotTargetOutcome::PendingAffineWhenBad(_),
            ) => {
                if attempt.selected_target_case_ordinal.is_some()
                    || attempt.selected_target_position.is_some()
                    || attempt.outcome.local_compilation().is_some()
                {
                    return Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch);
                }
            }
            (
                GeneratedResidualAffineTargetAttemptOutcome::WhenBadUnsupported(local),
                GeneratedResidualAffinePivotTargetOutcome::PendingAffineWhenBad(pending),
            )
            | (
                GeneratedResidualAffineTargetAttemptOutcome::WhenBadIdenticallyBad(local),
                GeneratedResidualAffinePivotTargetOutcome::PendingAffineWhenBad(pending),
            )
            | (
                GeneratedResidualAffineTargetAttemptOutcome::Accepted(local),
                GeneratedResidualAffinePivotTargetOutcome::PendingAffineWhenBad(pending),
            ) => {
                let (Some(target_case_ordinal), Some(target_position_in_matching_list)) = (
                    attempt.selected_target_case_ordinal,
                    attempt.selected_target_position,
                ) else {
                    return Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch);
                };
                target_position(target_case_ordinal, inventory, group)?;
                if pending
                    .matching_target_case_ordinals()
                    .get(target_position_in_matching_list)
                    != Some(&target_case_ordinal)
                    || local.binding().pivot_ordinal() != attempt_ordinal
                    || local.binding().target_case_ordinal() != target_case_ordinal
                    || local.binding().target_position_in_matching_list()
                        != target_position_in_matching_list
                {
                    return Err(
                        GeneratedResidualAffineGroupEffectiveCoverageError::ArcAuthorityMismatch {
                            attempt_ordinal,
                        },
                    );
                }
                let variant_matches = matches!(
                    (&attempt.outcome, local.as_ref()),
                    (
                        GeneratedResidualAffineTargetAttemptOutcome::WhenBadUnsupported(_),
                        GeneratedResidualAffineWhenBadCompilation::Unsupported(_),
                    ) | (
                        GeneratedResidualAffineTargetAttemptOutcome::WhenBadIdenticallyBad(_),
                        GeneratedResidualAffineWhenBadCompilation::IdenticallyBad(_),
                    ) | (
                        GeneratedResidualAffineTargetAttemptOutcome::Accepted(_),
                        GeneratedResidualAffineWhenBadCompilation::Certified(_),
                    )
                );
                if !variant_matches {
                    return Err(
                        GeneratedResidualAffineGroupEffectiveCoverageError::LocalVariantMismatch {
                            pivot_ordinal: attempt_ordinal,
                        },
                    );
                }
            }
            _ => return Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch),
        }
    }

    for (target_position_in_group, (&target_case_ordinal, disposition)) in group
        .case_ordinals()
        .iter()
        .zip(&certificate.target_dispositions)
        .enumerate()
    {
        let target = inventory
            .cases()
            .get(target_case_ordinal)
            .ok_or(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch)?;
        if target.ordinal_within_group() != target_position_in_group
            || disposition.target_case_ordinal != target_case_ordinal
            || disposition.target_locator != target.locator()
        {
            return Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch);
        }
        match &disposition.disposition {
            GeneratedResidualAffineGroupTargetDisposition::Consumed {
                accepted_attempt_ordinal,
                when_bad,
            } => {
                let attempt_local = accepted_local_for_attempt(
                    certificate,
                    *accepted_attempt_ordinal,
                    target_case_ordinal,
                )?;
                if !Arc::ptr_eq(attempt_local, when_bad) {
                    return Err(
                        GeneratedResidualAffineGroupEffectiveCoverageError::ArcAuthorityMismatch {
                            attempt_ordinal: *accepted_attempt_ordinal,
                        },
                    );
                }
            }
            GeneratedResidualAffineGroupTargetDisposition::Unconsumed {
                rejected_attempt_ordinals,
            } => {
                let mut previous = None;
                for &attempt_ordinal in rejected_attempt_ordinals {
                    if previous.is_some_and(|prior| prior >= attempt_ordinal) {
                        return Err(
                            GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch,
                        );
                    }
                    let attempt = certificate.attempts.get(attempt_ordinal).ok_or(
                        GeneratedResidualAffineGroupEffectiveCoverageError::ArcAuthorityMismatch {
                            attempt_ordinal,
                        },
                    )?;
                    if attempt.selected_target_case_ordinal != Some(target_case_ordinal)
                        || !matches!(
                            attempt.outcome,
                            GeneratedResidualAffineTargetAttemptOutcome::WhenBadUnsupported(_)
                                | GeneratedResidualAffineTargetAttemptOutcome::WhenBadIdenticallyBad(_)
                        )
                    {
                        return Err(
                            GeneratedResidualAffineGroupEffectiveCoverageError::ArcAuthorityMismatch {
                                attempt_ordinal,
                            },
                        );
                    }
                    previous = Some(attempt_ordinal);
                }
            }
        }
    }

    for attempt in &certificate.attempts {
        let Some(target_case_ordinal) = attempt.selected_target_case_ordinal else {
            continue;
        };
        let target_position_in_group = target_position(target_case_ordinal, inventory, group)?;
        let disposition = &certificate.target_dispositions[target_position_in_group].disposition;
        match &attempt.outcome {
            GeneratedResidualAffineTargetAttemptOutcome::Accepted(local) => match disposition {
                GeneratedResidualAffineGroupTargetDisposition::Consumed {
                    accepted_attempt_ordinal,
                    when_bad,
                } if *accepted_attempt_ordinal == attempt.attempt_ordinal
                    && Arc::ptr_eq(local, when_bad) => {}
                _ => {
                    return Err(
                        GeneratedResidualAffineGroupEffectiveCoverageError::ArcAuthorityMismatch {
                            attempt_ordinal: attempt.attempt_ordinal,
                        },
                    );
                }
            },
            GeneratedResidualAffineTargetAttemptOutcome::WhenBadUnsupported(_)
            | GeneratedResidualAffineTargetAttemptOutcome::WhenBadIdenticallyBad(_) => {
                match disposition {
                    GeneratedResidualAffineGroupTargetDisposition::Consumed {
                        accepted_attempt_ordinal,
                        ..
                    } if attempt.attempt_ordinal < *accepted_attempt_ordinal => {}
                    GeneratedResidualAffineGroupTargetDisposition::Unconsumed {
                        rejected_attempt_ordinals,
                    } if rejected_attempt_ordinals
                        .binary_search(&attempt.attempt_ordinal)
                        .is_ok() => {}
                    _ => {
                        return Err(
                            GeneratedResidualAffineGroupEffectiveCoverageError::ArcAuthorityMismatch {
                                attempt_ordinal: attempt.attempt_ordinal,
                            },
                        );
                    }
                }
            }
            GeneratedResidualAffineTargetAttemptOutcome::MatcherRejectedNoTarget
            | GeneratedResidualAffineTargetAttemptOutcome::MatcherRejectedRecenteringBoundary
            | GeneratedResidualAffineTargetAttemptOutcome::NoRemainingTargetCase => {
                return Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch);
            }
        }
    }

    for rule in &certificate.sealed_rules {
        let local = accepted_local_for_attempt(
            certificate,
            rule.accepted_attempt_ordinal,
            rule.target_case_ordinal,
        )?;
        let GeneratedResidualAffineWhenBadCompilation::Certified(accepted) = local.as_ref() else {
            return Err(
                GeneratedResidualAffineGroupEffectiveCoverageError::ArcAuthorityMismatch {
                    attempt_ordinal: rule.accepted_attempt_ordinal,
                },
            );
        };
        let target = inventory
            .cases()
            .get(rule.target_case_ordinal)
            .ok_or(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch)?;
        let leaf = accepted
            .leaf_classifications()
            .get(rule.leaf_ordinal)
            .ok_or(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch)?;
        if !Arc::ptr_eq(local, &rule.when_bad)
            || rule.pivot_ordinal != rule.accepted_attempt_ordinal
            || rule.target_locator != target.locator()
            || leaf.case() != rule.relative_case
            || leaf.disposition() != AffineWhenBadRelativeLeafDisposition::Applicable
        {
            return Err(
                GeneratedResidualAffineGroupEffectiveCoverageError::ArcAuthorityMismatch {
                    attempt_ordinal: rule.accepted_attempt_ordinal,
                },
            );
        }
    }
    for leaf in &certificate.residual_work {
        let target_position_in_group = target_position(leaf.target_case_ordinal, inventory, group)?;
        let target = &inventory.cases()[leaf.target_case_ordinal];
        if leaf.target_locator != target.locator() {
            return Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch);
        }
        let disposition = &certificate.target_dispositions[target_position_in_group].disposition;
        match leaf.kind {
            GeneratedResidualAffineResidualWorkKind::CompleteTargetRoot => {
                if leaf.accepted_attempt_ordinal.is_some()
                    || leaf.leaf_ordinal.is_some()
                    || leaf.relative_case.is_some()
                    || leaf.when_bad.is_some()
                    || !matches!(
                        disposition,
                        GeneratedResidualAffineGroupTargetDisposition::Unconsumed { .. }
                    )
                {
                    return Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch);
                }
            }
            GeneratedResidualAffineResidualWorkKind::ExceptionalDomain { condition_ordinal } => {
                let (Some(attempt_ordinal), Some(leaf_ordinal), Some(relative_case), Some(local)) = (
                    leaf.accepted_attempt_ordinal,
                    leaf.leaf_ordinal,
                    leaf.relative_case,
                    leaf.when_bad.as_ref(),
                ) else {
                    return Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch);
                };
                validate_exceptional_leaf_authority(
                    certificate,
                    disposition,
                    leaf.target_case_ordinal,
                    attempt_ordinal,
                    leaf_ordinal,
                    relative_case,
                    AffineWhenBadRelativeLeafDisposition::ExceptionalDomain { condition_ordinal },
                    local,
                )?;
            }
            GeneratedResidualAffineResidualWorkKind::ExceptionalLeak { pullback_ordinal } => {
                let (Some(attempt_ordinal), Some(leaf_ordinal), Some(relative_case), Some(local)) = (
                    leaf.accepted_attempt_ordinal,
                    leaf.leaf_ordinal,
                    leaf.relative_case,
                    leaf.when_bad.as_ref(),
                ) else {
                    return Err(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch);
                };
                validate_exceptional_leaf_authority(
                    certificate,
                    disposition,
                    leaf.target_case_ordinal,
                    attempt_ordinal,
                    leaf_ordinal,
                    relative_case,
                    AffineWhenBadRelativeLeafDisposition::ExceptionalLeak { pullback_ordinal },
                    local,
                )?;
            }
        }
    }
    Ok(())
}

fn accepted_local_for_attempt<'a>(
    certificate: &'a GeneratedResidualAffineGroupEffectiveCoverageCertificate,
    attempt_ordinal: usize,
    target_case_ordinal: usize,
) -> Result<
    &'a Arc<GeneratedResidualAffineWhenBadCompilation>,
    GeneratedResidualAffineGroupEffectiveCoverageError,
> {
    let attempt = certificate.attempts.get(attempt_ordinal).ok_or(
        GeneratedResidualAffineGroupEffectiveCoverageError::ArcAuthorityMismatch {
            attempt_ordinal,
        },
    )?;
    let GeneratedResidualAffineTargetAttemptOutcome::Accepted(local) = &attempt.outcome else {
        return Err(
            GeneratedResidualAffineGroupEffectiveCoverageError::ArcAuthorityMismatch {
                attempt_ordinal,
            },
        );
    };
    if attempt.attempt_ordinal != attempt_ordinal
        || attempt.pivot_ordinal != attempt_ordinal
        || attempt.selected_target_case_ordinal != Some(target_case_ordinal)
        || !matches!(
            local.as_ref(),
            GeneratedResidualAffineWhenBadCompilation::Certified(_)
        )
    {
        return Err(
            GeneratedResidualAffineGroupEffectiveCoverageError::ArcAuthorityMismatch {
                attempt_ordinal,
            },
        );
    }
    Ok(local)
}

#[allow(clippy::too_many_arguments)]
fn validate_exceptional_leaf_authority(
    certificate: &GeneratedResidualAffineGroupEffectiveCoverageCertificate,
    disposition: &GeneratedResidualAffineGroupTargetDisposition,
    target_case_ordinal: usize,
    attempt_ordinal: usize,
    leaf_ordinal: usize,
    relative_case: AffineWhenBadRelativeCaseId,
    expected_disposition: AffineWhenBadRelativeLeafDisposition,
    local: &Arc<GeneratedResidualAffineWhenBadCompilation>,
) -> Result<(), GeneratedResidualAffineGroupEffectiveCoverageError> {
    let attempt_local =
        accepted_local_for_attempt(certificate, attempt_ordinal, target_case_ordinal)?;
    let GeneratedResidualAffineWhenBadCompilation::Certified(accepted) = attempt_local.as_ref()
    else {
        return Err(
            GeneratedResidualAffineGroupEffectiveCoverageError::ArcAuthorityMismatch {
                attempt_ordinal,
            },
        );
    };
    let classification = accepted
        .leaf_classifications()
        .get(leaf_ordinal)
        .ok_or(GeneratedResidualAffineGroupEffectiveCoverageError::ReplayMismatch)?;
    let disposition_authority_matches = matches!(
        disposition,
        GeneratedResidualAffineGroupTargetDisposition::Consumed {
            accepted_attempt_ordinal,
            when_bad,
        } if *accepted_attempt_ordinal == attempt_ordinal && Arc::ptr_eq(when_bad, attempt_local)
    );
    if !Arc::ptr_eq(attempt_local, local)
        || !disposition_authority_matches
        || classification.case() != relative_case
        || classification.disposition() != expected_disposition
    {
        return Err(
            GeneratedResidualAffineGroupEffectiveCoverageError::ArcAuthorityMismatch {
                attempt_ordinal,
            },
        );
    }
    Ok(())
}

fn payload_eq_checked(
    left: &GeneratedResidualAffineGroupEffectiveCoverageCertificate,
    right: &GeneratedResidualAffineGroupEffectiveCoverageCertificate,
) -> Result<bool, GeneratedResidualAffineGroupEffectiveCoverageError> {
    let left_census = outer_payload_census(
        &left.attempts,
        &left.target_dispositions,
        &left.sealed_rules,
        &left.residual_work,
        left.stats.outer_retained_bytes,
    )?;
    let right_census = outer_payload_census(
        &right.attempts,
        &right.target_dispositions,
        &right.sealed_rules,
        &right.residual_work,
        right.stats.outer_retained_bytes,
    )?;
    check_outer_payload_limits(
        OuterPayloadCensus {
            units: left_census.units.max(right_census.units),
            bytes: left_census.bytes.max(right_census.bytes),
            integer_bits: left_census.integer_bits.max(right_census.integer_bits),
        },
        intersect_outer_limits(left.limits, right.limits),
    )?;
    if left_census.units != left.stats.outer_payload_comparison_units
        || left_census.bytes != left.stats.outer_payload_comparison_bytes
        || left_census.integer_bits != left.stats.outer_payload_comparison_integer_bits
        || right_census.units != right.stats.outer_payload_comparison_units
        || right_census.bytes != right.stats.outer_payload_comparison_bytes
        || right_census.integer_bits != right.stats.outer_payload_comparison_integer_bits
        || left.schema != right.schema
        || !Arc::ptr_eq(&left.matcher, &right.matcher)
        || left.limits != right.limits
        || left.stats != right.stats
        || left.attempts.len() != right.attempts.len()
        || left.target_dispositions.len() != right.target_dispositions.len()
        || left.sealed_rules.len() != right.sealed_rules.len()
        || left.residual_work.len() != right.residual_work.len()
    {
        return Ok(false);
    }
    for (left, right) in left.attempts.iter().zip(&right.attempts) {
        if left.attempt_ordinal != right.attempt_ordinal
            || left.pivot_ordinal != right.pivot_ordinal
            || left.selected_target_case_ordinal != right.selected_target_case_ordinal
            || left.selected_target_position != right.selected_target_position
        {
            return Ok(false);
        }
        let equal = match (&left.outcome, &right.outcome) {
            (
                GeneratedResidualAffineTargetAttemptOutcome::MatcherRejectedNoTarget,
                GeneratedResidualAffineTargetAttemptOutcome::MatcherRejectedNoTarget,
            )
            | (
                GeneratedResidualAffineTargetAttemptOutcome::MatcherRejectedRecenteringBoundary,
                GeneratedResidualAffineTargetAttemptOutcome::MatcherRejectedRecenteringBoundary,
            )
            | (
                GeneratedResidualAffineTargetAttemptOutcome::NoRemainingTargetCase,
                GeneratedResidualAffineTargetAttemptOutcome::NoRemainingTargetCase,
            ) => true,
            (
                GeneratedResidualAffineTargetAttemptOutcome::WhenBadUnsupported(left),
                GeneratedResidualAffineTargetAttemptOutcome::WhenBadUnsupported(right),
            )
            | (
                GeneratedResidualAffineTargetAttemptOutcome::WhenBadIdenticallyBad(left),
                GeneratedResidualAffineTargetAttemptOutcome::WhenBadIdenticallyBad(right),
            )
            | (
                GeneratedResidualAffineTargetAttemptOutcome::Accepted(left),
                GeneratedResidualAffineTargetAttemptOutcome::Accepted(right),
            ) => left.payload_eq_checked(right)?,
            _ => false,
        };
        if !equal {
            return Ok(false);
        }
    }
    for (left, right) in left
        .target_dispositions
        .iter()
        .zip(&right.target_dispositions)
    {
        if left.target_case_ordinal != right.target_case_ordinal
            || left.target_locator != right.target_locator
        {
            return Ok(false);
        }
        match (&left.disposition, &right.disposition) {
            (
                GeneratedResidualAffineGroupTargetDisposition::Consumed {
                    accepted_attempt_ordinal: left_attempt,
                    ..
                },
                GeneratedResidualAffineGroupTargetDisposition::Consumed {
                    accepted_attempt_ordinal: right_attempt,
                    ..
                },
            ) if left_attempt == right_attempt => {}
            (
                GeneratedResidualAffineGroupTargetDisposition::Unconsumed {
                    rejected_attempt_ordinals: left_rejected,
                },
                GeneratedResidualAffineGroupTargetDisposition::Unconsumed {
                    rejected_attempt_ordinals: right_rejected,
                },
            ) if left_rejected == right_rejected => {}
            _ => return Ok(false),
        }
    }
    for (left, right) in left.sealed_rules.iter().zip(&right.sealed_rules) {
        if left.accepted_attempt_ordinal != right.accepted_attempt_ordinal
            || left.pivot_ordinal != right.pivot_ordinal
            || left.target_case_ordinal != right.target_case_ordinal
            || left.target_locator != right.target_locator
            || left.leaf_ordinal != right.leaf_ordinal
            || left.relative_case != right.relative_case
        {
            return Ok(false);
        }
    }
    for (left, right) in left.residual_work.iter().zip(&right.residual_work) {
        if left.target_case_ordinal != right.target_case_ordinal
            || left.target_locator != right.target_locator
            || left.accepted_attempt_ordinal != right.accepted_attempt_ordinal
            || left.leaf_ordinal != right.leaf_ordinal
            || left.relative_case != right.relative_case
            || left.kind != right.kind
            || left.when_bad.is_some() != right.when_bad.is_some()
        {
            return Ok(false);
        }
    }
    validate_arc_authorities(left)?;
    validate_arc_authorities(right)?;
    Ok(true)
}

fn intersect_outer_limits(
    mut left: GeneratedResidualAffineGroupEffectiveCoverageLimits,
    right: GeneratedResidualAffineGroupEffectiveCoverageLimits,
) -> GeneratedResidualAffineGroupEffectiveCoverageLimits {
    left.max_outer_payload_comparison_units = left
        .max_outer_payload_comparison_units
        .min(right.max_outer_payload_comparison_units);
    left.max_outer_payload_comparison_bytes = left
        .max_outer_payload_comparison_bytes
        .min(right.max_outer_payload_comparison_bytes);
    left.max_outer_payload_comparison_integer_bits = left
        .max_outer_payload_comparison_integer_bits
        .min(right.max_outer_payload_comparison_integer_bits);
    left
}

fn arc_control_and_padding_bytes<T>()
-> Result<usize, GeneratedResidualAffineGroupEffectiveCoverageError> {
    checked_add(
        "group effective outer retained bytes",
        checked_mul(
            "group effective outer retained bytes",
            2,
            size_of::<usize>(),
        )?,
        align_of::<T>().saturating_sub(1),
    )
}

fn charge_capacity_bytes<T>(
    retained: usize,
    capacity: usize,
    limit: usize,
) -> Result<usize, GeneratedResidualAffineGroupEffectiveCoverageError> {
    bounded_add(
        "group effective outer retained bytes",
        retained,
        checked_mul(
            "group effective outer retained bytes",
            capacity,
            size_of::<T>(),
        )?,
        limit,
    )
}

fn preflight_capacity_bytes<T>(
    retained: usize,
    requested_capacity: usize,
    limit: usize,
) -> Result<(), GeneratedResidualAffineGroupEffectiveCoverageError> {
    let lower_bound = checked_add(
        "group effective outer retained bytes",
        retained,
        checked_mul(
            "group effective outer retained bytes",
            requested_capacity,
            size_of::<T>(),
        )?,
    )?;
    check_limit("group effective outer retained bytes", lower_bound, limit)
}

fn remaining(
    resource: &'static str,
    limit: usize,
    used: usize,
) -> Result<usize, GeneratedResidualAffineGroupEffectiveCoverageError> {
    limit.checked_sub(used).ok_or(
        GeneratedResidualAffineGroupEffectiveCoverageError::ResourceLimit {
            resource,
            requested: used,
            limit,
        },
    )
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffineGroupEffectiveCoverageError> {
    left.checked_add(right).ok_or(
        GeneratedResidualAffineGroupEffectiveCoverageError::ResourceCountOverflow { resource },
    )
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffineGroupEffectiveCoverageError> {
    left.checked_mul(right).ok_or(
        GeneratedResidualAffineGroupEffectiveCoverageError::ResourceCountOverflow { resource },
    )
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedResidualAffineGroupEffectiveCoverageError> {
    if requested > limit {
        Err(
            GeneratedResidualAffineGroupEffectiveCoverageError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

fn bounded_add(
    resource: &'static str,
    current: usize,
    increment: usize,
    limit: usize,
) -> Result<usize, GeneratedResidualAffineGroupEffectiveCoverageError> {
    let requested = checked_add(resource, current, increment)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn try_reserve_exact<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), GeneratedResidualAffineGroupEffectiveCoverageError> {
    values.try_reserve_exact(additional).map_err(|_| {
        GeneratedResidualAffineGroupEffectiveCoverageError::AllocationFailure {
            resource,
            requested: additional,
        }
    })
}

#[cfg(test)]
mod tests;
