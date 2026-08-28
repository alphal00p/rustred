//! Sealed capability for the one admitted committed-source authority copy.

/// Move-only proof that one shallow committed-source copy is being created for
/// fresh-session authority assembly.
///
/// The private field and parent-only constructor prevent publication-epoch
/// code from minting this capability. Deliberately do not implement `Clone`,
/// `Copy`, or `Default`.
pub(in crate::solver::closure) struct CommittedExceptionalAuthorityCopyPermit {
    _private: (),
}

impl CommittedExceptionalAuthorityCopyPermit {
    pub(super) const fn new() -> Self {
        Self { _private: () }
    }
}
