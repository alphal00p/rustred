//! Whole-expression change of polynomial coordinates through the native CAS.
//! All arithmetic on polynomials remains owned by Symbolica.

use super::{Error, bounds::Work, native};
use crate::family::IntegralFamily;
use crate::scalar_numerator::error::{check_limit, checked_add, checked_mul};
use crate::scalar_numerator::{
    ScalarNumeratorLimits,
    service::PreparedScalarProducts,
    syntax::{ExactComparisonBudget, census_atom},
};
use std::collections::BTreeSet;
use symbolica::{
    atom::{Atom, AtomCore, AtomView},
    id::{Pattern, Replacement},
    prelude::Rational,
};

#[derive(Clone)]
struct Shape {
    terms: usize,
    degree: usize,
    variables: BTreeSet<usize>,
    /// Proven by this existing bounded syntax walk, never by sampling.
    real_rational_coefficients: bool,
}
impl Shape {
    fn constant(zero: bool) -> Self {
        Self {
            terms: usize::from(!zero),
            degree: 0,
            variables: BTreeSet::new(),
            real_rational_coefficients: true,
        }
    }
}

// Bound unique monomials of total degree <=degree in the present variables.
// This is allocation/work metadata, not a polynomial multiplication routine.
fn envelope(variables: usize, degree: usize, cap: usize) -> Result<usize, Error> {
    let total = checked_add("weighted denominator monomial envelope", variables, degree)?;
    let count = variables.min(degree);
    let mut value = 1u128;
    for i in 1..=count {
        value =
            value
                .checked_mul((total - count + i) as u128)
                .ok_or(Error::ResourceCountOverflow {
                    resource: "weighted denominator monomial envelope",
                })?
                / i as u128;
        if value > cap as u128 {
            return Ok(cap);
        }
    }
    usize::try_from(value).map_err(|_| Error::ResourceCountOverflow {
        resource: "weighted denominator monomial envelope",
    })
}

fn product(
    a: Shape,
    b: Shape,
    limits: ScalarNumeratorLimits,
    work: &mut Work,
    admit: bool,
) -> Result<Shape, Error> {
    let degree = checked_add("weighted denominator degree", a.degree, b.degree)?;
    check_limit(
        "weighted scalar-product degree",
        degree,
        limits.max_scalar_product_degree.min(i32::MAX as usize - 1),
    )?;
    let variables = a
        .variables
        .union(&b.variables)
        .copied()
        .collect::<BTreeSet<_>>();
    if !admit {
        return Ok(Shape {
            terms: usize::from(a.terms != 0 && b.terms != 0),
            degree,
            variables,
            real_rational_coefficients: a.real_rational_coefficients
                && b.real_rational_coefficients,
        });
    }
    let pairs = checked_mul("weighted denominator product work", a.terms, b.terms)?;
    if admit {
        work.charge(pairs)?;
    }
    let terms = pairs.min(envelope(variables.len(), degree, pairs)?);
    if admit {
        check_limit(
            "weighted denominator polynomial terms",
            terms,
            limits.max_polynomial_terms,
        )?;
    }
    Ok(Shape {
        terms,
        degree,
        variables,
        real_rational_coefficients: a.real_rational_coefficients && b.real_rational_coefficients,
    })
}

fn shape(
    atom: AtomView<'_>,
    prepared: &PreparedScalarProducts,
    comparisons: &mut ExactComparisonBudget,
    work: &mut Work,
    admit: bool,
) -> Result<Shape, Error> {
    work.charge(1)?;
    match atom {
        AtomView::Num(_) => {
            let mut result = Shape::constant(atom.is_zero());
            result.real_rational_coefficients = Rational::try_from(atom).is_ok();
            Ok(result)
        }
        AtomView::Var(_) | AtomView::Fun(_) => {
            for (index, variable) in prepared.scalar_products.iter().enumerate() {
                if comparisons.equals(atom, variable.as_view())? {
                    return Ok(Shape {
                        terms: 1,
                        degree: 1,
                        variables: BTreeSet::from([index]),
                        real_rational_coefficients: true,
                    });
                }
            }
            for variable in &prepared.scalar_products {
                if crate::scalar_numerator::syntax::contains_exact_atom(
                    atom,
                    variable.as_view(),
                    comparisons,
                )? {
                    return Err(Error::NonPolynomialScalarProducts {
                        detail: "a loop scalar product occurs inside a scalar function".into(),
                    });
                }
            }
            let mut result = Shape::constant(false);
            result.real_rational_coefficients = false;
            Ok(result)
        }
        AtomView::Add(sum) => {
            if admit {
                check_limit(
                    "weighted denominator input terms",
                    sum.get_nargs(),
                    prepared.limits.max_input_terms,
                )?;
            }
            let mut result = Shape::constant(true);
            for term in sum.iter() {
                let next = shape(term, prepared, comparisons, work, admit)?;
                result.degree = result.degree.max(next.degree);
                result.real_rational_coefficients &= next.real_rational_coefficients;
                result.variables.extend(next.variables);
                let sum = checked_add("weighted denominator sum work", result.terms, next.terms)?;
                // Native Add appends child coefficient lists and sorts once;
                // it does not repeatedly add every previous coefficient.
                if admit {
                    work.charge(next.terms)?;
                }
                result.terms = sum.min(envelope(result.variables.len(), result.degree, sum)?);
                if admit {
                    check_limit(
                        "weighted denominator polynomial terms",
                        result.terms,
                        prepared.limits.max_polynomial_terms,
                    )?;
                }
            }
            Ok(result)
        }
        AtomView::Mul(mul) => {
            check_limit(
                "weighted scalar factors per term",
                mul.get_nargs(),
                prepared.limits.max_factors_per_term,
            )?;
            let mut result = Shape::constant(false);
            for factor in mul.iter() {
                result = product(
                    result,
                    shape(factor, prepared, comparisons, work, admit)?,
                    prepared.limits,
                    work,
                    admit,
                )?;
            }
            Ok(result)
        }
        AtomView::Pow(power) => {
            let base = shape(power.get_base(), prepared, comparisons, work, admit)?;
            let exponent_shape = shape(power.get_exp(), prepared, comparisons, work, admit)?;
            if !exponent_shape.variables.is_empty() {
                return Err(Error::NonPolynomialScalarProducts {
                    detail: "loop scalar product in an exponent".into(),
                });
            }
            if base.variables.is_empty() {
                let mut result = Shape::constant(atom.is_zero());
                result.real_rational_coefficients = Rational::try_from(atom).is_ok();
                return Ok(result);
            }
            let exponent = Rational::try_from(power.get_exp()).map_err(|_| {
                Error::NonPolynomialScalarProducts {
                    detail: "noninteger scalar-product exponent".into(),
                }
            })?;
            if !exponent.is_integer() || exponent.is_negative() {
                return Err(Error::NonPolynomialScalarProducts {
                    detail: "negative or noninteger scalar-product exponent".into(),
                });
            }
            let mut magnitude = exponent
                .numerator()
                .to_i64()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or(Error::ScalarProductExponentOverflow)?;
            let degree = checked_mul("weighted denominator power degree", base.degree, magnitude)?;
            check_limit(
                "weighted scalar-product degree",
                degree,
                prepared
                    .limits
                    .max_scalar_product_degree
                    .min(i32::MAX as usize - 1),
            )?;
            if !admit {
                return Ok(Shape {
                    terms: 1,
                    degree,
                    variables: base.variables,
                    real_rational_coefficients: base.real_rational_coefficients,
                });
            }
            let mut base = base;
            let mut result = Shape::constant(false);
            while magnitude != 0 {
                if magnitude & 1 != 0 {
                    result = product(result, base.clone(), prepared.limits, work, true)?;
                }
                magnitude >>= 1;
                if magnitude != 0 {
                    base = product(base.clone(), base, prepared.limits, work, true)?;
                }
            }
            Ok(result)
        }
    }
}

pub(super) struct Translated {
    pub atom: Atom,
    pub real_rational_coefficients: bool,
}

pub(super) fn translate(
    prepared: &PreparedScalarProducts,
    family: &IntegralFamily,
    numerator: &Atom,
    mass: &Atom,
    work: &mut Work,
    comparisons: &mut ExactComparisonBudget,
) -> Result<Translated, Error> {
    // Grammar/degree validation precedes every rewrite, but a Gram expansion
    // support estimate is deliberately not used to admit a Den-basis result.
    let input_terms = match numerator.as_view() {
        AtomView::Add(sum) => sum.get_nargs(),
        _ => 1,
    };
    check_limit(
        "scalar-numerator input terms",
        input_terms,
        prepared.limits.max_input_terms,
    )?;
    check_limit(
        "scalar-numerator factors per term",
        1,
        prepared.limits.max_factors_per_term,
    )?;
    #[cfg(test)]
    work.set_diagnostic_phase("source grammar");
    shape(numerator.as_view(), prepared, comparisons, work, false)?;
    let mut replacements = Vec::new();
    for (coordinate, source) in prepared.scalar_products.iter().enumerate() {
        let expansion = family.scalar_product_expansion(coordinate)?;
        let mut target = expansion.constant().to_expression() * mass;
        for (coefficient, variable) in expansion
            .denominator_coefficients()
            .iter()
            .zip(&prepared.scalar_products)
        {
            target += coefficient.to_expression() * variable;
        }
        // Loop labels are caller-owned atoms and may look like wildcards.
        // Both sides are native literal patterns, never pattern templates.
        replacements.push(Replacement::new(
            Pattern::Literal(source.clone()),
            Pattern::Literal(target),
        ));
    }
    // replace_multiple is a single simultaneous traversal: inserted RHS atoms
    // are never visited again. Reuse the existing coordinate labels as local
    // D variables, avoiding caller-colliding temporary symbols entirely.
    let translated = native("whole scalar numerator denominator substitution", || {
        numerator.replace_multiple(&replacements)
    })?;
    census_atom(
        translated.as_view(),
        "weighted translated denominator nodes",
        prepared.limits.max_output_atom_nodes,
        prepared.limits,
    )?;
    // A routed affine form can retain nested numeric multiples, e.g.
    // 3*(D+1)-2*(D+2)+1. Count its collected linear support before raising
    // it to a power. Native numeric distribution preserves those powers;
    // full expansion here would recreate the intermediate blow-up avoided
    // by the whole-expression coordinate change.
    #[cfg(test)]
    work.set_diagnostic_phase("denominator numeric collection");
    work.charge(census_atom(
        translated.as_view(),
        "weighted numeric collection nodes",
        prepared.limits.max_output_atom_nodes,
        prepared.limits,
    )?)?;
    let translated = native("denominator-coordinate numeric collection", || {
        translated.expand_num()
    })?;
    census_atom(
        translated.as_view(),
        "weighted numeric-collected denominator nodes",
        prepared.limits.max_output_atom_nodes,
        prepared.limits,
    )?;
    #[cfg(test)]
    work.set_diagnostic_phase("denominator polynomial admission");
    let admitted = shape(translated.as_view(), prepared, comparisons, work, true)?;
    Ok(Translated {
        atom: translated,
        real_rational_coefficients: admitted.real_rational_coefficients,
    })
}

pub(super) fn validate_coefficient(
    prepared: &PreparedScalarProducts,
    coefficient: &Atom,
    comparisons: &mut ExactComparisonBudget,
) -> Result<(), Error> {
    for variable in &prepared.scalar_products {
        if crate::scalar_numerator::syntax::contains_exact_atom(
            coefficient.as_view(),
            variable.as_view(),
            comparisons,
        )? {
            return Err(Error::NonPolynomialScalarProducts {
                detail: "local denominator coordinate remained inside a scalar coefficient".into(),
            });
        }
    }
    Ok(())
}
