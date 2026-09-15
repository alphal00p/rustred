//! Cold diagnostics for promoting source-port output into existing artifacts.
//!
//! Reports independently check replay, guards, unbounded coverage and descent.
//! Only the consuming complete-program path may construct the existing
//! `ClosedArtifact`, after full original-source replay and supplied-domain
//! cell lowering. Reports alone remain non-authoritative. No codec lives here.

mod certificate;
mod geometry;
mod normalization;
mod ordinary;
mod program;
mod replay;
pub(crate) use program::lower::ReplayedOriginalDomain;
#[cfg(test)]
pub(in crate::foundry::artifact) use program::lower::durable_tests::installed_k1 as installed_k1_for_codec_test;
pub(in crate::foundry::artifact) use program::lower::{PreparedOriginalDomain, ReplayLimits};

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::family::IntegralFamily;
use crate::sector::{CoordinatePriority, CoordinatePriorityLimits, Mask, OrderingPolicy, zero};
use crate::solver::{SectorConfig, SectorSolution, SectorSolver, SourceSystem};

/// Fail-closed cold-audit failure, with no partially installed state.
#[derive(Debug)]
pub struct SourcePortAuditError(pub String);

impl fmt::Display for SourcePortAuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SourcePortAuditError {}

fn error(value: impl fmt::Display) -> SourcePortAuditError {
    SourcePortAuditError(value.to_string())
}

/// One non-authoritative report over a whole mathematical integer orthant.
#[derive(Debug)]
pub struct SourcePortSectorAudit<const N: usize> {
    pub sector: [bool; N],
    pub ordering: OrderingPolicy,
    pub rules: usize,
    pub exact_replayed_rules: usize,
    pub uniformly_descending_rules: usize,
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
    zero_sectors: Arc<[[bool; N]]>,
    zero_certificates: Vec<zero::Certificate>,
    original_row_ids: Vec<crate::identity::RowId>,
    original_sources: normalization::OriginalSourceCorpus,
}

impl<const N: usize> SourcePortAudit<N> {
    /// The initial bridge admits uncut, unshifted vacuum sources only. It is
    /// generic in loop count, denominator count and momentum routing.
    pub fn try_new(
        family: &IntegralFamily,
        zero_sectors: Arc<[[bool; N]]>,
    ) -> Result<Self, SourcePortAuditError> {
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
            zero_sectors,
            zero_certificates,
            original_row_ids,
            original_sources,
        })
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

    fn check_sector(
        &self,
        sector: [bool; N],
        permutation: Option<[usize; N]>,
        solution: &SectorSolution<N>,
    ) -> Result<program::SectorCheck<N>, SourcePortAuditError> {
        let start = Instant::now();
        if self.zero_sectors.contains(&sector) {
            return Err(error("a solved sector was also declared zero"));
        }
        let ordering = match permutation {
            None => OrderingPolicy::SpiredUncutV1,
            Some(slots) => {
                // Validate the source-port permutation before indexing ranks.
                crate::solver::IntegralOrder::new(sector, [false; N])
                    .with_permutation(slots)
                    .map_err(error)?;
                let mut ranks = [0; N];
                for (rank, slot) in slots.into_iter().enumerate() {
                    ranks[slot] = rank;
                }
                let priority =
                    CoordinatePriority::try_new(N, &ranks, CoordinatePriorityLimits::default())
                        .map_err(error)?;
                OrderingPolicy::try_spired_with_coordinate_priority(&priority).map_err(error)?
            }
        };
        let config = SectorConfig {
            permutation,
            zero_sectors: self.zero_sectors.clone(),
            ..SectorConfig::default()
        };
        let solver = SectorSolver::new(&self.sources, sector, config).map_err(error)?;
        let mut order = crate::solver::IntegralOrder::new(sector, [false; N]);
        if let Some(slots) = permutation {
            order = order.with_permutation(slots).map_err(error)?;
        }
        let mut report = SourcePortSectorAudit {
            sector,
            ordering,
            rules: solution.rules.len(),
            exact_replayed_rules: 0,
            uniformly_descending_rules: 0,
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
        let mut stored_boxes = Vec::new();
        let mut checked_boxes = Vec::new();
        let mut retained_rules = Vec::new();
        for (ordinal, rule) in solution.rules.iter().enumerate() {
            let stored = match geometry::application_boxes(
                rule,
                self.sources.index_variables(),
                &sector,
                &[],
            ) {
                Ok(stored) => stored,
                Err(issue) => {
                    report
                        .issues
                        .push(format!("rule {ordinal} stored guard geometry: {issue}"));
                    continue;
                }
            };
            stored_boxes.extend(geometry::copy_boxes(&stored)?);
            match replay::replay_rule(
                &self.sources,
                &self.original_row_ids,
                &self.original_sources,
                solver.basis(),
                &order,
                &self.zero_sectors,
                rule,
                &stored,
            ) {
                Ok(replay) => {
                    report.exact_replayed_rules += 1;
                    report.replay_source_entries += replay.ordinary.contributions.len();
                    report.additional_replay_guard_branches += replay.additional_exceptions.len();
                    let checked = match geometry::application_boxes(
                        rule,
                        self.sources.index_variables(),
                        &sector,
                        &replay.additional_exceptions,
                    ) {
                        Ok(checked) => checked,
                        Err(issue) => {
                            report.issues.push(format!(
                                "rule {ordinal} certificate guard geometry: {issue}"
                            ));
                            continue;
                        }
                    };
                    match geometry::prove_descent(
                        rule,
                        &checked,
                        &sector,
                        ordering,
                        self.sources.index_variables(),
                    ) {
                        Ok(()) => {
                            report.uniformly_descending_rules += 1;
                            let retained =
                                program::CheckedRule::retain(rule, replay.ordinary, checked)?;
                            checked_boxes.extend(geometry::copy_boxes(&retained.application)?);
                            retained_rules.push(retained);
                        }
                        Err(issue) => report
                            .issues
                            .push(format!("rule {ordinal} descent: {issue}")),
                    }
                }
                Err(issue) => report
                    .issues
                    .push(format!("rule {ordinal} replay: {issue}")),
            }
        }
        let terminal_boxes = geometry::terminal_boxes(&solution.finite_residuals, &sector)?;
        let terminals = solution
            .finite_residuals
            .iter()
            .map(|terminal| std::array::from_fn(|axis| i64::from(terminal[axis].value())))
            .collect();
        stored_boxes.extend(geometry::copy_boxes(&terminal_boxes)?);
        checked_boxes.extend(terminal_boxes);
        (
            report.stored_guard_uncovered_boxes,
            report.stored_guard_unbounded_boxes,
        ) = geometry::uncovered(N, stored_boxes)?;
        (
            report.checked_rule_uncovered_boxes,
            report.checked_rule_unbounded_boxes,
        ) = geometry::uncovered(N, checked_boxes)?;
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
        let expected = 1_u128
            .checked_shl(u32::try_from(N).map_err(error)?)
            .ok_or_else(|| error("sector census exceeds diagnostic counter capacity"))?;
        let mut seen: BTreeSet<_> = self.zero_sectors.iter().copied().collect();
        for sector in sectors {
            if !seen.insert(sector) {
                return Err(error("duplicate or zero solved sector"));
            }
        }
        if seen.len() as u128 != expected {
            return Err(error("sector census does not exhaust all masks"));
        }
        Ok(())
    }
}
