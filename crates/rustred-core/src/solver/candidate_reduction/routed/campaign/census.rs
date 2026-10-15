//! Optional exact dispatch bookkeeping. No matching or algebra happens here.
use super::super::CandidateRoutedError;
use std::collections::BTreeMap;

/// Owner-qualified ordinals from the actual one-step evaluator selection.
/// `baseline_rule_ordinal` also records applicability provenance when a
/// whole-piece alternative replaces it; `selected_rule_ordinal` was applied.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CandidateRoutedSelectionIdentity {
    pub owner: Vec<bool>,
    pub batch_ordinal: usize,
    pub baseline_rule_ordinal: usize,
    pub selected_rule_ordinal: usize,
}

/// Optional census storage policy, not an algebraic or entry-domain bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateRoutedTraceCensusLimits {
    pub max_unique_selections: usize,
    /// Sum of stored identity/count payloads and owner coordinates. This does
    /// not claim to include BTreeMap allocator/node overhead or process RSS.
    /// The separate unique-selection allowance bounds that metadata too.
    pub max_identity_payload_bytes: usize,
}
impl Default for CandidateRoutedTraceCensusLimits {
    fn default() -> Self {
        Self {
            max_unique_selections: 1_000_000,
            max_identity_payload_bytes: 64 * 1024 * 1024,
        }
    }
}

/// A census is exposed as complete only through a successful, frontier-free
/// campaign report. Terminal and zero keys remain in its existing trace report.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CandidateRoutedTraceCensus {
    selections: BTreeMap<CandidateRoutedSelectionIdentity, u64>,
    identity_payload_bytes: usize,
}
impl CandidateRoutedTraceCensus {
    /// Successful concrete one-step applications in this deduplicated native
    /// invocation, grouped by actual composite selection. These multiplicities
    /// are not symbolic-domain classification occurrences or per-input totals.
    pub fn selections(&self) -> &BTreeMap<CandidateRoutedSelectionIdentity, u64> {
        &self.selections
    }
    pub fn identity_payload_bytes(&self) -> usize {
        self.identity_payload_bytes
    }

    /// Stage the complete cumulative census before any membership mutation.
    pub(super) fn merged(
        &self,
        delta: &Self,
        limits: CandidateRoutedTraceCensusLimits,
        check: &mut dyn FnMut() -> Result<(), CandidateRoutedError>,
    ) -> Result<Self, CandidateRoutedError> {
        let mut count = self.selections.len();
        let mut bytes = self.identity_payload_bytes;
        for (identity, addition) in &delta.selections {
            check()?;
            if let Some(old) = self.selections.get(identity) {
                old.checked_add(*addition).ok_or_else(|| {
                    limit(
                        "trace census selection multiplicity",
                        usize::MAX,
                        usize::MAX,
                    )
                })?;
            } else {
                count = count.checked_add(1).ok_or_else(|| {
                    limit(
                        "trace census unique selections",
                        usize::MAX,
                        limits.max_unique_selections,
                    )
                })?;
                bytes = bytes
                    .checked_add(std::mem::size_of::<(CandidateRoutedSelectionIdentity, u64)>())
                    .and_then(|n| n.checked_add(identity.owner.len()))
                    .ok_or_else(|| {
                        limit(
                            "trace census identity payload bytes",
                            usize::MAX,
                            limits.max_identity_payload_bytes,
                        )
                    })?;
            }
        }
        if count > limits.max_unique_selections {
            return Err(limit(
                "trace census unique selections",
                count,
                limits.max_unique_selections,
            ));
        }
        if bytes > limits.max_identity_payload_bytes {
            return Err(limit(
                "trace census identity payload bytes",
                bytes,
                limits.max_identity_payload_bytes,
            ));
        }
        let mut merged = Self::default();
        for (identity, count) in &self.selections {
            check()?;
            merged.selections.insert(identity.clone(), *count);
        }
        for (identity, addition) in &delta.selections {
            check()?;
            *merged.selections.entry(identity.clone()).or_default() += addition;
        }
        merged.identity_payload_bytes = bytes;
        check()?;
        Ok(merged)
    }

    pub(super) fn record(
        &mut self,
        owner: &[bool],
        batch: usize,
        baseline_rule: usize,
        selected_rule: usize,
        limits: CandidateRoutedTraceCensusLimits,
    ) -> Result<(), CandidateRoutedError> {
        // Allocate at most one bounded physical-arity key, never a formula or
        // a row for every visited integral. Existing identity counts coalesce.
        let payload = std::mem::size_of::<(CandidateRoutedSelectionIdentity, u64)>()
            .checked_add(owner.len())
            .ok_or_else(|| {
                limit(
                    "trace census identity payload bytes",
                    usize::MAX,
                    limits.max_identity_payload_bytes,
                )
            })?;
        if payload > limits.max_identity_payload_bytes {
            return Err(limit(
                "trace census identity payload bytes",
                payload,
                limits.max_identity_payload_bytes,
            ));
        }
        let identity = CandidateRoutedSelectionIdentity {
            owner: owner.to_vec(),
            batch_ordinal: batch,
            baseline_rule_ordinal: baseline_rule,
            selected_rule_ordinal: selected_rule,
        };
        if let Some(count) = self.selections.get_mut(&identity) {
            *count = count.checked_add(1).ok_or_else(|| {
                limit(
                    "trace census selection multiplicity",
                    usize::MAX,
                    usize::MAX,
                )
            })?;
            return Ok(());
        }
        let count = self.selections.len().checked_add(1).ok_or_else(|| {
            limit(
                "trace census unique selections",
                usize::MAX,
                limits.max_unique_selections,
            )
        })?;
        if count > limits.max_unique_selections {
            return Err(limit(
                "trace census unique selections",
                count,
                limits.max_unique_selections,
            ));
        }
        let bytes = self
            .identity_payload_bytes
            .checked_add(payload)
            .ok_or_else(|| {
                limit(
                    "trace census identity payload bytes",
                    usize::MAX,
                    limits.max_identity_payload_bytes,
                )
            })?;
        if bytes > limits.max_identity_payload_bytes {
            return Err(limit(
                "trace census identity payload bytes",
                bytes,
                limits.max_identity_payload_bytes,
            ));
        }
        self.selections.insert(identity, 1);
        self.identity_payload_bytes = bytes;
        Ok(())
    }
}
fn limit(resource: &'static str, requested: usize, limit: usize) -> CandidateRoutedError {
    CandidateRoutedError::ResourceLimit {
        resource,
        requested,
        limit,
    }
}

#[cfg(test)]
mod tests;
