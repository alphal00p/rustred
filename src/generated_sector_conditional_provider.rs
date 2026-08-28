//! Conditional-rule fallback for terminal leaves of generated sector coverage.
//!
//! The ordinary sector provider remains authoritative wherever its replayed
//! root coverage selected a global descending rule.  Only a root
//! `Uncovered` or `Unsupported` leaf activates this wrapper's deterministic
//! scan of condition-bound pivots collected from the corresponding live-leaf
//! queue.  Every pivot retains its partial re-elimination proof and is applied
//! on its own exact centered equality locus; the source work-item leaf is
//! provenance, not an extra applicability restriction.
//!
//! An inapplicable conditional pivot, an empty conditional system, and a
//! coordinate-empty source leaf all delegate unchanged to the wrapped
//! provider.  None of them is promoted to a master or zero rule.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::conditional_reelimination::certificate_payload_eq;
use crate::reduction_engine::{ConcreteRuleDecision, ConcreteRuleProvider};
use crate::{
    ConcreteIntegralKey, ConditionalParametricRule, ConditionalParametricRuleApplication,
    ConditionalParametricRuleError, ConditionalParametricRuleLimits,
    GeneratedPartialReeliminationCertificate, GeneratedPartialReeliminationCompilation,
    GeneratedSectorLiveLeafOutcome, GeneratedSectorLiveLeafQueueCertificate,
    GeneratedSectorLiveLeafQueueError, GeneratedSymbolicRowSpanCertificate, IntegralFamily,
    ParametricCoefficientContext, ParametricSectorCoverageError, ParametricSectorLeafDisposition,
    SectorFoundationError, SectorMask, SymbolicSectorCaseId,
};

pub const GENERATED_SECTOR_CONDITIONAL_RULE_PROVIDER_V1_SCHEMA: &str =
    "rustred-generated-sector-conditional-rule-provider-v1";

/// Aggregate retained-proof and query budgets for the conditional fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedSectorConditionalRuleProviderLimits {
    pub conditional_rule: ConditionalParametricRuleLimits,
    pub max_queue_certificates: usize,
    pub max_total_root_leaves: usize,
    pub max_total_work_items: usize,
    pub max_total_certified_partial_reeliminations: usize,
    pub max_total_empty_partial_systems: usize,
    pub max_total_conditional_pivots: usize,
    pub max_total_installed_rules: usize,
    pub max_total_skipped_contradictory_loci: usize,
    pub max_total_conditional_transcript_bytes: usize,
    pub max_queries: usize,
    pub max_rules_considered_per_query: usize,
    pub max_total_rule_attempts: usize,
}

impl Default for GeneratedSectorConditionalRuleProviderLimits {
    fn default() -> Self {
        Self {
            conditional_rule: ConditionalParametricRuleLimits::default(),
            max_queue_certificates: 1_000_000,
            max_total_root_leaves: 16_000_000,
            max_total_work_items: 16_000_000,
            max_total_certified_partial_reeliminations: 16_000_000,
            max_total_empty_partial_systems: 16_000_000,
            max_total_conditional_pivots: 100_000_000,
            max_total_installed_rules: 100_000_000,
            max_total_skipped_contradictory_loci: 100_000_000,
            max_total_conditional_transcript_bytes: 8 * 1024 * 1024 * 1024,
            max_queries: 100_000_000,
            max_rules_considered_per_query: 16_000_000,
            max_total_rule_attempts: 1_000_000_000,
        }
    }
}

/// Immutable census of all replayed queue material retained by the wrapper.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedSectorConditionalRuleProviderBuildStats {
    queue_certificates: usize,
    root_leaves: usize,
    work_items: usize,
    certified_partial_reeliminations: usize,
    empty_partial_systems: usize,
    conditional_pivots: usize,
    installed_rules: usize,
    skipped_contradictory_loci: usize,
    /// Bytes of the queue-owned conditional transcripts plus the one retained
    /// source-certificate copy shared by every installed rule from that
    /// transcript.
    conditional_transcript_bytes: usize,
}

impl GeneratedSectorConditionalRuleProviderBuildStats {
    pub const fn queue_certificates(self) -> usize {
        self.queue_certificates
    }
    pub const fn root_leaves(self) -> usize {
        self.root_leaves
    }
    pub const fn work_items(self) -> usize {
        self.work_items
    }
    pub const fn certified_partial_reeliminations(self) -> usize {
        self.certified_partial_reeliminations
    }
    pub const fn empty_partial_systems(self) -> usize {
        self.empty_partial_systems
    }
    pub const fn conditional_pivots(self) -> usize {
        self.conditional_pivots
    }
    pub const fn installed_rules(self) -> usize {
        self.installed_rules
    }
    pub const fn skipped_contradictory_loci(self) -> usize {
        self.skipped_contradictory_loci
    }
    pub const fn conditional_transcript_bytes(self) -> usize {
        self.conditional_transcript_bytes
    }
}

/// Runtime routing census.  Failed conditional applications are fail-closed;
/// work performed before an error remains counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedSectorConditionalRuleProviderStats {
    queries: usize,
    missing_sector_delegations: usize,
    global_rule_delegations: usize,
    terminal_fallback_queries: usize,
    conditional_rule_attempts: usize,
    conditional_reductions: usize,
    inapplicable_conditional_rules: usize,
    exhausted_fallback_delegations: usize,
}

impl GeneratedSectorConditionalRuleProviderStats {
    pub const fn queries(self) -> usize {
        self.queries
    }
    pub const fn missing_sector_delegations(self) -> usize {
        self.missing_sector_delegations
    }
    pub const fn global_rule_delegations(self) -> usize {
        self.global_rule_delegations
    }
    pub const fn terminal_fallback_queries(self) -> usize {
        self.terminal_fallback_queries
    }
    pub const fn conditional_rule_attempts(self) -> usize {
        self.conditional_rule_attempts
    }
    pub const fn conditional_reductions(self) -> usize {
        self.conditional_reductions
    }
    pub const fn inapplicable_conditional_rules(self) -> usize {
        self.inapplicable_conditional_rules
    }
    pub const fn exhausted_fallback_delegations(self) -> usize {
        self.exhausted_fallback_delegations
    }
}

/// Stable source coordinates for an installed condition-bound pivot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedSectorConditionalRuleProvenance {
    work_item_ordinal: usize,
    source_case: SymbolicSectorCaseId,
    pivot_ordinal: usize,
}

impl GeneratedSectorConditionalRuleProvenance {
    pub const fn work_item_ordinal(&self) -> usize {
        self.work_item_ordinal
    }
    pub const fn source_case(&self) -> SymbolicSectorCaseId {
        self.source_case
    }
    pub const fn pivot_ordinal(&self) -> usize {
        self.pivot_ordinal
    }
}

/// A contradictory centered equality is a certified empty intersection with
/// the queue's sector.  It is retained explicitly rather than silently lost.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedSectorSkippedConditionalLocus {
    provenance: GeneratedSectorConditionalRuleProvenance,
    position: usize,
    value: i64,
    active: bool,
}

impl GeneratedSectorSkippedConditionalLocus {
    pub const fn provenance(&self) -> &GeneratedSectorConditionalRuleProvenance {
        &self.provenance
    }
    pub const fn position(&self) -> usize {
        self.position
    }
    pub const fn value(&self) -> i64 {
        self.value
    }
    pub const fn active(&self) -> bool {
        self.active
    }
}

#[derive(Clone)]
struct InstalledConditionalRule {
    provenance: GeneratedSectorConditionalRuleProvenance,
    source_certificate: Arc<GeneratedPartialReeliminationCertificate>,
    rule: ConditionalParametricRule,
}

impl fmt::Debug for InstalledConditionalRule {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InstalledConditionalRule")
            .field("provenance", &self.provenance)
            .field("rule", &self.rule)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
struct SectorConditionalQueue {
    queue: GeneratedSectorLiveLeafQueueCertificate,
    rules: Box<[InstalledConditionalRule]>,
    skipped: Box<[GeneratedSectorSkippedConditionalLocus]>,
}

/// Generic wrapper that adds certified condition-bound pivots only as a
/// fallback on terminal leaves of the corresponding root coverage.
pub struct GeneratedSectorConditionalRuleProvider<'family, Provider> {
    family: &'family IntegralFamily,
    context: &'family ParametricCoefficientContext,
    inner: Provider,
    index_arity: usize,
    sectors: BTreeMap<SectorMask, SectorConditionalQueue>,
    limits: GeneratedSectorConditionalRuleProviderLimits,
    build_stats: GeneratedSectorConditionalRuleProviderBuildStats,
    stats: GeneratedSectorConditionalRuleProviderStats,
}

impl<'family, Provider> GeneratedSectorConditionalRuleProvider<'family, Provider>
where
    Provider: ConcreteRuleProvider,
{
    pub const SCHEMA: &'static str = GENERATED_SECTOR_CONDITIONAL_RULE_PROVIDER_V1_SCHEMA;

    pub fn try_new(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        queues: impl IntoIterator<Item = GeneratedSectorLiveLeafQueueCertificate>,
        inner: Provider,
        limits: GeneratedSectorConditionalRuleProviderLimits,
    ) -> Result<Self, GeneratedSectorConditionalRuleProviderError<Provider::Error>> {
        Self::try_new_impl(family, context, queues, inner, None, limits)
    }

    /// Install queues whose complete payloads were just replayed by a
    /// family-wide certificate against one immutable generated row span.
    /// Public callers use [`Self::try_new`] and receive independent replay.
    pub(crate) fn try_new_with_replayed_queues(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        queues: impl IntoIterator<Item = GeneratedSectorLiveLeafQueueCertificate>,
        inner: Provider,
        shared_row_span: &Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: GeneratedSectorConditionalRuleProviderLimits,
    ) -> Result<Self, GeneratedSectorConditionalRuleProviderError<Provider::Error>> {
        Self::try_new_impl(
            family,
            context,
            queues,
            inner,
            Some(shared_row_span),
            limits,
        )
    }

    /// Check all queue/rule retention bounds using borrowed transcript
    /// metadata before a family-level owner deep-clones any queue.
    pub(crate) fn preflight_queues<'a>(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        queues: impl IntoIterator<Item = &'a GeneratedSectorLiveLeafQueueCertificate>,
        limits: GeneratedSectorConditionalRuleProviderLimits,
    ) -> Result<
        GeneratedSectorConditionalRuleProviderBuildStats,
        GeneratedSectorConditionalRuleProviderError<Provider::Error>,
    > {
        validate_family_context::<Provider::Error>(family, context)?;
        let mut sectors = std::collections::BTreeSet::new();
        let mut stats = GeneratedSectorConditionalRuleProviderBuildStats::default();
        for queue in queues {
            stats.queue_certificates = bounded_add::<Provider::Error>(
                "conditional queue certificates",
                stats.queue_certificates,
                1,
                limits.max_queue_certificates,
            )?;
            validate_queue_scope::<Provider::Error>(family, context, queue)?;
            let sector = queue.sector().clone();
            if !sectors.insert(sector.clone()) {
                return Err(
                    GeneratedSectorConditionalRuleProviderError::DuplicateSector { sector },
                );
            }
            stats = preflight_queue_rules::<Provider::Error>(queue, limits, stats)?;
        }
        Ok(stats)
    }

    fn try_new_impl(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        queues: impl IntoIterator<Item = GeneratedSectorLiveLeafQueueCertificate>,
        inner: Provider,
        replayed_row_span: Option<&Arc<GeneratedSymbolicRowSpanCertificate>>,
        limits: GeneratedSectorConditionalRuleProviderLimits,
    ) -> Result<Self, GeneratedSectorConditionalRuleProviderError<Provider::Error>> {
        validate_family_context::<Provider::Error>(family, context)?;
        let index_arity = context.index_count();
        let inner_arity = inner.index_arity();
        if inner_arity != index_arity {
            return Err(
                GeneratedSectorConditionalRuleProviderError::InnerProviderArityChanged {
                    expected: index_arity,
                    actual: inner_arity,
                },
            );
        }

        let mut sectors = BTreeMap::new();
        let mut build_stats = GeneratedSectorConditionalRuleProviderBuildStats::default();
        for queue in queues {
            build_stats.queue_certificates = bounded_add::<Provider::Error>(
                "conditional queue certificates",
                build_stats.queue_certificates,
                1,
                limits.max_queue_certificates,
            )?;
            validate_queue_scope::<Provider::Error>(family, context, &queue)?;
            if let Some(row_span) = replayed_row_span {
                if !Arc::ptr_eq(queue.discovery().row_span_arc(), row_span) {
                    return Err(GeneratedSectorConditionalRuleProviderError::ReplayedRowSpanAllocationMismatch {
                        sector: queue.sector().clone(),
                    });
                }
            }
            let sector = queue.sector().clone();
            if sectors.contains_key(&sector) {
                return Err(
                    GeneratedSectorConditionalRuleProviderError::DuplicateSector { sector },
                );
            }
            // All queue-owned and rule-owned aggregate retention is knowable
            // from the immutable transcript.  Enforce it before replaying the
            // queue or deep-cloning any partial certificate.
            preflight_queue_rules::<Provider::Error>(&queue, limits, build_stats)?;
            if replayed_row_span.is_none() {
                queue.replay(family, context)?;
            }
            let (compiled, skipped, next_stats) = compile_queue_rules::<Provider::Error>(
                family,
                context,
                &queue,
                limits,
                build_stats,
                replayed_row_span.is_none(),
            )?;
            build_stats = next_stats;
            sectors.insert(
                sector,
                SectorConditionalQueue {
                    queue,
                    rules: compiled.into_boxed_slice(),
                    skipped: skipped.into_boxed_slice(),
                },
            );
        }

        Ok(Self {
            family,
            context,
            inner,
            index_arity,
            sectors,
            limits,
            build_stats,
            stats: GeneratedSectorConditionalRuleProviderStats::default(),
        })
    }

    pub const fn family(&self) -> &IntegralFamily {
        self.family
    }
    pub const fn context(&self) -> &ParametricCoefficientContext {
        self.context
    }
    pub const fn inner(&self) -> &Provider {
        &self.inner
    }
    pub fn inner_mut(&mut self) -> &mut Provider {
        &mut self.inner
    }
    pub fn into_inner(self) -> Provider {
        self.inner
    }
    pub const fn limits(&self) -> GeneratedSectorConditionalRuleProviderLimits {
        self.limits
    }
    pub const fn build_stats(&self) -> GeneratedSectorConditionalRuleProviderBuildStats {
        self.build_stats
    }
    pub const fn stats(&self) -> GeneratedSectorConditionalRuleProviderStats {
        self.stats
    }
    pub fn queues(
        &self,
    ) -> impl ExactSizeIterator<Item = &GeneratedSectorLiveLeafQueueCertificate> {
        self.sectors.values().map(|sector| &sector.queue)
    }
    pub fn rule_provenance(
        &self,
        sector: &SectorMask,
    ) -> Option<impl ExactSizeIterator<Item = &GeneratedSectorConditionalRuleProvenance>> {
        self.sectors
            .get(sector)
            .map(|entry| entry.rules.iter().map(|rule| &rule.provenance))
    }
    pub fn skipped_loci(
        &self,
        sector: &SectorMask,
    ) -> Option<&[GeneratedSectorSkippedConditionalLocus]> {
        self.sectors.get(sector).map(|entry| entry.skipped.as_ref())
    }

    /// Replay every input queue, its source partial certificate, and each
    /// installed condition-bound rule.  Runtime query counters are excluded.
    pub fn replay(
        &self,
    ) -> Result<(), GeneratedSectorConditionalRuleProviderError<Provider::Error>> {
        self.replay_impl(true)
    }

    /// Revalidate installed bindings after the owning family certificate has
    /// just replayed every retained queue. This still rebuilds and compares
    /// each installed rule, but does not replay a queue, partial certificate,
    /// or generated row span a second time.
    pub(crate) fn replay_with_replayed_queues(
        &self,
    ) -> Result<(), GeneratedSectorConditionalRuleProviderError<Provider::Error>> {
        self.replay_impl(false)
    }

    fn replay_impl(
        &self,
        replay_queues: bool,
    ) -> Result<(), GeneratedSectorConditionalRuleProviderError<Provider::Error>> {
        validate_family_context::<Provider::Error>(self.family, self.context)?;
        self.validate_inner_arity()?;
        let mut replayed_stats = GeneratedSectorConditionalRuleProviderBuildStats::default();
        for (sector, entry) in &self.sectors {
            replayed_stats.queue_certificates = bounded_add::<Provider::Error>(
                "conditional queue certificates",
                replayed_stats.queue_certificates,
                1,
                self.limits.max_queue_certificates,
            )?;
            validate_queue_scope::<Provider::Error>(self.family, self.context, &entry.queue)?;
            if entry.queue.sector() != sector {
                return Err(
                    GeneratedSectorConditionalRuleProviderError::ReplayMismatch {
                        detail: "retained queue is stored under a different sector",
                    },
                );
            }
            preflight_queue_rules::<Provider::Error>(&entry.queue, self.limits, replayed_stats)?;
            if replay_queues {
                entry.queue.replay(self.family, self.context)?;
            }
            let (rules, skipped, next_stats) = compile_queue_rules::<Provider::Error>(
                self.family,
                self.context,
                &entry.queue,
                self.limits,
                replayed_stats,
                replay_queues,
            )?;
            replayed_stats = next_stats;
            if rules.len() != entry.rules.len() || skipped.as_slice() != entry.skipped.as_ref() {
                return Err(
                    GeneratedSectorConditionalRuleProviderError::ReplayMismatch {
                        detail: "conditional rule or skipped-locus census differs",
                    },
                );
            }
            for (retained, rebuilt) in entry.rules.iter().zip(rules.iter()) {
                if replay_queues {
                    retained.rule.replay(self.family, self.context)?;
                }
                if retained.provenance != rebuilt.provenance
                    || !retained.rule.payload_eq(&rebuilt.rule)
                    || !Arc::ptr_eq(&retained.source_certificate, retained.rule.certificate())
                    || !certificate_payload_eq(
                        &retained.source_certificate,
                        &rebuilt.source_certificate,
                    )
                {
                    return Err(
                        GeneratedSectorConditionalRuleProviderError::ReplayMismatch {
                            detail: "installed conditional rule provenance differs",
                        },
                    );
                }
            }
        }
        if replayed_stats != self.build_stats {
            return Err(
                GeneratedSectorConditionalRuleProviderError::ReplayMismatch {
                    detail: "conditional provider build census differs",
                },
            );
        }
        Ok(())
    }

    fn validate_inner_arity(
        &self,
    ) -> Result<(), GeneratedSectorConditionalRuleProviderError<Provider::Error>> {
        let actual = self.inner.index_arity();
        if actual == self.index_arity {
            Ok(())
        } else {
            Err(
                GeneratedSectorConditionalRuleProviderError::InnerProviderArityChanged {
                    expected: self.index_arity,
                    actual,
                },
            )
        }
    }

    fn delegate(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, GeneratedSectorConditionalRuleProviderError<Provider::Error>>
    {
        self.inner
            .decision_for(integral)
            .map_err(GeneratedSectorConditionalRuleProviderError::Inner)
    }
}

impl<Provider> ConcreteRuleProvider for GeneratedSectorConditionalRuleProvider<'_, Provider>
where
    Provider: ConcreteRuleProvider,
{
    type Error = GeneratedSectorConditionalRuleProviderError<Provider::Error>;

    fn index_arity(&self) -> usize {
        self.index_arity
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        self.validate_inner_arity()?;
        if integral.powers().len() != self.index_arity {
            return Err(GeneratedSectorConditionalRuleProviderError::WrongArity {
                expected: self.index_arity,
                actual: integral.powers().len(),
            });
        }
        self.stats.queries = bounded_add(
            "conditional provider queries",
            self.stats.queries,
            1,
            self.limits.max_queries,
        )?;
        let sector = SectorMask::try_from_indices(integral.powers())?;
        let Some(entry) = self.sectors.get(&sector) else {
            self.stats.missing_sector_delegations = checked_add(
                "conditional missing-sector delegations",
                self.stats.missing_sector_delegations,
                1,
            )?;
            return self.delegate(integral);
        };
        let classification = entry
            .queue
            .discovery()
            .coverage()
            .classification_for_indices(self.context, integral.powers())?
            .ok_or_else(
                || GeneratedSectorConditionalRuleProviderError::CoveragePointMissing {
                    sector: sector.clone(),
                },
            )?;

        if matches!(
            classification.disposition(),
            ParametricSectorLeafDisposition::DescendingRule { .. }
        ) {
            self.stats.global_rule_delegations = checked_add(
                "conditional global-rule delegations",
                self.stats.global_rule_delegations,
                1,
            )?;
            return self.delegate(integral);
        }
        if let ParametricSectorLeafDisposition::ProvedEmptyLocus { reason } =
            classification.disposition()
        {
            return Err(
                GeneratedSectorConditionalRuleProviderError::ProvedEmptyLocusMatched {
                    sector,
                    reason: reason.clone(),
                },
            );
        }

        self.stats.terminal_fallback_queries = checked_add(
            "conditional terminal-fallback queries",
            self.stats.terminal_fallback_queries,
            1,
        )?;
        check_limit::<Provider::Error>(
            "conditional rules considered per query",
            entry.rules.len(),
            self.limits.max_rules_considered_per_query,
        )?;

        // Borrow the immutable context and rule table separately from the
        // mutable statistics.  The returned reduction owns its rule proof.
        for installed in entry.rules.iter() {
            self.stats.conditional_rule_attempts = bounded_add(
                "aggregate conditional rule attempts",
                self.stats.conditional_rule_attempts,
                1,
                self.limits.max_total_rule_attempts,
            )?;
            match installed.rule.apply(self.context, integral.powers())? {
                ConditionalParametricRuleApplication::Applicable(reduction) => {
                    self.stats.conditional_reductions = checked_add(
                        "conditional reductions",
                        self.stats.conditional_reductions,
                        1,
                    )?;
                    return Ok(ConcreteRuleDecision::ConditionalReduction(reduction));
                }
                ConditionalParametricRuleApplication::Inapplicable(_) => {
                    self.stats.inapplicable_conditional_rules = checked_add(
                        "inapplicable conditional rules",
                        self.stats.inapplicable_conditional_rules,
                        1,
                    )?;
                }
            }
        }
        self.stats.exhausted_fallback_delegations = checked_add(
            "exhausted conditional-fallback delegations",
            self.stats.exhausted_fallback_delegations,
            1,
        )?;
        self.delegate(integral)
    }
}

fn preflight_queue_rules<ProviderError>(
    queue: &GeneratedSectorLiveLeafQueueCertificate,
    limits: GeneratedSectorConditionalRuleProviderLimits,
    mut stats: GeneratedSectorConditionalRuleProviderBuildStats,
) -> Result<
    GeneratedSectorConditionalRuleProviderBuildStats,
    GeneratedSectorConditionalRuleProviderError<ProviderError>,
>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    stats.root_leaves = bounded_add(
        "conditional root leaves",
        stats.root_leaves,
        queue.stats().global_leaves(),
        limits.max_total_root_leaves,
    )?;
    stats.work_items = bounded_add(
        "conditional queue work items",
        stats.work_items,
        queue.work_items().len(),
        limits.max_total_work_items,
    )?;
    stats.conditional_transcript_bytes = bounded_add(
        "conditional queue transcript bytes",
        stats.conditional_transcript_bytes,
        queue.stats().conditional_transcript_bytes(),
        limits.max_total_conditional_transcript_bytes,
    )?;

    for item in queue.work_items() {
        let GeneratedSectorLiveLeafOutcome::PartialReelimination { compilation, .. } =
            item.outcome()
        else {
            continue;
        };
        let GeneratedPartialReeliminationCompilation::Certified(certificate) = compilation else {
            stats.empty_partial_systems = bounded_add(
                "conditional empty partial systems",
                stats.empty_partial_systems,
                1,
                limits.max_total_empty_partial_systems,
            )?;
            continue;
        };
        stats.certified_partial_reeliminations = bounded_add(
            "conditional certified partial re-eliminations",
            stats.certified_partial_reeliminations,
            1,
            limits.max_total_certified_partial_reeliminations,
        )?;
        stats.conditional_transcript_bytes = bounded_add(
            "conditional retained transcript bytes",
            stats.conditional_transcript_bytes,
            certificate.stats().transcript_bytes(),
            limits.max_total_conditional_transcript_bytes,
        )?;
        stats.conditional_pivots = bounded_add(
            "conditional pivot loci",
            stats.conditional_pivots,
            certificate.centered_pivot_loci().len(),
            limits.max_total_conditional_pivots,
        )?;
        let mut installed = 0usize;
        let mut skipped = 0usize;
        for locus in certificate.centered_pivot_loci() {
            let contradictory = locus.centered_assignment().entries().iter().try_fold(
                false,
                |found, &(position, value)| {
                    Ok::<_, SectorFoundationError>(
                        found || queue.sector().is_active(position)? != (value >= 1),
                    )
                },
            )?;
            if contradictory {
                skipped = checked_add("skipped contradictory conditional loci", skipped, 1)?;
            } else {
                installed = checked_add("installed conditional rules", installed, 1)?;
            }
        }
        stats.installed_rules = bounded_add(
            "installed conditional rules",
            stats.installed_rules,
            installed,
            limits.max_total_installed_rules,
        )?;
        stats.skipped_contradictory_loci = bounded_add(
            "skipped contradictory conditional loci",
            stats.skipped_contradictory_loci,
            skipped,
            limits.max_total_skipped_contradictory_loci,
        )?;
    }
    Ok(stats)
}

fn compile_queue_rules<ProviderError>(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    queue: &GeneratedSectorLiveLeafQueueCertificate,
    limits: GeneratedSectorConditionalRuleProviderLimits,
    mut stats: GeneratedSectorConditionalRuleProviderBuildStats,
    replay_partial_certificates: bool,
) -> Result<
    (
        Vec<InstalledConditionalRule>,
        Vec<GeneratedSectorSkippedConditionalLocus>,
        GeneratedSectorConditionalRuleProviderBuildStats,
    ),
    GeneratedSectorConditionalRuleProviderError<ProviderError>,
>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    stats.root_leaves = bounded_add(
        "conditional root leaves",
        stats.root_leaves,
        queue.stats().global_leaves(),
        limits.max_total_root_leaves,
    )?;
    stats.work_items = bounded_add(
        "conditional queue work items",
        stats.work_items,
        queue.work_items().len(),
        limits.max_total_work_items,
    )?;
    stats.conditional_transcript_bytes = bounded_add(
        "conditional queue transcript bytes",
        stats.conditional_transcript_bytes,
        queue.stats().conditional_transcript_bytes(),
        limits.max_total_conditional_transcript_bytes,
    )?;

    let mut rules = Vec::new();
    let mut skipped = Vec::new();
    for item in queue.work_items() {
        let GeneratedSectorLiveLeafOutcome::PartialReelimination { compilation, .. } =
            item.outcome()
        else {
            continue;
        };
        let GeneratedPartialReeliminationCompilation::Certified(certificate) = compilation else {
            stats.empty_partial_systems = bounded_add(
                "conditional empty partial systems",
                stats.empty_partial_systems,
                1,
                limits.max_total_empty_partial_systems,
            )?;
            continue;
        };
        stats.certified_partial_reeliminations = bounded_add(
            "conditional certified partial re-eliminations",
            stats.certified_partial_reeliminations,
            1,
            limits.max_total_certified_partial_reeliminations,
        )?;
        // `ConditionalParametricRule` owns an `Arc` proof while the consumed
        // queue remains retained for root routing and replay. Account for the
        // one deep certificate copy before cloning it; all pivots from this
        // certificate then share that same `Arc`.
        stats.conditional_transcript_bytes = bounded_add(
            "conditional retained transcript bytes",
            stats.conditional_transcript_bytes,
            certificate.stats().transcript_bytes(),
            limits.max_total_conditional_transcript_bytes,
        )?;
        let certificate = Arc::new(certificate.clone());
        for pivot_ordinal in 0..certificate.centered_pivot_loci().len() {
            stats.conditional_pivots = bounded_add(
                "conditional pivot loci",
                stats.conditional_pivots,
                1,
                limits.max_total_conditional_pivots,
            )?;
            let provenance = GeneratedSectorConditionalRuleProvenance {
                work_item_ordinal: item.ordinal(),
                source_case: item.source_case(),
                pivot_ordinal,
            };
            let compiled = if replay_partial_certificates {
                ConditionalParametricRule::try_from_certificate_pivot(
                    family,
                    context,
                    certificate.clone(),
                    pivot_ordinal,
                    queue.sector().clone(),
                    limits.conditional_rule,
                )
            } else {
                ConditionalParametricRule::try_from_replayed_certificate_pivot(
                    family,
                    context,
                    certificate.clone(),
                    pivot_ordinal,
                    queue.sector().clone(),
                    limits.conditional_rule,
                )
            };
            match compiled {
                Ok(rule) => {
                    stats.installed_rules = bounded_add(
                        "installed conditional rules",
                        stats.installed_rules,
                        1,
                        limits.max_total_installed_rules,
                    )?;
                    rules.push(InstalledConditionalRule {
                        provenance,
                        source_certificate: certificate.clone(),
                        rule,
                    });
                }
                Err(ConditionalParametricRuleError::EmptyConditionalSectorLocus {
                    position,
                    value,
                    active,
                }) => {
                    stats.skipped_contradictory_loci = bounded_add(
                        "skipped contradictory conditional loci",
                        stats.skipped_contradictory_loci,
                        1,
                        limits.max_total_skipped_contradictory_loci,
                    )?;
                    skipped.push(GeneratedSectorSkippedConditionalLocus {
                        provenance,
                        position,
                        value,
                        active,
                    });
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok((rules, skipped, stats))
}

fn validate_family_context<ProviderError>(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
) -> Result<(), GeneratedSectorConditionalRuleProviderError<ProviderError>>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    if !family
        .coefficient_context()
        .has_same_variable_map(context.base())
    {
        return Err(GeneratedSectorConditionalRuleProviderError::WrongContext);
    }
    if family.denominator_count() != context.index_count() {
        return Err(GeneratedSectorConditionalRuleProviderError::WrongArity {
            expected: family.denominator_count(),
            actual: context.index_count(),
        });
    }
    Ok(())
}

fn validate_queue_scope<ProviderError>(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    queue: &GeneratedSectorLiveLeafQueueCertificate,
) -> Result<(), GeneratedSectorConditionalRuleProviderError<ProviderError>>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    if queue.family_fingerprint() != family.fingerprint() {
        return Err(GeneratedSectorConditionalRuleProviderError::WrongFamily);
    }
    if queue.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedSectorConditionalRuleProviderError::WrongContext);
    }
    if queue.sector().arity() != context.index_count() {
        return Err(GeneratedSectorConditionalRuleProviderError::WrongArity {
            expected: context.index_count(),
            actual: queue.sector().arity(),
        });
    }
    Ok(())
}

fn checked_add<ProviderError>(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedSectorConditionalRuleProviderError<ProviderError>>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    left.checked_add(right)
        .ok_or(GeneratedSectorConditionalRuleProviderError::ResourceCountOverflow { resource })
}

fn bounded_add<ProviderError>(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedSectorConditionalRuleProviderError<ProviderError>>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit<ProviderError>(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedSectorConditionalRuleProviderError<ProviderError>>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    if requested <= limit {
        Ok(())
    } else {
        Err(GeneratedSectorConditionalRuleProviderError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}

#[derive(Debug)]
pub enum GeneratedSectorConditionalRuleProviderError<ProviderError>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    WrongFamily,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    InnerProviderArityChanged {
        expected: usize,
        actual: usize,
    },
    DuplicateSector {
        sector: SectorMask,
    },
    ReplayedRowSpanAllocationMismatch {
        sector: SectorMask,
    },
    CoveragePointMissing {
        sector: SectorMask,
    },
    ProvedEmptyLocusMatched {
        sector: SectorMask,
        reason: crate::ParametricSectorEmptyLocusReason,
    },
    ReplayMismatch {
        detail: &'static str,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Queue(GeneratedSectorLiveLeafQueueError),
    Conditional(ConditionalParametricRuleError),
    Coverage(ParametricSectorCoverageError),
    Sector(SectorFoundationError),
    Inner(ProviderError),
}

impl<ProviderError> fmt::Display for GeneratedSectorConditionalRuleProviderError<ProviderError>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFamily => formatter.write_str("conditional provider family mismatch"),
            Self::WrongContext => formatter.write_str("conditional provider context mismatch"),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "conditional provider arity is {actual}, expected {expected}"
            ),
            Self::InnerProviderArityChanged { expected, actual } => write!(
                formatter,
                "conditional provider inner arity is {actual}, expected {expected}"
            ),
            Self::DuplicateSector { sector } => {
                write!(formatter, "duplicate conditional queue for sector {sector}")
            }
            Self::ReplayedRowSpanAllocationMismatch { sector } => write!(
                formatter,
                "already-replayed conditional queue for {sector} does not retain the family-shared row-span allocation"
            ),
            Self::CoveragePointMissing { sector } => write!(
                formatter,
                "conditional queue root coverage for {sector} omitted its own integer point"
            ),
            Self::ProvedEmptyLocusMatched { sector, reason } => write!(
                formatter,
                "conditional provider query in sector {sector} matched a structurally proved-empty locus: {reason:?}"
            ),
            Self::ReplayMismatch { detail } => {
                write!(formatter, "conditional provider replay mismatch: {detail}")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "conditional provider {resource} count overflowed usize"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "conditional provider {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::Queue(error) => error.fmt(formatter),
            Self::Conditional(error) => error.fmt(formatter),
            Self::Coverage(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
            Self::Inner(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl<ProviderError> std::error::Error for GeneratedSectorConditionalRuleProviderError<ProviderError> where
    ProviderError: std::error::Error + Send + Sync + 'static
{
}

impl<ProviderError> From<GeneratedSectorLiveLeafQueueError>
    for GeneratedSectorConditionalRuleProviderError<ProviderError>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    fn from(value: GeneratedSectorLiveLeafQueueError) -> Self {
        Self::Queue(value)
    }
}

impl<ProviderError> From<ConditionalParametricRuleError>
    for GeneratedSectorConditionalRuleProviderError<ProviderError>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    fn from(value: ConditionalParametricRuleError) -> Self {
        Self::Conditional(value)
    }
}

impl<ProviderError> From<ParametricSectorCoverageError>
    for GeneratedSectorConditionalRuleProviderError<ProviderError>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    fn from(value: ParametricSectorCoverageError) -> Self {
        Self::Coverage(value)
    }
}

impl<ProviderError> From<SectorFoundationError>
    for GeneratedSectorConditionalRuleProviderError<ProviderError>
where
    ProviderError: std::error::Error + Send + Sync + 'static,
{
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

#[cfg(test)]
mod replayed_queue_tests {
    use super::*;
    use crate::{
        AffineDenominator, GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
        GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueLimits,
        GeneratedSymbolicRowSpanCompiler, IntegralOrderingPolicy, ParametricIbpGenerator,
        ParametricSectorRuleProvider, ParametricSectorRuleProviderLimits,
        algebra::CoefficientContext,
    };

    fn family() -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        IntegralFamily::new(
            "replayed-conditional-row-span-binding",
            vec!["k".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                coefficients.parse("-m2").unwrap(),
                vec![coefficients.one()],
            )],
            Vec::new(),
            vec![coefficients.zero()],
        )
        .unwrap()
    }

    #[test]
    fn already_replayed_queue_rejects_an_equal_fresh_row_span_allocation() {
        let family = family();
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let discovery_limits = GeneratedSectorDiscoveryLimits::default();
        let sector = SectorMask::try_new([true]).unwrap();
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            sector.clone(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let queue = GeneratedSectorLiveLeafQueueCompiler::compile(
            &family,
            &context,
            &discovery,
            GeneratedSectorLiveLeafQueueLimits::default(),
        )
        .unwrap();
        let equal_fresh = Arc::new(
            GeneratedSymbolicRowSpanCompiler::compile(
                &family,
                &context,
                discovery_limits.coverage.generated_when_bad.ibp,
                discovery_limits.coverage.generated_when_bad.row_span,
            )
            .unwrap(),
        );
        let inner = ParametricSectorRuleProvider::try_new(
            &family,
            &context,
            [],
            ParametricSectorRuleProviderLimits::default(),
        )
        .unwrap();

        assert!(matches!(
            GeneratedSectorConditionalRuleProvider::try_new_with_replayed_queues(
                &family,
                &context,
                [queue],
                inner,
                &equal_fresh,
                GeneratedSectorConditionalRuleProviderLimits::default(),
            ),
            Err(GeneratedSectorConditionalRuleProviderError::ReplayedRowSpanAllocationMismatch {
                sector: actual,
            }) if actual == sector
        ));
    }
}
