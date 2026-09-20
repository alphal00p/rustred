//! Install independently verified terminal equalities outside the hot path.

use std::collections::BTreeSet;

use crate::family::IntegralKey;
use crate::reduction::terminal_normalization::TerminalAliasPlan;

use super::{CandidateReducer, CandidateReductionError};

impl<const N: usize> CandidateReducer<N> {
    /// Install a sealed, exactly verified same-family terminal plan.
    ///
    /// The original terminal declarations remain unchanged. Future reductions
    /// coalesce equivalent output keys before retaining their decompositions in
    /// the cache. Installation requires an empty cache, preventing a mixture of
    /// old and new output conventions; an explicit `clear_cache` permits a new
    /// plan. Successful installation replaces any weighted normalization plan;
    /// rejected installation leaves both conventions unchanged.
    /// No source identity or completeness claim is added to this candidate
    /// owner by installing exact terminal equalities.
    pub fn install_terminal_aliases(
        &mut self,
        plan: TerminalAliasPlan,
    ) -> Result<(), CandidateReductionError> {
        if !self.cache.is_empty() {
            return Err(CandidateReductionError::InvalidInput(
                "terminal aliases require an empty reduction cache".into(),
            ));
        }
        if plan.family_fingerprint() != self.family_fingerprint() {
            return Err(CandidateReductionError::InvalidInput(
                "terminal aliases belong to another family".into(),
            ));
        }
        if plan.ordering() != self.ordering {
            return Err(CandidateReductionError::InvalidInput(
                "terminal aliases use another integral ordering".into(),
            ));
        }
        if plan.raw_terminals() != &self.terminals {
            return Err(CandidateReductionError::InvalidInput(
                "terminal aliases must cover exactly the declared raw terminal set".into(),
            ));
        }
        self.terminal_aliases = Some(plan);
        self.terminal_normalization = None;
        Ok(())
    }

    /// Verified optional output normalization, prepared once rather than per rule.
    pub fn terminal_aliases(&self) -> Option<&TerminalAliasPlan> {
        self.terminal_aliases.as_ref()
    }

    /// Possible output representatives. This is not an independent-master basis
    /// and does not replace the raw catalog/provenance contract of `terminals`.
    pub fn canonical_terminals(&self) -> &BTreeSet<IntegralKey> {
        if let Some(plan) = &self.terminal_normalization {
            return plan.canonical_terminals();
        }
        self.terminal_aliases
            .as_ref()
            .map_or(&self.terminals, TerminalAliasPlan::canonical_terminals)
    }
}
