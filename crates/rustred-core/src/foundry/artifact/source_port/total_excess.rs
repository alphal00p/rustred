//! Complete checked-program diagnostics with sector-dependent degree bounds.
//!
//! The simplex is never replaced by a box as a proof domain. Existing exact
//! replay, guards, descent and predicate coverage remain the authority for each
//! sector. Diagnostics and private retained programs share this single proof
//! pass. A report alone is not a durable artifact or installation input.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use crate::algebra::Coefficient;
use crate::family::IntegralFamily;
use crate::foundry::completion::{CompletionGeometryLimits, LatticeBox};
use crate::sector::{Mask, OrderingPolicy};
use crate::solver::SectorSolution;

use super::program::{CheckedRule, CheckedSector, SectorCheck};
use super::scope::{EntryDegreeBound, EntryScope};
use super::{
    AffineApplicationDomain, SourcePortAudit, SourcePortAuditError, SourcePortInstallEvent,
    SourcePortSectorAudit, error, geometry, sector_ordering,
};

/// A successful exact diagnostic over a complete root-sector census. Entry
/// degree and successor degree are deliberately separate. This immutable
/// report cannot publish a `ClosedArtifact`; encoding and runtime admission
/// must separately retain and enforce the same entry/envelope semantics.
#[derive(Debug)]
pub struct SourcePortTotalExcessAudit<const N: usize> {
    entry: EntryScope,
    ordering: OrderingPolicy,
    bounds: BTreeMap<[bool; N], u64>,
    sectors: Vec<SourcePortSectorAudit<N>>,
}

/// Successful preparation retains the exact checked rules as well as its
/// diagnostic projection. Only the complete ordered proof below constructs
/// this owner; a caller-editable report cannot reconstruct its evidence.
/// Lowered-cell coverage and successor admission are still separate gates.
pub(super) struct PreparedTotalExcess<const N: usize> {
    pub(super) report: SourcePortTotalExcessAudit<N>,
    pub(super) sectors: BTreeMap<[bool; N], CheckedSector<N>>,
}

impl<const N: usize> SourcePortTotalExcessAudit<N> {
    pub fn family_fingerprint(&self) -> &str {
        self.entry.family_fingerprint()
    }

    pub fn root_sector(&self) -> &Mask {
        self.entry.root()
    }

    /// `sum(max(n_i-1,0) + max(-n_i,0))` for starting integrals. Positive
    /// propagator excess is included; this is not a numerator-only bound.
    pub fn max_entry_total_excess_degree(&self) -> u64 {
        self.entry.bound().limit()
    }

    pub fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    /// Nonzero-sector successor budgets in deterministic mask order. Zero
    /// sectors need no bound because their independently proved value is zero.
    pub fn successor_degrees(&self) -> &BTreeMap<[bool; N], u64> {
        &self.bounds
    }

    pub fn max_successor_total_excess_degree(&self) -> u64 {
        self.bounds.values().copied().max().unwrap_or(0)
    }

    /// Reports follow the actual admitted ordering from harder sectors to
    /// easier sectors. Every report covers its propagated successor degree.
    pub fn sectors(&self) -> &[SourcePortSectorAudit<N>] {
        &self.sectors
    }

    pub fn contains_entry(&self, powers: &[i64]) -> Result<bool, SourcePortAuditError> {
        self.entry.contains(powers).map_err(error)
    }
}

impl<const N: usize> SourcePortAudit<N> {
    /// Check the complete nonzero sector census using inductive total-excess
    /// envelopes. Every root subsector admits entry degree `D`; descendants
    /// may have a larger degree, explicitly recorded in the returned report.
    /// Full original identities, guards and descent are checked unchanged.
    pub fn audit_complete_through_total_excess(
        &self,
        family: &IntegralFamily,
        sectors: impl IntoIterator<Item = ([bool; N], Option<[usize; N]>, SectorSolution<N>)>,
        max_entry_total_excess_degree: u64,
    ) -> Result<SourcePortTotalExcessAudit<N>, SourcePortAuditError> {
        self.audit_complete_through_total_excess_with_observer(
            family,
            sectors,
            max_entry_total_excess_degree,
            |_| {},
        )
    }

    /// The diagnostic observer sees the same original-source checks as the
    /// ordinary installer; this method performs no lowering or publication.
    pub fn audit_complete_through_total_excess_with_observer(
        &self,
        family: &IntegralFamily,
        sectors: impl IntoIterator<Item = ([bool; N], Option<[usize; N]>, SectorSolution<N>)>,
        max_entry_total_excess_degree: u64,
        mut observe: impl FnMut(SourcePortInstallEvent<'_, N>),
    ) -> Result<SourcePortTotalExcessAudit<N>, SourcePortAuditError> {
        self.prepare_complete_through_total_excess_with_observer(
            family,
            sectors,
            max_entry_total_excess_degree,
            &mut observe,
        )
        .map(|prepared| prepared.report)
    }

    /// The same complete proof feeds diagnostics and consuming retention.
    /// Checked rules now live until the whole pass succeeds, including for a
    /// report-only call; no second source replay or search is performed.
    pub(super) fn prepare_complete_through_total_excess_with_observer(
        &self,
        family: &IntegralFamily,
        sectors: impl IntoIterator<Item = ([bool; N], Option<[usize; N]>, SectorSolution<N>)>,
        max_entry_total_excess_degree: u64,
        observe: &mut dyn FnMut(SourcePortInstallEvent<'_, N>),
    ) -> Result<PreparedTotalExcess<N>, SourcePortAuditError> {
        self.limits.validate()?;
        if family.fingerprint() != self.original_sources.family_fingerprint() {
            return Err(error(
                "total-excess audit family differs from original sources",
            ));
        }
        let entry = EntryScope::try_new(
            family,
            &self.root_sector,
            EntryDegreeBound::MaxTotalExcessDegree(max_entry_total_excess_degree),
        )
        .map_err(error)?;
        let geometry = self.limits.cover_replay.geometry();
        let census_size = super::scope::sector_count(&self.root_sector).map_err(error)?;
        let mut budget = EnvelopeBudget::new(geometry);
        budget.charge(census_size, mul(census_size, N)?, mul(census_size, N)?)?;
        if N == 0 || N > geometry.max_arity {
            return Err(error("total-excess envelope has unsupported arity"));
        }
        let mut inputs = BTreeMap::new();
        let mut ordering = None;
        for (sector, permutation, solution) in sectors {
            entry.validate_sector(&sector).map_err(error)?;
            if inputs.len() >= census_size || inputs.contains_key(&sector) {
                return Err(error("duplicate or excessive sector in total-excess audit"));
            }
            let incoming = sector_ordering(sector, permutation)?;
            if ordering.is_some_and(|value| value != incoming) {
                return Err(error(
                    "total-excess sectors have incompatible coordinate priorities",
                ));
            }
            ordering = Some(incoming);
            inputs.insert(sector, (permutation, solution));
        }
        self.validate_sector_masks(inputs.keys().copied())?;
        let ordering =
            ordering.ok_or_else(|| error("zero-only total-excess roots are unsupported"))?;
        if !ordering.is_spired() {
            return Err(error(
                "total-excess envelope requires the checked Spired ordering",
            ));
        }
        // Sector order comes from the persisted integral comparator on sector
        // corners, never from BTreeMap mask iteration or bit significance.
        let mut ordered = inputs
            .into_iter()
            .map(|(sector, (permutation, solution))| {
                let corner = sector.map(i64::from);
                Ok((
                    ordering.complexity_key(&corner).map_err(error)?,
                    sector,
                    permutation,
                    solution,
                ))
            })
            .collect::<Result<Vec<_>, SourcePortAuditError>>()?;
        ordered.sort_by(|left, right| right.0.cmp(&left.0));
        let mut bounds: BTreeMap<_, _> = ordered
            .iter()
            .map(|(_, sector, _, _)| (*sector, max_entry_total_excess_degree))
            .collect();
        let mut reports = Vec::with_capacity(ordered.len());
        let mut retained = BTreeMap::new();
        let started = Instant::now();
        for (ordinal, (_, sector, permutation, solution)) in ordered.into_iter().enumerate() {
            let degree = bounds[&sector];
            observe(SourcePortInstallEvent::CheckingSector {
                ordinal,
                sector,
                rules: solution.rules.len(),
                elapsed: started.elapsed(),
            });
            let checked = self.check_sector_in_degree(
                sector,
                permutation,
                &solution,
                Some(degree),
                started,
                observe,
            )?;
            observe(SourcePortInstallEvent::CheckedSector {
                ordinal,
                report: &checked.report,
                elapsed: started.elapsed(),
            });
            require_complete(&checked, degree)?;
            for rule in &checked.rules {
                propagate_rule(
                    &entry,
                    ordering,
                    sector,
                    degree,
                    rule,
                    &self.zero_sectors,
                    self.sources.index_variables(),
                    &mut bounds,
                    &mut budget,
                )?;
            }
            let (report, checked_sector) = CheckedSector::retain(checked);
            reports.push(report);
            retained.insert(sector, checked_sector);
        }
        Ok(PreparedTotalExcess {
            report: SourcePortTotalExcessAudit {
                entry,
                ordering,
                bounds,
                sectors: reports,
            },
            sectors: retained,
        })
    }
}

fn require_complete<const N: usize>(
    checked: &SectorCheck<N>,
    degree: u64,
) -> Result<(), SourcePortAuditError> {
    let report = &checked.report;
    if report.max_total_excess_degree != Some(degree)
        || !report.issues.is_empty()
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
            "sector {:?} does not cover successor total excess {degree}: {report:?}",
            report.sector,
        )));
    }
    Ok(())
}

fn propagate_rule<const N: usize>(
    entry: &EntryScope,
    ordering: OrderingPolicy,
    sector: [bool; N],
    degree: u64,
    rule: &CheckedRule<N>,
    zero_sectors: &[[bool; N]],
    indices: &[usize; N],
    bounds: &mut BTreeMap<[bool; N], u64>,
    budget: &mut EnvelopeBudget,
) -> Result<(), SourcePortAuditError> {
    for term in &rule.rhs {
        visit_successor_degrees(
            entry,
            ordering,
            &sector,
            degree,
            &rule.application,
            &term.shift,
            &term.coefficient,
            rule.affine.as_deref(),
            &rule.affine_exclusions,
            |child| zero_sectors.iter().any(|zero| zero.as_slice() == child),
            indices,
            budget,
            |child, propagated| {
                let child: [bool; N] = child
                    .try_into()
                    .map_err(|_| error("successor arity differs"))?;
                let target = bounds
                    .get_mut(&child)
                    .ok_or_else(|| error("successor sector has no checked census entry"))?;
                *target = (*target).max(propagated);
                Ok(())
            },
        )?;
    }
    Ok(())
}

/// Shared conservative successor obligation for retained rules and actual
/// replayed cells. The caller supplies authenticated source predicates and
/// checked strict descent; geometry alone never grants that authority.
#[allow(clippy::too_many_arguments)]
pub(in crate::foundry::artifact) fn visit_successor_degrees(
    entry: &EntryScope,
    ordering: OrderingPolicy,
    sector: &[bool],
    degree: u64,
    application: &[LatticeBox],
    shift: &[i64],
    coefficient: &Coefficient,
    affine: Option<&AffineApplicationDomain>,
    affine_exclusions: &[Arc<AffineApplicationDomain>],
    mut is_zero: impl FnMut(&[bool]) -> bool,
    indices: &[usize],
    budget: &mut EnvelopeBudget,
    mut require_destination: impl FnMut(&[bool], u64) -> Result<(), SourcePortAuditError>,
) -> Result<(), SourcePortAuditError> {
    let arity = sector.len();
    if !ordering.is_spired() || shift.len() != arity || indices.len() != arity {
        return Err(error(
            "successor obligation requires matching arity and Spired ordering",
        ));
    }
    entry.validate_sector(sector).map_err(error)?;
    if coefficient.is_zero() {
        return Ok(());
    }
    // Keep the dynamic adapter's three buffers for the entire traversal. The
    // partition ledger already charges per-piece mask/comparator work.
    budget.charge(0, mul(arity, 3)?, mul(arity, 3)?)?;
    let mut child = Vec::new();
    let mut child_corner = Vec::new();
    let mut source_corner = Vec::new();
    child
        .try_reserve_exact(arity)
        .map_err(|_| SourcePortAuditError::ResourceBudgetExhausted {
            resource: "total-excess successor mask allocation",
        })?;
    child_corner.try_reserve_exact(arity).map_err(|_| {
        SourcePortAuditError::ResourceBudgetExhausted {
            resource: "total-excess successor corner allocation",
        }
    })?;
    source_corner.try_reserve_exact(arity).map_err(|_| {
        SourcePortAuditError::ResourceBudgetExhausted {
            resource: "total-excess successor corner allocation",
        }
    })?;
    child.resize(arity, false);
    child_corner.resize(arity, 0_i64);
    source_corner.extend(sector.iter().copied().map(i64::from));
    for source in application {
        budget.charge(0, 0, arity)?;
        if !EntryDegreeBound::MaxTotalExcessDegree(degree)
            .intersects_local_box(sector, source)
            .map_err(error)?
        {
            continue;
        }
        for piece in budget.partition(source, sector, shift)? {
            if !EntryDegreeBound::MaxTotalExcessDegree(degree)
                .intersects_local_box(sector, &piece)
                .map_err(error)?
            {
                continue;
            }
            // These exact domain proofs can remove fictitious activating
            // branches of rectangular prefilters, without discarding any
            // unresolved affine predicate or excluding a sampled point.
            if affine.is_some_and(|domain| domain.is_proved_empty_in_box(&piece))
                || affine_exclusions
                    .iter()
                    .any(|domain| domain.is_proved_to_contain_box(&piece))
            {
                continue;
            }
            for axis in 0..arity {
                let local = i128::from(piece.lower()[axis]);
                let physical = if sector[axis] { 1 + local } else { -local };
                child[axis] = physical + i128::from(shift[axis]) > 0;
                child_corner[axis] = i64::from(child[axis]);
            }
            if is_zero(&child) {
                continue;
            }
            if child.as_slice() == sector {
                // On an unchanged sector, total excess is the first
                // varying Spired complexity component. Already checked
                // strict descent therefore implies E(child) <= E(parent).
                continue;
            }
            let lower = ordering
                .compare(&child_corner, &source_corner)
                .map_err(error)?
                == Ordering::Less;
            let admitted = entry.validate_sector(&child).is_ok();
            if !lower || !admitted {
                // Whole-domain descent can rely on a coefficient that
                // vanishes exactly on this sign cell. Repeat its native
                // restriction before treating a prefilter edge as real.
                if geometry::checked_coefficient_vanishes(
                    coefficient,
                    &piece,
                    sector,
                    indices,
                    budget.limits,
                    affine,
                )? {
                    continue;
                }
                return Err(error(format!(
                    "unresolved nonlower or out-of-root successor sector {sector:?} -> {child:?}, shift={:?}",
                    shift,
                )));
            }
            let propagated = successor_degree(degree, sector, &child, shift)?;
            require_destination(&child, propagated)?;
        }
    }
    Ok(())
}

/// Since E(n) + p(sector(n)) = sum(abs(n_i)), the triangle inequality
/// bounds any fixed physical displacement. All sums widen before narrowing.
fn successor_degree(
    degree: u64,
    parent: &[bool],
    child: &[bool],
    shift: &[i64],
) -> Result<u64, SourcePortAuditError> {
    let absolute = shift.iter().try_fold(u128::from(degree), |value, shift| {
        value
            .checked_add(u128::from(shift.unsigned_abs()))
            .ok_or_else(overflow)
    })?;
    let value = absolute
        .checked_add(parent.iter().filter(|&&active| active).count() as u128)
        .and_then(|value| value.checked_sub(child.iter().filter(|&&active| active).count() as u128))
        .ok_or_else(overflow)?;
    u64::try_from(value).map_err(|_| overflow())
}

/// One cumulative structural ledger, shared by every sector, box and term.
/// Native coefficient restriction additionally retains its existing per-query
/// proof limits; this ledger does not bound Symbolica's internal allocations.
pub(in crate::foundry::artifact) struct EnvelopeBudget {
    limits: CompletionGeometryLimits,
    boxes: usize,
    coordinates: usize,
    work: usize,
}

impl EnvelopeBudget {
    pub(in crate::foundry::artifact) fn new(limits: CompletionGeometryLimits) -> Self {
        Self {
            limits,
            boxes: 0,
            coordinates: 0,
            work: 0,
        }
    }

    pub(in crate::foundry::artifact) fn charge(
        &mut self,
        boxes: usize,
        coordinates: usize,
        work: usize,
    ) -> Result<(), SourcePortAuditError> {
        let boxes = add(self.boxes, boxes)?;
        let coordinates = add(self.coordinates, coordinates)?;
        let work = add(self.work, work)?;
        if boxes > self.limits.max_requested_boxes
            || coordinates > self.limits.max_requested_box_coordinate_cells
            || work > self.limits.max_split_operations
        {
            return Err(SourcePortAuditError::ResourceBudgetExhausted {
                resource: "total-excess successor geometry",
            });
        }
        self.boxes = boxes;
        self.coordinates = coordinates;
        self.work = work;
        Ok(())
    }

    fn partition(
        &mut self,
        source: &LatticeBox,
        sector: &[bool],
        shift: &[i64],
    ) -> Result<Vec<LatticeBox>, SourcePortAuditError> {
        let count =
            super::scope::successor::partition_count(source, sector, shift).map_err(error)?;
        let coordinates = mul(mul(count, sector.len())?, 2)?;
        if count > self.limits.max_uncovered_boxes
            || coordinates > self.limits.max_uncovered_box_coordinate_cells
        {
            return Err(SourcePortAuditError::ResourceBudgetExhausted {
                resource: "total-excess sign partition",
            });
        }
        // At most 2P-1 temporary/result boxes; additionally account masks,
        // lower-corner degree work and comparator keys for every final piece.
        self.charge(
            mul(count, 2)?,
            mul(coordinates, 4)?,
            add(mul(mul(count, sector.len())?, 8)?, count - 1)?,
        )?;
        geometry::sign_partition_with_limits(
            source,
            sector,
            shift,
            CompletionGeometryLimits {
                max_uncovered_boxes: count,
                max_uncovered_box_coordinate_cells: coordinates,
                max_split_operations: count - 1,
                ..self.limits
            },
        )
    }
}

fn add(left: usize, right: usize) -> Result<usize, SourcePortAuditError> {
    left.checked_add(right).ok_or_else(overflow)
}

fn mul(left: usize, right: usize) -> Result<usize, SourcePortAuditError> {
    left.checked_mul(right).ok_or_else(overflow)
}

fn overflow() -> SourcePortAuditError {
    error("total-excess successor count or degree overflow")
}

#[cfg(test)]
mod tests;
