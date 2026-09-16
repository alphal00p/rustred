//! Compose a regenerated preconditioner derivation after an exact frame hit.
//!
//! Output is merely a proposal. The caller retains the original full weighted
//! replay, exact pole-domain admission, normalization and publication gates.

use crate::algebra::Coefficient;
use crate::solver::{
    SectorRule, Seed, SourceSystem, instantiate_polynomial_source_port, translate_source_port,
};

use super::{BasisReplay, SourcePortAuditError, error};

pub(super) fn compose<const N: usize>(
    system: &SourceSystem<N>,
    rule: &SectorRule<N>,
    seeds: &[Seed<N>],
    shifts: [i16; N],
    replay: &BasisReplay<'_>,
) -> Result<Vec<Coefficient>, SourcePortAuditError> {
    let affine = rule.candidate.case.affine();
    if affine.is_some_and(|case| !case.is_tangent(&shifts)) {
        return Err(error(
            "preconditioner recentering is not tangent to its affine case",
        ));
    }
    let template: Coefficient = system
        .rows()
        .iter()
        .flatten()
        .next()
        .ok_or_else(|| error("preconditioner source corpus is empty"))?
        .coefficient
        .one()
        .into();
    let same_map = |coefficient: &Coefficient| {
        coefficient.numerator.variables() == template.get_variables()
            && coefficient.denominator.variables() == template.get_variables()
    };
    if !same_map(replay.pivot) || replay.pivot.is_zero() {
        return Err(error("preconditioner pivot is zero or has a foreign map"));
    }
    let pivot = translate_source_port(replay.pivot, system.index_variables(), &shifts);
    let mut groups = vec![Vec::new(); seeds.len()];
    for (selected_row, weight) in replay.weights {
        let source = rule
            .candidate
            .sources
            .get(*selected_row)
            .ok_or_else(|| error("selected preconditioner source is out of range"))?;
        if !same_map(weight) {
            return Err(error("preconditioner selected weight has a foreign map"));
        }
        let group = seeds
            .iter()
            .position(|seed| seed == &source.seed)
            .ok_or_else(|| {
                error("selected preconditioner seed is absent from ordinary requests")
            })?;
        let weight = &translate_source_port(weight, system.index_variables(), &shifts) / &pivot;
        groups[group].push((source.basis_row, weight));
    }
    let length = seeds
        .len()
        .checked_mul(system.rows().len())
        .ok_or_else(|| error("preconditioner request count overflows"))?;
    let zero: Coefficient = template.numerator.zero().into();
    let mut output = vec![zero; length];
    for (group, (seed, weights)) in seeds.iter().zip(groups).enumerate() {
        if weights.is_empty() {
            continue;
        }
        let mut translated = *seed;
        for (axis, shift) in shifts.iter().enumerate() {
            if !seed.integral[axis].is_symbolic() && *shift != 0 {
                return Err(error(
                    "preconditioner recentering changes an absolute numeric coordinate",
                ));
            }
            translated.shifts[axis] = translated.shifts[axis]
                .checked_add(*shift)
                .ok_or_else(|| error("preconditioner seed translation overflows"))?;
        }
        translated.integral = translated.integral.shifted(shifts).map_err(error)?;
        let original = replay
            .derivation
            .compose(&weights, &template, |scale| {
                instantiate_polynomial_source_port(
                    scale,
                    &translated,
                    system.index_variables(),
                    system.fixed(),
                    affine,
                )
            })
            .map_err(error)?;
        if original.len() != system.rows().len() {
            return Err(error("preconditioner derivation/source counts differ"));
        }
        let start = group * system.rows().len();
        output[start..start + original.len()].clone_from_slice(&original);
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
