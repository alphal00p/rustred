//! Probe-local bootstrap census for unresolved full-rank K=6 orbits.
//!
//! This is deliberately test-only discovery telemetry. A modular hit, sampled
//! obstruction, exact replay, or scheduler stop recorded here cannot install a
//! rule, owner, terminal, sector layer, or closing artifact.

use crate::foundry::completion::source_discovery::scheduler::{
    ProbeLocalBudgetCause, ProbeLocalIterationDisposition, ProbeLocalObstructionScheduler,
    ProbeLocalOutcome, ProbeLocalOutcomeKind, ProbeLocalSchedulerLimits, ProbeLocalStage,
};
use crate::foundry::completion::source_discovery::{
    CampaignModularProbe, OrdinarySourceIncidenceIndex,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, MaximalStratumAnchor,
};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpConfig, ParametricIbpGenerator,
};
use crate::sector::{
    InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain, SectorMonotoneDomain,
};

use super::manifest::FULL_RANK_ORBITS;
use super::{canonical_family, derive_k6_terminal_authority};

const PROBE_MODULI: [u64; 3] = [1_000_000_007, 1_000_000_009, 998_244_353];
const BASE_PARAMETERS: [i64; 1] = [37];
const CHART_COORDINATES: [u64; 6] = [1, 2, 3, 4, 5, 6];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BootstrapTerminalTelemetry {
    Replayed {
        selected_sources: usize,
        residual_terms: usize,
        pivot_guards: usize,
        nonzero_guards: usize,
        replay_source_terms: usize,
        replay_exact_operations: usize,
    },
    SupportDidNotLift {
        selected_sources: usize,
        exact_forbidden_rank: usize,
        exact_augmented_rank: usize,
    },
    ExactLiftError,
    SampledDual {
        obstruction_entries: usize,
        structurally_incident_rows: usize,
        evaluated_unseen_rows: usize,
        evaluated_source_terms: usize,
        paired_source_terms: usize,
    },
    OneEpochLimit {
        requested_iteration: usize,
        final_requests: usize,
    },
    Rejected {
        stage: ProbeLocalStage,
    },
    Stalled {
        nonzero_residual_requests: usize,
    },
    OtherBudgetStop {
        stage: ProbeLocalStage,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BootstrapProbeTelemetry {
    orbit_ordinal: usize,
    representative: [i64; 6],
    active_propagators: usize,
    probe_ordinal: usize,
    modulus: u64,
    request_count: usize,
    physical_rows: usize,
    physical_columns: usize,
    physical_entries: usize,
    allowed_columns: usize,
    forbidden_columns: usize,
    forbidden_rank: usize,
    augmented_rank: usize,
    exact_lift_attempted: bool,
    disposition: ProbeLocalIterationDisposition,
    outcome: ProbeLocalOutcomeKind,
    terminal: BootstrapTerminalTelemetry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ObstructionBlockWidthTelemetry {
    representative: [i64; 6],
    configured_width: usize,
    disposition: ProbeLocalIterationDisposition,
    final_requests: usize,
    residual_candidate_work: usize,
    residual_source_term_work: usize,
    block_candidate_work: usize,
    block_source_term_work: usize,
    block_signature_work: usize,
    block_selection_work: usize,
    cache_logical_rows: usize,
    cache_logical_terms: usize,
    cache_rows: usize,
    cache_value_cells: usize,
    cache_physical_evaluations: usize,
    cache_hits: usize,
    exact_lift_attempts: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExpectedBootstrapSector {
    representative: [i64; 6],
    active_propagators: usize,
    allowed_columns: usize,
    forbidden_columns: usize,
    nominated_requests: usize,
    nonzero_residual_requests: usize,
    added_requests: usize,
    final_requests: usize,
}

const EXPECTED_BOOTSTRAP_SECTORS: [ExpectedBootstrapSector; 6] = [
    ExpectedBootstrapSector {
        representative: [0, 0, 1, 0, 1, 1],
        active_propagators: 3,
        allowed_columns: 13,
        forbidden_columns: 239,
        nominated_requests: 3_799,
        nonzero_residual_requests: 3_763,
        added_requests: 32,
        final_requests: 122,
    },
    ExpectedBootstrapSector {
        representative: [0, 0, 1, 1, 0, 1],
        active_propagators: 3,
        allowed_columns: 19,
        forbidden_columns: 233,
        nominated_requests: 3_857,
        nonzero_residual_requests: 3_822,
        added_requests: 32,
        final_requests: 122,
    },
    ExpectedBootstrapSector {
        representative: [0, 0, 1, 1, 1, 1],
        active_propagators: 4,
        allowed_columns: 9,
        forbidden_columns: 243,
        nominated_requests: 3_797,
        nonzero_residual_requests: 3_765,
        added_requests: 32,
        final_requests: 122,
    },
    ExpectedBootstrapSector {
        representative: [0, 1, 1, 1, 1, 0],
        active_propagators: 4,
        allowed_columns: 0,
        forbidden_columns: 252,
        nominated_requests: 3_764,
        nonzero_residual_requests: 3_727,
        added_requests: 32,
        final_requests: 122,
    },
    ExpectedBootstrapSector {
        representative: [0, 1, 1, 1, 1, 1],
        active_propagators: 5,
        // Exact factorized proper-subsector domains authenticate only the
        // sampled columns in their sparse preimages; the rest remain blind.
        allowed_columns: 44,
        forbidden_columns: 208,
        nominated_requests: 4_208,
        nonzero_residual_requests: 4_173,
        added_requests: 32,
        final_requests: 122,
    },
    ExpectedBootstrapSector {
        representative: [1, 1, 1, 1, 1, 1],
        active_propagators: 6,
        allowed_columns: 0,
        forbidden_columns: 252,
        nominated_requests: 3_764,
        nonzero_residual_requests: 3_586,
        added_requests: 32,
        final_requests: 122,
    },
];

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn census_limits() -> ProbeLocalSchedulerLimits {
    let mut limits = ProbeLocalSchedulerLimits::default();
    limits.max_probes = PROBE_MODULI.len();
    limits.max_retained_probe_coordinate_cells =
        PROBE_MODULI.len() * (BASE_PARAMETERS.len() + CHART_COORDINATES.len());
    limits.max_retained_outcomes = PROBE_MODULI.len();
    limits.max_iterations_per_probe = 1;
    limits.max_requests_per_probe = 3_912;
    limits.max_request_coordinate_cells_per_probe = 6 * 3_912;
    limits.max_aggregate_epochs = PROBE_MODULI.len();
    limits.max_aggregate_epoch_request_work = PROBE_MODULI.len() * 90;
    limits.max_aggregate_materialized_source_terms = PROBE_MODULI.len() * 918;
    limits.max_aggregate_modular_entry_work = PROBE_MODULI.len() * 918;
    limits.max_aggregate_merge_request_work = PROBE_MODULI.len() * (90 + 3_822);
    limits.max_retained_iteration_records = PROBE_MODULI.len();
    limits.max_exact_lift_attempts = PROBE_MODULI.len();
    limits
}

fn bounded_proposal_pressure_limits() -> ProbeLocalSchedulerLimits {
    let mut limits = ProbeLocalSchedulerLimits::default();
    limits.max_probes = 1;
    limits.max_retained_probe_coordinate_cells = BASE_PARAMETERS.len() + CHART_COORDINATES.len();
    limits.max_retained_outcomes = 1;
    limits.max_iterations_per_probe = 8;
    limits.max_requests_per_probe = 100_000;
    limits.max_request_coordinate_cells_per_probe = 6 * 100_000;
    limits.max_residual_proposals_per_iteration = 32;
    limits.max_aggregate_epochs = 8;
    limits.max_aggregate_epoch_request_work = 100_000;
    limits.max_aggregate_materialized_source_terms = 2_000_000;
    limits.max_aggregate_modular_entry_work = 2_000_000;
    limits.max_aggregate_residual_candidate_work = 100_000;
    limits.max_aggregate_residual_source_term_work = 1_000_000;
    limits.max_aggregate_prospective_classification_work = 1_000_000;
    limits.max_aggregate_merge_request_work = 2_000_000;
    limits.max_retained_iteration_records = 8;
    limits.max_exact_lift_attempts = 1;
    limits
}

fn probes(
    limits: ProbeLocalSchedulerLimits,
) -> Result<Vec<CampaignModularProbe>, Box<dyn std::error::Error>> {
    PROBE_MODULI
        .into_iter()
        .map(|modulus| {
            CampaignModularProbe::try_new(
                modulus,
                BASE_PARAMETERS,
                CHART_COORDINATES,
                limits.campaign,
            )
            .map_err(Into::into)
        })
        .collect()
}

fn bootstrap_stratum(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    sector: Mask,
    target: &IntegralShift,
    limits: ProbeLocalSchedulerLimits,
) -> Result<DecoratedStratum, Box<dyn std::error::Error>> {
    let zero_sources = generator.translate_completed_source_rows(
        completed,
        [target.clone()],
        limits.source_discovery.translation,
    )?;
    let incidence = OrdinarySourceIncidenceIndex::try_new(&zero_sources, limits.source_discovery)?;
    let bootstrap = incidence.try_nominate_target_unit(target, limits.source_discovery)?;
    assert_eq!(bootstrap.raw_incidence_visits(), 90);
    assert_eq!(bootstrap.unique_before_existing_exclusion(), 90);
    assert_eq!(bootstrap.excluded_existing_requests(), 0);
    assert_eq!(bootstrap.requests().len(), 90);

    let selected = generator.translate_selected_completed_source_rows(
        completed,
        bootstrap.requests().iter().cloned(),
        limits.campaign.translated_sources,
    )?;
    let physical_shifts = selected
        .sources()
        .iter()
        .flat_map(|source| source.terms().keys())
        .map(|shift| shift.values().to_vec())
        .collect::<Vec<_>>();
    let domain =
        SectorMonotoneDomain::try_maximal_for_rule(sector, target.values(), &physical_shifts)?;
    Ok(DecoratedStratum::try_guard_blind(
        selected.family_fingerprint(),
        selected.context_fingerprint(),
        domain,
        limits.campaign.stratum,
    )?)
}

fn terminal_telemetry(outcome: &ProbeLocalOutcome) -> BootstrapTerminalTelemetry {
    match outcome {
        ProbeLocalOutcome::Replayed { circuit, .. } => BootstrapTerminalTelemetry::Replayed {
            selected_sources: circuit.source_combination().len(),
            residual_terms: circuit.residual_terms().len(),
            pivot_guards: circuit.pivot_guards().len(),
            nonzero_guards: circuit.nonzero_guards().len(),
            replay_source_terms: circuit.replay().source_terms(),
            replay_exact_operations: circuit.replay().exact_operations(),
        },
        ProbeLocalOutcome::SupportDidNotLift { inconclusive, .. } => {
            BootstrapTerminalTelemetry::SupportDidNotLift {
                selected_sources: inconclusive.selected_source_instances().len(),
                exact_forbidden_rank: inconclusive.exact_forbidden_rank(),
                exact_augmented_rank: inconclusive.exact_augmented_rank(),
            }
        }
        ProbeLocalOutcome::ExactLiftError { .. } => BootstrapTerminalTelemetry::ExactLiftError,
        ProbeLocalOutcome::SampledDual(dual) => {
            let census = dual.census();
            BootstrapTerminalTelemetry::SampledDual {
                obstruction_entries: dual.obstruction().len(),
                structurally_incident_rows: census.structurally_incident_rows(),
                evaluated_unseen_rows: census.evaluated_unseen_rows(),
                evaluated_source_terms: census.evaluated_source_terms(),
                paired_source_terms: census.paired_source_terms(),
            }
        }
        ProbeLocalOutcome::BudgetStop { context, stop } => match stop.cause() {
            ProbeLocalBudgetCause::Outer {
                resource: "probe-local iterations per probe",
                requested,
                limit: 1,
                ..
            } if stop.stage() == ProbeLocalStage::EpochAdmission => {
                BootstrapTerminalTelemetry::OneEpochLimit {
                    requested_iteration: *requested,
                    final_requests: context.requests().map_or(0, |requests| requests.len()),
                }
            }
            _ => BootstrapTerminalTelemetry::OtherBudgetStop {
                stage: stop.stage(),
            },
        },
        ProbeLocalOutcome::Rejected { stage, .. } => {
            BootstrapTerminalTelemetry::Rejected { stage: *stage }
        }
        ProbeLocalOutcome::Stalled { stall, .. } => BootstrapTerminalTelemetry::Stalled {
            nonzero_residual_requests: stall.nonzero_residual_requests(),
        },
    }
}

fn run_k6_probe_local_bootstrap_census()
-> Result<Vec<BootstrapProbeTelemetry>, Box<dyn std::error::Error>> {
    let family = canonical_family()?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let completed = complete_ordinary(&generator);
    assert_eq!(completed.source_row_count(), 9);

    let limits = census_limits();
    let owners = ImmutableOwnerSnapshot::try_from_terminal_authority(
        derive_k6_terminal_authority()?,
        limits.campaign.stratum,
    )?;
    assert_eq!(owners.owner_count(), 35);

    let target = IntegralShift::try_new([0; 6])?;
    let mut census = Vec::new();
    census.try_reserve_exact(EXPECTED_BOOTSTRAP_SECTORS.len() * PROBE_MODULI.len())?;
    for (orbit_ordinal, orbit) in FULL_RANK_ORBITS.into_iter().enumerate() {
        let expected = EXPECTED_BOOTSTRAP_SECTORS[orbit_ordinal];
        assert_eq!(orbit.representative, expected.representative);
        let sector = Mask::try_from_indices(&orbit.representative)?;
        let complete_sector = SectorInteriorDomain::try_new(
            sector.clone(),
            sector.active_bits().iter().map(|&active| {
                if active {
                    InteriorBounds::new(1, i64::MAX)
                } else {
                    InteriorBounds::new(i64::MIN, 0)
                }
            }),
        )?;
        // A bootstrap census is discovery telemetry, not a reason to search a
        // carrier already closed by an installed factorization program.
        if owners.authenticates_same_sector_domain(OrderingPolicy::default(), &complete_sector) {
            continue;
        }
        let stratum = bootstrap_stratum(&generator, &completed, sector, &target, limits)?;
        let report = ProbeLocalObstructionScheduler::try_new(
            &generator,
            &completed,
            target.clone(),
            MaximalStratumAnchor::try_new(stratum, limits.campaign.stratum)?,
            owners.clone(),
            OrderingPolicy::default(),
            probes(limits)?,
            limits,
        )?
        .run()?;
        assert_eq!(report.probes().len(), PROBE_MODULI.len());
        assert_eq!(report.census().epochs(), PROBE_MODULI.len());
        assert_eq!(
            report.census().epoch_request_work(),
            PROBE_MODULI.len() * 90
        );
        assert_eq!(
            report.census().materialized_source_terms(),
            PROBE_MODULI.len() * 918
        );
        assert_eq!(
            report.census().modular_entry_work(),
            PROBE_MODULI.len() * 918
        );
        assert_eq!(
            report.census().merge_request_work(),
            PROBE_MODULI.len() * (90 + expected.added_requests)
        );
        assert_eq!(
            report.census().retained_iteration_records(),
            PROBE_MODULI.len()
        );
        assert_eq!(report.census().exact_lift_attempts(), 0);

        for probe in report.probes() {
            let [iteration] = probe.iterations() else {
                panic!("every K6 census probe must execute exactly one epoch")
            };
            assert_eq!(probe.base_parameters(), BASE_PARAMETERS);
            assert_eq!(probe.chart_coordinates(), CHART_COORDINATES);
            census.push(BootstrapProbeTelemetry {
                orbit_ordinal,
                representative: orbit.representative,
                active_propagators: orbit
                    .representative
                    .into_iter()
                    .filter(|&active| active != 0)
                    .count(),
                probe_ordinal: probe.probe_ordinal(),
                modulus: probe.modulus(),
                request_count: iteration.request_count(),
                physical_rows: iteration.physical_rows(),
                physical_columns: iteration.physical_columns(),
                physical_entries: iteration.physical_entries(),
                allowed_columns: iteration.allowed_columns(),
                forbidden_columns: iteration.forbidden_columns(),
                forbidden_rank: iteration.forbidden_rank(),
                augmented_rank: iteration.augmented_rank(),
                exact_lift_attempted: matches!(
                    probe.outcome(),
                    ProbeLocalOutcome::Replayed { .. }
                        | ProbeLocalOutcome::SupportDidNotLift { .. }
                        | ProbeLocalOutcome::ExactLiftError { .. }
                ),
                disposition: iteration.disposition(),
                outcome: probe.outcome().kind(),
                terminal: terminal_telemetry(probe.outcome()),
            });
        }
    }
    Ok(census)
}

fn run_obstruction_block_width_probe(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    owners: ImmutableOwnerSnapshot,
    representative: [i64; 6],
    configured_width: usize,
) -> Result<ObstructionBlockWidthTelemetry, Box<dyn std::error::Error>> {
    let mut limits = census_limits();
    limits.campaign.modular.max_obstruction_block_directions = configured_width;
    let target = IntegralShift::try_new([0; 6])?;
    let sector = Mask::try_from_indices(&representative)?;
    let stratum = bootstrap_stratum(generator, completed, sector, &target, limits)?;
    let probe = CampaignModularProbe::try_new(
        PROBE_MODULI[0],
        BASE_PARAMETERS,
        CHART_COORDINATES,
        limits.campaign,
    )?;
    let report = ProbeLocalObstructionScheduler::try_new(
        generator,
        completed,
        target,
        MaximalStratumAnchor::try_new(stratum, limits.campaign.stratum)?,
        owners,
        OrderingPolicy::default(),
        [probe],
        limits,
    )?
    .run()?;
    let [probe] = report.probes() else {
        return Err("width comparison must retain exactly one probe".into());
    };
    let [iteration] = probe.iterations() else {
        return Err("width comparison must execute exactly one epoch".into());
    };
    let ProbeLocalOutcome::BudgetStop { stop, .. } = probe.outcome() else {
        return Err("width comparison must end only at its one-epoch bound".into());
    };
    if stop.stage() != ProbeLocalStage::EpochAdmission {
        return Err("width comparison stopped before its one-epoch boundary".into());
    }
    let census = report.census();
    Ok(ObstructionBlockWidthTelemetry {
        representative,
        configured_width,
        disposition: iteration.disposition(),
        final_requests: probe.outcome().final_requests().map_or(0, <[_]>::len),
        residual_candidate_work: census.residual_candidate_work(),
        residual_source_term_work: census.residual_source_term_work(),
        block_candidate_work: census.obstruction_block_candidate_work(),
        block_source_term_work: census.obstruction_block_source_term_work(),
        block_signature_work: census.obstruction_block_signature_work(),
        block_selection_work: census.obstruction_block_selection_work(),
        cache_logical_rows: census.row_cache_logical_rows(),
        cache_logical_terms: census.row_cache_logical_terms(),
        cache_rows: census.row_cache_rows(),
        cache_value_cells: census.row_cache_value_cells(),
        cache_physical_evaluations: census.row_cache_physical_evaluations(),
        cache_hits: census.row_cache_hits(),
        exact_lift_attempts: census.exact_lift_attempts(),
    })
}

#[test]
fn k6_probe_local_bootstrap_census() {
    let census = run_k6_probe_local_bootstrap_census().unwrap();
    eprintln!("K6 probe-local bootstrap census: {census:#?}");
    assert_eq!(
        census.len(),
        EXPECTED_BOOTSTRAP_SECTORS.len() * PROBE_MODULI.len(),
        "exact product preimages leave a discovery fringe in every K6 orbit",
    );
    for (entry_ordinal, entry) in census.iter().enumerate() {
        let expected = EXPECTED_BOOTSTRAP_SECTORS[entry.orbit_ordinal];
        assert_eq!(entry.orbit_ordinal, entry_ordinal / PROBE_MODULI.len());
        assert_eq!(entry.probe_ordinal, entry_ordinal % PROBE_MODULI.len());
        assert_eq!(entry.modulus, PROBE_MODULI[entry.probe_ordinal]);
        assert_eq!(entry.representative, expected.representative);
        assert_eq!(entry.active_propagators, expected.active_propagators);
        assert_eq!(entry.request_count, 90);
        assert_eq!(entry.physical_rows, 90);
        assert_eq!(entry.physical_columns, 253);
        assert_eq!(entry.physical_entries, 918);
        assert_eq!(entry.allowed_columns, expected.allowed_columns);
        assert_eq!(entry.forbidden_columns, expected.forbidden_columns);
        assert_eq!(
            entry.allowed_columns + entry.forbidden_columns + 1,
            entry.physical_columns,
            "the target column belongs to neither partition side"
        );
        let expected_rank = if entry.orbit_ordinal == 4 { 89 } else { 90 };
        assert_eq!(entry.forbidden_rank, expected_rank);
        assert_eq!(entry.augmented_rank, expected_rank);
        assert!(!entry.exact_lift_attempted);
        assert_eq!(
            entry.disposition,
            ProbeLocalIterationDisposition::NoHitAugmented {
                nominated_requests: expected.nominated_requests,
                nonzero_residual_requests: expected.nonzero_residual_requests,
                added_requests: expected.added_requests,
            }
        );
        assert_eq!(entry.outcome, ProbeLocalOutcomeKind::BudgetStop);
        assert_eq!(
            entry.terminal,
            BootstrapTerminalTelemetry::OneEpochLimit {
                requested_iteration: 2,
                final_requests: expected.final_requests,
            }
        );
    }
}

#[test]
fn k6_path_and_star_width_one_vs_four_cache_census_is_non_authoritative() {
    const PATH: [i64; 6] = [0, 0, 1, 0, 1, 1];
    const STAR: [i64; 6] = [0, 0, 1, 1, 0, 1];
    // These are the separately authenticated root-aware degree-one closure
    // baselines: (exact replay, guard-total replay, uncovered boxes). The
    // proposal-only width experiment below cannot change or replace them.
    const FRONTIERS: [([i64; 6], usize, (usize, usize, usize)); 2] =
        [(PATH, 3_763, (9, 4, 10)), (STAR, 3_822, (22, 12, 4))];

    let family = canonical_family().unwrap();
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())
            .unwrap();
    let completed = complete_ordinary(&generator);
    let limits = census_limits();
    let owners = ImmutableOwnerSnapshot::try_from_terminal_authority(
        derive_k6_terminal_authority().unwrap(),
        limits.campaign.stratum,
    )
    .unwrap();

    for (representative, expected_primary_nonzero, closure_baseline) in FRONTIERS {
        let width_one = run_obstruction_block_width_probe(
            &generator,
            &completed,
            owners.clone(),
            representative,
            1,
        )
        .unwrap();
        let width_four = run_obstruction_block_width_probe(
            &generator,
            &completed,
            owners.clone(),
            representative,
            4,
        )
        .unwrap();
        eprintln!(
            "K6 {:?} closure baseline {:?}; obstruction block width 1: {:#?}; width 4: {:#?}",
            representative, closure_baseline, width_one, width_four,
        );

        assert_eq!(width_one.configured_width, 1);
        assert_eq!(width_four.configured_width, 4);
        assert_eq!(width_one.disposition, width_four.disposition);
        assert!(matches!(
            width_one.disposition,
            ProbeLocalIterationDisposition::NoHitAugmented {
                nonzero_residual_requests,
                added_requests: 32,
                ..
            } if nonzero_residual_requests == expected_primary_nonzero
        ));
        assert_eq!(width_one.final_requests, 122);
        assert_eq!(width_four.final_requests, 122);
        assert_eq!(
            width_one.residual_candidate_work,
            width_four.residual_candidate_work,
        );
        assert_eq!(
            width_one.residual_source_term_work,
            width_four.residual_source_term_work,
        );

        for telemetry in [width_one, width_four] {
            assert_eq!(
                telemetry.cache_logical_rows,
                telemetry.residual_candidate_work + telemetry.block_candidate_work,
            );
            assert_eq!(
                telemetry.cache_logical_terms,
                telemetry.residual_source_term_work + telemetry.block_source_term_work,
            );
            assert_eq!(
                telemetry.cache_physical_evaluations + telemetry.cache_hits,
                telemetry.cache_logical_rows,
            );
            assert_eq!(telemetry.cache_rows, telemetry.cache_physical_evaluations);
            assert!(telemetry.cache_hits > 0);
            assert!(telemetry.cache_value_cells > 0);
            assert_eq!(telemetry.exact_lift_attempts, 0);
        }
        assert_eq!(
            width_one.cache_physical_evaluations,
            width_one.residual_candidate_work,
        );
        assert_eq!(width_one.cache_hits, width_one.block_candidate_work);
        assert!(width_four.block_candidate_work >= width_one.block_candidate_work);
        assert!(width_four.block_signature_work > width_one.block_signature_work);
        assert!(width_four.block_selection_work >= width_one.block_selection_work);
    }
}

/// Bounded, deliberately ignored pressure probe for the first dependency
/// sector. The frozen census above is the deterministic regression; this
/// experiment follows up to eight fresh epochs while a bounded frontier-ranked
/// proposal batch grows the frame after each complete obstruction census.
#[test]
#[ignore = "bounded K6 research pressure probe"]
fn k6_first_sector_bounded_proposal_pressure() {
    let family = canonical_family().unwrap();
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())
            .unwrap();
    let completed = complete_ordinary(&generator);
    let limits = bounded_proposal_pressure_limits();
    let owners = ImmutableOwnerSnapshot::try_from_terminal_authority(
        derive_k6_terminal_authority().unwrap(),
        limits.campaign.stratum,
    )
    .unwrap();
    let target = IntegralShift::try_new([0; 6]).unwrap();
    let representative = FULL_RANK_ORBITS[0].representative;
    let sector = Mask::try_from_indices(&representative).unwrap();
    let stratum = bootstrap_stratum(&generator, &completed, sector, &target, limits).unwrap();
    let probe = CampaignModularProbe::try_new(
        PROBE_MODULI[0],
        BASE_PARAMETERS,
        CHART_COORDINATES,
        limits.campaign,
    )
    .unwrap();

    let report = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target,
        MaximalStratumAnchor::try_new(stratum, limits.campaign.stratum).unwrap(),
        owners,
        OrderingPolicy::default(),
        [probe],
        limits,
    )
    .unwrap()
    .run()
    .unwrap();
    eprintln!(
        "K6 bounded-proposal aggregate residual candidates={} source_terms={} prospective_reservation={}",
        report.census().residual_candidate_work(),
        report.census().residual_source_term_work(),
        report.census().prospective_classification_reservation(),
    );
    assert_eq!(report.census().residual_candidate_work(), 63_573);
    assert_eq!(report.census().residual_source_term_work(), 645_012);
    assert_eq!(
        report.census().prospective_classification_reservation(),
        report.census().residual_source_term_work(),
    );
    let [probe] = report.probes() else {
        panic!("the pressure campaign must retain its one declared probe")
    };
    eprintln!(
        "K6 bounded-proposal outcome={:?}, epochs={}",
        probe.outcome().kind(),
        probe.iterations().len()
    );
    for iteration in probe.iterations() {
        eprintln!(
            "epoch={} requests={} rows={} columns={} entries={} ranks={}/{} disposition={:?}",
            iteration.epoch_ordinal(),
            iteration.request_count(),
            iteration.physical_rows(),
            iteration.physical_columns(),
            iteration.physical_entries(),
            iteration.forbidden_rank(),
            iteration.augmented_rank(),
            iteration.disposition(),
        );
    }

    assert_eq!(probe.iterations().len(), 8);
    assert_eq!(probe.iterations()[0].request_count(), 90);
    assert_eq!(probe.iterations()[1].request_count(), 90 + 32);
    for pair in probe.iterations().windows(2) {
        assert!(pair[1].request_count() > pair[0].request_count());
        assert!(pair[1].request_count() <= pair[0].request_count() + 32);
    }
    let final_iteration = &probe.iterations()[7];
    assert_eq!(final_iteration.request_count(), 314);
    assert_eq!(final_iteration.physical_columns(), 413);
    assert_eq!(final_iteration.physical_entries(), 3_055);
    assert_eq!(final_iteration.forbidden_rank(), 279);
    assert_eq!(final_iteration.augmented_rank(), 279);
}
