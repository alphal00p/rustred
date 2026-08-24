//! Concrete application of a replayed family-wide generated-rule system.
//!
//! This is the topology-independent bridge from
//! [`crate::GeneratedFamilyRuleSystemCertificate`] to the demand-driven
//! [`crate::ConcreteRuleProvider`] interface.  It installs only material that
//! the family certificate already owns:
//!
//! 1. globally valid parametric sector rules;
//! 2. condition-bound live-leaf rules as a fallback;
//! 3. an explicit caller master policy;
//! 4. proof-bearing canonicalization under the certificate's verified
//!    internal family symmetries; and
//! 5. the certificate's zero-sector policy as the outermost layer.
//!
//! The outer zero layer is intentional: selecting a key as a master cannot
//! override an analytic zero proof or a cut zero.  No unresolved key is ever
//! inferred to be a master.  An unsupported generated leaf remains the
//! original typed provider error after conditional fallback is exhausted.

use std::fmt;
use std::sync::Arc;

use crate::generated_provider_stack::{
    GeneratedProviderStack, GeneratedProviderStackBuildError, GeneratedStackConditionalProvider,
    GeneratedStackMasterProvider, GeneratedStackSectorProvider, GeneratedStackSymmetryProvider,
    build_generated_provider_stack, replay_generated_provider_symmetry,
};
use crate::reduction_engine::{ConcreteRuleDecision, ConcreteRuleProvider};
use crate::{
    CertifiedRewriteLimits, CertifiedSymmetryCanonicalizingRuleProviderError,
    CertifiedSymmetryCanonicalizingRuleProviderLimits, CertifiedZeroSectorRuleProviderError,
    ConcreteIntegralKey, GeneratedFamilyPipelineStage, GeneratedFamilyRuleSystemCertificate,
    GeneratedFamilyRuleSystemError, GeneratedFamilySectorFailure, GeneratedFamilySectorResource,
    GeneratedFamilySectorStatus, GeneratedSectorConditionalRuleProvider,
    GeneratedSectorConditionalRuleProviderBuildStats, GeneratedSectorConditionalRuleProviderError,
    GeneratedSectorConditionalRuleProviderLimits, IntegralFamily, MasterPolicyError,
    MasterPolicyLimits, MasterPolicyTerminal, ParametricCoefficientContext,
    ParametricSectorRuleProvider, ParametricSectorRuleProviderError,
    ParametricSectorRuleProviderLimits, SectorMask,
};

pub const GENERATED_FAMILY_RULE_SYSTEM_PROVIDER_V2_SCHEMA: &str =
    "rustred.generated-family-rule-system-provider.v2";

pub type GeneratedFamilyConditionalProviderError =
    GeneratedSectorConditionalRuleProviderError<ParametricSectorRuleProviderError>;
pub type GeneratedFamilyMasterProviderError =
    MasterPolicyError<GeneratedFamilyConditionalProviderError>;
pub type GeneratedFamilySymmetryProviderError =
    CertifiedSymmetryCanonicalizingRuleProviderError<GeneratedFamilyMasterProviderError>;
pub type GeneratedFamilyRuleSystemProviderStackError =
    CertifiedZeroSectorRuleProviderError<GeneratedFamilySymmetryProviderError>;

type SectorProvider<'family> = GeneratedStackSectorProvider<'family>;
type ConditionalProvider<'family> = GeneratedStackConditionalProvider<'family>;
type MasterProvider<'family> = GeneratedStackMasterProvider<'family>;
type SymmetryProvider<'family> = GeneratedStackSymmetryProvider<'family>;
type ProviderStack<'family> = GeneratedProviderStack<'family>;

/// Provider-local retention and query limits.  Nested limits are retained
/// exactly, while the four outer caps bound aggregate family material before
/// any certificate payload is cloned into a provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedFamilyRuleSystemProviderLimits {
    pub sector_rules: ParametricSectorRuleProviderLimits,
    pub conditional_rules: GeneratedSectorConditionalRuleProviderLimits,
    pub master_policy: MasterPolicyLimits,
    pub symmetry: CertifiedSymmetryCanonicalizingRuleProviderLimits,
    pub certified_rewrite: CertifiedRewriteLimits,
    /// Caller declarations consumed during construction, before
    /// symmetry-orbit deduplication. This bounds canonicalization work even
    /// for duplicate or nonterminating iterators.
    pub max_input_terminals: usize,
    pub max_retained_generated_sectors: usize,
    pub max_total_candidate_attempts: usize,
    pub max_total_global_leaves: usize,
    pub max_total_live_leaf_work_items: usize,
}

impl Default for GeneratedFamilyRuleSystemProviderLimits {
    fn default() -> Self {
        Self {
            sector_rules: ParametricSectorRuleProviderLimits::default(),
            conditional_rules: GeneratedSectorConditionalRuleProviderLimits::default(),
            master_policy: MasterPolicyLimits::default(),
            symmetry: CertifiedSymmetryCanonicalizingRuleProviderLimits::default(),
            certified_rewrite: CertifiedRewriteLimits::default(),
            max_input_terminals: 10_000_000,
            max_retained_generated_sectors: 1_000_000,
            max_total_candidate_attempts: 16_000_000,
            max_total_global_leaves: 16_000_000,
            max_total_live_leaf_work_items: 16_000_000,
        }
    }
}

/// Immutable retained-input census, except that explicit master-policy counts
/// track checked insertions/removals made through this provider.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedFamilyRuleSystemProviderBuildStats {
    sector_transcripts: usize,
    excluded_sectors: usize,
    proved_zero_sectors: usize,
    retained_generated_sectors: usize,
    candidate_attempts: usize,
    global_leaves: usize,
    live_leaf_work_items: usize,
    master_terminals: usize,
    master_certificate_fingerprint_bytes: usize,
    conditional: GeneratedSectorConditionalRuleProviderBuildStats,
}

impl GeneratedFamilyRuleSystemProviderBuildStats {
    pub const fn sector_transcripts(self) -> usize {
        self.sector_transcripts
    }
    pub const fn excluded_sectors(self) -> usize {
        self.excluded_sectors
    }
    pub const fn proved_zero_sectors(self) -> usize {
        self.proved_zero_sectors
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

/// A family-wide concrete provider with the stable ordering
/// `zero(symmetry(master(conditional(global))))`.
pub struct GeneratedFamilyRuleSystemProvider<'family> {
    family: &'family IntegralFamily,
    context: &'family ParametricCoefficientContext,
    certificate: GeneratedFamilyRuleSystemCertificate,
    stack: ProviderStack<'family>,
    limits: GeneratedFamilyRuleSystemProviderLimits,
    build_stats: GeneratedFamilyRuleSystemProviderBuildStats,
}

impl<'family> GeneratedFamilyRuleSystemProvider<'family> {
    pub const SCHEMA: &'static str = GENERATED_FAMILY_RULE_SYSTEM_PROVIDER_V2_SCHEMA;

    /// Construct with no master declarations.  Every uncovered integral stays
    /// uncovered, and every unsupported leaf stays a typed error.
    pub fn try_new(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        certificate: GeneratedFamilyRuleSystemCertificate,
        limits: GeneratedFamilyRuleSystemProviderLimits,
    ) -> Result<Self, GeneratedFamilyRuleSystemProviderError> {
        Self::try_with_terminals(family, context, certificate, [], limits)
    }

    /// Construct with caller-owned, explicit master declarations.  The
    /// declarations are installed inside the zero-sector layer and therefore
    /// cannot shadow a zero proof.
    pub fn try_with_terminals(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        certificate: GeneratedFamilyRuleSystemCertificate,
        terminals: impl IntoIterator<Item = (ConcreteIntegralKey, MasterPolicyTerminal)>,
        limits: GeneratedFamilyRuleSystemProviderLimits,
    ) -> Result<Self, GeneratedFamilyRuleSystemProviderError> {
        let terminals = terminals.into_iter();
        let mut build_stats = preflight_certificate(&certificate, limits)?;
        let has_interrupted_sector = reject_interrupted_sectors(&certificate).is_err();
        if !has_interrupted_sector {
            ParametricSectorRuleProvider::preflight_certificates(
                family,
                context,
                certificate.sectors().iter().filter_map(|transcript| {
                    if let GeneratedFamilySectorStatus::Unresolved { discovery, .. } =
                        transcript.status()
                    {
                        Some(discovery.coverage())
                    } else {
                        None
                    }
                }),
                limits.sector_rules,
            )
            .map_err(wrap_sector_error)?;
            GeneratedSectorConditionalRuleProvider::<SectorProvider<'family>>::preflight_queues(
                family,
                context,
                certificate.sectors().iter().filter_map(|transcript| {
                    if let GeneratedFamilySectorStatus::Unresolved {
                        live_leaf_queue, ..
                    } = transcript.status()
                    {
                        Some(live_leaf_queue)
                    } else {
                        None
                    }
                }),
                limits.conditional_rules,
            )
            .map_err(wrap_conditional_error)?;
        }
        certificate.replay(family, context)?;
        reject_interrupted_sectors(&certificate)?;

        // Preserve retained-certificate interruption precedence, then reject
        // a known-oversized declaration stream before cloning provider
        // material. The per-item check below remains authoritative for
        // iterators with an inexact lower bound.
        let terminal_lower_bound = terminals.size_hint().0;
        if terminal_lower_bound > limits.max_input_terminals {
            return Err(GeneratedFamilyRuleSystemProviderError::ResourceLimit {
                resource: "family provider input terminal declarations",
                requested: terminal_lower_bound,
                limit: limits.max_input_terminals,
            });
        }

        let certificate_zero_limits = certificate.limits().inventory.zero_sectors;
        if limits.certified_rewrite.zero_sector != certificate_zero_limits {
            return Err(
                GeneratedFamilyRuleSystemProviderError::ZeroAnalysisLimitsMismatch {
                    certificate: certificate_zero_limits,
                    provider: limits.certified_rewrite.zero_sector,
                },
            );
        }

        let mut coverages = Vec::with_capacity(build_stats.retained_generated_sectors);
        let mut queues = Vec::with_capacity(build_stats.retained_generated_sectors);
        for transcript in certificate.sectors() {
            if let GeneratedFamilySectorStatus::Unresolved {
                discovery,
                live_leaf_queue,
                ..
            } = transcript.status()
            {
                coverages.push(discovery.coverage().clone());
                queues.push(live_leaf_queue.clone());
            }
        }

        let shared_row_span = certificate.row_span_arc().cloned();
        let built = build_generated_provider_stack(
            family,
            context,
            certificate.inventory_restrictions().clone(),
            certificate.inventory_power_shift_policy(),
            certificate.ordering(),
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
        certificate: GeneratedFamilyRuleSystemCertificate,
        selected: impl IntoIterator<Item = ConcreteIntegralKey>,
        limits: GeneratedFamilyRuleSystemProviderLimits,
    ) -> Result<Self, GeneratedFamilyRuleSystemProviderError> {
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
    pub const fn certificate(&self) -> &GeneratedFamilyRuleSystemCertificate {
        &self.certificate
    }
    pub const fn inventory_restrictions(&self) -> &crate::SectorRestrictions {
        self.certificate.inventory_restrictions()
    }
    pub const fn inventory_power_shift_policy(&self) -> crate::PowerShiftPolicy {
        self.certificate.inventory_power_shift_policy()
    }
    pub const fn ordering(&self) -> crate::IntegralOrderingPolicy {
        self.certificate.ordering()
    }
    pub const fn limits(&self) -> GeneratedFamilyRuleSystemProviderLimits {
        self.limits
    }
    pub const fn build_stats(&self) -> GeneratedFamilyRuleSystemProviderBuildStats {
        self.build_stats
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
    ) -> Result<(), GeneratedFamilyRuleSystemProviderError> {
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
    ) -> Result<(), GeneratedFamilyRuleSystemProviderError> {
        self.insert_terminal(integral, MasterPolicyTerminal::Selected)
    }

    pub fn insert_certified_master(
        &mut self,
        integral: ConcreteIntegralKey,
        certificate_fingerprint: impl Into<Arc<str>>,
    ) -> Result<(), GeneratedFamilyRuleSystemProviderError> {
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
    ) -> Result<bool, GeneratedFamilyRuleSystemProviderError> {
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

    /// Replay the original family certificate, every installed global and
    /// conditional proof, and all policy/provider bindings.  Runtime counters
    /// are deliberately excluded.
    pub fn replay(&self) -> Result<(), GeneratedFamilyRuleSystemProviderError> {
        self.certificate.replay(self.family, self.context)?;
        reject_interrupted_sectors(&self.certificate)?;
        self.validate_binding()?;
        replay_generated_provider_symmetry(
            self.symmetry_provider(),
            self.certificate.row_span_arc(),
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
            return Err(GeneratedFamilyRuleSystemProviderError::ReplayMismatch {
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
        self.build_stats.master_terminals = self.stack.inner().inner().terminals().len();
        self.build_stats.master_certificate_fingerprint_bytes = self
            .stack
            .inner()
            .inner()
            .total_certificate_fingerprint_bytes();
    }

    fn validate_binding(&self) -> Result<(), GeneratedFamilyRuleSystemProviderError> {
        if self.family.fingerprint() != self.certificate.family_fingerprint() {
            return Err(GeneratedFamilyRuleSystemProviderError::ReplayMismatch {
                detail: "provider family differs from the family certificate",
            });
        }
        if self.context.fingerprint() != self.certificate.context_fingerprint() {
            return Err(GeneratedFamilyRuleSystemProviderError::ReplayMismatch {
                detail: "provider context differs from the family certificate",
            });
        }
        if self.stack.restrictions() != self.certificate.inventory_restrictions()
            || self.stack.analyzer().policy() != self.certificate.inventory_power_shift_policy()
            || self.stack.analyzer().limits() != self.certificate.limits().inventory.zero_sectors
            || self.stack.rewrite_limits() != self.limits.certified_rewrite
            || self.symmetry_provider().family().fingerprint() != self.family.fingerprint()
            || self.symmetry_provider().context().fingerprint() != self.context.fingerprint()
            || self.symmetry_provider().restrictions() != self.certificate.inventory_restrictions()
            || self.symmetry_provider().ordering() != self.certificate.ordering()
            || self.symmetry_provider().limits() != self.limits.symmetry
            || self.master_provider().limits() != self.limits.master_policy
            || self.conditional_provider().limits() != self.limits.conditional_rules
            || self.sector_provider().limits() != self.limits.sector_rules
        {
            return Err(GeneratedFamilyRuleSystemProviderError::ReplayMismatch {
                detail: "provider policy or nested limits differ from retained build inputs",
            });
        }

        let expected_count = self
            .certificate
            .sectors()
            .iter()
            .filter(|entry| entry.status().is_unresolved())
            .count();
        if self.sector_provider().certificates().len() != expected_count
            || self.conditional_provider().queues().len() != expected_count
        {
            return Err(GeneratedFamilyRuleSystemProviderError::ReplayMismatch {
                detail: "installed generated-sector set differs from the family certificate",
            });
        }
        let shared_row_span = self.certificate.row_span_arc();
        if expected_count > 0 && shared_row_span.is_none() {
            return Err(GeneratedFamilyRuleSystemProviderError::ReplayMismatch {
                detail: "installed generated sectors have no family-shared row span",
            });
        }
        match (self.symmetry_provider().row_span_arc(), shared_row_span) {
            (Some(installed), Some(expected)) if Arc::ptr_eq(installed, expected) => {}
            (None, None) => {}
            _ => {
                return Err(GeneratedFamilyRuleSystemProviderError::ReplayMismatch {
                    detail: "symmetry provider lost the family-shared row-span allocation",
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
                return Err(GeneratedFamilyRuleSystemProviderError::ReplayMismatch {
                    detail: "explicit master policy contains a noncanonical symmetry-orbit key",
                });
            }
        }

        let shared_row_span = if expected_count == 0 {
            None
        } else {
            Some(
                shared_row_span.ok_or(GeneratedFamilyRuleSystemProviderError::ReplayMismatch {
                    detail: "installed generated sectors have no family-shared row span",
                })?,
            )
        };

        for transcript in self.certificate.sectors() {
            let GeneratedFamilySectorStatus::Unresolved {
                discovery,
                live_leaf_queue,
                ..
            } = transcript.status()
            else {
                continue;
            };
            let installed = self
                .sector_provider()
                .certificates()
                .get(transcript.sector())
                .ok_or(GeneratedFamilyRuleSystemProviderError::ReplayMismatch {
                    detail: "family generated sector is absent from the global provider",
                })?;
            if !installed.payload_eq(discovery.coverage()) {
                return Err(GeneratedFamilyRuleSystemProviderError::ReplayMismatch {
                    detail: "installed global coverage differs from the family certificate",
                });
            }
            if !Arc::ptr_eq(
                installed.row_span_arc(),
                shared_row_span.ok_or(GeneratedFamilyRuleSystemProviderError::ReplayMismatch {
                    detail: "installed global coverage has no family-shared row span",
                })?,
            ) {
                return Err(GeneratedFamilyRuleSystemProviderError::ReplayMismatch {
                    detail: "installed global coverage lost the family-shared row-span allocation",
                });
            }
            let installed_queue = self
                .conditional_provider()
                .queues()
                .find(|queue| queue.sector() == transcript.sector())
                .ok_or(GeneratedFamilyRuleSystemProviderError::ReplayMismatch {
                    detail: "family generated sector is absent from the conditional provider",
                })?;
            if !installed_queue.payload_eq(live_leaf_queue) {
                return Err(GeneratedFamilyRuleSystemProviderError::ReplayMismatch {
                    detail: "installed conditional queue differs from the family certificate",
                });
            }
            if !Arc::ptr_eq(
                installed_queue.discovery().row_span_arc(),
                shared_row_span.ok_or(GeneratedFamilyRuleSystemProviderError::ReplayMismatch {
                    detail: "installed conditional queue has no family-shared row span",
                })?,
            ) {
                return Err(GeneratedFamilyRuleSystemProviderError::ReplayMismatch {
                    detail: "installed conditional queue lost the family-shared row-span allocation",
                });
            }
        }
        Ok(())
    }
}

impl ConcreteRuleProvider for GeneratedFamilyRuleSystemProvider<'_> {
    type Error = GeneratedFamilyRuleSystemProviderError;

    fn index_arity(&self) -> usize {
        self.family.denominator_count()
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        self.stack
            .decision_for(integral)
            .map_err(GeneratedFamilyRuleSystemProviderError::Provider)
    }
}

fn preflight_certificate(
    certificate: &GeneratedFamilyRuleSystemCertificate,
    limits: GeneratedFamilyRuleSystemProviderLimits,
) -> Result<GeneratedFamilyRuleSystemProviderBuildStats, GeneratedFamilyRuleSystemProviderError> {
    current_build_stats(
        certificate,
        GeneratedSectorConditionalRuleProviderBuildStats::default(),
        0,
        0,
        limits,
    )
}

fn current_build_stats(
    certificate: &GeneratedFamilyRuleSystemCertificate,
    conditional: GeneratedSectorConditionalRuleProviderBuildStats,
    master_terminals: usize,
    master_certificate_fingerprint_bytes: usize,
    limits: GeneratedFamilyRuleSystemProviderLimits,
) -> Result<GeneratedFamilyRuleSystemProviderBuildStats, GeneratedFamilyRuleSystemProviderError> {
    let mut stats = GeneratedFamilyRuleSystemProviderBuildStats {
        sector_transcripts: certificate.sectors().len(),
        master_terminals,
        master_certificate_fingerprint_bytes,
        conditional,
        ..GeneratedFamilyRuleSystemProviderBuildStats::default()
    };
    for transcript in certificate.sectors() {
        match transcript.status() {
            GeneratedFamilySectorStatus::Excluded(_) => stats.excluded_sectors += 1,
            GeneratedFamilySectorStatus::ProvedZero(_) => stats.proved_zero_sectors += 1,
            GeneratedFamilySectorStatus::Unresolved {
                discovery,
                live_leaf_queue,
                ..
            } => {
                stats.retained_generated_sectors = bounded_add(
                    "family provider generated sectors",
                    stats.retained_generated_sectors,
                    1,
                    limits.max_retained_generated_sectors,
                )?;
                stats.candidate_attempts = bounded_add(
                    "family provider candidate attempts",
                    stats.candidate_attempts,
                    discovery.coverage().candidate_attempts().len(),
                    limits.max_total_candidate_attempts,
                )?;
                stats.global_leaves = bounded_add(
                    "family provider global leaves",
                    stats.global_leaves,
                    discovery.coverage().classifications().len(),
                    limits.max_total_global_leaves,
                )?;
                stats.live_leaf_work_items = bounded_add(
                    "family provider live-leaf work items",
                    stats.live_leaf_work_items,
                    live_leaf_queue.work_items().len(),
                    limits.max_total_live_leaf_work_items,
                )?;
            }
            GeneratedFamilySectorStatus::ResourceLimited { .. }
            | GeneratedFamilySectorStatus::Failed { .. } => {}
        }
    }
    Ok(stats)
}

fn reject_interrupted_sectors(
    certificate: &GeneratedFamilyRuleSystemCertificate,
) -> Result<(), GeneratedFamilyRuleSystemProviderError> {
    for transcript in certificate.sectors() {
        match transcript.status() {
            GeneratedFamilySectorStatus::ResourceLimited { resource, .. } => {
                return Err(
                    GeneratedFamilyRuleSystemProviderError::InterruptedResource {
                        sector: transcript.sector().clone(),
                        stage: resource.stage(),
                        resource: resource.clone(),
                    },
                );
            }
            GeneratedFamilySectorStatus::Failed { failure, .. } => {
                return Err(GeneratedFamilyRuleSystemProviderError::InterruptedFailure {
                    sector: transcript.sector().clone(),
                    stage: failure.stage(),
                    failure: failure.clone(),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn wrap_sector_error(
    error: ParametricSectorRuleProviderError,
) -> GeneratedFamilyRuleSystemProviderError {
    wrap_conditional_error(GeneratedSectorConditionalRuleProviderError::Inner(error))
}

fn wrap_conditional_error(
    error: GeneratedFamilyConditionalProviderError,
) -> GeneratedFamilyRuleSystemProviderError {
    wrap_master_error(MasterPolicyError::Inner(error))
}

fn wrap_master_error(
    error: GeneratedFamilyMasterProviderError,
) -> GeneratedFamilyRuleSystemProviderError {
    wrap_symmetry_error(CertifiedSymmetryCanonicalizingRuleProviderError::Inner(
        error,
    ))
}

fn wrap_symmetry_error(
    error: GeneratedFamilySymmetryProviderError,
) -> GeneratedFamilyRuleSystemProviderError {
    GeneratedFamilyRuleSystemProviderError::Provider(CertifiedZeroSectorRuleProviderError::Inner(
        error,
    ))
}

fn map_stack_build_error(
    error: GeneratedProviderStackBuildError,
) -> GeneratedFamilyRuleSystemProviderError {
    match error {
        GeneratedProviderStackBuildError::InputTerminalResource { requested, limit } => {
            GeneratedFamilyRuleSystemProviderError::ResourceLimit {
                resource: "family provider input terminal declarations",
                requested,
                limit,
            }
        }
        GeneratedProviderStackBuildError::ResourceCountOverflow { resource } => {
            GeneratedFamilyRuleSystemProviderError::ResourceCountOverflow { resource }
        }
        GeneratedProviderStackBuildError::Sector(error) => wrap_sector_error(error),
        GeneratedProviderStackBuildError::Conditional(error) => wrap_conditional_error(error),
        GeneratedProviderStackBuildError::Master(error) => wrap_master_error(error),
        GeneratedProviderStackBuildError::Symmetry(error) => wrap_symmetry_error(error),
        GeneratedProviderStackBuildError::Zero(error) => {
            GeneratedFamilyRuleSystemProviderError::Provider(error)
        }
    }
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedFamilyRuleSystemProviderError> {
    let requested = left
        .checked_add(right)
        .ok_or(GeneratedFamilyRuleSystemProviderError::ResourceCountOverflow { resource })?;
    if requested > limit {
        Err(GeneratedFamilyRuleSystemProviderError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(requested)
    }
}

#[derive(Debug)]
pub enum GeneratedFamilyRuleSystemProviderError {
    InterruptedResource {
        sector: SectorMask,
        stage: GeneratedFamilyPipelineStage,
        resource: GeneratedFamilySectorResource,
    },
    InterruptedFailure {
        sector: SectorMask,
        stage: GeneratedFamilyPipelineStage,
        failure: GeneratedFamilySectorFailure,
    },
    ZeroAnalysisLimitsMismatch {
        certificate: crate::ZeroSectorLimits,
        provider: crate::ZeroSectorLimits,
    },
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
    FamilyCertificate(GeneratedFamilyRuleSystemError),
    Provider(GeneratedFamilyRuleSystemProviderStackError),
}

impl fmt::Display for GeneratedFamilyRuleSystemProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InterruptedResource {
                sector,
                stage,
                resource,
            } => write!(
                formatter,
                "family provider rejected sector {sector} interrupted by a {stage:?} resource: {resource:?}"
            ),
            Self::InterruptedFailure {
                sector,
                stage,
                failure,
            } => write!(
                formatter,
                "family provider rejected sector {sector} interrupted by a {stage:?} failure: {failure:?}"
            ),
            Self::ZeroAnalysisLimitsMismatch {
                certificate,
                provider,
            } => write!(
                formatter,
                "family provider zero-analysis limits {provider:?} differ from the certificate inventory limits {certificate:?}"
            ),
            Self::ReplayMismatch { detail } => {
                write!(formatter, "family provider replay mismatch: {detail}")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "family provider {resource} count overflowed usize"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "family provider {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::FamilyCertificate(error) => error.fmt(formatter),
            Self::Provider(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedFamilyRuleSystemProviderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::FamilyCertificate(error) => Some(error),
            Self::Provider(error) => Some(error),
            _ => None,
        }
    }
}

impl From<GeneratedFamilyRuleSystemError> for GeneratedFamilyRuleSystemProviderError {
    fn from(value: GeneratedFamilyRuleSystemError) -> Self {
        Self::FamilyCertificate(value)
    }
}
