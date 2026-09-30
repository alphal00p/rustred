use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rustred::solver::RoutedCandidateReducer;
use serde_json::{Value, json};

mod routes;

use super::{
    RoutedCampaignRequest,
    input::{Selection, mask},
};
use crate::{
    AppError, CandidateOwnerBundle, CandidateOwnerLoadLimits, load_generated_candidate_owners,
};

pub(super) fn prepare<const N: usize>(
    request: &RoutedCampaignRequest,
    selection: &Selection,
    limits: CandidateOwnerLoadLimits,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<Option<RoutedCandidateReducer<N>>, AppError> {
    prepare_with_fingerprints(request, selection, limits, cancellation, observer, None)
}

/// Bind checkpoint identity to the exact admitted payloads before native import.
pub(super) fn prepare_with_fingerprints<const N: usize>(
    request: &RoutedCampaignRequest,
    selection: &Selection,
    limits: CandidateOwnerLoadLimits,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
    mut fingerprints: Option<&mut dyn FnMut(Vec<String>) -> Result<(), String>>,
) -> Result<Option<RoutedCandidateReducer<N>>, AppError> {
    let mut bytes = Vec::new();
    let mut masks = Vec::new();
    for (ordinal, owner) in selection.owners.iter().enumerate() {
        if cancellation.load(Ordering::Relaxed) {
            return Ok(None);
        }
        observer(
            json!({"event":"preparation", "phase":"owner_read", "completed":ordinal, "total":selection.owners.len()}),
        );
        let path = request.owner_base.join(&owner.path);
        let file =
            File::open(&path).map_err(|e| AppError::input(format!("{}: {e}", path.display())))?;
        if file
            .metadata()
            .map_err(|e| AppError::input(e.to_string()))?
            .len()
            != owner.bytes
        {
            return Err(AppError::input(format!(
                "{}: owner size differs from manifest",
                path.display()
            )));
        }
        let mut payload = Vec::new();
        file.take(owner.bytes + 1)
            .read_to_end(&mut payload)
            .map_err(|e| AppError::input(format!("{}: {e}", path.display())))?;
        if u64::try_from(payload.len()).ok() != Some(owner.bytes) {
            return Err(AppError::input("owner changed during bounded read"));
        }
        bytes.push(payload);
        masks.push(mask(&owner.mask, N)?);
    }
    if let Some(bind) = fingerprints.as_mut() {
        bind(
            bytes
                .iter()
                .map(|payload| blake3::hash(payload).to_hex().to_string())
                .collect(),
        )
        .map_err(AppError::input)?;
    }
    observer(json!({"event":"preparation", "phase":"native_owner_load", "owners":bytes.len()}));
    if cancellation.load(Ordering::Relaxed) {
        return Ok(None);
    }
    let inputs = bytes
        .iter()
        .zip(&masks)
        .map(|(bytes, owner_sector)| CandidateOwnerBundle {
            bytes,
            owner_sector,
        })
        .collect::<Vec<_>>();
    let (family, programs) =
        load_generated_candidate_owners::<N>(&inputs, limits, request.reduction_limits)?;
    if family.fingerprint() != selection.family_fingerprint {
        return Err(AppError::input(
            "loaded family differs from selection fingerprint",
        ));
    }
    if family.external_count() != 0 {
        return Err(AppError::input(
            "this signed loop-map selection format requires a vacuum family",
        ));
    }
    let owner_count = programs.owner_count();
    let terminal_count = programs.terminal_count();
    drop(inputs);
    drop(bytes);
    observer(
        json!({"event":"loaded", "owners":owner_count, "saved_terminals":terminal_count,
        "family_fingerprint":family.fingerprint(), "rank_bound":programs.context().scope().max_numerator_rank,
        "finite_case_policy":format!("{:?}", programs.context().scope().finite_case_policy)}),
    );
    let Some(routes) = routes::prepare::<N>(
        &family,
        &selection.initial_frontier_routes,
        request.workers,
        cancellation,
        observer,
    )?
    else {
        return Ok(None);
    };
    observer(json!({"event":"routes_verified", "routes":routes.len()}));
    if cancellation.load(Ordering::Relaxed) {
        return Ok(None);
    }
    RoutedCandidateReducer::try_new(Arc::new(programs), routes, request.trace_limits)
        .map(Some)
        .map_err(|e| AppError::input(format!("routed programs: {e:?}")))
}
