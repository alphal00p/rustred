//! Standard-monomial count of a monomial ideal.
//!
//! For a Groebner basis `B` of `I`, the standard monomials (those divisible by
//! no leading monomial of `B`) form a basis of `k[x]/I`. Symbolica keeps its
//! staircase enumeration private, so this is a small reimplementation.

/// Dimension of `k[x]/I` read off the leading monomials of a Groebner basis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Staircase {
    /// Finite number of standard monomials.
    Finite(usize),
    /// Some variable has no pure power among the leading monomials, so the
    /// ideal is positive-dimensional.
    Infinite,
}

/// The finite staircase has more than `limit` standard monomials.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StaircaseLimit;

/// Count standard monomials in `variables` variables.
///
/// Invariant: every entry of `leading` has length `variables`.
pub(super) fn standard_monomials(
    leading: &[Vec<u16>],
    variables: usize,
    limit: usize,
) -> Result<Staircase, StaircaseLimit> {
    debug_assert!(leading.iter().all(|monomial| monomial.len() == variables));
    if leading
        .iter()
        .any(|monomial| monomial.iter().all(|&exponent| exponent == 0))
    {
        return Ok(Staircase::Finite(0));
    }
    let has_pure_power = |variable: usize| {
        leading.iter().any(|monomial| {
            monomial
                .iter()
                .enumerate()
                .all(|(other, &exponent)| (other == variable) == (exponent > 0))
        })
    };
    if !(0..variables).all(has_pure_power) {
        return Ok(Staircase::Infinite);
    }
    let mut exponents = vec![0; variables];
    let mut count = 0;
    count_from(leading, &mut exponents, 0, limit, &mut count)?;
    Ok(Staircase::Finite(count))
}

/// Enumerate standard monomials agreeing with `exponents` before `variable`
/// and zero from `variable` onwards.
///
/// The standard monomials are closed under division. When raising the
/// exponent of `variable` (with later exponents zero) yields a leading-term
/// multiple, so does every larger exponent and every extension in later
/// variables; the loop therefore stops there. Pure powers bound every loop.
fn count_from(
    leading: &[Vec<u16>],
    exponents: &mut [u16],
    variable: usize,
    limit: usize,
    count: &mut usize,
) -> Result<(), StaircaseLimit> {
    if variable == exponents.len() {
        *count += 1;
        return if *count > limit {
            Err(StaircaseLimit)
        } else {
            Ok(())
        };
    }
    while !is_divisible(exponents, leading) {
        count_from(leading, exponents, variable + 1, limit, count)?;
        exponents[variable] += 1;
    }
    exponents[variable] = 0;
    Ok(())
}

fn is_divisible(monomial: &[u16], leading: &[Vec<u16>]) -> bool {
    leading.iter().any(|divisor| {
        divisor
            .iter()
            .zip(monomial)
            .all(|(&needed, &present)| needed <= present)
    })
}

#[cfg(test)]
mod tests {
    use super::{Staircase, StaircaseLimit, standard_monomials};

    #[test]
    fn counts_box_and_corner_staircases() {
        assert_eq!(
            standard_monomials(&[vec![2, 0], vec![0, 3]], 2, 100),
            Ok(Staircase::Finite(6))
        );
        assert_eq!(
            standard_monomials(&[vec![2, 0], vec![1, 1], vec![0, 2]], 2, 100),
            Ok(Staircase::Finite(3))
        );
        assert_eq!(
            standard_monomials(
                &[vec![3, 0, 0], vec![0, 1, 0], vec![0, 0, 2], vec![1, 0, 1]],
                3,
                100
            ),
            Ok(Staircase::Finite(4))
        );
        assert_eq!(
            standard_monomials(&[vec![1, 0], vec![0, 1]], 2, 100),
            Ok(Staircase::Finite(1))
        );
    }

    #[test]
    fn unit_ideal_has_an_empty_staircase() {
        assert_eq!(
            standard_monomials(&[vec![0, 0], vec![1, 0]], 2, 100),
            Ok(Staircase::Finite(0))
        );
    }

    #[test]
    fn missing_pure_power_is_positive_dimensional() {
        assert_eq!(
            standard_monomials(&[vec![2, 0], vec![1, 1]], 2, 100),
            Ok(Staircase::Infinite)
        );
        assert_eq!(standard_monomials(&[], 1, 100), Ok(Staircase::Infinite));
    }

    #[test]
    fn limit_is_an_error_not_a_count() {
        assert_eq!(
            standard_monomials(&[vec![4, 0], vec![0, 4]], 2, 16),
            Ok(Staircase::Finite(16))
        );
        assert_eq!(
            standard_monomials(&[vec![4, 0], vec![0, 4]], 2, 15),
            Err(StaircaseLimit)
        );
    }
}
