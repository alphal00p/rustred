use std::collections::BTreeMap;

use rustred::legacy_oracle_support::symbolica_atom::{Atom, Symbol};

use super::Integral;
use rustred::Coefficient;

/// A sparse linear combination of integrals with exact Symbolica coefficients.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinearCombination {
    terms: BTreeMap<Integral, Coefficient>,
}

impl LinearCombination {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_term(integral: Integral, coefficient: Coefficient) -> Self {
        let mut result = Self::new();
        result.add_term(integral, coefficient);
        result
    }

    pub fn terms(&self) -> &BTreeMap<Integral, Coefficient> {
        &self.terms
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn len(&self) -> usize {
        self.terms.len()
    }

    pub fn coefficient(&self, integral: &Integral) -> Option<&Coefficient> {
        self.terms.get(integral)
    }

    pub fn add_term(&mut self, integral: Integral, coefficient: Coefficient) {
        if coefficient.is_zero() {
            return;
        }

        if let Some(current) = self.terms.get_mut(&integral) {
            let sum = &*current + &coefficient;
            if sum.is_zero() {
                self.terms.remove(&integral);
            } else {
                *current = sum;
            }
        } else {
            self.terms.insert(integral, coefficient);
        }
    }

    pub fn remove(&mut self, integral: &Integral) -> Option<Coefficient> {
        self.terms.remove(integral)
    }

    pub fn add_scaled(&mut self, other: &Self, factor: &Coefficient) {
        if factor.is_zero() {
            return;
        }
        for (integral, coefficient) in &other.terms {
            self.add_term(integral.clone(), coefficient * factor);
        }
    }

    pub fn scaled(&self, factor: &Coefficient) -> Self {
        let mut result = Self::new();
        result.add_scaled(self, factor);
        result
    }

    pub fn to_atom(&self, integral_symbol: Symbol) -> Atom {
        self.terms
            .iter()
            .fold(Atom::num(0), |sum, (integral, coefficient)| {
                sum + coefficient.to_expression() * integral.to_atom(integral_symbol)
            })
    }
}
