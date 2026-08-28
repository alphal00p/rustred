use super::error::ParametricIbpError;

/// Return the exact ordinary-IBP and LI row census without constructing any
/// symbolic row. Resource-bounded callers use this preflight before entering
/// the generator's allocation and exact-algebra work.
pub(super) fn checked_row_counts(
    loops: usize,
    externals: usize,
) -> Result<(usize, usize), ParametricIbpError> {
    let contractions = loops
        .checked_add(externals)
        .ok_or(ParametricIbpError::RowCountOverflow { loops, externals })?;
    let ordinary = loops
        .checked_mul(contractions)
        .ok_or(ParametricIbpError::RowCountOverflow { loops, externals })?;
    let li = if externals < 2 {
        0
    } else {
        let predecessor = externals - 1;
        let (left, right) = if externals % 2 == 0 {
            (externals / 2, predecessor)
        } else {
            (externals, predecessor / 2)
        };
        left.checked_mul(right)
            .ok_or(ParametricIbpError::RowCountOverflow { loops, externals })?
    };
    Ok((ordinary, li))
}
