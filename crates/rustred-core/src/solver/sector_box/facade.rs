//! Bridge facade: reduce targets with a fresh sector-box session and
//! return RustRed rules ([`DynamicSolution`]), like `solve_laporta`.
//!
//! The engine is const-generic over the storage arity `N`; this entry
//! point takes `N` explicitly (it must equal the family's denominator
//! count). Callers that dispatch at runtime choose their own arities, e.g.
//! `rustred::dispatch_arity!(arity, [6, 10, 15], solve_box_laporta(...), n
//! => ...)`, instead of compiling the engine for every registered arity.

use std::sync::Arc;

use crate::family::IntegralFamily;
use crate::sector::CutConstraint;

use super::super::SolverError;
use super::super::bridge::DynamicSolution;
use super::{
    BoxLaportaSession, BoxOptions, CensusRouter, IdentityRouter, Lane, ParameterAssignment,
    SectorRouter,
};

/// Sector routing of the facade.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouterChoice {
    /// Zero census only: every nonzero sector is its own representative.
    Identity,
    /// Sector-symmetry census of the whole family (all slots).
    Census,
    /// Sector-symmetry census of the subsectors of one top sector.
    CensusOfTop(Vec<bool>),
}

/// Reduce `targets` (all of them, plus `preferred`, are demanded) onto the
/// sector-box masters and materialise them in `lane` with the `fixed`
/// parameter values.
#[allow(clippy::too_many_arguments)]
pub fn solve_box_laporta<const N: usize>(
    family: Arc<IntegralFamily>,
    cuts: &CutConstraint,
    targets: &[Vec<i16>],
    preferred: &[Vec<i16>],
    router: RouterChoice,
    options: BoxOptions,
    fixed: &ParameterAssignment,
    lane: &Lane,
) -> Result<DynamicSolution, SolverError> {
    if family.denominator_count() != N {
        return Err(SolverError::InvalidInput(format!(
            "arity {N} differs from the family denominator count {}",
            family.denominator_count()
        )));
    }
    let convert = |list: &[Vec<i16>]| -> Result<Vec<[i16; N]>, SolverError> {
        list.iter()
            .map(|values| {
                <[i16; N]>::try_from(values.as_slice()).map_err(|_| {
                    SolverError::InvalidInput(format!(
                        "integral {values:?} does not have {N} indices"
                    ))
                })
            })
            .collect()
    };
    let targets = convert(targets)?;
    let preferred = convert(preferred)?;
    let router: Arc<dyn SectorRouter<N>> = match router {
        RouterChoice::Identity => IdentityRouter::shared(&family)?,
        RouterChoice::Census => Arc::new(CensusRouter::discover(family.clone(), &[true; N])?),
        RouterChoice::CensusOfTop(top) => {
            if top.len() != N {
                return Err(SolverError::InvalidInput(
                    "census top mask has the wrong length".into(),
                ));
            }
            Arc::new(CensusRouter::discover(family.clone(), &top)?)
        }
    };
    let mut session = BoxLaportaSession::<N>::new(family, cuts, router, options)?;
    session.extend(&targets, &preferred)?;
    session.materialize(&targets, fixed, lane)
}
