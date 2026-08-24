//! Generic, replayable family-wide generated-rule orchestration.
//!
//! This layer composes the topology-independent parts of RustRed's current
//! LiteRed-style pipeline:
//!
//! 1. enumerate and analyze every raw sector;
//! 2. visit only sectors for which the sufficient zero test returned a
//!    full-column-rank witness, in the inventory's certified subsector-first
//!    order;
//! 3. derive an initial symbolic rule covering from freshly generated IBP/LI
//!    rows; and
//! 4. retain the exceptional leaves in the generated live-leaf queue.
//!
//! A full-column-rank witness is only a failure of the current sufficient
//! zero criterion.  Consequently even a successfully compiled sector remains
//! [`GeneratedFamilySectorStatus::Unresolved`]: this module never infers a
//! master integral, analytic non-vanishing, or a topology-specific recurrence.
//!
//! `SectorRestrictions` and `PowerShiftPolicy` govern the inventory only.
//! The current generated discovery/queue APIs do not accept either policy:
//! they compile family IBP/LI rows and handle every move outside the selected
//! orthant conservatively as a sector leak/exceptional leaf.  This is sound
//! under cuts and formal shifts but may retain work that a future
//! restriction-aware generated stage could discard.  Accordingly the
//! certificate exposes these values as `inventory_*` metadata and never
//! claims that they were applied to generated-stage rules.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::{
    AdaptiveRuleSearchError, CoordinateEqualityLocusError, CylindricalOrderingError,
    CylindricalPreparePointError, CylindricalPreparePointScheduleError, ExactAlgebraError,
    FamilySectorInventoryCertificate, FamilySectorInventoryCompiler, FamilySectorInventoryError,
    FamilySectorInventoryLimits, FamilySectorInventoryStatus, FeynmanPolynomialError,
    FullColumnRankWitness, GeneratedCylindricalCandidateAuthorityError,
    GeneratedCylindricalPersistentEliminationError, GeneratedCylindricalResidualStartError,
    GeneratedCylindricalRowSystemError, GeneratedCylindricalSectorRootStartError,
    GeneratedPartialReeliminationError, GeneratedSectorDiscoveryCertificate,
    GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryError,
    GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCertificate,
    GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueError,
    GeneratedSectorLiveLeafQueueLimits, GeneratedSymbolicRowSpanCertificate,
    GeneratedSymbolicRowSpanCompiler, GeneratedSymbolicRowSpanError, GeneratedWhenBadError,
    IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext,
    ParametricCoefficientError, ParametricEliminationError, ParametricIbpError,
    ParametricRelationError, ParametricRuleError, ParametricSectorCoverageError, PowerShiftPolicy,
    SectorExclusion, SectorFoundationError, SectorMask, SectorRestrictions,
    SymbolicSectorCaseError, WhenBadCompilerError, ZeroSectorCertificate, ZeroSectorError,
    ZeroSectorResource,
};

pub const GENERATED_FAMILY_RULE_SYSTEM_V1_SCHEMA: &str = "rustred.generated-family-rule-system.v1";

/// The exact family-wide orchestration strategy retained in the certificate.
///
/// There is deliberately only one sound production strategy at present.  The
/// enum makes a future algorithm change explicit and schema-bound rather than
/// silently changing the meaning of an existing certificate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GeneratedFamilyRuleSystemStrategy {
    #[default]
    InventoryDiscoveryAndLiveLeafQueue,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedFamilyRuleSystemConfig {
    pub strategy: GeneratedFamilyRuleSystemStrategy,
}

/// Nested proof/search limits plus small family-wide transcript caps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedFamilyRuleSystemLimits {
    pub inventory: FamilySectorInventoryLimits,
    pub discovery: GeneratedSectorDiscoveryLimits,
    pub live_leaf_queue: GeneratedSectorLiveLeafQueueLimits,
    pub max_sector_transcripts: usize,
    pub max_unresolved_sector_attempts: usize,
}

impl Default for GeneratedFamilyRuleSystemLimits {
    fn default() -> Self {
        Self {
            inventory: FamilySectorInventoryLimits::default(),
            discovery: GeneratedSectorDiscoveryLimits::default(),
            live_leaf_queue: GeneratedSectorLiveLeafQueueLimits::default(),
            max_sector_transcripts: 1_048_576,
            max_unresolved_sector_attempts: 1_048_576,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedFamilyPipelineStage {
    Inventory,
    Discovery,
    LiveLeafQueue,
}

/// Exact resource interruption retained for one sector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedFamilySectorResource {
    Inventory(ZeroSectorResource),
    Discovery(GeneratedSectorDiscoveryError),
    LiveLeafQueue(GeneratedSectorLiveLeafQueueError),
}

impl GeneratedFamilySectorResource {
    pub const fn stage(&self) -> GeneratedFamilyPipelineStage {
        match self {
            Self::Inventory(_) => GeneratedFamilyPipelineStage::Inventory,
            Self::Discovery(_) => GeneratedFamilyPipelineStage::Discovery,
            Self::LiveLeafQueue(_) => GeneratedFamilyPipelineStage::LiveLeafQueue,
        }
    }
}

/// Exact non-resource interruption retained for one sector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedFamilySectorFailure {
    Inventory(ZeroSectorError),
    Discovery(GeneratedSectorDiscoveryError),
    LiveLeafQueue(GeneratedSectorLiveLeafQueueError),
}

impl GeneratedFamilySectorFailure {
    pub const fn stage(&self) -> GeneratedFamilyPipelineStage {
        match self {
            Self::Inventory(_) => GeneratedFamilyPipelineStage::Inventory,
            Self::Discovery(_) => GeneratedFamilyPipelineStage::Discovery,
            Self::LiveLeafQueue(_) => GeneratedFamilyPipelineStage::LiveLeafQueue,
        }
    }
}

/// Complete per-sector result.  `Unresolved` is intentionally not named
/// `Solved`: the payload is a generated conditional rewrite transcript, not a
/// proof that every integral in the sector reduces or a declaration of its
/// masters.
#[derive(Clone, Debug)]
pub enum GeneratedFamilySectorStatus {
    Excluded(SectorExclusion),
    ProvedZero(ZeroSectorCertificate),
    Unresolved {
        no_zero_certificate: FullColumnRankWitness,
        solve_ordinal: usize,
        discovery: GeneratedSectorDiscoveryCertificate,
        live_leaf_queue: GeneratedSectorLiveLeafQueueCertificate,
    },
    ResourceLimited {
        no_zero_certificate: Option<FullColumnRankWitness>,
        solve_ordinal: Option<usize>,
        completed_discovery: Option<GeneratedSectorDiscoveryCertificate>,
        resource: GeneratedFamilySectorResource,
    },
    Failed {
        no_zero_certificate: Option<FullColumnRankWitness>,
        solve_ordinal: Option<usize>,
        completed_discovery: Option<GeneratedSectorDiscoveryCertificate>,
        failure: GeneratedFamilySectorFailure,
    },
}

impl GeneratedFamilySectorStatus {
    pub const fn is_excluded(&self) -> bool {
        matches!(self, Self::Excluded(_))
    }

    pub const fn is_proved_zero(&self) -> bool {
        matches!(self, Self::ProvedZero(_))
    }

    pub const fn is_unresolved(&self) -> bool {
        matches!(self, Self::Unresolved { .. })
    }

    pub const fn is_resource_limited(&self) -> bool {
        matches!(self, Self::ResourceLimited { .. })
    }

    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// The sufficient zero-test witness, if this sector reached the generated
    /// solve queue.  Its presence is not a nonzero proof.
    pub const fn no_zero_certificate(&self) -> Option<&FullColumnRankWitness> {
        match self {
            Self::Unresolved {
                no_zero_certificate,
                ..
            } => Some(no_zero_certificate),
            Self::ResourceLimited {
                no_zero_certificate,
                ..
            }
            | Self::Failed {
                no_zero_certificate,
                ..
            } => no_zero_certificate.as_ref(),
            Self::Excluded(_) | Self::ProvedZero(_) => None,
        }
    }

    pub const fn solve_ordinal(&self) -> Option<usize> {
        match self {
            Self::Unresolved { solve_ordinal, .. } => Some(*solve_ordinal),
            Self::ResourceLimited { solve_ordinal, .. } | Self::Failed { solve_ordinal, .. } => {
                *solve_ordinal
            }
            Self::Excluded(_) | Self::ProvedZero(_) => None,
        }
    }

    pub const fn completed_discovery(&self) -> Option<&GeneratedSectorDiscoveryCertificate> {
        match self {
            Self::Unresolved { discovery, .. } => Some(discovery),
            Self::ResourceLimited {
                completed_discovery,
                ..
            }
            | Self::Failed {
                completed_discovery,
                ..
            } => completed_discovery.as_ref(),
            Self::Excluded(_) | Self::ProvedZero(_) => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GeneratedFamilySectorTranscript {
    sector: SectorMask,
    status: GeneratedFamilySectorStatus,
}

impl GeneratedFamilySectorTranscript {
    pub const fn sector(&self) -> &SectorMask {
        &self.sector
    }

    pub const fn status(&self) -> &GeneratedFamilySectorStatus {
        &self.status
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedFamilyRuleSystemStats {
    shared_row_span_compilation_attempts: usize,
    shared_row_span_certificates: usize,
    shared_row_span_sector_reuses: usize,
    shared_row_span_candidate_reuses: usize,
    sector_transcripts: usize,
    excluded: usize,
    proved_zero: usize,
    unresolved: usize,
    resource_limited: usize,
    failed: usize,
    discovery_attempts: usize,
    completed_discoveries: usize,
    live_leaf_queue_attempts: usize,
    completed_live_leaf_queues: usize,
    generated_candidate_attempts: usize,
    generated_global_leaves: usize,
    queued_exceptional_leaves: usize,
}

macro_rules! family_stats_getters {
    ($($name:ident),+ $(,)?) => {$ (
        pub const fn $name(self) -> usize { self.$name }
    )+ };
}

impl GeneratedFamilyRuleSystemStats {
    family_stats_getters!(
        shared_row_span_compilation_attempts,
        shared_row_span_certificates,
        shared_row_span_sector_reuses,
        shared_row_span_candidate_reuses,
        sector_transcripts,
        excluded,
        proved_zero,
        unresolved,
        resource_limited,
        failed,
        discovery_attempts,
        completed_discoveries,
        live_leaf_queue_attempts,
        completed_live_leaf_queues,
        generated_candidate_attempts,
        generated_global_leaves,
        queued_exceptional_leaves,
    );
}

/// Family-wide certificate retaining the complete inventory and every
/// generated-sector outcome in raw-mask order.  `solve_order` separately
/// records the exact subsector-first visitation order.
#[derive(Clone, Debug)]
pub struct GeneratedFamilyRuleSystemCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    inventory_restrictions: SectorRestrictions,
    inventory_power_shift_policy: PowerShiftPolicy,
    ordering: IntegralOrderingPolicy,
    config: GeneratedFamilyRuleSystemConfig,
    limits: GeneratedFamilyRuleSystemLimits,
    inventory: FamilySectorInventoryCertificate,
    row_span: Option<Arc<GeneratedSymbolicRowSpanCertificate>>,
    row_span_interruption: Option<GeneratedSymbolicRowSpanError>,
    solve_order: Box<[SectorMask]>,
    sectors: Box<[GeneratedFamilySectorTranscript]>,
    stats: GeneratedFamilyRuleSystemStats,
}

impl GeneratedFamilyRuleSystemCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }
    /// Restrictions used by family inventory/zero-sector analysis.  Current
    /// generated-stage APIs remain restriction-agnostic and conservative.
    pub const fn inventory_restrictions(&self) -> &SectorRestrictions {
        &self.inventory_restrictions
    }

    /// Power-shift policy used by family inventory/zero-sector analysis.
    pub const fn inventory_power_shift_policy(&self) -> PowerShiftPolicy {
        self.inventory_power_shift_policy
    }
    pub const fn ordering(&self) -> IntegralOrderingPolicy {
        self.ordering
    }
    pub const fn config(&self) -> GeneratedFamilyRuleSystemConfig {
        self.config
    }
    pub const fn limits(&self) -> GeneratedFamilyRuleSystemLimits {
        self.limits
    }
    pub const fn inventory(&self) -> &FamilySectorInventoryCertificate {
        &self.inventory
    }
    /// The single generated IBP/LI(+verified symmetry transport) source basis
    /// shared by every unresolved sector.  It is absent when the inventory
    /// contains no generated-stage work.
    pub fn row_span_arc(&self) -> Option<&Arc<GeneratedSymbolicRowSpanCertificate>> {
        self.row_span.as_ref()
    }
    pub const fn row_span_interruption(&self) -> Option<&GeneratedSymbolicRowSpanError> {
        self.row_span_interruption.as_ref()
    }
    pub fn solve_order(&self) -> &[SectorMask] {
        &self.solve_order
    }
    pub fn sectors(&self) -> &[GeneratedFamilySectorTranscript] {
        &self.sectors
    }
    pub fn status(&self, sector: &SectorMask) -> Option<&GeneratedFamilySectorStatus> {
        self.sectors
            .binary_search_by(|entry| entry.sector.cmp(sector))
            .ok()
            .map(|position| &self.sectors[position].status)
    }
    pub const fn stats(&self) -> GeneratedFamilyRuleSystemStats {
        self.stats
    }

    /// Replay every retained proof and rerun every interrupted stage with the
    /// exact stored policies.  Successful nested certificates compare their
    /// own complete payloads; interrupted stages must reproduce the exact
    /// typed error.
    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedFamilyRuleSystemError> {
        validate_scope(family, context)?;
        if self.schema != GENERATED_FAMILY_RULE_SYSTEM_V1_SCHEMA {
            return Err(GeneratedFamilyRuleSystemError::SchemaMismatch);
        }
        if self.family_fingerprint.as_ref() != family.fingerprint() {
            return Err(GeneratedFamilyRuleSystemError::WrongFamily);
        }
        if self.context_fingerprint.as_ref() != context.fingerprint() {
            return Err(GeneratedFamilyRuleSystemError::WrongContext);
        }
        validate_strategy(self.config.strategy)?;
        self.inventory.replay(family)?;
        validate_inventory_binding(
            &self.inventory,
            &self.inventory_restrictions,
            self.inventory_power_shift_policy,
            self.ordering,
            self.limits.inventory,
        )?;
        check_limit(
            "family generated-rule sector transcripts",
            self.sectors.len(),
            self.limits.max_sector_transcripts,
        )?;
        check_limit(
            "family generated-rule unresolved sector attempts",
            self.inventory.unresolved_solve_order().len(),
            self.limits.max_unresolved_sector_attempts,
        )?;
        let expected_shared_row_span = !self.inventory.unresolved_solve_order().is_empty();
        if (self.row_span.is_some() || self.row_span_interruption.is_some())
            != expected_shared_row_span
            || (self.row_span.is_some() && self.row_span_interruption.is_some())
        {
            return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
                detail: "shared row-span outcome differs from generated-stage work",
            });
        }
        if let Some(row_span) = &self.row_span {
            validate_family_row_span_binding(family, context, row_span, self.limits.discovery)?;
            row_span.replay(family, context)?;
        } else if let Some(expected) = &self.row_span_interruption {
            match GeneratedSymbolicRowSpanCompiler::compile(
                family,
                context,
                self.limits.discovery.coverage.generated_when_bad.ibp,
                self.limits.discovery.coverage.generated_when_bad.row_span,
            ) {
                Err(actual) if actual == *expected => {}
                _ => {
                    return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
                        detail: "shared row-span interruption differs on replay",
                    });
                }
            }
        }
        self.verify_and_replay_sectors(family, context)?;
        let recomputed = compute_stats(&self.sectors, self.row_span.is_some())?;
        if recomputed != self.stats {
            return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
                detail: "family-wide sector census differs",
            });
        }
        Ok(())
    }

    fn verify_and_replay_sectors(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedFamilyRuleSystemError> {
        check_limit(
            "family generated-rule sector transcripts",
            self.sectors.len(),
            self.limits.max_sector_transcripts,
        )?;

        if self.sectors.len() != self.inventory.entries().len()
            || !self
                .sectors
                .windows(2)
                .all(|pair| pair[0].sector < pair[1].sector)
        {
            return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
                detail: "family sector transcript set differs from the inventory",
            });
        }
        let expected_order = self
            .inventory
            .unresolved_solve_order()
            .iter()
            .map(|entry| entry.sector().clone())
            .collect::<Vec<_>>();
        check_limit(
            "family generated-rule unresolved sector attempts",
            expected_order.len(),
            self.limits.max_unresolved_sector_attempts,
        )?;
        if expected_order.as_slice() != self.solve_order.as_ref() {
            return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
                detail: "unresolved sector solve order differs from the inventory",
            });
        }
        let ordinals = expected_order
            .iter()
            .cloned()
            .enumerate()
            .map(|(ordinal, sector)| (sector, ordinal))
            .collect::<BTreeMap<_, _>>();

        for (inventory_entry, transcript) in
            self.inventory.entries().iter().zip(self.sectors.iter())
        {
            if inventory_entry.sector() != transcript.sector() {
                return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
                    detail: "raw sector transcript order differs from the inventory",
                });
            }
            match (inventory_entry.status(), transcript.status()) {
                (
                    FamilySectorInventoryStatus::Excluded(expected),
                    GeneratedFamilySectorStatus::Excluded(actual),
                ) if expected == actual => {}
                (
                    FamilySectorInventoryStatus::ProvedZero(expected),
                    GeneratedFamilySectorStatus::ProvedZero(actual),
                ) if expected == actual => {
                    actual.replay(family)?;
                }
                (
                    FamilySectorInventoryStatus::ResourceLimited(expected),
                    GeneratedFamilySectorStatus::ResourceLimited {
                        no_zero_certificate: None,
                        solve_ordinal: None,
                        completed_discovery: None,
                        resource: GeneratedFamilySectorResource::Inventory(actual),
                    },
                ) if expected == actual => {}
                (
                    FamilySectorInventoryStatus::Failed(expected),
                    GeneratedFamilySectorStatus::Failed {
                        no_zero_certificate: None,
                        solve_ordinal: None,
                        completed_discovery: None,
                        failure: GeneratedFamilySectorFailure::Inventory(actual),
                    },
                ) if expected == actual => {}
                (
                    FamilySectorInventoryStatus::UnresolvedNoZeroCertificate(expected_witness),
                    status,
                ) => {
                    let expected_ordinal = *ordinals.get(transcript.sector()).ok_or(
                        GeneratedFamilyRuleSystemError::ReplayMismatch {
                            detail: "unresolved sector is absent from the solve order",
                        },
                    )?;
                    if let Some(row_span) = self.row_span.as_ref().cloned() {
                        replay_unresolved_status(
                            family,
                            context,
                            transcript.sector(),
                            expected_witness,
                            expected_ordinal,
                            status,
                            self.ordering,
                            row_span,
                            self.limits,
                        )?;
                    } else {
                        replay_unresolved_row_span_interruption(
                            expected_witness,
                            expected_ordinal,
                            status,
                            self.row_span_interruption.as_ref().ok_or(
                                GeneratedFamilyRuleSystemError::ReplayMismatch {
                                    detail: "unresolved sector has no shared row-span outcome",
                                },
                            )?,
                        )?;
                    }
                }
                _ => {
                    return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
                        detail: "family sector status differs from the inventory",
                    });
                }
            }
        }
        Ok(())
    }
}

pub struct GeneratedFamilyRuleSystemCompiler;

impl GeneratedFamilyRuleSystemCompiler {
    #[allow(clippy::too_many_arguments)]
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        restrictions: SectorRestrictions,
        power_shift_policy: PowerShiftPolicy,
        ordering: IntegralOrderingPolicy,
        config: GeneratedFamilyRuleSystemConfig,
        limits: GeneratedFamilyRuleSystemLimits,
    ) -> Result<GeneratedFamilyRuleSystemCertificate, GeneratedFamilyRuleSystemError> {
        validate_scope(family, context)?;
        validate_strategy(config.strategy)?;
        let inventory = FamilySectorInventoryCompiler::compile(
            family,
            restrictions.clone(),
            power_shift_policy,
            ordering,
            limits.inventory,
        )?;
        check_limit(
            "family generated-rule sector transcripts",
            inventory.entries().len(),
            limits.max_sector_transcripts,
        )?;
        check_limit(
            "family generated-rule unresolved sector attempts",
            inventory.unresolved_solve_order().len(),
            limits.max_unresolved_sector_attempts,
        )?;

        let (row_span, row_span_interruption) = if inventory.unresolved_solve_order().is_empty() {
            (None, None)
        } else {
            match GeneratedSymbolicRowSpanCompiler::compile(
                family,
                context,
                limits.discovery.coverage.generated_when_bad.ibp,
                limits.discovery.coverage.generated_when_bad.row_span,
            ) {
                Ok(certificate) => (Some(Arc::new(certificate)), None),
                Err(error) => (None, Some(error)),
            }
        };

        let mut statuses = inventory
            .entries()
            .iter()
            .map(|entry| {
                let status = match entry.status() {
                    FamilySectorInventoryStatus::Excluded(exclusion) => {
                        Some(GeneratedFamilySectorStatus::Excluded(exclusion.clone()))
                    }
                    FamilySectorInventoryStatus::ProvedZero(certificate) => {
                        Some(GeneratedFamilySectorStatus::ProvedZero(certificate.clone()))
                    }
                    FamilySectorInventoryStatus::ResourceLimited(resource) => {
                        Some(GeneratedFamilySectorStatus::ResourceLimited {
                            no_zero_certificate: None,
                            solve_ordinal: None,
                            completed_discovery: None,
                            resource: GeneratedFamilySectorResource::Inventory(resource.clone()),
                        })
                    }
                    FamilySectorInventoryStatus::Failed(error) => {
                        Some(GeneratedFamilySectorStatus::Failed {
                            no_zero_certificate: None,
                            solve_ordinal: None,
                            completed_discovery: None,
                            failure: GeneratedFamilySectorFailure::Inventory(error.clone()),
                        })
                    }
                    FamilySectorInventoryStatus::UnresolvedNoZeroCertificate(_) => None,
                };
                (entry.sector().clone(), status)
            })
            .collect::<Vec<_>>();
        let positions = statuses
            .iter()
            .enumerate()
            .map(|(position, (sector, _))| (sector.clone(), position))
            .collect::<BTreeMap<_, _>>();

        for (solve_ordinal, solve_entry) in inventory.unresolved_solve_order().iter().enumerate() {
            let position = *positions.get(solve_entry.sector()).ok_or(
                GeneratedFamilyRuleSystemError::InternalInvariant(
                    "inventory solve sector has no raw-sector transcript",
                ),
            )?;
            let witness = match inventory.status(solve_entry.sector()) {
                Some(FamilySectorInventoryStatus::UnresolvedNoZeroCertificate(witness)) => {
                    witness.clone()
                }
                _ => {
                    return Err(GeneratedFamilyRuleSystemError::InternalInvariant(
                        "inventory solve order contains a sector without a full-rank witness",
                    ));
                }
            };
            let status = if let Some(row_span) = row_span.as_ref().cloned() {
                compile_unresolved_sector(
                    family,
                    context,
                    solve_entry.sector(),
                    witness,
                    solve_ordinal,
                    ordering,
                    row_span,
                    limits,
                )
            } else {
                unresolved_row_span_interruption_status(
                    witness,
                    solve_ordinal,
                    row_span_interruption.as_ref().ok_or(
                        GeneratedFamilyRuleSystemError::InternalInvariant(
                            "unresolved sector has no shared row-span outcome",
                        ),
                    )?,
                )
            };
            if statuses[position].1.replace(status).is_some() {
                return Err(GeneratedFamilyRuleSystemError::InternalInvariant(
                    "an unresolved sector was compiled more than once",
                ));
            }
        }

        let sectors = statuses
            .into_iter()
            .map(|(sector, status)| {
                Ok(GeneratedFamilySectorTranscript {
                    sector,
                    status: status.ok_or(GeneratedFamilyRuleSystemError::InternalInvariant(
                        "an unresolved sector was omitted from the solve order",
                    ))?,
                })
            })
            .collect::<Result<Vec<_>, GeneratedFamilyRuleSystemError>>()?;
        let stats = compute_stats(&sectors, row_span.is_some())?;
        let solve_order = inventory
            .unresolved_solve_order()
            .iter()
            .map(|entry| entry.sector().clone())
            .collect::<Vec<_>>();
        let certificate = GeneratedFamilyRuleSystemCertificate {
            schema: GENERATED_FAMILY_RULE_SYSTEM_V1_SCHEMA,
            family_fingerprint: family.fingerprint().into(),
            context_fingerprint: context.fingerprint().into(),
            inventory_restrictions: restrictions,
            inventory_power_shift_policy: power_shift_policy,
            ordering,
            config,
            limits,
            inventory,
            row_span,
            row_span_interruption,
            solve_order: solve_order.into_boxed_slice(),
            sectors: sectors.into_boxed_slice(),
            stats,
        };
        certificate.verify_and_replay_sectors(family, context)?;
        Ok(certificate)
    }
}

fn compile_unresolved_sector(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    sector: &SectorMask,
    witness: FullColumnRankWitness,
    solve_ordinal: usize,
    ordering: IntegralOrderingPolicy,
    row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    limits: GeneratedFamilyRuleSystemLimits,
) -> GeneratedFamilySectorStatus {
    let discovery = match GeneratedSectorDiscoveryCompiler::compile_with_replayed_row_span(
        family,
        context,
        sector.clone(),
        ordering,
        row_span.clone(),
        limits.discovery,
    ) {
        Ok(discovery) => discovery,
        Err(error) if discovery_error_is_resource(&error) => {
            return GeneratedFamilySectorStatus::ResourceLimited {
                no_zero_certificate: Some(witness),
                solve_ordinal: Some(solve_ordinal),
                completed_discovery: None,
                resource: GeneratedFamilySectorResource::Discovery(error),
            };
        }
        Err(error) => {
            return GeneratedFamilySectorStatus::Failed {
                no_zero_certificate: Some(witness),
                solve_ordinal: Some(solve_ordinal),
                completed_discovery: None,
                failure: GeneratedFamilySectorFailure::Discovery(error),
            };
        }
    };
    match GeneratedSectorLiveLeafQueueCompiler::compile_with_replayed_row_span(
        family,
        context,
        &discovery,
        row_span,
        limits.live_leaf_queue,
    ) {
        Ok(live_leaf_queue) => GeneratedFamilySectorStatus::Unresolved {
            no_zero_certificate: witness,
            solve_ordinal,
            discovery,
            live_leaf_queue,
        },
        Err(error) if queue_error_is_resource(&error) => {
            GeneratedFamilySectorStatus::ResourceLimited {
                no_zero_certificate: Some(witness),
                solve_ordinal: Some(solve_ordinal),
                completed_discovery: Some(discovery),
                resource: GeneratedFamilySectorResource::LiveLeafQueue(error),
            }
        }
        Err(error) => GeneratedFamilySectorStatus::Failed {
            no_zero_certificate: Some(witness),
            solve_ordinal: Some(solve_ordinal),
            completed_discovery: Some(discovery),
            failure: GeneratedFamilySectorFailure::LiveLeafQueue(error),
        },
    }
}

fn unresolved_row_span_interruption_status(
    witness: FullColumnRankWitness,
    solve_ordinal: usize,
    error: &GeneratedSymbolicRowSpanError,
) -> GeneratedFamilySectorStatus {
    let error = GeneratedSectorDiscoveryError::RowSpan(error.clone());
    if discovery_error_is_resource(&error) {
        GeneratedFamilySectorStatus::ResourceLimited {
            no_zero_certificate: Some(witness),
            solve_ordinal: Some(solve_ordinal),
            completed_discovery: None,
            resource: GeneratedFamilySectorResource::Discovery(error),
        }
    } else {
        GeneratedFamilySectorStatus::Failed {
            no_zero_certificate: Some(witness),
            solve_ordinal: Some(solve_ordinal),
            completed_discovery: None,
            failure: GeneratedFamilySectorFailure::Discovery(error),
        }
    }
}

fn replay_unresolved_row_span_interruption(
    expected_witness: &FullColumnRankWitness,
    expected_ordinal: usize,
    status: &GeneratedFamilySectorStatus,
    expected: &GeneratedSymbolicRowSpanError,
) -> Result<(), GeneratedFamilyRuleSystemError> {
    let expected_discovery = GeneratedSectorDiscoveryError::RowSpan(expected.clone());
    let matches = if discovery_error_is_resource(&expected_discovery) {
        matches!(
            status,
            GeneratedFamilySectorStatus::ResourceLimited {
                no_zero_certificate: Some(actual_witness),
                solve_ordinal: Some(actual_ordinal),
                completed_discovery: None,
                resource: GeneratedFamilySectorResource::Discovery(actual),
            } if actual_witness == expected_witness
                && *actual_ordinal == expected_ordinal
                && actual == &expected_discovery
        )
    } else {
        matches!(
            status,
            GeneratedFamilySectorStatus::Failed {
                no_zero_certificate: Some(actual_witness),
                solve_ordinal: Some(actual_ordinal),
                completed_discovery: None,
                failure: GeneratedFamilySectorFailure::Discovery(actual),
            } if actual_witness == expected_witness
                && *actual_ordinal == expected_ordinal
                && actual == &expected_discovery
        )
    };
    if matches {
        Ok(())
    } else {
        Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
            detail: "sector row-span interruption differs from the shared family outcome",
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn replay_unresolved_status(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    sector: &SectorMask,
    expected_witness: &FullColumnRankWitness,
    expected_ordinal: usize,
    status: &GeneratedFamilySectorStatus,
    ordering: IntegralOrderingPolicy,
    row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    limits: GeneratedFamilyRuleSystemLimits,
) -> Result<(), GeneratedFamilyRuleSystemError> {
    if status.no_zero_certificate() != Some(expected_witness)
        || status.solve_ordinal() != Some(expected_ordinal)
    {
        return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
            detail: "unresolved sector witness or solve ordinal differs",
        });
    }
    match status {
        GeneratedFamilySectorStatus::Unresolved {
            discovery,
            live_leaf_queue,
            ..
        } => {
            validate_discovery_binding(discovery, sector, ordering, limits.discovery)?;
            validate_shared_discovery_allocation(discovery, &row_span)?;
            discovery.replay_with_replayed_row_span(family, context, row_span.clone())?;
            validate_queue_binding(live_leaf_queue, discovery, limits.live_leaf_queue)?;
            validate_shared_discovery_allocation(live_leaf_queue.discovery(), &row_span)?;
            live_leaf_queue.replay_with_replayed_row_span(family, context, row_span)?;
        }
        GeneratedFamilySectorStatus::ResourceLimited {
            completed_discovery,
            resource,
            ..
        } => match resource {
            GeneratedFamilySectorResource::Discovery(expected) => {
                if completed_discovery.is_some() || !discovery_error_is_resource(expected) {
                    return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
                        detail: "discovery resource transcript has an invalid stage payload",
                    });
                }
                replay_discovery_error(
                    family,
                    context,
                    sector,
                    ordering,
                    row_span,
                    limits.discovery,
                    expected,
                )?;
            }
            GeneratedFamilySectorResource::LiveLeafQueue(expected) => {
                if !queue_error_is_resource(expected) {
                    return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
                        detail: "live-leaf resource transcript retains a non-resource error",
                    });
                }
                let discovery = completed_discovery.as_ref().ok_or(
                    GeneratedFamilyRuleSystemError::ReplayMismatch {
                        detail: "live-leaf resource transcript omitted completed discovery",
                    },
                )?;
                validate_shared_discovery_allocation(discovery, &row_span)?;
                replay_queue_error(
                    family, context, sector, ordering, row_span, limits, discovery, expected,
                )?;
            }
            GeneratedFamilySectorResource::Inventory(_) => {
                return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
                    detail: "an unresolved solve sector carries an inventory resource status",
                });
            }
        },
        GeneratedFamilySectorStatus::Failed {
            completed_discovery,
            failure,
            ..
        } => match failure {
            GeneratedFamilySectorFailure::Discovery(expected) => {
                if completed_discovery.is_some() || discovery_error_is_resource(expected) {
                    return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
                        detail: "discovery failure transcript has an invalid stage payload",
                    });
                }
                replay_discovery_error(
                    family,
                    context,
                    sector,
                    ordering,
                    row_span,
                    limits.discovery,
                    expected,
                )?;
            }
            GeneratedFamilySectorFailure::LiveLeafQueue(expected) => {
                if queue_error_is_resource(expected) {
                    return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
                        detail: "live-leaf failure transcript retains a resource error",
                    });
                }
                let discovery = completed_discovery.as_ref().ok_or(
                    GeneratedFamilyRuleSystemError::ReplayMismatch {
                        detail: "live-leaf failure transcript omitted completed discovery",
                    },
                )?;
                validate_shared_discovery_allocation(discovery, &row_span)?;
                replay_queue_error(
                    family, context, sector, ordering, row_span, limits, discovery, expected,
                )?;
            }
            GeneratedFamilySectorFailure::Inventory(_) => {
                return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
                    detail: "an unresolved solve sector carries an inventory failure status",
                });
            }
        },
        GeneratedFamilySectorStatus::Excluded(_) | GeneratedFamilySectorStatus::ProvedZero(_) => {
            return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
                detail: "an unresolved inventory sector lost its generated-stage transcript",
            });
        }
    }
    Ok(())
}

fn replay_discovery_error(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    sector: &SectorMask,
    ordering: IntegralOrderingPolicy,
    row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    limits: GeneratedSectorDiscoveryLimits,
    expected: &GeneratedSectorDiscoveryError,
) -> Result<(), GeneratedFamilyRuleSystemError> {
    match GeneratedSectorDiscoveryCompiler::compile_with_replayed_row_span(
        family,
        context,
        sector.clone(),
        ordering,
        row_span,
        limits,
    ) {
        Err(actual) if actual == *expected => Ok(()),
        _ => Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
            detail: "generated-sector discovery interruption differs on replay",
        }),
    }
}

fn replay_queue_error(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    sector: &SectorMask,
    ordering: IntegralOrderingPolicy,
    row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    limits: GeneratedFamilyRuleSystemLimits,
    discovery: &GeneratedSectorDiscoveryCertificate,
    expected: &GeneratedSectorLiveLeafQueueError,
) -> Result<(), GeneratedFamilyRuleSystemError> {
    validate_discovery_binding(discovery, sector, ordering, limits.discovery)?;
    validate_shared_discovery_allocation(discovery, &row_span)?;
    discovery.replay_with_replayed_row_span(family, context, row_span.clone())?;
    match GeneratedSectorLiveLeafQueueCompiler::compile_with_replayed_row_span(
        family,
        context,
        discovery,
        row_span,
        limits.live_leaf_queue,
    ) {
        Err(actual) if actual == *expected => Ok(()),
        _ => Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
            detail: "generated live-leaf queue interruption differs on replay",
        }),
    }
}

fn validate_discovery_binding(
    discovery: &GeneratedSectorDiscoveryCertificate,
    sector: &SectorMask,
    ordering: IntegralOrderingPolicy,
    limits: GeneratedSectorDiscoveryLimits,
) -> Result<(), GeneratedFamilyRuleSystemError> {
    if discovery.sector() != sector
        || discovery.ordering() != ordering
        || discovery.limits() != limits
    {
        return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
            detail: "completed discovery is bound to another sector, ordering, or policy",
        });
    }
    Ok(())
}

fn validate_shared_discovery_allocation(
    discovery: &GeneratedSectorDiscoveryCertificate,
    row_span: &Arc<GeneratedSymbolicRowSpanCertificate>,
) -> Result<(), GeneratedFamilyRuleSystemError> {
    if !Arc::ptr_eq(discovery.row_span_arc(), row_span)
        || !Arc::ptr_eq(discovery.coverage().row_span_arc(), row_span)
        || !discovery
            .coverage()
            .candidate_attempts()
            .iter()
            .all(|attempt| {
                Arc::ptr_eq(
                    attempt.compilation().source_authentication().row_span_arc(),
                    row_span,
                )
            })
    {
        return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
            detail: "generated sector does not retain the family-shared row-span allocation",
        });
    }
    Ok(())
}

fn validate_family_row_span_binding(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    row_span: &GeneratedSymbolicRowSpanCertificate,
    limits: GeneratedSectorDiscoveryLimits,
) -> Result<(), GeneratedFamilyRuleSystemError> {
    if row_span.family_fingerprint() != family.fingerprint() {
        return Err(GeneratedFamilyRuleSystemError::WrongFamily);
    }
    if row_span.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedFamilyRuleSystemError::WrongContext);
    }
    if row_span.ibp_config() != limits.coverage.generated_when_bad.ibp
        || row_span.config() != limits.coverage.generated_when_bad.row_span
    {
        return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
            detail: "family shared row-span configuration differs from discovery policy",
        });
    }
    Ok(())
}

fn validate_queue_binding(
    queue: &GeneratedSectorLiveLeafQueueCertificate,
    discovery: &GeneratedSectorDiscoveryCertificate,
    limits: GeneratedSectorLiveLeafQueueLimits,
) -> Result<(), GeneratedFamilyRuleSystemError> {
    if queue.sector() != discovery.sector()
        || queue.ordering() != discovery.ordering()
        || queue.limits() != limits
        || queue.discovery().family_fingerprint() != discovery.family_fingerprint()
        || queue.discovery().context_fingerprint() != discovery.context_fingerprint()
        || queue.discovery().sector() != discovery.sector()
        || queue.discovery().limits() != discovery.limits()
        || queue.discovery().stats() != discovery.stats()
    {
        return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
            detail: "live-leaf queue is not bound to the completed discovery",
        });
    }
    Ok(())
}

fn validate_inventory_binding(
    inventory: &FamilySectorInventoryCertificate,
    restrictions: &SectorRestrictions,
    power_shift_policy: PowerShiftPolicy,
    ordering: IntegralOrderingPolicy,
    limits: FamilySectorInventoryLimits,
) -> Result<(), GeneratedFamilyRuleSystemError> {
    if inventory.restrictions() != restrictions
        || inventory.power_shift_policy() != power_shift_policy
        || inventory.ordering() != ordering
        || inventory.limits() != limits
    {
        return Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
            detail: "family inventory policy binding differs",
        });
    }
    Ok(())
}

fn validate_scope(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
) -> Result<(), GeneratedFamilyRuleSystemError> {
    if !family
        .coefficient_context()
        .has_same_variable_map(context.base())
    {
        return Err(GeneratedFamilyRuleSystemError::WrongContext);
    }
    if family.denominator_count() != context.index_count() {
        return Err(GeneratedFamilyRuleSystemError::WrongArity {
            expected: family.denominator_count(),
            actual: context.index_count(),
        });
    }
    Ok(())
}

fn validate_strategy(
    strategy: GeneratedFamilyRuleSystemStrategy,
) -> Result<(), GeneratedFamilyRuleSystemError> {
    match strategy {
        GeneratedFamilyRuleSystemStrategy::InventoryDiscoveryAndLiveLeafQueue => Ok(()),
    }
}

fn compute_stats(
    sectors: &[GeneratedFamilySectorTranscript],
    has_shared_row_span_certificate: bool,
) -> Result<GeneratedFamilyRuleSystemStats, GeneratedFamilyRuleSystemError> {
    let has_generated_work = sectors
        .iter()
        .any(|transcript| transcript.status().no_zero_certificate().is_some());
    let mut stats = GeneratedFamilyRuleSystemStats {
        sector_transcripts: sectors.len(),
        shared_row_span_compilation_attempts: usize::from(has_generated_work),
        shared_row_span_certificates: usize::from(has_shared_row_span_certificate),
        ..GeneratedFamilyRuleSystemStats::default()
    };
    for transcript in sectors {
        match transcript.status() {
            GeneratedFamilySectorStatus::Excluded(_) => stats.excluded += 1,
            GeneratedFamilySectorStatus::ProvedZero(_) => stats.proved_zero += 1,
            GeneratedFamilySectorStatus::Unresolved {
                discovery,
                live_leaf_queue,
                ..
            } => {
                stats.unresolved += 1;
                stats.discovery_attempts += 1;
                stats.completed_discoveries += 1;
                stats.live_leaf_queue_attempts += 1;
                stats.completed_live_leaf_queues += 1;
                stats.shared_row_span_sector_reuses += 1;
                stats.shared_row_span_candidate_reuses = checked_add(
                    "family shared row-span candidate reuses",
                    stats.shared_row_span_candidate_reuses,
                    discovery.stats().candidate_attempts(),
                )?;
                add_discovery_stats(&mut stats, discovery)?;
                add_queue_stats(&mut stats, live_leaf_queue)?;
            }
            GeneratedFamilySectorStatus::ResourceLimited {
                no_zero_certificate,
                completed_discovery,
                resource,
                ..
            } => {
                stats.resource_limited += 1;
                if no_zero_certificate.is_some() {
                    stats.discovery_attempts += 1;
                    stats.shared_row_span_sector_reuses += 1;
                }
                if let Some(discovery) = completed_discovery {
                    stats.completed_discoveries += 1;
                    stats.live_leaf_queue_attempts += 1;
                    add_discovery_stats(&mut stats, discovery)?;
                    stats.shared_row_span_candidate_reuses = checked_add(
                        "family shared row-span candidate reuses",
                        stats.shared_row_span_candidate_reuses,
                        discovery.stats().candidate_attempts(),
                    )?;
                }
                if matches!(resource, GeneratedFamilySectorResource::Inventory(_))
                    && no_zero_certificate.is_some()
                {
                    return Err(GeneratedFamilyRuleSystemError::InternalInvariant(
                        "inventory resource status reached the generated solve queue",
                    ));
                }
            }
            GeneratedFamilySectorStatus::Failed {
                no_zero_certificate,
                completed_discovery,
                failure,
                ..
            } => {
                stats.failed += 1;
                if no_zero_certificate.is_some() {
                    stats.discovery_attempts += 1;
                    stats.shared_row_span_sector_reuses += 1;
                }
                if let Some(discovery) = completed_discovery {
                    stats.completed_discoveries += 1;
                    stats.live_leaf_queue_attempts += 1;
                    add_discovery_stats(&mut stats, discovery)?;
                    stats.shared_row_span_candidate_reuses = checked_add(
                        "family shared row-span candidate reuses",
                        stats.shared_row_span_candidate_reuses,
                        discovery.stats().candidate_attempts(),
                    )?;
                }
                if matches!(failure, GeneratedFamilySectorFailure::Inventory(_))
                    && no_zero_certificate.is_some()
                {
                    return Err(GeneratedFamilyRuleSystemError::InternalInvariant(
                        "inventory failure status reached the generated solve queue",
                    ));
                }
            }
        }
    }
    if !has_shared_row_span_certificate {
        // A shared row-span interruption occurs before any per-sector
        // discovery compiler is entered.  The scheduled sector statuses
        // retain that one common interruption, but are not discovery attempts.
        stats.discovery_attempts = 0;
        stats.shared_row_span_sector_reuses = 0;
        stats.shared_row_span_candidate_reuses = 0;
    }
    Ok(stats)
}

fn add_discovery_stats(
    aggregate: &mut GeneratedFamilyRuleSystemStats,
    discovery: &GeneratedSectorDiscoveryCertificate,
) -> Result<(), GeneratedFamilyRuleSystemError> {
    aggregate.generated_candidate_attempts = checked_add(
        "family generated candidate attempts",
        aggregate.generated_candidate_attempts,
        discovery.stats().candidate_attempts(),
    )?;
    aggregate.generated_global_leaves = checked_add(
        "family generated global leaves",
        aggregate.generated_global_leaves,
        discovery.stats().global_leaves(),
    )?;
    Ok(())
}

fn add_queue_stats(
    aggregate: &mut GeneratedFamilyRuleSystemStats,
    queue: &GeneratedSectorLiveLeafQueueCertificate,
) -> Result<(), GeneratedFamilyRuleSystemError> {
    aggregate.queued_exceptional_leaves = checked_add(
        "family queued exceptional leaves",
        aggregate.queued_exceptional_leaves,
        queue.stats().queued_leaves(),
    )?;
    Ok(())
}

// Resource classification is structural: a nested bounded-operation error
// remains a resource interruption at the family layer rather than being
// blurred into an algorithm failure.
pub(crate) fn discovery_error_is_resource(error: &GeneratedSectorDiscoveryError) -> bool {
    match error {
        GeneratedSectorDiscoveryError::ResourceCountOverflow { .. }
        | GeneratedSectorDiscoveryError::ResourceLimit { .. } => true,
        GeneratedSectorDiscoveryError::Ibp(error) => ibp_error_is_resource(error),
        GeneratedSectorDiscoveryError::RowSpan(error) => row_span_error_is_resource(error),
        GeneratedSectorDiscoveryError::Adaptive(error) => adaptive_error_is_resource(error),
        GeneratedSectorDiscoveryError::Coverage(error) => coverage_error_is_resource(error),
        GeneratedSectorDiscoveryError::Relation(error) => relation_error_is_resource(error),
        GeneratedSectorDiscoveryError::Sector(error) => sector_error_is_resource(error),
        _ => false,
    }
}

pub(crate) fn queue_error_is_resource(error: &GeneratedSectorLiveLeafQueueError) -> bool {
    match error {
        GeneratedSectorLiveLeafQueueError::ResourceCountOverflow { .. }
        | GeneratedSectorLiveLeafQueueError::ResourceLimit { .. } => true,
        GeneratedSectorLiveLeafQueueError::Discovery(error) => discovery_error_is_resource(error),
        GeneratedSectorLiveLeafQueueError::RowSpan(error) => row_span_error_is_resource(error),
        GeneratedSectorLiveLeafQueueError::Coordinate(error) => coordinate_error_is_resource(error),
        GeneratedSectorLiveLeafQueueError::PartialReelimination(error) => {
            partial_error_is_resource(error)
        }
        GeneratedSectorLiveLeafQueueError::Elimination(error) => {
            elimination_error_is_resource(error)
        }
        GeneratedSectorLiveLeafQueueError::Relation(error) => relation_error_is_resource(error),
        GeneratedSectorLiveLeafQueueError::Sector(error) => sector_error_is_resource(error),
        _ => false,
    }
}

fn row_span_error_is_resource(error: &GeneratedSymbolicRowSpanError) -> bool {
    match error {
        GeneratedSymbolicRowSpanError::IncompleteRequiredSearch
        | GeneratedSymbolicRowSpanError::ResourceCountOverflow { .. }
        | GeneratedSymbolicRowSpanError::ResourceLimit { .. } => true,
        GeneratedSymbolicRowSpanError::Ibp(error) => ibp_error_is_resource(error),
        GeneratedSymbolicRowSpanError::Search(error) => symmetry_search_error_is_resource(error),
        GeneratedSymbolicRowSpanError::SymmetryReplay(error) => {
            symmetry_replay_error_is_resource(error)
        }
        GeneratedSymbolicRowSpanError::Transport(error) => {
            symmetry_transport_error_is_resource(error)
        }
        GeneratedSymbolicRowSpanError::Sector(error) => sector_error_is_resource(error),
        _ => false,
    }
}

fn symmetry_search_error_is_resource(error: &crate::InternalSymmetrySearchError) -> bool {
    match error {
        crate::InternalSymmetrySearchError::ResourceCountOverflow { .. } => true,
        crate::InternalSymmetrySearchError::MatrixConstruction(error)
        | crate::InternalSymmetrySearchError::UnexpectedVerificationFailure(error) => {
            symmetry_verification_error_is_resource(error)
        }
        _ => false,
    }
}

fn symmetry_verification_error_is_resource(error: &crate::SymmetryVerificationError) -> bool {
    match error {
        crate::SymmetryVerificationError::AllocationFailure { .. }
        | crate::SymmetryVerificationError::ResourceCountOverflow { .. }
        | crate::SymmetryVerificationError::ResourceLimit { .. } => true,
        crate::SymmetryVerificationError::ExactAlgebra(error) => exact_error_is_resource(error),
        crate::SymmetryVerificationError::Family(error) => generic_family_error_is_resource(error),
        _ => false,
    }
}

fn symmetry_replay_error_is_resource(error: &crate::InternalSymmetryReplayError) -> bool {
    match error {
        crate::InternalSymmetryReplayError::AffineVerification(error) => {
            symmetry_verification_error_is_resource(error)
        }
        crate::InternalSymmetryReplayError::Compatibility(error) => {
            symmetry_compatibility_error_is_resource(error)
        }
        _ => false,
    }
}

fn symmetry_compatibility_error_is_resource(
    error: &crate::InternalSymmetryCompatibilityError,
) -> bool {
    matches!(
        error,
        crate::InternalSymmetryCompatibilityError::ResourceCountOverflow { .. }
            | crate::InternalSymmetryCompatibilityError::AllocationFailure { .. }
    )
}

fn symmetry_transport_error_is_resource(error: &crate::SymbolicSymmetryRowTransportError) -> bool {
    match error {
        crate::SymbolicSymmetryRowTransportError::ResourceLimit { .. }
        | crate::SymbolicSymmetryRowTransportError::ResourceCountOverflow { .. } => true,
        crate::SymbolicSymmetryRowTransportError::Coefficient(error) => {
            coefficient_error_is_resource(error)
        }
        crate::SymbolicSymmetryRowTransportError::Relation(error) => {
            relation_error_is_resource(error)
        }
        crate::SymbolicSymmetryRowTransportError::Symmetry(error) => {
            symmetry_replay_error_is_resource(error)
        }
        _ => false,
    }
}

pub(crate) fn adaptive_error_is_resource(error: &AdaptiveRuleSearchError) -> bool {
    match error {
        AdaptiveRuleSearchError::ResourceCountOverflow { .. }
        | AdaptiveRuleSearchError::ResourceLimit { .. } => true,
        AdaptiveRuleSearchError::Elimination(error) => elimination_error_is_resource(error),
        AdaptiveRuleSearchError::Relation(error) => relation_error_is_resource(error),
        AdaptiveRuleSearchError::Rule(error) => rule_error_is_resource(error),
        AdaptiveRuleSearchError::Sector(error) => sector_error_is_resource(error),
        _ => false,
    }
}

pub(crate) fn coverage_error_is_resource(error: &ParametricSectorCoverageError) -> bool {
    match error {
        ParametricSectorCoverageError::ResourceCountOverflow { .. }
        | ParametricSectorCoverageError::ResourceLimit { .. } => true,
        ParametricSectorCoverageError::GeneratedWhenBad(error) => {
            generated_when_bad_error_is_resource(error)
        }
        ParametricSectorCoverageError::SectorCase(error) => sector_case_error_is_resource(error),
        ParametricSectorCoverageError::CoordinateLocus(error) => {
            coordinate_error_is_resource(error)
        }
        ParametricSectorCoverageError::ParametricCoefficient(error) => {
            coefficient_error_is_resource(error)
        }
        _ => false,
    }
}

pub(crate) fn generated_when_bad_error_is_resource(error: &GeneratedWhenBadError) -> bool {
    match error {
        GeneratedWhenBadError::ResourceCountOverflow { .. }
        | GeneratedWhenBadError::ResourceLimit { .. } => true,
        GeneratedWhenBadError::Ibp(error) => ibp_error_is_resource(error),
        GeneratedWhenBadError::RowSpan(error) => row_span_error_is_resource(error),
        GeneratedWhenBadError::Relation(error) => relation_error_is_resource(error),
        GeneratedWhenBadError::Rule(error) => rule_error_is_resource(error),
        GeneratedWhenBadError::WhenBad(error) => when_bad_error_is_resource(error),
        _ => false,
    }
}

fn coordinate_error_is_resource(error: &CoordinateEqualityLocusError) -> bool {
    match error {
        CoordinateEqualityLocusError::ResourceCountOverflow { .. }
        | CoordinateEqualityLocusError::ResourceLimit { .. } => true,
        CoordinateEqualityLocusError::SourcePartition(error) => {
            sector_case_error_is_resource(error)
        }
        CoordinateEqualityLocusError::ParametricCoefficient(error) => {
            coefficient_error_is_resource(error)
        }
        _ => false,
    }
}

fn partial_error_is_resource(error: &GeneratedPartialReeliminationError) -> bool {
    match error {
        GeneratedPartialReeliminationError::ResourceCountOverflow { .. }
        | GeneratedPartialReeliminationError::ResourceLimit { .. }
        | GeneratedPartialReeliminationError::AllocationFailure { .. } => true,
        GeneratedPartialReeliminationError::Ibp(error) => ibp_error_is_resource(error),
        GeneratedPartialReeliminationError::Relation(error) => relation_error_is_resource(error),
        GeneratedPartialReeliminationError::Elimination(error) => {
            elimination_error_is_resource(error)
        }
        GeneratedPartialReeliminationError::Coefficient(error) => {
            coefficient_error_is_resource(error)
        }
        _ => false,
    }
}

fn elimination_error_is_resource(error: &ParametricEliminationError) -> bool {
    match error {
        ParametricEliminationError::ResourceCountOverflow { .. }
        | ParametricEliminationError::ResourceLimit { .. } => true,
        ParametricEliminationError::Coefficient(error) => coefficient_error_is_resource(error),
        ParametricEliminationError::Relation(error) => relation_error_is_resource(error),
        ParametricEliminationError::Sector(error) => sector_error_is_resource(error),
        _ => false,
    }
}

fn rule_error_is_resource(error: &ParametricRuleError) -> bool {
    match error {
        ParametricRuleError::ResourceCountOverflow { .. }
        | ParametricRuleError::ResourceLimit { .. } => true,
        ParametricRuleError::ExactAlgebra(error) => exact_error_is_resource(error),
        ParametricRuleError::ParametricCoefficient(error) => coefficient_error_is_resource(error),
        ParametricRuleError::Relation(error) => relation_error_is_resource(error),
        ParametricRuleError::Elimination(error) => elimination_error_is_resource(error),
        ParametricRuleError::Sector(error) => sector_error_is_resource(error),
        ParametricRuleError::GeneratedCylindricalCandidate(error) => {
            generated_cylindrical_candidate_error_is_resource(error)
        }
        ParametricRuleError::GeneratedCylindricalWhenBad(error) => {
            when_bad_error_is_resource(error)
        }
        _ => false,
    }
}

pub(crate) fn when_bad_error_is_resource(error: &WhenBadCompilerError) -> bool {
    match error {
        WhenBadCompilerError::ResourceCountOverflow { .. }
        | WhenBadCompilerError::ResourceLimit { .. } => true,
        WhenBadCompilerError::ParametricRule(error) => rule_error_is_resource(error),
        WhenBadCompilerError::ParametricCoefficient(error) => coefficient_error_is_resource(error),
        WhenBadCompilerError::SectorCase(error) => sector_case_error_is_resource(error),
        WhenBadCompilerError::GeneratedCylindricalCandidate(error) => {
            generated_cylindrical_candidate_error_is_resource(error.as_ref())
        }
        WhenBadCompilerError::Relation(error) => relation_error_is_resource(error),
        WhenBadCompilerError::Sector(error) => sector_error_is_resource(error),
        _ => false,
    }
}

fn generated_cylindrical_candidate_error_is_resource(
    error: &GeneratedCylindricalCandidateAuthorityError,
) -> bool {
    match error {
        GeneratedCylindricalCandidateAuthorityError::ResourceLimit { .. }
        | GeneratedCylindricalCandidateAuthorityError::ResourceCountOverflow { .. }
        | GeneratedCylindricalCandidateAuthorityError::AllocationFailure { .. } => true,
        GeneratedCylindricalCandidateAuthorityError::Source(error) => {
            generated_cylindrical_persistent_error_is_resource(error)
        }
        GeneratedCylindricalCandidateAuthorityError::Relation(error) => {
            relation_error_is_resource(error)
        }
        _ => false,
    }
}

fn generated_cylindrical_persistent_error_is_resource(
    error: &GeneratedCylindricalPersistentEliminationError,
) -> bool {
    match error {
        GeneratedCylindricalPersistentEliminationError::ResourceLimit { .. }
        | GeneratedCylindricalPersistentEliminationError::ResourceCountOverflow { .. }
        | GeneratedCylindricalPersistentEliminationError::AllocationFailure { .. } => true,
        GeneratedCylindricalPersistentEliminationError::RowSystem(error) => {
            generated_cylindrical_row_system_error_is_resource(error)
        }
        GeneratedCylindricalPersistentEliminationError::Ordering(error) => {
            cylindrical_ordering_error_is_resource(error)
        }
        GeneratedCylindricalPersistentEliminationError::Relation(error) => {
            relation_error_is_resource(error)
        }
        GeneratedCylindricalPersistentEliminationError::Elimination(error) => {
            elimination_error_is_resource(error)
        }
        _ => false,
    }
}

fn generated_cylindrical_row_system_error_is_resource(
    error: &GeneratedCylindricalRowSystemError,
) -> bool {
    match error {
        GeneratedCylindricalRowSystemError::ResourceLimit { .. }
        | GeneratedCylindricalRowSystemError::ResourceCountOverflow { .. }
        | GeneratedCylindricalRowSystemError::AllocationFailure { .. } => true,
        GeneratedCylindricalRowSystemError::Start(error) => {
            generated_cylindrical_residual_start_error_is_resource(error)
        }
        GeneratedCylindricalRowSystemError::SectorRootStart(error) => {
            generated_cylindrical_sector_root_start_error_is_resource(error)
        }
        GeneratedCylindricalRowSystemError::Relation(error) => relation_error_is_resource(error),
        _ => false,
    }
}

fn generated_cylindrical_sector_root_start_error_is_resource(
    error: &GeneratedCylindricalSectorRootStartError,
) -> bool {
    match error {
        GeneratedCylindricalSectorRootStartError::SourceSectorResourceLimited(_)
        | GeneratedCylindricalSectorRootStartError::ResourceLimit { .. }
        | GeneratedCylindricalSectorRootStartError::ResourceCountOverflow { .. } => true,
        GeneratedCylindricalSectorRootStartError::SourceSectorAnalysisFailed(error) => {
            zero_sector_error_is_resource(error)
        }
        GeneratedCylindricalSectorRootStartError::Inventory(error) => {
            inventory_error_is_resource(error)
        }
        GeneratedCylindricalSectorRootStartError::RowSpan(error) => {
            row_span_error_is_resource(error)
        }
        GeneratedCylindricalSectorRootStartError::Coefficient(error) => {
            coefficient_error_is_resource(error)
        }
        GeneratedCylindricalSectorRootStartError::Ordering(error) => {
            cylindrical_ordering_error_is_resource(error)
        }
        GeneratedCylindricalSectorRootStartError::Schedule(error) => {
            cylindrical_schedule_error_is_resource(error)
        }
        GeneratedCylindricalSectorRootStartError::Sector(error) => sector_error_is_resource(error),
        _ => false,
    }
}

fn generated_cylindrical_residual_start_error_is_resource(
    error: &GeneratedCylindricalResidualStartError,
) -> bool {
    match error {
        GeneratedCylindricalResidualStartError::WorkItemOrdinalLimit { .. }
        | GeneratedCylindricalResidualStartError::ResourceLimit { .. } => true,
        GeneratedCylindricalResidualStartError::Queue(error) => queue_error_is_resource(error),
        GeneratedCylindricalResidualStartError::Ordering(error) => {
            cylindrical_ordering_error_is_resource(error)
        }
        GeneratedCylindricalResidualStartError::Schedule(error) => {
            cylindrical_schedule_error_is_resource(error)
        }
        _ => false,
    }
}

fn cylindrical_ordering_error_is_resource(error: &CylindricalOrderingError) -> bool {
    matches!(
        error,
        CylindricalOrderingError::SignedComplexityOverflow { .. }
            | CylindricalOrderingError::ResourceLimitExceeded { .. }
            | CylindricalOrderingError::ResourceCountOverflow { .. }
            | CylindricalOrderingError::AllocationFailure { .. }
    )
}

fn cylindrical_schedule_error_is_resource(error: &CylindricalPreparePointScheduleError) -> bool {
    match error {
        CylindricalPreparePointScheduleError::DepthTooLarge { .. }
        | CylindricalPreparePointScheduleError::CumulativeResourceLimit { .. }
        | CylindricalPreparePointScheduleError::ResourceCountOverflow { .. } => true,
        CylindricalPreparePointScheduleError::LayerFailure { source, .. } => {
            cylindrical_prepare_point_error_is_resource(source)
        }
        CylindricalPreparePointScheduleError::Ordering(error) => {
            cylindrical_ordering_error_is_resource(error)
        }
        _ => false,
    }
}

fn cylindrical_prepare_point_error_is_resource(error: &CylindricalPreparePointError) -> bool {
    match error {
        CylindricalPreparePointError::DepthTooLarge { .. }
        | CylindricalPreparePointError::ResourceLimit { .. }
        | CylindricalPreparePointError::ResourceCountOverflow { .. } => true,
        CylindricalPreparePointError::Ordering(error) => {
            cylindrical_ordering_error_is_resource(error)
        }
        CylindricalPreparePointError::Relation(error) => relation_error_is_resource(error),
        _ => false,
    }
}

fn inventory_error_is_resource(error: &FamilySectorInventoryError) -> bool {
    match error {
        FamilySectorInventoryError::ResourceLimit { .. }
        | FamilySectorInventoryError::ResourceCountOverflow { .. } => true,
        FamilySectorInventoryError::ZeroSector(error) => zero_sector_error_is_resource(error),
        FamilySectorInventoryError::Sector(error) => sector_error_is_resource(error),
        _ => false,
    }
}

fn zero_sector_error_is_resource(error: &ZeroSectorError) -> bool {
    match error {
        ZeroSectorError::ResourceLimit { .. }
        | ZeroSectorError::ResourceCountOverflow { .. }
        | ZeroSectorError::MatrixDimensionOverflow { .. } => true,
        ZeroSectorError::ExactAlgebra(error) => exact_error_is_resource(error),
        ZeroSectorError::Feynman(error) => feynman_error_is_resource(error),
        ZeroSectorError::Sector(error) => sector_error_is_resource(error),
        _ => false,
    }
}

fn feynman_error_is_resource(error: &FeynmanPolynomialError) -> bool {
    match error {
        FeynmanPolynomialError::ResourceLimit { .. }
        | FeynmanPolynomialError::ResourceCountOverflow { .. }
        | FeynmanPolynomialError::ParameterExponentOverflow { .. } => true,
        FeynmanPolynomialError::ExactAlgebra(error) => exact_error_is_resource(error),
        _ => false,
    }
}

fn sector_case_error_is_resource(error: &SymbolicSectorCaseError) -> bool {
    match error {
        SymbolicSectorCaseError::CaseIdOverflow
        | SymbolicSectorCaseError::ResourceCountOverflow { .. }
        | SymbolicSectorCaseError::ResourceLimit { .. }
        | SymbolicSectorCaseError::AllocationFailure { .. } => true,
        SymbolicSectorCaseError::ParametricCoefficient(error) => {
            coefficient_error_is_resource(error)
        }
        _ => false,
    }
}

fn ibp_error_is_resource(error: &ParametricIbpError) -> bool {
    match error {
        ParametricIbpError::RowCountOverflow { .. } => true,
        ParametricIbpError::Coefficient(error) => coefficient_error_is_resource(error),
        ParametricIbpError::Relation(error) => relation_error_is_resource(error),
        ParametricIbpError::Family(error) => generic_family_error_is_resource(error),
        _ => false,
    }
}

fn generic_family_error_is_resource(error: &crate::GenericFamilyError) -> bool {
    match error {
        crate::GenericFamilyError::ScalarProductCountOverflow { .. }
        | crate::GenericFamilyError::ResourceCountOverflow { .. }
        | crate::GenericFamilyError::ResourceLimit { .. }
        | crate::GenericFamilyError::AllocationFailure { .. }
        | crate::GenericFamilyError::MatrixDimensionOverflow { .. } => true,
        crate::GenericFamilyError::InvalidCoefficient { error, .. }
        | crate::GenericFamilyError::ExactAlgebra(error) => exact_error_is_resource(error),
        _ => false,
    }
}

fn relation_error_is_resource(error: &ParametricRelationError) -> bool {
    match error {
        ParametricRelationError::ResourceCountOverflow { .. }
        | ParametricRelationError::ResourceLimit { .. }
        | ParametricRelationError::AllocationFailure { .. } => true,
        ParametricRelationError::Coefficient(error) => coefficient_error_is_resource(error),
        _ => false,
    }
}

fn coefficient_error_is_resource(error: &ParametricCoefficientError) -> bool {
    match error {
        ParametricCoefficientError::ResourceCountOverflow { .. }
        | ParametricCoefficientError::ResourceLimit { .. } => true,
        ParametricCoefficientError::ExactAlgebra(error) => exact_error_is_resource(error),
        _ => false,
    }
}

fn exact_error_is_resource(error: &ExactAlgebraError) -> bool {
    matches!(
        error,
        ExactAlgebraError::ConfiguredExponentLimit { .. }
            | ExactAlgebraError::ExponentLimit { .. }
            | ExactAlgebraError::ResourceCountOverflow { .. }
            | ExactAlgebraError::ResourceLimit { .. }
    )
}

fn sector_error_is_resource(error: &SectorFoundationError) -> bool {
    matches!(
        error,
        SectorFoundationError::EnumerationLimitExceeded { .. }
            | SectorFoundationError::AllocationFailure { .. }
            | SectorFoundationError::ComplexityOverflow { .. }
    )
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedFamilyRuleSystemError> {
    left.checked_add(right)
        .ok_or(GeneratedFamilyRuleSystemError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedFamilyRuleSystemError> {
    if requested > limit {
        Err(GeneratedFamilyRuleSystemError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedFamilyRuleSystemError {
    SchemaMismatch,
    ReplayMismatch {
        detail: &'static str,
    },
    WrongFamily,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    InternalInvariant(&'static str),
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Inventory(FamilySectorInventoryError),
    ZeroSector(ZeroSectorError),
    Discovery(GeneratedSectorDiscoveryError),
    LiveLeafQueue(GeneratedSectorLiveLeafQueueError),
}

impl fmt::Display for GeneratedFamilyRuleSystemError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("family generated-rule schema mismatch"),
            Self::ReplayMismatch { detail } => {
                write!(formatter, "family generated-rule replay mismatch: {detail}")
            }
            Self::WrongFamily => formatter.write_str("family generated-rule family mismatch"),
            Self::WrongContext => {
                formatter.write_str("family generated-rule coefficient-context mismatch")
            }
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "family generated-rule index arity is {actual}, expected {expected}"
            ),
            Self::InternalInvariant(detail) => {
                write!(
                    formatter,
                    "family generated-rule invariant failed: {detail}"
                )
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
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::Inventory(error) => error.fmt(formatter),
            Self::ZeroSector(error) => error.fmt(formatter),
            Self::Discovery(error) => error.fmt(formatter),
            Self::LiveLeafQueue(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedFamilyRuleSystemError {}

impl From<FamilySectorInventoryError> for GeneratedFamilyRuleSystemError {
    fn from(value: FamilySectorInventoryError) -> Self {
        Self::Inventory(value)
    }
}

impl From<ZeroSectorError> for GeneratedFamilyRuleSystemError {
    fn from(value: ZeroSectorError) -> Self {
        Self::ZeroSector(value)
    }
}

impl From<GeneratedSectorDiscoveryError> for GeneratedFamilyRuleSystemError {
    fn from(value: GeneratedSectorDiscoveryError) -> Self {
        Self::Discovery(value)
    }
}

impl From<GeneratedSectorLiveLeafQueueError> for GeneratedFamilyRuleSystemError {
    fn from(value: GeneratedSectorLiveLeafQueueError) -> Self {
        Self::LiveLeafQueue(value)
    }
}

impl From<GeneratedSymbolicRowSpanError> for GeneratedFamilyRuleSystemError {
    fn from(value: GeneratedSymbolicRowSpanError) -> Self {
        Self::Discovery(GeneratedSectorDiscoveryError::RowSpan(value))
    }
}

#[cfg(test)]
mod resource_classification_tests {
    use super::*;

    fn massive_tadpole() -> IntegralFamily {
        let coefficients = crate::CoefficientContext::new(["d", "m2"]);
        IntegralFamily::new(
            "family-replay-tamper",
            vec!["k".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![crate::AffineDenominator::new(
                coefficients.parse("-m2").unwrap(),
                vec![coefficients.one()],
            )],
            Vec::new(),
            vec![coefficients.zero()],
        )
        .unwrap()
    }

    #[test]
    fn nested_symmetry_resource_interruptions_remain_resource_limited() {
        let search = GeneratedSymbolicRowSpanError::Search(
            crate::InternalSymmetrySearchError::UnexpectedVerificationFailure(
                crate::SymmetryVerificationError::Family(
                    crate::GenericFamilyError::ResourceLimit {
                        resource: "family scratch",
                        requested: 2,
                        limit: 1,
                    },
                ),
            ),
        );
        assert!(row_span_error_is_resource(&search));

        let replay = GeneratedSymbolicRowSpanError::SymmetryReplay(
            crate::InternalSymmetryReplayError::AffineVerification(
                crate::SymmetryVerificationError::AllocationFailure {
                    resource: "matrix scratch",
                    requested: 4,
                },
            ),
        );
        assert!(row_span_error_is_resource(&replay));

        let transport = GeneratedSymbolicRowSpanError::Transport(
            crate::SymbolicSymmetryRowTransportError::Symmetry(
                crate::InternalSymmetryReplayError::Compatibility(
                    crate::InternalSymmetryCompatibilityError::AllocationFailure {
                        resource: "permutation",
                        requested: 8,
                    },
                ),
            ),
        );
        assert!(row_span_error_is_resource(&transport));

        let transport_overflow = GeneratedSymbolicRowSpanError::Transport(
            crate::SymbolicSymmetryRowTransportError::ResourceCountOverflow {
                resource: "transport terms",
            },
        );
        assert!(row_span_error_is_resource(&transport_overflow));
    }

    #[test]
    fn nested_exact_and_nonresource_symmetry_failures_are_distinguished() {
        let exponent_policy =
            crate::GenericFamilyError::ExactAlgebra(ExactAlgebraError::ConfiguredExponentLimit {
                requested: 2,
                representation_limit: 1,
            });
        assert!(generic_family_error_is_resource(&exponent_policy));

        let proof_failure = GeneratedSymbolicRowSpanError::Search(
            crate::InternalSymmetrySearchError::UnexpectedVerificationFailure(
                crate::SymmetryVerificationError::SingularLoopMap,
            ),
        );
        assert!(!row_span_error_is_resource(&proof_failure));

        let compatibility_failure = GeneratedSymbolicRowSpanError::SymmetryReplay(
            crate::InternalSymmetryReplayError::Compatibility(
                crate::InternalSymmetryCompatibilityError::UnsupportedJacobian,
            ),
        );
        assert!(!row_span_error_is_resource(&compatibility_failure));
    }

    #[test]
    fn cylindrical_candidate_resource_failures_are_classified_transitively() {
        let direct_limit = WhenBadCompilerError::GeneratedCylindricalCandidate(Box::new(
            GeneratedCylindricalCandidateAuthorityError::ResourceLimit {
                resource: "candidate terms",
                requested: 2,
                limit: 1,
            },
        ));
        assert!(when_bad_error_is_resource(&direct_limit));

        let direct_allocation = WhenBadCompilerError::GeneratedCylindricalCandidate(Box::new(
            GeneratedCylindricalCandidateAuthorityError::AllocationFailure {
                resource: "candidate bindings",
                requested: 4,
            },
        ));
        assert!(when_bad_error_is_resource(&direct_allocation));

        let nested_queue_limit = WhenBadCompilerError::GeneratedCylindricalCandidate(Box::new(
            GeneratedCylindricalCandidateAuthorityError::Source(
                GeneratedCylindricalPersistentEliminationError::RowSystem(
                    GeneratedCylindricalRowSystemError::Start(
                        GeneratedCylindricalResidualStartError::Queue(
                            GeneratedSectorLiveLeafQueueError::ResourceLimit {
                                resource: "queued leaves",
                                requested: 8,
                                limit: 7,
                            },
                        ),
                    ),
                ),
            ),
        ));
        assert!(when_bad_error_is_resource(&nested_queue_limit));

        let nonresource = WhenBadCompilerError::GeneratedCylindricalCandidate(Box::new(
            GeneratedCylindricalCandidateAuthorityError::ForeignFamily,
        ));
        assert!(!when_bad_error_is_resource(&nonresource));
    }

    #[test]
    fn replay_enforces_family_wide_limits_and_aggregate_stats() {
        let family = massive_tadpole();
        let context = crate::ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let certificate = GeneratedFamilyRuleSystemCompiler::compile(
            &family,
            &context,
            SectorRestrictions::unrestricted(1).unwrap(),
            PowerShiftPolicy::FormalGeneric,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            GeneratedFamilyRuleSystemConfig::default(),
            GeneratedFamilyRuleSystemLimits::default(),
        )
        .unwrap();

        let mut transcript_limit = certificate.clone();
        transcript_limit.limits.max_sector_transcripts = 1;
        assert!(matches!(
            transcript_limit.replay(&family, &context),
            Err(GeneratedFamilyRuleSystemError::ResourceLimit {
                resource: "family generated-rule sector transcripts",
                requested: 2,
                limit: 1,
            })
        ));

        let mut attempt_limit = certificate.clone();
        attempt_limit.limits.max_unresolved_sector_attempts = 0;
        assert!(matches!(
            attempt_limit.replay(&family, &context),
            Err(GeneratedFamilyRuleSystemError::ResourceLimit {
                resource: "family generated-rule unresolved sector attempts",
                requested: 1,
                limit: 0,
            })
        ));

        let mut solve_order = certificate.clone();
        solve_order.solve_order = Vec::new().into_boxed_slice();
        assert!(matches!(
            solve_order.replay(&family, &context),
            Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
                detail: "unresolved sector solve order differs from the inventory",
            })
        ));

        let mut census = certificate;
        census.stats.generated_candidate_attempts += 1;
        assert!(matches!(
            census.replay(&family, &context),
            Err(GeneratedFamilyRuleSystemError::ReplayMismatch {
                detail: "family-wide sector census differs",
            })
        ));
    }
}
