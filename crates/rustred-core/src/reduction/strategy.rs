/// Execution strategy over the same authenticated reduction programs.
///
/// Immutable mathematical preparation is shared in every mode. Completed
/// integral reductions and their cache bookkeeping are used only by ordinary
/// depth-first execution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReductionStrategy {
    #[default]
    DepthFirst,
    /// Preserve depth-first occurrence order, consuming each child result once.
    /// No completed-result lookup, insertion, persistence or capture is allowed.
    DepthFirstWithoutMemoization,
    Ordered,
}

impl ReductionStrategy {
    pub(crate) fn uses_completed_cache(self) -> bool {
        self == Self::DepthFirst
    }
}
