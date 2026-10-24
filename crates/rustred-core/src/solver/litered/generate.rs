//! In-process `family-candidates`: solve every nonzero sector of a root
//! downset and admit the formulas into the uncertified candidate applier.
//!
//! The configuration is exactly the `rustred family-candidates` default
//! (`SectorConfig::default()` with the family-wide zero census,
//! `SectorSolveOptions::default()`, order `SpiredUncutV1`, sources with
//! capacity preconditioning), so the rules agree with saved bundles of the
//! same family. No bundle is written and nothing is certified.

use std::sync::Arc;

use crate::family::IntegralFamily;
use crate::reduction::ReductionLimits;
use crate::sector::{Mask, OrderingPolicy, zero};
use crate::solver::{
    CandidateFamilyAdmission, CandidateReducer, SectorConfig, SectorExecutor, SectorSolveOptions,
    SourceSystem,
};

use super::LiteRedError;

/// Counters of one in-process candidate generation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CandidateGenerationStatistics {
    pub sectors: usize,
    pub zero_sectors: usize,
    pub rules: usize,
    pub terminals: usize,
    pub seconds: f64,
}

/// Solve the root downset of `family` on `workers` threads and return the
/// candidate applier under `admission`.
pub fn generate_candidate_program<const N: usize>(
    family: &IntegralFamily,
    root: [bool; N],
    workers: usize,
    admission: CandidateFamilyAdmission,
) -> Result<(CandidateReducer<N>, CandidateGenerationStatistics), LiteRedError> {
    let start = std::time::Instant::now();
    let arity = family.denominator_count();
    if arity > N || arity >= usize::BITS as usize - 1 || arity > 24 {
        return Err(LiteRedError::InvalidInput(format!(
            "family arity {arity} unsupported by the in-process generator"
        )));
    }
    let analyzer = zero::Analyzer::try_unrestricted(family)
        .map_err(|e| LiteRedError::InvalidInput(e.to_string()))?;
    let mut zeros: Vec<[bool; N]> = Vec::new();
    let mut certificates = Vec::new();
    let mut sectors: Vec<[bool; N]> = Vec::new();
    for bits in 0..(1usize << arity) {
        let sector: [bool; N] = std::array::from_fn(|i| i < arity && bits & (1 << i) != 0);
        let mask = Mask::try_new(sector[..arity].iter().copied())
            .map_err(|e| LiteRedError::InvalidInput(e.to_string()))?;
        match analyzer
            .analyze(&mask)
            .map_err(|e| LiteRedError::InvalidInput(e.to_string()))?
        {
            zero::Decision::ProvedZero(certificate) => {
                zeros.push(sector);
                certificates.push(certificate);
            }
            zero::Decision::Inconclusive(_) => {
                if sector
                    .iter()
                    .zip(root)
                    .all(|(&active, allowed)| !active || allowed)
                {
                    sectors.push(sector);
                }
            }
            zero::Decision::Excluded(_) => {
                return Err(LiteRedError::InvalidInput(
                    "unrestricted zero analysis excluded a sector".into(),
                ));
            }
        }
    }
    sectors.sort_unstable();
    let sources = SourceSystem::<N>::from_family_with_capacity(family, true)
        .map_err(|e| LiteRedError::InvalidInput(e.to_string()))?;
    let config = SectorConfig {
        zero_sectors: Arc::from(zeros.clone()),
        ..Default::default()
    };
    let executor =
        SectorExecutor::new(workers).map_err(|e| LiteRedError::InvalidInput(e.to_string()))?;
    let records = executor
        .map(
            &sources,
            &sectors,
            &config,
            SectorSolveOptions::default(),
            |done| Ok::<_, LiteRedError>((done.sector, done.solution)),
        )
        .map_err(|e| LiteRedError::InvalidInput(e.to_string()))?;
    let statistics = CandidateGenerationStatistics {
        sectors: records.len(),
        zero_sectors: zeros.len(),
        rules: records.iter().map(|(_, s)| s.rules.len()).sum(),
        terminals: records.iter().map(|(_, s)| s.finite_residuals.len()).sum(),
        seconds: 0.0,
    };
    let reducer = CandidateReducer::try_new_with_admission(
        family,
        root,
        OrderingPolicy::SpiredUncutV1,
        records,
        certificates,
        ReductionLimits::default(),
        None,
        admission,
    )?;
    Ok((
        reducer,
        CandidateGenerationStatistics {
            seconds: start.elapsed().as_secs_f64(),
            ..statistics
        },
    ))
}
