//! Aggregate structural admission before importing any base or overlay algebra.

use rustred::persistence::SectionTag;

use super::super::{
    CandidateOwnerBundle, CandidateOwnerLoadLimits,
    load::ingress::{IngressBudget, charge},
};
use super::codec;
use crate::AppError;

/// Admit the combined base-owner and partial-overlay byte, Symbolica-state,
/// coefficient and collection totals before the first native frame is imported.
/// Native validation and exact overlay replay remain the later loaders' job.
/// Zero overlays preserve the original base loader's unchanged admission path.
pub fn validate_domain_overlay_ingress(
    base_inputs: &[CandidateOwnerBundle<'_>],
    patch_bytes: &[&[u8]],
    limits: CandidateOwnerLoadLimits,
) -> Result<(), AppError> {
    if patch_bytes.is_empty() {
        return Ok(());
    }
    if base_inputs.is_empty() {
        return Err(AppError::input(
            "partial overlays require immutable base owners",
        ));
    }
    let records = base_inputs
        .len()
        .checked_add(patch_bytes.len())
        .ok_or_else(|| AppError::limit("combined owner/overlay count overflow"))?;
    let mut budget = IngressBudget::new(limits.bundle, records)?;
    let mut input_bytes = 0;
    let mut state_bytes = 0;
    // Byte admission occurs before structural record decoding/allocation.
    for bytes in base_inputs
        .iter()
        .map(|owner| owner.bytes)
        .chain(patch_bytes.iter().copied())
    {
        charge(
            &mut input_bytes,
            bytes.len(),
            limits.max_total_input_bytes,
            "combined owner/overlay aggregate input-byte budget exceeded",
        )?;
    }
    for input in base_inputs {
        let (envelope, record, _) =
            super::super::codec::read_structure(input.bytes, limits.bundle)?;
        if record.sectors.len() != 1 {
            return Err(AppError::input(
                "partial overlay base must select exactly one stored owner sector",
            ));
        }
        budget.admit_structure(&envelope, &record)?;
        charge(
            &mut state_bytes,
            envelope
                .section(SectionTag::SYMBOLICA_STATE)
                .expect("checked section")
                .len(),
            limits.max_total_symbolica_state_bytes,
            "combined owner/overlay aggregate Symbolica-state byte budget exceeded",
        )?;
    }
    for bytes in patch_bytes {
        let (envelope, record, _) = codec::read_structure(bytes, limits.bundle)?;
        budget.admit_domain_rules(&envelope, &record.requested_cases, &record.rules)?;
        charge(
            &mut state_bytes,
            envelope
                .section(SectionTag::SYMBOLICA_STATE)
                .expect("checked section")
                .len(),
            limits.max_total_symbolica_state_bytes,
            "combined owner/overlay aggregate Symbolica-state byte budget exceeded",
        )?;
    }
    Ok(())
}
