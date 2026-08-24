//! Canonical products of stable master identifiers and exact linear
//! combinations over those products.
//!
//! Factorized multiloop sectors do not naturally end in a single integral of
//! the parent family.  For example, a four-loop corner can reduce to a
//! tadpole times a three-loop master or to two sunset masters.  This module
//! provides the small commutative algebra needed to represent those answers
//! without assigning an arbitrary parent-family integral to each product.
//!
//! The identifier type is deliberately generic.  Callers must use a stable,
//! canonical identifier (for example, a family fingerprint paired with a
//! master exponent vector), never a process-local symbol or insertion index.

use std::collections::BTreeMap;
use std::fmt;

use crate::Coefficient;

/// A canonical commutative product of master identifiers.
///
/// Factors are sorted by identifier and stored once with a positive
/// multiplicity.  The empty map is the multiplicative identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MasterProduct<Id> {
    factors: BTreeMap<Id, u32>,
}

impl<Id> Default for MasterProduct<Id> {
    fn default() -> Self {
        Self {
            factors: BTreeMap::new(),
        }
    }
}

impl<Id> MasterProduct<Id> {
    /// The empty master product, representing the scalar factor one.
    pub fn identity() -> Self {
        Self::default()
    }

    pub fn factors(&self) -> &BTreeMap<Id, u32> {
        &self.factors
    }

    pub fn is_identity(&self) -> bool {
        self.factors.is_empty()
    }

    pub fn distinct_factor_count(&self) -> usize {
        self.factors.len()
    }

    /// Total multiplicity, returned in a width that cannot overflow for a
    /// collection whose length fits in memory and whose entries are `u32`.
    pub fn total_factor_count(&self) -> u128 {
        self.factors.values().map(|&value| u128::from(value)).sum()
    }
}

impl<Id: Ord> MasterProduct<Id> {
    pub fn from_factor(factor: Id) -> Self {
        Self {
            factors: BTreeMap::from([(factor, 1)]),
        }
    }

    /// Canonicalize a stream of factors, checking repeated-factor
    /// multiplicity instead of allowing a release-build integer wrap.
    pub fn try_from_factors(
        factors: impl IntoIterator<Item = Id>,
    ) -> Result<Self, MasterProductError> {
        Self::try_from_multiplicities(factors.into_iter().map(|factor| (factor, 1)))
    }

    /// Canonicalize `(identifier, multiplicity)` pairs.
    ///
    /// Zero multiplicities are ignored.  Repeated identifiers are merged with
    /// checked addition.
    pub fn try_from_multiplicities(
        factors: impl IntoIterator<Item = (Id, u32)>,
    ) -> Result<Self, MasterProductError> {
        let mut canonical = BTreeMap::<Id, u32>::new();
        for (factor, multiplicity) in factors {
            if multiplicity == 0 {
                continue;
            }
            if let Some(current) = canonical.get_mut(&factor) {
                let previous = *current;
                *current = previous.checked_add(multiplicity).ok_or(
                    MasterProductError::MultiplicityOverflow {
                        current: previous,
                        added: multiplicity,
                    },
                )?;
            } else {
                canonical.insert(factor, multiplicity);
            }
        }
        Ok(Self { factors: canonical })
    }

    pub fn multiplicity(&self, factor: &Id) -> u32 {
        self.factors.get(factor).copied().unwrap_or(0)
    }
}

impl<Id: Ord + Clone> MasterProduct<Id> {
    /// Multiply two commutative products, checking factor multiplicities.
    pub fn checked_multiply(&self, other: &Self) -> Result<Self, MasterProductError> {
        let mut factors = self.factors.clone();
        for (factor, &multiplicity) in &other.factors {
            if let Some(current) = factors.get_mut(factor) {
                let previous = *current;
                *current = previous.checked_add(multiplicity).ok_or(
                    MasterProductError::MultiplicityOverflow {
                        current: previous,
                        added: multiplicity,
                    },
                )?;
            } else {
                factors.insert(factor.clone(), multiplicity);
            }
        }
        Ok(Self { factors })
    }
}

impl<Id: fmt::Display> fmt::Display for MasterProduct<Id> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.factors.is_empty() {
            return formatter.write_str("1");
        }
        for (position, (factor, multiplicity)) in self.factors.iter().enumerate() {
            if position != 0 {
                formatter.write_str("*")?;
            }
            write!(formatter, "{factor}")?;
            if *multiplicity != 1 {
                write!(formatter, "^{multiplicity}")?;
            }
        }
        Ok(())
    }
}

/// Construction error for [`MasterProduct`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MasterProductError {
    MultiplicityOverflow { current: u32, added: u32 },
}

impl fmt::Display for MasterProductError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MultiplicityOverflow { current, added } => write!(
                formatter,
                "master-factor multiplicity overflow while adding {added} to {current}"
            ),
        }
    }
}

impl std::error::Error for MasterProductError {}

/// An exact sparse linear combination of canonical master products.
///
/// As in RustRed's integral-valued linear combination, zero coefficients are
/// never retained and exact cancellation removes the corresponding key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductLinearCombination<Id> {
    terms: BTreeMap<MasterProduct<Id>, Coefficient>,
}

impl<Id> Default for ProductLinearCombination<Id> {
    fn default() -> Self {
        Self {
            terms: BTreeMap::new(),
        }
    }
}

impl<Id: Ord> ProductLinearCombination<Id> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_term(product: MasterProduct<Id>, coefficient: Coefficient) -> Self {
        let mut result = Self::new();
        result.add_term(product, coefficient);
        result
    }

    pub fn terms(&self) -> &BTreeMap<MasterProduct<Id>, Coefficient> {
        &self.terms
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn len(&self) -> usize {
        self.terms.len()
    }

    pub fn coefficient(&self, product: &MasterProduct<Id>) -> Option<&Coefficient> {
        self.terms.get(product)
    }

    pub fn add_term(&mut self, product: MasterProduct<Id>, coefficient: Coefficient) {
        if coefficient.is_zero() {
            return;
        }
        if let Some(current) = self.terms.get_mut(&product) {
            let sum = &*current + &coefficient;
            if sum.is_zero() {
                self.terms.remove(&product);
            } else {
                *current = sum;
            }
        } else {
            self.terms.insert(product, coefficient);
        }
    }

    pub fn remove(&mut self, product: &MasterProduct<Id>) -> Option<Coefficient> {
        self.terms.remove(product)
    }

    pub fn add_scaled(&mut self, other: &Self, factor: &Coefficient)
    where
        Id: Clone,
    {
        if factor.is_zero() {
            return;
        }
        for (product, coefficient) in &other.terms {
            self.add_term(product.clone(), coefficient * factor);
        }
    }

    pub fn scaled(&self, factor: &Coefficient) -> Self
    where
        Id: Clone,
    {
        let mut result = Self::new();
        result.add_scaled(self, factor);
        result
    }
}

impl<Id: Ord + Clone> ProductLinearCombination<Id> {
    /// Distribute and multiply two sparse product-valued combinations.
    ///
    /// Compatibility wrapper which bounds distinct output terms.  New
    /// reduction code should use [`Self::checked_convolve_with_limits`] to
    /// bound pair operations as well.
    pub fn checked_convolve(
        &self,
        other: &Self,
        max_terms: usize,
    ) -> Result<Self, ProductConvolutionError> {
        self.checked_convolve_with_limits(other, max_terms, u128::MAX)
    }

    /// Distribute and multiply two sparse product-valued combinations with
    /// independent bounds on distinct output terms and Cartesian pair
    /// operations.
    ///
    /// The pair count is known before coefficient work begins, so an
    /// insufficient operation budget fails without constructing a partial
    /// result.  `max_terms` is checked incrementally after exact cancellation:
    /// colliding product keys therefore consume pair operations but only one
    /// output term.
    pub fn checked_convolve_with_limits(
        &self,
        other: &Self,
        max_terms: usize,
        max_pair_operations: u128,
    ) -> Result<Self, ProductConvolutionError> {
        let requested_operations = (self.len() as u128).saturating_mul(other.len() as u128);
        if requested_operations > max_pair_operations {
            return Err(ProductConvolutionError::PairOperationLimit {
                limit: max_pair_operations,
                attempted: requested_operations,
            });
        }
        let mut result = Self::new();
        for (left_product, left_coefficient) in &self.terms {
            for (right_product, right_coefficient) in &other.terms {
                let product = left_product.checked_multiply(right_product)?;
                result.add_term(product, left_coefficient * right_coefficient);
                if result.len() > max_terms {
                    return Err(ProductConvolutionError::TermLimit {
                        limit: max_terms,
                        attempted: result.len(),
                    });
                }
            }
        }
        Ok(result)
    }
}

/// Checked convolution failure for [`ProductLinearCombination`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProductConvolutionError {
    FactorMultiplicity(MasterProductError),
    TermLimit { limit: usize, attempted: usize },
    PairOperationLimit { limit: u128, attempted: u128 },
}

impl From<MasterProductError> for ProductConvolutionError {
    fn from(error: MasterProductError) -> Self {
        Self::FactorMultiplicity(error)
    }
}

impl fmt::Display for ProductConvolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FactorMultiplicity(error) => error.fmt(formatter),
            Self::TermLimit { limit, attempted } => write!(
                formatter,
                "master-product convolution attempted {attempted} distinct terms, exceeding limit {limit}"
            ),
            Self::PairOperationLimit { limit, attempted } => write!(
                formatter,
                "master-product convolution requires {attempted} pair operations, exceeding limit {limit}"
            ),
        }
    }
}

impl std::error::Error for ProductConvolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::FactorMultiplicity(error) => Some(error),
            Self::TermLimit { .. } | Self::PairOperationLimit { .. } => None,
        }
    }
}
