//! Prospective monomial counting only; Symbolica still owns all coefficients,
//! polynomial multiplication, coalescing and cancellation.

use super::{MultiAffineNumeratorExpansionError, MultiAffineNumeratorFactor, multiset_support};

pub(super) struct PrefixSupport {
    used_variables: Vec<bool>,
    variables: usize,
    degree: u64,
    homogeneous: bool,
}

impl PrefixSupport {
    pub(super) fn try_new(arity: usize) -> Result<Self, MultiAffineNumeratorExpansionError> {
        let mut used_variables = Vec::new();
        used_variables.try_reserve_exact(arity).map_err(|_| {
            MultiAffineNumeratorExpansionError::AllocationFailure {
                resource: "multi-affine support variable incidence",
                requested: arity,
            }
        })?;
        used_variables.resize(arity, false);
        Ok(Self {
            used_variables,
            variables: 0,
            degree: 0,
            homogeneous: true,
        })
    }

    /// Input arity/context/constant shape and positive power have already been
    /// admitted. Row positions are exactly the native Temporary variable map;
    /// native coefficient zero tests determine incidence without new algebra.
    pub(super) fn include(
        &mut self,
        factor: &MultiAffineNumeratorFactor,
    ) -> Result<(), MultiAffineNumeratorExpansionError> {
        let mut has_variable = false;
        for (used, coefficient) in self
            .used_variables
            .iter_mut()
            .zip(factor.denominator_coefficients())
        {
            if !coefficient.is_zero() {
                has_variable = true;
                if !*used {
                    *used = true;
                    self.variables += 1;
                }
            }
        }
        if has_variable {
            self.degree = self.degree.checked_add(factor.power()).ok_or(
                MultiAffineNumeratorExpansionError::ResourceCountOverflow {
                    resource: "multi-affine prefix total degree",
                },
            )?;
            self.homogeneous &= factor.constant().is_zero();
        }
        Ok(())
    }

    pub(super) fn refine(&self, product_bound: usize) -> usize {
        if self.variables == 0 {
            return product_bound.min(1);
        }
        // Homogeneous degree D in U variables has C(D+U-1,U-1) possible
        // monomials; degree <=D has C(D+U,U). Both are independent upper
        // bounds, so take their minimum with the pair-product bound. Constant
        // factors have degree zero and do not break homogeneity.
        let Some(width) = self.variables.checked_add(usize::from(!self.homogeneous)) else {
            return product_bound;
        };
        match multiset_support(self.degree, width) {
            Ok(total_degree_bound) => product_bound.min(total_degree_bound),
            // The additional bound may overflow even when a sparse product
            // bound fits. Retain the original valid bound, never wrap it or
            // reject a previously admissible sparse product for that reason.
            Err(_) => product_bound,
        }
    }
}
