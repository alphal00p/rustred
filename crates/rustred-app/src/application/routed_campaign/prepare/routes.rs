//! Deterministic, bounded parallel verification of immutable routing witnesses.
//!
//! Only scheduling differs from serial preparation. Symbolica still computes
//! matrix inverses/products, and the existing generic verifier and transport
//! compiler retain every proof check. Owner programs and the family are never
//! cloned per worker; temporary algebra/session state is local to each call.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rustred::campaign::ParallelExecution;
use rustred::family::IntegralFamily;
use rustred::sector::symmetry::{
    self, CoefficientMatrix, MomentumMap, SavedValueTable, SavedValueTableBuilder,
    SavedVerifiedMap, integral_transport,
};
use rustred::solver::CandidateOwnerRoute;
use serde_json::{Value, json};
use symbolica::prelude::{Matrix, Q, Rational};

use super::super::input::{Route, mask};
use crate::AppError;

pub(super) fn prepare<const N: usize>(
    family: &Arc<IntegralFamily>,
    routes: &[Route],
    workers: usize,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<Option<Vec<CandidateOwnerRoute>>, AppError> {
    map_batches(
        routes.len(),
        workers,
        cancellation,
        |ordinal| verify::<N>(family, &routes[ordinal]),
        |completed, verified| {
            observer(
                json!({"event":"preparation", "phase":"native_map_verification",
                "completed":completed,"total":routes.len(),"verified":verified,
                "workers":workers}),
            );
        },
    )
}

/// Recorded verified maps of a selection's transport routes, in selection
/// order (the payload of a saved route preparation add-on).
#[derive(Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub(in crate::application) struct SavedRoutes {
    pub(in crate::application) values: SavedValueTable,
    /// `(selection ordinal, verified map)` of every route that requires a transport.
    pub(in crate::application) routes: Vec<(u32, SavedVerifiedMap)>,
}

/// Record the verified maps of `prepared`, the routes [`prepare`] returned
/// for the same selection, keyed by source sector.
pub(super) fn capture<'a, const N: usize>(
    routes: &[Route],
    prepared: impl Fn(&[bool; N]) -> Option<&'a CandidateOwnerRoute>,
    denominators: usize,
) -> Result<SavedRoutes, AppError> {
    let mut table = SavedValueTableBuilder::new();
    let mut saved = Vec::new();
    for (ordinal, route) in routes.iter().enumerate() {
        if !route.requires_transport {
            continue;
        }
        let source = mask(&route.source_mask, denominators)?;
        let key = std::array::from_fn(|axis| source.active_bits().get(axis) == Some(&true));
        let prepared = prepared(&key)
            .ok_or_else(|| AppError::input("a selected transport route was not prepared"))?;
        if prepared.owner_sector != mask(&route.owner_mask, denominators)? {
            return Err(AppError::input(
                "prepared route owner differs from the selection",
            ));
        }
        let ordinal =
            u32::try_from(ordinal).map_err(|_| AppError::limit("route ordinal exceeds u32"))?;
        let map = SavedVerifiedMap::capture(prepared.transport.verified_map(), &mut table)
            .map_err(|e| AppError::input(format!("route {ordinal}: {e}")))?;
        saved.push((ordinal, map));
    }
    Ok(SavedRoutes {
        values: table.finish(),
        routes: saved,
    })
}

/// Rebuild the routes [`prepare`] would return from recorded verified maps:
/// the same order, owners and transports, without exact map replay. The
/// transports are compiled as in [`prepare`] (cheap admission checks only).
pub(super) fn restore<const N: usize>(
    family: &Arc<IntegralFamily>,
    routes: &[Route],
    saved: &SavedRoutes,
    workers: usize,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<Option<Vec<CandidateOwnerRoute>>, AppError> {
    let loops = family.loop_count();
    let mut expected = Vec::new();
    for (ordinal, route) in routes.iter().enumerate() {
        if route.source_to_representative.len() != loops
            || route.owner_to_representative.len() != loops
        {
            return Err(AppError::input(
                "route matrix dimensions differ from native family",
            ));
        }
        if route.requires_transport {
            expected.push(ordinal);
        }
    }
    if saved.routes.len() != expected.len()
        || saved
            .routes
            .iter()
            .zip(&expected)
            .any(|((saved, _), expected)| *saved as usize != *expected)
    {
        return Err(AppError::input(
            "saved routes differ from the selection's transport routes",
        ));
    }
    let values = saved
        .values
        .materialize(family.coefficient_context())
        .map_err(|e| AppError::input(format!("saved route values: {e}")))?;
    map_batches(
        saved.routes.len(),
        workers,
        cancellation,
        |index| {
            let (ordinal, map) = &saved.routes[index];
            let route = &routes[*ordinal as usize];
            let source = mask(&route.source_mask, family.denominator_count())?;
            let target = mask(&route.owner_mask, family.denominator_count())?;
            let verified = map
                .restore(family, family, &values)
                .map_err(|e| AppError::input(format!("saved route {ordinal}: {e}")))?;
            let transport = integral_transport::compile(
                family,
                Arc::clone(family),
                Arc::new(verified),
                source,
                target.clone(),
                Default::default(),
            )
            .map_err(|e| AppError::input(format!("native integral transport: {e:?}")))?;
            Ok(Some(CandidateOwnerRoute {
                owner_sector: target,
                transport: Arc::new(transport),
            }))
        },
        |completed, restored| {
            observer(json!({"event":"preparation", "phase":"saved_route_restore",
                "completed":completed,"total":saved.routes.len(),"verified":restored,
                "workers":workers}));
        },
    )
}

/// The caller performs no algebra while the private pool computes a batch.
/// Results (including errors) are consumed in original order only after every
/// callback in the batch has joined. Cancellation discards all prepared routes;
/// it never returns a partial reducer or detaches an outstanding operation.
/// Cancellation takes precedence over errors observed in that batch; without
/// cancellation the lowest input ordinal wins regardless of completion order.
fn map_batches<T: Send>(
    count: usize,
    workers: usize,
    cancellation: &AtomicBool,
    operation: impl Fn(usize) -> Result<Option<T>, AppError> + Send + Sync,
    progress: impl Fn(usize, usize),
) -> Result<Option<Vec<T>>, AppError> {
    ParallelExecution::preflight_requested_core_budget(workers)
        .map_err(|error| AppError::input(error.to_string()))?;
    if cancellation.load(Ordering::Relaxed) {
        return Ok(None);
    }
    let mut prepared = Vec::new();
    if count == 0 {
        return Ok(Some(prepared));
    }
    prepared
        .try_reserve_exact(count)
        .map_err(|_| AppError::limit("prepared route allocation failed"))?;
    // Two jobs per requested core hide ordinary per-route cost variation while
    // keeping cancellation/progress boundaries proportional to this budget.
    // Width one is inline and retains one-route-at-a-time serial behavior.
    let batch_size = if workers == 1 {
        1
    } else {
        workers.saturating_mul(2)
    }
    .min(count);
    let execution = ParallelExecution::try_new(workers.min(count), batch_size)
        .map_err(|error| AppError::input(error.to_string()))?;
    for start in (0..count).step_by(batch_size) {
        if cancellation.load(Ordering::Relaxed) {
            return Ok(None);
        }
        progress(start, prepared.len());
        if cancellation.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let size = batch_size.min(count - start);
        let results = execution
            .map_ordered(size, |offset| {
                if cancellation.load(Ordering::Relaxed) {
                    None
                } else {
                    Some(operation(start + offset))
                }
            })
            .map_err(|error| AppError::execution(error.to_string()))?;
        if cancellation.load(Ordering::Relaxed) {
            return Ok(None);
        }
        for result in results {
            match result {
                Some(Ok(Some(route))) => prepared.push(route),
                Some(Ok(None)) => {}
                Some(Err(error)) => return Err(error),
                None => return Ok(None),
            }
        }
    }
    progress(count, prepared.len());
    if cancellation.load(Ordering::Relaxed) {
        Ok(None)
    } else {
        Ok(Some(prepared))
    }
}

fn matrix(
    rows: &[Vec<String>],
) -> Result<Matrix<symbolica::domains::rational::RationalField>, AppError> {
    let values = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|x| x.parse::<i64>().map(Rational::from))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::input(e.to_string()))?;
    Matrix::from_nested_vec(values, Q).map_err(|e| AppError::input(format!("native matrix: {e:?}")))
}

fn verify<const N: usize>(
    family: &Arc<IntegralFamily>,
    route: &Route,
) -> Result<Option<CandidateOwnerRoute>, AppError> {
    let loops = family.loop_count();
    // Preserve the serial validation order, including non-transport records.
    if route.source_to_representative.len() != loops || route.owner_to_representative.len() != loops
    {
        return Err(AppError::input(
            "route matrix dimensions differ from native family",
        ));
    }
    if !route.requires_transport {
        return Ok(None);
    }
    let source = mask(&route.source_mask, family.denominator_count())?;
    let target = mask(&route.owner_mask, family.denominator_count())?;
    let inverse = matrix(&route.owner_to_representative)?
        .inv()
        .map_err(|e| AppError::input(format!("native route inverse: {e:?}")))?;
    let composed = &matrix(&route.source_to_representative)? * &inverse;
    let context = family.coefficient_context();
    let mut entries = Vec::new();
    for i in 0..loops {
        for j in 0..loops {
            let value = composed[(i as u32, j as u32)]
                .to_string()
                .parse::<i64>()
                .map_err(|_| {
                    AppError::input("composed witness must remain an integral loop map")
                })?;
            entries.push(context.integer(value));
        }
    }
    let map_error = |e| AppError::input(format!("native map: {e:?}"));
    let momentum = MomentumMap::new(
        CoefficientMatrix::try_new(loops, loops, entries).map_err(map_error)?,
        CoefficientMatrix::try_new(loops, 0, []).map_err(map_error)?,
        CoefficientMatrix::try_new(0, 0, []).map_err(map_error)?,
    );
    let verified = symmetry::verify(family, family, momentum, Default::default())
        .map_err(|e| AppError::input(format!("native map verification: {e:?}")))?;
    let transport = integral_transport::compile(
        family,
        Arc::clone(family),
        Arc::new(verified),
        source,
        target.clone(),
        Default::default(),
    )
    .map_err(|e| AppError::input(format!("native integral transport: {e:?}")))?;
    Ok(Some(CandidateOwnerRoute {
        owner_sector: target,
        transport: Arc::new(transport),
    }))
}

#[cfg(test)]
mod tests;
