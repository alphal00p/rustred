use super::super::relation::RelationLimits;

/// Resource policy for exact ordinary-IBP and Lorentz-identity construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ParametricIbpConfig {
    pub relation_limits: RelationLimits,
}
