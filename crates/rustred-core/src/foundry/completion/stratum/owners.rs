use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::artifact::{
    ArtifactSchemaVersion, ClosedArtifact, ClosedTerminalAuthority, ZeroTerminalProof,
};
use crate::sector::{Mask, SectorInteriorDomain};

use super::identity::BoundedIdentityBuilder;
use super::{
    StratumRegistryError, StratumRegistryLimits, check_limit, checked_add, checked_mul, try_reserve,
};

/// Stable execution identity of one immutable terminal-owner snapshot.
///
/// The identity commits to the complete ordered owner payload, not merely its
/// counts. Later promotion or persistence must still rejoin and verify the
/// corresponding sealed snapshot rather than treating this diagnostic string
/// as independent terminal authority.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ImmutableOwnerSnapshotId(Arc<String>);

impl ImmutableOwnerSnapshotId {
    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ImmutableOwnerKind {
    ZeroSector,
    Factorization,
    Master,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ImmutableTerminalOwner {
    ZeroSector {
        sector: Mask,
        proof: ZeroTerminalProof,
    },
    Factorization {
        source_ordinal: usize,
        domain: SectorInteriorDomain,
    },
    Master {
        key: IntegralKey,
    },
}

impl ImmutableTerminalOwner {
    const fn kind(&self) -> ImmutableOwnerKind {
        match self {
            Self::ZeroSector { .. } => ImmutableOwnerKind::ZeroSector,
            Self::Factorization { .. } => ImmutableOwnerKind::Factorization,
            Self::Master { .. } => ImmutableOwnerKind::Master,
        }
    }

    fn arity(&self) -> usize {
        match self {
            Self::ZeroSector { sector, .. } => sector.arity(),
            Self::Factorization { domain, .. } => domain.arity(),
            Self::Master { key } => key.powers().len(),
        }
    }

    fn covers(&self, target: &SectorInteriorDomain) -> bool {
        match self {
            Self::ZeroSector { sector, .. } => sector == target.sector(),
            Self::Factorization { domain, .. } => domain_contains(domain, target),
            Self::Master { key } => domain_is_singleton_key(target, key),
        }
    }
}

/// Exact reference to a terminalizing owner retained in one immutable
/// snapshot. The owner ordinal is meaningful only together with that
/// snapshot's identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ImmutableOwnerWitness {
    owner_ordinal: usize,
    kind: ImmutableOwnerKind,
}

impl ImmutableOwnerWitness {
    pub(crate) const fn owner_ordinal(self) -> usize {
        self.owner_ordinal
    }

    pub(crate) const fn kind(self) -> ImmutableOwnerKind {
        self.kind
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SnapshotSource {
    Empty,
    ClosedArtifact {
        schema: ArtifactSchemaVersion,
        algorithm_id: Arc<String>,
        zero_sector_count: usize,
        factorization_count: usize,
        master_count: usize,
    },
    TerminalAuthority {
        authority_id: Arc<String>,
        zero_sector_count: usize,
        factorization_count: usize,
        master_count: usize,
    },
}

/// Frozen terminal-only view of authenticated terminal owners.
///
/// Ordinary RuleCells are intentionally absent: they are one-step identities,
/// not terminalizing owners by themselves. Only proof-backed zero sectors,
/// installed factorizations into sealed dependencies, and explicit master
/// points can discharge a lower-sector image in this snapshot.
#[derive(Clone, Debug)]
pub(crate) struct ImmutableOwnerSnapshot {
    family_fingerprint: Arc<String>,
    context_fingerprint: Arc<String>,
    arity: usize,
    source: SnapshotSource,
    owners: Arc<[ImmutableTerminalOwner]>,
    id: ImmutableOwnerSnapshotId,
    terminal_authority: Option<Arc<ClosedTerminalAuthority>>,
}

impl PartialEq for ImmutableOwnerSnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.arity == other.arity
            && self.source == other.source
            && self.owners == other.owners
            && self.id == other.id
            && self.terminal_authority.is_some() == other.terminal_authority.is_some()
    }
}

impl Eq for ImmutableOwnerSnapshot {}

impl ImmutableOwnerSnapshot {
    /// Construct a sound snapshot with no terminal owners. This is useful for
    /// structural discovery: every proper-subsector image then remains
    /// forbidden rather than being accepted speculatively.
    pub(crate) fn try_empty(
        family_fingerprint: &str,
        context_fingerprint: &str,
        arity: usize,
        limits: StratumRegistryLimits,
    ) -> Result<Self, StratumRegistryError> {
        require_identity(family_fingerprint, "owner family fingerprint")?;
        require_identity(context_fingerprint, "owner coefficient-context fingerprint")?;
        if arity == 0 {
            return Err(StratumRegistryError::WrongOwnerArity {
                owner: 0,
                expected: 1,
                actual: 0,
            });
        }
        let source = SnapshotSource::Empty;
        let id = build_snapshot_id(
            family_fingerprint,
            context_fingerprint,
            arity,
            &source,
            &[],
            limits,
        )?;
        Ok(Self {
            family_fingerprint: Arc::new(family_fingerprint.to_owned()),
            context_fingerprint: Arc::new(context_fingerprint.to_owned()),
            arity,
            source,
            owners: Arc::from([]),
            id,
            terminal_authority: None,
        })
    }

    /// Freeze only terminalizing regions from an already installed artifact.
    /// No caller-authored region can enter through this boundary.
    pub(crate) fn try_from_closed_artifact(
        artifact: &ClosedArtifact,
        limits: StratumRegistryLimits,
    ) -> Result<Self, StratumRegistryError> {
        let zero_sector_count = artifact.zero_sectors().len();
        let factorization_count = artifact.factorization_rules().len();
        let master_count = artifact.masters().len();
        let owner_count = checked_add(
            "immutable terminal-owner regions",
            checked_add(
                "immutable terminal-owner regions",
                zero_sector_count,
                factorization_count,
            )?,
            master_count,
        )?;
        check_limit(
            "immutable terminal-owner regions",
            owner_count,
            limits.max_owner_regions,
        )?;
        let coordinate_cells = checked_mul(
            "immutable terminal-owner coordinate cells",
            owner_count,
            artifact.arity(),
        )?;
        check_limit(
            "immutable terminal-owner coordinate cells",
            coordinate_cells,
            limits.max_owner_coordinate_cells,
        )?;

        let mut owners = Vec::new();
        try_reserve(&mut owners, owner_count, "immutable terminal-owner regions")?;
        for terminal in artifact.zero_sectors() {
            owners.push(ImmutableTerminalOwner::ZeroSector {
                sector: terminal.sector().clone(),
                proof: terminal.proof(),
            });
        }
        for (source_ordinal, rule) in artifact.factorization_rules().iter().enumerate() {
            owners.push(ImmutableTerminalOwner::Factorization {
                source_ordinal,
                domain: rule.application_domain().clone(),
            });
        }
        for master in artifact.masters() {
            owners.push(ImmutableTerminalOwner::Master {
                key: master.clone(),
            });
        }

        for (owner, region) in owners.iter().enumerate() {
            if region.arity() != artifact.arity() {
                return Err(StratumRegistryError::WrongOwnerArity {
                    owner,
                    expected: artifact.arity(),
                    actual: region.arity(),
                });
            }
        }
        let source = SnapshotSource::ClosedArtifact {
            schema: artifact.schema(),
            algorithm_id: Arc::new(artifact.algorithm_id().to_owned()),
            zero_sector_count,
            factorization_count,
            master_count,
        };
        let id = build_snapshot_id(
            artifact.family_fingerprint(),
            artifact.context_fingerprint(),
            artifact.arity(),
            &source,
            &owners,
            limits,
        )?;
        Ok(Self {
            family_fingerprint: Arc::new(artifact.family_fingerprint().to_owned()),
            context_fingerprint: Arc::new(artifact.context_fingerprint().to_owned()),
            arity: artifact.arity(),
            source,
            owners: Arc::from(owners),
            id,
            terminal_authority: None,
        })
    }

    /// Freeze terminal regions while retaining their installed proof
    /// authority for the complete snapshot lifetime. This boundary performs
    /// only bounded copying of already authenticated domains; it never reruns
    /// zero analysis or factorization compilation.
    pub(crate) fn try_from_terminal_authority(
        authority: Arc<ClosedTerminalAuthority>,
        limits: StratumRegistryLimits,
    ) -> Result<Self, StratumRegistryError> {
        let zero_sector_count = authority.zero_sectors().len();
        let factorization_count = authority.factorization_rules().len();
        let master_count = authority.parent_terminals().len();
        let owner_count = checked_add(
            "immutable terminal-owner regions",
            checked_add(
                "immutable terminal-owner regions",
                zero_sector_count,
                factorization_count,
            )?,
            master_count,
        )?;
        check_limit(
            "immutable terminal-owner regions",
            owner_count,
            limits.max_owner_regions,
        )?;
        let coordinate_cells = checked_mul(
            "immutable terminal-owner coordinate cells",
            owner_count,
            authority.arity(),
        )?;
        check_limit(
            "immutable terminal-owner coordinate cells",
            coordinate_cells,
            limits.max_owner_coordinate_cells,
        )?;

        let mut owners = Vec::new();
        try_reserve(&mut owners, owner_count, "immutable terminal-owner regions")?;
        for terminal in authority.zero_sectors() {
            owners.push(ImmutableTerminalOwner::ZeroSector {
                sector: terminal.sector().clone(),
                proof: terminal.proof(),
            });
        }
        for (source_ordinal, rule) in authority.factorization_rules().iter().enumerate() {
            owners.push(ImmutableTerminalOwner::Factorization {
                source_ordinal,
                domain: rule.application_domain().clone(),
            });
        }
        for master in authority.parent_terminals() {
            owners.push(ImmutableTerminalOwner::Master {
                key: master.clone(),
            });
        }
        for (owner, region) in owners.iter().enumerate() {
            if region.arity() != authority.arity() {
                return Err(StratumRegistryError::WrongOwnerArity {
                    owner,
                    expected: authority.arity(),
                    actual: region.arity(),
                });
            }
        }

        let source = SnapshotSource::TerminalAuthority {
            authority_id: Arc::new(authority.authority_id().to_owned()),
            zero_sector_count,
            factorization_count,
            master_count,
        };
        let id = build_snapshot_id(
            authority.family_fingerprint(),
            authority.context_fingerprint(),
            authority.arity(),
            &source,
            &owners,
            limits,
        )?;
        Ok(Self {
            family_fingerprint: Arc::new(authority.family_fingerprint().to_owned()),
            context_fingerprint: Arc::new(authority.context_fingerprint().to_owned()),
            arity: authority.arity(),
            source,
            owners: Arc::from(owners),
            id,
            terminal_authority: Some(authority),
        })
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.context_fingerprint.as_str()
    }

    pub(crate) const fn arity(&self) -> usize {
        self.arity
    }

    pub(crate) fn owner_count(&self) -> usize {
        self.owners.len()
    }

    pub(crate) const fn id(&self) -> &ImmutableOwnerSnapshotId {
        &self.id
    }

    pub(super) fn owner_for(&self, target: &SectorInteriorDomain) -> Option<ImmutableOwnerWitness> {
        // Installation forbids zero/factorization overlap. A compiled
        // factorization intentionally precedes its embedded parent-terminal
        // corner: that corner is reduced through sealed dependencies rather
        // than being mistaken for an independent master.
        self.owners
            .iter()
            .enumerate()
            .find(|(_, owner)| owner.covers(target))
            .map(|(owner_ordinal, owner)| ImmutableOwnerWitness {
                owner_ordinal,
                kind: owner.kind(),
            })
    }

    pub(super) fn verifies_witness(
        &self,
        target: &SectorInteriorDomain,
        witness: ImmutableOwnerWitness,
    ) -> bool {
        self.owners
            .get(witness.owner_ordinal)
            .is_some_and(|owner| owner.kind() == witness.kind && owner.covers(target))
    }

    pub(crate) fn try_verify(
        &self,
        limits: StratumRegistryLimits,
    ) -> Result<bool, StratumRegistryError> {
        check_limit(
            "immutable terminal-owner regions",
            self.owners.len(),
            limits.max_owner_regions,
        )?;
        let cells = checked_mul(
            "immutable terminal-owner coordinate cells",
            self.owners.len(),
            self.arity,
        )?;
        check_limit(
            "immutable terminal-owner coordinate cells",
            cells,
            limits.max_owner_coordinate_cells,
        )?;
        if self.owners.iter().any(|owner| owner.arity() != self.arity) {
            return Ok(false);
        }
        let expected = build_snapshot_id(
            self.family_fingerprint(),
            self.context_fingerprint(),
            self.arity,
            &self.source,
            &self.owners,
            limits,
        )?;
        Ok(expected == self.id
            && source_counts_match(&self.source, &self.owners)
            && self.source_authority_matches())
    }

    /// Rejoin the cheap snapshot payload to its strongly owned installed
    /// authority. Exact CAS validation already occurred at installation.
    fn source_authority_matches(&self) -> bool {
        match (&self.source, self.terminal_authority.as_ref()) {
            (SnapshotSource::Empty | SnapshotSource::ClosedArtifact { .. }, None) => true,
            (
                SnapshotSource::TerminalAuthority {
                    authority_id,
                    zero_sector_count,
                    factorization_count,
                    master_count,
                },
                Some(authority),
            ) => {
                authority_id.as_str() == authority.authority_id()
                    && self.family_fingerprint() == authority.family_fingerprint()
                    && self.context_fingerprint() == authority.context_fingerprint()
                    && self.arity == authority.arity()
                    && *zero_sector_count == authority.zero_sectors().len()
                    && *factorization_count == authority.factorization_rules().len()
                    && *master_count == authority.parent_terminals().len()
                    && authority_payload_matches(&self.owners, authority)
            }
            _ => false,
        }
    }
}

fn authority_payload_matches(
    owners: &[ImmutableTerminalOwner],
    authority: &ClosedTerminalAuthority,
) -> bool {
    let mut owners = owners.iter();
    for terminal in authority.zero_sectors() {
        if !matches!(
            owners.next(),
            Some(ImmutableTerminalOwner::ZeroSector { sector, proof })
                if sector == terminal.sector() && *proof == terminal.proof()
        ) {
            return false;
        }
    }
    for (source_ordinal, rule) in authority.factorization_rules().iter().enumerate() {
        if !matches!(
            owners.next(),
            Some(ImmutableTerminalOwner::Factorization {
                source_ordinal: actual_ordinal,
                domain,
            }) if *actual_ordinal == source_ordinal && domain == rule.application_domain()
        ) {
            return false;
        }
    }
    for terminal in authority.parent_terminals() {
        if !matches!(
            owners.next(),
            Some(ImmutableTerminalOwner::Master { key }) if key == terminal
        ) {
            return false;
        }
    }
    owners.next().is_none()
}

fn source_counts_match(source: &SnapshotSource, owners: &[ImmutableTerminalOwner]) -> bool {
    match source {
        SnapshotSource::Empty => owners.is_empty(),
        SnapshotSource::ClosedArtifact {
            zero_sector_count,
            factorization_count,
            master_count,
            ..
        } => {
            owners
                .iter()
                .filter(|owner| matches!(owner, ImmutableTerminalOwner::ZeroSector { .. }))
                .count()
                == *zero_sector_count
                && owners
                    .iter()
                    .filter(|owner| matches!(owner, ImmutableTerminalOwner::Factorization { .. }))
                    .count()
                    == *factorization_count
                && owners
                    .iter()
                    .filter(|owner| matches!(owner, ImmutableTerminalOwner::Master { .. }))
                    .count()
                    == *master_count
        }
        SnapshotSource::TerminalAuthority {
            zero_sector_count,
            factorization_count,
            master_count,
            ..
        } => {
            owners
                .iter()
                .filter(|owner| matches!(owner, ImmutableTerminalOwner::ZeroSector { .. }))
                .count()
                == *zero_sector_count
                && owners
                    .iter()
                    .filter(|owner| matches!(owner, ImmutableTerminalOwner::Factorization { .. }))
                    .count()
                    == *factorization_count
                && owners
                    .iter()
                    .filter(|owner| matches!(owner, ImmutableTerminalOwner::Master { .. }))
                    .count()
                    == *master_count
        }
    }
}

fn domain_contains(outer: &SectorInteriorDomain, inner: &SectorInteriorDomain) -> bool {
    outer.sector() == inner.sector()
        && outer
            .bounds()
            .iter()
            .zip(inner.bounds())
            .all(|(&outer, &inner)| {
                outer.lower() <= inner.lower() && inner.upper() <= outer.upper()
            })
}

fn domain_is_singleton_key(domain: &SectorInteriorDomain, key: &IntegralKey) -> bool {
    key.powers().len() == domain.arity()
        && domain
            .bounds()
            .iter()
            .zip(key.powers())
            .all(|(&bounds, &power)| bounds.lower() == power && bounds.upper() == power)
}

fn build_snapshot_id(
    family: &str,
    context: &str,
    arity: usize,
    source: &SnapshotSource,
    owners: &[ImmutableTerminalOwner],
    limits: StratumRegistryLimits,
) -> Result<ImmutableOwnerSnapshotId, StratumRegistryError> {
    let mut stable = BoundedIdentityBuilder::new(
        limits.max_owner_identity_bytes,
        "immutable terminal-owner identity bytes",
    );
    stable.push("rustred.immutable-terminal-owner-snapshot.v2:")?;
    stable.push_usize(family.len())?;
    stable.push("#")?;
    stable.push(family)?;
    stable.push(":")?;
    stable.push_usize(context.len())?;
    stable.push("#")?;
    stable.push(context)?;
    stable.push(":")?;
    stable.push_usize(arity)?;
    stable.push(":")?;
    match source {
        SnapshotSource::Empty => stable.push("empty")?,
        SnapshotSource::ClosedArtifact {
            schema,
            algorithm_id,
            zero_sector_count,
            factorization_count,
            master_count,
        } => {
            stable.push("artifact:")?;
            stable.push(schema.stable_id())?;
            stable.push(":")?;
            stable.push_usize(algorithm_id.len())?;
            stable.push("#")?;
            stable.push(algorithm_id)?;
            stable.push(":")?;
            stable.push_usize(*zero_sector_count)?;
            stable.push(":")?;
            stable.push_usize(*factorization_count)?;
            stable.push(":")?;
            stable.push_usize(*master_count)?;
        }
        SnapshotSource::TerminalAuthority {
            authority_id,
            zero_sector_count,
            factorization_count,
            master_count,
        } => {
            stable.push("terminal-authority:")?;
            stable.push_usize(authority_id.len())?;
            stable.push("#")?;
            stable.push(authority_id)?;
            stable.push(":")?;
            stable.push_usize(*zero_sector_count)?;
            stable.push(":")?;
            stable.push_usize(*factorization_count)?;
            stable.push(":")?;
            stable.push_usize(*master_count)?;
        }
    }
    stable.push(":owners[")?;
    for (ordinal, owner) in owners.iter().enumerate() {
        if ordinal != 0 {
            stable.push(",")?;
        }
        stable.push_usize(ordinal)?;
        stable.push("=")?;
        append_owner_identity(&mut stable, owner)?;
    }
    stable.push("]")?;
    Ok(ImmutableOwnerSnapshotId(Arc::new(stable.finish())))
}

fn append_owner_identity(
    stable: &mut BoundedIdentityBuilder,
    owner: &ImmutableTerminalOwner,
) -> Result<(), StratumRegistryError> {
    match owner {
        ImmutableTerminalOwner::ZeroSector { sector, proof } => {
            stable.push("zero:")?;
            stable.push(proof.stable_id())?;
            stable.push(":")?;
            append_mask(stable, sector.active_bits())?;
        }
        ImmutableTerminalOwner::Factorization {
            source_ordinal,
            domain,
        } => {
            stable.push("factorization:")?;
            stable.push_usize(*source_ordinal)?;
            stable.push(":")?;
            append_mask(stable, domain.sector().active_bits())?;
            stable.push(":")?;
            for (position, bounds) in domain.bounds().iter().enumerate() {
                if position != 0 {
                    stable.push(",")?;
                }
                stable.push_i64(bounds.lower())?;
                stable.push("..")?;
                stable.push_i64(bounds.upper())?;
            }
        }
        ImmutableTerminalOwner::Master { key } => {
            stable.push("master:")?;
            for (position, &power) in key.powers().iter().enumerate() {
                if position != 0 {
                    stable.push(",")?;
                }
                stable.push_i64(power)?;
            }
        }
    }
    Ok(())
}

fn append_mask(
    stable: &mut BoundedIdentityBuilder,
    active_bits: &[bool],
) -> Result<(), StratumRegistryError> {
    for &active in active_bits {
        stable.push(if active { "1" } else { "0" })?;
    }
    Ok(())
}

fn require_identity(value: &str, identity: &'static str) -> Result<(), StratumRegistryError> {
    if value.is_empty() {
        Err(StratumRegistryError::EmptyIdentity { identity })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::family::IntegralKey;
    use crate::foundry::artifact::{
        ArtifactSchemaVersion, ZeroTerminalProof, derive_k6_terminal_authority,
    };
    use crate::sector::{InteriorBounds, Mask, SectorInteriorDomain};

    use super::{
        ImmutableTerminalOwner, SnapshotSource, StratumRegistryError, StratumRegistryLimits,
        build_snapshot_id,
    };

    fn source() -> SnapshotSource {
        SnapshotSource::ClosedArtifact {
            schema: ArtifactSchemaVersion::CURRENT,
            algorithm_id: "owner-id-adversary".to_owned().into(),
            zero_sector_count: 1,
            factorization_count: 1,
            master_count: 1,
        }
    }

    fn owners(
        zero_proof: ZeroTerminalProof,
        factorization_upper: i64,
        master_power: i64,
    ) -> Vec<ImmutableTerminalOwner> {
        vec![
            ImmutableTerminalOwner::ZeroSector {
                sector: Mask::try_new([false]).unwrap(),
                proof: zero_proof,
            },
            ImmutableTerminalOwner::Factorization {
                source_ordinal: 0,
                domain: SectorInteriorDomain::try_new(
                    Mask::try_new([true]).unwrap(),
                    [InteriorBounds::new(1, factorization_upper)],
                )
                .unwrap(),
            },
            ImmutableTerminalOwner::Master {
                key: IntegralKey::try_new([master_power]).unwrap(),
            },
        ]
    }

    #[test]
    fn snapshot_identity_binds_every_ordered_owner_payload() {
        let source = source();
        let base = owners(ZeroTerminalProof::ScalelessVacuumPolynomial, 4, 1);
        let zero_changed = owners(ZeroTerminalProof::LeePomeranskyRankDeficiency, 4, 1);
        let factorization_changed = owners(ZeroTerminalProof::ScalelessVacuumPolynomial, 5, 1);
        let master_changed = owners(ZeroTerminalProof::ScalelessVacuumPolynomial, 4, 2);
        let limits = StratumRegistryLimits::default();
        let id = build_snapshot_id("family", "context", 1, &source, &base, limits).unwrap();

        for changed in [&zero_changed, &factorization_changed, &master_changed] {
            assert_ne!(
                id,
                build_snapshot_id("family", "context", 1, &source, changed, limits).unwrap()
            );
        }
    }

    #[test]
    fn snapshot_identity_bytes_are_bounded_before_retention() {
        let mut limits = StratumRegistryLimits::default();
        limits.max_owner_identity_bytes = 0;
        assert_eq!(
            build_snapshot_id("family", "context", 1, &SnapshotSource::Empty, &[], limits)
                .unwrap_err(),
            StratumRegistryError::ResourceLimit {
                resource: "immutable terminal-owner identity bytes",
                requested: "rustred.immutable-terminal-owner-snapshot.v2:".len(),
                limit: 0,
            }
        );
    }

    #[test]
    fn terminal_snapshot_strongly_rejoins_the_exact_sealed_arc() {
        let authority = derive_k6_terminal_authority().unwrap();
        let snapshot = super::ImmutableOwnerSnapshot::try_from_terminal_authority(
            Arc::clone(&authority),
            StratumRegistryLimits::default(),
        )
        .unwrap();
        assert!(Arc::ptr_eq(
            snapshot
                .terminal_authority
                .as_ref()
                .expect("a terminal-authority snapshot retains its exact proof owner"),
            &authority
        ));
        drop(authority);
        assert!(
            snapshot
                .try_verify(StratumRegistryLimits::default())
                .unwrap()
        );
    }
}
