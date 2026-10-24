use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

use crate::algebra::{Coefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::family::IntegralKey;
use crate::persistence::application_cache::{ApplicationCacheKey, ApplicationCacheRecord};
use crate::reduction::terminal_normalization::{TerminalAliasPlan, TerminalNormalizationPlan};
use crate::reduction::{
    CacheWeight, ReductionError, ReductionLimits, ReductionRequest, ReductionStatistics,
    SharedCacheBudget,
};
use crate::sector::{OrderingPolicy, zero};

use super::cache::{self, CacheEntry, CachedTerms};
use super::capture::CaptureObserver;
use super::model::{
    CandidateCacheRepresentation, CandidateDecomposition, CandidateReachabilityReport,
    CandidateReductionError, CandidateStatistics, PreparedRule, UnresolvedPolicy,
};

/// A request-scoped lookup of complete, already validated guarded records.
/// No provider is retained in the reducer or used by its ordinary APIs.
pub(super) type CandidateGuardedLookup<'a, E> =
    dyn FnMut(&IntegralKey) -> Result<Option<Arc<ApplicationCacheRecord>>, E> + 'a;

/// Immutable mathematical inputs shared by independent application sessions.
/// Terminal conventions are shared separately so changing one session's
/// convention cannot copy the rule program or affect another session.
#[derive(Debug)]
pub(super) struct CandidateProgram<const N: usize> {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) context: IndexedCoefficientContext,
    pub(super) root_sector: [bool; N],
    pub(super) max_numerator_rank: Option<u32>,
    pub(super) ordering: OrderingPolicy,
    pub(super) rules: BTreeMap<[bool; N], Vec<PreparedRule<N>>>,
    pub(super) whole_piece_alternatives: BTreeMap<[bool; N], Vec<usize>>,
    pub(super) terminals: BTreeSet<IntegralKey>,
    pub(super) zero_sectors: BTreeSet<[bool; N]>,
    pub(super) _zero_certificates: Vec<zero::Certificate>,
    pub(super) source_conditions: Vec<IndexedPolynomial>,
}

/// Deterministic, iterative application of explicitly experimental sector
/// formulas. This type cannot be converted to a closed artifact. Explicit
/// partial application preserves gaps as unresolved integrals, not terminals.
#[derive(Debug)]
pub struct CandidateReducer<const N: usize> {
    pub(super) program: Arc<CandidateProgram<N>>,
    pub(super) terminal_aliases: Option<Arc<TerminalAliasPlan>>,
    pub(super) terminal_normalization: Option<Arc<TerminalNormalizationPlan>>,
    pub(super) limits: ReductionLimits,
    pub(super) cache: BTreeMap<IntegralKey, CacheEntry>,
    pub(super) cache_representation: CandidateCacheRepresentation,
    pub(super) unresolved_policy: UnresolvedPolicy,
    pub(super) cache_weight: CacheWeight,
    pub(super) cache_budget: Arc<SharedCacheBudget>,
    pub(super) statistics: ReductionStatistics,
}

impl<const N: usize> Drop for CandidateReducer<N> {
    fn drop(&mut self) {
        if self.cache.is_empty() {
            debug_assert_eq!(self.cache_weight, CacheWeight::default());
            return;
        }
        // The outer completed-root cache owns separate retained payload. A
        // transient native session releases exactly its own share, including
        // after a typed error or unwind, without invoking pressure callbacks.
        let released = self.cache_budget.replace_without_pressure(
            self.cache.len(),
            self.cache_weight,
            0,
            CacheWeight::default(),
            self.limits,
        );
        debug_assert!(released.is_ok(), "candidate cache census did not release");
    }
}

impl<const N: usize> CandidateReducer<N> {
    /// Start an isolated application session over this exact immutable rule
    /// program and terminal convention. This neither reparses/replays rules
    /// nor clones their coefficients. The new cache, budget and work counters
    /// are empty; clearing either session cannot disturb the other.
    pub fn fork_empty(&self) -> Self {
        self.fork_empty_with_limits(self.limits)
    }

    /// As `fork_empty`, with explicitly selected operational allowances for
    /// the new session. Mathematical admission and terminal declarations are
    /// unchanged. No allowance or state is modified in the source session.
    pub fn fork_empty_with_limits(&self, limits: ReductionLimits) -> Self {
        self.fork_empty_with_shared_budget(limits, Arc::new(SharedCacheBudget::default()))
    }

    pub(super) fn fork_empty_with_shared_budget(
        &self,
        limits: ReductionLimits,
        cache_budget: Arc<SharedCacheBudget>,
    ) -> Self {
        Self {
            program: Arc::clone(&self.program),
            terminal_aliases: self.terminal_aliases.clone(),
            terminal_normalization: self.terminal_normalization.clone(),
            limits,
            cache: BTreeMap::new(),
            cache_representation: self.cache_representation,
            unresolved_policy: self.unresolved_policy,
            cache_weight: CacheWeight::default(),
            cache_budget,
            statistics: ReductionStatistics::default(),
        }
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.program.family_fingerprint
    }
    pub fn index_count(&self) -> usize {
        N
    }
    /// Physical key arity, which can be smaller than storage capacity `N`.
    pub fn physical_arity(&self) -> usize {
        self.program.context.index_count()
    }
    /// Original declared terminals, retained even when output aliases are installed.
    /// Offline catalogs and provenance checks should continue to use this set.
    pub fn terminals(&self) -> &BTreeSet<IntegralKey> {
        &self.program.terminals
    }
    pub fn ordering(&self) -> OrderingPolicy {
        self.program.ordering.clone()
    }
    /// Optional entry-domain bound on `sum(max(-n_i, 0))`. This is not a
    /// successor bound: valid reductions may visit integrals above this rank.
    pub fn max_numerator_rank(&self) -> Option<u32> {
        self.program.max_numerator_rank
    }
    pub fn limits(&self) -> ReductionLimits {
        self.limits
    }
    pub fn cache_representation(&self) -> CandidateCacheRepresentation {
        self.cache_representation
    }

    /// Select private cache storage before application, or after `clear_cache`.
    /// This changes neither candidate authority nor public output coefficients.
    /// Factorized storage charges a conservative expanded-term envelope and
    /// materializes every requested output transiently; no second cache exists.
    pub fn set_cache_representation(
        &mut self,
        representation: CandidateCacheRepresentation,
    ) -> Result<(), CandidateReductionError> {
        if !self.cache.is_empty() {
            return Err(CandidateReductionError::InvalidInput(
                "coefficient representation requires an empty reduction cache".into(),
            ));
        }
        self.cache_representation = representation;
        Ok(())
    }
    pub fn unresolved_policy(&self) -> UnresolvedPolicy {
        self.unresolved_policy
    }

    /// Policy changes require an empty cache so partial identities cannot
    /// satisfy a later strict request. Formula/source checks remain mandatory.
    pub fn set_unresolved_policy(
        &mut self,
        policy: UnresolvedPolicy,
    ) -> Result<(), CandidateReductionError> {
        if !self.cache.is_empty() {
            return Err(CandidateReductionError::InvalidInput(
                "unresolved policy requires an empty reduction cache".into(),
            ));
        }
        self.unresolved_policy = policy;
        Ok(())
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

    /// Apply checked formulas at one concrete integer point. Strict mode returns
    /// declared finite terminals; explicit partial mode also reports unresolved
    /// keys. Parameters remain exact and generic; specialization is caller-owned.
    pub fn reduce_unit_mass(
        &mut self,
        target: &IntegralKey,
    ) -> Result<CandidateDecomposition, CandidateReductionError> {
        self.reduce_unit_mass_impl(target, false)
    }

    /// Opt-in exact subtree guards and per-call work for typed factor providers.
    /// Remaining allowances are intersected with this owner's policy before
    /// execution. An unguarded cached node is recomputed once when reached;
    /// guarded calls reuse coefficients and their original domain together.
    pub fn reduce_unit_mass_guarded(
        &mut self,
        target: &IntegralKey,
        remaining: ReductionLimits,
    ) -> Result<
        (
            CandidateDecomposition,
            Vec<Coefficient>,
            ReductionStatistics,
        ),
        CandidateReductionError,
    > {
        self.reduce_guarded_impl(target, remaining, false, None, None)
    }

    /// A single fresh root may retain its complete guard union once instead
    /// of copying subtree unions into every inner memo entry. This private
    /// mode must never consume a previously populated coefficient cache.
    pub(super) fn reduce_fresh_unit_mass_guarded(
        &mut self,
        target: &IntegralKey,
        remaining: ReductionLimits,
    ) -> Result<
        (
            CandidateDecomposition,
            Vec<Coefficient>,
            ReductionStatistics,
        ),
        CandidateReductionError,
    > {
        self.reduce_guarded_impl(target, remaining, true, None, None)
    }

    pub(super) fn reduce_fresh_unit_mass_guarded_with_lookup(
        &mut self,
        target: &IntegralKey,
        remaining: ReductionLimits,
        lookup: &mut CandidateGuardedLookup<'_, super::CandidateGuardedCacheError>,
    ) -> Result<
        (
            CandidateDecomposition,
            Vec<Coefficient>,
            ReductionStatistics,
        ),
        super::CandidateGuardedCacheError,
    > {
        self.reduce_guarded_impl(target, remaining, true, Some(lookup), None)
    }

    pub(super) fn reduce_fresh_unit_mass_guarded_with_capture(
        &mut self,
        target: &IntegralKey,
        remaining: ReductionLimits,
        lookup: &mut CandidateGuardedLookup<'_, super::CandidateGuardedCacheError>,
        capture: &mut dyn CaptureObserver<N, super::CandidateGuardedCacheError>,
    ) -> Result<
        (
            CandidateDecomposition,
            Vec<Coefficient>,
            ReductionStatistics,
        ),
        super::CandidateGuardedCacheError,
    > {
        self.reduce_guarded_impl(target, remaining, true, Some(lookup), Some(capture))
    }

    fn reduce_guarded_impl<E: From<CandidateReductionError>>(
        &mut self,
        target: &IntegralKey,
        remaining: ReductionLimits,
        fresh_root: bool,
        lookup: Option<&mut CandidateGuardedLookup<'_, E>>,
        capture: Option<&mut dyn CaptureObserver<N, E>>,
    ) -> Result<
        (
            CandidateDecomposition,
            Vec<Coefficient>,
            ReductionStatistics,
        ),
        E,
    > {
        if fresh_root && !self.cache.is_empty() {
            return Err(
                CandidateReductionError::from(ReductionError::ReducerInvariant {
                    detail: "fresh guarded candidate root requires an empty inner cache",
                })
                .into(),
            );
        }
        let original_limits = self.limits;
        let previous_work = self.statistics;
        self.limits = original_limits.bounded_by(remaining);
        self.statistics = ReductionStatistics::default();
        let result: Result<_, E> = (|| {
            self.validate_entry_target(target)?;
            self.cache_budget
                .replace(
                    0,
                    CacheWeight::default(),
                    0,
                    CacheWeight::default(),
                    self.limits,
                )
                .map_err(CandidateReductionError::from)?;
            let (result, conditions, _request_guard_lease) = if let Some(capture) = capture {
                let result = self.reduce_unit_mass_impl_with_root_guards(
                    target,
                    true,
                    None,
                    lookup,
                    Some(capture),
                )?;
                (result, capture.root_conditions(), None)
            } else if fresh_root {
                let (result, conditions, lease) =
                    self.reduce_with_root_guard_union(target, lookup)?;
                (result, conditions, Some(lease))
            } else {
                let result = self.reduce_unit_mass_impl(target, true)?;
                let conditions = self
                    .cache
                    .get(target)
                    .and_then(|entry| entry.conditions.as_ref())
                    .ok_or(CandidateReductionError::from(
                        ReductionError::ReducerInvariant {
                            detail: "guarded candidate root lacks its condition cache",
                        },
                    ))?
                    .clone();
                (result, conditions, None)
            };
            for coefficient in result.terms().values().chain(&conditions) {
                self.program
                    .context
                    .base()
                    .validate_with_limits(coefficient, self.limits.exact_algebra)
                    .map_err(CandidateReductionError::from)?;
            }
            Ok((result, conditions, self.statistics))
        })();
        let work = self.statistics;
        self.statistics = previous_work;
        self.statistics.merge_work(work);
        self.limits = original_limits;
        result
    }

    fn reduce_unit_mass_impl(
        &mut self,
        target: &IntegralKey,
        guarded: bool,
    ) -> Result<CandidateDecomposition, CandidateReductionError> {
        self.reduce_unit_mass_impl_with_root_guards(target, guarded, None, None, None)
    }

    pub(super) fn reduce_unit_mass_impl_with_root_guards<E: From<CandidateReductionError>>(
        &mut self,
        target: &IntegralKey,
        guarded: bool,
        mut root_guards: Option<
            &mut dyn FnMut(&[Coefficient]) -> Result<(), CandidateReductionError>,
        >,
        mut lookup: Option<&mut CandidateGuardedLookup<'_, E>>,
        mut capture: Option<&mut dyn CaptureObserver<N, E>>,
    ) -> Result<CandidateDecomposition, E> {
        symbolica::utils::resources::checkpoint()
            .map_err(ReductionError::from)
            .map_err(CandidateReductionError::from)?;
        if self.unresolved_policy == UnresolvedPolicy::Preserve
            && (lookup.is_some() || capture.is_some())
        {
            return Err(CandidateReductionError::InvalidInput(
                "partial reduction cannot import or capture complete guarded records".into(),
            )
            .into());
        }
        if (root_guards.is_some() && (!guarded || !self.cache.is_empty()))
            || (lookup.is_some() && root_guards.is_none() && capture.is_none())
            || (capture.is_some() && (!guarded || !self.cache.is_empty() || root_guards.is_some()))
        {
            return Err(
                CandidateReductionError::from(ReductionError::ReducerInvariant {
                    detail: "root guard collection requires one fresh guarded candidate request",
                })
                .into(),
            );
        }
        let subtree_guards = guarded && root_guards.is_none() && capture.is_none();
        self.validate_entry_target(target)?;
        if self.cached_for(target, subtree_guards) {
            self.statistics.record_cache_hit();
            return self.materialize_target(target).map_err(E::from);
        }
        let program = Arc::clone(&self.program);
        let mut request = ReductionRequest::default();
        let mut stack = Vec::new();
        let mut active = BTreeSet::new();
        self.push_frame(&mut stack, Frame::Expand(target.clone()), &mut request)?;
        while let Some(frame) = stack.pop() {
            request
                .release_pending_frame()
                .map_err(CandidateReductionError::from)?;
            match frame {
                Frame::Expand(key) => {
                    if self.cached_for(&key, subtree_guards) {
                        self.statistics.record_cache_hit();
                        continue;
                    }
                    if lookup.is_some() && active.contains(&key) {
                        return Err(
                            CandidateReductionError::from(ReductionError::CycleDetected {
                                target: key,
                            })
                            .into(),
                        );
                    }
                    let conditions = if guarded {
                        Some(self.target_conditions(&key)?)
                    } else {
                        self.validate_target(&key)?;
                        None
                    };
                    if self.is_zero(&key) {
                        let terms = CachedTerms::from_sparse(
                            BTreeMap::new(),
                            self.cache_representation,
                            self.program.context.base(),
                            self.limits,
                        )?;
                        let conditions = retain_request_guards(
                            &key,
                            &mut root_guards,
                            &mut capture,
                            conditions,
                        )?;
                        self.cache_insert(key.clone(), terms, conditions)?;
                        if let Some(capture) = capture.as_mut() {
                            capture.completed(&key, self, false)?;
                        }
                        continue;
                    }
                    if self.program.terminals.contains(&key) {
                        // Exactly one sealed convention is installed. All
                        // outputs are declared fixed points; no replay or
                        // recursive terminal expansion enters the hot path.
                        let terms = self.terminal_terms(&key)?;
                        let terms = CachedTerms::from_sparse(
                            terms,
                            self.cache_representation,
                            self.program.context.base(),
                            self.limits,
                        )?;
                        let mut conditions = conditions;
                        if let Some(conditions) = conditions.as_mut() {
                            // The finite normalization factory uses rational
                            // constant geometry. Retain native output poles as
                            // well, before any downstream master cancellation.
                            let sparse =
                                terms.materialize(self.program.context.base(), self.limits)?;
                            for coefficient in sparse.values() {
                                retain_polynomial(conditions, &coefficient.denominator);
                            }
                        }
                        let conditions = retain_request_guards(
                            &key,
                            &mut root_guards,
                            &mut capture,
                            conditions,
                        )?;
                        self.cache_insert(key.clone(), terms, conditions)?;
                        if let Some(capture) = capture.as_mut() {
                            capture.completed(&key, self, false)?;
                        }
                        continue;
                    }
                    // Establish native coverage before an external result can
                    // replace a subtree. Reuse this selection on a miss, and
                    // never repeat the already attempted starting-root lookup.
                    // Native zero and terminal rows above require no IO.
                    let selected = if (lookup.is_some() && key != *target)
                        || self.unresolved_policy == UnresolvedPolicy::Preserve
                    {
                        match program
                            .evaluator(&key, self.limits)
                            .and_then(|evaluator| evaluator.selected_rule(&key))
                        {
                            Ok(selected) => Some(selected),
                            Err(CandidateReductionError::Uncovered { .. })
                                if self.unresolved_policy == UnresolvedPolicy::Preserve =>
                            {
                                // An identity leaf records ignorance, not a new terminal.
                                // Source guards were checked above and remain attached.
                                let terms = CachedTerms::from_sparse(
                                    BTreeMap::from([(key.clone(), program.context.base().one())]),
                                    self.cache_representation,
                                    program.context.base(),
                                    self.limits,
                                )?;
                                let conditions = retain_request_guards(
                                    &key,
                                    &mut root_guards,
                                    &mut capture,
                                    conditions,
                                )?;
                                self.cache_insert(key, terms, conditions)?;
                                continue;
                            }
                            Err(error) => return Err(error.into()),
                        }
                    } else {
                        None
                    };
                    if key != *target {
                        if let Some(provider) = lookup.as_mut() {
                            if let Some(record) = provider(&key)? {
                                if let Some(capture) = capture.as_mut() {
                                    capture.local(&key, &record.nonzero_conditions)?;
                                    let terms = CachedTerms::from_sparse(
                                        record.terms.clone(),
                                        self.cache_representation,
                                        self.program.context.base(),
                                        self.limits,
                                    )?;
                                    self.cache_insert(key.clone(), terms, None)?;
                                    self.statistics.record_cache_hit();
                                    capture.completed(&key, self, true)?;
                                } else {
                                    self.import_guarded_record(&key, &record, &mut root_guards)?;
                                }
                                continue;
                            }
                        }
                    }
                    if !active.insert(key.clone()) {
                        return Err(
                            CandidateReductionError::from(ReductionError::CycleDetected {
                                target: key,
                            })
                            .into(),
                        );
                    }
                    request
                        .record_rule_application(
                            self.limits
                                .diagnostic_limit(self.limits.max_rule_applications),
                        )
                        .map_err(CandidateReductionError::from)?;
                    let (terms, conditions) = if guarded {
                        let (terms, conditions) = if let Some((baseline, rule)) = selected {
                            program
                                .evaluator(&key, self.limits)?
                                .apply_selected_with_conditions(
                                    &key,
                                    baseline,
                                    rule,
                                    &mut request,
                                    &mut self.statistics,
                                )?
                        } else {
                            self.apply_guarded_candidate(&key, &mut request)?
                        };
                        (terms, Some(conditions))
                    } else {
                        (self.apply_candidate(&key, &mut request)?, None)
                    };
                    self.statistics.record_rule_application();
                    let conditions =
                        retain_request_guards(&key, &mut root_guards, &mut capture, conditions)?;
                    if let Some(capture) = capture.as_mut() {
                        for child in terms.keys() {
                            capture.child(&key, child)?;
                        }
                    }
                    let mut children = Vec::new();
                    children.try_reserve_exact(terms.len()).map_err(|_| {
                        CandidateReductionError::from(ReductionError::AllocationFailure {
                            resource: "candidate child schedule",
                            requested: terms.len(),
                        })
                    })?;
                    children.extend(terms.keys().cloned());
                    self.push_frame(
                        &mut stack,
                        Frame::Combine {
                            target: key,
                            terms,
                            conditions,
                        },
                        &mut request,
                    )?;
                    for child in children.into_iter().rev() {
                        if self.cached_for(&child, subtree_guards) {
                            // A ready subtree is consumed without scheduling
                            // Expand, so its hit must be recorded here rather
                            // than relying on the stack's cached-node branch.
                            self.statistics.record_cache_hit();
                        } else {
                            self.push_frame(&mut stack, Frame::Expand(child), &mut request)?;
                        }
                    }
                }
                Frame::Combine {
                    target,
                    terms,
                    conditions,
                } => {
                    if conditions.is_some()
                        && terms.keys().any(|key| {
                            self.cache
                                .get(key)
                                .and_then(|entry| entry.conditions.as_ref())
                                .is_none()
                        })
                    {
                        return Err(CandidateReductionError::from(
                            ReductionError::ReducerInvariant {
                                detail: "guarded candidate child lacks its condition cache",
                            },
                        )
                        .into());
                    }
                    let conditions = conditions.map(|local| {
                        let mut seen = HashSet::new();
                        let mut output = Vec::new();
                        for condition in local.iter().chain(terms.keys().flat_map(|key| {
                            self.cache
                                .get(key)
                                .and_then(|entry| entry.conditions.as_deref())
                                .unwrap_or(&[])
                        })) {
                            if seen.insert(condition) {
                                output.push(condition.clone());
                            }
                        }
                        output
                    });
                    let output = cache::combine(
                        self.program.context.base(),
                        &self.cache,
                        self.cache_representation,
                        terms,
                        self.limits,
                        &mut request,
                        &mut self.statistics,
                    )?;
                    if !active.remove(&target) {
                        return Err(CandidateReductionError::from(
                            ReductionError::ReducerInvariant {
                                detail: "candidate combine target is not active",
                            },
                        )
                        .into());
                    }
                    self.cache_insert(target.clone(), output, conditions)?;
                    if let Some(capture) = capture.as_mut() {
                        capture.completed(&target, self, false)?;
                    }
                }
            }
        }
        self.materialize_target(target).map_err(E::from)
    }

    fn import_guarded_record(
        &mut self,
        target: &IntegralKey,
        record: &ApplicationCacheRecord,
        root_guards: &mut Option<
            &mut dyn FnMut(&[Coefficient]) -> Result<(), CandidateReductionError>,
        >,
    ) -> Result<(), CandidateReductionError> {
        if record.key != ApplicationCacheKey::guarded_root(target) {
            return Err(ReductionError::ReducerInvariant {
                detail: "candidate guarded provider returned a different exact key",
            }
            .into());
        }
        let retain = root_guards
            .as_mut()
            .ok_or(ReductionError::ReducerInvariant {
                detail: "candidate guarded provider requires a fresh root guard union",
            })?;
        // The provider validates context, terminal convention and payload at
        // admission. Preserve every supplied obligation, including those of
        // zero or cancelled subtrees, under the request's shared guard lease.
        retain(&record.nonzero_conditions)?;
        let terms = CachedTerms::from_sparse(
            record.terms.clone(),
            self.cache_representation,
            self.program.context.base(),
            self.limits,
        )?;
        self.cache_insert(target.clone(), terms, None)?;
        self.statistics.record_cache_hit();
        Ok(())
    }

    pub fn clear_cache(&mut self) -> Result<(), CandidateReductionError> {
        self.cache_budget.replace_without_pressure(
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

    fn cached_for(&self, key: &IntegralKey, guarded: bool) -> bool {
        self.cache
            .get(key)
            .is_some_and(|entry| !guarded || entry.conditions.is_some())
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
        if self.unresolved_policy != UnresolvedPolicy::Reject {
            return Err(CandidateReductionError::InvalidInput(
                "reachability checking requires strict unresolved policy".into(),
            ));
        }
        let mut requested = BTreeSet::new();
        for target in targets {
            self.validate_entry_target(&target)?;
            requested.insert(target);
        }
        self.clear_cache()?;
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
            .flat_map(|entry| entry.terms.terminal_keys())
            .filter(|key| self.program.terminals.contains(*key))
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
        request
            .retain_pending_frame(self.limits.diagnostic_limit(self.limits.max_pending_frames))?;
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
        terms: CachedTerms,
        conditions: Option<Vec<Coefficient>>,
    ) -> Result<(), CandidateReductionError> {
        let previous = self.cache.get(&target);
        let previous_count = usize::from(previous.is_some());
        let previous_weight = previous.map(|entry| entry.weight).unwrap_or_default();
        let mut weight = terms.weight(self.limits.diagnostics_enabled)?;
        if let Some(conditions) = &conditions {
            weight = weight.checked_add(crate::reduction::coefficient_cache_weight(
                conditions.iter(),
            )?)?;
        }
        let prospective_weight = self
            .cache_weight
            .checked_sub(previous_weight)?
            .checked_add(weight)?;
        self.cache_budget
            .replace(previous_count, previous_weight, 1, weight, self.limits)?;
        self.cache.insert(
            target,
            CacheEntry {
                terms,
                weight,
                conditions,
            },
        );
        self.cache_weight = prospective_weight;
        Ok(())
    }

    fn materialize_target(
        &self,
        target: &IntegralKey,
    ) -> Result<CandidateDecomposition, CandidateReductionError> {
        symbolica::utils::resources::checkpoint().map_err(ReductionError::from)?;
        let entry = self
            .cache
            .get(target)
            .ok_or(ReductionError::ReducerInvariant {
                detail: "candidate target is absent after work stack completed",
            })?;
        let terms = entry
            .terms
            .materialize(self.program.context.base(), self.limits)?;
        let unresolved = terms
            .keys()
            .filter(|key| !self.canonical_terminals().contains(*key))
            .cloned()
            .collect();
        symbolica::utils::resources::checkpoint().map_err(ReductionError::from)?;
        Ok(CandidateDecomposition {
            family_fingerprint: self.program.family_fingerprint.clone(),
            target: target.clone(),
            terms,
            unresolved,
        })
    }

    fn target_conditions(
        &self,
        target: &IntegralKey,
    ) -> Result<Vec<Coefficient>, CandidateReductionError> {
        self.target_conditions_with_limits(target, self.limits)
    }

    pub(super) fn target_conditions_with_limits(
        &self,
        target: &IntegralKey,
        limits: ReductionLimits,
    ) -> Result<Vec<Coefficient>, CandidateReductionError> {
        let mut conditions = Vec::new();
        super::evaluator::CandidateEvaluator {
            context: &self.program.context,
            root_sector: self.program.root_sector,
            ordering: &self.program.ordering,
            rules: &[],
            whole_piece_alternatives: &[],
            source_conditions: &self.program.source_conditions,
            zero_sectors: &self.program.zero_sectors,
            limits,
        }
        .validate_target_with_conditions(target, &mut |condition| {
            retain_polynomial(&mut conditions, condition)
        })?;
        Ok(conditions)
    }

    /// Exact leaf row under the installed native terminal convention. Callers
    /// must establish that `target` is a declared raw terminal first.
    pub(super) fn terminal_terms(
        &self,
        target: &IntegralKey,
    ) -> Result<BTreeMap<IntegralKey, Coefficient>, CandidateReductionError> {
        if let Some(plan) = &self.terminal_normalization {
            return plan.terms().get(target).cloned().ok_or_else(|| {
                ReductionError::ReducerInvariant {
                    detail: "sealed normalization omitted a raw terminal",
                }
                .into()
            });
        }
        let terminal = self
            .terminal_aliases
            .as_ref()
            .and_then(|plan| plan.aliases().get(target))
            .map_or(target, |alias| alias.representative());
        Ok(BTreeMap::from([(
            terminal.clone(),
            self.program.context.base().one(),
        )]))
    }

    fn apply_guarded_candidate(
        &mut self,
        target: &IntegralKey,
        request: &mut ReductionRequest,
    ) -> Result<(BTreeMap<IntegralKey, Coefficient>, Vec<Coefficient>), CandidateReductionError>
    {
        self.program
            .evaluator(target, self.limits)?
            .apply_with_conditions(target, request, &mut self.statistics)
    }
}

fn retain_request_guards<const N: usize, E: From<CandidateReductionError>>(
    key: &IntegralKey,
    root: &mut Option<&mut dyn FnMut(&[Coefficient]) -> Result<(), CandidateReductionError>>,
    capture: &mut Option<&mut dyn CaptureObserver<N, E>>,
    conditions: Option<Vec<Coefficient>>,
) -> Result<Option<Vec<Coefficient>>, E> {
    if let Some(capture) = capture.as_mut() {
        capture.local(key, conditions.as_deref().unwrap_or(&[]))?;
        Ok(None)
    } else {
        retain_root_guards(root, conditions).map_err(Into::into)
    }
}

fn retain_root_guards(
    collector: &mut Option<&mut dyn FnMut(&[Coefficient]) -> Result<(), CandidateReductionError>>,
    conditions: Option<Vec<Coefficient>>,
) -> Result<Option<Vec<Coefficient>>, CandidateReductionError> {
    if let Some(collector) = collector.as_mut() {
        collector(
            conditions
                .as_deref()
                .ok_or(ReductionError::ReducerInvariant {
                    detail: "fresh guarded candidate visit has no local conditions",
                })?,
        )?;
        Ok(None)
    } else {
        Ok(conditions)
    }
}

fn retain_polynomial(
    conditions: &mut Vec<Coefficient>,
    polynomial: &crate::algebra::CoefficientPolynomial,
) {
    if !polynomial.is_constant() {
        let coefficient = polynomial.clone().into();
        if !conditions.contains(&coefficient) {
            conditions.push(coefficient);
        }
    }
}

enum Frame {
    Expand(IntegralKey),
    Combine {
        target: IntegralKey,
        terms: BTreeMap<IntegralKey, Coefficient>,
        conditions: Option<Vec<Coefficient>>,
    },
}

#[cfg(test)]
mod guarded_tests {
    use super::*;
    use crate::solver::candidate_reduction::owner_test_support::{input, key, rule};

    #[test]
    fn cached_plain_candidate_still_obeys_the_current_elapsed_budget() {
        use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};
        let family = crate::solver::tests::tadpole();
        let record = input([true], None, vec![], &[[1]]);
        let mut reducer = CandidateReducer::try_new(
            &family,
            [true],
            record.ordering,
            [([true], record.solution)],
            vec![],
            Default::default(),
        )
        .unwrap();
        let target = key([1]);
        let expected = reducer.reduce_unit_mass(&target).unwrap();
        let budget = ResourceBudget::new(ResourceLimits {
            max_elapsed: Some(std::time::Duration::ZERO),
            ..Default::default()
        });
        assert!(matches!(
            budget.run(|| reducer.reduce_unit_mass(&target)),
            Err(CandidateReductionError::Application(
                ReductionError::Resources(ResourceLimitError::Elapsed { .. })
            ))
        ));
        assert_eq!(reducer.reduce_unit_mass(&target).unwrap(), expected);
    }

    #[test]
    fn guarded_candidate_retains_cancelled_original_poles_and_cached_subtree_domain() {
        let family = crate::solver::tests::tadpole();
        let record = input(
            [true],
            None,
            vec![
                rule(&family, [3], &[([2], 1)]),
                rule(&family, [2], &[([1], 1), ([1], -1)]),
            ],
            &[[1]],
        );
        let mut reducer = CandidateReducer::try_new(
            &family,
            [true],
            record.ordering,
            [([true], record.solution)],
            vec![],
            Default::default(),
        )
        .unwrap();
        let c = family.coefficient_context();
        let indexed = reducer.program.context.clone();
        let polynomial = |text| {
            let value = indexed.lift(&c.coefficient_fixture(text)).unwrap();
            indexed
                .admit_native_polynomial_result_with_limits(
                    value.raw().numerator.clone(),
                    Default::default(),
                )
                .unwrap()
        };
        std::sync::Arc::get_mut(&mut reducer.program)
            .unwrap()
            .source_conditions
            .push(polynomial("d-7"));
        let rules = std::sync::Arc::get_mut(&mut reducer.program)
            .unwrap()
            .rules
            .get_mut(&[true])
            .unwrap();
        rules[0].rhs[0].denominator = polynomial("d-4");
        rules[1].rhs[0].denominator = polynomial("d-5");
        rules[1].exceptions = vec![vec![polynomial("d-6"), polynomial("d-8")]];
        assert!(reducer.reduce_unit_mass(&key([3])).unwrap().is_zero());
        let (first, conditions, work) = reducer
            .reduce_unit_mass_guarded(&key([3]), Default::default())
            .unwrap();
        assert!(first.is_zero());
        assert_eq!(work.rule_applications(), 2);
        for text in ["d-4", "d-5", "d-6", "d-7"] {
            assert!(conditions.contains(&c.coefficient_fixture(text)), "{text}");
        }
        assert!(!conditions.contains(&c.coefficient_fixture("d-8")));
        let (warm, warm_conditions, warm_work) = reducer
            .reduce_unit_mass_guarded(
                &key([3]),
                ReductionLimits {
                    max_rule_applications: 0,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(warm, first);
        assert_eq!(warm_conditions, conditions);
        assert_eq!(warm_work.rule_applications(), 0);
        assert_eq!(warm_work.cache_hits(), 1);
        assert!(reducer.statistics().cached_coefficient_terms() > 0);
        assert_eq!(reducer.limits(), ReductionLimits::default());
        reducer.clear_cache().unwrap();
        assert!(matches!(
            reducer.reduce_unit_mass_guarded(
                &key([3]),
                ReductionLimits {
                    max_rule_applications: 1,
                    ..Default::default()
                }
            ),
            Err(CandidateReductionError::Application(
                ReductionError::RuleApplicationLimit { .. }
            ))
        ));
        assert_eq!(reducer.limits(), ReductionLimits::default());
    }
}
