//! Verify one cell-specific target-minus-RHS against the parent's COMPLETE
//! original weighted row sum, without copying/rebuilding that original sum.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};
use crate::identity::IndexShift;

use super::super::{SourcePortAuditError, error};
use super::original_combination::OriginalCombination;

/// The zero-product callback is the existing native exact sign/finite-face
/// verifier, closed over this cell's unbounded domain and authenticated zero
/// census. No callback/proof capability escapes the private lowering module.
/// All source, weight and RHS guard obligations must be retained separately.
pub(super) fn verify<const N: usize>(
    context: &IndexedCoefficientContext,
    original: &OriginalCombination,
    rhs: &[(IndexShift, IndexedCoefficient)],
    mut zero_product: impl FnMut(&[i64; N], &IndexedCoefficient) -> Result<bool, SourcePortAuditError>,
) -> Result<usize, SourcePortAuditError> {
    if context.index_count() != N {
        return Err(error(
            "combined refined replay has incompatible index arity",
        ));
    }
    let target = IndexShift::try_new([0; N], N).map_err(error)?;
    let one = context.one();
    let zero = context.zero();
    let mut rhs_columns = BTreeMap::new();
    for (shift, coefficient) in rhs {
        if shift.values().len() != N || shift == &target || coefficient.is_zero() {
            return Err(error(
                "combined refined RHS is not a canonical target-free row",
            ));
        }
        context.bind_sealed(coefficient).map_err(error)?;
        if rhs_columns.insert(shift, coefficient).is_some() {
            return Err(error("combined refined RHS repeats a physical column"));
        }
    }
    let columns: BTreeSet<_> = original
        .columns
        .keys()
        .chain(rhs_columns.keys().copied())
        .chain(std::iter::once(&target))
        .collect();
    let checked = columns.len();
    for shift in columns {
        let physical: &[i64; N] = shift
            .values()
            .try_into()
            .map_err(|_| error("combined original column has incompatible index arity"))?;
        // Full original-only columns remain borrowed. Only the few columns
        // modified by the desired identity allocate new native coefficients.
        let mut residual = Cow::Borrowed(original.columns.get(shift).unwrap_or(&zero));
        if shift == &target {
            residual = Cow::Owned(
                context
                    .sub_bound_with_limits(
                        context.bind_sealed(&residual).map_err(error)?,
                        context.bind_sealed(&one).map_err(error)?,
                        Default::default(),
                    )
                    .map_err(error)?,
            );
        }
        if let Some(coefficient) = rhs_columns.get(shift) {
            residual = Cow::Owned(
                context
                    .add_bound_with_limits(
                        context.bind_sealed(&residual).map_err(error)?,
                        context.bind_sealed(coefficient).map_err(error)?,
                        Default::default(),
                    )
                    .map_err(error)?,
            );
        }
        if residual.is_zero() {
            continue;
        }
        if !zero_product(physical, &residual)? {
            return Err(error(format!(
                "combined original replay has an unproved full residual at shift {:?}: {}",
                physical,
                residual.raw(),
            )));
        }
    }
    Ok(checked)
}

#[cfg(test)]
mod tests;
