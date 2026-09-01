//! Explicit retained-work bounds for one cold routing compilation.

/// Resource policy around sign-gauge enumeration and the retained recurrence.
/// Matrix arithmetic remains subject to the authenticated family's own
/// Symbolica limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FactorizedNumeratorLiftLimits {
    pub(crate) max_arity: usize,
    pub(crate) max_sign_gauges: usize,
    pub(crate) max_recurrence_branches: usize,
}

impl Default for FactorizedNumeratorLiftLimits {
    fn default() -> Self {
        Self {
            // K=21 at six loops is comfortably inside this structural bound.
            max_arity: 128,
            // Row signs are a finite 2^L gauge portfolio.  This default admits
            // twelve loops while keeping compilation explicitly bounded.
            max_sign_gauges: 4_096,
            max_recurrence_branches: 129,
        }
    }
}
