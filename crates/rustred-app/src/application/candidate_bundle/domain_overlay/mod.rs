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
