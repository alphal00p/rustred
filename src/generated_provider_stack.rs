//! Shared assembly for concrete providers backed by one replayed generated
//! row span. Certificate compilers retain their own preflight/interruption
//! policy; this module centralizes the policy-sensitive runtime stack.

use std::sync::Arc;

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

pub(crate) struct GeneratedProviderStackBuild<'family> {
    pub stack: GeneratedProviderStack<'family>,
    pub conditional_stats: GeneratedSectorConditionalRuleProviderBuildStats,
    pub master_terminals: usize,
    pub master_certificate_fingerprint_bytes: usize,
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
