//! Exact denominator factorization over one interned basis of irreducible factors.
//!
//! Step coefficients of IBP rules and transports have denominators that are
//! products of few low-degree factors shared by many coefficients. Each
//! stored coefficient therefore keeps its denominator as an integer content
//! times powers of basis factors. The finite-field arithmetics can then map a
//! coefficient by evaluating every basis factor once per sample point or
//! prime, instead of every denominator. Every factorization is checked by
//! multiplying it back; an inexact result is never stored.
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use symbolica::domains::integer::{Integer, IntegerRing};
use symbolica::poly::factor::Factorize;
use symbolica::poly::polynomial::MultivariatePolynomial;

pub(crate) type Polynomial = MultivariatePolynomial<IntegerRing, u16>;

/// `denominator = content * prod(basis[id]^exponent)`. Basis factors are
/// primitive, irreducible over the integers, nonconstant and have a positive
/// leading coefficient; `factors` is sorted by basis id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FactoredDenominator {
    pub content: Integer,
    pub factors: Box<[(u32, u32)]>,
}

/// Interned irreducible denominator factors. Ids are stable for the lifetime
/// of one cache; files store their own sorted local tables.
#[derive(Debug, Default)]
pub(crate) struct FactorBasis {
    factors: Vec<Arc<Polynomial>>,
    index: HashMap<Arc<Polynomial>, u32>,
}

impl FactorBasis {
    pub(crate) fn len(&self) -> usize {
        self.factors.len()
    }

    pub(crate) fn get(&self, id: u32) -> Option<&Arc<Polynomial>> {
        self.factors.get(id as usize)
    }

    /// Intern one normalized factor. The caller guarantees normalization.
    pub(crate) fn intern(&mut self, factor: Polynomial) -> Result<u32, String> {
        if let Some(&id) = self.index.get(&factor) {
            return Ok(id);
        }
        let id = u32::try_from(self.factors.len())
            .map_err(|_| "ordered step cache factor basis exceeds u32 ids".to_owned())?;
        let factor = Arc::new(factor);
        self.factors.push(Arc::clone(&factor));
        self.index.insert(factor, id);
        Ok(id)
    }

    /// Factor `denominator` over this basis, extending it as needed.
    pub(crate) fn factor(
        &mut self,
        denominator: &Polynomial,
    ) -> Result<FactoredDenominator, String> {
        if denominator.is_zero() {
            return Err("a step coefficient has a zero denominator".into());
        }
        if denominator.is_constant() {
            return Ok(FactoredDenominator {
                content: denominator.get_constant(),
                factors: Box::new([]),
            });
        }
        let mut content = Integer::one();
        let mut exponents = BTreeMap::<u32, u32>::new();
        for (factor, power) in denominator.factor() {
            let power = u32::try_from(power)
                .map_err(|_| "a denominator factor power exceeds u32".to_owned())?;
            if factor.is_constant() {
                content *= &factor.get_constant().pow(u64::from(power));
                continue;
            }
            let factor_content = factor.content();
            let mut primitive = factor.div_coeff(&factor_content);
            let mut scale = factor_content;
            if primitive.lcoeff().is_negative() {
                primitive = -primitive;
                scale = -scale;
            }
            content *= &scale.pow(u64::from(power));
            let id = self.intern(primitive)?;
            let entry = exponents.entry(id).or_insert(0);
            *entry = entry
                .checked_add(power)
                .ok_or_else(|| "a denominator factor power overflows u32".to_owned())?;
        }
        let factored = FactoredDenominator {
            content,
            factors: exponents.into_iter().collect(),
        };
        if self.expand(&factored, denominator)? != *denominator {
            return Err("denominator factorization does not reproduce the denominator".into());
        }
        Ok(factored)
    }

    /// Multiply a factored denominator back out, using `template` for the
    /// variable map.
    pub(crate) fn expand(
        &self,
        factored: &FactoredDenominator,
        template: &Polynomial,
    ) -> Result<Polynomial, String> {
        let mut product = template.constant(factored.content.clone());
        for &(id, power) in factored.factors.iter() {
            let factor = self.get(id).ok_or_else(|| {
                "a factored denominator refers to an absent basis factor".to_owned()
            })?;
            product = &product * &factor.pow(power as usize);
        }
        Ok(product)
    }
}
