//! Cold diagnostics for promoting source-port output into existing artifacts.
//!
//! Reports independently check replay, guards, unbounded coverage and descent.
//! Only the consuming complete-program path may construct the existing
//! `ClosedArtifact`, after full original-source replay and supplied-domain
//! cell lowering. Reports alone remain non-authoritative. No codec lives here.

mod affine;
mod certificate;
mod geometry;
mod limits;
pub use limits::SourcePortLimits;
pub(crate) use limits::{DEFAULT_PREDICATE_ATOMS, DEFAULT_PREDICATE_CONSISTENCY_WORK};
mod normalization;
mod ordinary;
pub(in crate::foundry::artifact) mod predicate_cover;
mod program;
mod progress;
pub use progress::{
    SourcePortInstallEvent, SourcePortSuccessorAttempt, SourcePortSuccessorCounts,
    SourcePortSuccessorSnapshot, SourcePortSuccessorStage,
};
mod replay;
mod rule_replay;
pub use rule_replay::{SourcePortReplayedRule, SourcePortRuleReplayAudit};
mod total_excess;
pub use total_excess::SourcePortTotalExcessAudit;
pub(in crate::foundry::artifact) use total_excess::{EnvelopeBudget, visit_successor_degrees};
pub(in crate::foundry::artifact) mod scope;
pub(crate) use program::lower::ReplayedOriginalDomain;
#[cfg(test)]
pub(in crate::foundry::artifact) use program::lower::durable_tests::installed_k1 as installed_k1_for_codec_test;
pub(in crate::foundry::artifact) use program::lower::{PreparedOriginalDomain, ReplayLimits};
#[cfg(test)]
pub(in crate::foundry::artifact) use program::scoped_tests::candidate as source_port_candidate_for_test;
#[cfg(test)]
pub(in crate::foundry::artifact) use program::scoped_tests::generated_scope as generated_scoped_k3_for_codec_test;

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::family::IntegralFamily;
use crate::sector::{CoordinatePriority, CoordinatePriorityLimits, Mask, OrderingPolicy, zero};
use crate::solver::{SectorConfig, SectorSolution, SectorSolver, SourceSystem};

pub use affine::AffineApplicationDomain;

/// The role of an affine case requiring an exact ownership proof.
///
/// Retaining the exact equations makes incomplete source or ownership proofs
/// actionable rather than reducing the case to an opaque string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AffineOwnershipRole {
    Target,
    Exceptional,
}

/// Fail-closed cold-audit failure, with no partially installed state.
#[derive(Clone, Debug)]
pub enum SourcePortAuditError {
    /// Ordinary bridge/validation failure.
    Message(String),
    /// An exact proof exhausted a caller-owned resource allowance.
    ResourceBudgetExhausted { resource: &'static str },
    /// The caller requested a policy beyond the supported traversal bound.
    UnsupportedResourcePolicy {
        resource: &'static str,
        requested: usize,
        supported_max: usize,
    },
    /// Search output lacks an exact ownership proof in the calling path.
    /// In particular, box-only callers must not discard coupled predicates.
    ///
    /// `sector` is the canonical positive/negative orthant mask and
    /// `equations` are the canonical coupled equalities in the family index
    /// variable map. `fixed` retains the coordinate face constraints, which
    /// are independent of the sector signs.
    UnsupportedAffineOwnership {
        domain: AffineApplicationDomain,
        role: AffineOwnershipRole,
    },
}

impl fmt::Display for SourcePortAuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(message) => f.write_str(message),
            Self::ResourceBudgetExhausted { resource } => {
                write!(f, "source-port proof budget exhausted: {resource}")
            }
            Self::UnsupportedResourcePolicy {
                resource,
                requested,
                supported_max,
            } => {
                write!(
                    f,
                    "unsupported source-port resource policy: {resource} requested {requested}, supported maximum {supported_max}"
                )
            }
            Self::UnsupportedAffineOwnership { domain, role } => {
                let role = match role {
                    AffineOwnershipRole::Target => "target",
                    AffineOwnershipRole::Exceptional => "exceptional",
                };
                write!(
                    f,
                    "{role} affine ownership is not yet supported by the artifact bridge \
                     (sector={:?}, fixed={:?}, equations={:?})",
                    domain.sector(),
                    domain.fixed(),
                    domain.equations(),
                )
            }
        }
    }
}

impl std::error::Error for SourcePortAuditError {}

impl SourcePortAuditError {
    pub(crate) fn message(value: impl Into<String>) -> Self {
        Self::Message(value.into())
    }

    /// Add location to an otherwise opaque downstream diagnostic without
    /// erasing typed resource or exceptional-domain errors.
    fn with_message_context(self, context: impl FnOnce() -> String) -> Self {
        match self {
            Self::Message(detail) => Self::Message(format!("{}: {detail}", context())),
            typed => typed,
        }
    }

    /// Return the exact affine diagnostic when this error is one.  This is a
    /// read-only inspection seam for generic campaign/reporting code; it does
    /// not imply that the current artifact bridge can consume the case.
    pub fn affine_ownership(
        &self,
    ) -> Option<(
        &[bool],
        &[Option<i16>],
        &[crate::algebra::CoefficientPolynomial],
        AffineOwnershipRole,
    )> {
        match self {
            Self::UnsupportedAffineOwnership { domain, role } => {
                Some((domain.sector(), domain.fixed(), domain.equations(), *role))
            }
            Self::Message(_)
            | Self::ResourceBudgetExhausted { .. }
            | Self::UnsupportedResourcePolicy { .. } => None,
        }
    }
}

fn error(value: impl fmt::Display) -> SourcePortAuditError {
    SourcePortAuditError::message(value.to_string())
}

fn sector_ordering<const N: usize>(
    sector: [bool; N],
    permutation: Option<[usize; N]>,
) -> Result<OrderingPolicy, SourcePortAuditError> {
    let Some(slots) = permutation else {
        return Ok(OrderingPolicy::SpiredUncutV1);
    };
    crate::solver::IntegralOrder::new(sector, [false; N])
        .with_permutation(slots)
        .map_err(error)?;
    let mut ranks = [0; N];
    for (rank, slot) in slots.into_iter().enumerate() {
        ranks[slot] = rank;
    }
    let priority = CoordinatePriority::try_new(N, &ranks, CoordinatePriorityLimits::default())
        .map_err(error)?;
    OrderingPolicy::try_spired_with_coordinate_priority(&priority).map_err(error)
}

/// One non-authoritative exact rule/coverage report. The optional degree
/// explicitly narrows coverage, never the original-source identity checks.
#[derive(Debug)]
pub struct SourcePortSectorAudit<const N: usize> {
    pub sector: [bool; N],
    /// `None` means whole-orthant coverage. Otherwise only local total excess
    /// `sum(max(n_i-1,0)+max(-n_i,0)) <= D` is requested. This says nothing
    /// about RHS closure or entry admission and cannot authorize an artifact.
    pub max_total_excess_degree: Option<u64>,
    pub ordering: OrderingPolicy,
    pub rules: usize,
    pub exact_replayed_rules: usize,
    pub uniformly_descending_rules: usize,
    /// Affine candidate rules omitted only when the independently retained
    /// predicate-aware cells and terminals prove a complete exact cover. Such a
    /// rule is redundant for execution, not silently accepted as an
    /// unverified identity.
    pub redundant_affine_rules: usize,
    pub replay_source_entries: usize,
    pub additional_replay_guard_branches: usize,
    pub finite_terminals: usize,
    pub stored_guard_uncovered_boxes: usize,
    pub stored_guard_unbounded_boxes: usize,
    pub checked_rule_uncovered_boxes: usize,
    pub checked_rule_unbounded_boxes: usize,
    pub issues: Vec<String>,
    pub elapsed: Duration,
}

/// Sources and zero-sector certificates prepared once for a cold vacuum audit.
/// The search itself is neither rerun nor invoked from this owner.
pub struct SourcePortAudit<const N: usize> {
    sources: SourceSystem<N>,
    root_sector: Mask,
    // Globally valid zero evidence is also needed by translated source replay
    // outside the requested root downset. Never filter this list to the scope.
    zero_sectors: Arc<[[bool; N]]>,
    zero_certificates: Vec<zero::Certificate>,
    original_row_ids: Vec<crate::identity::RowId>,
    original_sources: normalization::OriginalSourceCorpus,
    limits: SourcePortLimits,
}

impl<const N: usize> SourcePortAudit<N> {
    /// Check family eligibility for complete artifact installation before
    /// constructing a sector census or running search. This shares the final
    /// installer's exact unit-mass vacuum admission, not a frontend copy.
    /// Success is only a family-shape check, not evidence of closure.
    pub fn validate_install_family(family: &IntegralFamily) -> Result<(), SourcePortAuditError> {
        super::install::validate_unit_mass_family(family).map_err(error)
    }

    /// The initial bridge admits uncut, unshifted vacuum sources only. It is
    /// generic in loop count, denominator count and momentum routing.
    pub fn try_new(
        family: &IntegralFamily,
        zero_sectors: Arc<[[bool; N]]>,
    ) -> Result<Self, SourcePortAuditError> {
        Self::try_new_with_root_sector(family, zero_sectors, [true; N])
    }

    /// Certify every subsector of a caller-declared maximal root sector.
    ///
    /// A `true` position permits positive powers; a `false` position requires
    /// nonpositive powers, including arbitrary numerators. The zero evidence
    /// remains global because translated original identities can use it beyond
    /// this root domain. Omitted nonzero sectors inside the domain are errors.
    pub fn try_new_with_root_sector(
        family: &IntegralFamily,
        zero_sectors: Arc<[[bool; N]]>,
        root_sector: [bool; N],
    ) -> Result<Self, SourcePortAuditError> {
        let root_sector = Mask::try_new(root_sector).map_err(error)?;
        scope::sector_count(&root_sector).map_err(error)?;
        if family.external_count() != 0 || family.power_shifts().iter().any(|x| !x.is_zero()) {
            return Err(error(
                "source-port artifact audit currently requires an unshifted vacuum family",
            ));
        }
        let sources = SourceSystem::from_family(family).map_err(error)?;
        // SourceSystem's vacuum adapter is differentiated-loop-major. Bind
        // that audited schedule to generator RowIds, never basis ordinals.
        let loops = family.loop_count();
        let expected_rows = loops
            .checked_mul(loops)
            .ok_or_else(|| error("ordinary source count overflow"))?;
        if sources.rows().len() != expected_rows {
            return Err(error(
                "vacuum source corpus has unexpected ordinary row count",
            ));
        }
        let original_row_ids: Vec<_> = (0..loops)
            .flat_map(|differentiated_loop| {
                (0..loops).map(
                    move |contraction_momentum| crate::identity::RowId::OrdinaryIbp {
                        contraction_momentum,
                        differentiated_loop,
                    },
                )
            })
            .collect();
        let original_sources =
            normalization::OriginalSourceCorpus::try_new(family, &sources, &original_row_ids)?;
        let analyzer = zero::Analyzer::try_unrestricted(family).map_err(error)?;
        let mut seen = BTreeSet::new();
        let mut zero_certificates = Vec::with_capacity(zero_sectors.len());
        for &sector in zero_sectors.iter() {
            if !seen.insert(sector) {
                return Err(error("duplicate claimed zero sector"));
            }
            match analyzer
                .analyze(&Mask::try_new(sector).map_err(error)?)
                .map_err(error)?
            {
                zero::Decision::ProvedZero(certificate) => zero_certificates.push(certificate),
                _ => {
                    return Err(error(format!(
                        "claimed zero sector is not proved zero: {sector:?}"
                    )));
                }
            }
        }
        Ok(Self {
            sources,
            root_sector,
            zero_sectors,
            zero_certificates,
            original_row_ids,
            original_sources,
            limits: SourcePortLimits::default(),
        })
    }

    /// Select explicit audit/publication resources without changing source
    /// definitions, proof requirements, or deterministic artifact identity.
    pub fn with_limits(mut self, limits: SourcePortLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn original_source_count(&self) -> usize {
        self.sources.rows().len()
    }

    pub fn proved_zero_sector_count(&self) -> usize {
        self.zero_certificates.len()
    }

    /// The family/source parameter locus is retained separately from integer
    /// guard coverage, exactly as in the existing artifact authority pipeline.
    pub fn inherited_source_conditions(&self) -> &[crate::algebra::CoefficientPolynomial] {
        self.sources.conditions()
    }

    pub fn audit_sector(
        &self,
        sector: [bool; N],
        permutation: Option<[usize; N]>,
        solution: &SectorSolution<N>,
    ) -> Result<SourcePortSectorAudit<N>, SourcePortAuditError> {
        Ok(self.check_sector(sector, permutation, solution)?.report)
    }

    /// Replay the same full identities, guards and descent as the ordinary
    /// audit, but request coverage only through a total-excess degree. This
    /// diagnostic does not certify the reachable family or authorize durable
    /// publication: descendants can leave the requested entry degree.
    pub fn audit_sector_through_total_excess(
        &self,
        sector: [bool; N],
        permutation: Option<[usize; N]>,
        solution: &SectorSolution<N>,
        max_total_excess_degree: u64,
    ) -> Result<SourcePortSectorAudit<N>, SourcePortAuditError> {
        Ok(self
            .check_sector_in_degree(
                sector,
                permutation,
                solution,
                Some(max_total_excess_degree),
                Instant::now(),
                &mut |_| {},
            )?
            .report)
    }

    fn check_sector(
        &self,
        sector: [bool; N],
        permutation: Option<[usize; N]>,
        solution: &SectorSolution<N>,
    ) -> Result<program::SectorCheck<N>, SourcePortAuditError> {
        self.check_sector_with_observer(sector, permutation, solution, Instant::now(), &mut |_| {})
    }

    fn check_sector_with_observer(
        &self,
        sector: [bool; N],
        permutation: Option<[usize; N]>,
        solution: &SectorSolution<N>,
        started: Instant,
        observe: &mut dyn FnMut(SourcePortInstallEvent<'_, N>),
    ) -> Result<program::SectorCheck<N>, SourcePortAuditError> {
        self.check_sector_in_degree(sector, permutation, solution, None, started, observe)
    }

    fn check_sector_in_degree(
        &self,
        sector: [bool; N],
        permutation: Option<[usize; N]>,
        solution: &SectorSolution<N>,
        max_total_excess_degree: Option<u64>,
        started: Instant,
        observe: &mut dyn FnMut(SourcePortInstallEvent<'_, N>),
    ) -> Result<program::SectorCheck<N>, SourcePortAuditError> {
        if solution.max_numerator_rank.is_some() {
            return Err(error(
                "numerator-rank-scoped candidate requires a compatible successor-scope proof; unrestricted and total-excess installation cannot consume it",
            ));
        }
        self.limits.validate()?;
        let start = Instant::now();
        if !Mask::try_new(sector)
            .map_err(error)?
            .is_subsector_of(&self.root_sector)
            .map_err(error)?
        {
            return Err(error(
                "solved sector is outside the declared root-sector scope",
            ));
        }
        if self.zero_sectors.contains(&sector) {
            return Err(error("a solved sector was also declared zero"));
        }
        let ordering = sector_ordering(sector, permutation)?;
        let config = SectorConfig {
            permutation,
            zero_sectors: self.zero_sectors.clone(),
            ..SectorConfig::default()
        };
        let (solver, preconditioner) =
            SectorSolver::new_with_provenance(&self.sources, sector, config).map_err(error)?;
        let mut order = crate::solver::IntegralOrder::new(sector, [false; N]);
        if let Some(slots) = permutation {
            order = order.with_permutation(slots).map_err(error)?;
        }
        let mut report = SourcePortSectorAudit {
            sector,
            max_total_excess_degree,
            ordering,
            rules: solution.rules.len(),
            exact_replayed_rules: 0,
            uniformly_descending_rules: 0,
            redundant_affine_rules: 0,
            replay_source_entries: 0,
            additional_replay_guard_branches: 0,
            finite_terminals: solution.finite_residuals.len(),
            stored_guard_uncovered_boxes: 0,
            stored_guard_unbounded_boxes: 0,
            checked_rule_uncovered_boxes: 0,
            checked_rule_unbounded_boxes: 0,
            issues: Vec::new(),
            elapsed: Duration::ZERO,
        };
        let mut stored_partitions = Vec::new();
        let mut retained_rules = Vec::new();
        let mut affine_candidates = Vec::new();
        for (ordinal, rule) in solution.rules.iter().enumerate() {
            observe(SourcePortInstallEvent::CheckingRule {
                sector,
                ordinal,
                total: solution.rules.len(),
                elapsed: started.elapsed(),
            });
            let affine_target = match rule.candidate.case.affine() {
                Some(case) => Some(Arc::new(
                    AffineApplicationDomain::from_case(case, &sector).map_err(error)?,
                )),
                None => None,
            };
            let stored = match geometry::application_partition(
                rule,
                self.sources.index_variables(),
                &sector,
                &[],
            ) {
                Ok(stored) => stored,
                Err(issue) => {
                    if matches!(
                        &issue,
                        SourcePortAuditError::UnsupportedAffineOwnership { .. }
                    ) {
                        affine_candidates.push((ordinal, issue));
                    } else {
                        report
                            .issues
                            .push(format!("rule {ordinal} stored guard geometry: {issue}"));
                    }
                    continue;
                }
            };
            // `application_partition` returns no boxes for an affine target
            // proved contradictory with this sector.  It is not a replayable
            // rule and must be omitted, rather than admitted with a vacuous
            // application domain.  Coordinate targets cannot reach this
            // branch because their case box is always nonempty.
            if stored.boxes.is_empty() && rule.candidate.case.affine().is_some() {
                // This affine domain was proved empty in the sector.  It is
                // a vacuous candidate, so account for its omission directly
                // rather than sending it through the unresolved-affine
                // candidate list (which is reserved for nonempty domains
                // that still need an independent cover).
                report.redundant_affine_rules += 1;
                continue;
            }
            stored_partitions.push((stored, affine_target.clone()));
            let stored = &stored_partitions.last().expect("just inserted partition").0;
            // A coupled candidate without a source trace is only a search
            // witness.  Keep it in the independent affine-candidate census
            // (so a complete independent cover may make it redundant), but
            // never send it through replay or treat its face prefilter as an
            // executable rule.
            if rule.candidate.sources.is_empty() {
                let predicate = affine_target
                    .clone()
                    .map(|domain| (domain, AffineOwnershipRole::Target))
                    .or_else(|| {
                        stored
                            .affine_exclusions
                            .first()
                            .cloned()
                            .map(|domain| (domain, AffineOwnershipRole::Exceptional))
                    });
                if let Some((domain, role)) = predicate {
                    affine_candidates.push((
                        ordinal,
                        SourcePortAuditError::UnsupportedAffineOwnership {
                            domain: (*domain).clone(),
                            role,
                        },
                    ));
                    continue;
                }
            }
            match replay::replay_rule(
                &self.sources,
                &self.original_row_ids,
                &self.original_sources,
                solver.basis(),
                &order,
                &self.zero_sectors,
                rule,
                &stored.boxes,
                Some(&preconditioner),
            ) {
                Ok(replay) => {
                    report.replay_source_entries += replay.ordinary.contributions.len();
                    report.additional_replay_guard_branches += replay.additional_exceptions.len();
                    let checked = match geometry::application_partition(
                        rule,
                        self.sources.index_variables(),
                        &sector,
                        &replay.additional_exceptions,
                    ) {
                        Ok(checked) => checked,
                        Err(issue) => {
                            if affine_target.is_some()
                                && matches!(
                                    &issue,
                                    SourcePortAuditError::UnsupportedAffineOwnership { .. }
                                )
                            {
                                affine_candidates.push((ordinal, issue));
                            } else {
                                report.issues.push(format!(
                                    "rule {ordinal} certificate guard geometry: {issue}"
                                ));
                            }
                            continue;
                        }
                    };
                    match geometry::prove_descent(
                        rule,
                        &checked.boxes,
                        &checked.affine_exclusions,
                        &sector,
                        ordering,
                        self.sources.index_variables(),
                    ) {
                        Ok(()) => {
                            // Individual replay/descent is not whole-sector
                            // ownership. Exact predicates remain attached for
                            // the independent cover proof below and cold load.
                            report.exact_replayed_rules += 1;
                            report.uniformly_descending_rules += 1;
                            let retained = program::CheckedRule::retain(
                                rule,
                                replay.ordinary,
                                checked.boxes,
                                affine_target.clone(),
                                checked.affine_exclusions,
                            )?;
                            retained_rules.push(retained);
                        }
                        Err(issue) => {
                            if affine_target.is_some() {
                                affine_candidates.push((ordinal, issue));
                            } else {
                                report
                                    .issues
                                    .push(format!("rule {ordinal} descent: {issue}"));
                            }
                        }
                    }
                }
                Err(issue) => {
                    if affine_target.is_some() {
                        affine_candidates.push((ordinal, issue));
                    } else {
                        report
                            .issues
                            .push(format!("rule {ordinal} replay: {issue}"));
                    }
                }
            }
        }
        let terminal_boxes = geometry::terminal_boxes(&solution.finite_residuals, &sector)?;
        let terminals = solution
            .finite_residuals
            .iter()
            .map(|terminal| std::array::from_fn(|axis| i64::from(terminal[axis].value())))
            .collect();
        use predicate_cover::{PredicateCoverLimits, PredicateCoveragePiece};
        let stored_owners: Vec<_> = stored_partitions
            .iter()
            .map(|(partition, target)| PredicateCoveragePiece {
                boxes: &partition.boxes,
                affine_target: target.as_deref(),
                affine_exclusions: &partition.affine_exclusions,
            })
            .collect();
        let checked_owners: Vec<_> = retained_rules
            .iter()
            .map(|rule| PredicateCoveragePiece {
                boxes: &rule.application,
                affine_target: rule.affine.as_deref(),
                affine_exclusions: &rule.affine_exclusions,
            })
            .collect();
        let coverage = |owners: &[PredicateCoveragePiece<'_>]| {
            let limits = PredicateCoverLimits {
                max_predicates: self.limits.max_predicate_atoms,
                max_consistency_work: self.limits.max_predicate_consistency_work,
                ..Default::default()
            };
            let checked = match max_total_excess_degree {
                Some(degree) => predicate_cover::certify_predicate_cover_up_to_degree(
                    &sector,
                    owners,
                    &terminal_boxes,
                    scope::EntryDegreeBound::MaxTotalExcessDegree(degree),
                    PredicateCoverLimits {
                        geometry: self.limits.cover_replay.geometry(),
                        ..limits
                    },
                ),
                None => predicate_cover::certify_predicate_cover(
                    &sector,
                    owners,
                    &terminal_boxes,
                    limits,
                ),
            };
            match checked {
                Ok(_) => Ok((0, 0, None)),
                Err(issue) => match &issue {
                    predicate_cover::PredicateCoverError::Uncovered {
                        boxes,
                        unbounded_boxes,
                        ..
                    } => Ok((*boxes, *unbounded_boxes, Some(issue.to_string()))),
                    predicate_cover::PredicateCoverError::Budget(resource) => {
                        Err(SourcePortAuditError::ResourceBudgetExhausted { resource })
                    }
                    _ => Err(error(issue)),
                },
            }
        };
        let (stored_boxes, stored_unbounded, stored_issue) = coverage(&stored_owners)?;
        let (checked_boxes, checked_unbounded, checked_issue) = coverage(&checked_owners)?;
        (
            report.stored_guard_uncovered_boxes,
            report.stored_guard_unbounded_boxes,
        ) = (stored_boxes, stored_unbounded);
        (
            report.checked_rule_uncovered_boxes,
            report.checked_rule_unbounded_boxes,
        ) = (checked_boxes, checked_unbounded);
        if let Some(issue) = stored_issue {
            report
                .issues
                .push(format!("stored guard predicate coverage: {issue}"));
        }
        if let Some(issue) = checked_issue {
            report
                .issues
                .push(format!("checked rule predicate coverage: {issue}"));
        }
        let affine_cover_complete = report.stored_guard_uncovered_boxes == 0
            && report.stored_guard_unbounded_boxes == 0
            && report.checked_rule_uncovered_boxes == 0
            && report.checked_rule_unbounded_boxes == 0;
        if affine_cover_complete {
            report.redundant_affine_rules += affine_candidates.len();
        } else if !affine_candidates.is_empty() {
            report.issues.push(format!(
                "{} affine candidate rules cannot be omitted because the retained exact predicate cover is incomplete",
                affine_candidates.len(),
            ));
            for (ordinal, issue) in affine_candidates {
                report.issues.push(format!(
                    "rule {ordinal} deferred source/geometry proof: {issue}"
                ));
            }
        }
        report.elapsed = start.elapsed();
        Ok(program::SectorCheck {
            report,
            rules: retained_rules,
            terminals,
        })
    }

    /// Check only the finite mask census, not the individual report claims.
    /// Arity beyond this diagnostic counter's capacity fails explicitly.
    pub fn validate_sector_census(
        &self,
        reports: &[SourcePortSectorAudit<N>],
    ) -> Result<(), SourcePortAuditError> {
        self.validate_sector_masks(reports.iter().map(|report| report.sector))
    }

    fn validate_sector_masks(
        &self,
        sectors: impl IntoIterator<Item = [bool; N]>,
    ) -> Result<(), SourcePortAuditError> {
        let expected = scope::sector_count(&self.root_sector).map_err(error)?;
        let in_scope = |sector: &[bool; N]| {
            sector
                .iter()
                .zip(self.root_sector.active_bits())
                .all(|(&active, &allowed)| !active || allowed)
        };
        let mut seen: BTreeSet<_> = self.zero_sectors.iter().copied().filter(in_scope).collect();
        for sector in sectors {
            if !in_scope(&sector) {
                return Err(error(
                    "sector census contains an out-of-scope solved sector",
                ));
            }
            if !seen.insert(sector) {
                return Err(error("duplicate or zero solved sector"));
            }
        }
        if seen.len() != expected {
            return Err(error(
                "sector census does not exhaust the declared root-sector scope",
            ));
        }
        Ok(())
    }
}
