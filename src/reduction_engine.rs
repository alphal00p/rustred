//! Demand-driven application of verified concrete parametric reductions.
//!
//! This is the scalar rule-application kernel used after discovery.  It is
//! independent of loop count and topology: a caller supplies a rule provider
//! that returns a proof-bearing [`ConcreteReduction`] for a requested integral.
//! The engine substitutes only reachable rules, memoizes exact results,
//! retains every specialized nonzero condition, and rejects cycles,
//! family/context mismatches, non-descent, or resource exhaustion.  A missing
//! rule remains an explicitly uncovered terminal: it is never inferred to be
//! a master.
//!
//! Cache/statistics changes are transactional per top-level [`reduce`](
//! ParametricReductionEngine::reduce) call.  A failed call removes every cache
//! entry it staged and restores the engine statistics.  A provider is an
//! arbitrary caller-owned state machine, so mutations performed inside a
//! failed provider query cannot be rolled back by this module; providers that
//! need that property must implement their own transactional query semantics.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fmt::Write as _;
use std::sync::Arc;

use crate::certified_rewrite::{CertifiedRewriteDomainCondition, CertifiedZeroReduction};
use crate::{
    CertifiedConcreteRewrite, Coefficient, CoefficientContext, ConcreteIntegralKey,
    ConcreteReduction, ConditionalConcreteReduction, ExactAlgebraError, ExactAlgebraLimits,
    IntegralOrderingPolicy, ParametricCoefficientError, ParametricRelationError,
    SpecializedNonZeroCondition,
};

pub const PARAMETRIC_REDUCTION_ENGINE_V1_SCHEMA: &str = "rustred-parametric-reduction-engine-v1";

/// Why recursion stopped at one concrete integral.
///
/// `Uncovered` is deliberately separate from both kinds of master.  A
/// selected master is a caller policy choice, while a certified master binds
/// an external proof by its stable fingerprint.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConcreteTerminalStatus {
    Uncovered,
    SelectedMaster,
    CertifiedMaster { certificate_fingerprint: Arc<str> },
}

/// One exact provider decision at a concrete integral.
#[derive(Clone, Debug)]
pub enum ConcreteRuleDecision {
    Reduction(ConcreteReduction),
    /// A concrete specialization of a pivot valid only on its retained,
    /// certificate-bound conditional domain.  The authority may be a sparse
    /// coordinate-equality proof or a sealed generated-affine leaf; both are
    /// intentionally distinct from the global parametric-candidate path.
    ConditionalReduction(ConditionalConcreteReduction),
    CertifiedRewrite(CertifiedConcreteRewrite),
    ProvedZero(CertifiedZeroReduction),
    Terminal(ConcreteTerminalStatus),
}

/// A complete proof object for one applied nonterminal decision.
#[derive(Clone, Debug)]
pub enum ConcreteRuleApplicationTrace {
    Parametric(ConcreteReduction),
    ConditionalParametric(ConditionalConcreteReduction),
    CertifiedRewrite(CertifiedConcreteRewrite),
    ProvedZero(CertifiedZeroReduction),
}

pub trait ConcreteRuleProvider {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Fixed unshifted-index arity of every request and emitted rule target.
    fn index_arity(&self) -> usize;

    /// Return a verified rule or an explicit terminal classification.
    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReductionEngineLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_recursive_calls: usize,
    /// Conservative bound on simultaneously active native stack frames.
    /// This is distinct from the aggregate recursive-call budget.
    pub max_active_depth: usize,
    pub max_rule_applications: usize,
    pub max_sparse_updates: usize,
    pub max_cache_entries: usize,
    pub max_terms_per_result: usize,
    pub max_guard_polynomials: usize,
    pub max_guard_origins: usize,
    pub max_application_traces: usize,
    /// Aggregate trace entries retained across every cached prefix result.
    pub max_cached_application_trace_entries: usize,
    /// Aggregate bounded `Debug` bytes of cached proof-bearing traces.
    pub max_cached_proof_debug_bytes: usize,
}

impl Default for ReductionEngineLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_recursive_calls: 100_000_000,
            max_active_depth: 256,
            max_rule_applications: 100_000_000,
            max_sparse_updates: 1_000_000_000,
            max_cache_entries: 10_000_000,
            max_terms_per_result: 10_000_000,
            max_guard_polynomials: 10_000_000,
            max_guard_origins: 1_000_000,
            max_application_traces: 10_000_000,
            max_cached_application_trace_entries: 100_000_000,
            max_cached_proof_debug_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReductionEngineStats {
    recursive_calls: usize,
    rule_applications: usize,
    sparse_updates: usize,
    cache_hits: usize,
    cache_entries: usize,
    maximum_result_terms: usize,
    cached_application_trace_entries: usize,
    cached_proof_debug_bytes: usize,
}

impl ReductionEngineStats {
    pub const fn recursive_calls(self) -> usize {
        self.recursive_calls
    }
    pub const fn rule_applications(self) -> usize {
        self.rule_applications
    }
    pub const fn sparse_updates(self) -> usize {
        self.sparse_updates
    }
    pub const fn cache_hits(self) -> usize {
        self.cache_hits
    }
    pub const fn cache_entries(self) -> usize {
        self.cache_entries
    }
    pub const fn maximum_result_terms(self) -> usize {
        self.maximum_result_terms
    }
    pub const fn cached_application_trace_entries(self) -> usize {
        self.cached_application_trace_entries
    }
    pub const fn cached_proof_debug_bytes(self) -> usize {
        self.cached_proof_debug_bytes
    }
}

#[derive(Clone, Debug)]
pub struct ParametricReductionResult {
    family_fingerprint: Arc<str>,
    source: ConcreteIntegralKey,
    terms: BTreeMap<ConcreteIntegralKey, Coefficient>,
    required_nonzero: Vec<SpecializedNonZeroCondition>,
    certified_domain: Vec<CertifiedRewriteDomainCondition>,
    application_traces: Vec<ConcreteRuleApplicationTrace>,
    terminal_statuses: BTreeMap<ConcreteIntegralKey, ConcreteTerminalStatus>,
    uncovered_leaves: BTreeSet<ConcreteIntegralKey>,
    selected_masters: BTreeSet<ConcreteIntegralKey>,
    certified_masters: BTreeMap<ConcreteIntegralKey, Arc<str>>,
    stats: ReductionEngineStats,
    cached_application_trace_entries: usize,
    cached_proof_debug_bytes: usize,
}

impl ParametricReductionResult {
    pub const SCHEMA: &'static str = PARAMETRIC_REDUCTION_ENGINE_V1_SCHEMA;

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub const fn source(&self) -> &ConcreteIntegralKey {
        &self.source
    }

    pub const fn terms(&self) -> &BTreeMap<ConcreteIntegralKey, Coefficient> {
        &self.terms
    }

    pub fn required_nonzero(&self) -> &[SpecializedNonZeroCondition] {
        &self.required_nonzero
    }

    /// Generic-locus conditions introduced by certified zero/symmetry
    /// quotienting. Parametric guards remain available through
    /// [`Self::required_nonzero`].
    pub fn certified_domain(&self) -> &[CertifiedRewriteDomainCondition] {
        &self.certified_domain
    }

    /// Every applied proof survives provider drop and engine caching.
    pub fn application_traces(&self) -> &[ConcreteRuleApplicationTrace] {
        &self.application_traces
    }

    pub const fn terminal_statuses(
        &self,
    ) -> &BTreeMap<ConcreteIntegralKey, ConcreteTerminalStatus> {
        &self.terminal_statuses
    }

    pub const fn uncovered_leaves(&self) -> &BTreeSet<ConcreteIntegralKey> {
        &self.uncovered_leaves
    }

    pub const fn selected_masters(&self) -> &BTreeSet<ConcreteIntegralKey> {
        &self.selected_masters
    }

    pub const fn certified_masters(&self) -> &BTreeMap<ConcreteIntegralKey, Arc<str>> {
        &self.certified_masters
    }

    /// Require every surviving terminal to be an explicitly selected or
    /// certified master.  This never promotes an uncovered leaf.
    pub fn require_complete(&self) -> Result<&Self, IncompleteReductionError> {
        if self.uncovered_leaves.is_empty() {
            Ok(self)
        } else {
            Err(IncompleteReductionError {
                source: self.source.clone(),
                uncovered_leaves: self.uncovered_leaves.clone(),
            })
        }
    }

    pub const fn stats(&self) -> ReductionEngineStats {
        self.stats
    }

    /// Aggregate number of proof-bearing trace entries retained by all cache
    /// prefixes at the end of this reduction.
    pub const fn cached_application_trace_entries(&self) -> usize {
        self.cached_application_trace_entries
    }

    /// Aggregate bounded `Debug` size of proof-bearing traces retained by all
    /// cache prefixes at the end of this reduction.
    pub const fn cached_proof_debug_bytes(&self) -> usize {
        self.cached_proof_debug_bytes
    }
}

/// A reduction containing at least one nonzero uncovered terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncompleteReductionError {
    source: ConcreteIntegralKey,
    uncovered_leaves: BTreeSet<ConcreteIntegralKey>,
}

impl IncompleteReductionError {
    pub const fn source(&self) -> &ConcreteIntegralKey {
        &self.source
    }

    pub const fn uncovered_leaves(&self) -> &BTreeSet<ConcreteIntegralKey> {
        &self.uncovered_leaves
    }
}

impl fmt::Display for IncompleteReductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "reduction of {:?} retains {} uncovered terminal(s)",
            self.source,
            self.uncovered_leaves.len()
        )
    }
}

impl std::error::Error for IncompleteReductionError {}

#[derive(Clone, Debug)]
struct CachedReduction {
    terms: BTreeMap<ConcreteIntegralKey, Coefficient>,
    guards: Vec<SpecializedNonZeroCondition>,
    certified_domain: Vec<CertifiedRewriteDomainCondition>,
    application_traces: Vec<ConcreteRuleApplicationTrace>,
    terminal_statuses: BTreeMap<ConcreteIntegralKey, ConcreteTerminalStatus>,
}

pub struct ParametricReductionEngine<'context, Provider> {
    family_fingerprint: Arc<str>,
    context: &'context CoefficientContext,
    ordering: IntegralOrderingPolicy,
    provider: Provider,
    index_arity: usize,
    limits: ReductionEngineLimits,
    cache: BTreeMap<ConcreteIntegralKey, CachedReduction>,
    active: BTreeSet<ConcreteIntegralKey>,
    stats: ReductionEngineStats,
    cached_application_trace_entries: usize,
    cached_proof_debug_bytes: usize,
}

impl<'context, Provider> ParametricReductionEngine<'context, Provider>
where
    Provider: ConcreteRuleProvider,
{
    pub fn new(
        family_fingerprint: impl Into<Arc<str>>,
        context: &'context CoefficientContext,
        ordering: IntegralOrderingPolicy,
        provider: Provider,
        limits: ReductionEngineLimits,
    ) -> Self {
        let index_arity = provider.index_arity();
        Self {
            family_fingerprint: family_fingerprint.into(),
            context,
            ordering,
            provider,
            index_arity,
            limits,
            cache: BTreeMap::new(),
            active: BTreeSet::new(),
            stats: ReductionEngineStats::default(),
            cached_application_trace_entries: 0,
            cached_proof_debug_bytes: 0,
        }
    }

    pub fn provider(&self) -> &Provider {
        &self.provider
    }

    /// Semantic family identity authenticated by every accepted rule.
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    /// Exact base coefficient context used for all rule application and
    /// sparse collection, including a reduction whose source is identically
    /// zero and therefore issues no provider requests.
    pub const fn coefficient_context(&self) -> &CoefficientContext {
        self.context
    }

    /// Mutably access the provider after invalidating every engine cache entry.
    ///
    /// Provider arity is an invariant of this engine.  If mutation changes it,
    /// the next reduction fails with [`ReductionEngineError::ProviderArityChanged`].
    pub fn provider_mut(&mut self) -> &mut Provider {
        self.cache.clear();
        self.stats.cache_entries = 0;
        self.cached_application_trace_entries = 0;
        self.cached_proof_debug_bytes = 0;
        self.stats.cached_application_trace_entries = 0;
        self.stats.cached_proof_debug_bytes = 0;
        &mut self.provider
    }

    pub const fn index_arity(&self) -> usize {
        self.index_arity
    }

    pub const fn stats(&self) -> ReductionEngineStats {
        self.stats
    }

    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    pub fn reduce(
        &mut self,
        source: &ConcreteIntegralKey,
    ) -> Result<ParametricReductionResult, ReductionEngineError<Provider::Error>> {
        self.validate_provider_arity()?;
        self.validate_key_arity(source)?;
        let stats_before = self.stats;
        let cached_trace_entries_before = self.cached_application_trace_entries;
        let cached_proof_bytes_before = self.cached_proof_debug_bytes;
        let mut staged_cache_keys = Vec::new();
        let reduced = match self.reduce_one(source, &mut staged_cache_keys) {
            Ok(reduced) => reduced,
            Err(error) => {
                for key in staged_cache_keys {
                    self.cache.remove(&key);
                }
                self.active.clear();
                self.stats = stats_before;
                self.cached_application_trace_entries = cached_trace_entries_before;
                self.cached_proof_debug_bytes = cached_proof_bytes_before;
                return Err(error);
            }
        };
        let (uncovered_leaves, selected_masters, certified_masters) =
            classify_terminals(&reduced.terminal_statuses);
        Ok(ParametricReductionResult {
            family_fingerprint: self.family_fingerprint.clone(),
            source: source.clone(),
            terms: reduced.terms,
            required_nonzero: reduced.guards,
            certified_domain: reduced.certified_domain,
            application_traces: reduced.application_traces,
            terminal_statuses: reduced.terminal_statuses,
            uncovered_leaves,
            selected_masters,
            certified_masters,
            stats: self.stats,
            cached_application_trace_entries: self.cached_application_trace_entries,
            cached_proof_debug_bytes: self.cached_proof_debug_bytes,
        })
    }

    fn reduce_one(
        &mut self,
        source: &ConcreteIntegralKey,
        staged_cache_keys: &mut Vec<ConcreteIntegralKey>,
    ) -> Result<CachedReduction, ReductionEngineError<Provider::Error>> {
        self.validate_key_arity(source)?;
        let recursive_calls =
            checked_add(self.stats.recursive_calls, 1, "reduction recursive calls")?;
        check_limit(
            "recursive calls",
            recursive_calls,
            self.limits.max_recursive_calls,
        )?;
        self.stats.recursive_calls = recursive_calls;
        if let Some(cached) = self.cache.get(source) {
            self.stats.cache_hits = checked_add(self.stats.cache_hits, 1, "reduction cache hits")?;
            return Ok(cached.clone());
        }
        if self.active.contains(source) {
            return Err(ReductionEngineError::Cycle {
                integral: source.clone(),
            });
        }
        let active_depth = checked_add(self.active.len(), 1, "active reduction depth")?;
        check_limit("active depth", active_depth, self.limits.max_active_depth)?;
        self.active.insert(source.clone());

        let result = self.reduce_uncached(source, staged_cache_keys);
        self.active.remove(source);
        let result = result?;
        let requested = checked_add(self.cache.len(), 1, "reduction cache entries")?;
        check_limit("cache entries", requested, self.limits.max_cache_entries)?;
        let cached_trace_entries = checked_add(
            self.cached_application_trace_entries,
            result.application_traces.len(),
            "cached application trace entries",
        )?;
        check_limit(
            "cached application trace entries",
            cached_trace_entries,
            self.limits.max_cached_application_trace_entries,
        )?;
        let cached_proof_debug_bytes = charge_trace_debug_bytes(
            self.cached_proof_debug_bytes,
            &result.application_traces,
            self.limits.max_cached_proof_debug_bytes,
        )?;
        self.cache.insert(source.clone(), result.clone());
        staged_cache_keys.push(source.clone());
        self.cached_application_trace_entries = cached_trace_entries;
        self.cached_proof_debug_bytes = cached_proof_debug_bytes;
        self.stats.cache_entries = self.cache.len();
        self.stats.cached_application_trace_entries = cached_trace_entries;
        self.stats.cached_proof_debug_bytes = cached_proof_debug_bytes;
        Ok(result)
    }

    fn reduce_uncached(
        &mut self,
        source: &ConcreteIntegralKey,
        staged_cache_keys: &mut Vec<ConcreteIntegralKey>,
    ) -> Result<CachedReduction, ReductionEngineError<Provider::Error>> {
        let decision = self
            .provider
            .decision_for(source)
            .map_err(ReductionEngineError::Provider)?;
        let (rhs, required_nonzero, certified_domain, application_trace) = match decision {
            ConcreteRuleDecision::Reduction(rule) => {
                if rule.family_fingerprint() != self.family_fingerprint.as_ref() {
                    return Err(ReductionEngineError::WrongFamily);
                }
                if rule.source() != source {
                    return Err(ReductionEngineError::WrongRuleSource {
                        expected: source.clone(),
                        actual: rule.source().clone(),
                    });
                }
                if !rule.verify_application(
                    self.context,
                    self.ordering,
                    self.limits.exact_algebra,
                )? {
                    return Err(ReductionEngineError::InvalidDescentCertificate);
                }
                (
                    rule.rhs().clone(),
                    rule.required_nonzero().to_vec(),
                    Vec::new(),
                    ConcreteRuleApplicationTrace::Parametric(rule),
                )
            }
            ConcreteRuleDecision::ConditionalReduction(rule) => {
                if rule.family_fingerprint() != self.family_fingerprint.as_ref() {
                    return Err(ReductionEngineError::WrongFamily);
                }
                if rule.source() != source {
                    return Err(ReductionEngineError::WrongRuleSource {
                        expected: source.clone(),
                        actual: rule.source().clone(),
                    });
                }
                if !rule.verify_application(
                    self.context,
                    self.ordering,
                    self.limits.exact_algebra,
                )? {
                    return Err(ReductionEngineError::InvalidDescentCertificate);
                }
                (
                    rule.rhs().clone(),
                    rule.required_nonzero().to_vec(),
                    Vec::new(),
                    ConcreteRuleApplicationTrace::ConditionalParametric(rule),
                )
            }
            ConcreteRuleDecision::CertifiedRewrite(rule) => {
                if rule.family_fingerprint() != self.family_fingerprint.as_ref() {
                    return Err(ReductionEngineError::WrongFamily);
                }
                if rule.source() != source {
                    return Err(ReductionEngineError::WrongRuleSource {
                        expected: source.clone(),
                        actual: rule.source().clone(),
                    });
                }
                if !rule.verify_application(
                    self.context,
                    self.ordering,
                    self.limits.exact_algebra,
                )? {
                    return Err(ReductionEngineError::InvalidDescentCertificate);
                }
                (
                    rule.rhs().clone(),
                    rule.required_nonzero().to_vec(),
                    rule.domain().to_vec(),
                    ConcreteRuleApplicationTrace::CertifiedRewrite(rule),
                )
            }
            ConcreteRuleDecision::ProvedZero(zero) => {
                if zero.family_fingerprint() != self.family_fingerprint.as_ref() {
                    return Err(ReductionEngineError::WrongFamily);
                }
                if zero.source() != source {
                    return Err(ReductionEngineError::WrongRuleSource {
                        expected: source.clone(),
                        actual: zero.source().clone(),
                    });
                }
                (
                    BTreeMap::new(),
                    Vec::new(),
                    zero.domain().to_vec(),
                    ConcreteRuleApplicationTrace::ProvedZero(zero),
                )
            }
            ConcreteRuleDecision::Terminal(status) => {
                let mut terms = BTreeMap::new();
                add_checked_term(
                    self.context,
                    &mut terms,
                    source.clone(),
                    self.context.one(),
                    self.limits,
                )?;
                self.stats.maximum_result_terms = self.stats.maximum_result_terms.max(terms.len());
                return Ok(CachedReduction {
                    terms,
                    guards: Vec::new(),
                    certified_domain: Vec::new(),
                    application_traces: Vec::new(),
                    terminal_statuses: BTreeMap::from([(source.clone(), status)]),
                });
            }
        };
        let rule_applications = checked_add(
            self.stats.rule_applications,
            1,
            "reduction rule applications",
        )?;
        check_limit(
            "rule applications",
            rule_applications,
            self.limits.max_rule_applications,
        )?;
        self.stats.rule_applications = rule_applications;

        let mut output = BTreeMap::new();
        let mut guards = Vec::new();
        let mut retained_domain = Vec::new();
        let mut application_traces = Vec::new();
        let mut terminal_statuses = BTreeMap::new();
        for condition in &required_nonzero {
            let polynomial_coefficient: Coefficient = condition.polynomial().raw().clone().into();
            if let Err(error) = self
                .context
                .validate_with_limits(&polynomial_coefficient, self.limits.exact_algebra)
            {
                return match error {
                    ExactAlgebraError::VariableMapMismatch { .. } => {
                        Err(ReductionEngineError::ForeignGuard)
                    }
                    error => Err(error.into()),
                };
            }
            insert_guard(
                &mut guards,
                condition.clone(),
                self.limits.max_guard_polynomials,
                self.limits.max_guard_origins,
            )?;
        }
        for condition in certified_domain {
            let polynomial_coefficient: Coefficient = condition.polynomial().clone().into();
            if let Err(error) = self
                .context
                .validate_with_limits(&polynomial_coefficient, self.limits.exact_algebra)
            {
                return match error {
                    ExactAlgebraError::VariableMapMismatch { .. } => {
                        Err(ReductionEngineError::ForeignCertifiedDomain)
                    }
                    error => Err(error.into()),
                };
            }
            insert_certified_domain(
                &mut retained_domain,
                condition,
                self.limits.max_guard_polynomials,
                self.limits.max_guard_origins,
            )?;
        }
        insert_application_trace(
            &mut application_traces,
            application_trace,
            self.limits.max_application_traces,
        )?;

        for (target, coefficient) in &rhs {
            self.validate_key_arity(target)?;
            self.context
                .validate_with_limits(coefficient, self.limits.exact_algebra)?;
            let target_reduction = self.reduce_one(target, staged_cache_keys)?;
            for guard in target_reduction.guards {
                insert_guard(
                    &mut guards,
                    guard,
                    self.limits.max_guard_polynomials,
                    self.limits.max_guard_origins,
                )?;
            }
            for condition in target_reduction.certified_domain {
                insert_certified_domain(
                    &mut retained_domain,
                    condition,
                    self.limits.max_guard_polynomials,
                    self.limits.max_guard_origins,
                )?;
            }
            for trace in target_reduction.application_traces {
                insert_application_trace(
                    &mut application_traces,
                    trace,
                    self.limits.max_application_traces,
                )?;
            }
            for (terminal, status) in target_reduction.terminal_statuses {
                insert_terminal_status(&mut terminal_statuses, terminal, status)?;
            }
            for (leaf, leaf_coefficient) in target_reduction.terms {
                let sparse_updates =
                    checked_add(self.stats.sparse_updates, 1, "reduction sparse updates")?;
                check_limit(
                    "sparse updates",
                    sparse_updates,
                    self.limits.max_sparse_updates,
                )?;
                self.stats.sparse_updates = sparse_updates;
                let product = self.context.try_mul(
                    coefficient,
                    &leaf_coefficient,
                    self.limits.exact_algebra,
                )?;
                add_checked_term(self.context, &mut output, leaf, product, self.limits)?;
            }
        }
        terminal_statuses.retain(|terminal, _| output.contains_key(terminal));
        if output.keys().ne(terminal_statuses.keys()) {
            return Err(ReductionEngineError::TerminalCoverageMismatch);
        }
        self.stats.maximum_result_terms = self.stats.maximum_result_terms.max(output.len());
        Ok(CachedReduction {
            terms: output,
            guards,
            certified_domain: retained_domain,
            application_traces,
            terminal_statuses,
        })
    }

    fn validate_provider_arity(&self) -> Result<(), ReductionEngineError<Provider::Error>> {
        let actual = self.provider.index_arity();
        if actual == self.index_arity {
            Ok(())
        } else {
            Err(ReductionEngineError::ProviderArityChanged {
                expected: self.index_arity,
                actual,
            })
        }
    }

    fn validate_key_arity(
        &self,
        key: &ConcreteIntegralKey,
    ) -> Result<(), ReductionEngineError<Provider::Error>> {
        if key.powers().len() == self.index_arity {
            Ok(())
        } else {
            Err(ReductionEngineError::WrongArity {
                expected: self.index_arity,
                actual: key.powers().len(),
            })
        }
    }
}

fn classify_terminals(
    statuses: &BTreeMap<ConcreteIntegralKey, ConcreteTerminalStatus>,
) -> (
    BTreeSet<ConcreteIntegralKey>,
    BTreeSet<ConcreteIntegralKey>,
    BTreeMap<ConcreteIntegralKey, Arc<str>>,
) {
    let mut uncovered = BTreeSet::new();
    let mut selected = BTreeSet::new();
    let mut certified = BTreeMap::new();
    for (integral, status) in statuses {
        match status {
            ConcreteTerminalStatus::Uncovered => {
                uncovered.insert(integral.clone());
            }
            ConcreteTerminalStatus::SelectedMaster => {
                selected.insert(integral.clone());
            }
            ConcreteTerminalStatus::CertifiedMaster {
                certificate_fingerprint,
            } => {
                certified.insert(integral.clone(), certificate_fingerprint.clone());
            }
        }
    }
    (uncovered, selected, certified)
}

fn insert_terminal_status<ProviderError>(
    statuses: &mut BTreeMap<ConcreteIntegralKey, ConcreteTerminalStatus>,
    integral: ConcreteIntegralKey,
    status: ConcreteTerminalStatus,
) -> Result<(), ReductionEngineError<ProviderError>>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    if let Some(existing) = statuses.get(&integral) {
        if existing != &status {
            return Err(ReductionEngineError::ConflictingTerminalStatus {
                integral,
                first: existing.clone(),
                second: status,
            });
        }
        return Ok(());
    }
    statuses.insert(integral, status);
    Ok(())
}

fn insert_guard<ProviderError>(
    guards: &mut Vec<SpecializedNonZeroCondition>,
    guard: SpecializedNonZeroCondition,
    limit: usize,
    origin_limit: usize,
) -> Result<(), ReductionEngineError<ProviderError>>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    let current_origins = guards.iter().try_fold(0usize, |count, condition| {
        checked_add(count, condition.origins().len(), "reduction guard origins")
    })?;
    if let Some(existing) = guards
        .iter_mut()
        .find(|existing| existing.polynomial() == guard.polynomial())
    {
        let additional = guard
            .origins()
            .iter()
            .filter(|origin| !existing.origins().contains(*origin))
            .count();
        check_limit(
            "guard origins",
            checked_add(current_origins, additional, "reduction guard origins")?,
            origin_limit,
        )?;
        existing.merge_origins_from(&guard, origin_limit)?;
        return Ok(());
    }
    check_limit(
        "guard origins",
        checked_add(
            current_origins,
            guard.origins().len(),
            "reduction guard origins",
        )?,
        origin_limit,
    )?;
    let requested = checked_add(guards.len(), 1, "reduction guard polynomials")?;
    check_limit("guard polynomials", requested, limit)?;
    guards.push(guard);
    Ok(())
}

fn insert_certified_domain<ProviderError>(
    conditions: &mut Vec<CertifiedRewriteDomainCondition>,
    condition: CertifiedRewriteDomainCondition,
    polynomial_limit: usize,
    origin_limit: usize,
) -> Result<(), ReductionEngineError<ProviderError>>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    let current_origins = conditions.iter().try_fold(0usize, |count, existing| {
        checked_add(count, existing.origins().len(), "certified domain origins")
    })?;
    if let Some(existing) = conditions
        .iter_mut()
        .find(|existing| existing.polynomial() == condition.polynomial())
    {
        let additional = condition
            .origins()
            .iter()
            .filter(|origin| !existing.origins().contains(*origin))
            .count();
        check_limit(
            "certified domain origins",
            checked_add(current_origins, additional, "certified domain origins")?,
            origin_limit,
        )?;
        existing.merge_origins_from(&condition);
        return Ok(());
    }
    check_limit(
        "certified domain origins",
        checked_add(
            current_origins,
            condition.origins().len(),
            "certified domain origins",
        )?,
        origin_limit,
    )?;
    check_limit(
        "certified domain polynomials",
        checked_add(conditions.len(), 1, "certified domain polynomials")?,
        polynomial_limit,
    )?;
    conditions.push(condition);
    Ok(())
}

fn insert_application_trace<ProviderError>(
    traces: &mut Vec<ConcreteRuleApplicationTrace>,
    trace: ConcreteRuleApplicationTrace,
    limit: usize,
) -> Result<(), ReductionEngineError<ProviderError>>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    let requested = checked_add(traces.len(), 1, "retained rule application traces")?;
    check_limit("application traces", requested, limit)?;
    traces.push(trace);
    Ok(())
}

struct BoundedTraceByteCounter {
    bytes: usize,
    limit: usize,
}

impl fmt::Write for BoundedTraceByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.bytes = self.bytes.checked_add(value.len()).ok_or(fmt::Error)?;
        if self.bytes > self.limit {
            return Err(fmt::Error);
        }
        Ok(())
    }
}

fn charge_trace_debug_bytes<ProviderError>(
    retained: usize,
    traces: &[ConcreteRuleApplicationTrace],
    limit: usize,
) -> Result<usize, ReductionEngineError<ProviderError>>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    let mut writer = BoundedTraceByteCounter {
        bytes: 0,
        limit: limit.saturating_sub(retained),
    };
    for trace in traces {
        if write!(&mut writer, "{trace:?}").is_err() {
            return Err(ReductionEngineError::ResourceLimit {
                resource: "cached proof debug bytes",
                requested: limit.saturating_add(1),
                limit,
            });
        }
    }
    let requested = checked_add(retained, writer.bytes, "cached proof debug bytes")?;
    check_limit("cached proof debug bytes", requested, limit)?;
    Ok(requested)
}

fn add_checked_term<ProviderError>(
    context: &CoefficientContext,
    output: &mut BTreeMap<ConcreteIntegralKey, Coefficient>,
    key: ConcreteIntegralKey,
    coefficient: Coefficient,
    limits: ReductionEngineLimits,
) -> Result<(), ReductionEngineError<ProviderError>>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    if coefficient.is_zero() {
        return Ok(());
    }
    if let Some(existing) = output.get(&key) {
        let sum = context.try_add(existing, &coefficient, limits.exact_algebra)?;
        if sum.is_zero() {
            output.remove(&key);
        } else {
            output.insert(key, sum);
        }
    } else {
        let requested = checked_add(output.len(), 1, "terms in one reduction result")?;
        check_limit("terms per result", requested, limits.max_terms_per_result)?;
        output.insert(key, coefficient);
    }
    Ok(())
}

#[derive(Debug)]
pub enum ReductionEngineError<ProviderError>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    Provider(ProviderError),
    WrongArity {
        expected: usize,
        actual: usize,
    },
    ProviderArityChanged {
        expected: usize,
        actual: usize,
    },
    WrongFamily,
    WrongRuleSource {
        expected: ConcreteIntegralKey,
        actual: ConcreteIntegralKey,
    },
    InvalidDescentCertificate,
    ForeignGuard,
    ForeignCertifiedDomain,
    Cycle {
        integral: ConcreteIntegralKey,
    },
    ConflictingTerminalStatus {
        integral: ConcreteIntegralKey,
        first: ConcreteTerminalStatus,
        second: ConcreteTerminalStatus,
    },
    TerminalCoverageMismatch,
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ExactAlgebra(ExactAlgebraError),
    ParametricCoefficient(ParametricCoefficientError),
    Relation(ParametricRelationError),
}

impl<ProviderError> fmt::Display for ReductionEngineError<ProviderError>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(error) => write!(formatter, "concrete rule provider failed: {error}"),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "concrete reduction index arity is {actual}, expected {expected}"
            ),
            Self::ProviderArityChanged { expected, actual } => write!(
                formatter,
                "concrete rule provider arity changed from {expected} to {actual}"
            ),
            Self::WrongFamily => formatter.write_str("concrete rule belongs to another family"),
            Self::WrongRuleSource { expected, actual } => write!(
                formatter,
                "concrete rule source {actual:?} does not match requested {expected:?}"
            ),
            Self::InvalidDescentCertificate => {
                formatter.write_str("concrete reduction has an invalid descent certificate")
            }
            Self::ForeignGuard => formatter.write_str("concrete reduction guard is foreign"),
            Self::ForeignCertifiedDomain => {
                formatter.write_str("certified rewrite domain condition is foreign")
            }
            Self::Cycle { integral } => {
                write!(formatter, "concrete reduction cycle reached {integral:?}")
            }
            Self::ConflictingTerminalStatus {
                integral,
                first,
                second,
            } => write!(
                formatter,
                "concrete terminal {integral:?} has conflicting statuses {first:?} and {second:?}"
            ),
            Self::TerminalCoverageMismatch => formatter.write_str(
                "concrete reduction terminal statuses do not cover its surviving output terms",
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "reduction {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::ParametricCoefficient(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
        }
    }
}

impl<ProviderError> std::error::Error for ReductionEngineError<ProviderError> where
    ProviderError: std::error::Error + Send + Sync + 'static
{
}

impl<ProviderError> From<ExactAlgebraError> for ReductionEngineError<ProviderError>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

impl<ProviderError> From<ParametricRelationError> for ReductionEngineError<ProviderError>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

impl<ProviderError> From<ParametricCoefficientError> for ReductionEngineError<ProviderError>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    fn from(value: ParametricCoefficientError) -> Self {
        Self::ParametricCoefficient(value)
    }
}

fn checked_add<ProviderError>(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, ReductionEngineError<ProviderError>>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    left.checked_add(right)
        .ok_or(ReductionEngineError::ResourceCountOverflow { resource })
}

fn check_limit<ProviderError>(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ReductionEngineError<ProviderError>>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    if requested > limit {
        Err(ReductionEngineError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
