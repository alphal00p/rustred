use crate::family::ScalarProductCoordinate;

use super::super::error::SymbolicaAffineDenominatorError;

pub(in crate::input::affine) fn scalar_product_coordinate_count(
    loops: usize,
    externals: usize,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    upper_triangular_count(loops)?
        .checked_add(loops.checked_mul(externals).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "loop-external scalar products",
            },
        )?)
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "scalar-product coordinates",
        })
}

pub(in crate::input::affine) fn upper_triangular_count(
    size: usize,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    size.checked_add(1)
        .and_then(|next| size.checked_mul(next))
        .map(|product| product / 2)
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "upper-triangular scalar products",
        })
}

pub(in crate::input::affine) fn upper_triangular_index(
    left: usize,
    right: usize,
    size: usize,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    if left > right || right >= size {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "invalid upper-triangular scalar-product coordinate",
            },
        );
    }
    let preceding = left
        .checked_mul(size)
        .and_then(|value| {
            left.checked_mul(left.saturating_sub(1))
                .map(|triangle| value - triangle / 2)
        })
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "upper-triangular coordinate index",
        })?;
    preceding.checked_add(right - left).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "upper-triangular coordinate index",
        },
    )
}

pub(in crate::input::affine) fn scalar_product_coordinates(
    loops: usize,
    externals: usize,
    capacity: usize,
) -> Result<Vec<ScalarProductCoordinate>, SymbolicaAffineDenominatorError> {
    let mut coordinates = Vec::new();
    coordinates.try_reserve_exact(capacity).map_err(|_| {
        SymbolicaAffineDenominatorError::AllocationFailure {
            resource: "scalar-product coordinates",
            requested: capacity,
        }
    })?;
    for left in 0..loops {
        for right in left..loops {
            coordinates.push(ScalarProductCoordinate::LoopLoop { left, right });
        }
    }
    for loop_index in 0..loops {
        for external_index in 0..externals {
            coordinates.push(ScalarProductCoordinate::LoopExternal {
                loop_index,
                external_index,
            });
        }
    }
    if coordinates.len() != capacity {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "scalar-product coordinate census disagrees with construction",
            },
        );
    }
    Ok(coordinates)
}
