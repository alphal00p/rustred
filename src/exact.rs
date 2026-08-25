use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::integer::Integer;
use symbolica::domains::rational::{Q, Rational};
use symbolica::domains::{Ring, RingOps};
use symbolica::tensors::matrix::Matrix;

/// A canonical exact rational backed by Symbolica's GMP-enabled rational field.
///
/// This nominal wrapper keeps RustRed's public kinematic-coordinate type
/// separate from its rational-polynomial coefficients. All normalization and
/// arithmetic are delegated to Symbolica; RustRed does not implement a second
/// rational field.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExactRational(Rational);

/// Checked-construction and checked-division failures for ExactRational.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExactRationalError {
    ZeroDenominator,
    DivisionByZero,
}

impl fmt::Display for ExactRationalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDenominator => formatter.write_str("a rational denominator cannot be zero"),
            Self::DivisionByZero => formatter.write_str("cannot divide by zero"),
        }
    }
}

impl std::error::Error for ExactRationalError {}

impl ExactRational {
    /// Construct and canonically normalize an exact rational.
    ///
    /// This compatibility constructor retains the historical panic on a zero
    /// denominator. Input-dependent code should use try_new.
    pub fn new<N: Into<Integer>, D: Into<Integer>>(numerator: N, denominator: D) -> Self {
        Self::try_new(numerator, denominator).expect("a rational denominator cannot be zero")
    }

    /// Construct and canonically normalize an exact rational without a panic
    /// on a zero denominator.
    pub fn try_new<N: Into<Integer>, D: Into<Integer>>(
        numerator: N,
        denominator: D,
    ) -> Result<Self, ExactRationalError> {
        let numerator = numerator.into();
        let denominator = denominator.into();
        if denominator.is_zero() {
            return Err(ExactRationalError::ZeroDenominator);
        }

        // Rational::new delegates gcd reduction and denominator-sign
        // normalization to Symbolica's exact rational field. The zero case was
        // checked above because Symbolica's constructor is intentionally
        // panicking for that malformed input.
        Ok(Self(Rational::new(numerator, denominator)))
    }

    pub fn zero() -> Self {
        Self(Rational::zero())
    }

    pub fn one() -> Self {
        Self(Rational::one())
    }

    /// Return the arbitrary-precision canonical numerator.
    pub fn numerator(&self) -> &Integer {
        self.0.numerator_ref()
    }

    /// Return the arbitrary-precision positive canonical denominator.
    pub fn denominator(&self) -> &Integer {
        self.0.denominator_ref()
    }

    /// Narrow the numerator only when it is exactly representable as i64.
    pub fn numerator_i64(&self) -> Option<i64> {
        self.numerator().to_i64()
    }

    /// Narrow the denominator only when it is exactly representable as i64.
    pub fn denominator_i64(&self) -> Option<i64> {
        self.denominator().to_i64()
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub fn is_one(&self) -> bool {
        self.0.is_one()
    }

    pub fn is_negative(&self) -> bool {
        self.0.is_negative()
    }

    /// Invert this value using Symbolica's checked rational-field operation.
    pub fn try_reciprocal(&self) -> Result<Self, ExactRationalError> {
        Q.try_inv(&self.0)
            .map(Self)
            .ok_or(ExactRationalError::DivisionByZero)
    }

    /// Historical panicking reciprocal. Input-dependent code should use
    /// try_reciprocal.
    pub fn reciprocal(&self) -> Self {
        self.try_reciprocal().expect("cannot invert zero")
    }

    /// Divide through Symbolica's checked rational-field operation.
    pub fn try_div(&self, rhs: &Self) -> Result<Self, ExactRationalError> {
        Q.try_div(&self.0, &rhs.0)
            .map(Self)
            .ok_or(ExactRationalError::DivisionByZero)
    }

    /// Borrow the underlying canonical Symbolica value.
    pub fn as_rational(&self) -> &Rational {
        &self.0
    }

    /// Consume this wrapper and return its canonical Symbolica value.
    pub fn into_rational(self) -> Rational {
        self.0
    }

    /// Heap bytes retained by the numerator and denominator GMP allocations.
    ///
    /// The fixed-size `ExactRational` wrapper is deliberately excluded so
    /// callers can census inline and heap storage without double counting.
    /// Symbolica publicly exposes its GMP integer variant, whose `capacity()`
    /// reports the allocated bit capacity rather than only significant bits.
    pub(crate) fn retained_heap_bytes(&self) -> Option<usize> {
        fn integer_heap_bytes(value: &Integer) -> usize {
            match value {
                Integer::Large(value) => value.capacity().div_ceil(u8::BITS as usize),
                Integer::Single(_) | Integer::Double(_) => 0,
            }
        }

        integer_heap_bytes(self.numerator()).checked_add(integer_heap_bytes(self.denominator()))
    }

    fn from_symbolica(value: Rational) -> Self {
        Self(value)
    }
}

impl From<i64> for ExactRational {
    fn from(value: i64) -> Self {
        Self(Rational::from(value))
    }
}

impl From<Integer> for ExactRational {
    fn from(value: Integer) -> Self {
        Self(Rational::from(value))
    }
}

macro_rules! forward_binary_operator {
    ($trait:ident, $method:ident, $operator:tt) => {
        impl $trait for ExactRational {
            type Output = Self;

            fn $method(self, rhs: Self) -> Self::Output {
                Self(self.0 $operator rhs.0)
            }
        }

        impl $trait<&ExactRational> for ExactRational {
            type Output = ExactRational;

            fn $method(self, rhs: &ExactRational) -> Self::Output {
                ExactRational(self.0 $operator &rhs.0)
            }
        }

        impl $trait<ExactRational> for &ExactRational {
            type Output = ExactRational;

            fn $method(self, rhs: ExactRational) -> Self::Output {
                ExactRational(&self.0 $operator &rhs.0)
            }
        }

        impl $trait<&ExactRational> for &ExactRational {
            type Output = ExactRational;

            fn $method(self, rhs: &ExactRational) -> Self::Output {
                ExactRational(&self.0 $operator &rhs.0)
            }
        }
    };
}

forward_binary_operator!(Add, add, +);
forward_binary_operator!(Sub, sub, -);
forward_binary_operator!(Mul, mul, *);
forward_binary_operator!(Div, div, /);

impl Neg for ExactRational {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl Neg for &ExactRational {
    type Output = ExactRational;

    fn neg(self) -> Self::Output {
        ExactRational(Q.neg(&self.0))
    }
}

impl fmt::Display for ExactRational {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

type SymbolicaExactMatrix = Matrix<Q>;

fn matrix_shape(matrix: &[Vec<ExactRational>]) -> Result<(u32, u32, usize), String> {
    let rows = matrix.len();
    let columns = matrix.first().map_or(0, Vec::len);
    if let Some((row, actual)) = matrix
        .iter()
        .enumerate()
        .find_map(|(row, values)| (values.len() != columns).then_some((row, values.len())))
    {
        return Err(format!(
            "ragged matrix: row {row} has {actual} columns, expected {columns}"
        ));
    }

    let rows_u32 = u32::try_from(rows)
        .map_err(|_| format!("matrix row count {rows} exceeds Symbolica's u32 dimension"))?;
    let columns_u32 = u32::try_from(columns)
        .map_err(|_| format!("matrix column count {columns} exceeds Symbolica's u32 dimension"))?;
    let entries = rows
        .checked_mul(columns)
        .ok_or_else(|| format!("matrix element count overflows usize: {rows}*{columns}"))?;

    // Matrix::from_linear currently multiplies its two u32 dimensions before
    // converting to usize. Reject that overflow prospectively.
    rows_u32.checked_mul(columns_u32).ok_or_else(|| {
        format!("matrix element count exceeds Symbolica's u32 constructor range: {rows}*{columns}")
    })?;

    Ok((rows_u32, columns_u32, entries))
}

fn to_symbolica_matrix(matrix: &[Vec<ExactRational>]) -> Result<SymbolicaExactMatrix, String> {
    let (rows, columns, entries) = matrix_shape(matrix)?;
    let mut data = Vec::new();
    data.try_reserve_exact(entries)
        .map_err(|_| format!("failed to reserve {entries} exact entries for a Symbolica matrix"))?;
    for row in matrix {
        data.extend(row.iter().map(|value| value.0.clone()));
    }

    Matrix::from_linear(data, rows, columns, Q)
}

fn from_symbolica_matrix(matrix: SymbolicaExactMatrix) -> Vec<Vec<ExactRational>> {
    let rows = matrix.nrows();
    let columns = matrix.ncols();
    let mut data = matrix.into_vec().into_iter();
    (0..rows)
        .map(|_| {
            data.by_ref()
                .take(columns)
                .map(ExactRational::from_symbolica)
                .collect()
        })
        .collect()
}

fn call_symbolica_matrix<T>(
    operation: &'static str,
    callback: impl FnOnce() -> T,
) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(callback))
        .map_err(|_| format!("Symbolica panicked while computing matrix {operation}"))
}

/// Invert a non-empty square matrix over Symbolica's exact rational field.
pub(crate) fn invert_matrix(
    matrix: &[Vec<ExactRational>],
) -> Result<Vec<Vec<ExactRational>>, String> {
    let native = to_symbolica_matrix(matrix)?;
    let size = native.nrows();
    if size == 0 || native.ncols() != size {
        return Err("matrix must be non-empty and square".to_owned());
    }

    // Symbolica 2.2.0's generic inverse branch row-reduces every column of
    // [A|I], so pivots in I can mask a singular A for sizes 1 and >=4. Its
    // native determinant is an independent, exact singularity guard.
    let determinant = call_symbolica_matrix("inverse determinant guard", || native.det())?
        .map_err(|error| error.to_string())?;
    if determinant.is_zero() {
        return Err("denominator basis matrix is singular".to_owned());
    }

    let inverse =
        call_symbolica_matrix("inverse", || native.inv())?.map_err(|error| error.to_string())?;
    Ok(from_symbolica_matrix(inverse))
}

pub(crate) fn matrix_rank(matrix: Vec<Vec<ExactRational>>) -> Result<usize, String> {
    if matrix.is_empty() {
        return Ok(0);
    }
    let native = to_symbolica_matrix(&matrix)?;
    call_symbolica_matrix("rank", || native.rank())
}

pub(crate) fn matrix_multiply(
    left: &[Vec<ExactRational>],
    right: &[Vec<ExactRational>],
) -> Result<Vec<Vec<ExactRational>>, String> {
    if left.is_empty() || right.is_empty() {
        return Err("incompatible matrix dimensions".to_owned());
    }
    let left = to_symbolica_matrix(left)?;
    let right = to_symbolica_matrix(right)?;
    if left.ncols() != right.nrows() {
        return Err(format!(
            "incompatible matrix dimensions: ({},{}) and ({},{})",
            left.nrows(),
            left.ncols(),
            right.nrows(),
            right.ncols()
        ));
    }

    let product = call_symbolica_matrix("product", || &left * &right)?;
    Ok(from_symbolica_matrix(product))
}

pub(crate) fn matrix_transpose(
    matrix: &[Vec<ExactRational>],
) -> Result<Vec<Vec<ExactRational>>, String> {
    if matrix.is_empty() {
        return Ok(Vec::new());
    }
    let native = to_symbolica_matrix(matrix)?;
    let transposed = call_symbolica_matrix("transpose", || native.transpose())?;
    Ok(from_symbolica_matrix(transposed))
}

pub(crate) fn matrix_determinant(matrix: &[Vec<ExactRational>]) -> Result<ExactRational, String> {
    let native = to_symbolica_matrix(matrix)?;
    if native.nrows() == 0 || native.nrows() != native.ncols() {
        return Err("matrix must be non-empty and square".to_owned());
    }
    call_symbolica_matrix("determinant", || native.det())?
        .map(ExactRational::from_symbolica)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagonal(values: impl IntoIterator<Item = ExactRational>) -> Vec<Vec<ExactRational>> {
        let values = values.into_iter().collect::<Vec<_>>();
        (0..values.len())
            .map(|row| {
                (0..values.len())
                    .map(|column| {
                        if row == column {
                            values[row].clone()
                        } else {
                            ExactRational::zero()
                        }
                    })
                    .collect()
            })
            .collect()
    }

    fn assert_inverse(matrix: &[Vec<ExactRational>]) {
        let inverse = invert_matrix(matrix).unwrap();
        for product in [
            matrix_multiply(matrix, &inverse).unwrap(),
            matrix_multiply(&inverse, matrix).unwrap(),
        ] {
            for (row, values) in product.iter().enumerate() {
                for (column, value) in values.iter().enumerate() {
                    let expected = if row == column {
                        ExactRational::one()
                    } else {
                        ExactRational::zero()
                    };
                    assert_eq!(value, &expected);
                }
            }
        }

        // Cross-check every inverse column through Symbolica's independent
        // public linear solver. This does not compare inv() against itself:
        // solve() reduces only the coefficient columns of A.
        let native = to_symbolica_matrix(matrix).unwrap();
        for column in 0..matrix.len() {
            let right_hand_side = Matrix::new_vec(
                (0..matrix.len())
                    .map(|row| if row == column { Q.one() } else { Q.zero() })
                    .collect(),
                Q,
            );
            let solution = native.solve(&right_hand_side).unwrap();
            for row in 0..matrix.len() {
                assert_eq!(
                    inverse[row][column].as_rational(),
                    &solution[(row as u32, 0)]
                );
            }
        }
    }

    #[test]
    fn exact_rational_is_gmp_backed_and_canonical() {
        let huge = Integer::from(2).pow(257);
        let value = ExactRational::try_new(-&huge * 6, Integer::from(-18)).unwrap();
        assert_eq!(value.numerator(), &huge);
        assert_eq!(value.denominator(), &Integer::from(3));
        assert_eq!(value.numerator_i64(), None);
        assert_eq!(value.denominator_i64(), Some(3));

        let squared = &value * &value;
        assert_eq!(squared.numerator(), &huge.pow(2));
        assert_eq!(squared.denominator(), &Integer::from(9));
    }

    #[test]
    fn exact_rational_checked_boundaries_reject_zero() {
        assert_eq!(
            ExactRational::try_new(1, 0),
            Err(ExactRationalError::ZeroDenominator)
        );
        assert_eq!(
            ExactRational::zero().try_reciprocal(),
            Err(ExactRationalError::DivisionByZero)
        );
        assert_eq!(
            ExactRational::one().try_div(&ExactRational::zero()),
            Err(ExactRationalError::DivisionByZero)
        );
    }

    #[test]
    fn retained_heap_census_uses_gmp_capacity() {
        assert_eq!(ExactRational::new(3, 7).retained_heap_bytes(), Some(0));

        let huge = Integer::from(2).pow(257) + Integer::from(1);
        let value = ExactRational::try_new(huge.clone(), &huge + 2).unwrap();
        let retained = value.retained_heap_bytes().unwrap();
        let expected: usize = [value.numerator(), value.denominator()]
            .into_iter()
            .map(|integer| match integer {
                Integer::Large(integer) => integer.capacity().div_ceil(u8::BITS as usize),
                Integer::Single(_) | Integer::Double(_) => 0,
            })
            .sum();
        assert_eq!(retained, expected);
        assert!(retained >= 66, "both 258-bit GMP payloads are retained");
    }

    #[test]
    fn exact_matrix_inverse_uses_symbolica() {
        let matrix = vec![
            vec![1.into(), 0.into(), 0.into()],
            vec![0.into(), 0.into(), 1.into()],
            vec![1.into(), 2.into(), 1.into()],
        ];
        assert_inverse(&matrix);
    }

    #[test]
    fn exact_matrix_inverse_preserves_gmp_entries() {
        let huge = Integer::from(2).pow(257) + Integer::from(17);
        let matrix = diagonal([ExactRational::from(huge.clone()), ExactRational::new(1, 3)]);
        assert_inverse(&matrix);
        assert_eq!(
            matrix_determinant(&matrix).unwrap(),
            ExactRational::try_new(huge, 3).unwrap()
        );
    }

    #[test]
    fn inverse_handles_one_through_six_dimensions() {
        for size in 1_usize..=6 {
            let values = (0..size)
                .map(|index| ExactRational::from(i64::try_from(index + 2).unwrap()))
                .collect::<Vec<_>>();
            assert_inverse(&diagonal(values));
        }
    }

    #[test]
    fn inverse_rejects_singular_one_through_six_dimensions() {
        for size in 1_usize..=6 {
            let mut matrix = diagonal((0..size).map(|_| ExactRational::one()));
            if size == 1 {
                matrix[0][0] = ExactRational::zero();
            } else {
                matrix[size - 1] = matrix[0].clone();
            }
            assert_eq!(
                invert_matrix(&matrix).unwrap_err(),
                "denominator basis matrix is singular"
            );
        }
    }

    #[test]
    fn determinant_tracks_row_swap_sign() {
        let matrix = vec![
            vec![ExactRational::zero(), ExactRational::one()],
            vec![ExactRational::one(), ExactRational::zero()],
        ];
        assert_eq!(
            matrix_determinant(&matrix).unwrap(),
            ExactRational::from(-1)
        );
    }

    #[test]
    fn exhaustive_two_by_two_rank_determinant_and_inverse_agree() {
        for first in -2_i64..=2 {
            for second in -2_i64..=2 {
                for third in -2_i64..=2 {
                    for fourth in -2_i64..=2 {
                        let matrix = vec![
                            vec![first.into(), second.into()],
                            vec![third.into(), fourth.into()],
                        ];
                        let determinant = matrix_determinant(&matrix).unwrap();
                        let rank = matrix_rank(matrix.clone()).unwrap();
                        assert_eq!(determinant.is_zero(), rank < 2);
                        if determinant.is_zero() {
                            assert_eq!(
                                invert_matrix(&matrix).unwrap_err(),
                                "denominator basis matrix is singular"
                            );
                        } else {
                            assert_inverse(&matrix);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn matrix_shape_and_zero_column_boundaries_are_checked() {
        let ragged = vec![vec![ExactRational::one()], Vec::new()];
        assert!(matrix_rank(ragged.clone()).unwrap_err().contains("ragged"));
        assert!(matrix_transpose(&ragged).unwrap_err().contains("ragged"));
        assert!(
            matrix_multiply(&ragged, &ragged)
                .unwrap_err()
                .contains("ragged")
        );
        assert!(matrix_determinant(&ragged).unwrap_err().contains("ragged"));

        let zero_columns = vec![Vec::new(), Vec::new()];
        assert_eq!(matrix_rank(zero_columns.clone()).unwrap(), 0);
        assert!(matrix_transpose(&zero_columns).unwrap().is_empty());
        assert_eq!(matrix_rank(Vec::new()).unwrap(), 0);
        assert!(matrix_determinant(&[]).is_err());
    }

    #[test]
    fn rectangular_rank_product_and_transpose_use_native_matrices() {
        let matrix = vec![
            vec![1.into(), 2.into(), 3.into()],
            vec![2.into(), 4.into(), 6.into()],
        ];
        assert_eq!(matrix_rank(matrix.clone()).unwrap(), 1);
        assert_eq!(
            matrix_transpose(&matrix).unwrap(),
            vec![
                vec![1.into(), 2.into()],
                vec![2.into(), 4.into()],
                vec![3.into(), 6.into()]
            ]
        );

        let right = vec![vec![1.into()], vec![0.into()], vec![1.into()]];
        assert_eq!(
            matrix_multiply(&matrix, &right).unwrap(),
            vec![vec![4.into()], vec![8.into()]]
        );
        assert!(matrix_multiply(&right, &matrix).is_err());
    }
}
