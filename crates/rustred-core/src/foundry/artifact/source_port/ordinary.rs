//! Independent desired-identity membership in regenerated ordinary IBPs.
//!
//! A rational certificate can have avoidable poles. Such poles are reported
//! as properties of this certificate, not evidence that the rule is invalid.

use crate::algebra::Coefficient;
use crate::foundry::completion::LatticeBox;
use crate::solver::{
    ExactRow, IntegralOrder, SectorRule, SourceSystem, Term, instantiate_source_port,
    translate_source_port,
};

use super::{SourcePortAuditError, certificate, error, geometry};

mod native;

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
            return Ok(restricted.numerator.is_zero());
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
    let weights = if let Some(weights) = native::propose(&rows, &desired, order, strict)? {
        weights
    } else {
        // A discovery quotient only: symbolic powers keep their parent signs.
        // Weighted cancellation can make a discarded activating column zero
        // even when none of its individual source terms is uniformly zero.
        // The complete unprojected original identity is checked below.
        native::propose(&rows, &desired, order, |term| {
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
        })?
        .ok_or_else(|| {
            error("stored identity has no original-source proposal in either quotient")
        })?
    };
    native::verify(&rows, &desired, &weights, order, |term| {
        if let Some(affine) = affine {
            let restricted = affine
                .restrict_coefficient(&term.coefficient)
                .map_err(error)?;
            return Ok(restricted.numerator.is_zero());
        }
        geometry::uniformly_zero_term(
            rule,
            term,
            boxes,
            order.sector(),
            zero_sectors,
            system.index_variables(),
        )
    })?;
    certificate::OriginalSourceReplay::retain_checked(requests, weights)
}
