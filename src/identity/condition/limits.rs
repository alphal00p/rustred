/// Cardinality policy for one identity condition's deterministic source set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentityConditionLimits {
    pub max_sources: usize,
}

impl Default for IdentityConditionLimits {
    fn default() -> Self {
        Self {
            max_sources: 65_536,
        }
    }
}
