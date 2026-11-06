//! Owner-class keys and ids (types: M2-I0 rr-iface; classify and route: M2-T1 rr-class).
use std::sync::Arc;

use crate::sector::symmetry;

use super::census::SignedAffineLoopMap;
use super::error::PError;
use super::presentation::Presentation;
use super::{OwnerClassId, P_CLASS_KEY_VERSION, PresentationId};

/// Marked-matroid class of a (presentation, support) pair: own deterministic
/// bytes, never Symbolica's native encoding (plan M2 section 3.1).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OwnerClassKey {
    pub loops: u8,
    pub lines: u8,
    pub eps_lines: u8,
    pub canonical: Arc<[u8]>,
}

impl OwnerClassKey {
    /// The low 64 bits (first eight digest bytes, little endian) of
    /// `blake3(P_CLASS_KEY_VERSION LE || canonical)`. A name, not a proof:
    /// distinct bytes under one id are `PError::MalformedLibrary`.
    pub fn id(&self) -> OwnerClassId {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&P_CLASS_KEY_VERSION.to_le_bytes());
        hasher.update(&self.canonical);
        let digest = hasher.finalize();
        let (low, _) = digest
            .as_bytes()
            .split_first_chunk::<8>()
            .expect("a blake3 digest has 32 bytes");
        OwnerClassId(u64::from_le_bytes(*low))
    }

    /// Display form `L3T08-<16 hex>`.
    pub fn label(&self) -> String {
        format!("L{}T{:02}-{:016x}", self.loops, self.lines, self.id().0)
    }
}

/// Resource policy of classification and route verification.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClassLimits {
    pub symmetry: symmetry::Limits,
}

/// One labelled sector with its class key. `support` = `raw` plus the forced
/// eps axes of the presentation.
#[derive(Clone, Debug)]
pub struct ClassedSector {
    pub presentation: PresentationId,
    pub raw: u64,
    pub support: u64,
    pub key: OwnerClassKey,
}

/// The class of `raw` in `p`; `None` when the sector has no class geometry.
pub fn classify(
    _p: &Presentation,
    _raw: u64,
    _limits: &ClassLimits,
) -> Result<Option<ClassedSector>, PError> {
    Err(PError::NotImplemented("M2-T1"))
}

/// The verified map from `source` to `owner`: unit Jacobian, `q -> +-q`,
/// forced axes onto equal-shift axes.
pub fn route(
    _source: &ClassedSector,
    _owner: &ClassedSector,
    _limits: &ClassLimits,
) -> Result<Option<SignedAffineLoopMap>, PError> {
    Err(PError::NotImplemented("M2-T1"))
}
