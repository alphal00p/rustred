//! Bridge between Lorentz tensor projection and scalar integral reduction.
//!
//! [`VacuumTensorProjector`](crate::VacuumTensorProjector) leaves contracted
//! loop momenta as scalar-product monomials.  This module expands those
//! monomials in a [`VacuumFamily`](crate::VacuumFamily)'s complete denominator
//! basis, lowering denominator powers in the corresponding scalar integral.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::{
    Coefficient, FamilyError, Integral, LinearCombination, MetricPairing, ReductionError,
    ReductionTable, TensorReduction, VacuumFamily,
};

/// Default cap on distinct denominator monomials produced while expanding one
/// projected tensor term.
pub const DEFAULT_MAX_TENSOR_EXPANSION_TERMS: usize = 1_000_000;
/// Default cap on affine-monomial multiplications during one lowering call.
pub const DEFAULT_MAX_TENSOR_EXPANSION_OPERATIONS: u64 = 10_000_000;

/// A tensor result whose loop scalar products have been converted into scalar
/// integrals, grouped by the remaining free-index metric structure.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TensorIntegralReduction {
    structures: BTreeMap<MetricPairing, LinearCombination>,
}

impl TensorIntegralReduction {
    pub fn structures(&self) -> &BTreeMap<MetricPairing, LinearCombination> {
        &self.structures
    }

    pub fn coefficient(&self, metrics: &MetricPairing) -> Option<&LinearCombination> {
        self.structures.get(metrics)
    }

    pub fn is_zero(&self) -> bool {
        self.structures.is_empty()
    }

    pub fn len(&self) -> usize {
        self.structures.len()
    }

    /// Apply a generic scalar IBP table to every free-index structure.
    pub fn reduce_with_table(&self, table: &ReductionTable) -> Result<Self, TensorFamilyError> {
        let mut structures = BTreeMap::new();
        for (metrics, combination) in &self.structures {
            let reduced = table.reduce_combination(combination)?;
            if !reduced.is_zero() {
                structures.insert(metrics.clone(), reduced);
            }
        }
        Ok(Self { structures })
    }
}

/// Expands projected vacuum-tensor scalar products in one denominator family.
#[derive(Clone, Debug)]
pub struct TensorFamilyReducer<'family> {
    family: &'family VacuumFamily,
    max_expansion_terms: usize,
    max_expansion_operations: u64,
}

impl<'family> TensorFamilyReducer<'family> {
    pub fn new(family: &'family VacuumFamily) -> Self {
        Self {
            family,
            max_expansion_terms: DEFAULT_MAX_TENSOR_EXPANSION_TERMS,
            max_expansion_operations: DEFAULT_MAX_TENSOR_EXPANSION_OPERATIONS,
        }
    }

    pub fn with_max_expansion_terms(mut self, max_expansion_terms: usize) -> Self {
        self.max_expansion_terms = max_expansion_terms;
        self
    }

    pub fn family(&self) -> &'family VacuumFamily {
        self.family
    }

    pub fn max_expansion_terms(&self) -> usize {
        self.max_expansion_terms
    }

    pub fn with_max_expansion_operations(mut self, max_expansion_operations: u64) -> Self {
        self.max_expansion_operations = max_expansion_operations;
        self
    }

    pub fn max_expansion_operations(&self) -> u64 {
        self.max_expansion_operations
    }

    /// Lower every scalar-product monomial in `tensor`, using `base_integral`
    /// as the denominator powers multiplying the tensor numerator.
    ///
    /// The returned integrals are deliberately not symmetry-canonicalized;
    /// the downstream reduction table or specialized boundary reducer owns
    /// that policy.
    pub fn lower(
        &self,
        base_integral: &Integral,
        tensor: &TensorReduction,
    ) -> Result<TensorIntegralReduction, TensorFamilyError> {
        if base_integral.powers().len() != self.family.denominator_count() {
            return Err(TensorFamilyError::WrongIntegralArity {
                expected: self.family.denominator_count(),
                actual: base_integral.powers().len(),
            });
        }
        if self.max_expansion_terms == 0 {
            return Err(TensorFamilyError::ExpansionLimit {
                limit: 0,
                attempted: 1,
            });
        }

        let mut structures = BTreeMap::<MetricPairing, LinearCombination>::new();
        let mut operations = 0_u64;
        for term in tensor.terms() {
            let mut polynomial = BTreeMap::<Vec<u32>, Coefficient>::from([(
                vec![0; self.family.denominator_count()],
                self.family.coefficients().one(),
            )]);

            for (&scalar_product, &exponent) in term.scalar_products().factors() {
                let expansion = self.family.scalar_product_expansion(
                    usize::from(scalar_product.left().id()),
                    usize::from(scalar_product.right().id()),
                )?;
                let affine_terms = u64::try_from(
                    usize::from(!expansion.constant().is_zero())
                        + expansion
                            .denominator_coefficients()
                            .iter()
                            .filter(|coefficient| !coefficient.is_zero())
                            .count(),
                )
                .unwrap_or(u64::MAX);
                for _ in 0..exponent {
                    let iteration_operations = u64::try_from(polynomial.len())
                        .unwrap_or(u64::MAX)
                        .saturating_mul(affine_terms);
                    operations = operations.saturating_add(iteration_operations);
                    if operations > self.max_expansion_operations {
                        return Err(TensorFamilyError::OperationLimit {
                            limit: self.max_expansion_operations,
                            attempted: operations,
                        });
                    }
                    let mut next = BTreeMap::new();
                    for (shifts, coefficient) in &polynomial {
                        if !expansion.constant().is_zero() {
                            add_polynomial_term(
                                &mut next,
                                shifts.clone(),
                                coefficient * expansion.constant(),
                            );
                            check_term_limit(&next, self.max_expansion_terms)?;
                        }
                        for (denominator, basis_coefficient) in
                            expansion.denominator_coefficients().iter().enumerate()
                        {
                            if basis_coefficient.is_zero() {
                                continue;
                            }
                            let mut shifted = shifts.clone();
                            shifted[denominator] = shifted[denominator]
                                .checked_add(1)
                                .ok_or(TensorFamilyError::ExponentOverflow)?;
                            let basis_coefficient =
                                self.family.coefficients().rational(basis_coefficient);
                            add_polynomial_term(
                                &mut next,
                                shifted,
                                coefficient * &basis_coefficient,
                            );
                            check_term_limit(&next, self.max_expansion_terms)?;
                        }
                    }
                    polynomial = next;
                }
            }

            let combination = structures.entry(term.metrics().clone()).or_default();
            for (shifts, coefficient) in polynomial {
                let mut powers = Vec::with_capacity(shifts.len());
                for (&power, shift) in base_integral.powers().iter().zip(shifts) {
                    let shift =
                        i32::try_from(shift).map_err(|_| TensorFamilyError::ExponentOverflow)?;
                    powers.push(
                        power
                            .checked_sub(shift)
                            .ok_or(TensorFamilyError::ExponentOverflow)?,
                    );
                }
                combination.add_term(Integral::new(powers), term.coefficient() * &coefficient);
            }
        }
        structures.retain(|_, combination| !combination.is_zero());
        Ok(TensorIntegralReduction { structures })
    }
}

fn add_polynomial_term(
    polynomial: &mut BTreeMap<Vec<u32>, Coefficient>,
    monomial: Vec<u32>,
    coefficient: Coefficient,
) {
    if coefficient.is_zero() {
        return;
    }
    if let Some(current) = polynomial.get_mut(&monomial) {
        let sum = &*current + &coefficient;
        if sum.is_zero() {
            polynomial.remove(&monomial);
        } else {
            *current = sum;
        }
    } else {
        polynomial.insert(monomial, coefficient);
    }
}

fn check_term_limit(
    polynomial: &BTreeMap<Vec<u32>, Coefficient>,
    limit: usize,
) -> Result<(), TensorFamilyError> {
    if polynomial.len() > limit {
        Err(TensorFamilyError::ExpansionLimit {
            limit,
            attempted: polynomial.len(),
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TensorFamilyError {
    Family(FamilyError),
    Reduction(ReductionError),
    WrongIntegralArity { expected: usize, actual: usize },
    ExpansionLimit { limit: usize, attempted: usize },
    OperationLimit { limit: u64, attempted: u64 },
    ExponentOverflow,
}

impl fmt::Display for TensorFamilyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Family(error) => error.fmt(formatter),
            Self::Reduction(error) => error.fmt(formatter),
            Self::WrongIntegralArity { expected, actual } => write!(
                formatter,
                "tensor base integral has {actual} powers; expected {expected}"
            ),
            Self::ExpansionLimit { limit, attempted } => write!(
                formatter,
                "tensor denominator expansion would contain {attempted} terms (limit {limit})"
            ),
            Self::OperationLimit { limit, attempted } => write!(
                formatter,
                "tensor denominator expansion requires at least {attempted} operations (limit {limit})"
            ),
            Self::ExponentOverflow => formatter.write_str(
                "tensor numerator degree cannot be represented by RustRed integral exponents",
            ),
        }
    }
}

impl Error for TensorFamilyError {}

impl From<FamilyError> for TensorFamilyError {
    fn from(value: FamilyError) -> Self {
        Self::Family(value)
    }
}

impl From<ReductionError> for TensorFamilyError {
    fn from(value: ReductionError) -> Self {
        Self::Reduction(value)
    }
}
