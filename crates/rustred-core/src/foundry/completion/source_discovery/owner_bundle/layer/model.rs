use std::sync::Arc;

use crate::foundry::completion::LatticeBox;
use crate::foundry::completion::stratum::{
    ImmutableOwnerSnapshot, StratumRegistryError, StratumRegistryLimits,
};
use crate::sector::{InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain};

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
    /// Exact power-space image of the finite lattice carrier discharged by
    /// `cover`.  A later immutable snapshot may advertise this rectangle, but
    /// must never widen it to the whole sector.
    proven_domain: SectorInteriorDomain,
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
        let proven_domain = try_carrier_domain(&cover)?;
        let content_id = try_build_content_id(&cover, limits)?;
        Ok(Arc::new(Self {
            cover,
            proven_domain,
            content_id,
        }))
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

    /// Exact bounded sector interior on which this layer's cover was proved
    /// total.  This is the bijective power-space image of
    /// `proof_cover().closure_carrier()`.
    pub(crate) const fn proven_domain(&self) -> &SectorInteriorDomain {
        &self.proven_domain
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

fn try_carrier_domain(
    cover: &ClosedExactExecutableOwnerCover,
) -> Result<SectorInteriorDomain, StratumRegistryError> {
    let proof = cover.executable_cover().proof_cover();
    try_carrier_domain_from_lattice(proof.sector(), proof.closure_carrier())
}

pub(crate) fn try_carrier_domain_from_lattice(
    sector: &Mask,
    carrier: &LatticeBox,
) -> Result<SectorInteriorDomain, StratumRegistryError> {
    if carrier.arity() != sector.arity() {
        return Err(StratumRegistryError::Invariant {
            detail: "closed-sector carrier and sector have different arities",
        });
    }

    let mut bounds = Vec::new();
    bounds.try_reserve_exact(sector.arity()).map_err(|_| {
        StratumRegistryError::AllocationFailure {
            resource: "closed-sector proven-domain bounds",
            requested: sector.arity(),
        }
    })?;
    for ((&lower, &upper), &active) in carrier
        .lower()
        .iter()
        .zip(carrier.upper())
        .zip(sector.active_bits())
    {
        let upper = upper.ok_or(StratumRegistryError::Invariant {
            detail: "closed-sector proof carrier has an unbounded endpoint",
        })?;
        let (power_lower, power_upper) = if active {
            (i128::from(lower) + 1, i128::from(upper) + 1)
        } else {
            (-i128::from(upper), -i128::from(lower))
        };
        let power_lower =
            i64::try_from(power_lower).map_err(|_| StratumRegistryError::Invariant {
                detail: "closed-sector carrier lower endpoint is not an i64 power",
            })?;
        let power_upper =
            i64::try_from(power_upper).map_err(|_| StratumRegistryError::Invariant {
                detail: "closed-sector carrier upper endpoint is not an i64 power",
            })?;
        bounds.push(InteriorBounds::new(power_lower, power_upper));
    }
    SectorInteriorDomain::try_new(sector.clone(), bounds).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_carrier_chart_is_bijective_at_active_and_inactive_endpoints() {
        let sector = Mask::try_new([true, false]).unwrap();
        let carrier = LatticeBox::try_new([2, 3], [Some(5), Some(1_u64 << 63)]).unwrap();
        let domain = try_carrier_domain_from_lattice(&sector, &carrier).unwrap();
        assert_eq!(
            domain.bounds(),
            [InteriorBounds::new(3, 6), InteriorBounds::new(i64::MIN, -3),]
        );
    }

    #[test]
    fn unbounded_carrier_cannot_be_published_as_finite_authority() {
        let sector = Mask::try_new([true]).unwrap();
        let carrier = LatticeBox::try_new([0], [None]).unwrap();
        assert!(matches!(
            try_carrier_domain_from_lattice(&sector, &carrier),
            Err(StratumRegistryError::Invariant {
                detail: "closed-sector proof carrier has an unbounded endpoint",
            })
        ));
    }
}
