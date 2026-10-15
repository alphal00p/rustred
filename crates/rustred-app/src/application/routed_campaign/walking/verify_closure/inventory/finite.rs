//! Exact completed-trace census, separate from symbolic-cover occurrences.
use super::*;
use crate::application::routed_campaign::walking::finite_replay::{CensusLimits, CompletedCensus};
use rustred::solver::CandidateRoutedTraceCensusLimits;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(super) struct Selection {
    pub owner: Vec<bool>,
    pub batch_ordinal: usize,
    pub baseline_rule_ordinal: usize,
    pub selected_rule_ordinal: usize,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub(super) struct State {
    pub selections: BTreeMap<Selection, u64>,
    pub invocations: u64,
    pub rule_applications: u64,
    pub terminal_keys_per_invocation: u64,
    pub zero_keys_per_invocation: u64,
}

impl Collector {
    pub(in crate::application::routed_campaign::walking::verify_closure) fn finite_limits(
        &self,
    ) -> CensusLimits {
        CensusLimits {
            rules: CandidateRoutedTraceCensusLimits {
                max_unique_selections: self.max_unique_rules,
                ..Default::default()
            },
            max_terminal_keys: self.max_unique_terminals,
        }
    }

    pub(in crate::application::routed_campaign::walking::verify_closure) fn record_finite(
        &self,
        census: CompletedCensus,
        cancellation: &AtomicBool,
    ) -> Result<(), AppError> {
        if cancellation.load(std::sync::atomic::Ordering::Acquire) {
            return Err(AppError::execution(
                "finite inventory cancelled before publication",
            ));
        }
        let mut state = self.state.lock().expect("finite inventory merge");
        let arity = state
            .family
            .as_ref()
            .ok_or_else(|| AppError::internal_invariant("finite inventory family not prepared"))?
            .denominator_count();
        let mut finite = state.finite.take().unwrap_or_default();
        let result = (|| {
            for (identity, &count) in census.rules.selections() {
                if identity.owner.len() != arity || count == 0 {
                    return Err(AppError::internal_invariant(
                        "finite native selection has invalid owner or multiplicity",
                    ));
                }
                let selection = Selection {
                    owner: identity.owner.clone(),
                    batch_ordinal: identity.batch_ordinal,
                    baseline_rule_ordinal: identity.baseline_rule_ordinal,
                    selected_rule_ordinal: identity.selected_rule_ordinal,
                };
                if !finite.selections.contains_key(&selection)
                    && finite.selections.len() >= self.max_unique_rules
                {
                    return Err(AppError::limit(
                        "finite composite selection census exceeds inventory limit",
                    ));
                }
                let total = finite.selections.entry(selection).or_default();
                *total = total
                    .checked_add(count)
                    .ok_or_else(|| AppError::limit("finite selection multiplicity overflow"))?;
                finite.rule_applications = finite
                    .rule_applications
                    .checked_add(count)
                    .ok_or_else(|| AppError::limit("finite rule application count overflow"))?;
                let selected = RuleIdentity {
                    owner: identity.owner.clone(),
                    batch: identity.batch_ordinal,
                    rule: identity.selected_rule_ordinal,
                };
                if !state.rules.contains_key(&selected)
                    && state.rules.len() >= self.max_unique_rules
                {
                    return Err(AppError::limit(
                        "unique encountered rules exceed inventory limit",
                    ));
                }
                // Zero means no symbolic cover classification. Finite uses are
                // kept separately; do not mix these fundamentally different units.
                state.rules.entry(selected).or_default();
            }
            let terminal_count = u64::try_from(census.terminals.len())
                .map_err(|_| AppError::limit("finite terminal count range"))?;
            for key in census.terminals {
                if key.powers().len() != arity {
                    return Err(AppError::internal_invariant(
                        "finite terminal arity differs",
                    ));
                }
                if !state.terminals.contains(&key)
                    && state.terminals.len() >= self.max_unique_terminals
                {
                    return Err(AppError::limit(
                        "unique encountered terminals exceed inventory limit",
                    ));
                }
                state.terminals.insert(key);
            }
            finite.invocations = finite
                .invocations
                .checked_add(1)
                .ok_or_else(|| AppError::limit("finite inventory invocation overflow"))?;
            finite.terminal_keys_per_invocation = finite
                .terminal_keys_per_invocation
                .checked_add(terminal_count)
                .ok_or_else(|| AppError::limit("finite terminal count overflow"))?;
            finite.zero_keys_per_invocation = finite
                .zero_keys_per_invocation
                .checked_add(
                    u64::try_from(census.zero_keys)
                        .map_err(|_| AppError::limit("finite zero count range"))?,
                )
                .ok_or_else(|| AppError::limit("finite zero count overflow"))?;
            if cancellation.load(std::sync::atomic::Ordering::Acquire) {
                return Err(AppError::execution(
                    "finite inventory cancelled before completion",
                ));
            }
            Ok(())
        })();
        state.finite = Some(finite);
        if let Err(error) = &result {
            state.error = Some(AppError::new(error.kind(), error.to_string()));
        }
        result
    }
}

impl State {
    pub(super) fn summary(&self) -> Value {
        json!({"invocations":self.invocations,"unique_composite_selections":self.selections.len(),
            "rule_applications":self.rule_applications,"terminal_keys_per_invocation":self.terminal_keys_per_invocation,
            "zero_keys_per_invocation":self.zero_keys_per_invocation,
            "units":"successful concrete local applications in deduplicated finite replays; not symbolic cover classification occurrences",
            "replay":"authenticated whole finite recipe, exact native kernel, no symbolic reuse shortcuts",
            "source_and_guard_authority":"original immutable owner programs; census does not replace applicability conditions"})
    }
    pub(super) fn selected_uses(&self, id: &RuleIdentity) -> u64 {
        self.selections
            .iter()
            .filter(|(key, _)| {
                key.owner == id.owner
                    && key.batch_ordinal == id.batch
                    && key.selected_rule_ordinal == id.rule
            })
            .map(|(_, count)| *count)
            .sum()
    }
}
