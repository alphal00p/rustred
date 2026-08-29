//! Authenticated native rank, determinant, inverse, and matrix products.

mod congruence;
mod determinant;
mod inverse;
mod product;
mod rank;

pub(crate) use congruence::congruence_of_coefficient_matrix;
pub(crate) use determinant::determinant_of_coefficient_matrix;
pub(crate) use inverse::invert_and_verify_coefficient_matrix;
#[cfg(test)]
pub(crate) use inverse::verify_coefficient_matrix_inverse;
pub(crate) use product::{multiply_coefficient_matrices, multiply_three_coefficient_matrices};
pub(crate) use rank::rank_of_coefficient_matrix;
