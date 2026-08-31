use std::sync::Arc;

use crate::family::IntegralKey;
use crate::foundry::artifact::{
    ArtifactSchemaVersion, ClosedArtifact, ClosedTerminalAuthority, ZeroTerminalProof,
};
use crate::foundry::completion::source_discovery::ClosedSectorLayer;
use crate::sector::{Mask, OrderingPolicy, SectorInteriorDomain};

use super::identity::BoundedIdentityBuilder;
use super::{
    StratumRegistryError, StratumRegistryLimits, check_limit, checked_add, checked_mul, try_reserve,
};

/// Stable execution identity of one immutable owner snapshot.
///
/// The identity commits to the complete ordered lookup descriptors and the
/// one-time complete content ID of each solved layer, not merely owner counts.
/// It deliberately does not duplicate executable layer content. Later
/// promotion or persistence must rejoin the retained snapshot authority rather
/// than treating this digest string as independent proof authority.
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
    SolvedRewriteSector,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ImmutableOwner {
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
    SolvedRewriteSector {
        sector: Mask,
        ordering: OrderingPolicy,
        layer_ordinal: usize,
    },
}

impl ImmutableOwner {
    const fn kind(&self) -> ImmutableOwnerKind {
        match self {
            Self::ZeroSector { .. } => ImmutableOwnerKind::ZeroSector,
            Self::Factorization { .. } => ImmutableOwnerKind::Factorization,
            Self::Master { .. } => ImmutableOwnerKind::Master,
            Self::SolvedRewriteSector { .. } => ImmutableOwnerKind::SolvedRewriteSector,
        }
    }

    fn arity(&self) -> usize {
        match self {
            Self::ZeroSector { sector, .. } => sector.arity(),
            Self::Factorization { domain, .. } => domain.arity(),
            Self::Master { key } => key.powers().len(),
            Self::SolvedRewriteSector { sector, .. } => sector.arity(),
        }
    }

    fn covers(&self, ordering: OrderingPolicy, target: &SectorInteriorDomain) -> bool {
        match self {
            Self::ZeroSector { sector, .. } => sector == target.sector(),
            Self::Factorization { domain, .. } => domain_contains(domain, target),
            Self::Master { key } => domain_is_singleton_key(target, key),
            Self::SolvedRewriteSector {
                sector,
                ordering: owner_ordering,
                ..
            } => *owner_ordering == ordering && sector == target.sector(),
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

/// Frozen view of authenticated terminal and solved-sector owners.
///
/// Ordinary RuleCells are intentionally absent: they are one-step identities,
/// not terminalizing owners by themselves. Only proof-backed zero sectors,
/// installed factorizations into sealed dependencies, and explicit master
/// points can discharge a lower-sector image in a root snapshot. Published
/// closed-sector layers extend that immutable base without mutating it.
#[derive(Clone, Debug)]
pub(crate) struct ImmutableOwnerSnapshot {
    family_fingerprint: Arc<String>,
    context_fingerprint: Arc<String>,
    arity: usize,
    source: SnapshotSource,
    owners: Arc<[ImmutableOwner]>,
    id: ImmutableOwnerSnapshotId,
    terminal_authority: Option<Arc<ClosedTerminalAuthority>>,
    closed_layers: Arc<[Arc<ClosedSectorLayer>]>,
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
            && layer_content_ids_equal(&self.closed_layers, &other.closed_layers)
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
            closed_layers: Arc::from([]),
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
            "immutable owner regions",
            checked_add(
                "immutable owner regions",
                zero_sector_count,
                factorization_count,
            )?,
            master_count,
        )?;
        check_limit(
            "immutable owner regions",
            owner_count,
            limits.max_owner_regions,
        )?;
        let coordinate_cells = checked_mul(
            "immutable owner coordinate cells",
            owner_count,
            artifact.arity(),
        )?;
        check_limit(
            "immutable owner coordinate cells",
            coordinate_cells,
            limits.max_owner_coordinate_cells,
        )?;

        let mut owners = Vec::new();
        try_reserve(&mut owners, owner_count, "immutable owner regions")?;
        for terminal in artifact.zero_sectors() {
            owners.push(ImmutableOwner::ZeroSector {
                sector: terminal.sector().clone(),
                proof: terminal.proof(),
            });
        }
        for (source_ordinal, rule) in artifact.factorization_rules().iter().enumerate() {
            owners.push(ImmutableOwner::Factorization {
                source_ordinal,
                domain: rule.application_domain().clone(),
            });
        }
        for master in artifact.masters() {
            owners.push(ImmutableOwner::Master {
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
            &[],
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
            closed_layers: Arc::from([]),
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
            "immutable owner regions",
            checked_add(
                "immutable owner regions",
                zero_sector_count,
                factorization_count,
            )?,
            master_count,
        )?;
        check_limit(
            "immutable owner regions",
            owner_count,
            limits.max_owner_regions,
        )?;
        let coordinate_cells = checked_mul(
            "immutable owner coordinate cells",
            owner_count,
            authority.arity(),
        )?;
        check_limit(
            "immutable owner coordinate cells",
            coordinate_cells,
            limits.max_owner_coordinate_cells,
        )?;

        let mut owners = Vec::new();
        try_reserve(&mut owners, owner_count, "immutable owner regions")?;
        for terminal in authority.zero_sectors() {
            owners.push(ImmutableOwner::ZeroSector {
                sector: terminal.sector().clone(),
                proof: terminal.proof(),
            });
        }
        for (source_ordinal, rule) in authority.factorization_rules().iter().enumerate() {
            owners.push(ImmutableOwner::Factorization {
                source_ordinal,
                domain: rule.application_domain().clone(),
            });
        }
        for master in authority.parent_terminals() {
            owners.push(ImmutableOwner::Master {
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
            &[],
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
            closed_layers: Arc::from([]),
        })
    }

    /// Transactionally append one same-active-count wave of exact closed
    /// sectors.  Every layer must have been proved against this exact snapshot
    /// authority, so worker arrival order cannot change the predecessor seen by
    /// another member of the wave.
    ///
    /// Incoming layers are sorted by exact sector and ordering before owner
    /// ordinals are appended. Existing ordinals are never reordered. Symmetry
    /// aliases are intentionally absent: each owner covers only its published
    /// sector under its published ordering.
    pub(crate) fn try_extend_with_closed_layers(
        &self,
        layers: Vec<Arc<ClosedSectorLayer>>,
        limits: StratumRegistryLimits,
    ) -> Result<Self, StratumRegistryError> {
        if layers.is_empty() {
            return Err(StratumRegistryError::EmptyClosedSectorLayerBatch);
        }
        // Preflight aggregate retained capacity before allocating, indexing,
        // validating, or sorting the caller's potentially large batch.
        let owner_count = checked_add("immutable owner regions", self.owners.len(), layers.len())?;
        check_limit(
            "immutable owner regions",
            owner_count,
            limits.max_owner_regions,
        )?;
        let coordinate_cells =
            checked_mul("immutable owner coordinate cells", owner_count, self.arity)?;
        check_limit(
            "immutable owner coordinate cells",
            coordinate_cells,
            limits.max_owner_coordinate_cells,
        )?;
        let mut indexed_layers = Vec::new();
        try_reserve(
            &mut indexed_layers,
            layers.len(),
            "closed-sector extension batch",
        )?;
        for (input_ordinal, layer) in layers.into_iter().enumerate() {
            if layer.family_fingerprint() != self.family_fingerprint() {
                return Err(StratumRegistryError::WrongClosedSectorLayerFamily {
                    layer: input_ordinal,
                });
            }
            if layer.context_fingerprint() != self.context_fingerprint() {
                return Err(StratumRegistryError::WrongClosedSectorLayerContext {
                    layer: input_ordinal,
                });
            }
            if layer.sector().arity() != self.arity {
                return Err(StratumRegistryError::WrongOwnerArity {
                    owner: input_ordinal,
                    expected: self.arity,
                    actual: layer.sector().arity(),
                });
            }
            if !layer.predecessor_snapshot().same_authority_as(self) {
                return Err(StratumRegistryError::WrongClosedSectorLayerPredecessor {
                    layer: input_ordinal,
                });
            }
            indexed_layers.push((input_ordinal, layer));
        }

        indexed_layers.sort_by(|left, right| {
            left.1
                .sector()
                .active_count()
                .cmp(&right.1.sector().active_count())
                .then_with(|| left.1.sector().cmp(right.1.sector()))
                .then_with(|| left.1.ordering().cmp(&right.1.ordering()))
                .then_with(|| left.1.content_id().cmp(right.1.content_id()))
        });
        let expected_active_count = indexed_layers[0].1.sector().active_count();
        for (input_ordinal, layer) in &indexed_layers {
            let actual_active_count = layer.sector().active_count();
            if actual_active_count != expected_active_count {
                return Err(StratumRegistryError::MixedClosedSectorLayerFrontier {
                    layer: *input_ordinal,
                    expected_active_count,
                    actual_active_count,
                });
            }
        }
        require_frontier_advance(
            self.closed_layers
                .last()
                .map(|layer| layer.sector().active_count()),
            expected_active_count,
        )?;
        for pair in indexed_layers.windows(2) {
            if same_solved_key(&pair[0].1, &pair[1].1) {
                return Err(StratumRegistryError::DuplicateClosedSectorOwner {
                    first_layer: pair[0].0,
                    second_layer: pair[1].0,
                });
            }
        }
        for (input_ordinal, layer) in &indexed_layers {
            if let Some(existing_layer) = self.closed_layers.iter().position(|existing| {
                existing.sector() == layer.sector() && existing.ordering() == layer.ordering()
            }) {
                return Err(StratumRegistryError::DuplicateClosedSectorOwner {
                    first_layer: existing_layer,
                    second_layer: *input_ordinal,
                });
            }
        }

        let mut closed_layers = Vec::new();
        try_reserve(
            &mut closed_layers,
            self.closed_layers.len(),
            "immutable closed-sector layers",
        )?;
        closed_layers.extend(self.closed_layers.iter().cloned());
        try_reserve(
            &mut closed_layers,
            indexed_layers.len(),
            "immutable closed-sector layers",
        )?;

        let mut owners = Vec::new();
        try_reserve(&mut owners, self.owners.len(), "immutable owner regions")?;
        owners.extend(self.owners.iter().cloned());
        try_reserve(&mut owners, indexed_layers.len(), "immutable owner regions")?;
        for (_, layer) in indexed_layers {
            let layer_ordinal = closed_layers.len();
            owners.push(ImmutableOwner::SolvedRewriteSector {
                sector: layer.sector().clone(),
                ordering: layer.ordering(),
                layer_ordinal,
            });
            closed_layers.push(layer);
        }

        let id = build_snapshot_id(
            self.family_fingerprint(),
            self.context_fingerprint(),
            self.arity,
            &self.source,
            &owners,
            &closed_layers,
            limits,
        )?;
        Ok(Self {
            family_fingerprint: self.family_fingerprint.clone(),
            context_fingerprint: self.context_fingerprint.clone(),
            arity: self.arity,
            source: self.source.clone(),
            owners: Arc::from(owners),
            id,
            terminal_authority: self.terminal_authority.clone(),
            closed_layers: Arc::from(closed_layers),
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

    pub(crate) fn closed_layer_count(&self) -> usize {
        self.closed_layers.len()
    }

    pub(crate) const fn id(&self) -> &ImmutableOwnerSnapshotId {
        &self.id
    }

    /// Return whether two structurally equal snapshots retain the same
    /// installed proof authority.
    ///
    /// Snapshot IDs describe the bounded ordered lookup payload; they are not
    /// independently executable authority.  A terminal-authority snapshot
    /// must therefore rejoin the exact installed `Arc`, rather than accepting
    /// a separately installed authority with identical structural content.
    /// Solved-sector owners likewise require the same concrete layer `Arc` at
    /// every retained ordinal.
    pub(crate) fn same_authority_as(&self, other: &Self) -> bool {
        if self != other {
            return false;
        }
        let same_terminal_authority = match (&self.terminal_authority, &other.terminal_authority) {
            (None, None) => true,
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            _ => false,
        };
        same_terminal_authority
            && self.closed_layers.len() == other.closed_layers.len()
            && self
                .closed_layers
                .iter()
                .zip(other.closed_layers.iter())
                .all(|(left, right)| Arc::ptr_eq(left, right))
    }

    pub(crate) fn owner_for(
        &self,
        parent_sector: &Mask,
        ordering: OrderingPolicy,
        target: &SectorInteriorDomain,
    ) -> Option<ImmutableOwnerWitness> {
        if !target.sector().is_strict_subsector_of(parent_sector).ok()? {
            return None;
        }
        // Installation forbids zero/factorization overlap. A compiled
        // factorization intentionally precedes its embedded parent-terminal
        // corner: that corner is reduced through sealed dependencies rather
        // than being mistaken for an independent master. Root terminals also
        // precede solved-sector owners and remain ordering-independent.
        self.owners
            .iter()
            .enumerate()
            .find(|(_, owner)| owner.covers(ordering, target))
            .map(|(owner_ordinal, owner)| ImmutableOwnerWitness {
                owner_ordinal,
                kind: owner.kind(),
            })
    }

    pub(crate) fn verifies_witness(
        &self,
        parent_sector: &Mask,
        ordering: OrderingPolicy,
        target: &SectorInteriorDomain,
        witness: ImmutableOwnerWitness,
    ) -> bool {
        if !target
            .sector()
            .is_strict_subsector_of(parent_sector)
            .unwrap_or(false)
        {
            return false;
        }
        self.owners
            .get(witness.owner_ordinal)
            .is_some_and(|owner| owner.kind() == witness.kind && owner.covers(ordering, target))
    }

    pub(crate) fn try_verify(
        &self,
        limits: StratumRegistryLimits,
    ) -> Result<bool, StratumRegistryError> {
        check_limit(
            "immutable owner regions",
            self.owners.len(),
            limits.max_owner_regions,
        )?;
        let cells = checked_mul(
            "immutable owner coordinate cells",
            self.owners.len(),
            self.arity,
        )?;
        check_limit(
            "immutable owner coordinate cells",
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
            &self.closed_layers,
            limits,
        )?;
        Ok(expected == self.id
            && source_counts_match(&self.source, &self.owners)
            && self.source_authority_matches()
            && self.closed_layers_match(limits)?)
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

    fn closed_layers_match(
        &self,
        limits: StratumRegistryLimits,
    ) -> Result<bool, StratumRegistryError> {
        let Some(base_owner_count) = self.owners.len().checked_sub(self.closed_layers.len()) else {
            return Ok(false);
        };
        if self.owners[..base_owner_count]
            .iter()
            .any(|owner| matches!(owner, ImmutableOwner::SolvedRewriteSector { .. }))
        {
            return Ok(false);
        }
        for (layer_ordinal, (owner, layer)) in self.owners[base_owner_count..]
            .iter()
            .zip(self.closed_layers.iter())
            .enumerate()
        {
            let ImmutableOwner::SolvedRewriteSector {
                sector,
                ordering,
                layer_ordinal: owner_layer_ordinal,
            } = owner
            else {
                return Ok(false);
            };
            if *owner_layer_ordinal != layer_ordinal
                || layer.family_fingerprint() != self.family_fingerprint()
                || layer.context_fingerprint() != self.context_fingerprint()
                || layer.sector() != sector
                || layer.ordering() != *ordering
            {
                return Ok(false);
            }
        }

        if !closed_layer_wave_metadata_is_valid(&self.closed_layers) {
            return Ok(false);
        }
        let mut wave_start = 0usize;
        while wave_start < self.closed_layers.len() {
            let predecessor = self.closed_layers[wave_start].predecessor_snapshot();
            if !self.try_retains_predecessor_authority(
                predecessor,
                wave_start,
                base_owner_count,
                limits,
            )? {
                return Ok(false);
            }
            let mut wave_end = wave_start;
            while wave_end < self.closed_layers.len()
                && self.closed_layers[wave_end]
                    .predecessor_snapshot()
                    .closed_layer_count()
                    == wave_start
            {
                if !self.closed_layers[wave_end]
                    .predecessor_snapshot()
                    .same_authority_as(predecessor)
                {
                    return Ok(false);
                }
                wave_end += 1;
            }
            wave_start = wave_end;
        }
        Ok(true)
    }

    fn try_retains_predecessor_authority(
        &self,
        predecessor: &Self,
        predecessor_layer_count: usize,
        base_owner_count: usize,
        limits: StratumRegistryLimits,
    ) -> Result<bool, StratumRegistryError> {
        let predecessor_owner_count = checked_add(
            "immutable predecessor owner regions",
            base_owner_count,
            predecessor_layer_count,
        )?;
        if predecessor_layer_count >= self.closed_layers.len()
            || predecessor_owner_count >= self.owners.len()
        {
            return Ok(false);
        }
        let expected_id = build_snapshot_id(
            self.family_fingerprint(),
            self.context_fingerprint(),
            self.arity,
            &self.source,
            &self.owners[..predecessor_owner_count],
            &self.closed_layers[..predecessor_layer_count],
            limits,
        )?;
        Ok(predecessor.family_fingerprint == self.family_fingerprint
            && predecessor.context_fingerprint == self.context_fingerprint
            && predecessor.arity == self.arity
            && predecessor.source == self.source
            && predecessor.owners.len() == predecessor_owner_count
            && self.owners[..predecessor_owner_count] == predecessor.owners[..]
            && predecessor.id == expected_id
            && source_counts_match(&predecessor.source, &predecessor.owners)
            && predecessor.source_authority_matches()
            && terminal_authority_equal(
                self.terminal_authority.as_ref(),
                predecessor.terminal_authority.as_ref(),
            )
            && predecessor.closed_layers.len() == predecessor_layer_count
            && self.closed_layers[..predecessor_layer_count]
                .iter()
                .zip(predecessor.closed_layers.iter())
                .all(|(retained, prior)| Arc::ptr_eq(retained, prior)))
    }

    #[cfg(test)]
    pub(crate) fn solved_owner_matches_layer(&self, layer_ordinal: usize) -> bool {
        self.owners.iter().any(|owner| {
            matches!(
                owner,
                ImmutableOwner::SolvedRewriteSector {
                    sector,
                    ordering,
                    layer_ordinal: owner_layer,
                } if *owner_layer == layer_ordinal
                    && self.closed_layers.get(layer_ordinal).is_some_and(|layer|
                        layer.sector() == sector && layer.ordering() == *ordering)
            )
        })
    }
}

trait ClosedLayerWaveMetadata {
    fn predecessor_layer_count(&self) -> usize;
    fn active_count(&self) -> usize;
    fn sector(&self) -> &Mask;
    fn ordering(&self) -> OrderingPolicy;
}

impl ClosedLayerWaveMetadata for Arc<ClosedSectorLayer> {
    fn predecessor_layer_count(&self) -> usize {
        self.predecessor_snapshot().closed_layer_count()
    }

    fn active_count(&self) -> usize {
        self.sector().active_count()
    }

    fn sector(&self) -> &Mask {
        ClosedSectorLayer::sector(self)
    }

    fn ordering(&self) -> OrderingPolicy {
        ClosedSectorLayer::ordering(self)
    }
}

/// Reconstruct canonical transactional waves solely from immutable predecessor
/// prefix lengths and layer keys. Exact predecessor `Arc` equality is checked
/// separately once these boundaries are known.
fn closed_layer_wave_metadata_is_valid<T: ClosedLayerWaveMetadata>(layers: &[T]) -> bool {
    let mut wave_start = 0usize;
    let mut previous_active_count = None;
    while wave_start < layers.len() {
        let first = &layers[wave_start];
        if first.predecessor_layer_count() != wave_start {
            return false;
        }
        let active_count = first.active_count();
        if previous_active_count.is_some_and(|previous| active_count <= previous) {
            return false;
        }

        let mut wave_end = wave_start + 1;
        while wave_end < layers.len() && layers[wave_end].predecessor_layer_count() == wave_start {
            let previous = &layers[wave_end - 1];
            let current = &layers[wave_end];
            if current.active_count() != active_count
                || previous
                    .sector()
                    .cmp(current.sector())
                    .then_with(|| previous.ordering().cmp(&current.ordering()))
                    != std::cmp::Ordering::Less
            {
                return false;
            }
            wave_end += 1;
        }
        previous_active_count = Some(active_count);
        wave_start = wave_end;
    }
    true
}

fn require_frontier_advance(
    previous_active_count: Option<usize>,
    incoming_active_count: usize,
) -> Result<(), StratumRegistryError> {
    if let Some(previous_active_count) = previous_active_count {
        if incoming_active_count <= previous_active_count {
            return Err(
                StratumRegistryError::NonIncreasingClosedSectorLayerFrontier {
                    previous_active_count,
                    incoming_active_count,
                },
            );
        }
    }
    Ok(())
}

fn authority_payload_matches(
    owners: &[ImmutableOwner],
    authority: &ClosedTerminalAuthority,
) -> bool {
    let mut owners = owners.iter();
    for terminal in authority.zero_sectors() {
        if !matches!(
            owners.next(),
            Some(ImmutableOwner::ZeroSector { sector, proof })
                if sector == terminal.sector() && *proof == terminal.proof()
        ) {
            return false;
        }
    }
    for (source_ordinal, rule) in authority.factorization_rules().iter().enumerate() {
        if !matches!(
            owners.next(),
            Some(ImmutableOwner::Factorization {
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
            Some(ImmutableOwner::Master { key }) if key == terminal
        ) {
            return false;
        }
    }
    owners.all(|owner| matches!(owner, ImmutableOwner::SolvedRewriteSector { .. }))
}

fn terminal_authority_equal(
    left: Option<&Arc<ClosedTerminalAuthority>>,
    right: Option<&Arc<ClosedTerminalAuthority>>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => Arc::ptr_eq(left, right),
        _ => false,
    }
}

fn layer_content_ids_equal(
    left: &[Arc<ClosedSectorLayer>],
    right: &[Arc<ClosedSectorLayer>],
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.content_id() == right.content_id())
}

fn same_solved_key(left: &ClosedSectorLayer, right: &ClosedSectorLayer) -> bool {
    left.sector() == right.sector() && left.ordering() == right.ordering()
}

fn source_counts_match(source: &SnapshotSource, owners: &[ImmutableOwner]) -> bool {
    match source {
        SnapshotSource::Empty => owners
            .iter()
            .all(|owner| matches!(owner, ImmutableOwner::SolvedRewriteSector { .. })),
        SnapshotSource::ClosedArtifact {
            zero_sector_count,
            factorization_count,
            master_count,
            ..
        } => {
            owners
                .iter()
                .filter(|owner| matches!(owner, ImmutableOwner::ZeroSector { .. }))
                .count()
                == *zero_sector_count
                && owners
                    .iter()
                    .filter(|owner| matches!(owner, ImmutableOwner::Factorization { .. }))
                    .count()
                    == *factorization_count
                && owners
                    .iter()
                    .filter(|owner| matches!(owner, ImmutableOwner::Master { .. }))
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
                .filter(|owner| matches!(owner, ImmutableOwner::ZeroSector { .. }))
                .count()
                == *zero_sector_count
                && owners
                    .iter()
                    .filter(|owner| matches!(owner, ImmutableOwner::Factorization { .. }))
                    .count()
                    == *factorization_count
                && owners
                    .iter()
                    .filter(|owner| matches!(owner, ImmutableOwner::Master { .. }))
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
    owners: &[ImmutableOwner],
    closed_layers: &[Arc<ClosedSectorLayer>],
    limits: StratumRegistryLimits,
) -> Result<ImmutableOwnerSnapshotId, StratumRegistryError> {
    let mut stable = BoundedIdentityBuilder::new(
        limits.max_owner_identity_bytes,
        "immutable owner identity bytes",
    );
    stable.push("rustred.immutable-owner-snapshot.v3:")?;
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
        append_owner_identity(&mut stable, owner, closed_layers)?;
    }
    stable.push("]")?;
    Ok(ImmutableOwnerSnapshotId(Arc::new(stable.finish())))
}

fn append_owner_identity(
    stable: &mut BoundedIdentityBuilder,
    owner: &ImmutableOwner,
    closed_layers: &[Arc<ClosedSectorLayer>],
) -> Result<(), StratumRegistryError> {
    match owner {
        ImmutableOwner::ZeroSector { sector, proof } => {
            stable.push("zero:")?;
            stable.push(proof.stable_id())?;
            stable.push(":")?;
            append_mask(stable, sector.active_bits())?;
        }
        ImmutableOwner::Factorization {
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
        ImmutableOwner::Master { key } => {
            stable.push("master:")?;
            for (position, &power) in key.powers().iter().enumerate() {
                if position != 0 {
                    stable.push(",")?;
                }
                stable.push_i64(power)?;
            }
        }
        ImmutableOwner::SolvedRewriteSector {
            sector,
            ordering,
            layer_ordinal,
        } => {
            let layer =
                closed_layers
                    .get(*layer_ordinal)
                    .ok_or(StratumRegistryError::Invariant {
                        detail: "solved owner points outside the retained closed-sector layers",
                    })?;
            stable.push("solved-sector:")?;
            append_mask(stable, sector.active_bits())?;
            stable.push(":")?;
            stable.push(ordering.stable_id())?;
            stable.push(":")?;
            stable.push_usize(*layer_ordinal)?;
            stable.push(":")?;
            stable.push_usize(layer.content_id().as_str().len())?;
            stable.push("#")?;
            stable.push(layer.content_id().as_str())?;
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
    use crate::sector::{InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain};

    use super::{
        ClosedLayerWaveMetadata, ImmutableOwner, SnapshotSource, StratumRegistryError,
        StratumRegistryLimits, build_snapshot_id, closed_layer_wave_metadata_is_valid,
        require_frontier_advance,
    };

    struct TestWaveMetadata {
        predecessor_layer_count: usize,
        sector: Mask,
        ordering: OrderingPolicy,
    }

    impl TestWaveMetadata {
        fn new(predecessor_layer_count: usize, bits: &[bool]) -> Self {
            Self {
                predecessor_layer_count,
                sector: Mask::try_new(bits.iter().copied()).unwrap(),
                ordering: OrderingPolicy::default(),
            }
        }
    }

    impl ClosedLayerWaveMetadata for TestWaveMetadata {
        fn predecessor_layer_count(&self) -> usize {
            self.predecessor_layer_count
        }

        fn active_count(&self) -> usize {
            self.sector.active_count()
        }

        fn sector(&self) -> &Mask {
            &self.sector
        }

        fn ordering(&self) -> OrderingPolicy {
            self.ordering
        }
    }

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
    ) -> Vec<ImmutableOwner> {
        vec![
            ImmutableOwner::ZeroSector {
                sector: Mask::try_new([false]).unwrap(),
                proof: zero_proof,
            },
            ImmutableOwner::Factorization {
                source_ordinal: 0,
                domain: SectorInteriorDomain::try_new(
                    Mask::try_new([true]).unwrap(),
                    [InteriorBounds::new(1, factorization_upper)],
                )
                .unwrap(),
            },
            ImmutableOwner::Master {
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
        let id = build_snapshot_id("family", "context", 1, &source, &base, &[], limits).unwrap();

        for changed in [&zero_changed, &factorization_changed, &master_changed] {
            assert_ne!(
                id,
                build_snapshot_id("family", "context", 1, &source, changed, &[], limits).unwrap()
            );
        }
    }

    #[test]
    fn closed_layer_wave_metadata_requires_complete_increasing_canonical_waves() {
        let canonical = [
            TestWaveMetadata::new(0, &[false, false, true]),
            TestWaveMetadata::new(0, &[false, true, false]),
            TestWaveMetadata::new(2, &[false, true, true]),
            TestWaveMetadata::new(2, &[true, false, true]),
        ];
        assert!(closed_layer_wave_metadata_is_valid(&canonical));

        let same_rank_split = [
            TestWaveMetadata::new(0, &[false, false, true]),
            TestWaveMetadata::new(1, &[false, true, false]),
        ];
        assert!(!closed_layer_wave_metadata_is_valid(&same_rank_split));

        let decreasing = [
            TestWaveMetadata::new(0, &[false, true, true]),
            TestWaveMetadata::new(1, &[false, false, true]),
        ];
        assert!(!closed_layer_wave_metadata_is_valid(&decreasing));

        let mixed_frontier = [
            TestWaveMetadata::new(0, &[false, false, true]),
            TestWaveMetadata::new(0, &[false, true, true]),
        ];
        assert!(!closed_layer_wave_metadata_is_valid(&mixed_frontier));

        let noncanonical = [
            TestWaveMetadata::new(0, &[false, true, false]),
            TestWaveMetadata::new(0, &[false, false, true]),
        ];
        assert!(!closed_layer_wave_metadata_is_valid(&noncanonical));

        assert_eq!(
            require_frontier_advance(Some(3), 3),
            Err(
                StratumRegistryError::NonIncreasingClosedSectorLayerFrontier {
                    previous_active_count: 3,
                    incoming_active_count: 3,
                }
            )
        );
        assert_eq!(
            require_frontier_advance(Some(3), 2),
            Err(
                StratumRegistryError::NonIncreasingClosedSectorLayerFrontier {
                    previous_active_count: 3,
                    incoming_active_count: 2,
                }
            )
        );
        assert_eq!(require_frontier_advance(Some(3), 4), Ok(()));
    }

    #[test]
    fn snapshot_identity_bytes_are_bounded_before_retention() {
        let mut limits = StratumRegistryLimits::default();
        limits.max_owner_identity_bytes = 0;
        assert_eq!(
            build_snapshot_id(
                "family",
                "context",
                1,
                &SnapshotSource::Empty,
                &[],
                &[],
                limits,
            )
            .unwrap_err(),
            StratumRegistryError::ResourceLimit {
                resource: "immutable owner identity bytes",
                requested: "rustred.immutable-owner-snapshot.v3:".len(),
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
