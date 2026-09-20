//! One-sided, exact full-column-rank screen through Symbolica's native field.
//!
//! A nonzero modular maximal minor is a nonzero integer minor. Failure says
//! nothing about rational rank and must retain the rational/kernel path. This
//! service neither supplies a zero-sector proof nor reconstructs a witness.

use std::sync::LazyLock;

use symbolica::prelude::{FiniteFieldCore, Integer, Matrix, Zp64};

use super::admission::{RightKernelError, RightKernelLimits, try_vec};

const PRIME: u64 = (1_u64 << 61) - 1;

static FIELD: LazyLock<Zp64> = LazyLock::new(|| {
    // Zp64::new assumes primality. Symbolica's test is deterministic for u64.
    assert!(Integer::from(PRIME).is_prime(0));
    Zp64::new(PRIME)
});

pub(super) fn admitted(
    rows: usize,
    columns: usize,
    entries: usize,
    rank_operations: usize,
    limits: RightKernelLimits,
) -> bool {
    rows >= columns
        && rank_operations
            .checked_mul(2)
            .and_then(|work| work.checked_add(entries))
            .is_some_and(|work| work <= limits.max_rank_operations)
}

/// Shape and budgets have already been admitted by the caller. Keep the native
/// matrix local so it is dropped before any rational fallback is allocated.
pub(super) fn full_column_rank(
    entries: &[u16],
    rows: usize,
    columns: usize,
) -> Result<bool, RightKernelError> {
    let field = FIELD.clone();
    let mut modular_entries = try_vec(entries.len(), "modular rank matrix")?;
    modular_entries.extend(
        entries
            .iter()
            .map(|&value| field.to_element(u64::from(value))),
    );
    let mut matrix = Matrix::from_linear(modular_entries, rows as u32, columns as u32, field)
        .map_err(|_| RightKernelError::NativeShape)?;
    Ok(matrix.partial_row_reduce(columns as u32) as usize == columns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::matrix::right_kernel::{RightKernelDecision, first_primitive_right_kernel};

    fn limits() -> RightKernelLimits {
        RightKernelLimits {
            max_rows: 100,
            max_columns: 100,
            max_entries: 10_000,
            max_rank_operations: 1_000_000,
            max_rref_integer_bits: 10_000,
            max_kernel_entries: 100,
            max_kernel_integer_bits: 10_000,
        }
    }

    #[test]
    fn unlucky_real_prime_keeps_exact_full_rank() {
        // det = 32768^4 * 2 - 1 = PRIME. Rank is lost at the actual
        // production prime, although the integer matrix is invertible.
        let entries = [
            32768, 1, 0, 0, 0, 0, 0, 32768, 1, 0, 0, 0, 0, 0, 32768, 1, 0, 0, 0, 0, 0, 32768, 1, 0,
            0, 0, 0, 0, 2, 1, 1, 0, 0, 0, 0, 1,
        ];
        assert!(!full_column_rank(&entries, 6, 6).unwrap());
        assert_eq!(
            first_primitive_right_kernel(&entries, 6, 6, limits()).unwrap(),
            RightKernelDecision::FullColumnRank { rank: 6 },
        );
    }

    #[test]
    fn screening_reserves_fallback_and_conversion_work() {
        let mut bounded = limits();
        for budget in [7, 8, 19, 20] {
            bounded.max_rank_operations = budget;
            assert_eq!(admitted(2, 2, 4, 8, bounded), budget >= 20);
            let result = first_primitive_right_kernel(&[1, 0, 0, 1], 2, 2, bounded);
            if budget < 8 {
                assert_eq!(
                    result,
                    Err(RightKernelError::ResourceLimit {
                        resource: "rank operations",
                        requested: 8,
                        limit: budget,
                    })
                );
            } else {
                assert_eq!(
                    result.unwrap(),
                    RightKernelDecision::FullColumnRank { rank: 2 }
                );
            }
        }
        assert!(!admitted(1, 2, 2, 2, limits()));
        assert!(!admitted(1, 1, 1, usize::MAX, limits()));
        assert!(!admitted(1, 1, usize::MAX, 1, limits()));
    }

    #[test]
    fn modular_hit_does_not_bypass_original_bit_admission() {
        let mut bounded = limits();
        bounded.max_rref_integer_bits = 11;
        assert!(full_column_rank(&[1, 0, 0, 1], 2, 2).unwrap());
        assert_eq!(
            first_primitive_right_kernel(&[1, 0, 0, 1], 2, 2, bounded),
            Err(RightKernelError::ResourceLimit {
                resource: "RREF integer bits",
                requested: 12,
                limit: 11,
            }),
        );
    }

    #[test]
    fn screened_and_exact_fallback_agree_exhaustively() {
        // The original-work budget disables screening without changing exact
        // rank or deterministic first-free-column witnesses. Compare the whole
        // decision, including every replayed primitive-kernel coordinate.
        for (rows, columns) in [(1, 1), (1, 3), (2, 2), (3, 2)] {
            let count = rows * columns;
            for code in 0..3_usize.pow(count as u32) {
                let mut code = code;
                let entries: Vec<u16> = (0..count)
                    .map(|_| {
                        let entry = (code % 3) as u16;
                        code /= 3;
                        entry
                    })
                    .collect();
                let mut exact = limits();
                exact.max_rank_operations = count * rows.min(columns);
                assert_eq!(
                    first_primitive_right_kernel(&entries, rows, columns, limits()),
                    first_primitive_right_kernel(&entries, rows, columns, exact),
                    "shape={rows}x{columns} entries={entries:?}",
                );
            }
        }
        for entries in [[u16::MAX, 0, 0, u16::MAX], [u16::MAX; 4]] {
            let mut exact = limits();
            exact.max_rank_operations = 8;
            assert_eq!(
                first_primitive_right_kernel(&entries, 2, 2, limits()),
                first_primitive_right_kernel(&entries, 2, 2, exact),
            );
        }
    }

    #[test]
    fn modular_miss_keeps_kernel_resource_errors() {
        let entries = [2, 3, 4, 6];
        assert!(!full_column_rank(&entries, 2, 2).unwrap());
        for (kernel_entries, kernel_bits) in [(1, 10_000), (100, 1)] {
            let mut screened = limits();
            screened.max_kernel_entries = kernel_entries;
            screened.max_kernel_integer_bits = kernel_bits;
            let mut exact = screened;
            exact.max_rank_operations = 8;
            let result = first_primitive_right_kernel(&entries, 2, 2, screened);
            assert!(result.is_err());
            assert_eq!(result, first_primitive_right_kernel(&entries, 2, 2, exact));
        }
    }
}
