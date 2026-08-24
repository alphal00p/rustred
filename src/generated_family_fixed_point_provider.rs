//! Concrete application of a replayed residual fixed-point certificate.
//!
//! This module installs the exact latest generated material selected by
//! [`crate::GeneratedFamilyFixedPointCertificate`].  It does not infer a
//! master from an uncovered residual cell and it does not retain an older
//! discovery merely because that discovery happened to cover a concrete
//! search anchor.  Caller-declared terminals are the only master policy.
//!
//! The runtime stack is shared with the family provider and has the stable
//! order `zero(symmetry(master(conditional(global))))`.

use std::fmt;
use std::sync::Arc;

use crate::generated_provider_stack::{
    GeneratedProviderStack, GeneratedProviderStackBuildError, GeneratedStackConditionalProvider,
    GeneratedStackMasterProvider, GeneratedStackSectorProvider, GeneratedStackSymmetryProvider,
    build_generated_provider_stack, replay_generated_provider_symmetry,
};
use crate::reduction_engine::{ConcreteRuleDecision, ConcreteRuleProvider};
use crate::{
    CertifiedSymmetryCanonicalizingRuleProviderError, CertifiedZeroSectorRuleProviderError,
    ConcreteIntegralKey, GENERATED_FAMILY_FIXED_POINT_PROVIDER_V1_SCHEMA,
    GeneratedFamilyFixedPointAttemptOutcome, GeneratedFamilyFixedPointBasePreparationOutcome,
    GeneratedFamilyFixedPointCertificate, GeneratedFamilyFixedPointError,
    GeneratedFamilyFixedPointFinalStatus, GeneratedFamilyFixedPointInterruption,
    GeneratedFamilyFixedPointStage, GeneratedFamilyPipelineStage,
    GeneratedFamilyRuleSystemProviderLimits, GeneratedFamilySectorFailure,
    GeneratedFamilySectorResource, GeneratedFamilySectorStatus, GeneratedFixedPointMaterialRef,
    GeneratedSectorConditionalRuleProvider, GeneratedSectorConditionalRuleProviderBuildStats,
    GeneratedSectorConditionalRuleProviderError, IntegralFamily, MasterPolicyError,
    MasterPolicyTerminal, ParametricCoefficientContext, ParametricSectorRuleProvider,
    ParametricSectorRuleProviderError, SectorMask,
};

pub type GeneratedFamilyFixedPointProviderLimits = GeneratedFamilyRuleSystemProviderLimits;
pub type GeneratedFamilyFixedPointConditionalProviderError =
    GeneratedSectorConditionalRuleProviderError<ParametricSectorRuleProviderError>;
pub type GeneratedFamilyFixedPointMasterProviderError =
    MasterPolicyError<GeneratedFamilyFixedPointConditionalProviderError>;
pub type GeneratedFamilyFixedPointSymmetryProviderError =
    CertifiedSymmetryCanonicalizingRuleProviderError<GeneratedFamilyFixedPointMasterProviderError>;
pub type GeneratedFamilyFixedPointProviderStackError =
    CertifiedZeroSectorRuleProviderError<GeneratedFamilyFixedPointSymmetryProviderError>;

type SectorProvider<'family> = GeneratedStackSectorProvider<'family>;
type ConditionalProvider<'family> = GeneratedStackConditionalProvider<'family>;
type MasterProvider<'family> = GeneratedStackMasterProvider<'family>;
type SymmetryProvider<'family> = GeneratedStackSymmetryProvider<'family>;
type ProviderStack<'family> = GeneratedProviderStack<'family>;

/// Immutable census of the latest fixed-point material.  The two explicit
/// master counts are refreshed after checked terminal insertion/removal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedFamilyFixedPointProviderBuildStats {
    final_sector_statuses: usize,
    covered_generated_sectors: usize,
    residual_generated_sectors: usize,
    retained_generated_sectors: usize,
    candidate_attempts: usize,
    global_leaves: usize,
    live_leaf_work_items: usize,
    master_terminals: usize,
    master_certificate_fingerprint_bytes: usize,
    conditional: GeneratedSectorConditionalRuleProviderBuildStats,
}

impl GeneratedFamilyFixedPointProviderBuildStats {
    pub const fn final_sector_statuses(self) -> usize {
        self.final_sector_statuses
    }
    pub const fn covered_generated_sectors(self) -> usize {
        self.covered_generated_sectors
    }
    pub const fn residual_generated_sectors(self) -> usize {
        self.residual_generated_sectors
    }
    pub const fn retained_generated_sectors(self) -> usize {
        self.retained_generated_sectors
    }
    pub const fn candidate_attempts(self) -> usize {
        self.candidate_attempts
    }
    pub const fn global_leaves(self) -> usize {
        self.global_leaves
    }
    pub const fn live_leaf_work_items(self) -> usize {
        self.live_leaf_work_items
    }
    pub const fn master_terminals(self) -> usize {
        self.master_terminals
    }
    pub const fn master_certificate_fingerprint_bytes(self) -> usize {
        self.master_certificate_fingerprint_bytes
    }
    pub const fn conditional(self) -> GeneratedSectorConditionalRuleProviderBuildStats {
        self.conditional
    }
}

/// Transcript location reported when provider construction rejects an
/// interrupted fixed-point certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedFamilyFixedPointProviderInterruptionLocation {
    BasePreparation {
        preparation_ordinal: usize,
    },
    ResidualRound {
        round_ordinal: usize,
        sector_attempt_ordinal: usize,
    },
    FinalStatus,
}

/// A topology-independent concrete provider over the exact latest material
/// of one replayable residual fixed point.
pub struct GeneratedFamilyFixedPointProvider<'family> {
    family: &'family IntegralFamily,
    context: &'family ParametricCoefficientContext,
    certificate: GeneratedFamilyFixedPointCertificate,
    stack: ProviderStack<'family>,
    limits: GeneratedFamilyFixedPointProviderLimits,
    build_stats: GeneratedFamilyFixedPointProviderBuildStats,
}

impl<'family> GeneratedFamilyFixedPointProvider<'family> {
    pub const SCHEMA: &'static str = GENERATED_FAMILY_FIXED_POINT_PROVIDER_V1_SCHEMA;

    /// Construct without master declarations.  Residual integrals remain
    /// uncovered after generated global and conditional rules are exhausted.
    pub fn try_new(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        certificate: GeneratedFamilyFixedPointCertificate,
        limits: GeneratedFamilyFixedPointProviderLimits,
    ) -> Result<Self, GeneratedFamilyFixedPointProviderError> {
        Self::try_with_terminals(family, context, certificate, [], limits)
    }

    /// Construct with caller-owned, explicit terminal declarations.
    pub fn try_with_terminals(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        certificate: GeneratedFamilyFixedPointCertificate,
        terminals: impl IntoIterator<Item = (ConcreteIntegralKey, MasterPolicyTerminal)>,
        limits: GeneratedFamilyFixedPointProviderLimits,
    ) -> Result<Self, GeneratedFamilyFixedPointProviderError> {
        Self::try_with_terminals_impl(family, context, certificate, terminals, limits)
    }

    fn try_with_terminals_impl(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        certificate: GeneratedFamilyFixedPointCertificate,
        terminals: impl IntoIterator<Item = (ConcreteIntegralKey, MasterPolicyTerminal)>,
        limits: GeneratedFamilyFixedPointProviderLimits,
    ) -> Result<Self, GeneratedFamilyFixedPointProviderError> {
        let terminals = terminals.into_iter();
        let has_interruption = reject_interrupted_certificate(&certificate).is_err();

        // Reject obviously oversized valid material before replay performs a
        // potentially expensive fixed-point rebuild.  Interrupted transcripts
        // retain interruption precedence and are replayed before rejection.
        let preflight_stats = if has_interruption {
            None
        } else {
            let materials = resolve_latest_materials(&certificate, limits)?;
            let stats = preflight_materials(&certificate, &materials, limits)?;
            ParametricSectorRuleProvider::preflight_certificates(
                family,
                context,
                materials
                    .iter()
                    .map(GeneratedFixedPointMaterialRef::discovery)
                    .map(|discovery| discovery.coverage()),
                limits.sector_rules,
            )
            .map_err(wrap_sector_error)?;
            GeneratedSectorConditionalRuleProvider::<SectorProvider<'family>>::preflight_queues(
                family,
                context,
                materials
                    .iter()
                    .map(GeneratedFixedPointMaterialRef::live_leaf_queue),
                limits.conditional_rules,
            )
            .map_err(wrap_conditional_error)?;
            Some(stats)
        };

        certificate.replay(family, context)?;
        reject_interrupted_certificate(&certificate)?;

        let terminal_lower_bound = terminals.size_hint().0;
        if terminal_lower_bound > limits.max_input_terminals {
            return Err(GeneratedFamilyFixedPointProviderError::ResourceLimit {
                resource: "fixed-point provider input terminal declarations",
                requested: terminal_lower_bound,
                limit: limits.max_input_terminals,
            });
        }

        let zero_limits = certificate.base().limits().inventory.zero_sectors;
        if limits.certified_rewrite.zero_sector != zero_limits {
            return Err(
                GeneratedFamilyFixedPointProviderError::ZeroAnalysisLimitsMismatch {
                    certificate: zero_limits,
                    provider: limits.certified_rewrite.zero_sector,
                },
            );
        }

        let materials = resolve_latest_materials(&certificate, limits)?;
        let mut build_stats = match preflight_stats {
            Some(stats) => stats,
            None => preflight_materials(&certificate, &materials, limits)?,
        };
        validate_material_row_span(&certificate, &materials)?;

        let coverages = materials
            .iter()
            .map(|material| material.discovery().coverage().clone())
            .collect::<Vec<_>>();
        let queues = materials
            .iter()
            .map(|material| material.live_leaf_queue().clone())
            .collect::<Vec<_>>();
        let shared_row_span = certificate.base().row_span_arc().cloned();
        let built = build_generated_provider_stack(
            family,
            context,
            certificate.base().inventory_restrictions().clone(),
            certificate.base().inventory_power_shift_policy(),
            certificate.base().ordering(),
            shared_row_span,
            coverages,
            queues,
            terminals,
            limits,
        )
        .map_err(map_stack_build_error)?;
        build_stats.conditional = built.conditional_stats;
        build_stats.master_terminals = built.master_terminals;
        build_stats.master_certificate_fingerprint_bytes =
            built.master_certificate_fingerprint_bytes;

        let provider = Self {
            family,
            context,
            certificate,
            stack: built.stack,
            limits,
            build_stats,
        };
        provider.validate_binding()?;
        Ok(provider)
    }

    pub fn try_with_selected(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        certificate: GeneratedFamilyFixedPointCertificate,
        selected: impl IntoIterator<Item = ConcreteIntegralKey>,
        limits: GeneratedFamilyFixedPointProviderLimits,
    ) -> Result<Self, GeneratedFamilyFixedPointProviderError> {
        Self::try_with_terminals(
            family,
            context,
            certificate,
            selected
                .into_iter()
                .map(|key| (key, MasterPolicyTerminal::Selected)),
            limits,
        )
    }

    pub const fn family(&self) -> &IntegralFamily {
        self.family
    }
    pub const fn context(&self) -> &ParametricCoefficientContext {
        self.context
    }
    pub const fn certificate(&self) -> &GeneratedFamilyFixedPointCertificate {
        &self.certificate
    }
    pub const fn limits(&self) -> GeneratedFamilyFixedPointProviderLimits {
        self.limits
    }
    pub const fn build_stats(&self) -> GeneratedFamilyFixedPointProviderBuildStats {
        self.build_stats
    }
    pub const fn inventory_restrictions(&self) -> &crate::SectorRestrictions {
        self.certificate.base().inventory_restrictions()
    }
    pub const fn inventory_power_shift_policy(&self) -> crate::PowerShiftPolicy {
        self.certificate.base().inventory_power_shift_policy()
    }
    pub const fn ordering(&self) -> crate::IntegralOrderingPolicy {
        self.certificate.base().ordering()
    }
    pub fn terminals(
        &self,
    ) -> &std::collections::BTreeMap<ConcreteIntegralKey, MasterPolicyTerminal> {
        self.master_provider().terminals()
    }

    pub fn insert_terminal(
        &mut self,
        integral: ConcreteIntegralKey,
        terminal: MasterPolicyTerminal,
    ) -> Result<(), GeneratedFamilyFixedPointProviderError> {
        let canonical = self
            .symmetry_provider()
            .canonical_key(&integral)
            .map_err(wrap_symmetry_error)?;
        self.stack
            .inner_mut()
            .inner_mut()
            .insert_terminal(canonical, terminal)
            .map_err(wrap_master_error)?;
        self.refresh_master_stats();
        Ok(())
    }

    pub fn insert_selected_master(
        &mut self,
        integral: ConcreteIntegralKey,
    ) -> Result<(), GeneratedFamilyFixedPointProviderError> {
        self.insert_terminal(integral, MasterPolicyTerminal::Selected)
    }

    pub fn insert_certified_master(
        &mut self,
        integral: ConcreteIntegralKey,
        certificate_fingerprint: impl Into<Arc<str>>,
    ) -> Result<(), GeneratedFamilyFixedPointProviderError> {
        self.insert_terminal(
            integral,
            MasterPolicyTerminal::Certified {
                certificate_fingerprint: certificate_fingerprint.into(),
            },
        )
    }

    pub fn remove_master(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<bool, GeneratedFamilyFixedPointProviderError> {
        let canonical = self
            .symmetry_provider()
            .canonical_key(integral)
            .map_err(wrap_symmetry_error)?;
        let removed = self
            .stack
            .inner_mut()
            .inner_mut()
            .remove_terminal(&canonical)
            .map_err(wrap_master_error)?;
        self.refresh_master_stats();
        Ok(removed)
    }

    /// Replay the full fixed-point schedule and every installed generated,
    /// conditional, and symmetry proof.  Runtime query counters are excluded.
    pub fn replay(&self) -> Result<(), GeneratedFamilyFixedPointProviderError> {
        self.certificate.replay(self.family, self.context)?;
        reject_interrupted_certificate(&self.certificate)?;
        self.validate_binding()?;
        replay_generated_provider_symmetry(
            self.symmetry_provider(),
            self.certificate.base().row_span_arc(),
        )
        .map_err(wrap_symmetry_error)?;
        self.conditional_provider()
            .replay_with_replayed_queues()
            .map_err(wrap_conditional_error)?;
        let expected = current_build_stats(
            &self.certificate,
            self.conditional_provider().build_stats(),
            self.master_provider().terminals().len(),
            self.master_provider().total_certificate_fingerprint_bytes(),
            self.limits,
        )?;
        if expected != self.build_stats {
            return Err(GeneratedFamilyFixedPointProviderError::ReplayMismatch {
                detail: "provider retained-input census differs",
            });
        }
        Ok(())
    }

    pub const fn zero_provider(&self) -> &ProviderStack<'family> {
        &self.stack
    }
    pub const fn symmetry_provider(&self) -> &SymmetryProvider<'family> {
        self.stack.inner()
    }
    pub const fn master_provider(&self) -> &MasterProvider<'family> {
        self.stack.inner().inner()
    }
    pub const fn conditional_provider(&self) -> &ConditionalProvider<'family> {
        self.stack.inner().inner().inner()
    }
    pub const fn sector_provider(&self) -> &SectorProvider<'family> {
        self.stack.inner().inner().inner().inner()
    }

    fn refresh_master_stats(&mut self) {
        self.build_stats.master_terminals = self.master_provider().terminals().len();
        self.build_stats.master_certificate_fingerprint_bytes =
            self.master_provider().total_certificate_fingerprint_bytes();
    }

    fn validate_binding(&self) -> Result<(), GeneratedFamilyFixedPointProviderError> {
        if self.family.fingerprint() != self.certificate.family_fingerprint() {
            return Err(GeneratedFamilyFixedPointProviderError::ReplayMismatch {
                detail: "provider family differs from the fixed-point certificate",
            });
        }
        if self.context.fingerprint() != self.certificate.context_fingerprint() {
            return Err(GeneratedFamilyFixedPointProviderError::ReplayMismatch {
                detail: "provider context differs from the fixed-point certificate",
            });
        }
        let base = self.certificate.base();
        if self.stack.restrictions() != base.inventory_restrictions()
            || self.stack.analyzer().policy() != base.inventory_power_shift_policy()
            || self.stack.analyzer().limits() != base.limits().inventory.zero_sectors
            || self.stack.rewrite_limits() != self.limits.certified_rewrite
            || self.symmetry_provider().family().fingerprint() != self.family.fingerprint()
            || self.symmetry_provider().context().fingerprint() != self.context.fingerprint()
            || self.symmetry_provider().restrictions() != base.inventory_restrictions()
            || self.symmetry_provider().ordering() != base.ordering()
            || self.symmetry_provider().limits() != self.limits.symmetry
            || self.master_provider().limits() != self.limits.master_policy
            || self.conditional_provider().limits() != self.limits.conditional_rules
            || self.sector_provider().limits() != self.limits.sector_rules
        {
            return Err(GeneratedFamilyFixedPointProviderError::ReplayMismatch {
                detail: "provider policy or nested limits differ from retained build inputs",
            });
        }

        let materials = resolve_latest_materials(&self.certificate, self.limits)?;
        validate_material_row_span(&self.certificate, &materials)?;
        if self.sector_provider().certificates().len() != materials.len()
            || self.conditional_provider().queues().len() != materials.len()
        {
            return Err(GeneratedFamilyFixedPointProviderError::ReplayMismatch {
                detail: "installed sector set differs from exact latest fixed-point material",
            });
        }

        match (
            self.symmetry_provider().row_span_arc(),
            self.certificate.base().row_span_arc(),
        ) {
            (Some(installed), Some(expected)) if Arc::ptr_eq(installed, expected) => {}
            (None, None) if materials.is_empty() => {}
            _ => {
                return Err(GeneratedFamilyFixedPointProviderError::ReplayMismatch {
                    detail: "symmetry provider lost the fixed-point shared row-span allocation",
                });
            }
        }

        for terminal in self.master_provider().terminals().keys() {
            if self
                .symmetry_provider()
                .canonical_key(terminal)
                .map_err(wrap_symmetry_error)?
                != *terminal
            {
                return Err(GeneratedFamilyFixedPointProviderError::ReplayMismatch {
                    detail: "explicit master policy contains a noncanonical symmetry-orbit key",
                });
            }
        }

        let shared_row_span = self.certificate.base().row_span_arc();
        for material in &materials {
            let final_status = self.certificate.final_status(material.sector()).ok_or(
                GeneratedFamilyFixedPointProviderError::ReplayMismatch {
                    detail: "latest material has no fixed-point final status",
                },
            )?;
            if final_status.latest_material() != material.locator() {
                return Err(GeneratedFamilyFixedPointProviderError::ReplayMismatch {
                    detail: "latest material locator differs from its final status",
                });
            }
            let installed = self
                .sector_provider()
                .certificates()
                .get(material.sector())
                .ok_or(GeneratedFamilyFixedPointProviderError::ReplayMismatch {
                    detail: "latest fixed-point coverage was not installed",
                })?;
            if !installed.payload_eq(material.discovery().coverage()) {
                return Err(GeneratedFamilyFixedPointProviderError::ReplayMismatch {
                    detail: "installed coverage differs from latest fixed-point material",
                });
            }
            let shared = shared_row_span
                .ok_or(GeneratedFamilyFixedPointProviderError::MissingSharedRowSpan)?;
            if !Arc::ptr_eq(installed.row_span_arc(), shared) {
                return Err(GeneratedFamilyFixedPointProviderError::ReplayMismatch {
                    detail: "installed coverage lost the fixed-point shared row-span allocation",
                });
            }
            let installed_queue = self
                .conditional_provider()
                .queues()
                .find(|queue| queue.sector() == material.sector())
                .ok_or(GeneratedFamilyFixedPointProviderError::ReplayMismatch {
                    detail: "latest fixed-point queue was not installed",
                })?;
            if !installed_queue.payload_eq(material.live_leaf_queue()) {
                return Err(GeneratedFamilyFixedPointProviderError::ReplayMismatch {
                    detail: "installed queue differs from latest fixed-point material",
                });
            }
            if !Arc::ptr_eq(installed_queue.discovery().row_span_arc(), shared) {
                return Err(GeneratedFamilyFixedPointProviderError::ReplayMismatch {
                    detail: "installed queue lost the fixed-point shared row-span allocation",
                });
            }
        }
        Ok(())
    }
}

impl ConcreteRuleProvider for GeneratedFamilyFixedPointProvider<'_> {
    type Error = GeneratedFamilyFixedPointProviderError;

    fn index_arity(&self) -> usize {
        self.family.denominator_count()
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        self.stack
            .decision_for(integral)
            .map_err(GeneratedFamilyFixedPointProviderError::Provider)
    }
}

fn resolve_latest_materials(
    certificate: &GeneratedFamilyFixedPointCertificate,
    limits: GeneratedFamilyFixedPointProviderLimits,
) -> Result<Vec<GeneratedFixedPointMaterialRef<'_>>, GeneratedFamilyFixedPointProviderError> {
    // `latest_materials` owns its output vector, so enforce the provider's
    // retained-sector budget before that allocation rather than merely
    // rejecting the completed vector in `preflight_materials`.
    if certificate.final_statuses().len() > limits.max_retained_generated_sectors {
        return Err(GeneratedFamilyFixedPointProviderError::ResourceLimit {
            resource: "fixed-point provider generated sectors",
            requested: certificate.final_statuses().len(),
            limit: limits.max_retained_generated_sectors,
        });
    }
    certificate
        .latest_materials()
        .ok_or(GeneratedFamilyFixedPointProviderError::ReplayMismatch {
            detail: "a fixed-point final status has an unresolved material locator",
        })
}

fn validate_material_row_span(
    certificate: &GeneratedFamilyFixedPointCertificate,
    materials: &[GeneratedFixedPointMaterialRef<'_>],
) -> Result<(), GeneratedFamilyFixedPointProviderError> {
    if materials.is_empty() {
        return Ok(());
    }
    let shared = certificate
        .base()
        .row_span_arc()
        .ok_or(GeneratedFamilyFixedPointProviderError::MissingSharedRowSpan)?;
    for material in materials {
        if !Arc::ptr_eq(material.discovery().row_span_arc(), shared)
            || !Arc::ptr_eq(material.discovery().coverage().row_span_arc(), shared)
            || !Arc::ptr_eq(
                material.live_leaf_queue().discovery().row_span_arc(),
                shared,
            )
        {
            return Err(GeneratedFamilyFixedPointProviderError::ReplayMismatch {
                detail: "latest fixed-point material lost the base shared row-span allocation",
            });
        }
    }
    Ok(())
}

fn preflight_materials(
    certificate: &GeneratedFamilyFixedPointCertificate,
    materials: &[GeneratedFixedPointMaterialRef<'_>],
    limits: GeneratedFamilyFixedPointProviderLimits,
) -> Result<GeneratedFamilyFixedPointProviderBuildStats, GeneratedFamilyFixedPointProviderError> {
    let mut stats = GeneratedFamilyFixedPointProviderBuildStats {
        final_sector_statuses: certificate.final_statuses().len(),
        ..GeneratedFamilyFixedPointProviderBuildStats::default()
    };
    for status in certificate.final_statuses() {
        match status.status() {
            GeneratedFamilyFixedPointFinalStatus::CoveredByGeneratedRules => {
                stats.covered_generated_sectors = bounded_add(
                    "fixed-point provider covered generated sectors",
                    stats.covered_generated_sectors,
                    1,
                    limits.max_retained_generated_sectors,
                )?;
            }
            GeneratedFamilyFixedPointFinalStatus::AnchorWitnessSearchExhaustedWithinConfiguredBounds {
                ..
            }
            | GeneratedFamilyFixedPointFinalStatus::ExhaustedAtMaximumRounds { .. }
            | GeneratedFamilyFixedPointFinalStatus::StalledNoStrictResidualImprovement { .. }
            | GeneratedFamilyFixedPointFinalStatus::NotSelectedByPolicyBound { .. } => {
                stats.residual_generated_sectors = bounded_add(
                    "fixed-point provider residual generated sectors",
                    stats.residual_generated_sectors,
                    1,
                    limits.max_retained_generated_sectors,
                )?;
            }
            GeneratedFamilyFixedPointFinalStatus::ResourceLimited { .. }
            | GeneratedFamilyFixedPointFinalStatus::Failed { .. } => {}
        }
    }
    for material in materials {
        stats.retained_generated_sectors = bounded_add(
            "fixed-point provider generated sectors",
            stats.retained_generated_sectors,
            1,
            limits.max_retained_generated_sectors,
        )?;
        stats.candidate_attempts = bounded_add(
            "fixed-point provider candidate attempts",
            stats.candidate_attempts,
            material.discovery().coverage().candidate_attempts().len(),
            limits.max_total_candidate_attempts,
        )?;
        stats.global_leaves = bounded_add(
            "fixed-point provider global leaves",
            stats.global_leaves,
            material.discovery().coverage().classifications().len(),
            limits.max_total_global_leaves,
        )?;
        stats.live_leaf_work_items = bounded_add(
            "fixed-point provider live-leaf work items",
            stats.live_leaf_work_items,
            material.live_leaf_queue().work_items().len(),
            limits.max_total_live_leaf_work_items,
        )?;
    }
    Ok(stats)
}

fn current_build_stats(
    certificate: &GeneratedFamilyFixedPointCertificate,
    conditional: GeneratedSectorConditionalRuleProviderBuildStats,
    master_terminals: usize,
    master_certificate_fingerprint_bytes: usize,
    limits: GeneratedFamilyFixedPointProviderLimits,
) -> Result<GeneratedFamilyFixedPointProviderBuildStats, GeneratedFamilyFixedPointProviderError> {
    let materials = resolve_latest_materials(certificate, limits)?;
    let mut stats = preflight_materials(certificate, &materials, limits)?;
    stats.conditional = conditional;
    stats.master_terminals = master_terminals;
    stats.master_certificate_fingerprint_bytes = master_certificate_fingerprint_bytes;
    Ok(stats)
}

fn reject_interrupted_certificate(
    certificate: &GeneratedFamilyFixedPointCertificate,
) -> Result<(), GeneratedFamilyFixedPointProviderError> {
    for transcript in certificate.base().sectors() {
        match transcript.status() {
            GeneratedFamilySectorStatus::ResourceLimited { resource, .. } => {
                return Err(
                    GeneratedFamilyFixedPointProviderError::BaseInterruptedResource {
                        sector: transcript.sector().clone(),
                        stage: resource.stage(),
                        resource: resource.clone(),
                    },
                );
            }
            GeneratedFamilySectorStatus::Failed { failure, .. } => {
                return Err(
                    GeneratedFamilyFixedPointProviderError::BaseInterruptedFailure {
                        sector: transcript.sector().clone(),
                        stage: failure.stage(),
                        failure: failure.clone(),
                    },
                );
            }
            _ => {}
        }
    }
    for preparation in certificate.base_preparations() {
        match preparation.outcome() {
            GeneratedFamilyFixedPointBasePreparationOutcome::ResourceLimited {
                interruption,
                ..
            } => {
                return Err(
                    GeneratedFamilyFixedPointProviderError::FixedPointInterruptedResource {
                        sector: preparation.sector().clone(),
                        location:
                            GeneratedFamilyFixedPointProviderInterruptionLocation::BasePreparation {
                                preparation_ordinal: preparation.ordinal(),
                            },
                        stage: interruption.stage(),
                        interruption: interruption.clone(),
                    },
                );
            }
            GeneratedFamilyFixedPointBasePreparationOutcome::Failed { interruption, .. } => {
                return Err(
                    GeneratedFamilyFixedPointProviderError::FixedPointInterruptedFailure {
                        sector: preparation.sector().clone(),
                        location:
                            GeneratedFamilyFixedPointProviderInterruptionLocation::BasePreparation {
                                preparation_ordinal: preparation.ordinal(),
                            },
                        stage: interruption.stage(),
                        interruption: interruption.clone(),
                    },
                );
            }
            GeneratedFamilyFixedPointBasePreparationOutcome::Prepared { .. } => {}
        }
    }
    for round in certificate.rounds() {
        for attempt in round.attempts() {
            let (resource_limited, interruption) = match attempt.outcome() {
                GeneratedFamilyFixedPointAttemptOutcome::ResourceLimited {
                    interruption, ..
                } => (true, Some(interruption)),
                GeneratedFamilyFixedPointAttemptOutcome::Failed { interruption, .. } => {
                    (false, Some(interruption))
                }
                _ => (false, None),
            };
            let Some(interruption) = interruption else {
                continue;
            };
            let location = GeneratedFamilyFixedPointProviderInterruptionLocation::ResidualRound {
                round_ordinal: round.ordinal(),
                sector_attempt_ordinal: attempt.ordinal(),
            };
            if resource_limited {
                return Err(
                    GeneratedFamilyFixedPointProviderError::FixedPointInterruptedResource {
                        sector: attempt.sector().clone(),
                        location,
                        stage: interruption.stage(),
                        interruption: interruption.clone(),
                    },
                );
            }
            return Err(
                GeneratedFamilyFixedPointProviderError::FixedPointInterruptedFailure {
                    sector: attempt.sector().clone(),
                    location,
                    stage: interruption.stage(),
                    interruption: interruption.clone(),
                },
            );
        }
    }
    for status in certificate.final_statuses() {
        match status.status() {
            GeneratedFamilyFixedPointFinalStatus::ResourceLimited { interruption } => {
                return Err(
                    GeneratedFamilyFixedPointProviderError::FixedPointInterruptedResource {
                        sector: status.sector().clone(),
                        location:
                            GeneratedFamilyFixedPointProviderInterruptionLocation::FinalStatus,
                        stage: interruption.stage(),
                        interruption: interruption.clone(),
                    },
                );
            }
            GeneratedFamilyFixedPointFinalStatus::Failed { interruption } => {
                return Err(
                    GeneratedFamilyFixedPointProviderError::FixedPointInterruptedFailure {
                        sector: status.sector().clone(),
                        location:
                            GeneratedFamilyFixedPointProviderInterruptionLocation::FinalStatus,
                        stage: interruption.stage(),
                        interruption: interruption.clone(),
                    },
                );
            }
            _ => {}
        }
    }
    Ok(())
}

fn wrap_sector_error(
    error: ParametricSectorRuleProviderError,
) -> GeneratedFamilyFixedPointProviderError {
    wrap_conditional_error(GeneratedSectorConditionalRuleProviderError::Inner(error))
}

fn wrap_conditional_error(
    error: GeneratedFamilyFixedPointConditionalProviderError,
) -> GeneratedFamilyFixedPointProviderError {
    wrap_master_error(MasterPolicyError::Inner(error))
}

fn wrap_master_error(
    error: GeneratedFamilyFixedPointMasterProviderError,
) -> GeneratedFamilyFixedPointProviderError {
    wrap_symmetry_error(CertifiedSymmetryCanonicalizingRuleProviderError::Inner(
        error,
    ))
}

fn wrap_symmetry_error(
    error: GeneratedFamilyFixedPointSymmetryProviderError,
) -> GeneratedFamilyFixedPointProviderError {
    GeneratedFamilyFixedPointProviderError::Provider(CertifiedZeroSectorRuleProviderError::Inner(
        error,
    ))
}

fn map_stack_build_error(
    error: GeneratedProviderStackBuildError,
) -> GeneratedFamilyFixedPointProviderError {
    match error {
        GeneratedProviderStackBuildError::InputTerminalResource { requested, limit } => {
            GeneratedFamilyFixedPointProviderError::ResourceLimit {
                resource: "fixed-point provider input terminal declarations",
                requested,
                limit,
            }
        }
        GeneratedProviderStackBuildError::ResourceCountOverflow { resource } => {
            GeneratedFamilyFixedPointProviderError::ResourceCountOverflow { resource }
        }
        GeneratedProviderStackBuildError::Sector(error) => wrap_sector_error(error),
        GeneratedProviderStackBuildError::Conditional(error) => wrap_conditional_error(error),
        GeneratedProviderStackBuildError::Master(error) => wrap_master_error(error),
        GeneratedProviderStackBuildError::Symmetry(error) => wrap_symmetry_error(error),
        GeneratedProviderStackBuildError::Zero(error) => {
            GeneratedFamilyFixedPointProviderError::Provider(error)
        }
    }
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedFamilyFixedPointProviderError> {
    let requested = left
        .checked_add(right)
        .ok_or(GeneratedFamilyFixedPointProviderError::ResourceCountOverflow { resource })?;
    if requested > limit {
        Err(GeneratedFamilyFixedPointProviderError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(requested)
    }
}

#[derive(Debug)]
pub enum GeneratedFamilyFixedPointProviderError {
    BaseInterruptedResource {
        sector: SectorMask,
        stage: GeneratedFamilyPipelineStage,
        resource: GeneratedFamilySectorResource,
    },
    BaseInterruptedFailure {
        sector: SectorMask,
        stage: GeneratedFamilyPipelineStage,
        failure: GeneratedFamilySectorFailure,
    },
    FixedPointInterruptedResource {
        sector: SectorMask,
        location: GeneratedFamilyFixedPointProviderInterruptionLocation,
        stage: GeneratedFamilyFixedPointStage,
        interruption: GeneratedFamilyFixedPointInterruption,
    },
    FixedPointInterruptedFailure {
        sector: SectorMask,
        location: GeneratedFamilyFixedPointProviderInterruptionLocation,
        stage: GeneratedFamilyFixedPointStage,
        interruption: GeneratedFamilyFixedPointInterruption,
    },
    ZeroAnalysisLimitsMismatch {
        certificate: crate::ZeroSectorLimits,
        provider: crate::ZeroSectorLimits,
    },
    MissingSharedRowSpan,
    ReplayMismatch {
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
    Certificate(GeneratedFamilyFixedPointError),
    Provider(GeneratedFamilyFixedPointProviderStackError),
}

impl fmt::Display for GeneratedFamilyFixedPointProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BaseInterruptedResource {
                sector,
                stage,
                resource,
            } => write!(
                formatter,
                "fixed-point provider rejected base sector {sector} interrupted by a {stage:?} resource: {resource:?}"
            ),
            Self::BaseInterruptedFailure {
                sector,
                stage,
                failure,
            } => write!(
                formatter,
                "fixed-point provider rejected base sector {sector} interrupted by a {stage:?} failure: {failure:?}"
            ),
            Self::FixedPointInterruptedResource {
                sector,
                location,
                stage,
                interruption,
            } => write!(
                formatter,
                "fixed-point provider rejected sector {sector} interrupted by a resource at {location:?}/{stage:?}: {interruption:?}"
            ),
            Self::FixedPointInterruptedFailure {
                sector,
                location,
                stage,
                interruption,
            } => write!(
                formatter,
                "fixed-point provider rejected sector {sector} interrupted by a failure at {location:?}/{stage:?}: {interruption:?}"
            ),
            Self::ZeroAnalysisLimitsMismatch {
                certificate,
                provider,
            } => write!(
                formatter,
                "fixed-point provider zero-analysis limits {provider:?} differ from the base certificate limits {certificate:?}"
            ),
            Self::MissingSharedRowSpan => formatter.write_str(
                "fixed-point provider has generated material but no base shared row span",
            ),
            Self::ReplayMismatch { detail } => {
                write!(formatter, "fixed-point provider replay mismatch: {detail}")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "fixed-point provider {resource} count overflowed usize"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "fixed-point provider {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::Certificate(error) => error.fmt(formatter),
            Self::Provider(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedFamilyFixedPointProviderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Certificate(error) => Some(error),
            Self::Provider(error) => Some(error),
            _ => None,
        }
    }
}

impl From<GeneratedFamilyFixedPointError> for GeneratedFamilyFixedPointProviderError {
    fn from(value: GeneratedFamilyFixedPointError) -> Self {
        Self::Certificate(value)
    }
}
