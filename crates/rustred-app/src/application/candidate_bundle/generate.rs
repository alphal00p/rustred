use std::time::Instant;

use rustred::family::IntegralFamily;
use rustred::persistence::{CoefficientTableBuilder, NativeFamilyRecord};
use rustred::solver::{SectorConfig, SectorExecutionError, SectorExecutor, SectorSolveOptions};
use serde::Serialize;

use crate::application::AppError;
use crate::application::family_close::progress::{
    FamilyCloseProgress, Observer, emit, generation_stage, sector_mask,
};

use super::checkpoint::{CheckpointManifest, CheckpointStore};
use super::{codec, model::*, policy, preparation};

mod checkpoint;

pub fn family_candidates(
    request: FamilyCandidatesRequest,
) -> Result<CandidateBundleResult, AppError> {
    generate_request(request, None)
}

/// Observe generation without enabling source replay or closure certification.
/// Callbacks may overlap on sector workers and must synchronize their state.
/// Only preparation, generation and encoding events are emitted.
pub fn family_candidates_with_progress(
    request: FamilyCandidatesRequest,
    observe: impl Fn(FamilyCloseProgress) + Send + Sync,
) -> Result<CandidateBundleResult, AppError> {
    generate_request(request, Some(&observe))
}

fn generate_request(
    request: FamilyCandidatesRequest,
    observe: Observer<'_>,
) -> Result<CandidateBundleResult, AppError> {
    let started = Instant::now();
    if request.n_cores == 0 {
        return Err(AppError::input(
            "candidate generation n_cores must be positive",
        ));
    }
    let family = preparation::family(&request.source, request.input_format)?;
    let n = family.denominator_count();
    let root = preparation::root(n, &request.nonpositive_indices)?;
    preparation::validate_permutation(n, request.permutation.as_deref())?;
    emit(observe, || FamilyCloseProgress::Preparing {
        arity: n,
        elapsed: started.elapsed(),
    });
    macro_rules! dispatch {
        ($($n:literal),+) => { match n {
            $($n => generate::<$n>(family, request, &root, started, observe),)+
            _ => unreachable!("candidate arity checked"),
        } };
    }
    dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
}

fn generate<const N: usize>(
    family: IntegralFamily,
    request: FamilyCandidatesRequest,
    root: &[bool],
    started: Instant,
    observe: Observer<'_>,
) -> Result<CandidateBundleResult, AppError> {
    let prepared = preparation::prepare::<N>(family, root, request.permutation.as_deref())?;
    // Preserve the existing default timing boundary (pool construction belongs
    // to preparation). Checkpoint-only resume must never construct a pool.
    let default_executor = request
        .checkpoint
        .is_none()
        .then(|| SectorExecutor::new(request.n_cores))
        .transpose()
        .map_err(|e| AppError::execution(e.to_string()))?;
    let prepared_at = started.elapsed();
    emit(observe, || FamilyCloseProgress::Prepared {
        sectors: prepared.sectors.len(),
        zero_sectors: prepared
            .zeros
            .iter()
            .filter(|sector| {
                sector
                    .iter()
                    .zip(prepared.root)
                    .all(|(&active, allowed)| !active || allowed)
            })
            .count(),
        global_zero_sectors: prepared.zeros.len(),
        elapsed: prepared_at,
    });
    let store = request
        .checkpoint
        .as_ref()
        .map(|options| {
            let manifest = CheckpointManifest::for_request(
                &request,
                prepared.family.fingerprint(),
                root,
                prepared.sectors.iter().map(|s| s.to_vec()).collect(),
            )?;
            CheckpointStore::open(options, manifest, request.bundle_limits)
        })
        .transpose()?;
    let pending: Vec<_> = if let Some(store) = &store {
        store
            .pending()?
            .into_iter()
            .map(|(ordinal, sector)| {
                sector
                    .try_into()
                    .map(|sector| (ordinal, sector))
                    .map_err(|_| AppError::internal_invariant("checkpoint pending sector arity"))
            })
            .collect::<Result<_, _>>()?
    } else {
        prepared.sectors.iter().copied().enumerate().collect()
    };
    let reused_sectors = prepared.sectors.len() - pending.len();
    let jobs: Vec<_> = pending.iter().map(|(_, sector)| *sector).collect();
    let admitted_at = if store.is_some() {
        started.elapsed()
    } else {
        prepared_at
    };
    if store.is_some() {
        emit(observe, || FamilyCloseProgress::CheckpointPrepared {
            reused_sectors,
            pending_sectors: jobs.len(),
            elapsed: admitted_at,
        });
    }
    // A fully saved campaign only assembles; it must not initialize workers or
    // execute any sector search. Completion callbacks retain small receipts on
    // disk, not all expanded exact solutions in the executor's result vector.
    let solved = if jobs.is_empty() {
        Vec::new()
    } else {
        let executor = match default_executor {
            Some(executor) => executor,
            None => SectorExecutor::new(request.n_cores)
                .map_err(|e| AppError::execution(e.to_string()))?,
        };
        executor
            .map_with_observer(
                &prepared.sources,
                &jobs,
                &SectorConfig {
                    zero_sectors: prepared.zeros.clone(),
                    permutation: prepared.permutation,
                    symbolic_exact_backend: request.exact_backend.solver_backend(),
                    numerical_exact_backend: request.exact_backend.numerical_backend(),
                    ..Default::default()
                },
                SectorSolveOptions {
                    numerical_depth: request.numerical_depth,
                    ..Default::default()
                },
                |ordinal, sector, event| {
                    emit(observe, || FamilyCloseProgress::Generating {
                        ordinal: pending[ordinal].0,
                        sector: sector_mask(sector),
                        stage: generation_stage(event),
                        elapsed: started.elapsed(),
                    })
                },
                |done| {
                    emit(observe, || FamilyCloseProgress::GeneratedSector {
                        ordinal: pending[done.ordinal].0,
                        sector: sector_mask(done.sector),
                        rules: done.solution.rules.len(),
                        finite_residuals: done.solution.finite_residuals.len(),
                        elapsed: started.elapsed(),
                    });
                    if let Some(store) = &store {
                        let bytes = checkpoint::encode_sector(
                            &request,
                            &prepared.family,
                            root,
                            done.sector,
                            &done.solution,
                        )?;
                        let receipt = store.publish(pending[done.ordinal].0, &bytes)?;
                        emit(observe, || FamilyCloseProgress::CheckpointedSector {
                            ordinal: receipt.ordinal,
                            sector: sector_mask(done.sector),
                            bytes: receipt.bytes,
                            elapsed: started.elapsed(),
                        });
                        Ok::<_, AppError>(None)
                    } else {
                        Ok(Some((done.sector, done.solution)))
                    }
                },
            )
            .map_err(|e| execution_error(e, &pending))?
    };
    let solved_at = started.elapsed();
    emit(observe, || FamilyCloseProgress::Encoding {
        elapsed: solved_at,
    });
    let mut coefficients = CoefficientTableBuilder::new(request.bundle_limits.binary_limits());
    let sectors = if let Some(store) = &store {
        checkpoint::assemble(store, &prepared, request.bundle_limits, &mut coefficients)?
    } else {
        solved
            // Release each expanded exact solution once its compact record and
            // native coefficients are interned. Encounter order is unchanged;
            // keeping all solutions alive until report writing doubles up
            // their storage with the completed native output unnecessarily.
            .into_iter()
            .flatten()
            .map(|(sector, solution)| codec::sector_record(sector, &solution, &mut coefficients))
            .collect::<Result<Vec<_>, _>>()?
    };
    let assembled_at = started.elapsed();
    let solved_sectors = sectors.len();
    let generated_rules = sectors.iter().map(|s| s.rules.len()).sum();
    let finite_residuals = sectors.iter().map(|s| s.finite_residuals.len()).sum();
    let bundle = program_record(&request, &prepared.family, root, sectors);
    let family_record = NativeFamilyRecord::from_family(&prepared.family, &mut coefficients)
        .map_err(codec::binary_error)?;
    let coefficient_count = coefficients.len();
    let coefficients = coefficients.finish().map_err(codec::binary_error)?;
    let bytes = codec::write_records(
        &bundle,
        &family_record,
        &coefficients,
        request.bundle_limits,
    )?;
    let elapsed = started.elapsed();
    emit(observe, || FamilyCloseProgress::Encoded {
        bytes: bytes.len(),
        elapsed,
    });
    #[derive(Serialize)]
    struct Report<'a> {
        schema: &'static str,
        status: &'static str,
        workload: &'static str,
        family_fingerprint: &'a str,
        arity: usize,
        root_sector: &'a [bool],
        solved_sectors: usize,
        zero_sectors: usize,
        generated_rules: usize,
        finite_residuals: usize,
        workers: usize,
        exact_backend: &'static str,
        numerical_depth: u32,
        bytes: usize,
        unique_coefficients: usize,
        coefficient_table_bytes: usize,
        symbolica_state_bytes: usize,
        preparation_us: u128,
        solve_us: u128,
        bundle_encoding_us: u128,
        total_us: u128,
        #[serde(skip_serializing_if = "Option::is_none")]
        checkpoint: Option<checkpoint::Report>,
    }
    let report = Report {
        schema: FAMILY_CANDIDATES_SCHEMA,
        status: STATUS,
        workload: "prepare-solve-save; no source replay or closure certification",
        family_fingerprint: prepared.family.fingerprint(),
        arity: N,
        root_sector: root,
        solved_sectors,
        zero_sectors: prepared.zeros.len(),
        generated_rules,
        finite_residuals,
        workers: request.n_cores,
        exact_backend: request.exact_backend.as_str(),
        numerical_depth: request.numerical_depth,
        bytes: bytes.len(),
        unique_coefficients: coefficient_count,
        coefficient_table_bytes: coefficients.atoms.len(),
        symbolica_state_bytes: coefficients.state.len(),
        preparation_us: prepared_at.as_micros(),
        solve_us: (solved_at - admitted_at).as_micros(),
        bundle_encoding_us: (elapsed - solved_at).as_micros(),
        total_us: elapsed.as_micros(),
        checkpoint: store
            .as_ref()
            .map(|store| {
                Ok::<_, AppError>(checkpoint::Report {
                    reused_sectors,
                    newly_solved_sectors: pending.len(),
                    disk_bytes: store.charged_bytes()?,
                    resume_validation_us: (admitted_at - prepared_at).as_micros(),
                    assembly_us: (assembled_at - solved_at).as_micros(),
                })
            })
            .transpose()?,
    };
    Ok(CandidateBundleResult {
        bundle: bytes,
        report_toml: toml::to_string_pretty(&report)
            .map_err(|e| AppError::serialization(e.to_string()))?,
    })
}

fn execution_error<const N: usize>(
    error: SectorExecutionError<N, AppError>,
    pending: &[(usize, [bool; N])],
) -> AppError {
    let ordinal = pending[error.ordinal()].0;
    let kind = match &error {
        SectorExecutionError::Consume { source, .. } => source.kind(),
        _ => crate::application::AppErrorKind::Execution,
    };
    let error = match error {
        SectorExecutionError::Prepare { sector, source, .. } => SectorExecutionError::Prepare {
            ordinal,
            sector,
            source,
        },
        SectorExecutionError::Solve { sector, source, .. } => SectorExecutionError::Solve {
            ordinal,
            sector,
            source,
        },
        SectorExecutionError::Consume { sector, source, .. } => SectorExecutionError::Consume {
            ordinal,
            sector,
            source,
        },
    };
    AppError::new(kind, error.to_string())
}

fn program_record(
    request: &FamilyCandidatesRequest,
    family: &IntegralFamily,
    root: &[bool],
    sectors: Vec<SectorRecord>,
) -> ProgramRecord {
    ProgramRecord {
        schema: CANDIDATE_BUNDLE_SCHEMA.into(),
        status: STATUS.into(),
        solver_policy: policy::encode(request.numerical_depth),
        family_source: request.source.clone(),
        input_format: request.input_format.as_str().into(),
        family_fingerprint: family.fingerprint().to_owned(),
        root_sector: root.to_vec(),
        permutation: request.permutation.clone(),
        sectors,
    }
}
