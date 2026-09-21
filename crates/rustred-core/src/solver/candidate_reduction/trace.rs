//! Bounded successor inspection through the existing exact candidate evaluator.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::family::IntegralKey;
use crate::reduction::{ReductionError, ReductionRequest};

use super::{CandidateReducer, CandidateReductionError};

/// Additional graph limits for candidate successor inspection. Existing exact
/// coefficient, pending-frame, application and coalescing limits also apply.
/// These caps do not bound every transient allocation inside native algebra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateTraceLimits {
    /// Counts iterator entries, including duplicates, before retaining them.
    pub max_input_targets: usize,
    /// Counts distinct inputs and recursively scheduled nonzero successors.
    pub max_unique_integrals: usize,
}

impl Default for CandidateTraceLimits {
    fn default() -> Self {
        Self {
            max_input_targets: 100_000,
            max_unique_integrals: 100_000,
        }
    }
}

/// Exact candidate successor inspection, not original-source replay or a
/// coverage certificate. Uncovered keys have no currently applicable formula;
/// they are not inferred masters. Coefficients coalesce within each applied
/// RHS, but are not back-substituted across paths, so some frontier terms may
/// cancel in a later complete reduction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateTraceReport {
    family_fingerprint: Arc<String>,
    input_targets: usize,
    requested_targets: usize,
    reachable_integrals: usize,
    rule_applications: usize,
    uncovered: BTreeSet<IntegralKey>,
    declared_terminals: BTreeSet<IntegralKey>,
    visited_zeros: BTreeSet<IntegralKey>,
    max_negative_index_degree: u128,
    max_dot_excess: u128,
    max_positive_power_sum: u128,
}

impl CandidateTraceReport {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    /// Iterator entries consumed, including duplicates.
    pub fn input_targets(&self) -> usize {
        self.input_targets
    }
    /// Distinct supplied entries.
    pub fn requested_targets(&self) -> usize {
        self.requested_targets
    }
    pub fn reachable_integrals(&self) -> usize {
        self.reachable_integrals
    }
    /// Successful applications, including rules with an empty coalesced RHS.
    pub fn rule_applications(&self) -> usize {
        self.rule_applications
    }
    pub fn uncovered(&self) -> &BTreeSet<IntegralKey> {
        &self.uncovered
    }
    /// Raw declared terminal keys; output aliases/normalizations do not alter
    /// the candidate graph's terminal convention.
    pub fn declared_terminals(&self) -> &BTreeSet<IntegralKey> {
        &self.declared_terminals
    }
    /// Explicitly visited zero entries. The shared evaluator already removes
    /// zero-sector children, so those discarded edges are not counted here.
    pub fn visited_zeros(&self) -> &BTreeSet<IntegralKey> {
        &self.visited_zeros
    }
    /// Maximum sum(max(-n_i, 0)) over reached keys, not just the entry scope.
    pub fn max_negative_index_degree(&self) -> u128 {
        self.max_negative_index_degree
    }
    /// Maximum sum(max(n_i - 1, 0)) over the same reached keys.
    pub fn max_dot_excess(&self) -> u128 {
        self.max_dot_excess
    }
    /// Maximum sum(max(n_i, 0)) over the same reached keys.
    pub fn max_positive_power_sum(&self) -> u128 {
        self.max_positive_power_sum
    }

    fn record_degrees(&mut self, key: &IntegralKey) -> Result<(), CandidateReductionError> {
        let mut negative = 0_u128;
        let mut dots = 0_u128;
        let mut positive = 0_u128;
        let overflow = || {
            CandidateReductionError::InvalidInput("candidate trace degree census overflowed".into())
        };
        for &n in key.powers() {
            if n < 0 {
                negative = negative
                    .checked_add(u128::from(n.unsigned_abs()))
                    .ok_or_else(overflow)?;
            } else {
                let n = n as u128;
                positive = positive.checked_add(n).ok_or_else(overflow)?;
                dots = dots.checked_add(n.saturating_sub(1)).ok_or_else(overflow)?;
            }
        }
        self.max_negative_index_degree = self.max_negative_index_degree.max(negative);
        self.max_dot_excess = self.max_dot_excess.max(dots);
        self.max_positive_power_sum = self.max_positive_power_sum.max(positive);
        Ok(())
    }
}

fn schedule(
    target: IntegralKey,
    seen: &mut BTreeSet<IntegralKey>,
    pending: &mut BTreeSet<IntegralKey>,
    request: &mut ReductionRequest,
    unique_limit: usize,
    pending_limit: usize,
) -> Result<(), CandidateReductionError> {
    if seen.contains(&target) {
        return Ok(());
    }
    let requested =
        seen.len()
            .checked_add(1)
            .ok_or(CandidateReductionError::TraceIntegralLimit {
                requested: usize::MAX,
                limit: unique_limit,
            })?;
    if requested > unique_limit {
        return Err(CandidateReductionError::TraceIntegralLimit {
            requested,
            limit: unique_limit,
        });
    }
    request.retain_pending_frame(pending_limit)?;
    seen.insert(target.clone());
    pending.insert(target);
    Ok(())
}

impl<const N: usize> CandidateReducer<N> {
    /// Follow every encountered nonzero successor, preserving missing-rule
    /// keys as a sorted frontier. Only `Uncovered` is recoverable; malformed
    /// formulas, source-condition failures and every resource error abort.
    /// One aggregate request budget spans all entries and descendants.
    ///
    /// This performs no IBP search, terminal inference or back-substitution.
    /// It neither reads nor modifies the decomposition cache. Exact application
    /// and coalescing work can update the existing statistics. Even an empty
    /// frontier is not original-source validation or a family certificate.
    pub fn trace_targets(
        &mut self,
        targets: impl IntoIterator<Item = IntegralKey>,
        limits: CandidateTraceLimits,
    ) -> Result<CandidateTraceReport, CandidateReductionError> {
        let mut seen = BTreeSet::new();
        let mut pending = BTreeSet::new();
        let mut request = ReductionRequest::default();
        let mut input_targets = 0_usize;
        for target in targets {
            input_targets =
                input_targets
                    .checked_add(1)
                    .ok_or(CandidateReductionError::TraceInputLimit {
                        requested: usize::MAX,
                        limit: limits.max_input_targets,
                    })?;
            if input_targets > limits.max_input_targets {
                return Err(CandidateReductionError::TraceInputLimit {
                    requested: input_targets,
                    limit: limits.max_input_targets,
                });
            }
            self.validate_entry_target(&target)?;
            schedule(
                target,
                &mut seen,
                &mut pending,
                &mut request,
                limits.max_unique_integrals,
                self.limits.max_pending_frames,
            )?;
        }
        let mut report = CandidateTraceReport {
            family_fingerprint: self.family_fingerprint.clone(),
            input_targets,
            requested_targets: seen.len(),
            reachable_integrals: 0,
            rule_applications: 0,
            uncovered: BTreeSet::new(),
            declared_terminals: BTreeSet::new(),
            visited_zeros: BTreeSet::new(),
            max_negative_index_degree: 0,
            max_dot_excess: 0,
            max_positive_power_sum: 0,
        };
        while let Some(target) = pending.pop_first() {
            request.release_pending_frame()?;
            self.validate_target(&target)?;
            report.record_degrees(&target)?;
            if self.is_zero(&target) {
                report.visited_zeros.insert(target);
                continue;
            }
            if self.terminals.contains(&target) {
                report.declared_terminals.insert(target);
                continue;
            }
            request.record_rule_application(self.limits.max_rule_applications)?;
            // Shared application checks every physical edge before scheduling,
            // including edges to keys already encountered through another path.
            match self.apply_candidate(&target, &mut request) {
                Ok(terms) => {
                    self.statistics.record_rule_application();
                    report.rule_applications = report.rule_applications.checked_add(1).ok_or(
                        ReductionError::RuleApplicationLimit {
                            requested: usize::MAX,
                            limit: self.limits.max_rule_applications,
                        },
                    )?;
                    for (child, _) in terms {
                        schedule(
                            child,
                            &mut seen,
                            &mut pending,
                            &mut request,
                            limits.max_unique_integrals,
                            self.limits.max_pending_frames,
                        )?;
                    }
                }
                Err(CandidateReductionError::Uncovered { target }) => {
                    report.uncovered.insert(target);
                }
                Err(error) => return Err(error),
            }
        }
        report.reachable_integrals = seen.len();
        Ok(report)
    }
}
