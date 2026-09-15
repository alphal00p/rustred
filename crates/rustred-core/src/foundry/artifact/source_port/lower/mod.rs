//! One cold, exact lowering into the existing rule/cell/artifact owners.
//! No source search, elimination transcript or concrete anchor is invented.

mod domain;
mod original_combination;
mod refined_replay;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedPolynomial};
use crate::foundry::cell::{
    FixedIndexRestriction, RuleCell, SourceViewBatch, SourceViewConstruction,
};
use crate::foundry::completion::LatticeBox;
use crate::foundry::parametric::ParametricGuardOrigin;
use crate::identity::{IndexShift, ParametricIbpGenerator, RowId};
use crate::sector::{OrderingPolicy, SectorMonotoneDomain};

use super::super::geometry;
use super::{CheckedRule, SourcePortAuditError, error};
use original_combination::{Guard, JoinedContribution, append_guard};

/// Arithmetic payload exposed only by consuming the private checked record.
/// This description itself is not accepted by any rule constructor.
pub(crate) struct OriginalDomainParts {
    pub sources: Arc<SourceViewBatch>,
    pub application: SectorMonotoneDomain,
    pub mathematical_application: LatticeBox,
    pub fixed: Vec<FixedIndexRestriction>,
    pub ordering: OrderingPolicy,
    pub rhs: Vec<(IndexShift, IndexedCoefficient)>,
    pub contributions: Vec<(usize, RowId, IndexedCoefficient)>,
    pub guards: Vec<(IndexedPolynomial, Vec<ParametricGuardOrigin>)>,
    pub source_rows_used: usize,
    pub shift_columns_checked: usize,
}

/// Owns the precise identity and source batch that passed full residual replay.
/// Not Clone, not constructible outside this module, and no empty seal exists.
pub(crate) struct ReplayedOriginalDomain(OriginalDomainParts);

impl ReplayedOriginalDomain {
    pub(crate) fn into_parts(self) -> OriginalDomainParts {
        self.0
    }
}

pub(super) fn lower_rule<const N: usize>(
    corpus: &super::super::normalization::OriginalSourceCorpus,
    generator: &ParametricIbpGenerator<'_>,
    sector: [bool; N],
    ordering: OrderingPolicy,
    zero_sectors: &[[bool; N]],
    inherited: &[crate::algebra::CoefficientPolynomial],
    checked: CheckedRule<N>,
) -> Result<Vec<Arc<RuleCell>>, SourcePortAuditError> {
    let context = corpus.context();
    let indices = std::array::from_fn(|axis| context.base().parameter_names().len() + axis);
    let fixed: Vec<_> = checked
        .fixed
        .iter()
        .enumerate()
        .filter_map(|(position, value)| {
            value.map(|value| FixedIndexRestriction::new(position, value))
        })
        .collect();
    let fixed_pairs: Vec<_> = fixed
        .iter()
        .map(|value| (value.position(), value.value()))
        .collect();
    let translated = corpus.translate_normalized(
        generator,
        &checked.ordinary,
        &fixed,
        Default::default(),
        Default::default(),
    )?;
    if !matches!(
        translated.sources.construction(),
        SourceViewConstruction::Direct
    ) {
        return Err(error(
            "combined replay requires unchanged original source views",
        ));
    }
    let mut joined = Vec::with_capacity(translated.contributions.len());
    for (ordinal, row_id, weight) in &translated.contributions {
        let source = translated
            .sources
            .relations()
            .get(*ordinal)
            .ok_or_else(|| error("normalized contribution ordinal is absent"))?;
        if source.row_id() != row_id {
            return Err(error(
                "normalized contribution identifies another translated row",
            ));
        }
        let provenance = translated.sources.provenance()[*ordinal].translated();
        joined.push(JoinedContribution {
            source_ordinal: *ordinal,
            original_row: provenance.source_row().clone(),
            offset: provenance.offset().values().to_vec(),
            weight: weight.clone(),
        });
    }
    let mut original =
        original_combination::compile(context, &translated.sources, &joined, &fixed_pairs)?;
    // Do not erase obligations attached to contributions removed by exact
    // normalization/joining. Their provenance is a retained program condition,
    // never a fictitious row or elimination pivot.
    let mut retained_conditions = translated.weight_conditions;
    for condition in inherited.iter().chain(&checked.nonzero_conditions) {
        let polynomial = context
            .admit_native_polynomial_result_with_limits(condition.clone(), Default::default())
            .map_err(error)?;
        retained_conditions.push(
            context
                .specialize_fixed_polynomial_sealed(&polynomial, &fixed_pairs, Default::default())
                .map_err(error)?,
        );
    }
    for (condition_ordinal, condition) in retained_conditions.into_iter().enumerate() {
        append_guard(
            &mut original.guards,
            condition,
            ParametricGuardOrigin::OriginalDomainCondition { condition_ordinal },
        )?;
    }
    let mut rhs = Vec::with_capacity(checked.rhs.len());
    for term in checked.rhs {
        let shift = IndexShift::try_new(term.shift, N).map_err(error)?;
        let coefficient = context
            .admit_native_result_with_limits(term.coefficient, Default::default())
            .map_err(error)?;
        let (coefficient, denominator) = context
            .specialize_fixed_indices_sealed(&coefficient, &fixed_pairs, Default::default())
            .map_err(error)?;
        append_guard(
            &mut original.guards,
            denominator,
            ParametricGuardOrigin::RuleCoefficientDenominator {
                shift: shift.clone(),
            },
        )?;
        rhs.push((shift, coefficient));
    }
    let sources = Arc::new(translated.sources);
    let mut result = Vec::new();
    for application in checked.application {
        // A common refinement of all physical RHS walls allows one immutable
        // rule payload per cell; the original weighted sum is compiled once.
        let mut pieces = vec![application];
        for (shift, _) in &rhs {
            let wide: &[i64; N] = shift.values().try_into().map_err(error)?;
            let mut refined = Vec::new();
            for piece in pieces {
                refined.extend(geometry::sign_partition(&piece, &sector, wide)?);
                if refined.len() > 65_536 {
                    return Err(error("combined sign-cell refinement budget exceeded"));
                }
            }
            pieces = refined;
        }
        for piece in pieces {
            let mut retained = Vec::new();
            for (shift, coefficient) in &rhs {
                let wide: &[i64; N] = shift.values().try_into().map_err(error)?;
                if geometry::uniformly_zero_wide(
                    wide,
                    Some((coefficient.raw(), &indices)),
                    std::slice::from_ref(&piece),
                    &sector,
                    zero_sectors,
                )? {
                    continue;
                }
                retained.push((shift.clone(), coefficient.clone()));
            }
            if retained.is_empty() {
                return Err(error(
                    "combined source rule reduces a nonzero-sector target to zero; zero-rule lowering is not admitted",
                ));
            }
            for Guard { polynomial, .. } in &original.guards {
                domain::validate_guard(context, polynomial, &piece, &sector)?;
            }
            let shift_columns_checked = refined_replay::verify::<N>(
                context,
                &original,
                &retained,
                |shift, coefficient| {
                    geometry::uniformly_zero_wide(
                        shift,
                        Some((coefficient.raw(), &indices)),
                        std::slice::from_ref(&piece),
                        &sector,
                        zero_sectors,
                    )
                },
            )?;
            let application = domain::runtime_domain(&piece, &sector, &retained)?;
            // Uniform mathematical descent was already checked on the full
            // unbounded parent. Reuse the same existing order for executable
            // carrier witnesses; no representability bound becomes coverage.
            for (shift, _) in &retained {
                ordering
                    .prove_sector_monotone_shift_descent(&application, &[0; N], shift.values())
                    .map_err(error)?;
            }
            let verified = ReplayedOriginalDomain(OriginalDomainParts {
                sources: sources.clone(),
                application,
                mathematical_application: piece,
                fixed: fixed.clone(),
                ordering,
                rhs: retained,
                contributions: translated.contributions.clone(),
                guards: original
                    .guards
                    .iter()
                    .map(|guard| (guard.polynomial.clone(), guard.origins.clone()))
                    .collect(),
                source_rows_used: original.source_rows_multiplied,
                shift_columns_checked,
            });
            result.push(Arc::new(
                RuleCell::from_replayed_original_domain(context, verified).map_err(error)?,
            ));
        }
    }
    Ok(result)
}
