use crate::algebra::{Coefficient, CoefficientPolynomial};

use super::Integral;

/// One integral and its native Symbolica coefficient. No family or provenance
/// ownership is duplicated in individual terms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Term<const N: usize, C> {
    pub integral: Integral<N>,
    pub coefficient: C,
}

/// Terms in harder-first integral order, with distinct keys and no zero terms.
/// The constructor/preparation boundary establishes these invariants once.
pub type Row<const N: usize, C> = Vec<Term<N, C>>;
pub type PolynomialRow<const N: usize> = Row<N, CoefficientPolynomial>;
pub type ExactRow<const N: usize> = Row<N, Coefficient>;
