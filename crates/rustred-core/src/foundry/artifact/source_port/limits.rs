use crate::foundry::artifact::ArtifactCoverReplayLimits;
use crate::foundry::parametric::ParametricRuleLimits;

pub(crate) const DEFAULT_PREDICATE_CONSISTENCY_WORK: usize = 4_194_304;
pub(crate) const DEFAULT_PREDICATE_ATOMS: usize = 32;

/// Caller-owned resource policy for source-port audit and publication.
///
/// These limits never enter artifact bytes or mathematical evidence. Durable
/// loading selects its own policy through `ArtifactLoadLimits`. Raising a
/// limit permits more exact verification work; it never omits a proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourcePortLimits {
    pub rule_derivation: ParametricRuleLimits,
    /// Exact geometric work permitted during original-domain lowering and
    /// final complete-cover installation. This policy is independent of
    /// algebraic row/column limits. Its default matches durable loading; a
    /// caller's smaller limit, including zero, is never increased implicitly.
    pub cover_replay: ArtifactCoverReplayLimits,
    /// Native affine consistency work allowed per complete predicate-cover
    /// traversal, shared by all of its boxes and Boolean branches. Stored,
    /// replay-checked, and final installed covers are independent traversals.
    /// Zero permits no native work; it does not mean unlimited work.
    pub max_predicate_consistency_work: usize,
    /// Distinct affine atoms admitted per complete predicate-cover traversal.
    /// Zero admits coordinate-only covers. The supported ceiling bounds the
    /// depth of the exact Boolean traversal; it is not an authority shortcut.
    pub max_predicate_atoms: usize,
}

impl SourcePortLimits {
    /// Supported atom-policy ceiling for the bounded recursive cover walker.
    /// Native affine consistency retains its independent equation/work caps.
    pub const MAX_PREDICATE_ATOMS: usize = 256;

    /// Validate caller policy before beginning audit or publication work.
    pub fn validate(&self) -> Result<(), super::SourcePortAuditError> {
        if self.max_predicate_atoms > Self::MAX_PREDICATE_ATOMS {
            return Err(super::SourcePortAuditError::UnsupportedResourcePolicy {
                resource: "predicate atoms",
                requested: self.max_predicate_atoms,
                supported_max: Self::MAX_PREDICATE_ATOMS,
            });
        }
        Ok(())
    }
}

impl Default for SourcePortLimits {
    fn default() -> Self {
        Self {
            rule_derivation: ParametricRuleLimits::default(),
            cover_replay: ArtifactCoverReplayLimits::default(),
            max_predicate_consistency_work: DEFAULT_PREDICATE_CONSISTENCY_WORK,
            max_predicate_atoms: DEFAULT_PREDICATE_ATOMS,
        }
    }
}
