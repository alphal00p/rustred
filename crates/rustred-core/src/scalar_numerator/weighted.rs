//! Exact collection of complete physical prefactors, before entry admission.
pub(super) mod bounds;
mod denominator;
pub(super) mod polynomial;

use std::collections::{BTreeMap, BTreeSet};
use symbolica::atom::{Atom, AtomCore, AtomView};
use symbolica::prelude::{CoefficientView, Q, Rational, Z};

use super::error::{ScalarNumeratorError as Error, check_limit, checked_add};
use super::model::{
    LoweredScalarNumeratorTerm, ScalarNumeratorLimits, ScalarNumeratorLowering,
    WeightedScalarNumeratorLowering,
};
use super::service::PreparedScalarProducts;
use super::syntax::validate_scalar_syntax;
use super::syntax::{ExactComparisonBudget, census_atom, contains_exact_atom};
use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraError};
use crate::family::{IntegralFamily, IntegralKey};

fn native<T>(operation: &'static str, run: impl FnOnce() -> T) -> Result<T, Error> {
    symbolica::utils::resources::checkpoint().map_err(ExactAlgebraError::Resource)?;
    let value = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run))
        .map_err(|_| Error::SymbolicaPanic { operation })?;
    symbolica::utils::resources::checkpoint().map_err(ExactAlgebraError::Resource)?;
    Ok(value)
}

struct Conditions {
    values: BTreeSet<Atom>,
    nodes: usize,
    limits: ScalarNumeratorLimits,
}
impl Conditions {
    fn retain(&mut self, condition: Atom) -> Result<(), Error> {
        if condition.is_zero() {
            return Err(Error::InvalidWeightedInput {
                detail: "a required nonzero scalar or mass vanishes",
            });
        }
        if matches!(condition.as_view(), AtomView::Num(_)) || self.values.contains(&condition) {
            return Ok(());
        }
        self.nodes = checked_add(
            "weighted lowering guard nodes",
            self.nodes,
            census_atom(
                condition.as_view(),
                "weighted lowering guard nodes",
                self.limits.max_output_atom_nodes,
                self.limits,
            )?,
        )?;
        check_limit(
            "weighted lowering guard nodes",
            self.nodes,
            self.limits.max_output_atom_nodes,
            self.limits.diagnostics_enabled,
        )?;
        self.values.insert(condition);
        Ok(())
    }

    /// Preserve poles visible in the received expression before native
    /// collection can cancel them. Function arguments remain caller-owned,
    /// but any rational pole occurring inside them is retained as well.
    fn scan(&mut self, expression: AtomView<'_>) -> Result<(), Error> {
        let mut pending = vec![expression];
        while let Some(part) = pending.pop() {
            symbolica::utils::resources::checkpoint().map_err(ExactAlgebraError::Resource)?;
            match part {
                AtomView::Num(number) => match number.get_coeff_view() {
                    CoefficientView::Natural(..) | CoefficientView::Large(..) => {}
                    _ => {
                        return Err(Error::InvalidWeightedInput {
                            detail: "weighted lowering requires finite exact rational or complex-rational numeric atoms",
                        });
                    }
                },
                AtomView::Var(_) => {}
                AtomView::Fun(function) => pending.extend(function.iter()),
                AtomView::Mul(product) => pending.extend(product.iter()),
                AtomView::Add(sum) => pending.extend(sum.iter()),
                AtomView::Pow(power) => {
                    if Rational::try_from(power.get_exp())
                        .is_ok_and(|exponent| exponent.is_negative())
                    {
                        self.retain(power.get_base().to_owned())?;
                    }
                    pending.push(power.get_exp());
                    pending.push(power.get_base());
                }
            }
        }
        Ok(())
    }
}

/// Inspect only the received atoms. In particular this precedes caller-side
/// variable substitutions which might cancel a visible input denominator.
pub(super) fn input_nonzero_conditions(
    prepared: &PreparedScalarProducts,
    family: &IntegralFamily,
    numerator: &Atom,
    mass_squared: &Atom,
) -> Result<Vec<Atom>, Error> {
    let (conditions, _) = collect_input_conditions(prepared, family, numerator, mass_squared)?;
    Ok(conditions.values.into_iter().collect())
}

fn collect_input_conditions(
    prepared: &PreparedScalarProducts,
    family: &IntegralFamily,
    numerator: &Atom,
    mass_squared: &Atom,
) -> Result<(Conditions, ExactComparisonBudget), Error> {
    let limits = prepared.limits;
    for (expression, resource) in [
        (numerator, "weighted lowering input nodes"),
        (mass_squared, "weighted lowering mass nodes"),
    ] {
        census_atom(
            expression.as_view(),
            resource,
            limits.max_input_nodes,
            limits,
        )?;
    }
    let mut comparisons = ExactComparisonBudget::new(
        limits.max_loop_momentum_label_checks,
        limits.diagnostics_enabled,
    );
    for momentum in &prepared.loop_momenta {
        if contains_exact_atom(mass_squared.as_view(), momentum.as_view(), &mut comparisons)? {
            return Err(Error::InvalidWeightedInput {
                detail: "the common mass depends on an integrated loop momentum",
            });
        }
    }
    let mut conditions = Conditions {
        values: BTreeSet::new(),
        nodes: 0,
        limits,
    };
    conditions.scan(numerator.as_view())?;
    conditions.scan(mass_squared.as_view())?;
    conditions.retain(mass_squared.clone())?;
    for condition in family.domain().conditions() {
        conditions.retain(condition.polynomial().to_expression())?;
    }
    Ok((conditions, comparisons))
}

pub(super) fn lower_weighted(
    prepared: &PreparedScalarProducts,
    family: &IntegralFamily,
    numerator: &Atom,
    base_integral: &IntegralKey,
    mass_squared: &Atom,
) -> Result<WeightedScalarNumeratorLowering, Error> {
    let mut work = bounds::Work::new(prepared.limits);
    lower_weighted_in_request(
        prepared,
        family,
        numerator,
        base_integral,
        mass_squared,
        &mut work,
    )
}

pub(super) fn lower_weighted_in_request(
    prepared: &PreparedScalarProducts,
    family: &IntegralFamily,
    numerator: &Atom,
    base_integral: &IntegralKey,
    mass_squared: &Atom,
    work: &mut bounds::Work,
) -> Result<WeightedScalarNumeratorLowering, Error> {
    let limits = prepared.limits;
    if base_integral.powers().len() != family.denominator_count() {
        return Err(Error::WrongIntegralKeyArity {
            expected: family.denominator_count(),
            actual: base_integral.powers().len(),
        });
    }
    let (mut conditions, mut comparisons) =
        collect_input_conditions(prepared, family, numerator, mass_squared)?;
    validate_scalar_syntax(
        numerator.as_view(),
        prepared.dot_head,
        &prepared.loop_momenta,
        &mut comparisons,
    )?;
    #[cfg(test)]
    work.set_diagnostic_phase("coordinate translation/admission");
    let translated = denominator::translate(
        prepared,
        family,
        numerator,
        mass_squared,
        work,
        &mut comparisons,
    )?;
    let polynomial = polynomial::collect(prepared, &translated)?;
    check_limit(
        "weighted denominator polynomial terms",
        polynomial.coefficients.len(),
        limits.max_polynomial_terms,
        limits.diagnostics_enabled,
    )?;
    let context = family.coefficient_context();
    let mut weights = BTreeMap::<IntegralKey, Atom>::new();
    let mut retained_nodes = 0usize;
    for term in &polynomial {
        denominator::validate_coefficient(prepared, term.coefficient, &mut comparisons)?;
        conditions.scan(term.coefficient.as_view())?;
        let powers = base_integral
            .powers()
            .iter()
            .zip(term.exponents)
            .enumerate()
            .map(|(denominator, (&power, &exponent))| {
                power
                    .checked_sub(i64::from(exponent))
                    .ok_or(Error::IntegralPowerUnderflow { denominator, power })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let integral = IntegralKey::try_new(powers)?;
        let sum = term.coefficient.clone();
        let nodes = census_atom(
            sum.as_view(),
            "weighted lowering retained nodes",
            limits.max_output_atom_nodes,
            limits,
        )?;
        retained_nodes = checked_add("weighted lowering retained nodes", retained_nodes, nodes)?;
        check_limit(
            "weighted lowering retained nodes",
            retained_nodes,
            limits.max_output_atom_nodes,
            limits.diagnostics_enabled,
        )?;
        weights.insert(integral, sum);
    }
    let mut terms = Vec::new();
    let mut output_nodes = 0;
    for (integral, weight) in weights {
        #[cfg(test)]
        {
            work.capture_diagnostic_weight(&integral, &weight);
            work.set_diagnostic_phase("common-factor collection");
        }
        work.preflight_collection(weight.as_view())?;
        let weight = native("weighted common-factor collection", || {
            weight.collect_factors()
        })?;
        census_atom(
            weight.as_view(),
            "weighted collected-factor nodes",
            limits.max_output_atom_nodes,
            limits,
        )?;
        // In particular, a cancelled rational prefactor must not be refused
        // using the expanded support estimate of its uncollected denominators.
        if weight.is_zero() {
            continue;
        }
        #[cfg(test)]
        work.set_diagnostic_phase("rational collection");
        work.preflight(weight.as_view())?;
        let weight = native("weighted rational collection", || weight.together())?;
        census_atom(
            weight.as_view(),
            "weighted lowering collected nodes",
            limits.max_output_atom_nodes,
            limits,
        )?;
        conditions.scan(weight.as_view())?;
        if weight.is_zero() {
            continue;
        }
        #[cfg(test)]
        work.set_diagnostic_phase("base coefficient extraction");
        let (coefficient, scalar_spectator) = split_coefficient(context, weight, limits, work)?;
        if coefficient.is_zero() {
            continue;
        }
        output_nodes = checked_add(
            "weighted lowering output nodes",
            output_nodes,
            census_atom(
                scalar_spectator.as_view(),
                "weighted lowering output nodes",
                limits.max_output_atom_nodes,
                limits,
            )?,
        )?;
        output_nodes = checked_add(
            "weighted lowering output nodes",
            output_nodes,
            census_atom(
                coefficient.to_expression().as_view(),
                "weighted lowering output nodes",
                limits.max_output_atom_nodes,
                limits,
            )?,
        )?;
        check_limit(
            "weighted lowering output nodes",
            checked_add(
                "weighted lowering output nodes",
                output_nodes,
                conditions.nodes,
            )?,
            limits.max_output_atom_nodes,
            limits.diagnostics_enabled,
        )?;
        check_limit(
            "lowered scalar-numerator terms",
            checked_add("lowered scalar-numerator terms", terms.len(), 1)?,
            limits.max_lowered_terms,
            limits.diagnostics_enabled,
        )?;
        terms.push(LoweredScalarNumeratorTerm {
            coefficient,
            scalar_spectator,
            integral,
            common_mass_squared_power: 0,
        });
    }
    Ok(WeightedScalarNumeratorLowering {
        lowering: ScalarNumeratorLowering {
            family_identity: family.fingerprint_owner(),
            terms,
        },
        nonzero_conditions: conditions.values.into_iter().collect(),
    })
}

fn split_coefficient(
    context: &CoefficientContext,
    weight: Atom,
    limits: ScalarNumeratorLimits,
    work: &mut bounds::Work,
) -> Result<(Coefficient, Atom), Error> {
    let weight = native("weighted prefactor extraction", || {
        weight.collect_num().collect_factors()
    })?;
    census_atom(
        weight.as_view(),
        "weighted prefactor nodes",
        limits.max_output_atom_nodes,
        limits,
    )?;
    let factors = match weight.as_view() {
        AtomView::Mul(product) => product.iter().collect::<Vec<_>>(),
        factor => vec![factor],
    };
    let symbols = context
        .parameter_names()
        .iter()
        .flat_map(|name| {
            context
                .parameter(name)
                .unwrap()
                .to_expression()
                .get_all_symbols(true)
        })
        .collect::<BTreeSet<_>>();
    let mut coefficient = context.one();
    let mut spectator = Atom::one();
    for factor in factors {
        if factor
            .get_all_symbols(true)
            .iter()
            .all(|symbol| symbols.contains(symbol))
        {
            work.preflight(factor)?;
            let converted: Result<Coefficient, _> =
                native("weighted base coefficient conversion", || {
                    factor.try_to_rational_polynomial(&Q, &Z, Some(context.variables().clone()))
                })?;
            if let Ok(value) = converted {
                match context.validate_with_limits(&value, limits.exact_algebra) {
                    Ok(()) => {
                        coefficient =
                            context.try_mul(&coefficient, &value, limits.exact_algebra)?;
                        continue;
                    }
                    Err(ExactAlgebraError::VariableMapMismatch { .. }) => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
        spectator *= factor;
    }
    Ok((coefficient, spectator))
}

#[cfg(test)]
#[path = "weighted/diagnostics.rs"]
mod diagnostics;
