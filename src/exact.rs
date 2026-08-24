use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};

/// A small exact rational used for kinematic basis matrices.
///
/// Symbolic reduction coefficients live in Symbolica's rational-polynomial
/// field.  This type is deliberately limited to the integer/rational linear
/// algebra needed to rewrite loop scalar products into denominators.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExactRational {
    numerator: i64,
    denominator: i64,
}

impl ExactRational {
    pub const ZERO: Self = Self {
        numerator: 0,
        denominator: 1,
    };
    pub const ONE: Self = Self {
        numerator: 1,
        denominator: 1,
    };

    pub fn new(numerator: i64, denominator: i64) -> Self {
        assert!(denominator != 0, "a rational denominator cannot be zero");
        if numerator == 0 {
            return Self::ZERO;
        }

        let mut numerator = numerator;
        let mut denominator = denominator;
        if denominator < 0 {
            numerator = numerator
                .checked_neg()
                .expect("rational numerator overflow while normalizing sign");
            denominator = denominator
                .checked_neg()
                .expect("rational denominator overflow while normalizing sign");
        }

        let gcd = gcd_u64(numerator.unsigned_abs(), denominator as u64) as i64;
        Self {
            numerator: numerator / gcd,
            denominator: denominator / gcd,
        }
    }

    pub fn numerator(self) -> i64 {
        self.numerator
    }

    pub fn denominator(self) -> i64 {
        self.denominator
    }

    pub fn is_zero(self) -> bool {
        self.numerator == 0
    }

    pub fn reciprocal(self) -> Self {
        assert!(!self.is_zero(), "cannot invert zero");
        Self::new(self.denominator, self.numerator)
    }
}

fn gcd_u64(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a.max(1)
}

impl From<i64> for ExactRational {
    fn from(value: i64) -> Self {
        Self::new(value, 1)
    }
}

impl Add for ExactRational {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let numerator = self
            .numerator
            .checked_mul(rhs.denominator)
            .and_then(|x| {
                rhs.numerator
                    .checked_mul(self.denominator)
                    .and_then(|y| x.checked_add(y))
            })
            .expect("rational addition overflow");
        let denominator = self
            .denominator
            .checked_mul(rhs.denominator)
            .expect("rational denominator overflow");
        Self::new(numerator, denominator)
    }
}

impl Sub for ExactRational {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self + (-rhs)
    }
}

impl Mul for ExactRational {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let numerator = self
            .numerator
            .checked_mul(rhs.numerator)
            .expect("rational multiplication overflow");
        let denominator = self
            .denominator
            .checked_mul(rhs.denominator)
            .expect("rational denominator overflow");
        Self::new(numerator, denominator)
    }
}

impl Div for ExactRational {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        self * rhs.reciprocal()
    }
}

impl Neg for ExactRational {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::new(
            self.numerator
                .checked_neg()
                .expect("rational negation overflow"),
            self.denominator,
        )
    }
}

impl fmt::Display for ExactRational {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.denominator == 1 {
            write!(formatter, "{}", self.numerator)
        } else {
            write!(formatter, "{}/{}", self.numerator, self.denominator)
        }
    }
}

/// Invert a square matrix over exact rationals.
pub(crate) fn invert_matrix(
    matrix: &[Vec<ExactRational>],
) -> Result<Vec<Vec<ExactRational>>, String> {
    let size = matrix.len();
    if size == 0 || matrix.iter().any(|row| row.len() != size) {
        return Err("matrix must be non-empty and square".to_owned());
    }

    let mut augmented = vec![vec![ExactRational::ZERO; size * 2]; size];
    for row in 0..size {
        for column in 0..size {
            augmented[row][column] = matrix[row][column];
        }
        augmented[row][size + row] = ExactRational::ONE;
    }

    for pivot_column in 0..size {
        let pivot_row = (pivot_column..size)
            .find(|&row| !augmented[row][pivot_column].is_zero())
            .ok_or_else(|| "denominator basis matrix is singular".to_owned())?;
        augmented.swap(pivot_column, pivot_row);

        let pivot = augmented[pivot_column][pivot_column];
        for entry in &mut augmented[pivot_column] {
            *entry = *entry / pivot;
        }

        for row in 0..size {
            if row == pivot_column {
                continue;
            }
            let factor = augmented[row][pivot_column];
            if factor.is_zero() {
                continue;
            }
            for column in 0..size * 2 {
                augmented[row][column] =
                    augmented[row][column] - factor * augmented[pivot_column][column];
            }
        }
    }

    Ok(augmented
        .into_iter()
        .map(|row| row[size..].to_vec())
        .collect())
}

pub(crate) fn matrix_rank(mut matrix: Vec<Vec<ExactRational>>) -> usize {
    if matrix.is_empty() {
        return 0;
    }
    let columns = matrix[0].len();
    let mut rank = 0;

    for column in 0..columns {
        let Some(pivot_row) = (rank..matrix.len()).find(|&row| !matrix[row][column].is_zero())
        else {
            continue;
        };
        matrix.swap(rank, pivot_row);
        let pivot = matrix[rank][column];
        for entry in &mut matrix[rank] {
            *entry = *entry / pivot;
        }
        for row in rank + 1..matrix.len() {
            let factor = matrix[row][column];
            if factor.is_zero() {
                continue;
            }
            for c in column..columns {
                matrix[row][c] = matrix[row][c] - factor * matrix[rank][c];
            }
        }
        rank += 1;
        if rank == matrix.len() {
            break;
        }
    }
    rank
}

pub(crate) fn matrix_multiply(
    left: &[Vec<ExactRational>],
    right: &[Vec<ExactRational>],
) -> Result<Vec<Vec<ExactRational>>, String> {
    if left.is_empty() || right.is_empty() || left[0].len() != right.len() {
        return Err("incompatible matrix dimensions".to_owned());
    }
    if left.iter().any(|row| row.len() != left[0].len())
        || right.iter().any(|row| row.len() != right[0].len())
    {
        return Err("ragged matrix".to_owned());
    }
    Ok(left
        .iter()
        .map(|left_row| {
            (0..right[0].len())
                .map(|column| {
                    left_row
                        .iter()
                        .zip(right)
                        .map(|(&coefficient, right_row)| coefficient * right_row[column])
                        .fold(ExactRational::ZERO, Add::add)
                })
                .collect()
        })
        .collect())
}

pub(crate) fn matrix_transpose(matrix: &[Vec<ExactRational>]) -> Vec<Vec<ExactRational>> {
    if matrix.is_empty() {
        return Vec::new();
    }
    (0..matrix[0].len())
        .map(|column| matrix.iter().map(|row| row[column]).collect())
        .collect()
}

pub(crate) fn matrix_determinant(matrix: &[Vec<ExactRational>]) -> Result<ExactRational, String> {
    let size = matrix.len();
    if size == 0 || matrix.iter().any(|row| row.len() != size) {
        return Err("matrix must be non-empty and square".to_owned());
    }
    let mut matrix = matrix.to_vec();
    let mut determinant = ExactRational::ONE;
    for column in 0..size {
        let Some(pivot) = (column..size).find(|&row| !matrix[row][column].is_zero()) else {
            return Ok(ExactRational::ZERO);
        };
        if pivot != column {
            matrix.swap(pivot, column);
            determinant = -determinant;
        }
        let pivot_value = matrix[column][column];
        determinant = determinant * pivot_value;
        for row in column + 1..size {
            let factor = matrix[row][column] / pivot_value;
            for entry in column..size {
                matrix[row][entry] = matrix[row][entry] - factor * matrix[column][entry];
            }
        }
    }
    Ok(determinant)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_matrix_inverse() {
        let matrix = vec![
            vec![1.into(), 0.into(), 0.into()],
            vec![0.into(), 0.into(), 1.into()],
            vec![1.into(), 2.into(), 1.into()],
        ];
        let inverse = invert_matrix(&matrix).unwrap();
        for row in 0..3 {
            for column in 0..3 {
                let product = (0..3)
                    .map(|index| matrix[row][index] * inverse[index][column])
                    .fold(ExactRational::ZERO, Add::add);
                assert_eq!(product, if row == column { 1.into() } else { 0.into() });
            }
        }
    }
}
