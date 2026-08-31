use std::sync::Arc;

use crate::foundry::completion::stratum::{
    ImmutableOwnerSnapshot, StratumRegistryError, StratumRegistryLimits,
};
use crate::sector::{Mask, OrderingPolicy};

use super::super::ClosedExactExecutableOwnerCover;
use super::content::try_build_content_id;
#[cfg(test)]
use super::content::{
    try_build_content_id_with_first_cell_guard_for_test,
    try_build_content_id_with_first_circuit_for_test,
};

/// Deterministic bounded identity of one complete published sector layer.
///
/// The digest covers every executable and proof-bearing semantic field while
/// deliberately excluding modular diagnostics and in-memory pointer tokens.
/// It supports reconstruction checks, structural snapshot identities, and
/// canonical ordering. It is not proof authority: joins retain and compare the
/// concrete [`Arc<ClosedSectorLayer>`] chain.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ClosedSectorLayerContentId(Arc<String>);

impl ClosedSectorLayerContentId {
    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub(super) fn from_bounded_digest(value: String) -> Self {
        Self(Arc::new(value))
    }
}

/// Strong in-memory authority for one exact-sector executable rewrite cover.
///
/// The ownership direction is persistent and acyclic: this layer owns a cover
/// whose seal owns its predecessor snapshot; only a later snapshot may retain
/// this layer. No mutation or identity-string rejoining can alter that chain.
#[derive(Debug)]
pub(crate) struct ClosedSectorLayer {
    cover: ClosedExactExecutableOwnerCover,
    content_id: ClosedSectorLayerContentId,
}

impl ClosedSectorLayer {
    /// Consume an already sealed cover and compute its complete bounded
    /// content identity exactly once at the cold publication boundary.
    /// Expensive exact/CAS authentication is not repeated here or later.
    pub(crate) fn try_publish(
        cover: ClosedExactExecutableOwnerCover,
        limits: StratumRegistryLimits,
    ) -> Result<Arc<Self>, StratumRegistryError> {
        let content_id = try_build_content_id(&cover, limits)?;
        Ok(Arc::new(Self { cover, content_id }))
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.cover
            .executable_cover()
            .proof_cover()
            .family_fingerprint()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.cover
            .executable_cover()
            .proof_cover()
            .context_fingerprint()
    }

    pub(crate) const fn sector(&self) -> &Mask {
        self.cover.executable_cover().proof_cover().sector()
    }

    pub(crate) const fn ordering(&self) -> OrderingPolicy {
        self.cover.executable_cover().proof_cover().ordering()
    }

    pub(crate) const fn predecessor_snapshot(&self) -> &ImmutableOwnerSnapshot {
        self.cover.predecessor_snapshot()
    }

    pub(crate) const fn executable_cover(&self) -> &ClosedExactExecutableOwnerCover {
        &self.cover
    }

    pub(crate) const fn content_id(&self) -> &ClosedSectorLayerContentId {
        &self.content_id
    }

    /// Cold audit helper. Production snapshots trust the immutable stored ID
    /// and retained `Arc`; they never traverse and rehash this payload.
    #[cfg(test)]
    pub(crate) fn try_recompute_content_id(
        &self,
        limits: StratumRegistryLimits,
    ) -> Result<ClosedSectorLayerContentId, StratumRegistryError> {
        try_build_content_id(&self.cover, limits)
    }

    /// Encoder-regression seam: replace only the first circuit occurrence in
    /// the canonical stream without mutating the sealed authoritative layer.
    #[cfg(test)]
    pub(crate) fn try_content_id_with_first_circuit_for_test(
        &self,
        circuit: &crate::foundry::completion::frame::exact::ExactTargetCircuit,
        limits: StratumRegistryLimits,
    ) -> Result<ClosedSectorLayerContentId, StratumRegistryError> {
        try_build_content_id_with_first_circuit_for_test(&self.cover, limits, circuit)
    }

    /// Encoder-regression seam: replace only the first executable RuleCell
    /// guard in the canonical stream without mutating the sealed layer.
    #[cfg(test)]
    pub(crate) fn try_content_id_with_first_cell_guard_for_test(
        &self,
        guard: &crate::algebra::IndexedPolynomial,
        limits: StratumRegistryLimits,
    ) -> Result<ClosedSectorLayerContentId, StratumRegistryError> {
        try_build_content_id_with_first_cell_guard_for_test(&self.cover, limits, guard)
    }
}
