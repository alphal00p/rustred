//! Native rules-only partial overlays, independent of complete sector payloads.
//!
//! Algebra uses the same Symbolica state/coefficient table as other programs.
//! Loading binds a named immutable owner, replays exact sources and proves rule
//! descent, but does not install new terminals or assert recursive closure.

mod codec;
mod ingress;
mod model;

use std::sync::Arc;

use rustred::foundry::artifact::{SourcePortLimits, SourcePortRuleReplayAudit};
use rustred::solver::{BoundOwnerOverlay, CandidateOwnerPrograms, OwnerOverlayLimits};

use super::CandidateBundleLimits;
use crate::AppError;

pub use ingress::validate_domain_overlay_ingress;

/// Runtime boundary admission of frozen overlay frames and native family
/// identity. Equation replay and owner installation remain first-use work.
pub(crate) fn validate_deferred_overlay<const N: usize>(
    bytes: &[u8],
    owner: [bool; N],
    baseline_digest: [u8; 32],
    family: &rustred::family::IntegralFamily,
    limits: CandidateBundleLimits,
) -> Result<(), AppError> {
    use rustred::persistence::{LazyDecodedCoefficientTable, SectionTag};
    let (envelope, record, native_family) = codec::read_structure(bytes, limits)?;
    if rustred::storage_array(&record.owner_sector, false) != Some(owner)
        || record.base_owner_blake3 != baseline_digest
        || record.family_fingerprint != family.fingerprint()
    {
        return Err(AppError::input(
            "deferred overlay owner, base digest or family differs",
        ));
    }
    let table = LazyDecodedCoefficientTable::new(
        Arc::from(
            envelope
                .section(SectionTag::SYMBOLICA_STATE)
                .expect("admitted overlay state"),
        ),
        Arc::from(
            envelope
                .section(SectionTag::COEFFICIENTS)
                .expect("admitted overlay coefficients"),
        ),
        limits.binary_limits(),
    )
    .map_err(super::codec::binary_error)?;
    for rule in &record.rules {
        super::codec::rules::validate_rule_ids(rule, table.len())?;
    }
    if record
        .requested_cases
        .iter()
        .flat_map(|case| &case.equations)
        .any(|&id| id as usize >= table.len())
    {
        return Err(AppError::input(
            "deferred overlay case coefficient ID is outside its table",
        ));
    }
    native_family
        .validate_against_family(
            family,
            &table,
            limits.family_limits(),
            limits.binary_limits(),
        )
        .map_err(super::codec::binary_error)
}

/// Caller-owned cold-load resources, independent of saved search scope.
#[derive(Clone, Copy, Debug, Default)]
pub struct CandidateDomainOverlayLoadLimits {
    pub bundle: CandidateBundleLimits,
    pub overlay: OwnerOverlayLimits,
    pub replay: SourcePortLimits,
}

/// Encode already generated, residual-free partial rules. No search, filesystem
/// access, source replay or closure claim occurs. The base digest must identify
/// the exact immutable owner payload; loaders independently check that binding.
pub fn encode_generated_domain_overlay<const N: usize>(
    programs: &Arc<CandidateOwnerPrograms<N>>,
    overlay: &BoundOwnerOverlay<N>,
    base_owner_blake3: [u8; 32],
    limits: CandidateBundleLimits,
) -> Result<Vec<u8>, AppError> {
    codec::encode(programs, overlay, base_owner_blake3, limits)
}

/// Cold-load a partial rules-only payload and rebind it after independent exact
/// identity, guard and strict-descent validation. This does not append it; callers
/// may atomically append the returned overlays only after all have succeeded.
/// No rule is silently widened to a sector and no residual becomes a terminal.
/// Only import generated native frames from a trusted matching RustRed/Symbolica
/// stack; Symbolica state import is not a hostile-input parser or transaction.
pub fn load_generated_domain_overlay<const N: usize>(
    programs: &Arc<CandidateOwnerPrograms<N>>,
    bytes: &[u8],
    expected_owner: [bool; N],
    expected_base_owner_blake3: [u8; 32],
    limits: CandidateDomainOverlayLoadLimits,
) -> Result<(BoundOwnerOverlay<N>, SourcePortRuleReplayAudit<N>), AppError> {
    codec::load(
        programs,
        bytes,
        expected_owner,
        expected_base_owner_blake3,
        limits,
    )
}

#[cfg(test)]
mod tests;
