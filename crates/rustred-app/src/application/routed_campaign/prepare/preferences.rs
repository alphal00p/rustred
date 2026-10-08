//! Immutable alternative payloads, with explicit baseline-residual deferral.
use std::fs::File;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};

use rustred::sector::Mask;
use serde_json::{Value, json};

use super::super::input::{PreferredResidualPolicy, Selection, mask};
use crate::{AppError, RoutedCampaignRequest};

pub(super) struct Payload {
    pub bytes: Vec<u8>,
    pub digest: [u8; 32],
    pub owner: Mask,
}

/// Declared aggregate sizes have already passed selection admission. Identity
/// uses actual immutable bytes, never optional display/checksum metadata.
pub(super) fn read<const N: usize>(
    request: &RoutedCampaignRequest,
    selection: &Selection,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<Option<Vec<Payload>>, AppError> {
    let mut payloads = Vec::new();
    for (ordinal, record) in selection.preferred_owner_programs.iter().enumerate() {
        if cancellation.load(Ordering::Relaxed) {
            return Ok(None);
        }
        match record.residual_policy {
            PreferredResidualPolicy::DeferToBaseline => {}
        }
        observer(
            json!({"event":"preparation", "phase":"preferred_owner_read",
            "completed":ordinal, "total":selection.preferred_owner_programs.len()}),
        );
        let path = request.owner_base.join(&record.path);
        let file = File::open(&path)
            .map_err(|error| AppError::input(format!("{}: {error}", path.display())))?;
        if file
            .metadata()
            .map_err(|e| AppError::input(e.to_string()))?
            .len()
            != record.bytes
        {
            return Err(AppError::input(
                "preferred owner size differs from selection",
            ));
        }
        let read_limit = record
            .bytes
            .checked_add(1)
            .ok_or_else(|| AppError::limit("preferred owner byte count overflow"))?;
        let mut bytes = Vec::new();
        file.take(read_limit)
            .read_to_end(&mut bytes)
            .map_err(|error| AppError::input(format!("{}: {error}", path.display())))?;
        if u64::try_from(bytes.len()).ok() != Some(record.bytes) {
            return Err(AppError::input(
                "preferred owner changed during bounded read",
            ));
        }
        payloads.push(Payload {
            digest: *blake3::hash(&bytes).as_bytes(),
            bytes,
            owner: mask(&record.owner_mask, selection.physical_arity())?,
        });
    }
    Ok(Some(payloads))
}
