//! Bounded exact-owner sweep for one three-loop sector.
//!
//! This is deliberately test-only discovery telemetry. Every retained owner
//! is backed by an exactly replayed physical-frame circuit, a semantic guard
//! DAG, and a cold outer-extension proof. The empty lower-sector snapshot and
//! empty terminal list are intentional: an uncovered finite point is reported
//! as an obstruction and can never become a master by observation.
//!
//! The fixed single modular sample is only a bounded support nomination pass.
//! Its no-hits are inconclusive and confer no negative or closure authority.

use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::completion::CompletionGeometryLimits;
use crate::foundry::completion::frame::admission::{
    ExactCircuitOuterExtensionWitness, ExactCircuitOwnerCover, ExactCircuitOwnerCoverError,
    ExactCircuitOwnerCoverLimits, ExactCircuitOwnerInput, ExactCircuitSemanticDag,
    ExactCircuitSemanticLimits, ExactOwnerCoverStatus,
};
use crate::foundry::completion::frame::exact::{
    ExactCircuitLift, ExactCircuitLimits, try_lift_exact_circuit,
};
use crate::foundry::completion::frame::modular::{ModularKernelLimits, ModularTargetQuery};
use crate::foundry::completion::frame::{PhysicalFrameLimits, PhysicalFramePlan};
use crate::foundry::completion::guard::decision::GuardDecisionDagLimits;
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, StratumRegistryError, StratumRegistryLimits,
    TargetColumnPartition,
};
use crate::identity::{ParametricIbpConfig, ParametricIbpGenerator, TranslatedSourceLimits};
use crate::sector::{Error as SectorError, Mask, OrderingPolicy, SectorMonotoneDomain};

use super::canonical_family;
use super::manifest::FULL_RANK_ORBITS;

mod expected;

use expected::{EXPECTED_FULL_RANK_DEGREE_ONE_SWEEP, assert_expected_sweep};

const DEGREE: usize = 1;
const PRIME: u64 = 1_000_000_007;
const BASE_PARAMETERS: [i64; 1] = [37];
const CHART_COORDINATES: [u64; 6] = [1, 2, 3, 4, 5, 6];
const MAX_COLUMNS: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
struct SectorSweepTelemetry {
    sector: Mask,
    ordinary_sources: usize,
    frame_rows: usize,
    frame_columns: usize,
    frame_entries: usize,
    partitioned_targets: usize,
    inactive_activation_targets: usize,
    modular_hits: usize,
    modular_no_hits: usize,
    exact_replayed: usize,
    exact_support_did_not_lift: usize,
    admitted_owners: usize,
    cover: SweepCoverTelemetry,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SweepCoverTelemetry {
    /// The bounded modular discovery budget nominated no replayed owner. The
    /// cover compiler was called and returned its typed empty-input error.
    NoAdmittedOwners {
        /// With no owner orthant, the entire sector chart remains uncovered.
        full_orthant_free_dimension: usize,
    },
    Compiled {
        guard_total_owners: usize,
        status: ExactOwnerCoverStatus,
        uncovered_boxes: usize,
        uncovered_free_dimension_histogram: Box<[usize]>,
        maximum_uncovered_free_dimension: usize,
        maximum_uncovered_varying_dimension: usize,
        missing_terminal_points: usize,
        guard_incomplete_owners: usize,
    },
}

fn sweep_sector(sector: Mask) -> Result<SectorSweepTelemetry, Box<dyn std::error::Error>> {
    let family = canonical_family()?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let context = generator.context().clone();
    let prepared = generator.prepare_ordinary_ibp()?;
    let ordinary_sources = prepared.len();
    let generated = (0..ordinary_sources)
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    let completed = prepared.complete(generated)?;
    let frame = PhysicalFramePlan::try_new(
        &generator,
        &completed,
        sector.clone(),
        DEGREE,
        frame_limits(),
    )?;
    if frame.columns().len() > MAX_COLUMNS {
        return Err(format!(
            "bounded K6 sweep admitted {} columns, over its hard cap {MAX_COLUMNS}",
            frame.columns().len()
        )
        .into());
    }

    let registry_limits = registry_limits();
    let empty_owners = ImmutableOwnerSnapshot::try_empty(
        frame.family_fingerprint(),
        frame.context_fingerprint(),
        frame.sector().arity(),
        registry_limits,
    )?;
    let shifts = frame
        .columns()
        .iter()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let mut partitions = Vec::new();
    partitions.try_reserve_exact(frame.columns().len())?;
    let mut inactive_activation_targets = 0usize;
    for target_column in 0..frame.columns().len() {
        let domain = SectorMonotoneDomain::try_maximal_for_rule(
            sector.clone(),
            frame.columns()[target_column].values(),
            &shifts,
        )?;
        let stratum = DecoratedStratum::try_guard_blind(
            frame.family_fingerprint(),
            frame.context_fingerprint(),
            domain,
            registry_limits,
        )?;
        let partition = TargetColumnPartition::try_new(
            &frame,
            target_column,
            stratum,
            empty_owners.clone(),
            OrderingPolicy::default(),
            registry_limits,
        );
        match partition {
            Ok(partition) => partitions.push(Some(partition)),
            Err(StratumRegistryError::Sector(SectorError::InactiveLineActivation { .. })) => {
                inactive_activation_targets += 1;
                partitions.push(None);
            }
            Err(error) => return Err(error.into()),
        }
    }

    // This one declared sample is only a deterministic support-discovery
    // budget. A ModularNoHit remains inconclusive; only a Hit which survives
    // exact lift, full physical replay, semantic admission, and outer proof
    // enters the owner cover below.
    let sampled = frame.try_modular_sample(
        &context,
        PRIME,
        &BASE_PARAMETERS,
        &CHART_COORDINATES,
        modular_limits(),
    )?;
    let mut modular_hits = 0usize;
    let mut modular_no_hits = 0usize;
    let mut exact_replayed = 0usize;
    let mut exact_support_did_not_lift = 0usize;
    let mut extensions = Vec::new();
    extensions.try_reserve_exact(partitions.len())?;
    for partition in partitions.iter().flatten() {
        let query = sampled.query_target(
            partition.target_column(),
            partition.forbidden_columns(),
            modular_limits(),
        )?;
        let ModularTargetQuery::Hit(hit) = query else {
            modular_no_hits += 1;
            continue;
        };
        modular_hits += 1;
        match try_lift_exact_circuit(&context, &hit, partition, exact_limits())? {
            ExactCircuitLift::ModularSupportDidNotLift(_) => {
                exact_support_did_not_lift += 1;
            }
            ExactCircuitLift::Replayed(circuit) => {
                exact_replayed += 1;
                let circuit = Arc::new(circuit);
                let semantic = Arc::new(ExactCircuitSemanticDag::try_compile(
                    &context,
                    partition,
                    &[circuit],
                    semantic_limits(),
                )?);
                extensions.push((
                    partition.target_column(),
                    ExactCircuitOuterExtensionWitness::try_prove(partition, semantic)?,
                ));
            }
        }
    }

    let owner_inputs = extensions
        .into_iter()
        .filter_map(|(target_column, extension)| {
            partitions[target_column]
                .as_ref()
                .map(|partition| ExactCircuitOwnerInput::new(partition, extension))
        })
        .collect::<Vec<_>>();
    let compiled = ExactCircuitOwnerCover::try_compile(
        &context,
        owner_inputs,
        Vec::<IntegralKey>::new(),
        owner_cover_limits(),
    );
    let cover = match compiled {
        Err(ExactCircuitOwnerCoverError::EmptyOwnerInputs) if exact_replayed == 0 => {
            SweepCoverTelemetry::NoAdmittedOwners {
                full_orthant_free_dimension: sector.arity(),
            }
        }
        Err(error) => return Err(error.into()),
        Ok(cover) => {
            let mut free_dimension_histogram = vec![0usize; sector.arity() + 1];
            let mut maximum_uncovered_free_dimension = 0usize;
            let mut maximum_uncovered_varying_dimension = 0usize;
            for uncovered in cover.uncovered_partition().boxes() {
                free_dimension_histogram[uncovered.free_dimension()] += 1;
                maximum_uncovered_free_dimension =
                    maximum_uncovered_free_dimension.max(uncovered.free_dimension());
                maximum_uncovered_varying_dimension =
                    maximum_uncovered_varying_dimension.max(uncovered.varying_dimension());
            }
            SweepCoverTelemetry::Compiled {
                guard_total_owners: cover
                    .owners()
                    .iter()
                    .filter(|owner| owner.is_guard_total())
                    .count(),
                status: cover.status(),
                uncovered_boxes: cover.uncovered_partition().boxes().len(),
                uncovered_free_dimension_histogram: free_dimension_histogram.into_boxed_slice(),
                maximum_uncovered_free_dimension,
                maximum_uncovered_varying_dimension,
                missing_terminal_points: cover.missing_terminals().len(),
                guard_incomplete_owners: cover.guard_incomplete_owners().len(),
            }
        }
    };

    Ok(SectorSweepTelemetry {
        sector,
        ordinary_sources,
        frame_rows: frame.row_count(),
        frame_columns: frame.columns().len(),
        frame_entries: frame.entry_count(),
        partitioned_targets: partitions.iter().flatten().count(),
        inactive_activation_targets,
        modular_hits,
        modular_no_hits,
        exact_replayed,
        exact_support_did_not_lift,
        admitted_owners: exact_replayed,
        cover,
    })
}

fn frame_limits() -> PhysicalFrameLimits {
    PhysicalFrameLimits {
        translated_sources: TranslatedSourceLimits {
            max_requested_offsets: 7,
            max_translated_sources: 63,
            max_translated_term_entries: 1_024,
            max_translated_condition_entries: 1_024,
            max_retained_condition_source_entries: 4_096,
            max_retained_index_coordinate_cells: 8_192,
            ..TranslatedSourceLimits::default()
        },
        max_arity: 6,
        max_degree: DEGREE,
        max_offsets: 7,
        max_offset_coordinate_cells: 42,
        max_source_instances: 63,
        max_physical_columns: MAX_COLUMNS,
        max_physical_column_coordinate_cells: MAX_COLUMNS * 6,
        max_physical_entries: 1_024,
        max_csr_row_offsets: 64,
    }
}

fn registry_limits() -> StratumRegistryLimits {
    StratumRegistryLimits {
        max_guard_branches: 0,
        max_guard_identity_bytes: 0,
        max_stratum_identity_bytes: 16_384,
        max_owner_regions: 0,
        max_owner_coordinate_cells: 0,
        max_owner_identity_bytes: 16_384,
        max_physical_columns: MAX_COLUMNS,
        max_column_coordinate_cells: MAX_COLUMNS * 6,
        max_target_sector_cells: 65_536,
        max_owner_probes: 0,
        max_retained_owner_witnesses: 0,
    }
}

fn modular_limits() -> ModularKernelLimits {
    ModularKernelLimits {
        max_point_coordinates: 7,
        max_matrix_rows: 63,
        max_matrix_columns: MAX_COLUMNS,
        max_source_conditions: 1_024,
        max_structural_entries: 1_024,
        max_retained_entries: 1_024,
        max_csr_row_offsets: 64,
        max_projected_columns: MAX_COLUMNS,
        max_projected_entries: 1_024,
        max_reducer_dense_cells: 63 * MAX_COLUMNS,
        max_reducer_total_fill_entries: 64 * 1_024,
        max_reducer_fill_multiple: 20,
    }
}

fn exact_limits() -> ExactCircuitLimits {
    ExactCircuitLimits {
        max_physical_columns: MAX_COLUMNS,
        max_selected_rows: 63,
        max_projected_physical_columns: MAX_COLUMNS,
        max_augmented_columns: MAX_COLUMNS + 1,
        max_projected_input_nonzero_entries: 1_024,
        max_native_decomposition_nonzero_entries: 64 * 1_024,
        max_pivot_dependency_entries: 64 * 1_024,
        max_source_combination_terms: 63,
        max_replay_source_terms: 1_024,
        max_replay_exact_operations: 1_000_000,
        max_circuit_terms: MAX_COLUMNS,
        max_dependency_owner_witnesses: 0,
        max_guards: 1_024,
        max_guard_origins: 8_192,
        max_condition_source_entries: 8_192,
        ..ExactCircuitLimits::default()
    }
}

fn semantic_limits() -> ExactCircuitSemanticLimits {
    ExactCircuitSemanticLimits {
        max_candidates: 1,
        max_residual_terms: MAX_COLUMNS,
        max_source_contributions: 63,
        max_pivot_guards: 63,
        max_nonzero_guards: 1_024,
        max_guard_origins: 8_192,
        max_condition_sources: 8_192,
        max_condition_source_coordinate_cells: 49_152,
        max_dependency_owners: 0,
        max_guard_coefficient_equations: 8_192,
        max_guard_base_monomial_exponents: 49_152,
        max_guard_generators: 8_192,
        max_guard_identity_bytes: 1_048_576,
        max_modular_sample_point_entries: 7,
        max_modular_diagnostic_entries: 1_024,
        max_exact_polynomials: 16_384,
        max_polynomial_terms: 131_072,
        max_exponent_entries: 1_048_576,
        max_integer_coefficient_bits: 1_048_576,
        guard_dag: GuardDecisionDagLimits {
            max_context_identity_bytes: 1_048_576,
            max_candidates: 1,
            max_unique_atoms: 1_024,
            max_candidate_atom_references: 1_024,
            max_atom_identity_bytes: 1_048_576,
            max_states: 2_048,
            max_state_words: 2_048,
            max_candidate_scans: 2_048,
            max_nodes: 2_048,
            max_edges: 4_096,
            max_pending_work_items: 2_048,
        },
        ..ExactCircuitSemanticLimits::default()
    }
}

fn owner_cover_limits() -> ExactCircuitOwnerCoverLimits {
    ExactCircuitOwnerCoverLimits {
        max_owner_inputs: MAX_COLUMNS,
        max_owner_coordinate_cells: MAX_COLUMNS * 6,
        max_explicit_terminals: 0,
        max_terminal_coordinate_cells: 0,
        max_finite_complement_points: 65_536,
        max_finite_complement_coordinate_cells: 65_536 * 6,
        max_point_owner_probes: 1_048_576,
        geometry: CompletionGeometryLimits {
            max_arity: 6,
            max_requested_generators: MAX_COLUMNS,
            max_requested_generator_coordinate_cells: MAX_COLUMNS * 6,
            max_minimal_generators: MAX_COLUMNS,
            max_requested_boxes: 0,
            max_requested_box_coordinate_cells: 0,
            max_uncovered_boxes: 65_536,
            max_uncovered_box_coordinate_cells: 65_536 * 6,
            max_split_operations: 1_048_576,
        },
        ..ExactCircuitOwnerCoverLimits::default()
    }
}

fn assert_structural_accounting(report: &SectorSweepTelemetry) {
    assert_eq!(report.ordinary_sources, 9);
    assert_eq!(report.frame_rows, 63);
    assert_eq!(report.frame_entries, 630);
    assert_eq!(
        report.partitioned_targets + report.inactive_activation_targets,
        report.frame_columns
    );
    assert_eq!(
        report.modular_hits + report.modular_no_hits,
        report.partitioned_targets
    );
    assert_eq!(
        report.exact_replayed + report.exact_support_did_not_lift,
        report.modular_hits
    );
    assert_eq!(report.admitted_owners, report.exact_replayed);
    match &report.cover {
        SweepCoverTelemetry::NoAdmittedOwners {
            full_orthant_free_dimension,
        } => {
            assert_eq!(report.admitted_owners, 0);
            assert_eq!(*full_orthant_free_dimension, report.sector.arity());
        }
        SweepCoverTelemetry::Compiled {
            guard_total_owners,
            uncovered_boxes,
            uncovered_free_dimension_histogram,
            maximum_uncovered_free_dimension,
            maximum_uncovered_varying_dimension,
            ..
        } => {
            assert!(*guard_total_owners <= report.admitted_owners);
            assert_eq!(
                uncovered_free_dimension_histogram.iter().sum::<usize>(),
                *uncovered_boxes
            );
            assert!(*maximum_uncovered_free_dimension <= report.sector.arity());
            assert!(*maximum_uncovered_varying_dimension <= report.sector.arity());
        }
    }
}

#[test]
fn canonical_s4a_degree_one_reports_an_exact_owner_cover_obstruction() {
    let sector = Mask::try_from_indices(&[0, 1, 1, 1, 1, 0]).unwrap();
    let report = sweep_sector(sector.clone()).unwrap();
    let repeated = sweep_sector(sector).unwrap();
    eprintln!("K6 S4a degree-one exact owner sweep: {report:#?}");
    assert_eq!(report, repeated);
    assert_structural_accounting(&report);
    assert_expected_sweep(&report, &EXPECTED_FULL_RANK_DEGREE_ONE_SWEEP[3]);
    assert!(!matches!(
        report.cover,
        SweepCoverTelemetry::Compiled {
            status: ExactOwnerCoverStatus::Closed,
            ..
        }
    ));
}

#[test]
fn every_full_rank_orbit_has_bounded_exact_owner_sweep_telemetry() {
    assert_eq!(
        FULL_RANK_ORBITS.map(|orbit| orbit.representative),
        EXPECTED_FULL_RANK_DEGREE_ONE_SWEEP.map(|expected| expected.representative),
        "the typed sweep baseline must cover the complete orbit manifest in canonical order"
    );
    for expected in EXPECTED_FULL_RANK_DEGREE_ONE_SWEEP {
        let report =
            sweep_sector(Mask::try_from_indices(&expected.representative).unwrap()).unwrap();
        eprintln!(
            "K6 orbit {:?} degree-one exact owner sweep: {report:#?}",
            expected.representative
        );
        assert_structural_accounting(&report);
        assert_expected_sweep(&report, &expected);
        assert!(!matches!(
            report.cover,
            SweepCoverTelemetry::Compiled {
                status: ExactOwnerCoverStatus::Closed,
                ..
            }
        ));
    }
}
