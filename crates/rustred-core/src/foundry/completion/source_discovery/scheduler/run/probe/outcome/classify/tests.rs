use super::super::super::super::super::{
    ProbeLocalBudgetCause, ProbeLocalBudgetScope, ProbeLocalOutcome, ProbeLocalRejection,
    ProbeLocalStage, ProbeLocalStopContext,
};
use super::super::{campaign_stop_or_rejection, sampled_dual_stop_or_rejection};
use super::*;

#[test]
fn deeply_nested_translation_resource_limit_is_resumable() {
    const RESOURCE: &str = "nested translated coefficient terms";
    let error = CampaignError::TranslatedSources(TranslatedSourceError::RequestTranslation {
        canonical_request_ordinal: 2,
        source_ordinal: 3,
        error: ParametricRelationError::IdentityCondition(IdentityConditionError::Coefficient(
            IndexedAlgebraError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
                resource: RESOURCE,
                requested: 13,
                limit: 8,
            }),
        )),
    });

    assert_eq!(
        campaign_budget_cause(&error),
        Some(ProbeLocalBudgetCause::SourceDiscovery {
            resource: RESOURCE,
            requested: 13,
            limit: 8,
        })
    );
    let outcome = campaign_stop_or_rejection(
        4,
        5,
        ProbeLocalStage::EpochBuild,
        ProbeLocalStopContext::BeforeBootstrap,
        error,
    );
    let ProbeLocalOutcome::BudgetStop { stop, .. } = outcome else {
        panic!("nested translated resource limit must be a resumable stop");
    };
    assert_eq!(stop.probe_ordinal(), 4);
    assert_eq!(stop.epoch_ordinal(), 5);
    assert_eq!(stop.cause().scope(), ProbeLocalBudgetScope::Probe);
    assert_eq!(stop.cause().resource(), RESOURCE);
}

#[test]
fn nested_modular_allocation_failure_is_resumable() {
    const RESOURCE: &str = "nested modular sparse entries";
    let error = CampaignError::Modular(ModularKernelError::AllocationFailure {
        resource: RESOURCE,
        requested: 21,
    });

    assert_eq!(
        campaign_budget_cause(&error),
        Some(ProbeLocalBudgetCause::AllocationFailure {
            scope: ProbeLocalBudgetScope::Probe,
            resource: RESOURCE,
            requested: 21,
        })
    );
}

#[test]
fn sampled_dual_partition_stratum_resource_is_resumable() {
    const RESOURCE: &str = "nested stratum owner cells";
    let error = SampledDeclaredModuleDualError::PartitionVerification(
        StratumRegistryError::Sector(sector::Error::AllocationFailure {
            resource: RESOURCE,
            requested: 34,
        }),
    );

    assert_eq!(
        sampled_dual_budget_cause(&error),
        Some(ProbeLocalBudgetCause::AllocationFailure {
            scope: ProbeLocalBudgetScope::Probe,
            resource: RESOURCE,
            requested: 34,
        })
    );
    assert!(matches!(
        sampled_dual_stop_or_rejection(0, 1, ProbeLocalStopContext::BeforeBootstrap, error,),
        ProbeLocalOutcome::BudgetStop { .. }
    ));
}

#[test]
fn growing_epoch_nested_stratum_resource_is_resumable() {
    const RESOURCE: &str = "growing maximal-stratum bounds";
    let error = CampaignError::Stratum(StratumRegistryError::Sector(
        sector::Error::AllocationFailure {
            resource: RESOURCE,
            requested: 89,
        },
    ));

    assert_eq!(
        campaign_budget_cause(&error),
        Some(ProbeLocalBudgetCause::AllocationFailure {
            scope: ProbeLocalBudgetScope::Probe,
            resource: RESOURCE,
            requested: 89,
        })
    );
    assert!(matches!(
        campaign_stop_or_rejection(
            1,
            2,
            ProbeLocalStage::EpochBuild,
            ProbeLocalStopContext::BeforeBootstrap,
            error,
        ),
        ProbeLocalOutcome::BudgetStop { .. }
    ));
}

#[test]
fn nested_source_candidate_allocation_is_resumable() {
    let error = SourceDiscoveryError::CandidateEvaluation {
        candidate_ordinal: 1,
        source_ordinal: 2,
        error: ModularSourceEvaluationError::AllocationFailure { requested: 55 },
    };

    assert!(matches!(
        source_budget_cause(&error),
        Some(ProbeLocalBudgetCause::AllocationFailure {
            scope: ProbeLocalBudgetScope::Probe,
            requested: 55,
            ..
        })
    ));
}

#[test]
fn nested_non_resource_failure_remains_rejected() {
    let error = CampaignError::Modular(ModularKernelError::CoefficientDenominatorZero {
        row: 1,
        physical_column: 2,
    });
    assert_eq!(campaign_budget_cause(&error), None);

    let outcome = campaign_stop_or_rejection(
        0,
        0,
        ProbeLocalStage::ModularQuery,
        ProbeLocalStopContext::BeforeBootstrap,
        error,
    );
    assert!(matches!(
        outcome,
        ProbeLocalOutcome::Rejected {
            error: ProbeLocalRejection::Campaign(CampaignError::Modular(
                ModularKernelError::CoefficientDenominatorZero { .. }
            )),
            ..
        }
    ));
}
