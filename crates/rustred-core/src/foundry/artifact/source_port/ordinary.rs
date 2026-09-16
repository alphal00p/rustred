//! Independent desired-identity membership in regenerated ordinary IBPs.
//!
//! A rational certificate can have avoidable poles. Such poles are reported
//! as properties of this certificate, not evidence that the rule is invalid.

use crate::algebra::Coefficient;
use crate::foundry::completion::LatticeBox;
use crate::solver::{
    ExactRow, IntegralOrder, PreconditionProvenance, SectorRule, SourceSystem, Term,
    instantiate_source_port, translate_source_port,
};

use super::{SourcePortAuditError, certificate, error, geometry};

mod guards;
mod native;
mod provenance;

/// Recovered exact selected-frame weights before canonical normalization.
pub(super) struct BasisReplay<'a> {
    pub derivation: &'a PreconditionProvenance,
    pub weights: &'a [(usize, Coefficient)],
    pub pivot: &'a Coefficient,
}

#[cfg(test)]
mod tests;

pub(super) fn weights<const N: usize>(
    system: &SourceSystem<N>,
    original_row_ids: &[crate::identity::RowId],
    order: &IntegralOrder<N>,
    zero_sectors: &[[bool; N]],
    rule: &SectorRule<N>,
    shifts: [i16; N],
    boxes: &[LatticeBox],
    basis_replay: Option<&BasisReplay<'_>>,
) -> Result<certificate::OriginalSourceReplay<N>, SourcePortAuditError> {
    if original_row_ids.len() != system.rows().len() {
        return Err(error("ordinary replay row-ID count mismatch"));
    }
    let affine = rule.candidate.case.affine();
    if let Some(affine) = affine {
        if affine.index_variables() != system.index_variables() || !affine.is_tangent(&shifts) {
            return Err(error(
                "ordinary replay recentering does not preserve its affine case",
            ));
        }
    }
    let template = &system
        .rows()
        .iter()
        .flatten()
        .next()
        .ok_or_else(|| error("ordinary corpus has no polynomial template"))?
        .coefficient;
    let one: Coefficient = template.one().into();
    let mut seeds = Vec::new();
    for source in &rule.candidate.sources {
        if !seeds.contains(&source.seed) {
            seeds.push(source.seed);
        }
    }
    let mut rows: Vec<ExactRow<N>> = Vec::new();
    let mut requests = Vec::new();
    for seed in &seeds {
        let offset = certificate::source_offset(rule, seed, &shifts);
        for (ordinal, source) in system.rows().iter().enumerate() {
            if let Some(affine) = affine {
                // Recenter the original seed BEFORE restricting coefficients.
                // The chart acts only on coefficients; coupled integral-key
                // axes and every original RowId/offset remain independent.
                let mut translated = *seed;
                translated.integral = translated.integral.shifted(shifts).map_err(error)?;
                for (shift, displacement) in translated.shifts.iter_mut().zip(shifts) {
                    *shift = shift
                        .checked_add(displacement)
                        .ok_or_else(|| error("ordinary source translation overflow"))?;
                }
                rows.push(
                    instantiate_source_port(
                        source,
                        &translated,
                        system.index_variables(),
                        system.fixed(),
                        order,
                        &[],
                        Some(affine),
                    )
                    .map_err(error)?,
                );
                requests.push((original_row_ids[ordinal].clone(), offset));
                continue;
            }
            // Do not inherit the search's assumed-sector zero pruning.
            let original = instantiate_source_port(
                source,
                seed,
                system.index_variables(),
                system.fixed(),
                order,
                &[],
                None,
            )
            .map_err(error)?;
            let mut row = Vec::with_capacity(original.len());
            for term in original {
                let translated = Term {
                    integral: term.integral.shifted(shifts).map_err(error)?,
                    coefficient: translate_source_port(
                        &term.coefficient,
                        system.index_variables(),
                        &shifts,
                    ),
                };
                row.push(translated);
            }
            row.sort_unstable_by(|left, right| order.compare(&left.integral, &right.integral));
            rows.push(row);
            requests.push((original_row_ids[ordinal].clone(), offset));
        }
    }
    let mut desired = vec![Term {
        integral: rule.candidate.target,
        coefficient: one.clone(),
    }];
    for term in &rule.candidate.rhs {
        let coefficient = if let Some(affine) = affine {
            affine
                .restrict_coefficient(&term.coefficient)
                .map_err(error)?
        } else {
            term.coefficient.clone()
        };
        desired.push(Term {
            integral: term.integral,
            coefficient: -coefficient,
        });
    }
    let strict = |term: &Term<N, Coefficient>| {
        if term.integral == rule.candidate.target {
            return Ok(false);
        }
        if let Some(affine) = affine {
            // A coupled target case may make a physical source column vanish
            // only after restriction to its exact affine chart.  The chart
            // and polynomial reduction are owned by the authenticated
            // `AffineCase`; never replace this with a rectangular hull or a
            // sampled point test.
            let restricted = affine
                .restrict_coefficient(&term.coefficient)
                .map_err(error)?;
            let restricted = Term {
                integral: term.integral,
                coefficient: restricted,
            };
            return geometry::uniformly_zero_term(
                rule,
                &restricted,
                boxes,
                order.sector(),
                zero_sectors,
                system.index_variables(),
            );
        }
        geometry::uniformly_zero_term(
            rule,
            term,
            boxes,
            order.sector(),
            zero_sectors,
            system.index_variables(),
        )
    };
    // Normalization multiplies a checked polynomial scale. Inherited source
    // conditions are retained separately; do not change support when any of
    // those conditions can introduce a new index-dependent exceptional face.
    let enable_compact =
        guards::conditions_are_index_free(system.conditions(), system.index_variables());
    let accept_compact = |weights: &[Coefficient]| {
        guards::preserves_domain(
            weights,
            rule,
            boxes,
            system.index_variables(),
            order.sector(),
        )
        .unwrap_or(false)
    };
    // A regenerated forward derivation can avoid a second membership solve.
    // It remains only a proposal until the same full original identity and
    // no-new-index-poles gates succeed. Any miss keeps the existing fallback.
    if enable_compact
        && let Some(basis_replay) = basis_replay
        && let Ok(weights) = provenance::compose(system, rule, &seeds, shifts, basis_replay)
        && accept_compact(&weights)
        && native::verify(&rows, &desired, &weights, order, &strict).is_ok()
    {
        return certificate::OriginalSourceReplay::retain_checked(requests, weights);
    }
    let weights = if let Some(weights) = native::propose_verified(
        &rows,
        &desired,
        order,
        &strict,
        &strict,
        &accept_compact,
        enable_compact,
    )? {
        weights
    } else {
        // A discovery quotient only: symbolic powers keep their parent signs.
        // Weighted cancellation can make a discarded activating column zero
        // even when none of its individual source terms is uniformly zero.
        // The complete unprojected original identity is checked below.
        native::propose_verified(
            &rows,
            &desired,
            order,
            |term| {
                let assumed = std::array::from_fn(|axis| {
                    if term.integral[axis].is_symbolic() {
                        order.sector()[axis]
                    } else {
                        term.integral[axis].value() > 0
                    }
                });
                Ok(term.integral != rule.candidate.target
                    && assumed != *order.sector()
                    && zero_sectors.contains(&assumed))
            },
            &strict,
            accept_compact,
            enable_compact,
        )?
        .ok_or_else(|| {
            error("stored identity has no original-source proposal in either quotient")
        })?
    };
    certificate::OriginalSourceReplay::retain_checked(requests, weights)
}
