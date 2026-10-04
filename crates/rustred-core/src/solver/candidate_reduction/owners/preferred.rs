//! Fresh, trusted rule-program composition; not terminal or closure authority.
use std::{collections::BTreeMap, sync::Arc};

use super::{CandidateOwnerContext, CandidateOwnerInput, CandidateOwnerPrograms};
use crate::solver::CandidateReductionError;

impl<const N: usize> CandidateOwnerPrograms<N> {
    /// Try preferred formulas first while retaining exactly the baseline's
    /// finite terminal boundary. Preferred-only residual points explicitly
    /// defer to the original program, even if a preferred formula overlaps.
    /// Only ordinary `Uncovered` rule evaluation falls through; other errors
    /// retain their existing meaning. Both inputs receive full native admission.
    ///
    /// A saved denominator exclusion remains native inapplicability and may
    /// yield `Uncovered`; actual algebra/source/descent/resource errors do not.
    /// Rule-local diagnostic APIs still inspect explicitly selected saved
    /// formulas, not this collection's dispatch priority.
    ///
    /// This builds a fresh immutable collection, not an append-lineage extension.
    /// It performs no source replay and confers no proof or closure authority.
    /// The caller must bind both trusted payloads and this dispatch policy when
    /// persisting the resulting collection. Empty preferences use `try_new`
    /// unchanged. Same-owner repair overlays are deliberately unsupported.
    pub fn try_new_with_preferences(
        context: Arc<CandidateOwnerContext<N>>,
        inputs: impl IntoIterator<Item = CandidateOwnerInput<N>>,
        preferences: impl IntoIterator<Item = CandidateOwnerInput<N>>,
    ) -> Result<Self, CandidateReductionError> {
        Self::try_new_with_preference_rule_subsets(context, inputs, preferences, &BTreeMap::new())
    }

    /// Fresh cold composition with an optional saved-ordinal subset per
    /// preferred owner. An absent owner entry enables every preferred rule;
    /// an empty slice enables none. Slices must be strictly increasing.
    ///
    /// EVERY input rule receives full native admission before selection. Only
    /// then are unselected prepared objects dropped, without renumbering or
    /// copying retained formulas. Original preferred residual holes and the
    /// baseline terminal boundary are independent of this selection. This is
    /// not a mutation, append-lineage update, source proof, or closure claim.
    pub fn try_new_with_preference_rule_subsets(
        context: Arc<CandidateOwnerContext<N>>,
        inputs: impl IntoIterator<Item = CandidateOwnerInput<N>>,
        preferences: impl IntoIterator<Item = CandidateOwnerInput<N>>,
        rule_subsets: &BTreeMap<[bool; N], &[usize]>,
    ) -> Result<Self, CandidateReductionError> {
        let mut baseline = Self::try_new(context.clone(), inputs)?;
        let mut preferences = preferences.into_iter().peekable();
        if preferences.peek().is_none() {
            if !rule_subsets.is_empty() {
                return Err(CandidateReductionError::InvalidInput(
                    "preferred rule subset has no corresponding preferred owner".into(),
                ));
            }
            return Ok(baseline);
        }
        let preferred = Self::try_new(context, preferences)?;
        for (sector, ordinals) in rule_subsets {
            let candidate = preferred.owners.get(sector).ok_or_else(|| {
                CandidateReductionError::InvalidInput(
                    "preferred rule subset has no corresponding preferred owner".into(),
                )
            })?;
            let rules = &candidate.batches[0].rules;
            if ordinals.len() > rules.len() || ordinals.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(CandidateReductionError::InvalidInput(
                    "preferred rule ordinals must be a bounded strictly increasing subset".into(),
                ));
            }
            // Admission assigns saved ordinals before any subset is considered.
            // Membership is checked against actual admitted IDs, not Vec slots.
            if ordinals.iter().any(|ordinal| {
                rules
                    .binary_search_by_key(ordinal, |rule| rule.ordinal)
                    .is_err()
            }) {
                return Err(CandidateReductionError::InvalidInput(
                    "preferred rule subset contains an unknown saved ordinal".into(),
                ));
            }
        }
        // Validate every binding before assembling any replacement. All native
        // rules, including declared residuals, were admitted by `try_new` above.
        for (sector, candidate) in &preferred.owners {
            let Some(original) = baseline.owners.get(sector) else {
                return Err(CandidateReductionError::InvalidInput(
                    "preferred program owner is missing from baseline".into(),
                ));
            };
            if candidate.root != original.root || candidate.ordering != original.ordering {
                return Err(CandidateReductionError::InvalidInput(
                    "preferred program root or mathematical order differs from baseline".into(),
                ));
            }
        }
        for (sector, candidate) in preferred.owners {
            // Both collections were just built locally. Move owned payloads:
            // no coefficient clones, extra native preparation or proof copies.
            let mut candidate = Arc::try_unwrap(candidate).expect("fresh preferred owner");
            let mut first = Arc::try_unwrap(candidate.batches.pop().expect("one fresh batch"))
                .expect("fresh preferred batch");
            if let Some(ordinals) = rule_subsets.get(&sector) {
                first
                    .rules
                    .retain(|rule| ordinals.binary_search(&rule.ordinal).is_ok());
                // These are private Vec positions/derived work bounds, NOT
                // saved provenance IDs. Rebuild both after compaction.
                first = super::model::PreparedOwnerBatch::new(
                    first.rules,
                    first.terminals,
                    first.overlay,
                );
            }
            let original = Arc::get_mut(baseline.owners.get_mut(&sector).expect("validated owner"))
                .expect("fresh baseline owner");
            let mut fallback = Arc::try_unwrap(original.batches.pop().expect("one fresh batch"))
                .expect("fresh baseline batch");
            let mut deferred = std::mem::take(&mut first.terminals);
            deferred.retain(|key| !fallback.terminals.contains(key));
            first.deferred_points = Some(deferred);
            first.terminals = std::mem::take(&mut fallback.terminals);
            original.batches = vec![Arc::new(first), Arc::new(fallback)];
        }
        Ok(baseline)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod subset_tests;
