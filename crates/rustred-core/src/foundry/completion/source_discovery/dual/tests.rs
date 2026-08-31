use crate::foundry::artifact::{ClosedArtifact, derive_one_loop_unit_mass_tadpole};
use crate::foundry::completion::frame::modular::{
    ModularSourceEvaluationError, ModularTargetQuery,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, GuardBranch, GuardBranchIdentity, ImmutableOwnerSnapshot,
    StratumRegistryLimits,
};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator, TranslatedSourceLimits,
    TranslatedSourceRequest,
};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::super::nominate::empty_obstruction_nominations_for_test;
use super::super::residual::pair_selected_sources_for_test;
use super::super::{
    AccumulatedSourceRequests, CampaignLimits, CampaignModularProbe, FreshTaskEpoch,
    OrdinarySourceIncidenceIndex, SourceDiscoveryError, SourceDiscoveryLimits,
};
use super::{SampledDeclaredModuleDual, SampledDeclaredModuleDualError};

const PRIME: u64 = 1_000_000_007;

fn request(source: usize, offset: i64) -> TranslatedSourceRequest {
    TranslatedSourceRequest::new(source, IntegralShift::try_new([offset]).unwrap())
}

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn fixed_tadpole_inputs(
    family: &ClosedArtifact,
    pivot: i64,
    physical_shifts: &[Vec<i64>],
) -> (DecoratedStratum, ImmutableOwnerSnapshot) {
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        &[pivot],
        physical_shifts,
    )
    .unwrap();
    let limits = StratumRegistryLimits::default();
    let stratum = DecoratedStratum::try_guard_blind(
        family.family_fingerprint(),
        family.context_fingerprint(),
        domain,
        limits,
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        family.family_fingerprint(),
        family.context_fingerprint(),
        1,
        limits,
    )
    .unwrap();
    (stratum, owners)
}

#[test]
fn valid_empty_complete_census_mints_owned_sampled_declared_module_dual() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let campaign_limits = CampaignLimits::default();
    let source_limits = SourceDiscoveryLimits::default();
    let zero_sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&zero_sources, source_limits).unwrap();
    let requests = AccumulatedSourceRequests::try_new(1, [request(0, 0)], campaign_limits).unwrap();
    let (stratum, owners) = fixed_tadpole_inputs(&artifact, 0, &[vec![0], vec![1]]);
    let epoch = FreshTaskEpoch::try_new(
        0,
        &generator,
        &completed,
        requests,
        IntegralShift::try_new([0]).unwrap(),
        stratum,
        owners,
        OrderingPolicy::default(),
        campaign_limits,
    )
    .unwrap();

    // At n=1 and d=4, the two unseen boundary translations are both
    // structurally incident but have zero complete residual against the
    // checked one-row obstruction.
    let probe = CampaignModularProbe::try_new(PRIME, [4], [0], campaign_limits).unwrap();
    let query = epoch
        .try_query(generator.context(), &probe, campaign_limits)
        .unwrap();
    let obstruction = query
        .obstruction()
        .expect("the one-row tadpole query must remain a modular no-hit");
    let nominations = incidence
        .try_nominate_obstruction(obstruction, source_limits)
        .unwrap();
    assert_eq!(nominations.requests(), &[request(0, -1), request(0, 1)]);
    let residuals = incidence
        .try_retain_nonzero_residuals(
            &generator,
            &completed,
            &nominations,
            query.sampled(),
            obstruction,
            source_limits,
        )
        .unwrap();
    assert!(residuals.requests().is_empty());

    let evidence = SampledDeclaredModuleDual::try_new(
        &incidence,
        &epoch,
        &query,
        &nominations,
        &residuals,
        source_limits,
    )
    .unwrap();
    let expected_sample = query.sampled().sample_fingerprint().clone();
    let expected_stratum = epoch.fixed_stratum().id().clone();
    let expected_snapshot = epoch.fixed_snapshot_id().clone();
    let expected_diagnostics = obstruction.diagnostics().clone();
    let expected_target_coefficient = obstruction
        .entries()
        .iter()
        .find(|entry| entry.logical_column() == obstruction.target_logical_column())
        .unwrap()
        .coefficient()
        .clone();

    // The admitted evidence owns every retained payload and survives the
    // plan-local query and epoch which were used only for admission joins.
    drop(query);
    drop(epoch);
    assert!(std::sync::Arc::ptr_eq(
        evidence.sample_fingerprint(),
        &expected_sample
    ));
    assert_eq!(evidence.target_shift().values(), &[0]);
    assert_eq!(evidence.stratum_id(), &expected_stratum);
    assert_eq!(evidence.snapshot_id(), &expected_snapshot);
    assert_eq!(evidence.ordering(), OrderingPolicy::default());
    assert_eq!(evidence.final_requests(), &[request(0, 0)]);
    let rank = evidence.rank_census();
    assert_eq!(
        rank.forbidden_columns(),
        expected_diagnostics.forbidden_columns.len()
    );
    assert_eq!(rank.forbidden_rank(), expected_diagnostics.forbidden_rank);
    assert_eq!(rank.augmented_rank(), expected_diagnostics.augmented_rank);
    assert_eq!(
        rank.forbidden_pivot_columns(),
        expected_diagnostics.forbidden_pivot_columns.len()
    );
    assert_eq!(
        rank.augmented_pivot_columns(),
        expected_diagnostics.augmented_pivot_columns.len()
    );
    assert_eq!(
        rank.forbidden_independent_source_rows(),
        expected_diagnostics.forbidden_independent_source_rows.len()
    );
    assert_eq!(
        rank.augmented_independent_source_rows(),
        expected_diagnostics.augmented_independent_source_rows.len()
    );
    assert_eq!(
        rank.forbidden_input_nonzeros(),
        expected_diagnostics.forbidden_input_nonzeros
    );
    assert_eq!(
        rank.augmented_input_nonzeros(),
        expected_diagnostics.augmented_input_nonzeros
    );
    assert_eq!(
        rank.forbidden_lower_pattern_nonzeros(),
        expected_diagnostics.forbidden_lower_pattern_nonzeros
    );
    assert_eq!(
        rank.augmented_lower_pattern_nonzeros(),
        expected_diagnostics.augmented_lower_pattern_nonzeros
    );
    assert_eq!(
        rank.forbidden_upper_nonzeros(),
        expected_diagnostics.forbidden_upper_nonzeros
    );
    assert_eq!(
        rank.augmented_upper_nonzeros(),
        expected_diagnostics.augmented_upper_nonzeros
    );
    assert_eq!(
        rank.forbidden_total_fill_nonzeros(),
        expected_diagnostics.forbidden_total_fill_nonzeros
    );
    assert_eq!(
        rank.augmented_total_fill_nonzeros(),
        expected_diagnostics.augmented_total_fill_nonzeros
    );
    assert_eq!(
        evidence
            .obstruction()
            .iter()
            .filter(|entry| entry.is_target())
            .count(),
        1
    );
    assert_eq!(
        evidence
            .obstruction()
            .iter()
            .find(|entry| entry.is_target())
            .unwrap()
            .shift(),
        evidence.target_shift()
    );
    assert_eq!(
        evidence
            .obstruction()
            .iter()
            .find(|entry| entry.is_target())
            .unwrap()
            .coefficient(),
        &expected_target_coefficient
    );
    assert!(
        evidence
            .obstruction()
            .iter()
            .all(|entry| entry.shift().len() == 1)
    );
    let census = evidence.census();
    assert_eq!(census.declared_source_rows(), 1);
    assert_eq!(census.final_request_count(), 1);
    assert_eq!(census.raw_incidence_visits(), 4);
    assert_eq!(census.structurally_incident_rows(), 3);
    assert_eq!(census.evaluated_unseen_rows(), 2);
    assert_eq!(census.already_materialized_incident_rows(), 1);
    assert_eq!(census.evaluated_source_terms(), 4);
    assert_eq!(census.paired_source_terms(), 2);
}

#[test]
fn sampled_dual_rejects_opaque_guarded_strata_without_a_sample_witness() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let campaign_limits = CampaignLimits::default();
    let source_limits = SourceDiscoveryLimits::default();
    let zero_sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&zero_sources, source_limits).unwrap();
    let requests = AccumulatedSourceRequests::try_new(1, [request(0, 0)], campaign_limits).unwrap();
    let (guard_blind, owners) = fixed_tadpole_inputs(&artifact, 0, &[vec![0], vec![1]]);
    let guard = GuardBranchIdentity::try_new(
        "sampled-dual-opaque-guard",
        GuardBranch::NonZero,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let guarded = DecoratedStratum::try_new(
        guard_blind.family_fingerprint(),
        guard_blind.context_fingerprint(),
        guard_blind.domain().clone(),
        [guard],
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let epoch = FreshTaskEpoch::try_new(
        0,
        &generator,
        &completed,
        requests,
        IntegralShift::try_new([0]).unwrap(),
        guarded,
        owners,
        OrderingPolicy::default(),
        campaign_limits,
    )
    .unwrap();
    let probe = CampaignModularProbe::try_new(PRIME, [4], [0], campaign_limits).unwrap();
    let query = epoch
        .try_query(generator.context(), &probe, campaign_limits)
        .unwrap();
    let obstruction = query.obstruction().unwrap();
    let nominations = incidence
        .try_nominate_obstruction(obstruction, source_limits)
        .unwrap();
    let residuals = incidence
        .try_retain_nonzero_residuals(
            &generator,
            &completed,
            &nominations,
            query.sampled(),
            obstruction,
            source_limits,
        )
        .unwrap();
    assert!(residuals.requests().is_empty());
    assert_eq!(
        SampledDeclaredModuleDual::try_new(
            &incidence,
            &epoch,
            &query,
            &nominations,
            &residuals,
            source_limits,
        )
        .unwrap_err(),
        SampledDeclaredModuleDualError::GuardedStratumRequiresSampleWitness { guard_count: 1 }
    );
}

#[test]
fn sampled_dual_rejects_incomplete_nominations_and_arbitrary_or_foreign_partitions() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let campaign_limits = CampaignLimits::default();
    let source_limits = SourceDiscoveryLimits::default();
    let zero_sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&zero_sources, source_limits).unwrap();
    let requests = AccumulatedSourceRequests::try_new(1, [request(0, 0)], campaign_limits).unwrap();
    let (stratum, owners) = fixed_tadpole_inputs(&artifact, 0, &[vec![0], vec![1]]);
    let epoch = FreshTaskEpoch::try_new(
        0,
        &generator,
        &completed,
        requests.clone(),
        IntegralShift::try_new([0]).unwrap(),
        stratum.clone(),
        owners.clone(),
        OrderingPolicy::default(),
        campaign_limits,
    )
    .unwrap();
    let probe = CampaignModularProbe::try_new(PRIME, [4], [0], campaign_limits).unwrap();
    let query = epoch
        .try_query(generator.context(), &probe, campaign_limits)
        .unwrap();
    let obstruction = query.obstruction().unwrap();
    let nominations = incidence
        .try_nominate_obstruction(obstruction, source_limits)
        .unwrap();
    let residuals = incidence
        .try_retain_nonzero_residuals(
            &generator,
            &completed,
            &nominations,
            query.sampled(),
            obstruction,
            source_limits,
        )
        .unwrap();
    assert!(residuals.requests().is_empty());

    let incomplete = empty_obstruction_nominations_for_test(&incidence, obstruction).unwrap();
    let incomplete_residuals = incidence
        .try_retain_nonzero_residuals(
            &generator,
            &completed,
            &incomplete,
            query.sampled(),
            obstruction,
            source_limits,
        )
        .unwrap();
    assert_eq!(
        SampledDeclaredModuleDual::try_new(
            &incidence,
            &epoch,
            &query,
            &incomplete,
            &incomplete_residuals,
            source_limits,
        )
        .unwrap_err(),
        SampledDeclaredModuleDualError::IncompleteNominationCensus
    );

    // A direct query_target call with the fixed target but an incomplete
    // forbidden projection is a hit, not sampled-dual evidence.
    let incomplete_projection = epoch.projected_query_for_test(
        generator.context(),
        &probe,
        epoch.target_column(),
        &[],
        campaign_limits,
    );
    assert!(matches!(
        incomplete_projection.query(),
        ModularTargetQuery::Hit(_)
    ));
    assert_eq!(
        SampledDeclaredModuleDual::try_new(
            &incidence,
            &epoch,
            &incomplete_projection,
            &nominations,
            &residuals,
            source_limits,
        )
        .unwrap_err(),
        SampledDeclaredModuleDualError::QueryIsModularHit
    );

    // Swapping target and forbidden columns also gives a checked no-hit for
    // this one-row matrix, but its logical map is not the exhaustive fixed
    // partition followed by the fixed target.
    let other_column = (0..epoch.plan().columns().len())
        .find(|&column| column != epoch.target_column())
        .unwrap();
    let arbitrary_projection = epoch.projected_query_for_test(
        generator.context(),
        &probe,
        other_column,
        &[epoch.target_column()],
        campaign_limits,
    );
    assert!(arbitrary_projection.obstruction().is_some());
    assert_eq!(
        SampledDeclaredModuleDual::try_new(
            &incidence,
            &epoch,
            &arbitrary_projection,
            &nominations,
            &residuals,
            source_limits,
        )
        .unwrap_err(),
        SampledDeclaredModuleDualError::ObstructionPartitionMismatch
    );

    let foreign_epoch = FreshTaskEpoch::try_new(
        0,
        &generator,
        &completed,
        requests,
        IntegralShift::try_new([0]).unwrap(),
        stratum,
        owners,
        OrderingPolicy::default(),
        campaign_limits,
    )
    .unwrap();
    let foreign_query = foreign_epoch
        .try_query(generator.context(), &probe, campaign_limits)
        .unwrap();
    assert_eq!(epoch.plan(), foreign_epoch.plan());
    assert!(!std::ptr::eq(epoch.plan(), foreign_epoch.plan()));
    assert_eq!(
        SampledDeclaredModuleDual::try_new(
            &incidence,
            &epoch,
            &foreign_query,
            &nominations,
            &residuals,
            source_limits,
        )
        .unwrap_err(),
        SampledDeclaredModuleDualError::PartitionPlanMismatch
    );
}

#[test]
fn sampled_dual_rejects_foreign_incidence_obstruction_sample_and_cutting_residuals() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let campaign_limits = CampaignLimits::default();
    let source_limits = SourceDiscoveryLimits::default();
    let zero_sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&zero_sources, source_limits).unwrap();
    let foreign_incidence =
        OrdinarySourceIncidenceIndex::try_new(&zero_sources, source_limits).unwrap();
    let requests = AccumulatedSourceRequests::try_new(1, [request(0, 0)], campaign_limits).unwrap();
    let (stratum, owners) = fixed_tadpole_inputs(&artifact, 0, &[vec![0], vec![1]]);
    let epoch = FreshTaskEpoch::try_new(
        0,
        &generator,
        &completed,
        requests,
        IntegralShift::try_new([0]).unwrap(),
        stratum,
        owners,
        OrderingPolicy::default(),
        campaign_limits,
    )
    .unwrap();

    let empty_probe = CampaignModularProbe::try_new(PRIME, [4], [0], campaign_limits).unwrap();
    let empty_query = epoch
        .try_query(generator.context(), &empty_probe, campaign_limits)
        .unwrap();
    let empty_obstruction = empty_query.obstruction().unwrap();
    let nominations = incidence
        .try_nominate_obstruction(empty_obstruction, source_limits)
        .unwrap();
    let residuals = incidence
        .try_retain_nonzero_residuals(
            &generator,
            &completed,
            &nominations,
            empty_query.sampled(),
            empty_obstruction,
            source_limits,
        )
        .unwrap();
    assert!(residuals.requests().is_empty());

    let mut truncated_pairing = residuals.clone();
    truncated_pairing.set_paired_source_terms_for_test(
        residuals
            .paired_source_terms()
            .checked_sub(1)
            .expect("the fixture has at least one exact support intersection"),
    );
    assert_eq!(
        SampledDeclaredModuleDual::try_new(
            &incidence,
            &epoch,
            &empty_query,
            &nominations,
            &truncated_pairing,
            source_limits,
        )
        .unwrap_err(),
        SampledDeclaredModuleDualError::ResidualTelemetryMismatch
    );

    let foreign_nominations = foreign_incidence
        .try_nominate_obstruction(empty_obstruction, source_limits)
        .unwrap();
    let foreign_residuals = foreign_incidence
        .try_retain_nonzero_residuals(
            &generator,
            &completed,
            &foreign_nominations,
            empty_query.sampled(),
            empty_obstruction,
            source_limits,
        )
        .unwrap();
    assert_eq!(
        SampledDeclaredModuleDual::try_new(
            &incidence,
            &epoch,
            &empty_query,
            &foreign_nominations,
            &foreign_residuals,
            source_limits,
        )
        .unwrap_err(),
        SampledDeclaredModuleDualError::NominationIncidenceMismatch
    );

    // Independently rerunning even the same raw probe creates a distinct
    // sample Arc and checked-obstruction owner; stale census provenance is
    // rejected before emptiness can be interpreted.
    let repeated_query = epoch
        .try_query(generator.context(), &empty_probe, campaign_limits)
        .unwrap();
    assert_eq!(
        repeated_query.sampled().sample_fingerprint(),
        empty_query.sampled().sample_fingerprint()
    );
    assert!(!std::sync::Arc::ptr_eq(
        repeated_query.sampled().sample_fingerprint(),
        empty_query.sampled().sample_fingerprint()
    ));
    assert_eq!(
        SampledDeclaredModuleDual::try_new(
            &incidence,
            &epoch,
            &repeated_query,
            &nominations,
            &residuals,
            source_limits,
        )
        .unwrap_err(),
        SampledDeclaredModuleDualError::NominationObstructionMismatch
    );

    let cutting_probe = CampaignModularProbe::try_new(PRIME, [37], [20], campaign_limits).unwrap();
    let cutting_query = epoch
        .try_query(generator.context(), &cutting_probe, campaign_limits)
        .unwrap();
    let cutting_obstruction = cutting_query.obstruction().unwrap();
    let cutting_nominations = incidence
        .try_nominate_obstruction(cutting_obstruction, source_limits)
        .unwrap();
    let cutting_residuals = incidence
        .try_retain_nonzero_residuals(
            &generator,
            &completed,
            &cutting_nominations,
            cutting_query.sampled(),
            cutting_obstruction,
            source_limits,
        )
        .unwrap();
    assert!(!cutting_residuals.requests().is_empty());
    assert_eq!(
        SampledDeclaredModuleDual::try_new(
            &incidence,
            &epoch,
            &cutting_query,
            &cutting_nominations,
            &cutting_residuals,
            source_limits,
        )
        .unwrap_err(),
        SampledDeclaredModuleDualError::CuttingResiduals {
            count: cutting_residuals.requests().len(),
        }
    );

    let mut nomination_cap = source_limits;
    nomination_cap.max_incidence_visits = 3;
    assert_eq!(
        SampledDeclaredModuleDual::try_new(
            &incidence,
            &epoch,
            &empty_query,
            &nominations,
            &residuals,
            nomination_cap,
        )
        .unwrap_err(),
        SampledDeclaredModuleDualError::NominationVerification(
            SourceDiscoveryError::ResourceLimit {
                resource: "source-discovery inverse-incidence visits",
                requested: 4,
                limit: 3,
            }
        )
    );

    let mut incidence_cap = source_limits;
    incidence_cap.max_arity = 0;
    assert_eq!(
        SampledDeclaredModuleDual::try_new(
            &incidence,
            &epoch,
            &empty_query,
            &nominations,
            &residuals,
            incidence_cap,
        )
        .unwrap_err(),
        SampledDeclaredModuleDualError::IncidenceVerification(
            SourceDiscoveryError::ResourceLimit {
                resource: "source-discovery arity",
                requested: 1,
                limit: 0,
            }
        )
    );

    let mut pairing_cap = source_limits;
    pairing_cap.max_sampled_dual_pairing_coordinate_cells = 3;
    assert_eq!(
        SampledDeclaredModuleDual::try_new(
            &incidence,
            &epoch,
            &empty_query,
            &nominations,
            &residuals,
            pairing_cap,
        )
        .unwrap_err(),
        SampledDeclaredModuleDualError::NominationVerification(
            SourceDiscoveryError::ResourceLimit {
                resource: "sampled-dual exact pairing coordinate cells",
                requested: 4,
                limit: 3,
            }
        )
    );

    let mut sample_cap = source_limits;
    sample_cap.max_sampled_dual_sample_coordinates = 0;
    assert_eq!(
        SampledDeclaredModuleDual::try_new(
            &incidence,
            &epoch,
            &empty_query,
            &nominations,
            &residuals,
            sample_cap,
        )
        .unwrap_err(),
        SampledDeclaredModuleDualError::Retention(SourceDiscoveryError::ResourceLimit {
            resource: "sampled-dual retained sample coordinates",
            requested: empty_query.sampled().sample_fingerprint().point().len(),
            limit: 0,
        })
    );

    let mut retention_cap = source_limits;
    retention_cap.max_sampled_dual_requests = 0;
    assert_eq!(
        SampledDeclaredModuleDual::try_new(
            &incidence,
            &epoch,
            &empty_query,
            &nominations,
            &residuals,
            retention_cap,
        )
        .unwrap_err(),
        SampledDeclaredModuleDualError::Retention(SourceDiscoveryError::ResourceLimit {
            resource: "sampled-dual retained source requests",
            requested: 1,
            limit: 0,
        })
    );

    let mut obstruction_cap = source_limits;
    obstruction_cap.max_sampled_dual_obstruction_entries = 0;
    assert_eq!(
        SampledDeclaredModuleDual::try_new(
            &incidence,
            &epoch,
            &empty_query,
            &nominations,
            &residuals,
            obstruction_cap,
        )
        .unwrap_err(),
        SampledDeclaredModuleDualError::Retention(SourceDiscoveryError::ResourceLimit {
            resource: "sampled-dual raw obstruction entries",
            requested: empty_obstruction.entries().len(),
            limit: 0,
        })
    );

    let mut diagnostic_cap = source_limits;
    diagnostic_cap.max_sampled_dual_diagnostic_ordinals = 0;
    assert_eq!(
        SampledDeclaredModuleDual::try_new(
            &incidence,
            &epoch,
            &empty_query,
            &nominations,
            &residuals,
            diagnostic_cap,
        )
        .unwrap_err(),
        SampledDeclaredModuleDualError::Retention(SourceDiscoveryError::ResourceLimit {
            resource: "sampled-dual inspected rank diagnostic ordinals",
            requested: empty_obstruction.diagnostics().forbidden_columns.len(),
            limit: 0,
        })
    );
}

#[test]
fn singular_incident_candidate_fails_before_sampled_dual_admission() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let campaign_limits = CampaignLimits::default();
    let source_limits = SourceDiscoveryLimits::default();
    let zero_sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&zero_sources, source_limits).unwrap();
    let requests = AccumulatedSourceRequests::try_new(1, [request(0, 0)], campaign_limits).unwrap();
    let (stratum, owners) = fixed_tadpole_inputs(&artifact, 0, &[vec![0], vec![1]]);
    let epoch = FreshTaskEpoch::try_new(
        0,
        &generator,
        &completed,
        requests,
        IntegralShift::try_new([0]).unwrap(),
        stratum,
        owners,
        OrderingPolicy::default(),
        campaign_limits,
    )
    .unwrap();
    let probe = CampaignModularProbe::try_new(PRIME, [4], [0], campaign_limits).unwrap();
    let query = epoch
        .try_query(generator.context(), &probe, campaign_limits)
        .unwrap();
    let obstruction = query.obstruction().unwrap();
    let nominations = incidence
        .try_nominate_obstruction(obstruction, source_limits)
        .unwrap();
    let mut selected = generator
        .translate_selected_completed_source_rows(
            &completed,
            nominations.requests().iter().cloned(),
            source_limits.translation,
        )
        .unwrap();
    let singular = generator
        .context()
        .lift(&generator.context().base().coefficient_fixture("1/(d-4)"))
        .unwrap();
    selected
        .replace_term_without_denominator_gate_for_test(generator.context(), 0, 0, singular)
        .unwrap();
    assert_eq!(
        pair_selected_sources_for_test(
            &incidence,
            &generator,
            &completed,
            &nominations,
            query.sampled(),
            obstruction,
            selected,
            source_limits,
        )
        .unwrap_err(),
        SourceDiscoveryError::CandidateEvaluation {
            candidate_ordinal: 0,
            source_ordinal: nominations.requests()[0].source_ordinal(),
            error: ModularSourceEvaluationError::TermDenominatorZero { term_ordinal: 0 },
        }
    );

    // No partial census escapes the singular call. A fresh clean evaluation
    // is required before the checked admission constructor can be invoked.
    let clean = incidence
        .try_retain_nonzero_residuals(
            &generator,
            &completed,
            &nominations,
            query.sampled(),
            obstruction,
            source_limits,
        )
        .unwrap();
    assert!(clean.requests().is_empty());
    SampledDeclaredModuleDual::try_new(
        &incidence,
        &epoch,
        &query,
        &nominations,
        &clean,
        source_limits,
    )
    .unwrap();
}
