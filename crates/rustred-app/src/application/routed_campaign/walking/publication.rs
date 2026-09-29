//! Publication scheduling is not mathematical applicability authority.
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
    /// depth 1 with canonical in-merge resolution; no resumable checkpoint.
    Epoch,
}
