mod application;
pub use application::{MasterNormalizationProfile, MasterReductionOperation, MasterReductionOptions, MasterReductionArtifact, master_reduce_saved_campaign, master_refine_published_artifact, master_reduction_inspect, load_master_reduction, load_master_relation_session};
pub use application::{
    CandidateArtifact, CandidateArtifactPage, CandidateGenerationEvents, CandidateGenerationJob,
    CandidateGenerationSession, CandidateGenerationSnapshot, CandidateGenerationState,
    CandidateTerminalNormalization, CandidateTerminalNormalizationLimits,
};
pub use application::{EntryPowerBudget, FiniteEntryDomain, entry_domain_plan};
pub use application::{
    OwnerDomainWalkInventory, OwnerDomainWalkInventoryOptions, owner_domain_walk_inventory,
};
/// Native family entry for embedded hosts; no source-text round trip.
pub use rustred::family::IntegralFamily as NativeIntegralFamily;
#[cfg(feature = "cli")]
mod cli;
#[cfg(test)]
mod test_gates;

pub use application::{
    OWNER_DOMAIN_WALK_AMENDMENT_MAX_BYTES, OWNER_DOMAIN_WALK_AMENDMENT_SCHEMA,
    OWNER_DOMAIN_WALK_CHECKPOINT_FORMAT, OWNER_DOMAIN_WALK_CHECKPOINT_MANIFEST_MAX_BYTES,
    OWNER_DOMAIN_WALK_CHECKPOINT_SCHEMA, OWNER_DOMAIN_WALK_EPOCH_SEMANTICS_VERSION,
    OWNER_DOMAIN_WALK_FINITE_REPLAY_VERSION, OWNER_DOMAIN_WALK_RESCUE_PLAN_SCHEMA,
    OWNER_DOMAIN_WALK_SCOPE_EXTENSION_SCHEMA, OWNER_DOMAIN_WALK_SEMANTICS_VERSION,
    OWNER_DOMAIN_WALK_VERIFY_SCHEMA, OwnerDomainMatchRequest, OwnerDomainMatchResult,
    OwnerDomainScanRequest, OwnerDomainScanResult, OwnerDomainWalkAmendment,
    OwnerDomainWalkApplySubdivision, OwnerDomainWalkCheckpointOptions,
    OwnerDomainWalkEpochDispatchPolicy, OwnerDomainWalkEpochInspectorLookup,
    OwnerDomainWalkEpochPublicationOrder, OwnerDomainWalkFiniteReplayLimits,
    OwnerDomainWalkFrontierPolicy, OwnerDomainWalkG2ResidualAnchors,
    OwnerDomainWalkPublicationPolicy, OwnerDomainWalkRecords, OwnerDomainWalkRequest,
    OwnerDomainWalkRescuePlan, OwnerDomainWalkRescuePlanOptions, OwnerDomainWalkRescueScope,
    OwnerDomainWalkResult, OwnerDomainWalkSchedulingPolicy, OwnerDomainWalkVerifyMutation,
    OwnerDomainWalkVerifyOptions, OwnerDomainWalkVerifyReferenceLevers,
    OwnerDomainWalkVerifyReinspect, OwnerDomainWalkVerifyScope, OwnerGuardedApplyRequest,
    OwnerGuardedApplyResult, RoutedCampaignRequest, RoutedCampaignResult, RoutedEntryWitnessLimits,
    RoutedEntryWitnessProposal, RoutedEntryWitnessRoundResult, RoutedEntryWitnessStats,
    RoutedFeedbackFixedResidualPolicy, RoutedFeedbackNomination, RoutedFeedbackOptions,
    RoutedFeedbackRoundResult, RoutedFeedbackSession, owner_domain_match_with_progress,
    owner_domain_scan_with_progress, owner_domain_walk_rescue_plan,
    owner_domain_walk_verify_closure, owner_domain_walk_with_progress,
    owner_guarded_apply_with_progress, routed_campaign_with_progress,
};
pub use rustred::persistence::{BinaryIoLimits, equivalent_generated_programs};

pub use application::{
    AppError, AppErrorKind, ArtifactLoadLimits, CANDIDATE_BUNDLE_SCHEMA,
    CANDIDATE_CERTIFICATION_SCHEMA, CampaignPlanRequest, CampaignPlanResult,
    CampaignPreflightRequest, CampaignPreflightResult, CandidateBundleInspection,
    CandidateBundleLimits, CandidateBundleResult, CandidateCaseInspection,
    CandidateCertificationRequest, CandidateCertificationResult, CandidateCheckpointOptions,
    CandidateCoefficientInspection, CandidateCoordinateGroups, CandidateDegreeRow,
    CandidateDiscoveryStrategy, CandidateDomainOverlayLoadLimits, CandidateExactBackend,
    CandidateFixedAxisInspection, CandidateIntegralInspection, CandidateIntegralOrder,
    CandidateOrderDirection, CandidateOwnerBundle, CandidateOwnerLoadLimits,
    CandidateProgramInspection, CandidateProgramInspectionOptions, CandidateRowFeature,
    CandidateRowPriority, CandidateRuleInspection, CandidateRulePortfolio,
    CandidateRulePortfolioTrigger, CandidateRuleQualityFeature, CandidateRuleQualityPriority,
    CandidateRuleQualityThreshold, CandidateRuleTrialLimits, CandidateSectorInspection,
    CandidateSectorPriority, CandidateSourcePriority, CandidateSourceSupportInspection,
    CandidateSourceSupportOptions, CandidateSourceVisitPlan, CandidateTermInspection,
    CaseIntersectionLimits, CheckedPriorityOwnerExport, ClosingArtifactGenerateRequest,
    ClosingArtifactGenerateResult, ClosingArtifactInspectRequest, ClosingArtifactInspectResult,
    ClosingArtifactReduceRequest, ClosingArtifactReduceResult, ClosingFamilySelector,
    DeriveRequest, DeriveResult, ExactMasterCoefficient, FAMILY_CANDIDATES_SCHEMA,
    FAMILY_CLOSE_SCHEMA, FOUNDRY_CAMPAIGN_MEASUREMENTS_SCHEMA,
    FOUNDRY_WAVE_CAMPAIGN_MEASUREMENTS_SCHEMA, FOUNDRY_WAVE_CAMPAIGN_REPORT_SCHEMA,
    FamilyCandidatesRequest, FamilyCloseGenerationStage, FamilyCloseProgress, FamilyCloseRequest,
    FamilyCloseResult, FamilySolveRequest, FamilySolveResult, FiniteCaseLimits, FiniteCasePolicy,
    FoundryCampaignCensus, FoundryCampaignCoverageObstruction, FoundryCampaignCoverageStatus,
    FoundryCampaignNeedsRefinementReason, FoundryCampaignOperationalLimit, FoundryCampaignProgress,
    FoundryCampaignRunRequest, FoundryCampaignRunResult, FoundryCampaignSnapshot,
    FoundryCampaignStop, FoundryCampaignTaskLocation, FoundryCampaignTaskLocationKind,
    FoundryWaveCampaignRunRequest, FoundryWaveCampaignRunResult, InputFormat,
    K6OrbitCampaignProgress, K6OrbitCampaignState, K6WaveCampaignProgress, K6WaveCampaignState,
    MAX_CANDIDATE_BUNDLE_BYTES, MAX_CLOSING_ARTIFACT_BYTES, MAX_CLOSING_RULE_APPLICATIONS,
    MAX_FOUNDRY_CAMPAIGN_PROBES, MAX_INPUT_BYTES, MAX_OUTPUT_BYTES,
    ParseClosingFamilySelectorError, ParseInputFormatError, ParseRelationSelectionError,
    RelationSelection, SourcePortLimits, campaign_plan, campaign_preflight, certify_candidates,
    certify_candidates_with_progress, closing_artifact_generate, closing_artifact_inspect,
    closing_artifact_reduce, derive, encode_checked_priority_owner,
    encode_checked_priority_owner_with_policy, encode_generated_candidate_sector,
    encode_generated_domain_overlay, family_candidates, family_candidates_with_progress,
    family_close, family_close_with_progress, family_solve, foundry_campaign_run,
    foundry_campaign_run_with_progress, foundry_wave_campaign_run,
    foundry_wave_campaign_run_with_progress, inspect_generated_candidate_bundle,
    inspect_generated_candidate_program, inspect_generated_candidate_source_support,
    load_generated_candidate_bundle, load_generated_candidate_checkpoint,
    load_generated_candidate_owners, load_generated_candidate_owners_with_preferences,
    load_generated_domain_overlay, validate_domain_overlay_ingress,
    with_replayed_candidate_rule_circuits,
};

/// Run the command-line adapter and return its stable process exit code.
#[cfg(feature = "cli")]
pub fn cli_main_entry() -> i32 {
    cli::main_entry()
}

/// Shared admission for the dynamic campaign entry points. Their persisted
/// representation retains its existing 16-coordinate limit independently of
/// the finite reducer's larger explicit arities.
pub(crate) fn ensure_runtime_arity(arity: usize) -> Result<(), AppError> {
    if arity > 16 || !rustred::compiled_runtime_arities().contains(&arity) {
        return Err(AppError::input(format!(
            "campaign arity {arity} is not compiled; registered arities at most 16 are {:?}",
            rustred::compiled_runtime_arities()
                .iter()
                .filter(|&&n| n <= 16)
                .collect::<Vec<_>>()
        )));
    }
    Ok(())
}
