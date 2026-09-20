//! Reuse the native candidate codec, one completed sector at a time. Checkpoint
//! transport never changes candidate authority or performs source replay.

use rustred::family::IntegralFamily;
use rustred::identity::ParametricIbpGenerator;
use rustred::persistence::{CoefficientTableBuilder, NativeFamilyRecord};
use rustred::solver::SectorSolution;
use serde::Serialize;

use super::super::{checkpoint::CheckpointStore, codec, model::*, preparation::Prepared};
use crate::application::AppError;

#[derive(Serialize)]
pub(super) struct Report {
    pub reused_sectors: usize,
    pub newly_solved_sectors: usize,
    pub disk_bytes: usize,
    pub resume_validation_us: u128,
    pub assembly_us: u128,
}

pub(super) fn encode_sector<const N: usize>(
    request: &FamilyCandidatesRequest,
    family: &IntegralFamily,
    root: &[bool],
    sector: [bool; N],
    solution: &SectorSolution<N>,
) -> Result<Vec<u8>, AppError> {
    let mut coefficients = CoefficientTableBuilder::new(request.bundle_limits.binary_limits());
    let sector = codec::sector_record(sector, solution, &mut coefficients)?;
    let program = super::program_record(request, family, root, vec![sector]);
    let family =
        NativeFamilyRecord::from_family(family, &mut coefficients).map_err(codec::binary_error)?;
    codec::write_records(
        &program,
        &family,
        &coefficients.finish().map_err(codec::binary_error)?,
        request.bundle_limits,
    )
}

pub(super) fn assemble<const N: usize>(
    store: &CheckpointStore,
    prepared: &Prepared<N>,
    limits: CandidateBundleLimits,
    coefficients: &mut CoefficientTableBuilder,
) -> Result<Vec<SectorRecord>, AppError> {
    let receipts = store.receipts()?;
    if receipts.len() != prepared.sectors.len() {
        return Err(AppError::internal_invariant(
            "checkpoint assembly has missing sectors",
        ));
    }
    let context = ParametricIbpGenerator::try_new(&prepared.family)
        .map_err(|e| AppError::execution(e.to_string()))?
        .context()
        .clone();
    let mut sectors = Vec::with_capacity(receipts.len());
    let mut budget = codec::CollectionBudget::new(limits, receipts.len())?;
    for (ordinal, receipt) in receipts.into_iter().enumerate() {
        if receipt.ordinal != ordinal {
            return Err(AppError::internal_invariant(
                "checkpoint assembly ordinal gap",
            ));
        }
        // Store read re-admits the actual bytes' structural binding before any
        // native State import. Only one native shard is alive at a time.
        let bytes = store.read(ordinal)?;
        let bundle = codec::read_with_budget(&bytes, limits, Some(&mut budget))?;
        let family = bundle
            .family
            .to_family(
                &bundle.coefficients,
                limits.family_limits(),
                limits.binary_limits(),
            )
            .map_err(codec::binary_error)?;
        if family.fingerprint() != prepared.family.fingerprint() || family.denominator_count() != N
        {
            return Err(AppError::input(
                "checkpoint native family differs from the prepared family",
            ));
        }
        let mut solutions = codec::solutions::<N>(
            &bundle,
            &context,
            prepared.sources.index_variables(),
            limits,
        )?;
        if solutions.len() != 1 || solutions[0].0 != prepared.sectors[ordinal] {
            return Err(AppError::input(
                "checkpoint reconstructed sector differs from manifest",
            ));
        }
        let (sector, solution) = solutions.pop().expect("checked one solution");
        sectors.push(codec::sector_record(sector, &solution, coefficients)?);
    }
    // Caller interns the family only AFTER all sectors. Copying every local
    // dictionary wholesale would change coefficient first-use order and IDs.
    Ok(sectors)
}
