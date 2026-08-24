//! Bounded adaptive discovery of concrete reductions from generated rows.
//!
//! This is the first RustRed analogue of LiteRed's `SolvejSector` search loop.
//! It does not contain recurrence formulae.  For each requested integral it
//! deterministically scouts nearby integer anchors, rebuilds exact sparse
//! elimination over the generated `K(n)` rows, compiles every replayed pivot
//! as a sector rule candidate, and accepts only a candidate whose concrete
//! guard/leak/descent predicate succeeds at that integral.
//!
//! Exhausting the configured search yields an explicit `Uncovered` terminal,
//! never a selected or certified master.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::reduction_engine::{ConcreteRuleDecision, ConcreteRuleProvider, ConcreteTerminalStatus};
use crate::{
    ConcreteIntegralKey, IntegralOrderingPolicy, ParametricCoefficientContext,
    ParametricElimination, ParametricEliminationError, ParametricEliminationLimits,
    ParametricEliminationOrdering, ParametricReductionRuleCandidate, ParametricRelation,
    ParametricRelationError, ParametricRowId, ParametricRuleApplication, ParametricRuleDerivation,
    ParametricRuleError, ParametricRuleLimits, ParametricRuleUndecidability, SectorFoundationError,
    SectorMask,
};

pub const ADAPTIVE_PARAMETRIC_RULE_SEARCH_V1_SCHEMA: &str =
    "rustred-adaptive-parametric-rule-search-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdaptiveRuleSearchLimits {
    pub elimination: ParametricEliminationLimits,
    pub rule: ParametricRuleLimits,
    pub max_search_depth: usize,
    /// All exact-diamond offsets considered before the sector filter. This is
    /// distinct from `max_scout_points_per_integral`, which counts only
    /// accepted points in the requested sector.
    pub max_enumerated_offsets_per_integral: usize,
    /// Heap-iterator transitions, including rejected partial assignments.
    /// This bounds work independently of the number of emitted shell points.
    pub max_offset_enumeration_steps_per_layer: usize,
    /// Aggregate scalar components retained by exact-diamond offsets for one
    /// requested integral.  This independently bounds `arity * offsets` and
    /// is checked before allocating even the first arity-sized vector.
    pub max_offset_components_per_integral: usize,
    pub max_scout_points_per_integral: usize,
    pub max_pivot_candidates_per_integral: usize,
    pub max_cached_decisions: usize,
}

impl Default for AdaptiveRuleSearchLimits {
    fn default() -> Self {
        Self {
            elimination: ParametricEliminationLimits::default(),
            rule: ParametricRuleLimits::default(),
            max_search_depth: 2,
            max_enumerated_offsets_per_integral: 1_000_000,
            max_offset_enumeration_steps_per_layer: 100_000_000,
            max_offset_components_per_integral: 256_000_000,
            max_scout_points_per_integral: 100_000,
            max_pivot_candidates_per_integral: 1_000_000,
            max_cached_decisions: 10_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AdaptiveRuleSearchStats {
    requests: usize,
    cache_hits: usize,
    enumerated_offsets: usize,
    scout_points: usize,
    eliminations: usize,
    pivot_candidates: usize,
    applicable_candidates: usize,
    inapplicable_candidates: usize,
    uncovered_requests: usize,
}

impl AdaptiveRuleSearchStats {
    pub const fn requests(self) -> usize {
        self.requests
    }
    pub const fn cache_hits(self) -> usize {
        self.cache_hits
    }
    pub const fn enumerated_offsets(self) -> usize {
        self.enumerated_offsets
    }
    pub const fn scout_points(self) -> usize {
        self.scout_points
    }
    pub const fn eliminations(self) -> usize {
        self.eliminations
    }
    pub const fn pivot_candidates(self) -> usize {
        self.pivot_candidates
    }
    pub const fn applicable_candidates(self) -> usize {
        self.applicable_candidates
    }
    pub const fn inapplicable_candidates(self) -> usize {
        self.inapplicable_candidates
    }
    pub const fn uncovered_requests(self) -> usize {
        self.uncovered_requests
    }
}

pub struct AdaptiveParametricRuleProvider<'relations> {
    context: &'relations ParametricCoefficientContext,
    source_rows: &'relations [ParametricRelation],
    ordering: IntegralOrderingPolicy,
    limits: AdaptiveRuleSearchLimits,
    cache: BTreeMap<ConcreteIntegralKey, ConcreteRuleDecision>,
    stats: AdaptiveRuleSearchStats,
}

impl<'relations> AdaptiveParametricRuleProvider<'relations> {
    pub const SCHEMA: &'static str = ADAPTIVE_PARAMETRIC_RULE_SEARCH_V1_SCHEMA;

    pub fn try_new(
        context: &'relations ParametricCoefficientContext,
        source_rows: &'relations [ParametricRelation],
        ordering: IntegralOrderingPolicy,
        limits: AdaptiveRuleSearchLimits,
    ) -> Result<Self, AdaptiveRuleSearchError> {
        if source_rows.is_empty() {
            return Err(AdaptiveRuleSearchError::EmptySourceRows);
        }
        for (row, relation) in source_rows.iter().enumerate() {
            if relation.context_fingerprint() != context.fingerprint() {
                return Err(AdaptiveRuleSearchError::WrongContext { row });
            }
            if relation.arity() != context.index_count() {
                return Err(AdaptiveRuleSearchError::WrongArity {
                    expected: context.index_count(),
                    actual: relation.arity(),
                });
            }
            if relation.family_fingerprint() != source_rows[0].family_fingerprint() {
                return Err(AdaptiveRuleSearchError::WrongFamily { row });
            }
        }
        Ok(Self {
            context,
            source_rows,
            ordering,
            limits,
            cache: BTreeMap::new(),
            stats: AdaptiveRuleSearchStats::default(),
        })
    }

    pub const fn context(&self) -> &ParametricCoefficientContext {
        self.context
    }

    pub fn source_rows(&self) -> &[ParametricRelation] {
        self.source_rows
    }

    pub const fn ordering(&self) -> IntegralOrderingPolicy {
        self.ordering
    }

    pub const fn limits(&self) -> AdaptiveRuleSearchLimits {
        self.limits
    }

    pub const fn stats(&self) -> AdaptiveRuleSearchStats {
        self.stats
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Deterministic LiteRed `preparepoints` layers in the requested sector.
    /// The points are concrete assignments for the original generated rows;
    /// callers that quotient before elimination must specialize at these
    /// assignments rather than first eliminating translated `K(n)` rows.
    pub fn scout_point_layers(
        &self,
        integral: &ConcreteIntegralKey,
    ) -> Result<Vec<Vec<Vec<i64>>>, AdaptiveRuleSearchError> {
        if integral.powers().len() != self.context.index_count() {
            return Err(AdaptiveRuleSearchError::WrongArity {
                expected: self.context.index_count(),
                actual: integral.powers().len(),
            });
        }
        let sector = SectorMask::try_from_indices(integral.powers())?;
        let mut enumerated = 0usize;
        let mut scouts = 0usize;
        let mut layers = Vec::new();
        for depth in 0..=self.limits.max_search_depth {
            let remaining = self
                .limits
                .max_enumerated_offsets_per_integral
                .checked_sub(enumerated)
                .ok_or(AdaptiveRuleSearchError::ResourceLimit {
                    resource: "enumerated search offsets per integral",
                    requested: enumerated,
                    limit: self.limits.max_enumerated_offsets_per_integral,
                })?;
            let offsets = exact_diamond_offsets(
                integral.powers().len(),
                depth,
                remaining,
                self.limits.max_offset_enumeration_steps_per_layer,
                remaining_offset_components(
                    integral.powers().len(),
                    enumerated,
                    self.limits.max_offset_components_per_integral,
                )?,
            )?;
            enumerated = checked_add(enumerated, offsets.len(), "enumerated search offsets")?;
            check_limit(
                "enumerated search offsets per integral",
                enumerated,
                self.limits.max_enumerated_offsets_per_integral,
            )?;
            let mut accepted = Vec::new();
            for offset in offsets {
                let point = integral
                    .powers()
                    .iter()
                    .zip(&offset)
                    .enumerate()
                    .map(|(position, (&power, &delta))| {
                        power
                            .checked_add(delta)
                            .ok_or(AdaptiveRuleSearchError::IndexOverflow { position })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if sector.contains_indices(&point)? {
                    accepted.push((self.ordering.complexity_key(&point)?, point));
                }
            }
            accepted.sort();
            accepted.dedup_by(|left, right| left.1 == right.1);
            scouts = checked_add(scouts, accepted.len(), "scout points per integral")?;
            check_limit(
                "scout points per integral",
                scouts,
                self.limits.max_scout_points_per_integral,
            )?;
            layers.push(accepted.into_iter().map(|(_, point)| point).collect());
        }
        Ok(layers)
    }

    /// Discover retained pivot candidates from every cumulative stencil
    /// depth through the configured maximum. Unlike
    /// [`ConcreteRuleProvider::decision_for`], this does not pre-emptively
    /// reject candidates before a caller can quotient their specialized RHS
    /// by proved zero sectors and verified symmetries. Candidates from a
    /// shallower system precede those from deeper systems.
    pub fn candidates_for_quotient(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<Vec<ParametricReductionRuleCandidate>, AdaptiveRuleSearchError> {
        self.candidates_for_quotient_through_depth(integral, self.limits.max_search_depth)
    }

    pub fn candidates_for_quotient_through_depth(
        &mut self,
        integral: &ConcreteIntegralKey,
        maximum_depth: usize,
    ) -> Result<Vec<ParametricReductionRuleCandidate>, AdaptiveRuleSearchError> {
        Ok(self
            .candidate_layers_for_quotient_through_depth(integral, maximum_depth)?
            .into_iter()
            .flatten()
            .collect())
    }

    /// Preserve cumulative-stencil depth boundaries so a quotient-aware
    /// caller can test all pivots at one depth and continue only when every
    /// one is inapplicable after its certified quotient.
    pub fn candidate_layers_for_quotient(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<Vec<Vec<ParametricReductionRuleCandidate>>, AdaptiveRuleSearchError> {
        self.candidate_layers_for_quotient_through_depth(integral, self.limits.max_search_depth)
    }

    pub fn candidate_layers_for_quotient_through_depth(
        &mut self,
        integral: &ConcreteIntegralKey,
        maximum_depth: usize,
    ) -> Result<Vec<Vec<ParametricReductionRuleCandidate>>, AdaptiveRuleSearchError> {
        if integral.powers().len() != self.context.index_count() {
            return Err(AdaptiveRuleSearchError::WrongArity {
                expected: self.context.index_count(),
                actual: integral.powers().len(),
            });
        }
        let sector = SectorMask::try_from_indices(integral.powers())?;
        let mut cumulative_rows = Vec::new();
        let mut translated_offsets = BTreeSet::new();
        let mut enumerated = 0usize;
        let mut scouts = 0usize;
        let mut candidate_layers = Vec::new();
        let mut candidate_count = 0usize;
        for depth in 0..=maximum_depth.min(self.limits.max_search_depth) {
            let remaining = self
                .limits
                .max_enumerated_offsets_per_integral
                .checked_sub(enumerated)
                .ok_or(AdaptiveRuleSearchError::ResourceLimit {
                    resource: "enumerated search offsets per integral",
                    requested: enumerated,
                    limit: self.limits.max_enumerated_offsets_per_integral,
                })?;
            let offsets = exact_diamond_offsets(
                integral.powers().len(),
                depth,
                remaining,
                self.limits.max_offset_enumeration_steps_per_layer,
                remaining_offset_components(
                    integral.powers().len(),
                    enumerated,
                    self.limits.max_offset_components_per_integral,
                )?,
            )?;
            enumerated = checked_add(enumerated, offsets.len(), "enumerated search offsets")?;
            let mut accepted = Vec::new();
            for offset in offsets {
                let scout = integral
                    .powers()
                    .iter()
                    .zip(&offset)
                    .enumerate()
                    .map(|(position, (&power, &delta))| {
                        power
                            .checked_add(delta)
                            .ok_or(AdaptiveRuleSearchError::IndexOverflow { position })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if sector.contains_indices(&scout)? {
                    accepted.push((self.ordering.complexity_key(&scout)?, scout, offset));
                }
            }
            accepted.sort();
            for (_, _, offset) in accepted {
                if !translated_offsets.insert(offset.clone()) {
                    continue;
                }
                scouts = checked_add(scouts, 1, "scout points per integral")?;
                check_limit(
                    "scout points per integral",
                    scouts,
                    self.limits.max_scout_points_per_integral,
                )?;
                let requested_rows = checked_add(
                    cumulative_rows.len(),
                    self.source_rows.len(),
                    "cumulative translated source rows",
                )?;
                if requested_rows > self.limits.elimination.max_source_rows {
                    return Err(ParametricEliminationError::ResourceLimit {
                        resource: "source rows",
                        requested: requested_rows,
                        limit: self.limits.elimination.max_source_rows,
                    }
                    .into());
                }
                if depth == 0 {
                    cumulative_rows.extend(self.source_rows.iter().cloned());
                } else {
                    let translation = crate::IndexShift::try_new(
                        offset.iter().copied(),
                        self.context.index_count(),
                    )?;
                    let label = offset
                        .iter()
                        .map(i64::to_string)
                        .collect::<Vec<_>>()
                        .join(",");
                    for (source, row) in self.source_rows.iter().enumerate() {
                        cumulative_rows.push(row.translated(
                            self.context,
                            &translation,
                            ParametricRowId::Derived {
                                label: format!(
                                    "adaptive-stencil|depth={depth}|offset=[{label}]|source={source}"
                                )
                                .into(),
                            },
                            self.limits.elimination.arithmetic,
                        )?);
                    }
                }
            }
            if cumulative_rows.is_empty() {
                continue;
            }
            let elimination = ParametricElimination::build(
                self.context,
                &cumulative_rows,
                ParametricEliminationOrdering::try_new(
                    self.ordering,
                    integral.powers().iter().copied(),
                )?,
                self.limits.elimination,
            )?;
            let requested_candidate_count = checked_add(
                candidate_count,
                elimination.pivots().len(),
                "pivot candidates per integral",
            )?;
            check_limit(
                "pivot candidates per integral",
                requested_candidate_count,
                self.limits.max_pivot_candidates_per_integral,
            )?;
            let derivation = ParametricRuleDerivation::try_new(
                self.context,
                &cumulative_rows,
                &elimination,
                self.limits.rule,
            )?;
            let mut candidates = Vec::with_capacity(elimination.pivots().len());
            for pivot in 0..elimination.pivots().len() {
                candidates.push(ParametricReductionRuleCandidate::try_from_derivation_pivot(
                    self.context,
                    &derivation,
                    pivot,
                    sector.clone(),
                    self.limits.rule,
                )?);
            }
            candidate_count = requested_candidate_count;
            candidate_layers.push(candidates);
        }
        Ok(candidate_layers)
    }

    fn discover(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, AdaptiveRuleSearchError> {
        if integral.powers().len() != self.context.index_count() {
            return Err(AdaptiveRuleSearchError::WrongArity {
                expected: self.context.index_count(),
                actual: integral.powers().len(),
            });
        }
        let sector = SectorMask::try_from_indices(integral.powers())?;
        let _depth_count = self.limits.max_search_depth.checked_add(1).ok_or(
            AdaptiveRuleSearchError::ResourceCountOverflow {
                resource: "search depth layers",
            },
        )?;
        let mut local_scouts = 0usize;
        let mut local_enumerated_offsets = 0usize;
        let mut local_candidates = 0usize;
        let mut cumulative_rows = Vec::new();
        let mut translated_offsets = BTreeSet::new();

        for depth in 0..=self.limits.max_search_depth {
            let remaining_offset_budget = self
                .limits
                .max_enumerated_offsets_per_integral
                .checked_sub(local_enumerated_offsets)
                .ok_or(AdaptiveRuleSearchError::ResourceLimit {
                    resource: "enumerated search offsets per integral",
                    requested: local_enumerated_offsets,
                    limit: self.limits.max_enumerated_offsets_per_integral,
                })?;
            let offsets_at_depth = exact_diamond_offsets(
                integral.powers().len(),
                depth,
                remaining_offset_budget,
                self.limits.max_offset_enumeration_steps_per_layer,
                remaining_offset_components(
                    integral.powers().len(),
                    local_enumerated_offsets,
                    self.limits.max_offset_components_per_integral,
                )?,
            )?;
            local_enumerated_offsets = checked_add(
                local_enumerated_offsets,
                offsets_at_depth.len(),
                "enumerated search offsets per integral",
            )?;
            check_limit(
                "enumerated search offsets per integral",
                local_enumerated_offsets,
                self.limits.max_enumerated_offsets_per_integral,
            )?;
            self.stats.enumerated_offsets = checked_add(
                self.stats.enumerated_offsets,
                offsets_at_depth.len(),
                "adaptive enumerated search offsets",
            )?;

            // LiteRed sorts the accepted `preparepoints` layer by its
            // integral ordering before equations are submitted. Preserve that
            // semantic ordering instead of inheriting generator DFS order.
            let mut accepted_offsets = Vec::new();
            for offset in offsets_at_depth {
                let scout = integral
                    .powers()
                    .iter()
                    .zip(&offset)
                    .enumerate()
                    .map(|(position, (&power, &delta))| {
                        power
                            .checked_add(delta)
                            .ok_or(AdaptiveRuleSearchError::IndexOverflow { position })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if sector.contains_indices(&scout)? {
                    let order = self.ordering.complexity_key(&scout)?;
                    accepted_offsets.push((order, scout, offset));
                }
            }
            accepted_offsets.sort_by(|left, right| {
                left.0
                    .cmp(&right.0)
                    .then_with(|| left.1.cmp(&right.1))
                    .then_with(|| left.2.cmp(&right.2))
            });

            let mut added_rows = false;
            for (_, _scout, offset) in accepted_offsets {
                // LiteRed's preparepoints stencil stays in the requested
                // sector. Out-of-sector translations cannot certify a rule
                // for this sector and are omitted from the cumulative system.
                if !translated_offsets.insert(offset.clone()) {
                    continue;
                }
                local_scouts = checked_add(local_scouts, 1, "scout points per integral")?;
                check_limit(
                    "scout points per integral",
                    local_scouts,
                    self.limits.max_scout_points_per_integral,
                )?;
                self.stats.scout_points =
                    checked_add(self.stats.scout_points, 1, "adaptive scout points")?;

                let requested_rows = checked_add(
                    cumulative_rows.len(),
                    self.source_rows.len(),
                    "cumulative translated source rows",
                )?;
                if requested_rows > self.limits.elimination.max_source_rows {
                    return Err(ParametricEliminationError::ResourceLimit {
                        resource: "source rows",
                        requested: requested_rows,
                        limit: self.limits.elimination.max_source_rows,
                    }
                    .into());
                }
                if depth == 0 {
                    // Depth zero is literally the generated input, preserving
                    // its original row identities and provenance.
                    cumulative_rows.extend(self.source_rows.iter().cloned());
                } else {
                    let translation = crate::IndexShift::try_new(
                        offset.iter().copied(),
                        self.context.index_count(),
                    )?;
                    let offset_label = offset
                        .iter()
                        .map(i64::to_string)
                        .collect::<Vec<_>>()
                        .join(",");
                    for (source_index, source) in self.source_rows.iter().enumerate() {
                        let row_id = ParametricRowId::Derived {
                            label: format!(
                                "adaptive-stencil|depth={depth}|offset=[{offset_label}]|source={source_index}"
                            )
                            .into(),
                        };
                        cumulative_rows.push(source.translated(
                            self.context,
                            &translation,
                            row_id,
                            self.limits.elimination.arithmetic,
                        )?);
                    }
                }
                added_rows = true;
            }
            if !added_rows {
                continue;
            }

            // The persisted order is scouted at the requested integral. The
            // cumulative relation stencil—not a changing anchor order—grows
            // at every search depth.
            let ordering = ParametricEliminationOrdering::try_new(
                self.ordering,
                integral.powers().iter().copied(),
            )?;
            let elimination = ParametricElimination::build(
                self.context,
                &cumulative_rows,
                ordering,
                self.limits.elimination,
            )?;
            self.stats.eliminations =
                checked_add(self.stats.eliminations, 1, "adaptive eliminations")?;
            // All candidates from this cumulative system share one immutable
            // retained source/elimination proof.  A returned reduction can
            // therefore replay its derivation after this local row buffer is
            // dropped, without cloning the full system once per pivot.
            let derivation = ParametricRuleDerivation::try_new(
                self.context,
                &cumulative_rows,
                &elimination,
                self.limits.rule,
            )?;

            for pivot_ordinal in 0..elimination.pivots().len() {
                local_candidates =
                    checked_add(local_candidates, 1, "pivot candidates per integral")?;
                check_limit(
                    "pivot candidates per integral",
                    local_candidates,
                    self.limits.max_pivot_candidates_per_integral,
                )?;
                self.stats.pivot_candidates =
                    checked_add(self.stats.pivot_candidates, 1, "adaptive pivot candidates")?;
                let candidate = ParametricReductionRuleCandidate::try_from_derivation_pivot(
                    self.context,
                    &derivation,
                    pivot_ordinal,
                    sector.clone(),
                    self.limits.rule,
                )?;
                match candidate.apply(self.context, integral.powers())? {
                    ParametricRuleApplication::Applicable(reduction) => {
                        self.stats.applicable_candidates = checked_add(
                            self.stats.applicable_candidates,
                            1,
                            "applicable adaptive candidates",
                        )?;
                        return Ok(ConcreteRuleDecision::Reduction(reduction));
                    }
                    ParametricRuleApplication::Inapplicable(_) => {
                        self.stats.inapplicable_candidates = checked_add(
                            self.stats.inapplicable_candidates,
                            1,
                            "inapplicable adaptive candidates",
                        )?;
                    }
                    ParametricRuleApplication::Undecidable(
                        ParametricRuleUndecidability::ConcreteIndicesRequired,
                    ) => return Err(AdaptiveRuleSearchError::UnexpectedUndecidableCandidate),
                }
            }
        }
        self.stats.uncovered_requests = checked_add(
            self.stats.uncovered_requests,
            1,
            "uncovered adaptive requests",
        )?;
        Ok(ConcreteRuleDecision::Terminal(
            ConcreteTerminalStatus::Uncovered,
        ))
    }
}

impl ConcreteRuleProvider for AdaptiveParametricRuleProvider<'_> {
    type Error = AdaptiveRuleSearchError;

    fn index_arity(&self) -> usize {
        self.context.index_count()
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        self.stats.requests = checked_add(self.stats.requests, 1, "adaptive rule requests")?;
        if let Some(cached) = self.cache.get(integral) {
            self.stats.cache_hits =
                checked_add(self.stats.cache_hits, 1, "adaptive rule cache hits")?;
            return Ok(cached.clone());
        }
        let decision = self.discover(integral)?;
        let requested = checked_add(self.cache.len(), 1, "adaptive cached decisions")?;
        check_limit(
            "cached decisions",
            requested,
            self.limits.max_cached_decisions,
        )?;
        self.cache.insert(integral.clone(), decision.clone());
        Ok(decision)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdaptiveRuleSearchError {
    EmptySourceRows,
    WrongContext {
        row: usize,
    },
    WrongFamily {
        row: usize,
    },
    WrongArity {
        expected: usize,
        actual: usize,
    },
    IndexOverflow {
        position: usize,
    },
    UnexpectedUndecidableCandidate,
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Elimination(ParametricEliminationError),
    Relation(ParametricRelationError),
    Rule(ParametricRuleError),
    Sector(SectorFoundationError),
}

impl fmt::Display for AdaptiveRuleSearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySourceRows => formatter.write_str("adaptive rule search needs source rows"),
            Self::WrongContext { row } => {
                write!(formatter, "adaptive source row {row} has a foreign context")
            }
            Self::WrongFamily { row } => {
                write!(formatter, "adaptive source row {row} has a foreign family")
            }
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "adaptive rule index arity is {actual}, expected {expected}"
            ),
            Self::IndexOverflow { position } => {
                write!(
                    formatter,
                    "adaptive scout index overflow at position {position}"
                )
            }
            Self::UnexpectedUndecidableCandidate => formatter.write_str(
                "a concrete adaptive rule application unexpectedly remained undecidable",
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "adaptive {resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "adaptive {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::Elimination(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Rule(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AdaptiveRuleSearchError {}

impl From<ParametricEliminationError> for AdaptiveRuleSearchError {
    fn from(value: ParametricEliminationError) -> Self {
        Self::Elimination(value)
    }
}

impl From<ParametricRelationError> for AdaptiveRuleSearchError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

impl From<ParametricRuleError> for AdaptiveRuleSearchError {
    fn from(value: ParametricRuleError) -> Self {
        Self::Rule(value)
    }
}

impl From<SectorFoundationError> for AdaptiveRuleSearchError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

fn exact_diamond_offsets(
    arity: usize,
    depth: usize,
    max_points: usize,
    max_steps: usize,
    max_components: usize,
) -> Result<Vec<Vec<i64>>, AdaptiveRuleSearchError> {
    if arity == 0 {
        return Err(AdaptiveRuleSearchError::WrongArity {
            expected: 1,
            actual: 0,
        });
    }
    let depth_i64 = i64::try_from(depth).map_err(|_| AdaptiveRuleSearchError::ResourceLimit {
        resource: "search depth",
        requested: depth,
        limit: i64::MAX as usize,
    })?;
    check_limit("enumerated search offset components", arity, max_components)?;
    let mut output = Vec::new();
    let mut current = vec![0i64; arity];
    // Heap-resident DFS frames avoid native recursion proportional to a
    // caller-controlled denominator count.
    #[derive(Clone, Copy)]
    struct Frame {
        position: usize,
        remaining: i64,
        next_value: i64,
    }
    let mut stack = Vec::new();
    stack.push(Frame {
        position: 0,
        remaining: depth_i64,
        next_value: -depth_i64,
    });
    let mut steps = 0usize;
    while let Some(frame) = stack.last().copied() {
        steps = checked_add(steps, 1, "search offset enumeration steps")?;
        check_limit(
            "search offset enumeration steps per layer",
            steps,
            max_steps,
        )?;
        if frame.position == arity {
            if frame.remaining == 0 {
                push_offset(&current, &mut output, max_points, max_components)?;
            }
            stack.pop();
            continue;
        }
        if frame.next_value > frame.remaining {
            stack.pop();
            continue;
        }
        let value = frame.next_value;
        let next_value = frame.next_value.checked_add(1).ok_or(
            AdaptiveRuleSearchError::ResourceCountOverflow {
                resource: "search offset enumeration",
            },
        )?;
        stack
            .last_mut()
            .expect("the copied frame is still present")
            .next_value = next_value;
        let remaining = frame.remaining - value.abs();
        current[frame.position] = value;
        stack.push(Frame {
            position: frame.position + 1,
            remaining,
            next_value: -remaining,
        });
    }
    Ok(output)
}

fn push_offset(
    current: &[i64],
    output: &mut Vec<Vec<i64>>,
    limit: usize,
    component_limit: usize,
) -> Result<(), AdaptiveRuleSearchError> {
    let requested = checked_add(output.len(), 1, "enumerated search offsets per layer")?;
    check_limit("enumerated search offsets per layer", requested, limit)?;
    let requested_components = requested.checked_mul(current.len()).ok_or(
        AdaptiveRuleSearchError::ResourceCountOverflow {
            resource: "enumerated search offset components",
        },
    )?;
    check_limit(
        "enumerated search offset components",
        requested_components,
        component_limit,
    )?;
    output.push(current.to_vec());
    Ok(())
}

fn remaining_offset_components(
    arity: usize,
    retained_offsets: usize,
    limit: usize,
) -> Result<usize, AdaptiveRuleSearchError> {
    let retained = retained_offsets.checked_mul(arity).ok_or(
        AdaptiveRuleSearchError::ResourceCountOverflow {
            resource: "enumerated search offset components",
        },
    )?;
    limit
        .checked_sub(retained)
        .ok_or(AdaptiveRuleSearchError::ResourceLimit {
            resource: "enumerated search offset components per integral",
            requested: retained,
            limit,
        })
}

fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, AdaptiveRuleSearchError> {
    left.checked_add(right)
        .ok_or(AdaptiveRuleSearchError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), AdaptiveRuleSearchError> {
    if requested > limit {
        Err(AdaptiveRuleSearchError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_diamond_uses_a_heap_stack_and_obeys_the_preallocation_limit() {
        let offsets = exact_diamond_offsets(10_000, 0, 1, 30_000, 10_000).unwrap();
        assert_eq!(offsets.len(), 1);
        assert_eq!(offsets[0].len(), 10_000);
        assert!(offsets[0].iter().all(|&value| value == 0));

        assert!(matches!(
            exact_diamond_offsets(2, 1, 3, 100, 8),
            Err(AdaptiveRuleSearchError::ResourceLimit {
                resource: "enumerated search offsets per layer",
                requested: 4,
                limit: 3,
            })
        ));

        assert!(matches!(
            exact_diamond_offsets(1, 1_000_000, 2, 10, 2),
            Err(AdaptiveRuleSearchError::ResourceLimit {
                resource: "search offset enumeration steps per layer",
                requested: 11,
                limit: 10,
            })
        ));

        assert!(matches!(
            exact_diamond_offsets(10_000, 0, 1, 30_000, 9_999),
            Err(AdaptiveRuleSearchError::ResourceLimit {
                resource: "enumerated search offset components",
                requested: 10_000,
                limit: 9_999,
            })
        ));

        assert!(matches!(
            exact_diamond_offsets(2, 1, 4, 100, 7),
            Err(AdaptiveRuleSearchError::ResourceLimit {
                resource: "enumerated search offset components",
                requested: 8,
                limit: 7,
            })
        ));
    }
}
