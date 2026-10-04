//! Bounded, opt-in observations of an already replayed source circuit.
//!
//! These identities hold only on their exact declared guarded domain, modulo
//! the audit owner's authenticated zero sectors. They are NOT unrestricted
//! polynomial identities, descent proofs, macro rules or installation seals.
//! Copying getter data produces proposals: any composition must regenerate
//! full original rows and re-enter the existing exact authority pipeline.

use std::time::Duration;

use crate::algebra::{
    Coefficient, CoefficientPolynomial, IndexedCoefficient, IndexedCoefficientContext,
    IndexedPolynomial, coefficient_clone_owned_retained_byte_bound,
    polynomial_clone_owned_heap_byte_bound,
};
use crate::foundry::completion::LatticeBox;
use crate::identity::{IndexShift, IntegralShift};
use crate::sector::{Mask, OrderingPolicy, zero};
use crate::solver::{Case, ExceptionalConditions, Integral, SectorRule, SectorSolution};

use super::super::certificate::OriginalRowNormalization;
use super::super::{
    AffineApplicationDomain, AffineOwnershipRole, OriginalSourceContribution, SourcePortAudit,
    SourcePortAuditError, error, geometry, replay,
};
use super::validate_ordinals;

/// Cumulative OUTPUT retention policy, not a bound on native replay scratch.
/// All newly retained coefficient copies are precharged with the existing
/// native clone-owned byte bounds. External supervision still owns RSS/time.
#[derive(Clone, Copy, Debug)]
pub struct ReplayCircuitLimits {
    pub max_rules: usize,
    pub max_source_entries: usize,
    pub max_rhs_terms: usize,
    pub max_conditions: usize,
    pub max_coefficient_terms: usize,
    pub max_coefficient_clone_owned_bytes: usize,
    pub max_coordinate_cells: usize,
}

impl Default for ReplayCircuitLimits {
    fn default() -> Self {
        Self {
            max_rules: 256,
            max_source_entries: 65_536,
            max_rhs_terms: 65_536,
            max_conditions: 262_144,
            max_coefficient_terms: 4_000_000,
            max_coefficient_clone_owned_bytes: 256 * 1024 * 1024,
            max_coordinate_cells: 4_000_000,
        }
    }
}

/// An immutable, selected identity observation. Canonical offsets already
/// include the recovered recentering: NEVER apply `raw_recenter` again.
#[derive(Debug)]
pub struct ReplayedSourceCircuit<const N: usize> {
    ordinal: usize,
    case: Case<N>,
    exceptions: ExceptionalConditions,
    application: Vec<LatticeBox>,
    contributions: Vec<OriginalSourceContribution>,
    rhs: Vec<(IndexShift, IndexedCoefficient)>,
    conditions: Vec<IndexedPolynomial>,
    raw_target: Integral<N>,
    raw_pivot: IndexedCoefficient,
    raw_recenter: [i16; N],
}

impl<const N: usize> ReplayedSourceCircuit<N> {
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub fn declared_case(&self) -> &Case<N> {
        &self.case
    }
    /// OR of all-zero conjunctions, not a list of independent nonzero guards.
    pub fn declared_exceptions(&self) -> &ExceptionalConditions {
        &self.exceptions
    }
    pub fn application_boxes(&self) -> impl ExactSizeIterator<Item = (&[u64], &[Option<u64>])> {
        self.application.iter().map(|b| (b.lower(), b.upper()))
    }
    pub fn contributions(&self) -> &[OriginalSourceContribution] {
        &self.contributions
    }
    pub fn rhs(&self) -> &[(IndexShift, IndexedCoefficient)] {
        &self.rhs
    }
    pub fn nonzero_conditions(&self) -> &[IndexedPolynomial] {
        &self.conditions
    }
    pub fn raw_target(&self) -> &Integral<N> {
        &self.raw_target
    }
    /// The recovered pivot BEFORE recentering. Contributions, RHS and
    /// conditions instead use the canonical target frame.
    pub fn raw_pivot(&self) -> &IndexedCoefficient {
        &self.raw_pivot
    }
    pub fn raw_recenter(&self) -> &[i16; N] {
        &self.raw_recenter
    }
}

/// One borrowed mathematical owner for all retained rules. In particular the
/// non-Clone zero certificates cannot be detached or replaced by asserted masks.
/// The audit's context/corpus/evidence are borrowed, never copied per rule.
pub struct ReplayedSourceCircuitBatch<'audit, const N: usize> {
    audit: &'audit SourcePortAudit<N>,
    sector: [bool; N],
    ordering: OrderingPolicy,
    circuits: Vec<ReplayedSourceCircuit<N>>,
    elapsed: Duration,
}

impl<const N: usize> ReplayedSourceCircuitBatch<'_, N> {
    pub fn family_fingerprint(&self) -> &str {
        self.audit.original_sources.family_fingerprint()
    }
    pub fn context(&self) -> &IndexedCoefficientContext {
        self.audit.original_sources.context()
    }
    pub fn root_sector(&self) -> &Mask {
        &self.audit.root_sector
    }
    pub fn sector(&self) -> &[bool; N] {
        &self.sector
    }
    pub fn ordering(&self) -> &OrderingPolicy {
        &self.ordering
    }
    pub fn zero_certificates(&self) -> &[zero::Certificate] {
        &self.audit.zero_certificates
    }
    pub fn circuits(&self) -> &[ReplayedSourceCircuit<N>] {
        &self.circuits
    }
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }
}

impl<const N: usize> SourcePortAudit<N> {
    /// Retain only explicitly selected exact replay observations. Count-only
    /// APIs do not take this path. No whole-sector/recursive proof is attempted.
    pub fn replay_sector_rule_circuits(
        &self,
        sector: [bool; N],
        permutation: Option<[usize; N]>,
        solution: &SectorSolution<N>,
        ordinals: &[usize],
        limits: ReplayCircuitLimits,
    ) -> Result<ReplayedSourceCircuitBatch<'_, N>, SourcePortAuditError> {
        validate_ordinals(ordinals, solution.rules.len())?;
        let mut budget = RetentionBudget::new(limits);
        budget.charge_rules(ordinals.len())?;
        let (ordering, circuits, elapsed) = self.with_replayed_sector_rule_iter(
            sector,
            permutation,
            &solution.order,
            &solution.rules,
            ordinals.iter().copied(),
            true,
            |ordinal, rule, partition, checked| {
                budget.charge::<N>(rule, &partition, &checked, self.sources.conditions())?;
                retain(self, ordinal, rule, partition, checked)
            },
        )?;
        Ok(ReplayedSourceCircuitBatch {
            audit: self,
            sector,
            ordering,
            circuits,
            elapsed,
        })
    }
}

pub(super) fn require_coordinate<const N: usize>(
    rule: &SectorRule<N>,
    partition: &geometry::ApplicationPartition,
    sector: &[bool; N],
) -> Result<(), SourcePortAuditError> {
    if let Some(case) = rule.candidate.case.affine() {
        return Err(SourcePortAuditError::UnsupportedAffineOwnership {
            domain: AffineApplicationDomain::from_case(case, sector).map_err(error)?,
            role: AffineOwnershipRole::Target,
        });
    }
    if let Some(domain) = partition.affine_exclusions.first() {
        return Err(SourcePortAuditError::UnsupportedAffineOwnership {
            domain: (**domain).clone(),
            role: AffineOwnershipRole::Exceptional,
        });
    }
    Ok(())
}

fn retain<const N: usize>(
    audit: &SourcePortAudit<N>,
    ordinal: usize,
    rule: &SectorRule<N>,
    partition: geometry::ApplicationPartition,
    checked: replay::Replay<N>,
) -> Result<ReplayedSourceCircuit<N>, SourcePortAuditError> {
    if checked.ordinary.normalization != OriginalRowNormalization::OriginalGeneratorOrdinaryV1 {
        return Err(error(
            "retained circuit is not in original generator normalization",
        ));
    }
    let raw = checked
        .raw_pivot
        .ok_or_else(|| error("retained circuit lost its recovered pivot"))?;
    let context = audit.original_sources.context();
    let algebra = audit.limits.rule_derivation.indexed_algebra.exact_algebra;
    let polynomial = |p| {
        context
            .admit_native_polynomial_result_with_limits(p, algebra)
            .map_err(error)
    };
    let coefficient = |c| {
        context
            .admit_native_result_with_limits(c, algebra)
            .map_err(error)
    };
    let mut conditions = Vec::new();
    // Retain every pre-cancellation condition; no joining or zero-weight
    // deletion occurs here, and exceptions keep their separate OR/AND meaning.
    for p in audit
        .sources
        .conditions()
        .iter()
        .chain(&checked.ordinary.source_conditions)
        .chain(
            checked
                .ordinary
                .contributions
                .iter()
                .map(|c| &c.weight.denominator),
        )
        .chain(
            rule.candidate
                .rhs
                .iter()
                .map(|t| &t.coefficient.denominator),
        )
    {
        if p.is_zero() {
            return Err(error("retained circuit contains a zero nonzero condition"));
        }
        conditions.push(polynomial(p.clone())?);
    }
    let contributions = checked
        .ordinary
        .contributions
        .into_iter()
        .map(|c| {
            Ok(OriginalSourceContribution {
                source_row: c.source_row,
                offset: IntegralShift::try_new(c.offset).map_err(error)?,
                weight: coefficient(c.weight)?,
            })
        })
        .collect::<Result<Vec<_>, SourcePortAuditError>>()?;
    let rhs = rule
        .candidate
        .rhs
        .iter()
        .map(|t| {
            let shifts = std::array::from_fn::<_, N, _>(|axis| {
                i64::from(t.integral[axis].value()) - i64::from(rule.candidate.target[axis].value())
            });
            if (0..N).any(|a| t.integral[a].is_symbolic() != rule.candidate.target[a].is_symbolic())
            {
                return Err(error("retained RHS changes target coordinate pattern"));
            }
            Ok((
                IndexShift::try_new(shifts, N).map_err(error)?,
                coefficient(t.coefficient.clone())?,
            ))
        })
        .collect::<Result<Vec<_>, SourcePortAuditError>>()?;
    Ok(ReplayedSourceCircuit {
        ordinal,
        case: rule.candidate.case.clone(),
        exceptions: rule.exceptions.clone(),
        application: partition.boxes,
        contributions,
        rhs,
        conditions,
        raw_target: raw.target,
        raw_pivot: coefficient(raw.coefficient)?,
        raw_recenter: raw.recenter,
    })
}

mod budget;
use budget::RetentionBudget;
#[cfg(test)]
mod tests;
