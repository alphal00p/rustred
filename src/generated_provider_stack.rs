//! Shared assembly for concrete providers backed by one replayed generated
//! row span. Certificate compilers retain their own preflight/interruption
//! policy; this module centralizes the policy-sensitive runtime stack.

use std::sync::Arc;

use crate::generated_sector_affine_effective_coverage::GeneratedSectorAffineEffectiveCoverageCertificate;
use crate::generated_sector_affine_provider::{
    GeneratedSectorAffineConditionalRuleProvider,
    GeneratedSectorAffineConditionalRuleProviderBuildStats,
    GeneratedSectorAffineConditionalRuleProviderError,
    GeneratedSectorAffineConditionalRuleProviderLimits,
};
use crate::{
    CertifiedSymmetryCanonicalizingRuleProvider, CertifiedSymmetryCanonicalizingRuleProviderError,
    CertifiedZeroSectorRuleProvider, CertifiedZeroSectorRuleProviderError, ConcreteIntegralKey,
    GeneratedFamilyRuleSystemProviderLimits, GeneratedSectorConditionalRuleProvider,
    GeneratedSectorConditionalRuleProviderBuildStats, GeneratedSectorConditionalRuleProviderError,
    GeneratedSectorLiveLeafQueueCertificate, GeneratedSymbolicRowSpanCertificate, IntegralFamily,
    IntegralOrderingPolicy, MasterPolicyError, MasterPolicyProvider, MasterPolicyTerminal,
    ParametricCoefficientContext, ParametricSectorCoverageCertificate,
    ParametricSectorRuleProvider, ParametricSectorRuleProviderError, PowerShiftPolicy,
    SectorRestrictions,
};

pub(crate) type GeneratedStackSectorProvider<'family> = ParametricSectorRuleProvider<'family>;
pub(crate) type GeneratedStackConditionalProvider<'family> =
    GeneratedSectorConditionalRuleProvider<'family, GeneratedStackSectorProvider<'family>>;
pub(crate) type GeneratedStackConditionalError =
    GeneratedSectorConditionalRuleProviderError<ParametricSectorRuleProviderError>;
pub(crate) type GeneratedStackMasterProvider<'family> =
    MasterPolicyProvider<GeneratedStackConditionalProvider<'family>>;
pub(crate) type GeneratedStackMasterError = MasterPolicyError<GeneratedStackConditionalError>;
pub(crate) type GeneratedStackSymmetryProvider<'family> =
    CertifiedSymmetryCanonicalizingRuleProvider<'family, GeneratedStackMasterProvider<'family>>;
pub(crate) type GeneratedStackSymmetryError =
    CertifiedSymmetryCanonicalizingRuleProviderError<GeneratedStackMasterError>;
pub(crate) type GeneratedProviderStack<'family> =
    CertifiedZeroSectorRuleProvider<'family, GeneratedStackSymmetryProvider<'family>>;
pub(crate) type GeneratedProviderStackError =
    CertifiedZeroSectorRuleProviderError<GeneratedStackSymmetryError>;

/// Separate V2 provider vocabulary.  The existing V1 aliases above remain
/// unchanged so no V1 certificate/provider meaning is reinterpreted.
pub(crate) type GeneratedAffineStackSectorProvider<'family> = ParametricSectorRuleProvider<'family>;
pub(crate) type GeneratedAffineStackAffineProvider<'family> =
    GeneratedSectorAffineConditionalRuleProvider<
        'family,
        GeneratedAffineStackSectorProvider<'family>,
    >;
pub(crate) type GeneratedAffineStackAffineError =
    GeneratedSectorAffineConditionalRuleProviderError<ParametricSectorRuleProviderError>;
pub(crate) type GeneratedAffineStackConditionalProvider<'family> =
    GeneratedSectorConditionalRuleProvider<'family, GeneratedAffineStackAffineProvider<'family>>;
pub(crate) type GeneratedAffineStackConditionalError =
    GeneratedSectorConditionalRuleProviderError<GeneratedAffineStackAffineError>;
pub(crate) type GeneratedAffineStackMasterProvider<'family> =
    MasterPolicyProvider<GeneratedAffineStackConditionalProvider<'family>>;
pub(crate) type GeneratedAffineStackMasterError =
    MasterPolicyError<GeneratedAffineStackConditionalError>;
pub(crate) type GeneratedAffineStackSymmetryProvider<'family> =
    CertifiedSymmetryCanonicalizingRuleProvider<
        'family,
        GeneratedAffineStackMasterProvider<'family>,
    >;
pub(crate) type GeneratedAffineStackSymmetryError =
    CertifiedSymmetryCanonicalizingRuleProviderError<GeneratedAffineStackMasterError>;
pub(crate) type GeneratedAffineProviderStack<'family> =
    CertifiedZeroSectorRuleProvider<'family, GeneratedAffineStackSymmetryProvider<'family>>;
pub(crate) type GeneratedAffineProviderStackError =
    CertifiedZeroSectorRuleProviderError<GeneratedAffineStackSymmetryError>;

pub(crate) struct GeneratedProviderStackBuild<'family> {
    pub stack: GeneratedProviderStack<'family>,
    pub conditional_stats: GeneratedSectorConditionalRuleProviderBuildStats,
    pub master_terminals: usize,
    pub master_certificate_fingerprint_bytes: usize,
}

/// Limits for the separate one-affine-epoch stack.  `base` retains every V1
/// limit unchanged; `affine` bounds only the inserted sealed-owner tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineProviderStackLimits {
    pub(crate) base: GeneratedFamilyRuleSystemProviderLimits,
    pub(crate) affine: GeneratedSectorAffineConditionalRuleProviderLimits,
}

impl Default for GeneratedAffineProviderStackLimits {
    fn default() -> Self {
        Self {
            base: GeneratedFamilyRuleSystemProviderLimits::default(),
            affine: GeneratedSectorAffineConditionalRuleProviderLimits::default(),
        }
    }
}

pub(crate) struct GeneratedAffineProviderStackBuild<'family> {
    pub(crate) stack: GeneratedAffineProviderStack<'family>,
    pub(crate) conditional_stats: GeneratedSectorConditionalRuleProviderBuildStats,
    pub(crate) affine_stats: GeneratedSectorAffineConditionalRuleProviderBuildStats,
    pub(crate) master_terminals: usize,
    pub(crate) master_certificate_fingerprint_bytes: usize,
}

#[derive(Debug)]
pub(crate) enum GeneratedProviderStackBuildError {
    InputTerminalResource { requested: usize, limit: usize },
    ResourceCountOverflow { resource: &'static str },
    Sector(ParametricSectorRuleProviderError),
    Conditional(GeneratedStackConditionalError),
    Master(GeneratedStackMasterError),
    Symmetry(GeneratedStackSymmetryError),
    Zero(GeneratedProviderStackError),
}

#[derive(Debug)]
pub(crate) enum GeneratedAffineProviderStackBuildError {
    InputTerminalResource {
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    MissingSharedRowSpanForAffineOwners,
    AffineOwnerRowSpanAllocationMismatch {
        sector: crate::SectorMask,
    },
    Sector(ParametricSectorRuleProviderError),
    Affine(GeneratedAffineStackAffineError),
    Conditional(GeneratedAffineStackConditionalError),
    Master(GeneratedAffineStackMasterError),
    Symmetry(GeneratedAffineStackSymmetryError),
    Zero(GeneratedAffineProviderStackError),
}

#[allow(clippy::too_many_arguments)]
/// Assemble already-preflighted owned coverage/queue material against the
/// row-span allocation already replayed by the caller's enclosing family
/// certificate. This function intentionally performs no family-certificate
/// replay or interruption policy of its own.
pub(crate) fn build_generated_provider_stack<'family>(
    family: &'family IntegralFamily,
    context: &'family ParametricCoefficientContext,
    restrictions: SectorRestrictions,
    power_shift_policy: PowerShiftPolicy,
    ordering: IntegralOrderingPolicy,
    row_span: Option<Arc<GeneratedSymbolicRowSpanCertificate>>,
    coverages: Vec<ParametricSectorCoverageCertificate>,
    queues: Vec<GeneratedSectorLiveLeafQueueCertificate>,
    terminals: impl IntoIterator<Item = (ConcreteIntegralKey, MasterPolicyTerminal)>,
    limits: GeneratedFamilyRuleSystemProviderLimits,
) -> Result<GeneratedProviderStackBuild<'family>, GeneratedProviderStackBuildError> {
    let terminals = terminals.into_iter();
    let terminal_lower_bound = terminals.size_hint().0;
    if terminal_lower_bound > limits.max_input_terminals {
        return Err(GeneratedProviderStackBuildError::InputTerminalResource {
            requested: terminal_lower_bound,
            limit: limits.max_input_terminals,
        });
    }

    let conditional = if let Some(row_span) = &row_span {
        let sector = ParametricSectorRuleProvider::try_new_with_replayed_certificates(
            family,
            context,
            coverages,
            row_span,
            limits.sector_rules,
        )
        .map_err(GeneratedProviderStackBuildError::Sector)?;
        GeneratedSectorConditionalRuleProvider::try_new_with_replayed_queues(
            family,
            context,
            queues,
            sector,
            row_span,
            limits.conditional_rules,
        )
        .map_err(GeneratedProviderStackBuildError::Conditional)?
    } else {
        let sector =
            ParametricSectorRuleProvider::try_new(family, context, coverages, limits.sector_rules)
                .map_err(GeneratedProviderStackBuildError::Sector)?;
        GeneratedSectorConditionalRuleProvider::try_new(
            family,
            context,
            queues,
            sector,
            limits.conditional_rules,
        )
        .map_err(GeneratedProviderStackBuildError::Conditional)?
    };
    let conditional_stats = conditional.build_stats();
    let master = MasterPolicyProvider::try_new(conditional, [], limits.master_policy)
        .map_err(GeneratedProviderStackBuildError::Master)?;
    let mut symmetry = if let Some(row_span) = &row_span {
        CertifiedSymmetryCanonicalizingRuleProvider::try_new_with_replayed_row_span(
            family,
            context,
            restrictions.clone(),
            row_span.clone(),
            ordering,
            master,
            row_span,
            limits.symmetry,
        )
        .map_err(GeneratedProviderStackBuildError::Symmetry)?
    } else {
        CertifiedSymmetryCanonicalizingRuleProvider::try_new_without_symmetries(
            family,
            context,
            restrictions.clone(),
            ordering,
            master,
            limits.symmetry,
        )
        .map_err(GeneratedProviderStackBuildError::Symmetry)?
    };
    let mut input_terminals = 0usize;
    for (integral, terminal) in terminals {
        input_terminals = input_terminals.checked_add(1).ok_or(
            GeneratedProviderStackBuildError::ResourceCountOverflow {
                resource: "input terminal declarations",
            },
        )?;
        if input_terminals > limits.max_input_terminals {
            return Err(GeneratedProviderStackBuildError::InputTerminalResource {
                requested: input_terminals,
                limit: limits.max_input_terminals,
            });
        }
        let canonical = symmetry
            .canonical_key(&integral)
            .map_err(GeneratedProviderStackBuildError::Symmetry)?;
        symmetry
            .inner_mut()
            .insert_terminal(canonical, terminal)
            .map_err(GeneratedProviderStackBuildError::Master)?;
    }
    let master_terminals = symmetry.inner().terminals().len();
    let master_certificate_fingerprint_bytes =
        symmetry.inner().total_certificate_fingerprint_bytes();
    let stack = CertifiedZeroSectorRuleProvider::try_new(
        family,
        restrictions,
        power_shift_policy,
        symmetry,
        limits.certified_rewrite,
    )
    .map_err(GeneratedProviderStackBuildError::Zero)?;
    Ok(GeneratedProviderStackBuild {
        stack,
        conditional_stats,
        master_terminals,
        master_certificate_fingerprint_bytes,
    })
}

/// Assemble one replayable affine-owner epoch at the only sound location in
/// the existing runtime order:
///
/// `zero(symmetry(master(conditional_v1(affine(global_v1)))))`.
///
/// This is intentionally a separate builder.  It neither changes the V1
/// stack nor claims recursive affine closure. Every supplied owner is bound
/// to the exact installed V1 coverage/queue material and independently
/// replayed before publication. At present the affine tier accepts at most
/// one owner per sector. Caller-provided coverage and queue material is only
/// for sectors without an affine owner: exact owner-sector material is
/// derived from each sealed owner after the combined borrowed preflights.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_generated_affine_provider_stack<'family>(
    family: &'family IntegralFamily,
    context: &'family ParametricCoefficientContext,
    restrictions: SectorRestrictions,
    power_shift_policy: PowerShiftPolicy,
    ordering: IntegralOrderingPolicy,
    row_span: Option<Arc<GeneratedSymbolicRowSpanCertificate>>,
    mut coverages: Vec<ParametricSectorCoverageCertificate>,
    mut queues: Vec<GeneratedSectorLiveLeafQueueCertificate>,
    owners: &[Arc<GeneratedSectorAffineEffectiveCoverageCertificate>],
    terminals: impl IntoIterator<Item = (ConcreteIntegralKey, MasterPolicyTerminal)>,
    limits: GeneratedAffineProviderStackLimits,
) -> Result<GeneratedAffineProviderStackBuild<'family>, GeneratedAffineProviderStackBuildError> {
    let terminals = terminals.into_iter();
    let terminal_lower_bound = terminals.size_hint().0;
    if terminal_lower_bound > limits.base.max_input_terminals {
        return Err(
            GeneratedAffineProviderStackBuildError::InputTerminalResource {
                requested: terminal_lower_bound,
                limit: limits.base.max_input_terminals,
            },
        );
    }

    // The complete owner set is preflighted from borrowed capabilities before
    // inspecting bindings or cloning any owner Arc/payload into the stack.
    GeneratedSectorAffineConditionalRuleProvider::<
        GeneratedAffineStackSectorProvider<'family>,
    >::preflight_owners(family, context, owners, limits.affine)
    .map_err(GeneratedAffineProviderStackBuildError::Affine)?;

    if !owners.is_empty() {
        let shared = row_span
            .as_ref()
            .ok_or(GeneratedAffineProviderStackBuildError::MissingSharedRowSpanForAffineOwners)?;
        for owner in owners {
            if !Arc::ptr_eq(owner.source_queue().discovery().row_span_arc(), shared) {
                return Err(
                    GeneratedAffineProviderStackBuildError::AffineOwnerRowSpanAllocationMismatch {
                        sector: owner.source_queue().sector().clone(),
                    },
                );
            }
        }
    }

    // Combined borrowed preflights both enforce the V1 aggregate bounds and
    // reject a caller-supplied duplicate for an affine-owned sector. Only
    // after they succeed are authoritative owner payloads cloned.
    ParametricSectorRuleProvider::preflight_certificates(
        family,
        context,
        coverages.iter().chain(
            owners
                .iter()
                .map(|owner| owner.source_queue().discovery().coverage()),
        ),
        limits.base.sector_rules,
    )
    .map_err(GeneratedAffineProviderStackBuildError::Sector)?;
    GeneratedSectorConditionalRuleProvider::<GeneratedAffineStackAffineProvider<'family>>::preflight_queues(
        family,
        context,
        queues
            .iter()
            .chain(owners.iter().map(|owner| owner.source_queue().as_ref())),
        limits.base.conditional_rules,
    )
    .map_err(GeneratedAffineProviderStackBuildError::Conditional)?;

    coverages.try_reserve_exact(owners.len()).map_err(|_| {
        GeneratedAffineProviderStackBuildError::AllocationFailure {
            resource: "authoritative affine-owner coverages",
            requested: owners.len(),
        }
    })?;
    queues.try_reserve_exact(owners.len()).map_err(|_| {
        GeneratedAffineProviderStackBuildError::AllocationFailure {
            resource: "authoritative affine-owner queues",
            requested: owners.len(),
        }
    })?;
    for owner in owners {
        coverages.push(owner.source_queue().discovery().coverage().clone());
        queues.push(owner.source_queue().as_ref().clone());
    }

    let sector = if let Some(row_span) = &row_span {
        ParametricSectorRuleProvider::try_new_with_replayed_certificates(
            family,
            context,
            coverages,
            row_span,
            limits.base.sector_rules,
        )
        .map_err(GeneratedAffineProviderStackBuildError::Sector)?
    } else {
        ParametricSectorRuleProvider::try_new(family, context, coverages, limits.base.sector_rules)
            .map_err(GeneratedAffineProviderStackBuildError::Sector)?
    };
    let affine = GeneratedSectorAffineConditionalRuleProvider::try_new(
        family,
        context,
        owners,
        sector,
        limits.affine,
    )
    .map_err(GeneratedAffineProviderStackBuildError::Affine)?;
    let affine_stats = affine.build_stats();
    let conditional = if let Some(row_span) = &row_span {
        GeneratedSectorConditionalRuleProvider::try_new_with_replayed_queues(
            family,
            context,
            queues,
            affine,
            row_span,
            limits.base.conditional_rules,
        )
        .map_err(GeneratedAffineProviderStackBuildError::Conditional)?
    } else {
        GeneratedSectorConditionalRuleProvider::try_new(
            family,
            context,
            queues,
            affine,
            limits.base.conditional_rules,
        )
        .map_err(GeneratedAffineProviderStackBuildError::Conditional)?
    };
    let conditional_stats = conditional.build_stats();
    let master = MasterPolicyProvider::try_new(conditional, [], limits.base.master_policy)
        .map_err(GeneratedAffineProviderStackBuildError::Master)?;
    let mut symmetry = if let Some(row_span) = &row_span {
        CertifiedSymmetryCanonicalizingRuleProvider::try_new_with_replayed_row_span(
            family,
            context,
            restrictions.clone(),
            row_span.clone(),
            ordering,
            master,
            row_span,
            limits.base.symmetry,
        )
        .map_err(GeneratedAffineProviderStackBuildError::Symmetry)?
    } else {
        CertifiedSymmetryCanonicalizingRuleProvider::try_new_without_symmetries(
            family,
            context,
            restrictions.clone(),
            ordering,
            master,
            limits.base.symmetry,
        )
        .map_err(GeneratedAffineProviderStackBuildError::Symmetry)?
    };
    let mut input_terminals = 0usize;
    for (integral, terminal) in terminals {
        input_terminals = input_terminals.checked_add(1).ok_or(
            GeneratedAffineProviderStackBuildError::ResourceCountOverflow {
                resource: "input terminal declarations",
            },
        )?;
        if input_terminals > limits.base.max_input_terminals {
            return Err(
                GeneratedAffineProviderStackBuildError::InputTerminalResource {
                    requested: input_terminals,
                    limit: limits.base.max_input_terminals,
                },
            );
        }
        let canonical = symmetry
            .canonical_key(&integral)
            .map_err(GeneratedAffineProviderStackBuildError::Symmetry)?;
        symmetry
            .inner_mut()
            .insert_terminal(canonical, terminal)
            .map_err(GeneratedAffineProviderStackBuildError::Master)?;
    }
    let master_terminals = symmetry.inner().terminals().len();
    let master_certificate_fingerprint_bytes =
        symmetry.inner().total_certificate_fingerprint_bytes();
    let stack = CertifiedZeroSectorRuleProvider::try_new(
        family,
        restrictions,
        power_shift_policy,
        symmetry,
        limits.base.certified_rewrite,
    )
    .map_err(GeneratedAffineProviderStackBuildError::Zero)?;
    Ok(GeneratedAffineProviderStackBuild {
        stack,
        conditional_stats,
        affine_stats,
        master_terminals,
        master_certificate_fingerprint_bytes,
    })
}

pub(crate) fn replay_generated_provider_symmetry(
    symmetry: &GeneratedStackSymmetryProvider<'_>,
    row_span: Option<&Arc<GeneratedSymbolicRowSpanCertificate>>,
) -> Result<(), GeneratedStackSymmetryError> {
    if let Some(row_span) = row_span {
        symmetry.replay_with_replayed_row_span(row_span)
    } else {
        symmetry.replay()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_sector_affine_effective_coverage::{
        GeneratedSectorAffineEffectiveCoverageCompiler,
        GeneratedSectorAffineEffectiveCoverageConfig, GeneratedSectorAffineEffectiveCoverageLimits,
    };
    use crate::{
        AffineDenominator, CoefficientContext, ConcreteRuleDecision, ConcreteRuleProvider,
        ConcreteTerminalStatus, GeneratedResidualAffineCaseInventoryCompiler,
        GeneratedResidualAffineCaseInventoryLimits, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueLimits, GuardOrigin, ParametricIbpGenerator, SectorMask,
    };

    fn family() -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_m2 = coefficients.parse("-m2").unwrap();
        IntegralFamily::new(
            "opaque-affine-stack-fixture",
            vec!["q0".into(), "q1".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![one.clone(), zero.clone(), zero.clone()],
                ),
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![zero.clone(), zero.clone(), one.clone()],
                ),
                AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
            ],
            Vec::new(),
            vec![zero.clone(), zero.clone(), zero],
        )
        .unwrap()
    }

    fn owner_fixture() -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedSectorAffineEffectiveCoverageCertificate>,
    ) {
        let family = family();
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string("011").unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
        queue_limits.translation_radius = 0;
        queue_limits.max_translation_points = 1;
        let queue = Arc::new(
            GeneratedSectorLiveLeafQueueCompiler::compile(
                &family,
                &context,
                &discovery,
                queue_limits,
            )
            .unwrap(),
        );
        let inventory = Arc::new(
            GeneratedResidualAffineCaseInventoryCompiler::compile(
                &family,
                &context,
                queue,
                GeneratedResidualAffineCaseInventoryLimits::default(),
            )
            .unwrap(),
        );
        let owner = Arc::new(
            GeneratedSectorAffineEffectiveCoverageCompiler::compile(
                &family,
                &context,
                inventory,
                GeneratedSectorAffineEffectiveCoverageConfig::new(0),
                GeneratedSectorAffineEffectiveCoverageLimits::default(),
            )
            .unwrap(),
        );
        owner.replay(&family, &context).unwrap();
        (family, context, owner)
    }

    fn material(
        owner: &GeneratedSectorAffineEffectiveCoverageCertificate,
    ) -> (
        Arc<GeneratedSymbolicRowSpanCertificate>,
        Vec<ParametricSectorCoverageCertificate>,
        Vec<GeneratedSectorLiveLeafQueueCertificate>,
    ) {
        let queue = owner.source_queue();
        (
            Arc::clone(queue.discovery().row_span_arc()),
            vec![queue.discovery().coverage().clone()],
            vec![queue.as_ref().clone()],
        )
    }

    #[test]
    fn separate_affine_stack_routes_sealed_owner_and_preserves_master_precedence() {
        let (family, context, owner) = owner_fixture();
        let restrictions = SectorRestrictions::unrestricted(family.denominator_count()).unwrap();
        let limits = GeneratedAffineProviderStackLimits::default();
        let source = ConcreteIntegralKey::try_new([0, 1, 2]).unwrap();

        assert!(matches!(
            build_generated_affine_provider_stack(
                &family,
                &context,
                restrictions.clone(),
                PowerShiftPolicy::FormalGeneric,
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                None,
                Vec::new(),
                Vec::new(),
                std::slice::from_ref(&owner),
                [],
                limits,
            ),
            Err(GeneratedAffineProviderStackBuildError::MissingSharedRowSpanForAffineOwners)
        ));
        let shared = owner.source_queue().discovery().row_span_arc();
        let detached_equal_row_span = Arc::new(shared.as_ref().clone());
        assert!(matches!(
            build_generated_affine_provider_stack(
                &family,
                &context,
                restrictions.clone(),
                PowerShiftPolicy::FormalGeneric,
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                Some(detached_equal_row_span),
                Vec::new(),
                Vec::new(),
                std::slice::from_ref(&owner),
                [],
                limits,
            ),
            Err(
                GeneratedAffineProviderStackBuildError::AffineOwnerRowSpanAllocationMismatch { .. }
            )
        ));
        let (row_span, coverages, _) = material(owner.as_ref());
        assert!(matches!(
            build_generated_affine_provider_stack(
                &family,
                &context,
                restrictions.clone(),
                PowerShiftPolicy::FormalGeneric,
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                Some(row_span),
                coverages,
                Vec::new(),
                std::slice::from_ref(&owner),
                [],
                limits,
            ),
            Err(GeneratedAffineProviderStackBuildError::Sector(
                ParametricSectorRuleProviderError::DuplicateSector { .. }
            ))
        ));
        let (row_span, _, queues) = material(owner.as_ref());
        assert!(matches!(
            build_generated_affine_provider_stack(
                &family,
                &context,
                restrictions.clone(),
                PowerShiftPolicy::FormalGeneric,
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                Some(row_span),
                Vec::new(),
                queues,
                std::slice::from_ref(&owner),
                [],
                limits,
            ),
            Err(GeneratedAffineProviderStackBuildError::Conditional(
                GeneratedSectorConditionalRuleProviderError::DuplicateSector { .. }
            ))
        ));

        let row_span = Arc::clone(owner.source_queue().discovery().row_span_arc());
        let mut built = build_generated_affine_provider_stack(
            &family,
            &context,
            restrictions.clone(),
            PowerShiftPolicy::FormalGeneric,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            Some(row_span),
            Vec::new(),
            Vec::new(),
            std::slice::from_ref(&owner),
            [],
            limits,
        )
        .unwrap();
        assert_eq!(built.affine_stats.installed_owners(), 1);
        assert_eq!(built.affine_stats.installed_sectors(), 1);
        let canonical = built.stack.inner().canonical_key(&source).unwrap();
        let decision = built.stack.decision_for(&source).unwrap();
        let ConcreteRuleDecision::ConditionalReduction(reduction) = decision else {
            panic!(
                "the separate V2 stack did not route the sealed affine owner; source={source:?}, canonical={canonical:?}, decision={decision:?}"
            )
        };
        assert_eq!(reduction.source(), &source);
        assert!(reduction.coordinate_rule().is_none());
        assert_eq!(reduction.pivot_ordinal(), 2);
        assert_eq!(reduction.rhs().len(), 1);
        let master = ConcreteIntegralKey::try_new([0, 1, 1]).unwrap();
        let expected = family.coefficient_context().parse("(d-2)/(2*m2)").unwrap();
        assert_eq!(reduction.rhs().get(&master), Some(&expected));
        assert_eq!(reduction.required_nonzero().len(), 2);
        for condition in reduction.required_nonzero() {
            assert_eq!(condition.origins().len(), 1);
            assert!(
                condition
                    .origins()
                    .contains(&GuardOrigin::GeneratedAffineSealedCondition)
            );
        }
        let guarded_polynomials = reduction
            .required_nonzero()
            .iter()
            .map(|condition| condition.polynomial().raw().clone().into())
            .collect::<Vec<_>>();
        for expected in ["-2*m2", "2*m2"] {
            let expected = family.coefficient_context().parse(expected).unwrap();
            assert!(guarded_polynomials.iter().any(|actual| {
                family
                    .coefficient_context()
                    .try_sub(actual, &expected, Default::default())
                    .is_ok_and(|delta| delta.is_zero())
            }));
        }
        let affine = built.stack.inner().inner().inner().inner();
        assert!(Arc::ptr_eq(affine.owners().next().unwrap(), &owner));
        assert_eq!(affine.stats().applications(), 1);
        assert_eq!(affine.stats().delegations(), 0);
        affine.replay().unwrap();

        let row_span = Arc::clone(owner.source_queue().discovery().row_span_arc());
        let mut selected = build_generated_affine_provider_stack(
            &family,
            &context,
            restrictions,
            PowerShiftPolicy::FormalGeneric,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            Some(row_span),
            Vec::new(),
            Vec::new(),
            std::slice::from_ref(&owner),
            [(source.clone(), MasterPolicyTerminal::Selected)],
            limits,
        )
        .unwrap();
        assert!(matches!(
            selected.stack.decision_for(&source).unwrap(),
            ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::SelectedMaster)
        ));
        let affine = selected.stack.inner().inner().inner().inner();
        assert_eq!(affine.stats().queries(), 0);
        assert_eq!(affine.stats().applications(), 0);
    }
}
