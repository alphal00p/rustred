use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rustred::solver::RoutedCandidateReducer;
use serde_json::{Value, json};

mod overlays;
mod preferences;
mod routes;

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
        masks.push(mask(&owner.mask, owner.mask.len())?);
    }
    let Some(overlay_payloads) = overlays::read(request, selection, cancellation, observer)? else {
        return Ok(None);
    };
    let Some(preferred_payloads) =
        preferences::read::<N>(request, selection, cancellation, observer)?
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
    if family.external_count() != 0 {
        return Err(AppError::input(
            "this signed loop-map selection format requires a vacuum family",
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
