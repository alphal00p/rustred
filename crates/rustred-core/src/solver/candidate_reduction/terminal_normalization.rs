//! Once-only installation of a finite, flat weighted terminal convention.

use crate::reduction::terminal_normalization::{
    TerminalNormalizationPlan, TerminalNormalizationStatistics,
};

use super::{CandidateReducer, CandidateReductionError};

impl<const N: usize> CandidateReducer<N> {
    /// Install exact finite terminal identities without changing candidate
    /// source authority or completeness. Raw catalog declarations remain fixed.
    /// An empty cache is mandatory. Successful installation replaces a previous
    /// unit-alias plan; any rejected installation leaves the owner unchanged.
    pub fn install_terminal_normalization(
        &mut self,
        plan: TerminalNormalizationPlan,
    ) -> Result<(), CandidateReductionError> {
        let invalid = |message: &str| CandidateReductionError::InvalidInput(message.into());
        if !self.cache.is_empty() {
            return Err(invalid(
                "terminal normalization requires an empty reduction cache",
            ));
        }
        if plan.family_fingerprint() != self.family_fingerprint() {
            return Err(invalid("terminal normalization belongs to another family"));
        }
        if plan.ordering() != self.ordering {
            return Err(invalid(
                "terminal normalization uses another integral ordering",
            ));
        }
        if plan.raw_terminals() != &self.terminals {
            return Err(invalid(
                "terminal normalization must cover exactly the declared raw terminal set",
            ));
        }
        for row in plan.terms().values() {
            for coefficient in row.values() {
                self.context
                    .base()
                    .validate_with_limits(coefficient, self.limits.exact_algebra)
                    .map_err(|e| invalid(&e.to_string()))?;
            }
        }
        self.terminal_normalization = Some(plan);
        self.terminal_aliases = None;
        Ok(())
    }

    pub fn terminal_normalization(&self) -> Option<&TerminalNormalizationPlan> {
        self.terminal_normalization.as_ref()
    }

    pub fn terminal_normalization_statistics(&self) -> Option<&TerminalNormalizationStatistics> {
        self.terminal_normalization
            .as_ref()
            .map(TerminalNormalizationPlan::statistics)
    }
}
