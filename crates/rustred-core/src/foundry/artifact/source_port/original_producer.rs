//! Checked, explicitly weighted ordinary identities; no source search or install.
//!
//! This coordinate-domain producer deliberately stops before owner insertion.
//! Every returned cell retains the existing original-domain replay seal. No
//! caller-supplied relation, zero oracle, terminal or closure claim is accepted.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedPolynomial};
use crate::family::IntegralFamily;
use crate::foundry::cell::{FixedIndexRestriction, RuleCell, RuleCellLimits, SourceViewBatch};
use crate::foundry::completion::LatticeBox;
use crate::foundry::parametric::ParametricRuleLimits;
use crate::identity::{
    IndexShift, IntegralShift, ParametricIbpConfig, ParametricIbpGenerator, RowId,
    TranslatedSourceLimits, TranslatedSourceRequest,
};
use crate::sector::{Mask, OrderingPolicy};

use super::super::ArtifactCoverReplayLimits;
use super::{PreparedOriginalDomain, ReplayLimits, SourcePortAuditError, error, geometry};

/// One weight multiplying a FULL original ordinary row at an explicit offset.
/// Weights must already normalize the requested target coefficient to one.
/// Offsets are relative to the target indices, before fixed specialization.
#[derive(Clone, Debug)]
pub struct OriginalSourceContribution {
    pub source_row: RowId,
    pub offset: IntegralShift,
    pub weight: IndexedCoefficient,
}

/// One proposed coordinate-domain identity `I(n) = sum(rhs)`.
///
/// Bounds are exact nonnegative sector-local coordinates: active `n_i - 1`,
/// inactive `-n_i`; `None` means mathematical infinity. Fixed restrictions
/// must hold on the entire box. Affine domains/exclusions are intentionally
/// not exposed by this first producer; a rectangle must never stand in for one.
/// Source and weight poles are regenerated, not supplied through this request.
#[derive(Clone, Debug)]
pub struct OriginalSourceCombinationRequest {
    pub root_sector: Mask,
    pub sector: Mask,
    pub ordering: OrderingPolicy,
    pub lower: Vec<u64>,
    pub upper: Vec<Option<u64>>,
    pub fixed: Vec<FixedIndexRestriction>,
    pub contributions: Vec<OriginalSourceContribution>,
    pub rhs: Vec<(IndexShift, IndexedCoefficient)>,
    /// Additional pre-cancellation/normalization conditions, never a waiver
    /// of the source, weight or RHS denominator conditions regenerated below.
    pub retained_conditions: Vec<IndexedPolynomial>,
}

/// Existing native budgets, with no implicit search or elimination budget.
#[derive(Clone, Copy, Debug, Default)]
pub struct OriginalSourceCombinationLimits {
    pub source_generation: ParametricIbpConfig,
    pub translated_sources: TranslatedSourceLimits,
    pub rule: ParametricRuleLimits,
    pub cell: RuleCellLimits,
    pub geometry: ArtifactCoverReplayLimits,
}

/// Checked local identity and uniform descent, NOT an installed owner.
///
/// All returned sign cells exactly partition the requested coordinate box;
/// any failed cell rejects the whole request. This proves no other cases,
/// recursive coverage, terminal independence or complete owner coverage.
#[derive(Debug)]
pub struct CheckedOriginalSourceCombination {
    family_fingerprint: String,
    context_fingerprint: String,
    root_sector: Mask,
    sector: Mask,
    ordering: OrderingPolicy,
    requested: LatticeBox,
    cells: Vec<(LatticeBox, Arc<RuleCell>)>,
}

impl CheckedOriginalSourceCombination {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }
    pub fn root_sector(&self) -> &Mask {
        &self.root_sector
    }
    pub fn sector(&self) -> &Mask {
        &self.sector
    }
    pub fn ordering(&self) -> &OrderingPolicy {
        &self.ordering
    }
    pub fn requested_bounds(&self) -> (&[u64], &[Option<u64>]) {
        (self.requested.lower(), self.requested.upper())
    }
    pub fn cells(&self) -> impl ExactSizeIterator<Item = &RuleCell> {
        self.cells.iter().map(|(_, cell)| cell.as_ref())
    }
    /// Exact mathematical bounds, not the cell's finite machine carrier.
    pub fn cell_bounds(&self, ordinal: usize) -> Option<(&[u64], &[Option<u64>])> {
        self.cells
            .get(ordinal)
            .map(|(piece, _)| (piece.lower(), piece.upper()))
    }
}

/// Regenerate ordinary sources and check an explicit combination directly.
///
/// No preconditioning, canonical pivot rediscovery, rule search, source-port
/// normalization adapter, zero-sector deletion or artifact installation occurs.
/// Duplicate source requests are rejected before joining: cancellation cannot
/// hide a weight denominator. Original and RHS conditions survive exact zero
/// coefficient removal on a cell. Unsupported prefix-order certificates fail
/// closed; this producer does not extend the sector-monotone proof capability.
pub fn check_original_source_combination(
    family: &IntegralFamily,
    request: OriginalSourceCombinationRequest,
    limits: OriginalSourceCombinationLimits,
) -> Result<CheckedOriginalSourceCombination, SourcePortAuditError> {
    let n = family.denominator_count();
    let sector = request.sector.active_bits();
    let root = request.root_sector.active_bits();
    if n == 0
        || sector.len() != n
        || root.len() != n
        || request.lower.len() != n
        || request.upper.len() != n
        || sector
            .iter()
            .zip(root)
            .any(|(&active, &allowed)| active && !allowed)
    {
        return Err(error(
            "original combination family/root/sector/domain mismatch",
        ));
    }
    if !request.ordering.is_source_port_uncut() || !request.ordering.is_support_primary() {
        return Err(error(
            "original combination cells require a supported support-primary order",
        ));
    }
    check(n, limits.geometry.max_arity, "original combination arity")?;
    request
        .ordering
        .compare(&vec![0; n], &vec![0; n])
        .map_err(error)?;
    check_boxes(1, n, limits)?;
    check(
        request.fixed.len(),
        limits.cell.max_fixed_restrictions,
        "original fixed restrictions",
    )?;
    check(
        request.contributions.len(),
        limits.rule.max_source_combination_terms,
        "original contributions",
    )?;
    check(
        request.contributions.len(),
        limits.cell.max_source_views,
        "original source views",
    )?;
    check(
        request.rhs.len(),
        limits.cell.max_retained_terms,
        "original RHS terms",
    )?;
    check(
        request.retained_conditions.len(),
        limits.cell.max_guards,
        "original retained conditions",
    )?;
    if request.contributions.is_empty() || request.rhs.is_empty() {
        return Err(error(
            "original combination needs sources and a nonempty RHS",
        ));
    }
    let requested = LatticeBox::try_new(request.lower, request.upper).map_err(error)?;
    let fixed_pairs: Vec<_> = request
        .fixed
        .iter()
        .map(|f| (f.position(), f.value()))
        .collect();
    for (i, &(axis, value)) in fixed_pairs.iter().enumerate() {
        if axis >= n || (i > 0 && fixed_pairs[i - 1].0 >= axis) || (value > 0) != sector[axis] {
            return Err(error(
                "original fixed coordinates are incompatible or noncanonical",
            ));
        }
        let local = if sector[axis] {
            i128::from(value) - 1
        } else {
            -i128::from(value)
        };
        let local = u64::try_from(local).map_err(error)?;
        if requested.lower()[axis] != local || requested.upper()[axis] != Some(local) {
            return Err(error(
                "original fixed coordinate does not hold on the requested box",
            ));
        }
    }
    let generator = ParametricIbpGenerator::try_new_with_config(family, limits.source_generation)
        .map_err(error)?;
    let context = generator.context();
    let batch = generator.prepare_ordinary_ibp().map_err(error)?;
    check(
        batch.len(),
        limits.rule.max_source_rows,
        "regenerated ordinary rows",
    )?;
    let generated_rows = (0..batch.len()).map(|i| batch.generate(i)).collect();
    let completed = batch.complete(generated_rows).map_err(error)?;
    let row_ids: BTreeMap<_, _> = completed
        .relations()
        .iter()
        .enumerate()
        .map(|(ordinal, row)| (row.row_id().clone(), ordinal))
        .collect();
    let mut joined = BTreeMap::new();
    for contribution in request.contributions {
        context
            .validate_with_limits(
                &contribution.weight,
                limits.rule.indexed_algebra.exact_algebra,
            )
            .map_err(error)?;
        let ordinal = *row_ids.get(&contribution.source_row).ok_or_else(|| {
            error("original source RowId is absent from regenerated ordinary rows")
        })?;
        if contribution.offset.len() != n {
            return Err(error("original source offset has wrong arity"));
        }
        let key = TranslatedSourceRequest::new(ordinal, contribution.offset);
        if joined
            .insert(key, (contribution.source_row, contribution.weight))
            .is_some()
        {
            return Err(error("duplicate original RowId/offset contribution"));
        }
    }
    let selected = generator
        .translate_selected_completed_source_rows(
            &completed,
            joined.keys().cloned(),
            limits.translated_sources,
        )
        .map_err(error)?;
    if !selected.requests().iter().eq(joined.keys()) {
        return Err(error("regenerated source request chronology changed"));
    }
    let ordinals: Vec<_> = (0..selected.len()).collect();
    let sources = Arc::new(
        SourceViewBatch::try_select(selected.into_translated_batch(), &ordinals, limits.cell)
            .map_err(error)?,
    );
    let mut contributions = Vec::with_capacity(joined.len());
    for (ordinal, (source_request, (row_id, weight))) in joined.into_iter().enumerate() {
        let provenance = sources.provenance()[ordinal].translated();
        if provenance.source_row() != &row_id || provenance.offset() != source_request.offset() {
            return Err(error(
                "regenerated source RowId/offset differs from requested contribution",
            ));
        }
        contributions.push((
            ordinal,
            sources.relations()[ordinal].row_id().clone(),
            weight,
        ));
    }
    let mut conditions = request.retained_conditions;
    let mut rhs = Vec::with_capacity(request.rhs.len());
    let mut seen_rhs = std::collections::BTreeSet::new();
    for (shift, coefficient) in request.rhs {
        if shift.values().len() != n
            || shift.values().iter().all(|&v| v == 0)
            || !seen_rhs.insert(shift.clone())
        {
            return Err(error(
                "original RHS must have unique target-free shifts of family arity",
            ));
        }
        context
            .validate_with_limits(&coefficient, limits.cell.indexed_algebra.exact_algebra)
            .map_err(error)?;
        let (coefficient, denominator) = context
            .specialize_fixed_indices_sealed(
                &coefficient,
                &fixed_pairs,
                limits.cell.indexed_algebra,
            )
            .map_err(error)?;
        check(
            conditions
                .len()
                .checked_add(1)
                .ok_or_else(|| error("original guard count overflow"))?,
            limits.cell.max_guards,
            "original retained conditions",
        )?;
        conditions.push(denominator);
        rhs.push((shift, coefficient));
    }
    let parent = PreparedOriginalDomain::try_new(
        context,
        sources,
        contributions,
        request.fixed,
        None,
        Arc::from([]),
        conditions,
        ReplayLimits {
            rule: limits.rule,
            cell: limits.cell,
            geometry: limits.geometry.geometry(),
        },
    )?;
    let mut pieces = vec![
        LatticeBox::try_new(
            requested.lower().iter().copied(),
            requested.upper().iter().copied(),
        )
        .map_err(error)?,
    ];
    let mut partition_work = 0usize;
    for (shift, _) in &rhs {
        let mut next = Vec::new();
        for piece in pieces {
            let parts = geometry::sign_partition_with_limits(
                &piece,
                sector,
                shift.values(),
                limits.geometry.geometry(),
            )?;
            partition_work = partition_work
                .checked_add(
                    parts
                        .len()
                        .checked_mul(n)
                        .ok_or_else(|| error("original sign partition work overflow"))?,
                )
                .ok_or_else(|| error("original sign partition work overflow"))?;
            check(
                partition_work,
                limits.geometry.max_split_operations,
                "original sign partition work",
            )?;
            check_boxes(
                next.len()
                    .checked_add(parts.len())
                    .ok_or_else(|| error("original sign cell count overflow"))?,
                n,
                limits,
            )?;
            next.extend(parts);
        }
        pieces = next;
    }
    let mut cells = Vec::with_capacity(pieces.len());
    let zeros: &[Vec<bool>] = &[];
    for piece in pieces {
        let mut retained = Vec::new();
        for (shift, coefficient) in &rhs {
            if parent.coefficient_vanishes(context, coefficient, &piece, sector)? {
                continue; // Full original/RHS guards were retained before this proof.
            }
            validate_root_successor(root, &piece, shift.values())?;
            retained.push((shift.clone(), coefficient.clone()));
        }
        let cell = parent.verify_cell(
            context,
            request.ordering.clone(),
            sector,
            zeros,
            LatticeBox::try_new(piece.lower().iter().copied(), piece.upper().iter().copied())
                .map_err(error)?,
            retained,
        )?;
        cells.push((piece, cell));
    }
    Ok(CheckedOriginalSourceCombination {
        family_fingerprint: family.fingerprint().to_owned(),
        context_fingerprint: context.fingerprint().to_owned(),
        root_sector: request.root_sector,
        sector: request.sector,
        ordering: request.ordering,
        requested,
        cells,
    })
}

fn validate_root_successor(
    root: &[bool],
    piece: &LatticeBox,
    shift: &[i64],
) -> Result<(), SourcePortAuditError> {
    // A forbidden root axis is inactive in the source sector: n=-local.
    // Its largest child power is shift-local_lower, without machine clipping.
    if root
        .iter()
        .enumerate()
        .any(|(i, &allowed)| !allowed && i128::from(shift[i]) > i128::from(piece.lower()[i]))
    {
        return Err(error(
            "original RHS activates support outside the declared owner root",
        ));
    }
    Ok(())
}

fn check(
    requested: usize,
    limit: usize,
    resource: &'static str,
) -> Result<(), SourcePortAuditError> {
    if requested > limit {
        Err(SourcePortAuditError::ResourceBudgetExhausted { resource })
    } else {
        Ok(())
    }
}

fn check_boxes(
    count: usize,
    n: usize,
    limits: OriginalSourceCombinationLimits,
) -> Result<(), SourcePortAuditError> {
    check(
        count,
        limits.geometry.max_requested_boxes,
        "original sign cells",
    )?;
    let coordinates = count
        .checked_mul(n)
        .and_then(|v| v.checked_mul(2))
        .ok_or_else(|| error("original sign cell coordinate count overflow"))?;
    check(
        coordinates,
        limits.geometry.max_requested_box_coordinate_cells,
        "original sign cell coordinates",
    )
}

#[cfg(test)]
mod tests;
