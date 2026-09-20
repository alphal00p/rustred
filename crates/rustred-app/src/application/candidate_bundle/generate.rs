use std::time::Instant;

use rustred::family::IntegralFamily;
use rustred::persistence::{CoefficientTableBuilder, NativeFamilyRecord};
use rustred::solver::{SectorConfig, SectorExecutor, SectorSolveOptions};
use serde::Serialize;

use crate::application::AppError;
use crate::application::family_close::progress::{
    FamilyCloseProgress, Observer, emit, generation_stage, sector_mask,
};

use super::{codec, model::*, policy, preparation};

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
    let executor =
        SectorExecutor::new(request.n_cores).map_err(|e| AppError::execution(e.to_string()))?;
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
    let solved = executor
        .map_with_observer(
            &prepared.sources,
            &prepared.sectors,
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
                    ordinal,
                    sector: sector_mask(sector),
                    stage: generation_stage(event),
                    elapsed: started.elapsed(),
                })
            },
            |done| {
                emit(observe, || FamilyCloseProgress::GeneratedSector {
                    ordinal: done.ordinal,
                    sector: sector_mask(done.sector),
                    rules: done.solution.rules.len(),
                    finite_residuals: done.solution.finite_residuals.len(),
                    elapsed: started.elapsed(),
                });
                Ok::<_, std::io::Error>((done.sector, done.solution))
            },
        )
        .map_err(|e| AppError::execution(e.to_string()))?;
    let solved_at = started.elapsed();
    emit(observe, || FamilyCloseProgress::Encoding {
        elapsed: solved_at,
    });
    let mut coefficients = CoefficientTableBuilder::new(request.bundle_limits.binary_limits());
    let bundle = ProgramRecord {
        schema: CANDIDATE_BUNDLE_SCHEMA.into(),
        status: STATUS.into(),
        solver_policy: policy::encode(request.numerical_depth),
        family_source: request.source,
        input_format: request.input_format.as_str().into(),
        family_fingerprint: prepared.family.fingerprint().to_owned(),
        root_sector: root.to_vec(),
        permutation: request.permutation,
        sectors: solved
            .iter()
            .map(|(sector, solution)| codec::sector_record(*sector, solution, &mut coefficients))
            .collect::<Result<Vec<_>, _>>()?,
    };
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
    }
    let report = Report {
        schema: FAMILY_CANDIDATES_SCHEMA,
        status: STATUS,
        workload: "prepare-solve-save; no source replay or closure certification",
        family_fingerprint: prepared.family.fingerprint(),
        arity: N,
        root_sector: root,
        solved_sectors: solved.len(),
        zero_sectors: prepared.zeros.len(),
        generated_rules: solved.iter().map(|(_, s)| s.rules.len()).sum(),
        finite_residuals: solved.iter().map(|(_, s)| s.finite_residuals.len()).sum(),
        workers: request.n_cores,
        exact_backend: request.exact_backend.as_str(),
        numerical_depth: request.numerical_depth,
        bytes: bytes.len(),
        unique_coefficients: coefficient_count,
        coefficient_table_bytes: coefficients.atoms.len(),
        symbolica_state_bytes: coefficients.state.len(),
        preparation_us: prepared_at.as_micros(),
        solve_us: (solved_at - prepared_at).as_micros(),
        bundle_encoding_us: (elapsed - solved_at).as_micros(),
        total_us: elapsed.as_micros(),
    };
    Ok(CandidateBundleResult {
        bundle: bytes,
        report_toml: toml::to_string_pretty(&report)
            .map_err(|e| AppError::serialization(e.to_string()))?,
    })
}
