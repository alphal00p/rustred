use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rustred::solver::RoutedCandidateReducer;
use serde_json::{Value, json};

mod overlays;
mod preferences;
mod routes;

pub(in crate::application) use routes::SavedRoutes;

/// How [`prepare_deferred_with_reader`] obtained its routes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::application) enum RouteRestore {
    /// No saved routes were offered; every route was verified.
    #[default]
    Verified,
    /// Every route was rebuilt from the offered saved routes.
    Restored,
    /// The saved routes did not fit the selection; every route was verified.
    Rejected(String),
}

/// Record the verified maps of a prepared application's routes.
pub(in crate::application) fn capture_saved_routes<const N: usize>(
    selection: &Selection,
    application: &rustred::solver::RoutedCandidateApplication<N>,
) -> Result<SavedRoutes, AppError> {
    let routes = application
        .routes()
        .collect::<std::collections::BTreeMap<_, _>>();
    let denominators = application.family().denominator_count();
    routes::capture::<N>(
        &selection.initial_frontier_routes,
        |key| routes.get(key).copied(),
        denominators,
    )
}

use super::{
    RoutedCampaignRequest,
    input::{Selection, mask},
};
use crate::application::candidate_bundle::{
    load_generated_candidate_owners_with_preference_rule_subsets, validate_domain_overlay_ingress,
};
use crate::{
    AppError, CandidateOwnerBundle, CandidateOwnerLoadLimits, load_generated_candidate_owners,
    load_generated_candidate_owners_with_preferences,
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
pub(in crate::application) fn prepare_with_fingerprints<const N: usize>(
    request: &RoutedCampaignRequest,
    selection: &Selection,
    limits: CandidateOwnerLoadLimits,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
    fingerprints: Option<&mut dyn FnMut(Vec<String>) -> Result<(), String>>,
) -> Result<Option<RoutedCandidateReducer<N>>, AppError> {
    prepare_with_reader(
        request,
        selection,
        limits,
        cancellation,
        observer,
        fingerprints,
        &mut read_payload,
    )
}

struct SelectedPayloads {
    bytes: Vec<Vec<u8>>,
    masks: Vec<rustred::sector::Mask>,
    overlay_payloads: Vec<overlays::Payload>,
    preferred_payloads: Vec<preferences::Payload>,
    owner_digests: Vec<[u8; 32]>,
}

fn read_selected_payloads<const N: usize>(
    request: &RoutedCampaignRequest,
    selection: &Selection,
    _limits: CandidateOwnerLoadLimits,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
    mut fingerprints: Option<&mut dyn FnMut(Vec<String>) -> Result<(), String>>,
    reader: &mut dyn FnMut(&Path, u64) -> Result<Vec<u8>, AppError>,
) -> Result<Option<SelectedPayloads>, AppError> {
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
        let payload = reader(&path, owner.bytes)?;
        if u64::try_from(payload.len()).ok() != Some(owner.bytes) {
            return Err(AppError::input("owner changed during bounded read"));
        }
        bytes.push(payload);
        masks.push(mask(&owner.mask, owner.mask.len())?);
    }
    let Some(overlay_payloads) =
        overlays::read(request, selection, cancellation, observer, reader)?
    else {
        return Ok(None);
    };
    let Some(preferred_payloads) =
        preferences::read::<N>(request, selection, cancellation, observer, reader)?
    else {
        return Ok(None);
    };
    // Hash each immutable input at most once. Default, non-checkpoint loads
    // without repairs retain their previous no-digest path.
    let owner_digests: Vec<[u8; 32]> = if fingerprints.is_some() || !overlay_payloads.is_empty() {
        bytes
            .iter()
            .map(|payload| *blake3::hash(payload).as_bytes())
            .collect()
    } else {
        Vec::new()
    };
    if let Some(bind) = fingerprints.as_mut() {
        bind(
            owner_digests
                .iter()
                .chain(overlay_payloads.iter().map(|payload| &payload.digest))
                .chain(preferred_payloads.iter().map(|payload| &payload.digest))
                .map(|digest| blake3::Hash::from_bytes(*digest).to_hex().to_string())
                .collect(),
        )
        .map_err(AppError::input)?;
    }
    observer(json!({"event":"preparation", "phase":"native_owner_load", "owners":bytes.len()}));
    if cancellation.load(Ordering::Relaxed) {
        return Ok(None);
    }
    Ok(Some(SelectedPayloads {
        bytes,
        masks,
        overlay_payloads,
        preferred_payloads,
        owner_digests,
    }))
}

pub(in crate::application) fn prepare_with_reader<const N: usize>(
    request: &RoutedCampaignRequest,
    selection: &Selection,
    limits: CandidateOwnerLoadLimits,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
    fingerprints: Option<&mut dyn FnMut(Vec<String>) -> Result<(), String>>,
    reader: &mut dyn FnMut(&Path, u64) -> Result<Vec<u8>, AppError>,
) -> Result<Option<RoutedCandidateReducer<N>>, AppError> {
    let Some(SelectedPayloads {
        bytes,
        masks,
        overlay_payloads,
        preferred_payloads,
        owner_digests,
    }) = read_selected_payloads::<N>(
        request,
        selection,
        limits,
        cancellation,
        observer,
        fingerprints,
        reader,
    )?
    else {
        return Ok(None);
    };
    let inputs = bytes
        .iter()
        .zip(&masks)
        .map(|(bytes, owner_sector)| CandidateOwnerBundle {
            bytes,
            owner_sector,
        })
        .collect::<Vec<_>>();
    let preferred_inputs = preferred_payloads
        .iter()
        .map(|payload| CandidateOwnerBundle {
            bytes: &payload.bytes,
            owner_sector: &payload.owner,
        })
        .collect::<Vec<_>>();
    if !overlay_payloads.is_empty() {
        let patches = overlay_payloads
            .iter()
            .map(|payload| payload.bytes.as_slice())
            .collect::<Vec<_>>();
        // Whole-collection admission precedes the first Symbolica state import.
        if preferred_inputs.is_empty() {
            validate_domain_overlay_ingress(&inputs, &patches, limits)?;
        } else {
            let combined = inputs
                .iter()
                .chain(&preferred_inputs)
                .copied()
                .collect::<Vec<_>>();
            validate_domain_overlay_ingress(&combined, &patches, limits)?;
        }
    }
    let (family, programs) = if preferred_inputs.is_empty() {
        load_generated_candidate_owners::<N>(&inputs, limits, request.reduction_limits)?
    } else if selection
        .preferred_owner_programs
        .iter()
        .any(|record| record.rule_ordinals.is_some())
    {
        let subsets = selection
            .preferred_owner_programs
            .iter()
            .filter_map(|record| {
                record.rule_ordinals.as_deref().map(|ordinals| {
                    let bits = record.owner_mask.as_bytes();
                    (std::array::from_fn(|axis| bits[axis] == b'1'), ordinals)
                })
            })
            .collect();
        load_generated_candidate_owners_with_preference_rule_subsets::<N>(
            &inputs,
            &preferred_inputs,
            &subsets,
            limits,
            request.reduction_limits,
        )?
    } else {
        load_generated_candidate_owners_with_preferences::<N>(
            &inputs,
            &preferred_inputs,
            limits,
            request.reduction_limits,
        )?
    };
    if family.fingerprint() != selection.family_fingerprint {
        return Err(AppError::input(
            "loaded family differs from selection fingerprint",
        ));
    }
    drop(inputs);
    drop(preferred_inputs);
    drop(bytes);
    drop(preferred_payloads);
    let Some(programs) = overlays::install(
        Arc::new(programs),
        selection,
        &overlay_payloads,
        &owner_digests,
        limits,
        cancellation,
        observer,
    )?
    else {
        return Ok(None);
    };
    drop(overlay_payloads);
    let owner_count = programs.owner_count();
    let terminal_count = programs.terminal_count();
    let mut loaded = json!({"event":"loaded", "owners":owner_count, "saved_terminals":terminal_count,
        "family_fingerprint":family.fingerprint(), "rank_bound":programs.context().scope().max_numerator_rank,
        "finite_case_policy":format!("{:?}", programs.context().scope().finite_case_policy)});
    if !selection.preferred_owner_programs.is_empty() {
        loaded["preferred_owner_programs"] = selection.preferred_owner_programs.len().into();
        loaded["preferred_residual_policy"] = "defer-to-baseline".into();
    }
    if selection
        .preferred_owner_programs
        .iter()
        .any(|record| record.rule_ordinals.is_some())
    {
        loaded["preferred_rule_subset_policy"] = super::input::PREFERRED_RULE_SUBSET_POLICY.into();
        loaded["preferred_rule_subsets"] = selection
            .preferred_owner_programs
            .iter()
            .filter_map(|record| {
                record.rule_ordinals.as_ref().map(
                    |ordinals| json!({"owner_mask":record.owner_mask,"rule_ordinals":ordinals}),
                )
            })
            .collect::<Vec<_>>()
            .into();
    }
    observer(loaded);
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
    RoutedCandidateReducer::try_new(programs, routes, request.trace_limits)
        .map(Some)
        .map_err(|e| AppError::input(format!("routed programs: {e:?}")))
}

/// Runtime-only path: all immutable bytes are authenticated now, while owner
/// formulas are admitted and prepared once when the application requests them.
pub(in crate::application) fn prepare_deferred_with_reader<const N: usize>(
    family: Arc<rustred::family::IntegralFamily>,
    request: &RoutedCampaignRequest,
    selection: &Selection,
    limits: CandidateOwnerLoadLimits,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
    fingerprints: Option<&mut dyn FnMut(Vec<String>) -> Result<(), String>>,
    reader: &mut dyn FnMut(&Path, u64) -> Result<Vec<u8>, AppError>,
    saved_routes: Option<&SavedRoutes>,
    route_restore: &mut RouteRestore,
) -> Result<Option<rustred::solver::RoutedCandidateApplication<N>>, AppError> {
    let Some(SelectedPayloads {
        bytes,
        masks,
        overlay_payloads,
        preferred_payloads,
        owner_digests,
    }) = read_selected_payloads::<N>(
        request,
        selection,
        limits,
        cancellation,
        observer,
        fingerprints,
        reader,
    )?
    else {
        return Ok(None);
    };
    if family.fingerprint() != selection.family_fingerprint {
        return Err(AppError::input(
            "deferred shared family differs from the owner selection",
        ));
    }
    let inputs = bytes
        .iter()
        .zip(&masks)
        .map(|(bytes, owner_sector)| CandidateOwnerBundle {
            bytes,
            owner_sector,
        })
        .collect::<Vec<_>>();
    let preferences = preferred_payloads
        .iter()
        .map(|payload| CandidateOwnerBundle {
            bytes: &payload.bytes,
            owner_sector: &payload.owner,
        })
        .collect::<Vec<_>>();
    let subsets = selection
        .preferred_owner_programs
        .iter()
        .filter_map(|record| {
            record.rule_ordinals.as_deref().map(|ordinals| {
                let bits = record.owner_mask.as_bytes();
                (
                    std::array::from_fn(|axis| bits.get(axis) == Some(&b'1')),
                    ordinals,
                )
            })
        })
        .collect();
    let overlays = overlay_payloads
        .iter()
        .map(|payload| {
            let bits = selection.owners[payload.owner_ordinal].mask.as_bytes();
            crate::application::candidate_bundle::DeferredOwnerOverlay {
                bytes: payload.bytes.as_slice(),
                owner: std::array::from_fn(|axis| bits.get(axis) == Some(&b'1')),
                baseline_digest: owner_digests[payload.owner_ordinal],
            }
        })
        .collect::<Vec<_>>();
    let programs = crate::application::candidate_bundle::prepare_deferred_candidate_owners::<N>(
        family.clone(),
        &inputs,
        &preferences,
        &subsets,
        &overlays,
        limits,
        request.reduction_limits,
    )?;
    observer(
        json!({"event":"loaded", "owners":programs.owner_count(),"saved_terminals":programs.terminal_count(),
        "family_fingerprint":family.fingerprint(),"owner_preparation":"deferred","prepared_owners":0}),
    );
    drop(inputs);
    drop(preferences);
    drop(overlays);
    drop(bytes);
    drop(preferred_payloads);
    drop(overlay_payloads);
    let restored = match saved_routes {
        Some(saved) => match routes::restore::<N>(
            &family,
            &selection.initial_frontier_routes,
            saved,
            request.workers,
            cancellation,
            observer,
        ) {
            Ok(routes) => {
                *route_restore = RouteRestore::Restored;
                Some(routes)
            }
            Err(error) => {
                observer(json!({"event":"saved_routes_rejected", "reason":error.to_string()}));
                *route_restore = RouteRestore::Rejected(error.to_string());
                None
            }
        },
        None => None,
    };
    let routes = match restored {
        Some(routes) => routes,
        None => routes::prepare::<N>(
            &family,
            &selection.initial_frontier_routes,
            request.workers,
            cancellation,
            observer,
        )?,
    };
    let Some(routes) = routes else {
        return Ok(None);
    };
    observer(json!({"event":"routes_verified", "routes":routes.len()}));
    if cancellation.load(Ordering::Relaxed) {
        return Ok(None);
    }
    rustred::solver::RoutedCandidateApplication::try_new_deferred(
        Arc::new(programs),
        routes,
        request.trace_limits,
    )
    .map(Some)
    .map_err(|error| AppError::input(format!("deferred routed programs: {error}")))
}

fn read_payload(path: &Path, bytes: u64) -> Result<Vec<u8>, AppError> {
    let file = File::open(path).map_err(|e| AppError::input(format!("{}: {e}", path.display())))?;
    if file
        .metadata()
        .map_err(|e| AppError::input(e.to_string()))?
        .len()
        != bytes
    {
        return Err(AppError::input("owner size differs from manifest"));
    }
    let limit = bytes
        .checked_add(1)
        .ok_or_else(|| AppError::limit("owner byte limit overflow"))?;
    let mut payload = Vec::new();
    file.take(limit)
        .read_to_end(&mut payload)
        .map_err(|e| AppError::input(e.to_string()))?;
    if payload.len() as u64 != bytes {
        return Err(AppError::input("owner changed during bounded read"));
    }
    Ok(payload)
}
