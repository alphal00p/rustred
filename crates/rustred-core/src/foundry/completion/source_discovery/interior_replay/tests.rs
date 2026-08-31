use crate::foundry::artifact::{ClosedArtifact, derive_one_loop_unit_mass_tadpole};
use crate::foundry::completion::source_discovery::{
    CampaignModularProbe, ExactExecutableOwnerProposal, OrdinarySourceIncidenceIndex,
    SourceDiscoveryLimits,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, MaximalStratumAnchor, StratumRegistryLimits,
};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::{
    InteriorReplayRunDisposition, InteriorReplayRunLimits, support_shapes_match,
    try_run_interior_replay_task,
};

const PRIME: u64 = 1_000_000_007;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn anchor_and_owners(
    artifact: &ClosedArtifact,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    target: &IntegralShift,
    limits: InteriorReplayRunLimits,
) -> (MaximalStratumAnchor, ImmutableOwnerSnapshot) {
    let discovery = SourceDiscoveryLimits::default();
    let zero_sources = generator
        .translate_completed_source_rows(
            completed,
            [IntegralShift::try_new([0]).unwrap()],
            discovery.translation,
        )
        .unwrap();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&zero_sources, discovery).unwrap();
    let nominations = incidence
        .try_nominate_target_unit(target, discovery)
        .unwrap();
    let selected = generator
        .translate_selected_completed_source_rows(
            completed,
            nominations.requests().iter().cloned(),
            limits.scheduler.campaign.translated_sources,
        )
        .unwrap();
    let physical_shifts = selected
        .sources()
        .iter()
        .flat_map(|source| source.terms().keys())
        .map(|shift| shift.values().to_vec())
        .collect::<Vec<_>>();
    let registry = StratumRegistryLimits::default();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        target.values(),
        &physical_shifts,
    )
    .unwrap();
    let stratum = DecoratedStratum::try_guard_blind(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        domain,
        registry,
    )
    .unwrap();
    let anchor = MaximalStratumAnchor::try_new(stratum, registry).unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        1,
        registry,
    )
    .unwrap();
    (anchor, owners)
}

fn run_tadpole(
    artifact: &ClosedArtifact,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    target_power: i64,
    mut limits: InteriorReplayRunLimits,
) -> Result<super::InteriorReplayTaskReport, super::InteriorReplayRunError> {
    let target = IntegralShift::try_new([target_power]).unwrap();
    let (anchor, owners) = anchor_and_owners(artifact, generator, completed, &target, limits);
    // Canonical replay performs the same source reconstruction as the
    // scheduler. Keep their nested translation/campaign policies aligned in
    // this focused regression.
    limits.canonical_replay.campaign = limits.scheduler.campaign;
    limits.canonical_replay.source_discovery = limits.scheduler.source_discovery;
    let probes = [2, 3].map(|coordinate| {
        CampaignModularProbe::try_new(PRIME, [37], [coordinate], limits.scheduler.campaign).unwrap()
    });
    try_run_interior_replay_task(
        generator,
        completed,
        target,
        anchor,
        owners,
        OrderingPolicy::default(),
        probes,
        limits,
    )
}

fn compiled_support(report: &super::InteriorReplayTaskReport) -> &super::InteriorReplaySupportSet {
    let InteriorReplayRunDisposition::OwnerProposal {
        proposal: ExactExecutableOwnerProposal::Compiled { .. },
        support: Some(support),
    } = report.disposition()
    else {
        panic!("the streamed target must compile")
    };
    support
}

#[test]
fn live_scheduler_report_replays_and_compiles_before_old_epochs_drop() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let report = run_tadpole(
        &artifact,
        &generator,
        &completed,
        1,
        InteriorReplayRunLimits::default(),
    )
    .unwrap();

    assert_eq!(report.scheduler_outcomes().replayed(), 2);
    assert_eq!(report.scheduler_outcomes().support_did_not_lift(), 0);
    assert_eq!(report.scheduler_outcomes().exact_lift_error(), 0);
    assert_eq!(report.scheduler_outcomes().sampled_dual(), 0);
    assert_eq!(report.scheduler_outcomes().budget_stop(), 0);
    assert_eq!(report.scheduler_outcomes().rejected(), 0);
    assert_eq!(report.scheduler_outcomes().stalled(), 0);
    let telemetry = report.replay().unwrap();
    assert_eq!(telemetry.replayed_nominations(), 2);
    assert_eq!(telemetry.unique_candidates(), 1);

    let InteriorReplayRunDisposition::OwnerProposal {
        proposal:
            ExactExecutableOwnerProposal::Compiled {
                owner,
                obstructions,
            },
        support: Some(support),
    } = report.disposition()
    else {
        panic!("the tadpole replay must retain a compiled exact owner")
    };
    assert!(obstructions.is_empty());
    assert_eq!(owner.executable_candidates().len(), 1);
    assert!(
        owner.executable_candidates()[0]
            .circuit()
            .is_bound_to(owner.epoch().plan())
    );
    assert_eq!(support.candidates().len(), 1);
    assert!(!support.candidates()[0].sources().is_empty());
    assert!(!support.candidates()[0].residuals().is_empty());
    assert_eq!(support.census().candidates(), 1);
}

#[test]
fn relative_support_shape_is_comparable_but_not_admission_authority() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let first = run_tadpole(
        &artifact,
        &generator,
        &completed,
        1,
        InteriorReplayRunLimits::default(),
    )
    .unwrap();
    let second = run_tadpole(
        &artifact,
        &generator,
        &completed,
        2,
        InteriorReplayRunLimits::default(),
    )
    .unwrap();

    assert!(support_shapes_match(
        compiled_support(&first),
        compiled_support(&second)
    ));
}

#[test]
fn support_resource_failure_cannot_publish_a_partial_owner_report() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = InteriorReplayRunLimits::default();
    limits.max_relative_source_supports = 0;
    let error = run_tadpole(&artifact, &generator, &completed, 1, limits).unwrap_err();
    assert!(matches!(
        error,
        super::InteriorReplayRunError::ResourceLimit {
            resource: "relative source supports",
            ..
        }
    ));
}

#[test]
fn coordinate_cap_is_checked_before_relative_support_allocation() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = InteriorReplayRunLimits::default();
    limits.max_relative_coordinate_cells = 0;
    let error = run_tadpole(&artifact, &generator, &completed, 1, limits).unwrap_err();
    assert!(matches!(
        error,
        super::InteriorReplayRunError::ResourceLimit {
            resource: "relative support coordinate cells",
            requested: 1,
            limit: 0,
        }
    ));
}
