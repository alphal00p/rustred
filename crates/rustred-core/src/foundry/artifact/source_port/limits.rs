use crate::foundry::parametric::ParametricRuleLimits;

pub(crate) const DEFAULT_PREDICATE_CONSISTENCY_WORK: usize = 4_194_304;

/// Caller-owned resource policy for source-port audit and publication.
///
/// These limits never enter artifact bytes or mathematical evidence. Durable
/// loading selects its own policy through `ArtifactLoadLimits`. Raising a
/// limit permits more exact verification work; it never omits a proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourcePortLimits {
    pub rule_derivation: ParametricRuleLimits,
    /// Native affine consistency work allowed per complete predicate-cover
    /// traversal, shared by all of its boxes and Boolean branches. Stored,
    /// replay-checked, and final installed covers are independent traversals.
    /// Zero permits no native work; it does not mean unlimited work.
    pub max_predicate_consistency_work: usize,
}

impl Default for SourcePortLimits {
    fn default() -> Self {
        Self {
            rule_derivation: ParametricRuleLimits::default(),
            max_predicate_consistency_work: DEFAULT_PREDICATE_CONSISTENCY_WORK,
        }
    }
}
