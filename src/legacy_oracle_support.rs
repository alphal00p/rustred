//! Private dependency bridge for the separately packaged legacy oracles.
//!
//! This module is intentionally doc-hidden and feature-gated. It exposes only
//! the representation-level seams needed by the authored finite-shell oracles.
//! New production code must use RustRed's ordinary public APIs instead.

use crate::ExactRational;

/// Conservative coefficient-degree observations used by legacy finite-shell
/// resource preflights.
pub mod coefficient_degree {
    use crate::Coefficient;

    /// Per-variable numerator and denominator degrees.
    #[inline]
    pub fn coefficient_variable_degrees(coefficient: &Coefficient) -> Vec<(u128, u128)> {
        crate::coefficient::coefficient_variable_degrees(coefficient)
    }

    /// Whether one degree fits Symbolica's configured coefficient exponent.
    #[inline]
    pub fn symbolica_coefficient_degree_is_representable(requested: u128) -> bool {
        crate::coefficient::symbolica_coefficient_degree_is_representable(requested)
    }

    /// Conservative maximum per-variable degree needed by a product.
    #[inline]
    pub fn coefficient_product_degree_bound(left: &Coefficient, right: &Coefficient) -> u128 {
        crate::coefficient::coefficient_product_degree_bound(left, right)
    }

    /// Conservative maximum per-variable degree needed by a sum.
    #[inline]
    pub fn coefficient_sum_degree_bound(left: &Coefficient, right: &Coefficient) -> u128 {
        crate::coefficient::coefficient_sum_degree_bound(left, right)
    }
}

/// Exact matrix operations retained solely by the legacy topology oracles.
pub mod exact_matrix {
    use crate::ExactRational;

    #[inline]
    pub fn invert_matrix(matrix: &[Vec<ExactRational>]) -> Result<Vec<Vec<ExactRational>>, String> {
        crate::exact::invert_matrix(matrix)
    }

    #[inline]
    pub fn matrix_rank(matrix: Vec<Vec<ExactRational>>) -> Result<usize, String> {
        crate::exact::matrix_rank(matrix)
    }

    #[inline]
    pub fn matrix_multiply(
        left: &[Vec<ExactRational>],
        right: &[Vec<ExactRational>],
    ) -> Result<Vec<Vec<ExactRational>>, String> {
        crate::exact::matrix_multiply(left, right)
    }

    #[inline]
    pub fn matrix_determinant(matrix: &[Vec<ExactRational>]) -> Result<ExactRational, String> {
        crate::exact::matrix_determinant(matrix)
    }

    #[inline]
    pub fn matrix_transpose(
        matrix: &[Vec<ExactRational>],
    ) -> Result<Vec<Vec<ExactRational>>, String> {
        crate::exact::matrix_transpose(matrix)
    }
}

/// Heap bytes retained by one exact rational's GMP payload.
#[inline]
pub fn exact_rational_retained_heap_bytes(value: &ExactRational) -> Option<usize> {
    value.retained_heap_bytes()
}

/// Minimal Symbolica Atom surface required by the legacy concrete engine and
/// Vakint adapter.
///
/// Deliberately do not re-export Symbolica's prelude or polynomial domains.
pub mod symbolica_atom {
    pub use symbolica::atom::{Atom, AtomCore, AtomView, FunctionBuilder, Symbol};
    pub use symbolica::{get_symbol, try_parse, try_symbol};
}

#[cfg(test)]
mod tests {
    use super::{coefficient_degree, exact_matrix, symbolica_atom};
    use crate::{CoefficientContext, ExactRational};

    #[test]
    fn coefficient_and_matrix_bridges_preserve_core_results() {
        let context = CoefficientContext::new(["x"]);
        let x = context.parse("x").unwrap();
        let square = &x * &x;
        assert_eq!(
            coefficient_degree::coefficient_variable_degrees(&square),
            vec![(2, 0)]
        );
        assert_eq!(
            coefficient_degree::coefficient_product_degree_bound(&x, &square),
            3
        );
        assert_eq!(
            coefficient_degree::coefficient_sum_degree_bound(&x, &square),
            2
        );
        assert!(
            coefficient_degree::symbolica_coefficient_degree_is_representable(u16::MAX as u128)
        );

        let matrix = vec![vec![ExactRational::from(2)]];
        assert_eq!(exact_matrix::matrix_rank(matrix.clone()).unwrap(), 1);
        assert_eq!(
            exact_matrix::matrix_determinant(&matrix).unwrap(),
            ExactRational::from(2)
        );
        let inverse = exact_matrix::invert_matrix(&matrix).unwrap();
        assert_eq!(
            exact_matrix::matrix_multiply(&matrix, &inverse).unwrap(),
            vec![vec![ExactRational::one()]]
        );
        assert_eq!(
            exact_matrix::matrix_transpose(&[
                vec![ExactRational::from(1), ExactRational::from(2)],
                vec![ExactRational::from(3), ExactRational::from(4)],
            ])
            .unwrap(),
            vec![
                vec![ExactRational::from(1), ExactRational::from(3)],
                vec![ExactRational::from(2), ExactRational::from(4)],
            ]
        );
    }

    #[test]
    fn atom_bridge_is_exactly_sufficient_for_parsing_and_views() {
        use symbolica_atom::{AtomCore as _, try_parse};

        let atom = try_parse!("x+1", default_namespace = "rustred_legacy_test").unwrap();
        assert!(atom.as_view().to_canonical_string().contains("x"));
    }
}
