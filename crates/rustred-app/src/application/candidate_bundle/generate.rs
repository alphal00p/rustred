use std::time::Instant;

use rustred::family::IntegralFamily;
use rustred::solver::{SectorConfig, SectorExecutor, SectorSolveOptions};
use serde::Serialize;

use crate::application::AppError;

use super::{codec, model::*, preparation};

pub fn family_candidates(
    request: FamilyCandidatesRequest,
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
    macro_rules! dispatch {
        ($($n:literal),+) => { match n {
            $($n => generate::<$n>(family, request, &root, started),)+
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
) -> Result<CandidateBundleResult, AppError> {
    let prepared = preparation::prepare::<N>(family, root, request.permutation.as_deref())?;
    let executor =
        SectorExecutor::new(request.n_cores).map_err(|e| AppError::execution(e.to_string()))?;
    let prepared_at = started.elapsed();
    let solved = executor
        .map(
            &prepared.sources,
            &prepared.sectors,
            &SectorConfig {
                zero_sectors: prepared.zeros.clone(),
                permutation: prepared.permutation,
                ..Default::default()
            },
            SectorSolveOptions::default(),
            |done| Ok::<_, std::io::Error>((done.sector, done.solution)),
        )
        .map_err(|e| AppError::execution(e.to_string()))?;
    let solved_at = started.elapsed();
    let bundle = Bundle {
        schema: CANDIDATE_BUNDLE_SCHEMA.into(),
        status: STATUS.into(),
        solver_policy: SOLVER_POLICY.into(),
        family_source: request.source,
        input_format: request.input_format.as_str().into(),
        family_fingerprint: prepared.family.fingerprint().to_owned(),
        root_sector: root.to_vec(),
        permutation: request.permutation,
        sectors: solved
            .iter()
            .map(|(sector, solution)| codec::sector_record(*sector, solution))
            .collect(),
    };
    let bytes = codec::write(&bundle, request.bundle_limits)?;
    let elapsed = started.elapsed();
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
        bytes: usize,
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
        bytes: bytes.len(),
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
