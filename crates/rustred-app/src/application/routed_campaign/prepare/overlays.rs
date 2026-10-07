//! Immutable, source-replayed partial rules added after the original owners.
//!
//! Loading is deliberately cold and append-only. Checkpoint binding uses these
//! exact bytes in selection order; neither a path nor a replay receipt replaces
//! their identity. A changed selection requires a fresh walk.

use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rustred::solver::CandidateOwnerPrograms;
use serde_json::{Value, json};

use crate::application::candidate_bundle::{
    CandidateDomainOverlayLoadLimits, CandidateOwnerLoadLimits, load_generated_domain_overlay,
};
use crate::{AppError, RoutedCampaignRequest};

use super::super::input::Selection;

pub(super) struct Payload {
    pub bytes: Vec<u8>,
    pub digest: [u8; 32],
    pub owner_ordinal: usize,
}

/// Selection parsing admits aggregate declared bytes before these allocations.
/// A size check plus bounded read rejects a file that changes while being read.
pub(super) fn read(
    request: &RoutedCampaignRequest,
    selection: &Selection,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<Option<Vec<Payload>>, AppError> {
    let mut payloads = Vec::new();
    for (ordinal, record) in selection.domain_rule_overlays.iter().enumerate() {
        if cancellation.load(Ordering::Relaxed) {
            return Ok(None);
        }
        observer(
            json!({"event":"preparation", "phase":"domain_rule_overlay_read",
            "completed":ordinal, "total":selection.domain_rule_overlays.len()}),
        );
        let path = request.owner_base.join(&record.path);
        let file = File::open(&path)
            .map_err(|error| AppError::input(format!("{}: {error}", path.display())))?;
        if file
            .metadata()
            .map_err(|error| AppError::input(error.to_string()))?
            .len()
            != record.bytes
        {
            return Err(AppError::input(
                "domain-rule overlay size differs from selection",
            ));
        }
        let mut bytes = Vec::new();
        let read_limit = record
            .bytes
            .checked_add(1)
            .ok_or_else(|| AppError::limit("domain-rule overlay byte count overflow"))?;
        file.take(read_limit)
            .read_to_end(&mut bytes)
            .map_err(|error| AppError::input(format!("{}: {error}", path.display())))?;
        if u64::try_from(bytes.len()).ok() != Some(record.bytes) {
            return Err(AppError::input(
                "domain-rule overlay changed during bounded read",
            ));
        }
        let owner_ordinal = selection
            .owners
            .iter()
            .position(|owner| owner.mask == record.owner_mask)
            .ok_or_else(|| AppError::input("domain-rule overlay owner is missing"))?;
        let digest = *blake3::hash(&bytes).as_bytes();
        payloads.push(Payload {
            bytes,
            digest,
            owner_ordinal,
        });
    }
    Ok(Some(payloads))
}

/// Decode, replay exact source equations/guards and prove descent once, then
/// append without changing any original rule ordinal or terminal declaration.
pub(super) fn install<const N: usize>(
    mut programs: Arc<CandidateOwnerPrograms<N>>,
    selection: &Selection,
    payloads: &[Payload],
    base_digests: &[[u8; 32]],
    limits: CandidateOwnerLoadLimits,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<Option<Arc<CandidateOwnerPrograms<N>>>, AppError> {
    let terminal_count = programs.terminal_count();
    for (ordinal, payload) in payloads.iter().enumerate() {
        if cancellation.load(Ordering::Relaxed) {
            return Ok(None);
        }
        observer(
            json!({"event":"preparation", "phase":"domain_rule_overlay_replay",
            "completed":ordinal, "total":payloads.len()}),
        );
        let bits = selection.owners[payload.owner_ordinal]
            .mask
            .bytes()
            .map(|byte| byte == b'1')
            .collect::<Vec<_>>();
        let owner = rustred::storage_array(&bits, false)
            .ok_or_else(|| AppError::input("domain-rule overlay owner arity mismatch"))?;
        let (overlay, replay) = load_generated_domain_overlay(
            &programs,
            &payload.bytes,
            owner,
            base_digests[payload.owner_ordinal],
            CandidateDomainOverlayLoadLimits {
                bundle: limits.bundle,
                ..Default::default()
            },
        )?;
        let rules = overlay.rule_count();
        programs = programs
            .append_residual_free_domain_overlays(vec![overlay], Default::default())
            .map_err(|error| AppError::execution(error.to_string()))?;
        if programs.terminal_count() != terminal_count {
            return Err(AppError::execution(
                "domain-rule overlay changed declared terminals",
            ));
        }
        observer(
            json!({"event":"preparation", "phase":"domain_rule_overlay_installed",
            "completed":ordinal+1, "total":payloads.len(), "rules":rules,
            "replayed_rules":replay.rules.len(), "new_terminals":0}),
        );
    }
    Ok(Some(programs))
}
