use symbolica::prelude::AtomView;

use crate::algebra::Coefficient;

use super::budget::{planned_coefficient_clone_census, planned_unit_coefficient_census};
use super::construction::check_limit;
use super::error::SymbolicaAffineDenominatorError;
use super::model::SymbolicaAffineDenominatorCompiler;
use super::projection::coefficient_contains_momentum;
use super::work::{ExactWorkBudget, ProjectionAllocationBudget};

pub(super) struct CheckedEvaluator<'a> {
    compiler: &'a SymbolicaAffineDenominatorCompiler,
    pub(super) arithmetic_operations: u64,
    pub(super) work: ExactWorkBudget,
    pub(super) projection_work: ProjectionAllocationBudget,
}

impl<'a> CheckedEvaluator<'a> {
    pub(super) fn new(compiler: &'a SymbolicaAffineDenominatorCompiler) -> Self {
        Self {
            compiler,
            arithmetic_operations: 0,
            work: ExactWorkBudget::default(),
            projection_work: ProjectionAllocationBudget::default(),
        }
    }

    pub(super) fn evaluate(
        &mut self,
        atom: AtomView<'_>,
        scalar_product_allowed: bool,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        match atom {
            AtomView::Num(_) => self.compiler.numeric_coefficient(atom),
            AtomView::Var(variable) => {
                let symbol = variable.get_symbol();
                let position = self
                    .compiler
                    .symbol_positions
                    .get(&symbol)
                    .copied()
                    .ok_or_else(|| {
                        SymbolicaAffineDenominatorError::UnknownSymbol(atom.to_owned())
                    })?;
                Ok(self.compiler.combined.parameter_at(position))
            }
            AtomView::Add(sum) => {
                let mut result = self.compiler.combined.zero();
                for child in sum.iter() {
                    let child = self.evaluate(child, scalar_product_allowed)?;
                    self.charge_arithmetic()?;
                    result = self.compiler.checked_add(&result, &child, &mut self.work)?;
                }
                Ok(result)
            }
            AtomView::Mul(product) => {
                let mut result = self.compiler.combined.one();
                for child in product.iter() {
                    let child = self.evaluate(child, scalar_product_allowed)?;
                    self.charge_arithmetic()?;
                    result = self.compiler.checked_mul(&result, &child, &mut self.work)?;
                }
                Ok(result)
            }
            AtomView::Pow(power) => {
                let exponent = i64::try_from(power.get_exp()).map_err(|_| {
                    SymbolicaAffineDenominatorError::UnsupportedPower(atom.to_owned())
                })?;
                let absolute = exponent.unsigned_abs();
                if absolute > u64::from(self.compiler.limits.max_abs_power) {
                    return Err(SymbolicaAffineDenominatorError::UnsupportedPower(
                        atom.to_owned(),
                    ));
                }
                let base = self.evaluate(power.get_base(), scalar_product_allowed)?;
                if exponent < 0 && coefficient_contains_momentum(&base, self.compiler.base_count())?
                {
                    return Err(SymbolicaAffineDenominatorError::NegativeMomentumPower {
                        atom: power.get_base().to_owned(),
                        exponent,
                    });
                }
                self.checked_power(&base, exponent)
            }
            AtomView::Fun(function) => {
                if function.get_symbol() != self.compiler.scalar_product {
                    return Err(SymbolicaAffineDenominatorError::UnsupportedFunction(
                        atom.to_owned(),
                    ));
                }
                if !scalar_product_allowed {
                    return Err(SymbolicaAffineDenominatorError::NestedScalarProduct(
                        atom.to_owned(),
                    ));
                }
                if function.get_nargs() != 2 {
                    return Err(SymbolicaAffineDenominatorError::MalformedScalarProduct {
                        atom: atom.to_owned(),
                        arguments: function.get_nargs(),
                    });
                }
                let mut arguments = function.iter();
                let left_atom = arguments.next().ok_or(
                    SymbolicaAffineDenominatorError::InternalVerificationFailure {
                        detail: "binary scalar product has no first argument",
                    },
                )?;
                let right_atom = arguments.next().ok_or(
                    SymbolicaAffineDenominatorError::InternalVerificationFailure {
                        detail: "binary scalar product has no second argument",
                    },
                )?;
                if arguments.next().is_some() {
                    return Err(
                        SymbolicaAffineDenominatorError::InternalVerificationFailure {
                            detail: "binary scalar product retained an extra argument",
                        },
                    );
                }
                let left = self.evaluate(left_atom, false)?;
                let right = self.evaluate(right_atom, false)?;
                self.compiler.validate_vector_linear(&left, 0, left_atom)?;
                self.compiler
                    .validate_vector_linear(&right, 1, right_atom)?;
                self.charge_arithmetic()?;
                let product = self.compiler.checked_mul(&left, &right, &mut self.work)?;
                self.compiler.contract_explicit_scalar_product(
                    product,
                    &mut self.work,
                    &mut self.projection_work,
                )
            }
        }
    }

    pub(super) fn checked_power(
        &mut self,
        base: &Coefficient,
        exponent: i64,
    ) -> Result<Coefficient, SymbolicaAffineDenominatorError> {
        let variables = self.compiler.combined.parameter_names().len();
        let unit_census = planned_unit_coefficient_census(variables)?;
        check_limit(
            "combined power-result integer bits",
            unit_census.integer_bits,
            self.compiler.limits.max_coefficient_integer_bits,
        )?;
        check_limit(
            "combined power-result retained bytes",
            unit_census.retained_bytes,
            self.compiler.limits.max_combined_retained_bytes,
        )?;
        if exponent == 0 {
            return Ok(self.compiler.combined.one());
        }

        self.compiler.combined.preflight_power_with_limits(
            base,
            exponent.unsigned_abs(),
            self.compiler.limits.exact_algebra,
        )?;

        let mut clone_census = unit_census;
        clone_census.checked_add_assign(
            planned_coefficient_clone_census(base, variables)?,
            "combined power base-clone census",
        )?;
        check_limit(
            "combined power base-clone integer bits",
            clone_census.integer_bits,
            self.compiler.limits.max_coefficient_integer_bits,
        )?;
        check_limit(
            "combined power base-clone retained bytes",
            clone_census.retained_bytes,
            self.compiler.limits.max_combined_retained_bytes,
        )?;
        let mut remaining = exponent.unsigned_abs();
        let mut result = self.compiler.combined.one();
        let mut factor = base.clone();
        while remaining != 0 {
            if remaining & 1 == 1 {
                self.charge_arithmetic()?;
                result = self
                    .compiler
                    .checked_mul(&result, &factor, &mut self.work)?;
            }
            remaining >>= 1;
            if remaining != 0 {
                self.charge_arithmetic()?;
                factor = self
                    .compiler
                    .checked_mul(&factor, &factor, &mut self.work)?;
            }
        }
        if exponent < 0 {
            self.charge_arithmetic()?;
            self.compiler
                .checked_div(&self.compiler.combined.one(), &result, &mut self.work)
        } else {
            Ok(result)
        }
    }

    fn charge_arithmetic(&mut self) -> Result<(), SymbolicaAffineDenominatorError> {
        self.arithmetic_operations = self.arithmetic_operations.checked_add(1).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "expression arithmetic operations",
            },
        )?;
        if self.arithmetic_operations > self.compiler.limits.max_arithmetic_operations {
            return Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "expression arithmetic operations",
                requested: u128::from(self.arithmetic_operations),
                limit: u128::from(self.compiler.limits.max_arithmetic_operations),
            });
        }
        Ok(())
    }
}
