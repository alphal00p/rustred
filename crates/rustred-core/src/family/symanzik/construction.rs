//! Topology-neutral construction of the Symanzik U, F, and G polynomials.

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::family::{IntegralFamily, ScalarProductCoordinate};

use super::context::FeynmanPolynomialContext;
use super::error::FeynmanPolynomialError;
use super::model::{FeynmanPolynomial, FeynmanPolynomialLimits};
use super::operations::{checked_adjugate, checked_determinant, verify_homogeneous};
use super::work::{FeynmanWorkBudget, checked_add, checked_mul};

/// Authenticated generic Symanzik data for one complete affine family.
#[derive(Debug)]
pub struct SymanzikPolynomials {
    context: FeynmanPolynomialContext,
    u: FeynmanPolynomial,
    f: FeynmanPolynomial,
    g: FeynmanPolynomial,
}

impl SymanzikPolynomials {
    pub fn try_from_family_with_limits(
        family: &IntegralFamily,
        limits: FeynmanPolynomialLimits,
    ) -> Result<Self, FeynmanPolynomialError> {
        catch_unwind(AssertUnwindSafe(|| Self::build(family, limits)))
            .map_err(|_| FeynmanPolynomialError::SymbolicaPanic)?
    }

    fn build(
        family: &IntegralFamily,
        limits: FeynmanPolynomialLimits,
    ) -> Result<Self, FeynmanPolynomialError> {
        let context = FeynmanPolynomialContext::try_new(family, limits)?;
        let mut work = FeynmanWorkBudget::new(limits);
        let loops = family.loop_count();
        let externals = family.external_count();
        let assembly_columns = checked_add(
            family.denominator_count(),
            1,
            "Feynman polynomial assembly entries",
        )?;
        let assembly_entries = checked_mul(
            family.denominator_count(),
            assembly_columns,
            "Feynman polynomial assembly entries",
        )?;
        work.charge_term_operations(assembly_entries)?;
        let mut a = vec![vec![context.zero(); loops]; loops];
        let mut q = vec![vec![context.zero(); externals]; loops];
        let mut c = context.zero();
        let half = context.coefficients.try_div(
            &context.coefficients.one(),
            &context.coefficients.integer(2),
            limits.exact_algebra,
        )?;

        for (denominator_index, denominator) in family.denominators().iter().enumerate() {
            let constant = context.parameter_monomial(denominator_index, denominator.constant())?;
            c = context.add(&c, &constant, &mut work)?;
            for (coordinate_index, coordinate) in family.coordinates().iter().enumerate() {
                let coefficient = &denominator.coefficients()[coordinate_index];
                if coefficient.is_zero() {
                    continue;
                }
                match *coordinate {
                    ScalarProductCoordinate::LoopLoop { left, right } => {
                        let coefficient = if left == right {
                            coefficient.clone()
                        } else {
                            context.coefficients.try_mul(
                                coefficient,
                                &half,
                                limits.exact_algebra,
                            )?
                        };
                        let monomial =
                            context.parameter_monomial(denominator_index, &coefficient)?;
                        a[left][right] = context.add(&a[left][right], &monomial, &mut work)?;
                        if left != right {
                            a[right][left] = context.add(&a[right][left], &monomial, &mut work)?;
                        }
                    }
                    ScalarProductCoordinate::LoopExternal {
                        loop_index,
                        external_index,
                    } => {
                        let coefficient = context.coefficients.try_mul(
                            coefficient,
                            &half,
                            limits.exact_algebra,
                        )?;
                        let monomial =
                            context.parameter_monomial(denominator_index, &coefficient)?;
                        q[loop_index][external_index] =
                            context.add(&q[loop_index][external_index], &monomial, &mut work)?;
                    }
                }
            }
        }

        let u = checked_determinant(&context, &a, &mut work)?;
        if u.is_zero() {
            let f = context.zero();
            let g = context.zero();
            return Ok(Self { context, u, f, g });
        }

        let momentum_square = if externals == 0 {
            context.zero()
        } else {
            let adjugate = checked_adjugate(&context, &a, &mut work)?;
            let mut momentum_square = context.zero();
            let loop_external_entries =
                checked_mul(loops, externals, "Feynman Gram-contraction entries")?;
            let gram_contraction_entries = checked_mul(
                loop_external_entries,
                loop_external_entries,
                "Feynman Gram-contraction entries",
            )?;
            work.charge_term_operations(gram_contraction_entries)?;
            for loop_left in 0..loops {
                for loop_right in 0..loops {
                    for external_left in 0..externals {
                        for external_right in 0..externals {
                            let gram = &family.external_gram()[external_left][external_right];
                            if gram.is_zero()
                                || q[loop_left][external_left].is_zero()
                                || adjugate[loop_left][loop_right].is_zero()
                                || q[loop_right][external_right].is_zero()
                            {
                                continue;
                            }
                            let product = context.mul(
                                &q[loop_left][external_left],
                                &adjugate[loop_left][loop_right],
                                &mut work,
                            )?;
                            let product =
                                context.mul(&product, &q[loop_right][external_right], &mut work)?;
                            let product = context.scale(&product, gram, &mut work)?;
                            momentum_square = context.add(&momentum_square, &product, &mut work)?;
                        }
                    }
                }
            }
            momentum_square
        };
        let uc = context.mul(&u, &c, &mut work)?;
        let f = context.sub(&uc, &momentum_square, &mut work)?;
        let g = context.add(&u, &f, &mut work)?;
        verify_homogeneous(&u, loops, "U")?;
        verify_homogeneous(&f, loops + 1, "F")?;
        context.authenticate(&g)?;
        Ok(Self { context, u, f, g })
    }

    pub fn context(&self) -> &FeynmanPolynomialContext {
        &self.context
    }

    pub fn u(&self) -> &FeynmanPolynomial {
        &self.u
    }

    pub fn f(&self) -> &FeynmanPolynomial {
        &self.f
    }

    pub fn g(&self) -> &FeynmanPolynomial {
        &self.g
    }

    /// Checked gradient of `G`, corresponding to LiteRed's cached
    /// `FeynParGdG` data.
    pub fn try_gradient(&self) -> Result<Vec<FeynmanPolynomial>, FeynmanPolynomialError> {
        self.context.try_gradient(&self.g)
    }
}
