//! Private checked input for the existing artifact installer.
//!
//! No public artifact, persistence format, runtime evaluator or new algebra
//! authority is introduced here. The owner retains the evidence already checked
//! by the cold audit, without retaining compact search powers or a solver basis.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::family::IntegralFamily;
use crate::foundry::completion::LatticeBox;
use crate::sector::OrderingPolicy;
use crate::solver::{SectorRule, SectorSolution};

use super::certificate::{OriginalRowNormalization, OriginalSourceReplay};
use super::normalization::OriginalSourceCorpus;
use super::{
    AffineApplicationDomain, SourcePortAudit, SourcePortAuditError, SourcePortInstallEvent,
    SourcePortSectorAudit, error,
};

#[path = "lower/mod.rs"]
pub(super) mod lower;

/// One physical displacement from canonical target indices, never a compact
/// source-port Power. Native coefficients remain bound to the owned corpus.
pub(super) struct CheckedRhs<const N: usize> {
    pub(super) shift: [i64; N],
    pub(super) coefficient: Coefficient,
}

/// The target is I(n) on the fixed-coordinate face and application boxes.
/// Boxes are local orthant coordinates and retain genuine infinite endpoints.
/// Their union includes every stored/recomputed index guard. Full nonzero
/// polynomials separately retain parameter poles and source applicability.
pub(super) struct CheckedRule<const N: usize> {
    pub(super) fixed: [Option<i64>; N],
    pub(super) rhs: Vec<CheckedRhs<N>>,
    pub(super) application: Vec<LatticeBox>,
    pub(super) nonzero_conditions: Vec<CoefficientPolynomial>,
    pub(super) ordinary: OriginalSourceReplay<N>,
    /// Exact coupled target locus. `application` is only its coordinate-face
    /// prefilter; runtime ownership must evaluate this predicate on original
    /// powers. Exclusions retain their own exact predicates below.
    pub(super) affine: Option<Arc<AffineApplicationDomain>>,
    /// Exact exceptional affine predicates removed from the rectangular
    /// application prefilter.  Kept separate from `affine`: an affine target
    /// and its affine guard branches form a predicate partition, not a box.
    pub(super) affine_exclusions: Vec<Arc<AffineApplicationDomain>>,
}

impl<const N: usize> CheckedRule<N> {
    /// Called only after original-source replay, guard admission and uniform
    /// descent. This is representation transfer, not another algebra proof.
    pub(super) fn retain(
        rule: &SectorRule<N>,
        ordinary: OriginalSourceReplay<N>,
        application: Vec<LatticeBox>,
        affine: Option<Arc<AffineApplicationDomain>>,
        affine_exclusions: Vec<Arc<AffineApplicationDomain>>,
    ) -> Result<Self, SourcePortAuditError> {
        if ordinary.normalization != OriginalRowNormalization::OriginalGeneratorOrdinaryV1 {
            return Err(error(
                "checked rule requires original-generator source weights",
            ));
        }
        let face = rule.candidate.case.face();
        if rule.candidate.target != face.integral() {
            return Err(error(
                "checked program target is not canonical for its face",
            ));
        }
        if affine.is_some() != rule.candidate.case.affine().is_some() {
            return Err(error("affine target evidence does not match target case"));
        }
        let fixed = std::array::from_fn(|axis| face.fixed()[axis].map(i64::from));
        let mut rhs = Vec::with_capacity(rule.candidate.rhs.len());
        let mut nonzero_conditions = ordinary.source_conditions.clone();
        for term in &rule.candidate.rhs {
            if (0..N).any(|axis| {
                term.integral[axis].is_symbolic() != rule.candidate.target[axis].is_symbolic()
            }) {
                return Err(error(
                    "checked RHS changes the fixed target-coordinate pattern",
                ));
            }
            rhs.push(CheckedRhs {
                shift: std::array::from_fn(|axis| {
                    i64::from(term.integral[axis].value())
                        - i64::from(rule.candidate.target[axis].value())
                }),
                coefficient: term.coefficient.clone(),
            });
            retain_condition(&mut nonzero_conditions, &term.coefficient.denominator)?;
        }
        for contribution in &ordinary.contributions {
            retain_condition(&mut nonzero_conditions, &contribution.weight.denominator)?;
        }
        Ok(Self {
            fixed,
            rhs,
            application,
            nonzero_conditions,
            ordinary,
            affine,
            affine_exclusions,
        })
    }
}

fn retain_condition(
    retained: &mut Vec<CoefficientPolynomial>,
    condition: &CoefficientPolynomial,
) -> Result<(), SourcePortAuditError> {
    if condition.is_zero() {
        return Err(error(
            "checked program contains an identically zero nonzero condition",
        ));
    }
    if !condition.is_constant() && !retained.contains(condition) {
        retained.push(condition.clone());
    }
    Ok(())
}

/// Intermediate check results may be incomplete. Public diagnostics project
/// only the report; the consuming program gate rejects every incomplete check.
pub(super) struct SectorCheck<const N: usize> {
    pub(super) report: SourcePortSectorAudit<N>,
    pub(super) rules: Vec<CheckedRule<N>>,
    pub(super) terminals: Vec<[i64; N]>,
}

struct CheckedSector<const N: usize> {
    sector: [bool; N],
    rules: Vec<CheckedRule<N>>,
    terminals: Vec<[i64; N]>,
}

/// Sole private installation input for this producer. It owns the full family
/// and original context/corpus, not just fingerprints. Consuming it lowers
/// supplied-domain cells and runs the existing complete-artifact installer.
/// Durable loading instead replays stored original combinations through the
/// same private cell verifier, without reconstructing solver output.
pub(super) struct CheckedProgram<const N: usize> {
    family: IntegralFamily,
    root_sector: crate::sector::Mask,
    original_sources: OriginalSourceCorpus,
    zero_sectors: Arc<[[bool; N]]>,
    inherited_source_conditions: Vec<CoefficientPolynomial>,
    ordering: OrderingPolicy,
    sectors: BTreeMap<[bool; N], CheckedSector<N>>,
}

impl<const N: usize> CheckedProgram<N> {
    #[cfg(test)]
    fn install(self) -> Result<super::super::ClosedArtifact, SourcePortAuditError> {
        self.install_with_observer(Instant::now(), &mut |_| {})
    }

    fn install_with_observer(
        self,
        started: Instant,
        observe: &mut dyn FnMut(SourcePortInstallEvent<'_, N>),
    ) -> Result<super::super::ClosedArtifact, SourcePortAuditError> {
        use super::super::install::{
            ClosingArtifactCandidate, SOURCE_PORT_ALGORITHM_ID, install_source_port,
        };
        use super::super::model::{
            ArtifactSchemaVersion, CommonMassHomogeneityProof, ZeroSectorTerminal,
            ZeroTerminalProof,
        };
        use crate::family::IntegralKey;
        use crate::identity::ParametricIbpGenerator;
        use crate::sector::Mask;

        let generator = ParametricIbpGenerator::try_new(&self.family).map_err(error)?;
        let context = self.original_sources.context().clone();
        let mut rule_cells = Vec::new();
        let mut masters = BTreeSet::new();
        let sector_count = self.sectors.len();
        for (_, sector) in self.sectors {
            let cells_before = rule_cells.len();
            let total = sector.rules.len();
            for (ordinal, rule) in sector.rules.into_iter().enumerate() {
                observe(SourcePortInstallEvent::LoweringRule {
                    sector: sector.sector,
                    ordinal,
                    total,
                    elapsed: started.elapsed(),
                });
                rule_cells.extend(lower::lower_rule(
                    &self.original_sources,
                    &generator,
                    sector.sector,
                    self.ordering,
                    &self.zero_sectors,
                    &self.inherited_source_conditions,
                    rule,
                )?);
            }
            for terminal in sector.terminals {
                masters.insert(IntegralKey::try_new(terminal).map_err(error)?);
            }
            observe(SourcePortInstallEvent::LoweredSector {
                sector: sector.sector,
                cells: rule_cells.len() - cells_before,
                elapsed: started.elapsed(),
            });
        }
        drop(generator);
        let zero_sectors = self
            .zero_sectors
            .iter()
            .map(|sector| {
                Ok(ZeroSectorTerminal::new(
                    Mask::try_new(*sector).map_err(error)?,
                    ZeroTerminalProof::LeePomeranskyRankDeficiency,
                ))
            })
            .collect::<Result<_, SourcePortAuditError>>()?;
        let candidate = ClosingArtifactCandidate {
            schema: ArtifactSchemaVersion::CURRENT,
            algorithm_id: SOURCE_PORT_ALGORITHM_ID,
            arity: N,
            ordering: self.ordering,
            supported_root_power_bounds: super::scope::root_bounds(&self.root_sector)
                .map_err(error)?,
            family: self.family,
            context,
            source_relations: self.original_sources.into_relations(),
            rules: Vec::new(),
            rule_cells,
            canonicalizer: None,
            dependencies: Vec::new(),
            factorization_rules: Vec::new(),
            masters,
            zero_sectors,
            common_mass_homogeneity: Some(CommonMassHomogeneityProof::UniformVacuumMassSquared),
        };
        observe(SourcePortInstallEvent::Installing {
            sectors: sector_count,
            rule_cells: candidate.rule_cells.len(),
            terminals: candidate.masters.len(),
            elapsed: started.elapsed(),
        });
        let artifact = install_source_port(candidate).map_err(error)?;
        observe(SourcePortInstallEvent::Installed {
            elapsed: started.elapsed(),
        });
        Ok(artifact)
    }
}

impl<const N: usize> SourcePortAudit<N> {
    /// Consume complete real solver output into the existing in-memory artifact.
    /// Durable decoding replays the same exact predicates and source weights.
    pub fn install_complete(
        self,
        family: IntegralFamily,
        sectors: impl IntoIterator<Item = ([bool; N], Option<[usize; N]>, SectorSolution<N>)>,
    ) -> Result<super::super::ClosedArtifact, SourcePortAuditError> {
        self.install_complete_with_observer(family, sectors, |_| {})
    }

    /// Observe existing exact checking, cell lowering and final installation.
    /// This is the same consuming authority path as `install_complete`; the
    /// borrowed callback cannot grant authority or alter scheduling, inputs,
    /// source provenance, or durable bytes. No worker pool is created here.
    pub fn install_complete_with_observer(
        self,
        family: IntegralFamily,
        sectors: impl IntoIterator<Item = ([bool; N], Option<[usize; N]>, SectorSolution<N>)>,
        mut observe: impl FnMut(SourcePortInstallEvent<'_, N>),
    ) -> Result<super::super::ClosedArtifact, SourcePortAuditError> {
        let started = Instant::now();
        self.retain_program_with_observer(family, sectors, started, &mut observe)?
            .install_with_observer(started, &mut observe)
    }
    /// Consume real solver output, never caller-editable diagnostic reports.
    /// Each declared sector order is independently checked. A single existing
    /// artifact cannot silently flatten incompatible coordinate priorities.
    #[cfg(test)]
    pub(super) fn retain_program(
        self,
        family: IntegralFamily,
        sectors: impl IntoIterator<Item = ([bool; N], Option<[usize; N]>, SectorSolution<N>)>,
    ) -> Result<CheckedProgram<N>, SourcePortAuditError> {
        self.retain_program_with_observer(family, sectors, Instant::now(), &mut |_| {})
    }

    fn retain_program_with_observer(
        self,
        family: IntegralFamily,
        sectors: impl IntoIterator<Item = ([bool; N], Option<[usize; N]>, SectorSolution<N>)>,
        started: Instant,
        observe: &mut dyn FnMut(SourcePortInstallEvent<'_, N>),
    ) -> Result<CheckedProgram<N>, SourcePortAuditError> {
        if family.fingerprint() != self.original_sources.family_fingerprint() {
            return Err(error(
                "checked program family differs from the original-source corpus",
            ));
        }
        let mut retained = BTreeMap::new();
        let mut ordering = None;
        for (ordinal, (sector, permutation, solution)) in sectors.into_iter().enumerate() {
            if retained.contains_key(&sector) {
                return Err(error("duplicate solved sector in checked program"));
            }
            observe(SourcePortInstallEvent::CheckingSector {
                ordinal,
                sector,
                rules: solution.rules.len(),
                elapsed: started.elapsed(),
            });
            let checked = self.check_sector(sector, permutation, &solution)?;
            observe(SourcePortInstallEvent::CheckedSector {
                ordinal,
                report: &checked.report,
                elapsed: started.elapsed(),
            });
            retain_common_order(&mut ordering, checked.report.ordering)?;
            let report = &checked.report;
            if !report.issues.is_empty()
                || report
                    .exact_replayed_rules
                    .checked_add(report.redundant_affine_rules)
                    != Some(report.rules)
                || report
                    .uniformly_descending_rules
                    .checked_add(report.redundant_affine_rules)
                    != Some(report.rules)
                || checked.rules.len() != report.exact_replayed_rules
                || checked.terminals.len() != report.finite_terminals
                || report.checked_rule_uncovered_boxes != 0
                || report.checked_rule_unbounded_boxes != 0
            {
                return Err(error(format!(
                    "sector {sector:?} cannot enter the checked program: \
                     {}/{} replayed, {}/{} descending, {} redundant, {} retained, {} uncovered boxes, issues={:?}",
                    report.exact_replayed_rules,
                    report.rules,
                    report.uniformly_descending_rules,
                    report.rules,
                    report.redundant_affine_rules,
                    checked.rules.len(),
                    report.checked_rule_uncovered_boxes,
                    report.issues,
                )));
            }
            // Preserve rule precedence, but terminal identity has no chronology.
            let terminals = checked.terminals.into_iter().collect::<BTreeSet<_>>();
            retained.insert(
                sector,
                CheckedSector {
                    sector,
                    rules: checked.rules,
                    terminals: terminals.into_iter().collect(),
                },
            );
        }
        self.validate_sector_masks(retained.keys().copied())?;
        if retained.is_empty() {
            return Err(error(
                "zero-only source-port root scopes are not yet supported",
            ));
        }
        let inherited_source_conditions = self.sources.conditions().to_vec();
        Ok(CheckedProgram {
            family,
            root_sector: self.root_sector,
            original_sources: self.original_sources,
            zero_sectors: self.zero_sectors,
            inherited_source_conditions,
            ordering: ordering.unwrap_or(OrderingPolicy::SpiredUncutV1),
            sectors: retained,
        })
    }
}

fn retain_common_order(
    retained: &mut Option<OrderingPolicy>,
    incoming: OrderingPolicy,
) -> Result<(), SourcePortAuditError> {
    if retained.is_some_and(|ordering| ordering != incoming) {
        return Err(error(
            "checked program sectors have incompatible coordinate priorities",
        ));
    }
    *retained = Some(incoming);
    Ok(())
}

#[cfg(test)]
pub(super) fn lower_sector_for_test<const N: usize>(
    audit: &SourcePortAudit<N>,
    family: &IntegralFamily,
    sector: [bool; N],
    permutation: Option<[usize; N]>,
    solution: &SectorSolution<N>,
) -> Result<usize, SourcePortAuditError> {
    let checked = audit.check_sector(sector, permutation, solution)?;
    if !checked.report.issues.is_empty()
        || checked.report.checked_rule_uncovered_boxes != 0
        || checked.report.checked_rule_unbounded_boxes != 0
        || checked.report.exact_replayed_rules + checked.report.redundant_affine_rules
            != checked.report.rules
    {
        return Err(error(format!(
            "incomplete checked sector: {:?}",
            checked.report
        )));
    }
    let generator = crate::identity::ParametricIbpGenerator::try_new(family).map_err(error)?;
    let mut count = 0;
    for (ordinal, rule) in checked.rules.into_iter().enumerate() {
        let cells = lower::lower_rule(
            &audit.original_sources,
            &generator,
            sector,
            checked.report.ordering,
            &audit.zero_sectors,
            audit.sources.conditions(),
            rule,
        )
        .map_err(|issue| error(format!("rule {ordinal} lowering: {issue}")))?;
        count += cells.len();
    }
    Ok(count)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(in crate::foundry::artifact) mod scoped_tests;
