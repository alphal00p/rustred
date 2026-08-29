use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::rational::RationalField;
use symbolica::prelude::{Integer, Matrix, Q, Rational, Ring, RingOps, Z};

use crate::sector::Mask;

use super::analysis::ZeroSectorAnalyzer;
use super::error::ZeroSectorError;
use super::limits::{check_limit, checked_add, checked_mul};
use super::model::ZeroSectorResource;

#[derive(Clone, Debug)]
pub(super) enum EffectiveRankDecision {
    Zero {
        active_parameter_order: Box<[usize]>,
        primitive_kernel: Box<[Integer]>,
        rank: usize,
        exponent_row_count: usize,
    },
    Full {
        active_parameter_order: Box<[usize]>,
        rank: usize,
        exponent_row_count: usize,
        column_count: usize,
    },
    Resource(ZeroSectorResource),
    Failed(ZeroSectorError),
}

pub(super) struct ExponentMatrix {
    pub(super) rows: Vec<Vec<u16>>,
    pub(super) active_parameter_order: Box<[usize]>,
    pub(super) columns: usize,
}

impl ZeroSectorAnalyzer {
    pub(super) fn compute_effective_checked(&self, effective: &Mask) -> EffectiveRankDecision {
        match catch_unwind(AssertUnwindSafe(|| self.compute_effective(effective))) {
            Ok(Ok(decision)) => decision,
            Ok(Err(ZeroSectorError::ResourceLimit {
                resource,
                requested,
                limit,
            })) => EffectiveRankDecision::Resource(ZeroSectorResource {
                resource,
                requested,
                limit,
            }),
            Ok(Err(error)) => EffectiveRankDecision::Failed(error),
            Err(_) => EffectiveRankDecision::Failed(ZeroSectorError::SymbolicaPanic),
        }
    }

    fn compute_effective(
        &self,
        effective: &Mask,
    ) -> Result<EffectiveRankDecision, ZeroSectorError> {
        let matrix = self.exponent_matrix(effective)?;
        if matrix.rows.is_empty() {
            check_limit(
                "certificate kernel entries",
                matrix.columns,
                self.limits.max_certificate_entries,
            )?;
            check_limit(
                "certificate kernel integer bits",
                1,
                self.limits.max_kernel_integer_bits,
            )?;
            let mut kernel = vec![Integer::zero(); matrix.columns];
            if let Some(first) = kernel.first_mut() {
                *first = Integer::one();
            }
            return Ok(EffectiveRankDecision::Zero {
                active_parameter_order: matrix.active_parameter_order,
                primitive_kernel: kernel.into_boxed_slice(),
                rank: 0,
                exponent_row_count: 0,
            });
        }

        let rank_operations = checked_mul(
            checked_mul(matrix.rows.len(), matrix.columns, "rank operations")?,
            matrix.rows.len().min(matrix.columns),
            "rank operations",
        )?;
        check_limit(
            "rank operations",
            rank_operations,
            self.limits.max_rank_operations,
        )?;
        let minor_bit_bound = matrix.preflight_rref_bits(self.limits.max_rref_integer_bits)?;
        let row_count = matrix.rows.len();
        let mut reduced = matrix.to_symbolica_matrix()?;
        let rank = reduced.row_reduce(matrix.columns as u32);
        validate_rational_matrix_bits(&reduced, self.limits.max_rref_integer_bits)?;
        if rank == matrix.columns {
            return Ok(EffectiveRankDecision::Full {
                active_parameter_order: matrix.active_parameter_order,
                rank,
                exponent_row_count: row_count,
                column_count: matrix.columns,
            });
        }
        check_limit(
            "certificate kernel entries",
            matrix.columns,
            self.limits.max_certificate_entries,
        )?;
        let kernel_bit_bound = checked_mul(
            matrix.columns,
            minor_bit_bound,
            "certificate kernel integer bit bound",
        )?;
        check_limit(
            "certificate kernel integer bits",
            kernel_bit_bound,
            self.limits.max_kernel_integer_bits,
        )?;
        let rational_kernel = deterministic_rref_kernel(&reduced, rank, matrix.columns)?;
        let primitive_kernel =
            primitive_integer_kernel(&rational_kernel, self.limits.max_kernel_integer_bits)?;
        replay_integer_kernel(&matrix.rows, &primitive_kernel)?;
        Ok(EffectiveRankDecision::Zero {
            active_parameter_order: matrix.active_parameter_order,
            primitive_kernel: primitive_kernel.into_boxed_slice(),
            rank,
            exponent_row_count: row_count,
        })
    }
}

impl ExponentMatrix {
    /// Bound exact Gaussian-elimination temporaries from the Leibniz bound
    /// `r! M^r <= (r M)^r` for every integer minor. Canonical RREF entries
    /// are ratios of minors; one rational product/addition can temporarily
    /// need at most twice that many bits plus carry bits.
    fn preflight_rref_bits(&self, limit: usize) -> Result<usize, ZeroSectorError> {
        let rank_dimension = self.rows.len().min(self.columns);
        let maximum_entry = self.rows.iter().flatten().copied().max().unwrap_or(1);
        let entry_bits = usize::try_from(u16::BITS - maximum_entry.leading_zeros())
            .map_err(|_| ZeroSectorError::ResourceCountOverflow {
                resource: "rank matrix entry bit length",
            })?
            .max(1);
        let dimension_bits = ceil_log2(rank_dimension.max(1));
        let minor_bits = checked_add(
            checked_mul(
                rank_dimension,
                checked_add(entry_bits, dimension_bits, "RREF minor bit bound")?,
                "RREF minor bit bound",
            )?,
            1,
            "RREF minor bit bound",
        )?;
        let temporary_bits = checked_add(
            checked_mul(2, minor_bits, "RREF integer bit bound")?,
            2,
            "RREF integer bit bound",
        )?;
        check_limit("RREF integer bits", temporary_bits, limit)?;
        Ok(minor_bits)
    }

    fn to_symbolica_matrix(&self) -> Result<Matrix<RationalField>, ZeroSectorError> {
        let entries = self
            .rows
            .iter()
            .flatten()
            .map(|&entry| Rational::from(i64::from(entry)))
            .collect::<Vec<_>>();
        Matrix::from_linear(entries, self.rows.len() as u32, self.columns as u32, Q)
            .map_err(|detail| ZeroSectorError::MatrixShape { detail })
    }
}

fn deterministic_rref_kernel(
    reduced: &Matrix<RationalField>,
    rank: usize,
    columns: usize,
) -> Result<Vec<Rational>, ZeroSectorError> {
    let mut pivot_for_row = Vec::with_capacity(rank);
    let mut pivot_columns = vec![false; columns];
    for row in 0..rank {
        let pivot = (0..columns)
            .find(|&column| !Q.is_zero(&reduced[(row as u32, column as u32)]))
            .ok_or_else(|| ZeroSectorError::CertificateReplayFailure {
                detail: format!("RREF row {row} has no pivot"),
            })?;
        if pivot_columns[pivot] {
            return Err(ZeroSectorError::CertificateReplayFailure {
                detail: format!("RREF pivot column {pivot} is repeated"),
            });
        }
        if !Q.is_one(&reduced[(row as u32, pivot as u32)]) {
            return Err(ZeroSectorError::CertificateReplayFailure {
                detail: format!("RREF pivot at row {row}, column {pivot} is not normalized"),
            });
        }
        pivot_columns[pivot] = true;
        pivot_for_row.push(pivot);
    }
    let free = pivot_columns
        .iter()
        .position(|&pivot| !pivot)
        .ok_or_else(|| ZeroSectorError::CertificateReplayFailure {
            detail: "rank-deficient matrix has no free column".to_owned(),
        })?;
    let mut kernel = vec![Rational::zero(); columns];
    kernel[free] = Rational::one();
    for (row, &pivot) in pivot_for_row.iter().enumerate() {
        kernel[pivot] = Q.neg(&reduced[(row as u32, free as u32)]);
    }
    Ok(kernel)
}

fn primitive_integer_kernel(
    kernel: &[Rational],
    max_integer_bits: usize,
) -> Result<Vec<Integer>, ZeroSectorError> {
    let mut common_denominator = Integer::one();
    for value in kernel {
        let denominator = value.denominator_ref();
        let gcd = common_denominator.gcd(denominator);
        let reduced = exact_integer_quotient(&common_denominator, &gcd)?;
        common_denominator = Z.mul(&reduced, denominator);
        check_integer_bits(
            &common_denominator,
            "certificate kernel integer bits",
            max_integer_bits,
        )?;
    }
    let mut integers = Vec::with_capacity(kernel.len());
    for value in kernel {
        let scale = exact_integer_quotient(&common_denominator, value.denominator_ref())?;
        let integer = Z.mul(value.numerator_ref(), &scale);
        check_integer_bits(
            &integer,
            "certificate kernel integer bits",
            max_integer_bits,
        )?;
        integers.push(integer);
    }
    let mut content = Integer::zero();
    for value in &integers {
        if !value.is_zero() {
            content = if content.is_zero() {
                value.abs()
            } else {
                content.gcd(&value.abs())
            };
        }
    }
    if content.is_zero() {
        return Err(ZeroSectorError::CertificateReplayFailure {
            detail: "RREF produced a zero kernel".to_owned(),
        });
    }
    for value in &mut integers {
        *value = exact_integer_quotient(value, &content)?;
        check_integer_bits(value, "certificate kernel integer bits", max_integer_bits)?;
    }
    if integers
        .iter()
        .find(|value| !value.is_zero())
        .is_some_and(Integer::is_negative)
    {
        for value in &mut integers {
            *value = Z.neg(&*value);
        }
    }
    Ok(integers)
}

fn validate_rational_matrix_bits(
    matrix: &Matrix<RationalField>,
    limit: usize,
) -> Result<(), ZeroSectorError> {
    for row in matrix.row_iter() {
        for value in row {
            check_integer_bits(value.numerator_ref(), "RREF integer bits", limit)?;
            check_integer_bits(value.denominator_ref(), "RREF integer bits", limit)?;
        }
    }
    Ok(())
}

fn check_integer_bits(
    integer: &Integer,
    resource: &'static str,
    limit: usize,
) -> Result<(), ZeroSectorError> {
    let requested = integer_bit_length(integer)?;
    check_limit(resource, requested, limit)
}

fn integer_bit_length(integer: &Integer) -> Result<usize, ZeroSectorError> {
    let bits = match integer {
        Integer::Single(value) => u64::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u64::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u64::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| ZeroSectorError::ResourceCountOverflow {
        resource: "integer bit length",
    })
}

fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        (usize::BITS - (value - 1).leading_zeros()) as usize
    }
}

fn exact_integer_quotient(
    numerator: &Integer,
    denominator: &Integer,
) -> Result<Integer, ZeroSectorError> {
    if denominator.is_zero() {
        return Err(ZeroSectorError::CertificateReplayFailure {
            detail: "integer certificate normalization divided by zero".to_owned(),
        });
    }
    let (quotient, remainder) = numerator.quot_rem(denominator);
    if remainder.is_zero() {
        Ok(quotient)
    } else {
        Err(ZeroSectorError::CertificateReplayFailure {
            detail: "integer certificate normalization was inexact".to_owned(),
        })
    }
}

pub(super) fn replay_integer_kernel(
    rows: &[Vec<u16>],
    kernel: &[Integer],
) -> Result<(), ZeroSectorError> {
    let columns = rows.first().map_or(kernel.len(), Vec::len);
    if kernel.len() != columns || kernel.iter().all(Integer::is_zero) {
        return Err(ZeroSectorError::CertificateReplayFailure {
            detail: format!(
                "kernel has {} entries for {columns} columns, or is identically zero",
                kernel.len()
            ),
        });
    }
    for (row_index, row) in rows.iter().enumerate() {
        if row.len() != columns {
            return Err(ZeroSectorError::MatrixShape {
                detail: format!("exponent row {row_index} has inconsistent length"),
            });
        }
        let mut sum = Integer::zero();
        for (&entry, value) in row.iter().zip(kernel) {
            let product = Z.mul(&Integer::from(i64::from(entry)), value);
            Z.add_assign(&mut sum, &product);
        }
        if !sum.is_zero() {
            return Err(ZeroSectorError::CertificateReplayFailure {
                detail: format!("kernel product is nonzero on exponent row {row_index}"),
            });
        }
    }
    Ok(())
}
