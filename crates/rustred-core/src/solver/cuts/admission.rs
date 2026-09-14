use crate::family::{ContractionMomentum, IntegralFamily, ScalarProductCoordinate};

use super::{LinearCutError, unsupported};

pub(super) fn contractions<const N: usize>(
    family: &IntegralFamily,
    removed: &[bool; N],
) -> Result<Vec<(usize, usize)>, LinearCutError> {
    let mut result = Vec::new();
    for (axis, cut) in removed.iter().enumerate() {
        if !cut {
            continue;
        }
        if family.power_shifts().iter().any(|shift| !shift.is_zero()) {
            // Pending a reference-policy decision: C++ deltaReplRule omits
            // noninteger offsets. Never silently inherit that discrepancy.
            return Err(unsupported(
                axis,
                "noninteger offsets with cut preparation are not yet supported",
            ));
        }
        let denominator = &family.denominators()[axis];
        let mut terms = denominator
            .coefficients()
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.is_zero());
        let Some((coordinate, _)) = terms.next() else {
            return Err(unsupported(axis, "cut has no scalar-product term"));
        };
        if !denominator.constant().is_zero() || terms.next().is_some() {
            return Err(unsupported(
                axis,
                "cut must be a single unshifted loop-external product",
            ));
        }
        let ScalarProductCoordinate::LoopExternal {
            loop_index,
            external_index,
        } = family.coordinates()[coordinate]
        else {
            return Err(unsupported(axis, "cut must be linear in a loop momentum"));
        };
        for (other, removed) in removed.iter().enumerate() {
            if !removed {
                continue;
            }
            let derivative = family
                .derivative_contraction(
                    other,
                    loop_index,
                    ContractionMomentum::External(external_index),
                )
                .map_err(|error| super::SolverError::InvalidInput(error.to_string()))?;
            if derivative
                .denominator_coefficients()
                .iter()
                .any(|c| !c.is_zero())
                || (other == axis) == derivative.constant().is_zero()
            {
                return Err(unsupported(
                    axis,
                    "cut-derivative incidence must be diagonal with nonzero self-incidence",
                ));
            }
        }
        let ordinal = loop_index * (family.loop_count() + family.external_count())
            + family.external_count()
            - 1
            - external_index;
        result.push((axis, ordinal));
    }
    Ok(result)
}
