//! Publication scheduling is not mathematical applicability authority.
/// Which completed inspections may enter the next bounded rolling cut.
/// Both policies use the same exact P1/P2/P3 authority. OldestReady can change
/// IDs and reuse geometry; the choice is immutable across checkpoint resume.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OwnerDomainWalkEpochPublicationOrder {
    #[default]
    OldestPrefix,
    OldestReady,
}
impl OwnerDomainWalkEpochPublicationOrder {
    pub fn name(self) -> &'static str {
        match self {
            Self::OldestPrefix => "oldest-prefix",
            Self::OldestReady => "oldest-ready",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "oldest-prefix" => Some(Self::OldestPrefix),
            "oldest-ready" => Some(Self::OldestReady),
            _ => None,
        }
    }
    pub(crate) fn is_default(&self) -> bool {
        *self == Self::OldestPrefix
    }
    pub(crate) fn report_name(self) -> &'static str {
        match self {
            // Keep the existing default report contract unchanged.
            Self::OldestPrefix => "oldest_sequence_prefix",
            Self::OldestReady => "oldest_ready_sequences",
        }
    }
}

/// Pending-job selection within a rolling Epoch campaign. Observations may
/// change priority, never whether an obligation must be discharged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OwnerDomainWalkEpochDispatchPolicy {
    #[default]
    Fifo,
    Adaptive,
}
impl OwnerDomainWalkEpochDispatchPolicy {
    pub fn name(self) -> &'static str {
        match self {
            Self::Fifo => "fifo",
            Self::Adaptive => "adaptive",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "fifo" => Some(Self::Fifo),
            "adaptive" => Some(Self::Adaptive),
            _ => None,
        }
    }
}

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
