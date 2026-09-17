use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::algebra::{Coefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::family::IntegralKey;
use crate::reduction::{
    CacheWeight, ReductionError, ReductionLimits, ReductionRequest, ReductionStatistics,
    SharedCacheBudget, accumulate_master_in_request, coefficient_cache_weight,
};
use crate::sector::{OrderingPolicy, zero};

use super::model::{
    CandidateDecomposition, CandidateReachabilityReport, CandidateReductionError,
    CandidateStatistics, PreparedRule,
};

/// Deterministic, iterative application of explicitly experimental sector
/// formulas. This type cannot be converted to a closed artifact. It never
/// invents a terminal to hide a gap or invokes a source-generation campaign.
#[derive(Debug)]
pub struct CandidateReducer<const N: usize> {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) context: IndexedCoefficientContext,
    pub(super) root_sector: [bool; N],
    pub(super) ordering: OrderingPolicy,
    pub(super) rules: BTreeMap<[bool; N], Vec<PreparedRule<N>>>,
    pub(super) terminals: BTreeSet<IntegralKey>,
    pub(super) zero_sectors: BTreeSet<[bool; N]>,
    pub(super) _zero_certificates: Vec<zero::Certificate>,
    pub(super) source_conditions: Vec<IndexedPolynomial>,
    pub(super) limits: ReductionLimits,
    pub(super) cache: BTreeMap<IntegralKey, CandidateDecomposition>,
    pub(super) cache_weight: CacheWeight,
    pub(super) cache_budget: SharedCacheBudget,
    pub(super) statistics: ReductionStatistics,
}

impl<const N: usize> CandidateReducer<N> {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn index_count(&self) -> usize {
        N
    }
    pub fn terminals(&self) -> &BTreeSet<IntegralKey> {
        &self.terminals
    }
    pub fn limits(&self) -> ReductionLimits {
        self.limits
    }
    pub fn statistics(&self) -> CandidateStatistics {
        let census = self.cache_budget.snapshot();
        CandidateStatistics {
            work: self.statistics,
            cached_integrals: census.integrals,
            cached_coefficient_terms: census.coefficient_terms,
            cached_coefficient_bytes: census.coefficient_bytes,
        }
    }

    /// Apply checked formulas at one concrete integer point, returning only
    /// declared finite terminals. The symbolic base parameters (including d)
    /// remain exact and generic; numerical specialization is an adapter task.
    pub fn reduce_unit_mass(
        &mut self,
        target: &IntegralKey,
    ) -> Result<CandidateDecomposition, CandidateReductionError> {
        self.validate_target(target)?;
        if let Some(cached) = self.cache.get(target) {
            self.statistics.record_cache_hit();
            return Ok(cached.clone());
        }
        let mut request = ReductionRequest::default();
        let mut stack = Vec::new();
        let mut active = BTreeSet::new();
        self.push_frame(&mut stack, Frame::Expand(target.clone()), &mut request)?;
        while let Some(frame) = stack.pop() {
            request.release_pending_frame()?;
            match frame {
                Frame::Expand(key) => {
                    if self.cache.contains_key(&key) {
                        self.statistics.record_cache_hit();
                        continue;
                    }
                    self.validate_target(&key)?;
                    if self.is_zero(&key) {
                        self.cache_insert(key, BTreeMap::new())?;
                        continue;
                    }
                    if self.terminals.contains(&key) {
                        let terms = BTreeMap::from([(key.clone(), self.context.base().one())]);
                        self.cache_insert(key, terms)?;
                        continue;
                    }
                    if !active.insert(key.clone()) {
                        return Err(ReductionError::CycleDetected { target: key }.into());
                    }
                    request.record_rule_application(self.limits.max_rule_applications)?;
                    let terms = self.apply_candidate(&key, &mut request)?;
                    self.statistics.record_rule_application();
                    let mut children = Vec::new();
                    children.try_reserve_exact(terms.len()).map_err(|_| {
                        ReductionError::AllocationFailure {
                            resource: "candidate child schedule",
                            requested: terms.len(),
                        }
                    })?;
                    children.extend(terms.keys().cloned());
                    self.push_frame(
                        &mut stack,
                        Frame::Combine { target: key, terms },
                        &mut request,
                    )?;
                    for child in children.into_iter().rev() {
                        if !self.cache.contains_key(&child) {
                            self.push_frame(&mut stack, Frame::Expand(child), &mut request)?;
                        }
                    }
                }
                Frame::Combine { target, terms } => {
                    let mut output = BTreeMap::new();
                    for (child, factor) in terms {
                        let child =
                            self.cache
                                .get(&child)
                                .ok_or(ReductionError::ReducerInvariant {
                                    detail: "candidate child is absent at combine frame",
                                })?;
                        for (terminal, coefficient) in child.terms() {
                            let contribution = self.context.base().try_mul(
                                &factor,
                                coefficient,
                                self.limits.exact_algebra,
                            )?;
                            accumulate_master_in_request(
                                self.context.base(),
                                &mut output,
                                terminal,
                                contribution,
                                self.limits,
                                &mut request,
                                &mut self.statistics,
                            )?;
                        }
                    }
                    if !active.remove(&target) {
                        return Err(ReductionError::ReducerInvariant {
                            detail: "candidate combine target is not active",
                        }
                        .into());
                    }
                    self.cache_insert(target, output)?;
                }
            }
        }
        self.cache.get(target).cloned().ok_or_else(|| {
            ReductionError::ReducerInvariant {
                detail: "candidate target is absent after work stack completed",
            }
            .into()
        })
    }

    pub fn clear_cache(&mut self) -> Result<(), CandidateReductionError> {
        self.cache_budget.replace(
            self.cache.len(),
            self.cache_weight,
            0,
            CacheWeight::default(),
            self.limits,
        )?;
        self.cache.clear();
        self.cache_weight = CacheWeight::default();
        Ok(())
    }

    /// Check an explicit finite entry set and its reachable successor DAG.
    ///
    /// The batch starts with a fresh cache, and every nonterminal child
    /// reached by its reductions is recursively visited by the same
    /// concrete candidate engine. Consequently an uncovered point, vanished
    /// source condition, undefined denominator or non-descending edge fails
    /// closed. Candidate formulas have not replayed their original-source
    /// provenance, so the report is not a proof for even these finite entries.
    /// It also carries no claim about entries outside `targets` or about the
    /// infinite positive-power complement.
    pub fn check_targets(
        &mut self,
        targets: impl IntoIterator<Item = IntegralKey>,
    ) -> Result<CandidateReachabilityReport, CandidateReductionError> {
        self.clear_cache()?;
        let mut requested = BTreeSet::new();
        for target in targets {
            requested.insert(target);
        }
        for target in &requested {
            self.reduce_unit_mass(target)?;
        }
        let mut max_negative = 0_u128;
        let mut max_positive = 0_u128;
        for key in self.cache.keys() {
            let (negative, positive) = key
                .powers()
                .iter()
                .try_fold::<_, _, Result<_, ()>>(
                    (0_u128, 0_u128),
                    |(negative, positive), &power| {
                        if power < 0 {
                            Ok((
                                negative
                                    .checked_add(u128::from(power.unsigned_abs()))
                                    .ok_or(())?,
                                positive,
                            ))
                        } else {
                            Ok((
                                negative,
                                positive.checked_add(u128::from(power as u64)).ok_or(())?,
                            ))
                        }
                    },
                )
                .map_err(|_| {
                    CandidateReductionError::InvalidInput(
                        "candidate reachability power census overflowed".to_owned(),
                    )
                })?;
            max_negative = max_negative.max(negative);
            max_positive = max_positive.max(positive);
        }
        let reachable_terminals = self
            .cache
            .values()
            .flat_map(|decomposition| decomposition.terms().keys())
            .filter(|key| self.terminals.contains(*key))
            .collect::<BTreeSet<_>>()
            .len();
        Ok(CandidateReachabilityReport {
            requested_targets: requested.len(),
            reachable_integrals: self.cache.len(),
            reachable_terminals,
            max_negative_index_degree: max_negative,
            max_positive_power_sum: max_positive,
        })
    }

    fn push_frame(
        &self,
        stack: &mut Vec<Frame>,
        frame: Frame,
        request: &mut ReductionRequest,
    ) -> Result<(), CandidateReductionError> {
        request.retain_pending_frame(self.limits.max_pending_frames)?;
        stack
            .try_reserve(1)
            .map_err(|_| ReductionError::AllocationFailure {
                resource: "candidate application stack",
                requested: stack.len().saturating_add(1),
            })?;
        stack.push(frame);
        Ok(())
    }

    fn cache_insert(
        &mut self,
        target: IntegralKey,
        terms: BTreeMap<IntegralKey, Coefficient>,
    ) -> Result<(), CandidateReductionError> {
        let previous = self.cache.get(&target);
        let previous_count = usize::from(previous.is_some());
        let previous_weight = previous
            .map(|p| coefficient_cache_weight(p.terms.values()))
            .transpose()?
            .unwrap_or_default();
        let weight = coefficient_cache_weight(terms.values())?;
        let prospective_weight = self
            .cache_weight
            .checked_sub(previous_weight)?
            .checked_add(weight)?;
        self.cache_budget
            .replace(previous_count, previous_weight, 1, weight, self.limits)?;
        self.cache.insert(
            target.clone(),
            CandidateDecomposition {
                family_fingerprint: self.family_fingerprint.clone(),
                target,
                terms,
            },
        );
        self.cache_weight = prospective_weight;
        Ok(())
    }
}

enum Frame {
    Expand(IntegralKey),
    Combine {
        target: IntegralKey,
        terms: BTreeMap<IntegralKey, Coefficient>,
    },
}
