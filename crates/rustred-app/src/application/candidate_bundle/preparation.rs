//! Shared zero-sector/source preparation for family-close, saved candidate
//! generation, and later certification. Publication admission stays with the
//! certified callers, not this topology-generic preparation service.

use std::{borrow::Borrow, sync::Arc};

use rayon::prelude::*;
use rustred::family::IntegralFamily;
use rustred::sector::{Mask, zero};
use rustred::solver::SourceSystem;
use symbolica::utils::resources::{ResourceBudget, checkpoint};

use crate::application::input::prepare_input;
use crate::application::lowering::lower_project;
use crate::application::{AppError, InputFormat};

fn zero_error(error: zero::Error) -> AppError {
    AppError::from_native(error, crate::AppErrorKind::Execution)
}

pub(super) fn family(source: &str, format: InputFormat) -> Result<IntegralFamily, AppError> {
    crate::application::validate_ingress(source)?;
    let (_, _, _, lowered) = lower_project(prepare_input(source, format)?)?.into_parts();
    let family = lowered.into_family();
    validate_arity(family.denominator_count())?;
    Ok(family)
}

/// No frontend arity ceiling. Compiled dispatch, the chosen order and checked
/// mask enumeration validate their actual representation requirements later.
pub(super) fn validate_arity(arity: usize) -> Result<(), AppError> {
    if arity == 0 {
        return Err(AppError::input(
            "candidate family must have at least one denominator coordinate",
        ));
    }
    Ok(())
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
    prepare_inner(family, root, permutation, false, false)
}

pub(super) fn prepare_root<const N: usize>(
    family: IntegralFamily,
    root: &[bool],
    permutation: Option<&[usize]>,
) -> Result<Prepared<N>, AppError> {
    prepare_inner(family, root, permutation, true, false)
}

/// Preserve the host's exact family allocation. The owned preparation entry
/// remains unchanged for existing loaders which must return an owned family.
pub(super) fn prepare_shared<const N: usize>(
    family: Arc<IntegralFamily>,
    root: &[bool],
    permutation: Option<&[usize]>,
) -> Result<Prepared<N, Arc<IntegralFamily>>, AppError> {
    prepare_inner(family, root, permutation, false, false)
}

pub(super) fn prepare_shared_root<const N: usize>(
    family: Arc<IntegralFamily>,
    root: &[bool],
    permutation: Option<&[usize]>,
) -> Result<Prepared<N, Arc<IntegralFamily>>, AppError> {
    prepare_inner(family, root, permutation, true, false)
}

pub(super) fn prepare_shared_parallel<const N: usize>(
    family: Arc<IntegralFamily>,
    root: &[bool],
    permutation: Option<&[usize]>,
    root_only: bool,
) -> Result<Prepared<N, Arc<IntegralFamily>>, AppError> {
    prepare_inner(family, root, permutation, root_only, true)
}

fn prepare_inner<const N: usize, F: Borrow<IntegralFamily>>(
    family: F,
    root: &[bool],
    permutation: Option<&[usize]>,
    root_only: bool,
    parallel: bool,
) -> Result<Prepared<N, F>, AppError> {
    let arity = family.borrow().denominator_count();
    if root.len() != arity {
        return Err(AppError::input("candidate root has wrong arity"));
    }
    let root = rustred::storage_array::<_, N>(root, false)
        .ok_or_else(|| AppError::input("candidate root exceeds storage capacity"))?;
    validate_permutation(arity, permutation)?;
    let permutation = permutation.map(|p| std::array::from_fn(|i| p.get(i).copied().unwrap_or(i)));
    let analyzer = zero::Analyzer::try_unrestricted(family.borrow()).map_err(zero_error)?;
    let mut zeros: Vec<[bool; N]> = Vec::new();
    let mut sectors: Vec<[bool; N]> = Vec::new();
    let axes: Vec<_> = (0..arity).filter(|&i| !root_only || root[i]).collect();
    let count = u32::try_from(axes.len())
        .ok()
        .and_then(|bits| 1usize.checked_shl(bits))
        .ok_or_else(|| AppError::limit("zero-sector census mask count overflows platform"))?;
    let analyze = |bits| -> Result<([bool; N], bool), AppError> {
        checkpoint().map_err(AppError::resource)?;
        let mut sector = [false; N];
        for (bit, &axis) in axes.iter().enumerate() {
            sector[axis] = bits & (1usize << bit) != 0;
        }
        let mask = Mask::try_new(sector[..arity].iter().copied())
            .map_err(|e| AppError::execution(e.to_string()))?;
        let zero = match analyzer.analyze(&mask).map_err(zero_error)? {
            zero::Decision::ProvedZero(_) => true,
            zero::Decision::Inconclusive(_) => false,
            zero::Decision::Excluded(_) => {
                return Err(AppError::internal_invariant(
                    "unrestricted zero analysis excluded a sector",
                ));
            }
        };
        Ok::<_, AppError>((sector, zero))
    };
    let mut retain = |decision: Result<([bool; N], bool), AppError>| {
        let (sector, zero) = decision?;
        if zero {
            zeros.push(sector);
        } else if sector
            .iter()
            .zip(root)
            .all(|(&active, allowed)| !active || allowed)
        {
            sectors.push(sector);
        }
        Ok::<_, AppError>(())
    };
    if parallel {
        // An installed coordinator's TLS scopes do not follow Rayon's jobs.
        // Capture at dispatch so reused pools see this invocation's budgets.
        let resources = ResourceBudget::capture_current();
        let decisions: Vec<_> = (0..count)
            .into_par_iter()
            .map(|bits| resources.run_captured(|| analyze(bits)))
            .collect();
        for decision in decisions {
            retain(decision)?;
        }
    } else {
        for bits in 0..count {
            retain(analyze(bits))?;
        }
    }
    drop(analyzer);
    sectors.sort_unstable();
    let sources = SourceSystem::from_family_with_capacity(family.borrow(), true)
        .map_err(|error| AppError::from_native(error, crate::AppErrorKind::Execution))?;
    Ok(Prepared {
        family,
        root,
        permutation,
        zeros: zeros.into(),
        sectors,
        sources,
    })
}
