mod context;
mod coordinates;
mod gram;
mod input_shape;
mod symbols;

pub(super) use coordinates::{upper_triangular_count, upper_triangular_index};
pub(super) use input_shape::checked_atom_shape;
pub(super) use symbols::maximum_combined_symbol_bytes;

use super::error::SymbolicaAffineDenominatorError;

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SymbolicaAffineDenominatorError> {
    if requested > limit {
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource,
            requested: requested as u128,
            limit: limit as u128,
        })
    } else {
        Ok(())
    }
}
