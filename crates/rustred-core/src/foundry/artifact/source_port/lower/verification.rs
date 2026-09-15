//! One paired original-source parent and the common final cold proof.
use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::foundry::cell::{
    FixedIndexRestriction, RuleCell, RuleCellLimits, SourceViewBatch, SourceViewConstruction,
};
use crate::foundry::completion::{CompletionGeometryLimits, LatticeBox};
use crate::foundry::parametric::{ParametricGuardOrigin, ParametricRuleLimits};
use crate::identity::{IndexShift, RowId};
use crate::sector::OrderingPolicy;

use super::original_combination::{
    JoinedContribution, OriginalCombination, append_guard, validate_guard_limits,
};
use super::{
    OriginalDomainParts, ReplayedOriginalDomain, SourcePortAuditError, domain, error, geometry,
    refined_replay,
};

/// Explicit producer policy; never persisted as mathematical evidence.
#[derive(Clone, Copy, Default)]
pub(crate) struct ReplayLimits {
    pub rule: ParametricRuleLimits,
    pub cell: RuleCellLimits,
    pub geometry: CompletionGeometryLimits,
}

/// Arithmetic parent, not an installation seal. Its unchanged sources,
/// normalized weights, full native sum and fixed face cannot be separated or
/// replaced when a child cell is checked.
pub(in crate::foundry::artifact) struct PreparedOriginalDomain {
    sources: Arc<SourceViewBatch>,
    original: OriginalCombination,
    contributions: Vec<(usize, RowId, IndexedCoefficient)>,
    fixed: Vec<FixedIndexRestriction>,
    limits: ReplayLimits,
}

impl PreparedOriginalDomain {
    pub(in crate::foundry::artifact) fn try_new(
        context: &IndexedCoefficientContext,
        sources: Arc<SourceViewBatch>,
        contributions: Vec<(usize, RowId, IndexedCoefficient)>,
        fixed: Vec<FixedIndexRestriction>,
        retained_conditions: Vec<IndexedPolynomial>,
        limits: ReplayLimits,
    ) -> Result<Self, SourcePortAuditError> {
        if !matches!(sources.construction(), SourceViewConstruction::Direct)
            || sources.context_fingerprint() != context.fingerprint()
            || sources.len() > limits.cell.max_source_views
            || contributions.is_empty()
            || contributions.len() > limits.rule.max_source_combination_terms
            || fixed.len() > limits.cell.max_fixed_restrictions
            || retained_conditions.len() > limits.cell.max_guards
        {
            return Err(error(
                "original parent has incompatible source/context/shape or exceeds its input budget",
            ));
        }
        let mut pairs: Vec<(usize, i64)> = Vec::with_capacity(fixed.len());
        for item in &fixed {
            if item.position() >= context.index_count()
                || pairs
                    .last()
                    .is_some_and(|(axis, _)| *axis >= item.position())
            {
                return Err(error("original parent fixed coordinates are not canonical"));
            }
            pairs.push((item.position(), item.value()));
        }
        let mut joined = Vec::with_capacity(contributions.len());
        for (ordinal, row_id, weight) in &contributions {
            let source = sources
                .relations()
                .get(*ordinal)
                .ok_or_else(|| error("original parent source ordinal is absent"))?;
            if source.row_id() != row_id {
                return Err(error(
                    "original parent weight identifies a different translated source",
                ));
            }
            let provenance = sources
                .provenance()
                .get(*ordinal)
                .ok_or_else(|| error("original parent translated provenance is absent"))?
                .translated();
            joined.push(JoinedContribution {
                source_ordinal: *ordinal,
                original_row: provenance.source_row().clone(),
                offset: provenance.offset().values().to_vec(),
                weight: weight.clone(),
            });
        }
        let (mut original, normalized) = super::original_combination::compile_with_limits(
            context,
            &sources,
            &joined,
            &pairs,
            limits.rule,
        )?;
        for (condition_ordinal, polynomial) in retained_conditions.into_iter().enumerate() {
            let restricted = context
                .specialize_fixed_polynomial_sealed(
                    &polynomial,
                    &pairs,
                    limits.cell.indexed_algebra,
                )
                .map_err(error)?;
            append_guard(
                &mut original.guards,
                restricted,
                ParametricGuardOrigin::OriginalDomainCondition { condition_ordinal },
            )?;
            if original.guards.len() > limits.cell.max_guards {
                return Err(error(
                    "original parent conditions exceed their guard budget",
                ));
            }
            validate_guard_limits(&original.guards, limits.rule)?;
        }
        // These are the exact fixed-face weights already multiplied above;
        // no second native specialization or independently supplied payload.
        Ok(Self {
            sources,
            original,
            contributions: normalized,
            fixed,
            limits,
        })
    }

    /// Check the target face on the true box, not only on its i64 carrier.
    fn validate_face(
        &self,
        context: &IndexedCoefficientContext,
        sector: &[bool],
        piece: &LatticeBox,
    ) -> Result<Vec<(usize, i64)>, SourcePortAuditError> {
        let arity = context.index_count();
        if sector.len() != arity || piece.arity() != arity || arity > self.limits.geometry.max_arity
        {
            return Err(error(
                "original-domain proof has incompatible sector/box arity",
            ));
        }
        let mut pairs = Vec::with_capacity(self.fixed.len());
        for item in &self.fixed {
            let axis = item.position();
            if (item.value() > 0) != sector[axis] {
                return Err(error(
                    "original-domain fixed target lies outside its sector",
                ));
            }
            let local = if sector[axis] {
                i128::from(item.value()) - 1
            } else {
                -i128::from(item.value())
            };
            let local = u64::try_from(local).map_err(error)?;
            if piece.lower()[axis] != local || piece.upper()[axis] != Some(local) {
                return Err(error(
                    "fixed target does not hold on the complete mathematical box",
                ));
            }
            pairs.push((axis, item.value()));
        }
        Ok(pairs)
    }

    /// Both generation and decoding consume this exact same proof path.
    /// The caller's zero census must already be native-authenticated for the
    /// same family; the final installer revalidates it and the whole cover.
    pub(in crate::foundry::artifact) fn verify_cell<Z: AsRef<[bool]>>(
        &self,
        context: &IndexedCoefficientContext,
        ordering: OrderingPolicy,
        sector: &[bool],
        zeros: &[Z],
        piece: LatticeBox,
        rhs: Vec<(IndexShift, IndexedCoefficient)>,
    ) -> Result<Arc<RuleCell>, SourcePortAuditError> {
        if context.fingerprint() != self.sources.context_fingerprint() {
            return Err(error(
                "original cell context differs from its prepared parent",
            ));
        }
        preflight_cell_storage(context.index_count(), rhs.len(), self.limits.rule)?;
        let fixed_pairs = self.validate_face(context, sector, &piece)?;
        if context.fingerprint() != self.sources.context_fingerprint()
            || rhs.is_empty()
            || rhs.len() > self.limits.cell.max_retained_terms
        {
            return Err(error(
                "original cell RHS/context is invalid or exceeds its budget",
            ));
        }
        let mut guards = self.original.guards.clone();
        let mut retained = Vec::with_capacity(rhs.len());
        for (shift, coefficient) in rhs {
            if shift.values().len() != context.index_count() {
                return Err(error("original cell RHS has incompatible index arity"));
            }
            let (coefficient, denominator) = context
                .specialize_fixed_indices_sealed(
                    &coefficient,
                    &fixed_pairs,
                    self.limits.cell.indexed_algebra,
                )
                .map_err(error)?;
            append_guard(
                &mut guards,
                denominator,
                ParametricGuardOrigin::RuleCoefficientDenominator {
                    shift: shift.clone(),
                },
            )?;
            retained.push((shift, coefficient));
        }
        if guards.len() > self.limits.cell.max_guards {
            return Err(error(
                "original cell regenerated conditions exceed their guard budget",
            ));
        }
        validate_guard_limits(&guards, self.limits.rule)?;
        for guard in &guards {
            domain::validate_guard_with_limits(
                context,
                &guard.polynomial,
                &piece,
                sector,
                self.limits.cell,
            )?;
        }
        let vanishes = |coefficient: &IndexedCoefficient, piece: &LatticeBox| {
            geometry::bounded::coefficient_vanishes(
                context,
                coefficient,
                piece,
                sector,
                self.limits.cell.indexed_algebra,
                self.limits.geometry,
            )
        };
        let checked = refined_replay::verify_wide(
            context,
            &self.original,
            &retained,
            self.limits.rule,
            |shift, coefficient| {
                geometry::uniformly_zero_wide_with_limits(
                    shift,
                    Some(coefficient),
                    std::slice::from_ref(&piece),
                    sector,
                    zeros,
                    self.limits.geometry,
                    vanishes,
                )
            },
        )?;
        // Genuine infinity survives this proof. The finite machine carrier
        // is computed only after strict descent has been proved above it.
        geometry::prove_wide_descent_with_limits(
            retained
                .iter()
                .map(|(shift, coefficient)| (shift.values(), coefficient)),
            std::slice::from_ref(&piece),
            sector,
            ordering,
            self.limits.geometry,
            vanishes,
        )?;
        let application = domain::runtime_domain(&piece, sector, &retained)?;
        let record = ReplayedOriginalDomain(OriginalDomainParts {
            sources: self.sources.clone(),
            application,
            mathematical_application: piece,
            fixed: self.fixed.clone(),
            ordering,
            rhs: retained,
            contributions: self.contributions.clone(),
            guards: guards
                .into_iter()
                .map(|guard| (guard.polynomial, guard.origins))
                .collect(),
            source_rows_used: self.original.source_rows_multiplied,
            shift_columns_checked: checked,
            limits: self.limits,
        });
        RuleCell::from_replayed_original_domain(context, record)
            .map(Arc::new)
            .map_err(error)
    }
}

fn preflight_cell_storage(
    arity: usize,
    terms: usize,
    limits: ParametricRuleLimits,
) -> Result<(), SourcePortAuditError> {
    let check = |resource: &str, requested: usize, limit: usize| {
        if requested > limit {
            Err(error(format!(
                "{resource} budget exceeded: requested {requested}, limit {limit}"
            )))
        } else {
            Ok(())
        }
    };
    check(
        "combined sector mask cells",
        arity,
        limits.max_sector_mask_cells,
    )?;
    // A rule term, admission dependency and executable cell term each retain
    // one independently constructed descent proof. Each proof owns two key
    // coordinate Arcs; its embedded same-sector witness shares those Arcs.
    let keys = terms
        .checked_mul(arity)
        .and_then(|value| value.checked_mul(6))
        .ok_or_else(|| error("combined ordering-key coordinate count overflow"))?;
    check(
        "combined ordering-key coordinate cells",
        keys,
        limits.max_ordering_key_coordinate_cells,
    )?;
    // At most one same-sector bounds buffer per proof, plus the application
    // and interior construction buffers. Domain clones only add Arc handles.
    let endpoints = terms
        .checked_mul(3)
        .and_then(|value| value.checked_add(4))
        .and_then(|value| value.checked_mul(arity))
        .and_then(|value| value.checked_mul(2))
        .ok_or_else(|| error("combined domain endpoint count overflow"))?;
    check(
        "combined domain bound endpoint cells",
        endpoints,
        limits.max_domain_bound_endpoint_cells,
    )
}
