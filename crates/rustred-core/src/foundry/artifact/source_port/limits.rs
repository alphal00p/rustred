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
    /// Enforce operational count templates only for explicit diagnostics.
    pub diagnostics_enabled: bool,
    pub rule_cells: crate::foundry::cell::RuleCellLimits,
    pub rule_derivation: ParametricRuleLimits,
    /// Exact geometric work permitted during original-domain lowering and
    /// final complete-cover installation. This policy is independent of
    /// algebraic row/column limits. Its default matches durable loading; a
    /// caller's smaller limit, including zero, is never increased implicitly.
    pub cover_replay: ArtifactCoverReplayLimits,
    /// Native affine consistency work allowed per complete predicate-cover
    /// traversal, shared by all of its boxes and Boolean branches. Stored,
    /// replay-checked, and final installed covers are independent traversals.
    /// With diagnostics enabled, zero permits no native work.
    pub max_predicate_consistency_work: usize,
    /// Distinct affine atoms admitted per complete predicate-cover traversal.
    /// With diagnostics enabled, zero admits coordinate-only covers. The
    /// iterative Boolean traversal has no fixed atom-depth ceiling.
    pub max_predicate_atoms: usize,
}

impl SourcePortLimits {
    pub fn with_diagnostics(mut self) -> Self {
        self.diagnostics_enabled = true;
        self.cover_replay = self.cover_replay.with_diagnostics();
        self.rule_derivation = self.rule_derivation.with_diagnostics();
        self.rule_cells = self.rule_cells.with_diagnostics();
        self
    }

    /// Validate the caller's shared resource scope before proof work.
    pub fn validate(&self) -> Result<(), super::SourcePortAuditError> {
        symbolica::utils::resources::checkpoint().map_err(super::SourcePortAuditError::Resources)
    }
}

impl Default for SourcePortLimits {
    fn default() -> Self {
        Self {
            diagnostics_enabled: false,
            rule_cells: Default::default(),
            rule_derivation: ParametricRuleLimits::default(),
            cover_replay: ArtifactCoverReplayLimits::default(),
            max_predicate_consistency_work: DEFAULT_PREDICATE_CONSISTENCY_WORK,
            max_predicate_atoms: DEFAULT_PREDICATE_ATOMS,
        }
    }
}
