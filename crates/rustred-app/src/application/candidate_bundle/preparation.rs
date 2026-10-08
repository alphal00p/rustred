//! Shared zero-sector/source preparation for family-close, saved candidate
//! generation, and later certification. Publication admission stays with the
//! certified callers, not this topology-generic preparation service.

use std::{borrow::Borrow, sync::Arc};

use rustred::family::IntegralFamily;
use rustred::sector::{Mask, zero};
use rustred::solver::SourceSystem;

use crate::application::input::prepare_input;
use crate::application::lowering::lower_project;
use crate::application::{AppError, InputFormat, MAX_INPUT_BYTES};

pub(super) fn family(source: &str, format: InputFormat) -> Result<IntegralFamily, AppError> {
    if source.len() > MAX_INPUT_BYTES {
        return Err(AppError::limit(
            "candidate family source exceeds input limit",
        ));
    }
    let (_, _, _, lowered) = lower_project(prepare_input(source, format)?)?.into_parts();
    let family = lowered.into_family();
    if !(1..=16).contains(&family.denominator_count()) {
        return Err(AppError::input(
            "candidate generation supports denominator arities 1 through 16",
        ));
    }
    Ok(family)
}

pub(super) fn root(arity: usize, nonpositive: &[usize]) -> Result<Vec<bool>, AppError> {
    root_sector(arity, nonpositive, "candidate")
}

pub(in crate::application) fn root_sector(
    arity: usize,
    nonpositive: &[usize],
    operation: &str,
) -> Result<Vec<bool>, AppError> {
    let mut root = vec![true; arity];
    for &axis in nonpositive {
        let slot = root.get_mut(axis).ok_or_else(|| {
            AppError::input(format!(
                "{operation} nonpositive index {axis} lies outside 0..{arity}"
            ))
        })?;
        if !*slot {
            return Err(AppError::input(format!(
                "{operation} nonpositive index {axis} is repeated"
            )));
        }
        *slot = false;
    }
    Ok(root)
}

pub(super) fn validate_permutation(
    arity: usize,
    permutation: Option<&[usize]>,
) -> Result<(), AppError> {
    if let Some(permutation) = permutation {
        let mut sorted = permutation.to_vec();
        sorted.sort_unstable();
        if sorted != (0..arity).collect::<Vec<_>>() {
            return Err(AppError::input(
                "candidate permutation must contain each axis exactly once",
            ));
        }
    }
    Ok(())
}

pub(in crate::application) struct Prepared<const N: usize, F = IntegralFamily> {
    pub family: F,
    pub root: [bool; N],
    pub permutation: Option<[usize; N]>,
    pub zeros: Arc<[[bool; N]]>,
    pub sectors: Vec<[bool; N]>,
    pub sources: SourceSystem<N>,
}

pub(in crate::application) fn prepare<const N: usize>(
    family: IntegralFamily,
    root: &[bool],
    permutation: Option<&[usize]>,
) -> Result<Prepared<N>, AppError> {
    prepare_inner(family, root, permutation)
}

/// Preserve the host's exact family allocation. The owned preparation entry
/// remains unchanged for existing loaders which must return an owned family.
pub(super) fn prepare_shared<const N: usize>(
    family: Arc<IntegralFamily>,
    root: &[bool],
    permutation: Option<&[usize]>,
) -> Result<Prepared<N, Arc<IntegralFamily>>, AppError> {
    prepare_inner(family, root, permutation)
}

fn prepare_inner<const N: usize, F: Borrow<IntegralFamily>>(
    family: F,
    root: &[bool],
    permutation: Option<&[usize]>,
) -> Result<Prepared<N, F>, AppError> {
    let arity = family.borrow().denominator_count();
    if root.len() != arity {
        return Err(AppError::input("candidate root has wrong arity"));
    }
    let root = rustred::storage_array::<_, N>(root, false)
        .ok_or_else(|| AppError::input("candidate root exceeds storage capacity"))?;
    validate_permutation(arity, permutation)?;
    let permutation = permutation.map(|p| std::array::from_fn(|i| p.get(i).copied().unwrap_or(i)));
    let analyzer = zero::Analyzer::try_unrestricted(family.borrow())
        .map_err(|error| AppError::execution(error.to_string()))?;
    let mut zeros = Vec::new();
    let mut sectors = Vec::new();
    for bits in 0..(1usize << arity) {
        let sector = std::array::from_fn(|axis| bits & (1 << axis) != 0);
        let mask = Mask::try_new(sector[..arity].iter().copied())
            .map_err(|e| AppError::execution(e.to_string()))?;
        match analyzer
            .analyze(&mask)
            .map_err(|e| AppError::execution(e.to_string()))?
        {
            zero::Decision::ProvedZero(_) => zeros.push(sector),
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
                return Err(AppError::internal_invariant(
                    "unrestricted zero analysis excluded a sector",
                ));
            }
        }
    }
    drop(analyzer);
    sectors.sort_unstable();
    let sources = SourceSystem::from_family_with_capacity(family.borrow(), true)
        .map_err(|e| AppError::execution(e.to_string()))?;
    Ok(Prepared {
        family,
        root,
        permutation,
        zeros: zeros.into(),
        sectors,
        sources,
    })
}
