//! Publication scheduling is not mathematical applicability authority.
/// Experimental placement of Epoch's exact merged-store lookup. This changes
/// work accounting, not applicability authority; default behavior is unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OwnerDomainWalkEpochInspectorLookup {
    #[default]
    AllMiss,
    Snapshot,
}
impl OwnerDomainWalkEpochInspectorLookup {
    pub fn name(self) -> &'static str {
        match self {
            Self::AllMiss => "all-miss",
            Self::Snapshot => "snapshot",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "all-miss" => Some(Self::AllMiss),
            "snapshot" => Some(Self::Snapshot),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OwnerDomainWalkPublicationPolicy {
    #[default]
    Ordered,
    /// One queue and serialized admission, with fair ready-ticket publication.
    /// Requires explicit TransferUnreserved scheduling; IDs/covers may vary.
    Ready,
    /// Independent owner queues. Checkpoints and subdivision are unsupported.
    OwnerBatched,
    /// Walk semantics 3 (W2 epoch engine): whole-inspection commit, bulk
    /// merges by one coordinator, IDs assigned at merge. Stage S2: Lockstep
    /// depth1 with canonical in-merge resolution; explicit checkpoints use CP6.
    Epoch,
}
