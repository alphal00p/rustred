use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole;
use crate::foundry::completion::guard::{
    ExactGuardPredicateCatalog, ExactGuardPredicateCatalogLimits, ExactGuardProbeError,
    ExactGuardProbeLimits, ExactGuardProbeWitness,
};
use crate::foundry::completion::source_discovery::{
    CampaignError, CampaignLimits, CampaignModularProbe,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, GuardBranch, GuardBranchIdentity, ImmutableOwnerSnapshot,
    StratumRegistryLimits,
};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator, TranslatedSourceRequest,
};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::super::{SpiredCase, SpiredCoordinateFace, SpiredExecutionCase};
use super::{SpiredStreamingDiscovery, SpiredStreamingError, SpiredStreamingLimits};

const PRIME: u64 = 101;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn one_loop_case(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
) -> SpiredExecutionCase {
    one_loop_case_for_target(generator, completed, 1)
}

fn one_loop_case_for_target(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    target_value: i64,
) -> SpiredExecutionCase {
    let target = IntegralShift::try_new([target_value]).unwrap();
    let structural_shifts = completed.relations()[0]
        .terms()
        .keys()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        target.values(),
        &structural_shifts,
    )
    .unwrap();
    let stratum = DecoratedStratum::try_guard_blind(
        completed.family_fingerprint(),
        generator.context().fingerprint(),
        domain,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        stratum.family_fingerprint(),
        stratum.context_fingerprint(),
        1,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    SpiredExecutionCase::try_new(
        SpiredCase::CoordinateFace(SpiredCoordinateFace::new(stratum)),
        target,
        OrderingPolicy::default(),
        owners,
    )
    .unwrap()
}

fn one_loop_guarded_case(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    branch: GuardBranch,
) -> SpiredExecutionCase {
    let target = IntegralShift::try_new([1]).unwrap();
    let structural_shifts = completed.relations()[0]
        .terms()
        .keys()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        target.values(),
        &structural_shifts,
    )
    .unwrap();
    let limits = StratumRegistryLimits::default();
    let guard = GuardBranchIdentity::try_new("streaming-test-guard", branch, limits).unwrap();
    let stratum = DecoratedStratum::try_new(
        completed.family_fingerprint(),
        generator.context().fingerprint(),
        domain,
        [guard],
        limits,
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        stratum.family_fingerprint(),
        stratum.context_fingerprint(),
        1,
        limits,
    )
    .unwrap();
    SpiredExecutionCase::try_new(
        SpiredCase::CoordinateFace(SpiredCoordinateFace::new(stratum)),
        target,
        OrderingPolicy::default(),
        owners,
    )
    .unwrap()
}

fn indexed_guard(
    generator: &ParametricIbpGenerator<'_>,
    constant: i64,
) -> crate::algebra::IndexedPolynomial {
    let value = generator
        .context()
        .add(
            &generator.context().index(0).unwrap(),
            &generator.context().integer(constant),
        )
        .unwrap();
    generator
        .context()
        .numerator_condition_with_limits(&value, Default::default())
        .unwrap()
}

fn one_loop_indexed_guarded_case(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    guard: &crate::algebra::IndexedPolynomial,
    branch: GuardBranch,
) -> SpiredExecutionCase {
    let target = IntegralShift::try_new([1]).unwrap();
    let structural_shifts = completed.relations()[0]
        .terms()
        .keys()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        target.values(),
        &structural_shifts,
    )
    .unwrap();
    let limits = StratumRegistryLimits::default();
    let identity = GuardBranchIdentity::try_from_indexed_polynomial(
        generator.context(),
        guard,
        branch,
        Default::default(),
        limits,
    )
    .unwrap();
    let stratum = DecoratedStratum::try_new(
        completed.family_fingerprint(),
        generator.context().fingerprint(),
        domain,
        [identity],
        limits,
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        stratum.family_fingerprint(),
        stratum.context_fingerprint(),
        1,
        limits,
    )
    .unwrap();
    SpiredExecutionCase::try_new(
        SpiredCase::CoordinateFace(SpiredCoordinateFace::new(stratum)),
        target,
        OrderingPolicy::default(),
        owners,
    )
    .unwrap()
}

fn exact_guard_witness(
    generator: &ParametricIbpGenerator<'_>,
    case: &SpiredExecutionCase,
    probe: &CampaignModularProbe,
    guard: crate::algebra::IndexedPolynomial,
) -> ExactGuardProbeWitness {
    let catalog = ExactGuardPredicateCatalog::try_from_pulled_back(
        generator.context(),
        [guard],
        ExactGuardPredicateCatalogLimits::default(),
    )
    .unwrap();
    let anchor = probe.try_index_anchor_for_stratum(case.stratum()).unwrap();
    ExactGuardProbeWitness::try_new(
        generator.context(),
        case.stratum(),
        probe.base_parameters(),
        &anchor,
        &catalog,
        ExactGuardProbeLimits::default(),
    )
    .unwrap()
}

fn zero_request() -> TranslatedSourceRequest {
    TranslatedSourceRequest::new(0, IntegralShift::try_new([0]).unwrap())
}

fn probe(generator: &ParametricIbpGenerator<'_>) -> CampaignModularProbe {
    CampaignModularProbe::try_new(
        PRIME,
        vec![37; generator.context().base().parameter_names().len()],
        [2],
        CampaignLimits::default(),
    )
    .unwrap()
}

#[test]
fn guarded_strata_fail_closed_without_an_exact_sample_witness() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let probe = probe(&generator);

    for branch in [GuardBranch::Zero, GuardBranch::NonZero] {
        let case = one_loop_guarded_case(&generator, &completed, branch);
        assert!(matches!(
            SpiredStreamingDiscovery::try_new(
                generator.context(),
                &completed,
                &case,
                &probe,
                SpiredStreamingLimits::default(),
            ),
            Err(SpiredStreamingError::GuardedStratumRequiresSampleWitness { guard_count: 1 })
        ));
    }
}

#[test]
fn exact_indexed_zero_guard_joins_streaming_and_consumes_a_real_hit() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    // The retained raw chart coordinate x=2 maps to n=x+1=3.
    let guard = indexed_guard(&generator, -3);
    let case = one_loop_indexed_guarded_case(&generator, &completed, &guard, GuardBranch::Zero);
    let probe = probe(&generator);
    let witness = exact_guard_witness(&generator, &case, &probe, guard);
    let mut discovery = SpiredStreamingDiscovery::try_new_guarded(
        generator.context(),
        &completed,
        &case,
        &probe,
        &witness,
        SpiredStreamingLimits::default(),
    )
    .unwrap();
    assert_eq!(discovery.guard_witness(), Some(&witness));

    let hit = discovery
        .try_consume_chunk(std::slice::from_ref(&zero_request()))
        .unwrap()
        .expect("the exactly guarded K=1 source must expose the target");
    assert_eq!(hit.rows_consumed(), 1);
    assert_eq!(discovery.rows_consumed(), 1);
}

#[test]
fn guarded_stream_join_rejects_a_witness_for_another_exact_probe() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let guard = indexed_guard(&generator, -3);
    let case = one_loop_indexed_guarded_case(&generator, &completed, &guard, GuardBranch::Zero);
    let admitted_probe = probe(&generator);
    let witness = exact_guard_witness(&generator, &case, &admitted_probe, guard);
    let foreign_probe = CampaignModularProbe::try_new(
        PRIME,
        vec![41; generator.context().base().parameter_names().len()],
        [2],
        CampaignLimits::default(),
    )
    .unwrap();

    assert!(matches!(
        SpiredStreamingDiscovery::try_new_guarded(
            generator.context(),
            &completed,
            &case,
            &foreign_probe,
            &witness,
            SpiredStreamingLimits::default(),
        ),
        Err(SpiredStreamingError::GuardProbe(
            ExactGuardProbeError::WitnessBasePointMismatch
        ))
    ));
}

#[test]
fn guarded_stream_join_reuses_exact_witness_but_rejects_an_unlucky_prime() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    // n+98 evaluates exactly to 101 at the retained n=3 probe.
    let guard = indexed_guard(&generator, 98);
    let case = one_loop_indexed_guarded_case(&generator, &completed, &guard, GuardBranch::NonZero);
    let unlucky = probe(&generator);
    let witness = exact_guard_witness(&generator, &case, &unlucky, guard);
    assert!(matches!(
        SpiredStreamingDiscovery::try_new_guarded(
            generator.context(),
            &completed,
            &case,
            &unlucky,
            &witness,
            SpiredStreamingLimits::default(),
        ),
        Err(SpiredStreamingError::GuardProbe(
            ExactGuardProbeError::UnluckyGuardPrime {
                guard_ordinal: 0,
                modulus: PRIME,
            }
        ))
    ));

    let lucky = CampaignModularProbe::try_new(
        103,
        unlucky.base_parameters().iter().copied(),
        unlucky.chart_coordinates().iter().copied(),
        CampaignLimits::default(),
    )
    .unwrap();
    let mut discovery = SpiredStreamingDiscovery::try_new_guarded(
        generator.context(),
        &completed,
        &case,
        &lucky,
        &witness,
        SpiredStreamingLimits::default(),
    )
    .unwrap();
    assert!(
        discovery
            .try_consume_chunk(std::slice::from_ref(&zero_request()))
            .unwrap()
            .is_some()
    );
}

#[test]
fn one_loop_first_row_finds_compact_target_support() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    assert_eq!(completed.source_row_count(), 1);
    let case = one_loop_case(&generator, &completed);
    let probe = probe(&generator);
    let request = zero_request();
    let mut discovery = SpiredStreamingDiscovery::try_new(
        generator.context(),
        &completed,
        &case,
        &probe,
        SpiredStreamingLimits::default(),
    )
    .unwrap();
    assert_eq!(discovery.probe(), &probe);

    let hit = discovery
        .try_consume_chunk(std::slice::from_ref(&request))
        .unwrap()
        .expect("the tadpole recurrence must expose the target on its first source");
    assert_eq!(hit.rows_consumed(), 1);
    assert_eq!(hit.forbidden_rank(), 0);
    assert_eq!(hit.augmented_rank(), 1);
    assert_eq!(hit.support(), &[request]);
    assert_eq!(discovery.rows_consumed(), 1);
    assert_eq!(discovery.cached_shift_count(), 2);
    assert_eq!(discovery.forbidden_column_count(), 0);
    assert!(!discovery.is_poisoned());
}

#[test]
fn chunk_preparation_registers_a_forbidden_union_once_before_rows() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = one_loop_case_for_target(&generator, &completed, 0);
    let probe = probe(&generator);
    let request = zero_request();
    let mut discovery = SpiredStreamingDiscovery::try_new(
        generator.context(),
        &completed,
        &case,
        &probe,
        SpiredStreamingLimits::default(),
    )
    .unwrap();

    assert_eq!(
        discovery
            .try_prepare_requests(std::slice::from_ref(&request))
            .unwrap(),
        1
    );
    assert_eq!(discovery.rows_consumed(), 0);
    assert_eq!(discovery.forbidden_column_count(), 1);
    assert_eq!(discovery.registered_forbidden_column_count(), 1);
    assert_eq!(
        discovery
            .try_prepare_requests(std::slice::from_ref(&request))
            .unwrap(),
        0
    );
    assert!(discovery.try_consume_request(&request).is_ok());
}

#[test]
fn preparation_work_limits_admit_the_exact_boundary_and_count_duplicates() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let source_term_count = completed.relations()[0].terms().len();
    assert!(source_term_count > 0);
    let case = one_loop_case_for_target(&generator, &completed, 0);
    let probe = probe(&generator);
    let duplicate_count = 3;
    let duplicate_term_count = duplicate_count * source_term_count;
    let mut limits = SpiredStreamingLimits::default();
    limits.max_preparation_request_inspections = duplicate_count;
    limits.max_preparation_source_term_inspections = duplicate_term_count;
    let mut discovery =
        SpiredStreamingDiscovery::try_new(generator.context(), &completed, &case, &probe, limits)
            .unwrap();
    let requests = vec![zero_request(); duplicate_count];

    assert_eq!(discovery.try_prepare_requests(&requests).unwrap(), 1);
    assert_eq!(discovery.cached_shift_count(), source_term_count);
    assert!(!discovery.is_poisoned());

    assert!(matches!(
        discovery.try_prepare_requests(std::slice::from_ref(&zero_request())),
        Err(SpiredStreamingError::ResourceLimit {
            resource: "prepared translated-source request inspections",
            requested,
            limit,
        }) if requested == duplicate_count + 1 && limit == duplicate_count
    ));
    assert!(discovery.is_poisoned());
}

#[test]
fn duplicate_slice_cannot_evade_cumulative_source_term_work_limit() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let source_term_count = completed.relations()[0].terms().len();
    assert!(source_term_count > 0);
    let case = one_loop_case_for_target(&generator, &completed, 0);
    let probe = probe(&generator);
    let duplicate_count = 2;
    let requested_term_count = duplicate_count * source_term_count;
    let term_limit = requested_term_count - 1;
    let mut limits = SpiredStreamingLimits::default();
    limits.max_preparation_request_inspections = duplicate_count;
    limits.max_preparation_source_term_inspections = term_limit;
    let mut discovery =
        SpiredStreamingDiscovery::try_new(generator.context(), &completed, &case, &probe, limits)
            .unwrap();
    let requests = vec![zero_request(); duplicate_count];

    assert!(matches!(
        discovery.try_prepare_requests(&requests),
        Err(SpiredStreamingError::ResourceLimit {
            resource: "prepared exact source-term inspections",
            requested,
            limit,
        }) if requested == requested_term_count && limit == term_limit
    ));
    assert_eq!(discovery.cached_shift_count(), 0);
    assert_eq!(discovery.rows_consumed(), 0);
    assert!(discovery.is_poisoned());
}

#[test]
fn evaluation_failure_precedes_registry_mutation_and_does_not_poison() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = one_loop_case(&generator, &completed);
    let probe = probe(&generator);
    let mut discovery = SpiredStreamingDiscovery::try_new(
        generator.context(),
        &completed,
        &case,
        &probe,
        SpiredStreamingLimits::default(),
    )
    .unwrap();
    let invalid = TranslatedSourceRequest::new(1, IntegralShift::try_new([0]).unwrap());
    assert!(matches!(
        discovery.try_consume_request(&invalid),
        Err(SpiredStreamingError::Evaluation(
            super::super::DirectShiftedSourceError::SourceOrdinalOutOfRange { .. }
        ))
    ));
    assert_eq!(discovery.cached_shift_count(), 0);
    assert!(!discovery.is_poisoned());
    assert!(
        discovery
            .try_consume_request(&zero_request())
            .unwrap()
            .is_some()
    );
}

#[test]
fn cumulative_unique_shift_limits_cannot_be_evaded_by_per_shift_classification() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = one_loop_case(&generator, &completed);
    let probe = probe(&generator);
    let mut limits = SpiredStreamingLimits::default();
    limits.classification.max_physical_columns = 1;
    let mut discovery =
        SpiredStreamingDiscovery::try_new(generator.context(), &completed, &case, &probe, limits)
            .unwrap();
    assert!(matches!(
        discovery.try_consume_request(&zero_request()),
        Err(SpiredStreamingError::ResourceLimit {
            resource: "prospective physical columns",
            requested: 2,
            limit: 1,
        })
    ));
    assert!(discovery.is_poisoned());
}

#[test]
fn raw_chart_probe_is_checked_against_the_exact_case_before_streaming() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = one_loop_case(&generator, &completed);
    let probe = CampaignModularProbe::try_new(
        PRIME,
        vec![37; generator.context().base().parameter_names().len()],
        [i64::MAX as u64],
        CampaignLimits::default(),
    )
    .unwrap();

    assert!(matches!(
        SpiredStreamingDiscovery::try_new(
            generator.context(),
            &completed,
            &case,
            &probe,
            SpiredStreamingLimits::default(),
        ),
        Err(SpiredStreamingError::Probe(
            CampaignError::SampleCoordinateNotRepresentable {
                position: 0,
                active: true,
                coordinate,
            }
        )) if coordinate == i64::MAX as u64
    ));
}
