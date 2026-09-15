//! One exact original-domain lowering shared by generation and cold decoding.
mod domain;
#[cfg(test)]
pub(in crate::foundry::artifact) mod durable_tests;
mod original_combination;
mod refined_replay;
#[cfg(test)]
mod tests;
mod verification;

use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedPolynomial};
use crate::foundry::cell::{FixedIndexRestriction, RuleCell, SourceViewBatch};
use crate::foundry::completion::LatticeBox;
use crate::foundry::parametric::{AffineApplicationDomain, ParametricGuardOrigin};
use crate::identity::{IndexShift, ParametricIbpGenerator, RowId};
use crate::sector::{OrderingPolicy, SectorMonotoneDomain};

use super::super::geometry;
use super::{CheckedRule, SourcePortAuditError, error};
pub(in crate::foundry::artifact) use verification::PreparedOriginalDomain;
pub(crate) use verification::ReplayLimits;

/// Consumed only from the private full-domain replay record.
pub(crate) struct OriginalDomainParts {
    pub sources: Arc<SourceViewBatch>,
    pub application: SectorMonotoneDomain,
    pub mathematical_application: LatticeBox,
    pub fixed: Vec<FixedIndexRestriction>,
    pub ordering: OrderingPolicy,
    pub rhs: Vec<(IndexShift, IndexedCoefficient)>,
    pub contributions: Vec<(usize, RowId, IndexedCoefficient)>,
    pub guards: Vec<(IndexedPolynomial, Vec<ParametricGuardOrigin>)>,
    pub affine: Option<Arc<AffineApplicationDomain>>,
    pub affine_exclusions: Arc<[Arc<AffineApplicationDomain>]>,
    pub source_rows_used: usize,
    pub shift_columns_checked: usize,
    pub limits: ReplayLimits,
}

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
    let limits = ReplayLimits::default();
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
        .map(|item| (item.position(), item.value()))
        .collect();
    let affine = checked.affine.clone();
    let translated = corpus.translate_normalized(
        generator,
        &checked.ordinary,
        &fixed,
        Default::default(),
        limits.cell,
    )?;
    // Conditions of canceled contributions remain mandatory. All original
    // RHS denominators are also retained before any physical zero omission.
    let mut conditions = translated.weight_conditions;
    for condition in inherited.iter().chain(&checked.nonzero_conditions) {
        conditions.push(
            context
                .admit_native_polynomial_result_with_limits(
                    condition.clone(),
                    limits.cell.indexed_algebra.exact_algebra,
                )
                .map_err(error)?,
        );
    }
    let mut rhs = Vec::with_capacity(checked.rhs.len());
    for term in checked.rhs {
        let shift = IndexShift::try_new(term.shift, N).map_err(error)?;
        let coefficient = context
            .admit_native_result_with_limits(
                term.coefficient,
                limits.cell.indexed_algebra.exact_algebra,
            )
            .map_err(error)?;
        let (coefficient, denominator) = context
            .specialize_fixed_indices_sealed(
                &coefficient,
                &fixed_pairs,
                limits.cell.indexed_algebra,
            )
            .map_err(error)?;
        conditions.push(denominator);
        rhs.push((shift, coefficient));
    }
    let parent = PreparedOriginalDomain::try_new(
        context,
        Arc::new(translated.sources),
        translated.contributions,
        fixed,
        affine,
        checked.affine_exclusions.clone().into(),
        conditions,
        limits,
    )?;
    let mut result = Vec::new();
    for application in checked.application {
        let mut pieces = vec![application];
        for (shift, _) in &rhs {
            let mut refined = Vec::new();
            for piece in pieces {
                refined.extend(geometry::sign_partition(&piece, &sector, shift.values())?);
                if refined.len() > limits.geometry.max_requested_boxes {
                    return Err(error("combined sign-cell refinement budget exceeded"));
                }
            }
            pieces = refined;
        }
        for piece in pieces {
            let mut retained = Vec::new();
            for (shift, coefficient) in &rhs {
                if geometry::uniformly_zero_wide_with_limits(
                    shift.values(),
                    Some(coefficient),
                    std::slice::from_ref(&piece),
                    &sector,
                    zero_sectors,
                    limits.geometry,
                    |coefficient, piece| {
                        geometry::bounded::coefficient_vanishes(
                            context,
                            coefficient,
                            piece,
                            &sector,
                            limits.cell.indexed_algebra,
                            limits.geometry,
                        )
                    },
                )? {
                    continue;
                }
                retained.push((shift.clone(), coefficient.clone()));
            }
            if retained.is_empty() {
                return Err(error(
                    "combined lowering does not admit a nonzero-sector rule with zero RHS",
                ));
            }
            // Complete weighted original identity, all original poles and
            // true-unbounded descent are rechecked by the same cold entry.
            result.push(parent.verify_cell(
                context,
                ordering,
                &sector,
                zero_sectors,
                piece,
                retained,
            )?);
        }
    }
    Ok(result)
}
