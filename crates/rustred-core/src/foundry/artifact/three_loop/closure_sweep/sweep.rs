//! One bounded, exact-replay sweep over one or more stable physical frames.

use std::fmt;
use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::completion::frame::OneSidedChartFrame;
use crate::foundry::completion::frame::admission::{
    ExactCircuitOuterExtensionWitness, ExactCircuitOwnerCover, ExactCircuitOwnerCoverError,
    ExactCircuitOwnerInput, ExactCircuitSemanticDag, ExactOwnerCoverStatus,
    exact_content_equal_excluding_modular_telemetry,
};
use crate::foundry::completion::frame::exact::{
    ExactCircuitLift, ExactTargetCircuit, try_lift_exact_circuit,
};
use crate::foundry::completion::frame::modular::ModularTargetQuery;
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, StratumRegistryError, TargetColumnPartition,
};
use crate::identity::{ParametricIbpConfig, ParametricIbpGenerator};
use crate::sector::{Error as SectorError, Mask, OrderingPolicy, SectorMonotoneDomain};

use super::super::canonical_family;
use super::limits::{
    MAX_DEGREE, exact_limits, frame_limits, modular_limits, owner_cover_limits, registry_limits,
    semantic_limits,
};
use super::model::{DegreeSweepTelemetry, SectorSweepTelemetry, SweepCoverTelemetry};

const PRIME: u64 = 1_000_000_007;
const BASE_PARAMETERS: [i64; 1] = [37];
const CHART_COORDINATES: [u64; 6] = [1, 2, 3, 4, 5, 6];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SweepConfigurationError {
    EmptyDegreeSchedule,
    NonCanonicalDegreeSchedule,
    UnsupportedDegree { degree: usize, maximum: usize },
    MixedFrameScope { degree: usize, detail: &'static str },
}

impl fmt::Display for SweepConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDegreeSchedule => {
                formatter.write_str("bounded K6 sweep needs at least one frame degree")
            }
            Self::NonCanonicalDegreeSchedule => formatter.write_str(
                "bounded K6 sweep degrees must be strictly increasing and duplicate-free",
            ),
            Self::UnsupportedDegree { degree, maximum } => write!(
                formatter,
                "bounded K6 sweep degree {degree} is outside its supported range 1..={maximum}"
            ),
            Self::MixedFrameScope { degree, detail } => write!(
                formatter,
                "bounded K6 sweep degree-{degree} frame differs from the shared scope: {detail}"
            ),
        }
    }
}

impl std::error::Error for SweepConfigurationError {}

pub(super) fn sweep_sector(
    sector: Mask,
    degrees: &[usize],
) -> Result<SectorSweepTelemetry, Box<dyn std::error::Error>> {
    validate_degree_schedule(degrees)?;
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

    // The boxes are populated completely before any partition borrows a
    // frame. Their pointees remain stable while hit partitions from both
    // degrees coexist below.
    let mut frames = Vec::new();
    frames.try_reserve_exact(degrees.len())?;
    for &degree in degrees {
        frames.push(Box::new(OneSidedChartFrame::try_new(
            &generator,
            &completed,
            sector.clone(),
            degree,
            frame_limits(),
        )?));
    }
    validate_frame_scope(&frames)?;

    let registry_limits = registry_limits();
    let first = frames
        .first()
        .ok_or(SweepConfigurationError::EmptyDegreeSchedule)?;
    let empty_owners = ImmutableOwnerSnapshot::try_empty(
        first.plan().family_fingerprint(),
        first.plan().context_fingerprint(),
        first.plan().sector().arity(),
        registry_limits,
    )?;

    // Only replayed hits are retained. No-hit and inactive target partitions
    // fall out of scope immediately and cannot masquerade as negative proof.
    let mut admitted_partitions = Vec::new();
    let mut extensions = Vec::new();
    let mut degree_reports = Vec::new();
    degree_reports.try_reserve_exact(frames.len())?;

    for frame in &frames {
        let plan = frame.plan();
        let shifts = plan
            .columns()
            .iter()
            .map(|shift| shift.values())
            .collect::<Vec<_>>();
        let sampled = plan.try_modular_sample(
            &context,
            PRIME,
            &BASE_PARAMETERS,
            &CHART_COORDINATES,
            modular_limits(),
        )?;
        let mut report = DegreeSweepTelemetry {
            degree: frame.degree(),
            frame_offsets: frame.offsets().len(),
            frame_rows: plan.row_count(),
            frame_columns: plan.columns().len(),
            frame_entries: plan.entry_count(),
            partitioned_targets: 0,
            inactive_activation_targets: 0,
            modular_hits: 0,
            modular_no_hits: 0,
            exact_replayed: 0,
            exact_support_did_not_lift: 0,
            exact_content_duplicates: 0,
            admitted_owners: 0,
        };

        for target_column in 0..plan.columns().len() {
            let domain = SectorMonotoneDomain::try_maximal_for_rule(
                sector.clone(),
                plan.columns()[target_column].values(),
                &shifts,
            )?;
            let stratum = DecoratedStratum::try_guard_blind(
                plan.family_fingerprint(),
                plan.context_fingerprint(),
                domain,
                registry_limits,
            )?;
            let partition = match TargetColumnPartition::try_new(
                plan,
                target_column,
                stratum,
                empty_owners.clone(),
                OrderingPolicy::default(),
                registry_limits,
            ) {
                Ok(partition) => partition,
                Err(StratumRegistryError::Sector(SectorError::InactiveLineActivation {
                    ..
                })) => {
                    report.inactive_activation_targets += 1;
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            report.partitioned_targets += 1;

            let query = sampled.query_target(
                partition.target_column(),
                partition.forbidden_columns(),
                modular_limits(),
            )?;
            let ModularTargetQuery::Hit(hit) = query else {
                report.modular_no_hits += 1;
                continue;
            };
            report.modular_hits += 1;

            let circuit = match try_lift_exact_circuit(&context, &hit, &partition, exact_limits())?
            {
                ExactCircuitLift::ModularSupportDidNotLift(_) => {
                    report.exact_support_did_not_lift += 1;
                    continue;
                }
                ExactCircuitLift::Replayed(circuit) => {
                    report.exact_replayed += 1;
                    Arc::new(circuit)
                }
            };

            // The schedule currently supplies one sample per degree, hence at
            // most one circuit here. Keeping the exact-content gate local to
            // this target makes the intended rule explicit for future held-
            // out samples without ever collapsing cross-degree evidence.
            let mut candidates = Vec::new();
            if insert_exact_candidate(&mut candidates, circuit) {
                report.exact_content_duplicates += 1;
            }
            let semantic = Arc::new(ExactCircuitSemanticDag::try_compile(
                &context,
                &partition,
                &candidates,
                semantic_limits(candidates.len()),
            )?);
            let extension = ExactCircuitOuterExtensionWitness::try_prove(&partition, semantic)?;
            let partition_ordinal = admitted_partitions.len();
            admitted_partitions.push(partition);
            extensions.push((partition_ordinal, extension));
            report.admitted_owners += 1;
        }
        degree_reports.push(report);
    }

    let admitted_owners = extensions.len();
    let owner_inputs = extensions
        .into_iter()
        .map(|(partition, extension)| {
            ExactCircuitOwnerInput::new(&admitted_partitions[partition], extension)
        })
        .collect::<Vec<_>>();
    let compiled = ExactCircuitOwnerCover::try_compile(
        &context,
        owner_inputs,
        Vec::<IntegralKey>::new(),
        owner_cover_limits(),
    );
    let cover = cover_telemetry(compiled, sector.arity(), admitted_owners)?;

    Ok(SectorSweepTelemetry {
        sector,
        ordinary_sources,
        degrees: degree_reports.into_boxed_slice(),
        admitted_owners,
        cover,
    })
}

fn validate_degree_schedule(degrees: &[usize]) -> Result<(), SweepConfigurationError> {
    if degrees.is_empty() {
        return Err(SweepConfigurationError::EmptyDegreeSchedule);
    }
    if degrees.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(SweepConfigurationError::NonCanonicalDegreeSchedule);
    }
    if let Some(&degree) = degrees
        .iter()
        .find(|&&degree| degree == 0 || degree > MAX_DEGREE)
    {
        return Err(SweepConfigurationError::UnsupportedDegree {
            degree,
            maximum: MAX_DEGREE,
        });
    }
    Ok(())
}

fn validate_frame_scope(frames: &[Box<OneSidedChartFrame>]) -> Result<(), SweepConfigurationError> {
    let Some(first) = frames.first() else {
        return Err(SweepConfigurationError::EmptyDegreeSchedule);
    };
    for frame in frames.iter().skip(1) {
        let detail = if frame.plan().family_fingerprint() != first.plan().family_fingerprint() {
            Some("family fingerprint differs")
        } else if frame.plan().context_fingerprint() != first.plan().context_fingerprint() {
            Some("coefficient context differs")
        } else if frame.plan().sector() != first.plan().sector() {
            Some("sector differs")
        } else {
            None
        };
        if let Some(detail) = detail {
            return Err(SweepConfigurationError::MixedFrameScope {
                degree: frame.degree(),
                detail,
            });
        }
    }
    Ok(())
}

/// Return `true` only when the candidate was an exact-content duplicate.
fn insert_exact_candidate(
    candidates: &mut Vec<Arc<ExactTargetCircuit>>,
    candidate: Arc<ExactTargetCircuit>,
) -> bool {
    if candidates
        .iter()
        .any(|existing| exact_content_equal_excluding_modular_telemetry(existing, &candidate))
    {
        true
    } else {
        candidates.push(candidate);
        false
    }
}

fn cover_telemetry(
    compiled: Result<ExactCircuitOwnerCover, ExactCircuitOwnerCoverError>,
    arity: usize,
    admitted_owners: usize,
) -> Result<SweepCoverTelemetry, Box<dyn std::error::Error>> {
    match compiled {
        Err(ExactCircuitOwnerCoverError::EmptyOwnerInputs) if admitted_owners == 0 => {
            Ok(SweepCoverTelemetry::NoAdmittedOwners {
                full_orthant_free_dimension: arity,
            })
        }
        Err(error) => Err(error.into()),
        Ok(cover) => {
            let mut free_dimension_histogram = vec![0usize; arity + 1];
            let mut maximum_uncovered_free_dimension = 0usize;
            let mut maximum_uncovered_varying_dimension = 0usize;
            for uncovered in cover.uncovered_partition().boxes() {
                free_dimension_histogram[uncovered.free_dimension()] += 1;
                maximum_uncovered_free_dimension =
                    maximum_uncovered_free_dimension.max(uncovered.free_dimension());
                maximum_uncovered_varying_dimension =
                    maximum_uncovered_varying_dimension.max(uncovered.varying_dimension());
            }
            Ok(SweepCoverTelemetry::Compiled {
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
            })
        }
    }
}

pub(super) fn assert_structural_accounting(report: &SectorSweepTelemetry) {
    assert_eq!(report.ordinary_sources, 9);
    assert_eq!(
        report
            .degrees
            .iter()
            .map(|degree| degree.admitted_owners)
            .sum::<usize>(),
        report.admitted_owners
    );
    for degree in &report.degrees {
        assert_eq!(
            degree.frame_rows,
            degree.frame_offsets * report.ordinary_sources
        );
        assert_eq!(
            degree.partitioned_targets + degree.inactive_activation_targets,
            degree.frame_columns
        );
        assert_eq!(
            degree.modular_hits + degree.modular_no_hits,
            degree.partitioned_targets
        );
        assert_eq!(
            degree.exact_replayed + degree.exact_support_did_not_lift,
            degree.modular_hits
        );
        assert_eq!(
            degree.admitted_owners + degree.exact_content_duplicates,
            degree.exact_replayed
        );
    }
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

pub(super) fn is_closed(report: &SectorSweepTelemetry) -> bool {
    matches!(
        &report.cover,
        SweepCoverTelemetry::Compiled {
            status: ExactOwnerCoverStatus::Closed,
            ..
        }
    )
}
