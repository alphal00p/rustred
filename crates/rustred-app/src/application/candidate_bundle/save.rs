//! Native transport of already-generated candidates, without search or replay.

use rustred::family::IntegralFamily;
use rustred::persistence::{CoefficientTableBuilder, NativeFamilyRecord};
use rustred::solver::SectorSolution;

use crate::application::AppError;

use super::{codec, model::*, policy, preparation};

/// Encode one already-solved sector using the ordinary native candidate format.
///
/// This is trusted generated transport, **not** source replay or a closure
/// certificate. The caller must describe the actual ordinary-source solve in
/// `request`, including its coordinate permutation, numerical depth and finite
/// retention limits. `SectorSolution` retains its rank and finite policy, which
/// are checked, but does not authenticate the remainder of that history.
/// Materializer choices do not change the saved exact formula semantics and
/// are not part of the existing candidate payload; retain them in run receipts.
///
/// The supplied family is checked against the request's parsed source. Only
/// this sector is stored, even if the request root has a larger downset. Missing
/// successor sectors remain uncovered when loaded by the ordinary reducer.
/// No solve, zero census, certification, file write or checkpoint access occurs;
/// `request.checkpoint` and worker/geometry resources have no effect on export.
pub fn encode_generated_candidate_sector<const N: usize>(
    request: &FamilyCandidatesRequest,
    family: &IntegralFamily,
    sector: [bool; N],
    solution: &SectorSolution<N>,
) -> Result<Vec<u8>, AppError> {
    if !(1..=16).contains(&N) || family.denominator_count() != N {
        return Err(AppError::input(
            "candidate export family/sector arity mismatch",
        ));
    }
    policy::validate_request_scope(request)?;
    let root = preparation::root(N, &request.nonpositive_indices)?;
    preparation::validate_permutation(N, request.permutation.as_deref())?;
    if sector
        .iter()
        .zip(&root)
        .any(|(&active, &allowed)| active && !allowed)
    {
        return Err(AppError::input(
            "candidate export sector lies outside request root",
        ));
    }
    let parsed = preparation::family(&request.source, request.input_format)?;
    if parsed.fingerprint() != family.fingerprint() {
        return Err(AppError::input(
            "candidate export source differs from supplied family",
        ));
    }
    encode_sector(request, family, &root, sector, solution)
}

pub(super) fn encode_sector<const N: usize>(
    request: &FamilyCandidatesRequest,
    family: &IntegralFamily,
    root: &[bool],
    sector: [bool; N],
    solution: &SectorSolution<N>,
) -> Result<Vec<u8>, AppError> {
    if solution.max_numerator_rank != request.max_numerator_rank {
        return Err(AppError::input(
            "candidate sector numerator-rank scope differs from its request",
        ));
    }
    if solution.finite_case_policy != request.finite_case_policy {
        return Err(AppError::input(
            "candidate sector finite-case policy differs from its request",
        ));
    }
    let mut coefficients = CoefficientTableBuilder::new(request.bundle_limits.binary_limits());
    let sector = codec::sector_record(sector, solution, &mut coefficients)?;
    let program = program_record(request, family, root, vec![sector]);
    let family =
        NativeFamilyRecord::from_family(family, &mut coefficients).map_err(codec::binary_error)?;
    codec::write_records(
        &program,
        &family,
        &coefficients.finish().map_err(codec::binary_error)?,
        request.bundle_limits,
    )
}

pub(super) fn program_record(
    request: &FamilyCandidatesRequest,
    family: &IntegralFamily,
    root: &[bool],
    sectors: Vec<SectorRecord>,
) -> ProgramRecord {
    ProgramRecord {
        schema: CANDIDATE_BUNDLE_SCHEMA.into(),
        status: STATUS.into(),
        solver_policy: policy::encode_request(request),
        family_source: request.source.clone(),
        input_format: request.input_format.as_str().into(),
        family_fingerprint: family.fingerprint().to_owned(),
        root_sector: root.to_vec(),
        permutation: request.permutation.clone(),
        sectors,
    }
}
