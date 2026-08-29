use std::sync::Arc;

use symbolica::{
    atom::{NamespacedSymbol, SymbolBuilder},
    prelude::*,
};

use super::{
    Coefficient, CoefficientContextError, ExactAlgebraError, ExactAlgebraLimits,
    operations::{
        checked_coefficient_add_on_map, checked_coefficient_div_on_map,
        checked_coefficient_mul_on_map, checked_coefficient_neg_on_map,
        checked_coefficient_sub_on_map, preflight_power_degrees,
    },
    validation::validate_coefficient_on_map,
};
use crate::algebra::is_exact_plain_symbol;

const RUSTRED_NAMESPACE: &str = "rustred";

/// A shared Symbolica rational-polynomial coefficient domain.
#[derive(Clone, Debug)]
pub struct CoefficientContext {
    names: Arc<Vec<String>>,
    variables: Arc<Vec<PolyVariable>>,
    template: Coefficient,
}

impl CoefficientContext {
    /// Construct a validated Symbolica variable map without allowing malformed
    /// or duplicate caller labels to reach polynomial construction.
    pub fn try_new(
        parameter_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, CoefficientContextError> {
        let names: Vec<String> = parameter_names.into_iter().map(Into::into).collect();
        for (index, name) in names.iter().enumerate() {
            if names[..index].contains(name) {
                return Err(CoefficientContextError::DuplicateParameter(name.clone()));
            }
        }
        let mut variables = Vec::with_capacity(names.len());
        for name in &names {
            let qualified = format!("{RUSTRED_NAMESPACE}::{name}");
            let namespaced = NamespacedSymbol::try_parse(&qualified).ok_or_else(|| {
                CoefficientContextError::InvalidParameter {
                    name: name.clone(),
                    reason: "could not form a namespaced Symbolica symbol".to_owned(),
                }
            })?;
            let symbol = SymbolBuilder::new(namespaced).build().map_err(|reason| {
                CoefficientContextError::InvalidParameter {
                    name: name.clone(),
                    reason: reason.to_string(),
                }
            })?;
            if !is_exact_plain_symbol(symbol, &qualified) {
                return Err(CoefficientContextError::ParameterSymbolCollision {
                    name: name.clone(),
                });
            }
            variables.push(PolyVariable::Symbol(symbol));
        }
        let variables = Arc::new(variables);
        let template = RationalPolynomial::new(&Z, variables.clone());
        Ok(Self {
            names: Arc::new(names),
            variables,
            template,
        })
    }

    #[cfg(test)]
    pub(crate) fn new(parameter_names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::try_new(parameter_names).expect("test coefficient labels must be valid")
    }

    pub fn parameter_names(&self) -> &[String] {
        self.names.as_slice()
    }

    /// Whether two contexts use the exact same ordered Symbolica polynomial
    /// variable map and RustRed parameter labels.
    ///
    /// Matching names alone are not sufficient for safe coefficient
    /// composition. Higher-loop component services use this check before
    /// sharing a lower-loop reduction cache across authenticated families.
    pub fn has_same_variable_map(&self, other: &Self) -> bool {
        self.names == other.names && self.template.get_variables() == other.template.get_variables()
    }

    /// Whether `coefficient` uses exactly this context's ordered variable map
    /// in both polynomial parts.
    ///
    /// Symbolica normally unifies differing maps during arithmetic. Generic
    /// RustRed code uses this check before every proof-bearing composition so
    /// that an undeclared variable cannot be appended implicitly.
    pub fn contains(&self, coefficient: &Coefficient) -> bool {
        self.validate_with_limits(coefficient, ExactAlgebraLimits::default())
            .is_ok()
    }

    pub fn validate_with_limits(
        &self,
        coefficient: &Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<(), ExactAlgebraError> {
        validate_coefficient_on_map(coefficient, &self.variables, limits)
    }

    pub(crate) fn preflight_power_with_limits(
        &self,
        coefficient: &Coefficient,
        exponent: u64,
        limits: ExactAlgebraLimits,
    ) -> Result<(), ExactAlgebraError> {
        validate_coefficient_on_map(coefficient, &self.variables, limits)?;
        preflight_power_degrees(&coefficient.numerator, exponent, limits)?;
        preflight_power_degrees(&coefficient.denominator, exponent, limits)
    }

    pub(crate) fn variables(&self) -> &Arc<Vec<PolyVariable>> {
        &self.variables
    }

    pub(crate) fn template(&self) -> &Coefficient {
        &self.template
    }

    pub fn zero(&self) -> Coefficient {
        self.integer(0)
    }

    pub fn one(&self) -> Coefficient {
        self.integer(1)
    }

    pub fn integer(&self, value: i64) -> Coefficient {
        self.template
            .numerator
            .constant(Integer::from(value))
            .into()
    }

    pub fn parameter(&self, name: &str) -> Option<Coefficient> {
        self.names
            .iter()
            .position(|candidate| candidate == name)
            .map(|position| self.parameter_at(position))
    }

    pub(crate) fn parameter_at(&self, position: usize) -> Coefficient {
        self.template
            .numerator
            .variable(&self.variables[position])
            .expect("coefficient parameter is present in its own variable map")
            .into()
    }

    /// Checked exact addition for proof-bearing code.
    pub fn try_add(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<Coefficient, ExactAlgebraError> {
        checked_coefficient_add_on_map(left, right, &self.variables, limits)
    }

    /// Checked exact subtraction for proof-bearing code.
    pub fn try_sub(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<Coefficient, ExactAlgebraError> {
        checked_coefficient_sub_on_map(left, right, &self.variables, limits)
    }

    /// Checked exact multiplication for proof-bearing code.
    pub fn try_mul(
        &self,
        left: &Coefficient,
        right: &Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<Coefficient, ExactAlgebraError> {
        checked_coefficient_mul_on_map(left, right, &self.variables, limits)
    }

    /// Checked exact division for proof-bearing code.
    pub fn try_div(
        &self,
        numerator: &Coefficient,
        denominator: &Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<Coefficient, ExactAlgebraError> {
        checked_coefficient_div_on_map(numerator, denominator, &self.variables, limits)
    }

    /// Checked exact negation for proof-bearing code.
    pub fn try_neg(
        &self,
        value: &Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<Coefficient, ExactAlgebraError> {
        checked_coefficient_neg_on_map(value, &self.variables, limits)
    }

    #[cfg(test)]
    pub(crate) fn coefficient_fixture(&self, expression: &str) -> Coefficient {
        let atom = try_parse!(expression, default_namespace = RUSTRED_NAMESPACE)
            .expect("test coefficient must parse");
        let coefficient = atom
            .as_view()
            .try_to_rational_polynomial(&Q, &Z, Some(self.variables.clone()))
            .expect("test coefficient must be rational-polynomial");
        self.validate_with_limits(&coefficient, ExactAlgebraLimits::default())
            .expect("test coefficient must use the declared context");
        coefficient
    }
}
