//! Private checked input for the existing artifact installer.
//!
//! No public artifact, persistence format, runtime evaluator or new algebra
//! authority is introduced here. The owner retains the evidence already checked
//! by the cold audit, without retaining compact search powers or a solver basis.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::family::IntegralFamily;
use crate::foundry::completion::LatticeBox;
use crate::sector::{OrderingPolicy, zero};
use crate::solver::{SectorRule, SectorSolution};

use super::certificate::{OriginalRowNormalization, OriginalSourceReplay};
use super::normalization::OriginalSourceCorpus;
use super::{SourcePortAudit, SourcePortAuditError, SourcePortSectorAudit, error};

#[path = "lower/mod.rs"]
pub(super) mod lower;

/// One physical displacement from canonical target indices, never a compact
/// source-port Power. Native coefficients remain bound to the owned corpus.
#[allow(
    dead_code,
    reason = "Retained input for the next existing-artifact lowering slice."
)]
pub(super) struct CheckedRhs<const N: usize> {
    pub(super) shift: [i64; N],
    pub(super) coefficient: Coefficient,
}

/// The target is I(n) on the fixed-coordinate face and application boxes.
/// Boxes are local orthant coordinates and retain genuine infinite endpoints.
/// Their union includes every stored/recomputed index guard. Full nonzero
/// polynomials separately retain parameter poles and source applicability.
#[allow(
    dead_code,
    reason = "Retained input for the next existing-artifact lowering slice."
)]
pub(super) struct CheckedRule<const N: usize> {
    pub(super) fixed: [Option<i64>; N],
    pub(super) rhs: Vec<CheckedRhs<N>>,
    pub(super) application: Vec<LatticeBox>,
    pub(super) nonzero_conditions: Vec<CoefficientPolynomial>,
    pub(super) ordinary: OriginalSourceReplay<N>,
}

impl<const N: usize> CheckedRule<N> {
    /// Called only after original-source replay, guard admission and uniform
    /// descent. This is representation transfer, not another algebra proof.
    pub(super) fn retain(
        rule: &SectorRule<N>,
        ordinary: OriginalSourceReplay<N>,
        application: Vec<LatticeBox>,
    ) -> Result<Self, SourcePortAuditError> {
        if ordinary.normalization != OriginalRowNormalization::OriginalGeneratorOrdinaryV1 {
            return Err(error(
                "checked rule requires original-generator source weights",
            ));
        }
        let case =
            rule.candidate.case.coordinate().ok_or_else(|| {
                error("checked program currently requires coordinate target faces")
            })?;
        if rule.candidate.target != case.integral() {
            return Err(error(
                "checked program target is not canonical for its face",
            ));
        }
        let fixed = std::array::from_fn(|axis| case.fixed()[axis].map(i64::from));
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

#[allow(
    dead_code,
    reason = "Retained input for the next existing-artifact lowering slice."
)]
struct CheckedSector<const N: usize> {
    sector: [bool; N],
    rules: Vec<CheckedRule<N>>,
    terminals: Vec<[i64; N]>,
}

/// Sole private installation input for this producer. It owns the full family
/// and original context/corpus, not just fingerprints. It is NOT ClosedArtifact:
/// cell lowering, cold codec replay and installer admission still remain.
#[allow(
    dead_code,
    reason = "Private checked owner; existing-artifact installation is the next slice."
)]
pub(super) struct CheckedProgram<const N: usize> {
    family: IntegralFamily,
    original_sources: OriginalSourceCorpus,
    zero_sectors: Arc<[[bool; N]]>,
    zero_certificates: Vec<zero::Certificate>,
    inherited_source_conditions: Vec<CoefficientPolynomial>,
    ordering: OrderingPolicy,
    sectors: BTreeMap<[bool; N], CheckedSector<N>>,
}

impl<const N: usize> CheckedProgram<N> {
    fn install(self) -> Result<super::super::ClosedArtifact, SourcePortAuditError> {
        use super::super::install::{
            ClosingArtifactCandidate, SOURCE_PORT_ALGORITHM_ID, install_source_port,
        };
        use super::super::model::{
            ArtifactSchemaVersion, CommonMassHomogeneityProof, ZeroSectorTerminal,
            ZeroTerminalProof,
        };
        use crate::family::IntegralKey;
        use crate::identity::ParametricIbpGenerator;
        use crate::sector::{InteriorBounds, Mask};

        let generator = ParametricIbpGenerator::try_new(&self.family).map_err(error)?;
        let context = self.original_sources.context().clone();
        let mut rule_cells = Vec::new();
        let mut masters = BTreeSet::new();
        for (_, sector) in self.sectors {
            for rule in sector.rules {
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
            supported_root_power_bounds: vec![InteriorBounds::new(i64::MIN, i64::MAX); N]
                .into_boxed_slice(),
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
        install_source_port(candidate).map_err(error)
    }
}

impl<const N: usize> SourcePortAudit<N> {
    /// Consume complete real solver output into the existing in-memory artifact.
    /// Cold durable encoding is intentionally a separate, not-yet-enabled gate.
    pub fn install_complete(
        self,
        family: IntegralFamily,
        sectors: impl IntoIterator<Item = ([bool; N], Option<[usize; N]>, SectorSolution<N>)>,
    ) -> Result<super::super::ClosedArtifact, SourcePortAuditError> {
        self.retain_program(family, sectors)?.install()
    }
    /// Consume real solver output, never caller-editable diagnostic reports.
    /// Each declared sector order is independently checked. A single existing
    /// artifact cannot silently flatten incompatible coordinate priorities.
    #[allow(
        dead_code,
        reason = "Private checked producer; installer wiring follows the release gate."
    )]
    pub(super) fn retain_program(
        self,
        family: IntegralFamily,
        sectors: impl IntoIterator<Item = ([bool; N], Option<[usize; N]>, SectorSolution<N>)>,
    ) -> Result<CheckedProgram<N>, SourcePortAuditError> {
        if family.fingerprint() != self.original_sources.family_fingerprint() {
            return Err(error(
                "checked program family differs from the original-source corpus",
            ));
        }
        let mut retained = BTreeMap::new();
        let mut ordering = None;
        for (sector, permutation, solution) in sectors {
            if retained.contains_key(&sector) {
                return Err(error("duplicate solved sector in checked program"));
            }
            let checked = self.check_sector(sector, permutation, &solution)?;
            retain_common_order(&mut ordering, checked.report.ordering)?;
            let report = &checked.report;
            if !report.issues.is_empty()
                || report.exact_replayed_rules != report.rules
                || report.uniformly_descending_rules != report.rules
                || checked.rules.len() != report.rules
                || checked.terminals.len() != report.finite_terminals
                || report.checked_rule_uncovered_boxes != 0
                || report.checked_rule_unbounded_boxes != 0
            {
                return Err(error(format!(
                    "sector {sector:?} cannot enter the checked program: \
                     {}/{} replayed, {}/{} descending, {} uncovered boxes, issues={:?}",
                    report.exact_replayed_rules,
                    report.rules,
                    report.uniformly_descending_rules,
                    report.rules,
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
        let inherited_source_conditions = self.sources.conditions().to_vec();
        Ok(CheckedProgram {
            family,
            original_sources: self.original_sources,
            zero_sectors: self.zero_sectors,
            zero_certificates: self.zero_certificates,
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
mod tests;
