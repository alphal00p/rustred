/// Cardinality policy for one identity condition's deterministic source set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentityConditionLimits {
    pub diagnostics_enabled: bool,
    pub max_sources: usize,
}

impl Default for IdentityConditionLimits {
    fn default() -> Self {
        Self {
            diagnostics_enabled: false,
            max_sources: 65_536,
        }
    }
}

impl IdentityConditionLimits {
    pub fn with_diagnostics(mut self) -> Self {
        self.diagnostics_enabled = true;
        self
    }
}
