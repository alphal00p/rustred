//! Checked address-space admission for the bridge's exhaustive sector census.

use super::SolverError;

/// This bounds this census representation, not the mathematical family arity.
/// In particular, a wasm32 build must not wrap a 32-denominator census to one
/// sector or panic before returning the ordinary typed input error.
#[cfg(test)]
pub(super) fn sector_count<const N: usize>() -> Result<usize, SolverError> {
    sector_count_for(N)
}

pub(super) fn sector_count_for(arity: usize) -> Result<usize, SolverError> {
    u32::try_from(arity)
        .ok()
        .and_then(|shift| 1usize.checked_shl(shift))
        .ok_or_else(|| {
            SolverError::InvalidInput(format!(
                "exhaustive sector census for {arity} denominators exceeds this target's {}-bit address space",
                usize::BITS
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn census_width_overflow_is_a_typed_error_without_enumeration() {
        assert!(matches!(
            sector_count::<{ usize::BITS as usize }>(),
            Err(SolverError::InvalidInput(message))
                if message.contains("exhaustive sector census")
        ));
        assert!(matches!(
            sector_count::<{ usize::MAX }>(),
            Err(SolverError::InvalidInput(_))
        ));
    }

    #[test]
    fn representable_census_sizes_are_unchanged() {
        assert_eq!(sector_count::<1>().unwrap(), 2);
        assert_eq!(sector_count::<6>().unwrap(), 64);
        assert_eq!(
            sector_count::<{ usize::BITS as usize - 1 }>().unwrap(),
            usize::MAX / 2 + 1
        );
    }
}
