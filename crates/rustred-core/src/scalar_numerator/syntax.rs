use symbolica::atom::{Atom, AtomView, Symbol};
use symbolica::prelude::Rational;

use super::error::{ScalarNumeratorError, check_limit, checked_add};
use super::model::ScalarNumeratorLimits;

pub(super) fn census_atom(
    atom: AtomView<'_>,
    resource: &'static str,
    node_limit: usize,
    limits: ScalarNumeratorLimits,
) -> Result<usize, ScalarNumeratorError> {
    let mut count = 0usize;
    let mut pending = vec![(atom, 0usize)];
    while let Some((part, depth)) = pending.pop() {
        count = count.saturating_add(1);
        check_limit(resource, count, node_limit, limits.diagnostics_enabled)?;
        check_limit(
            "scalar-numerator Atom nesting depth",
            depth,
            limits.max_nesting_depth,
            limits.diagnostics_enabled,
        )?;
        let child_depth = depth.saturating_add(1);
        match part {
            AtomView::Fun(function) => {
                pending.extend(function.iter().map(|child| (child, child_depth)))
            }
            AtomView::Pow(power) => {
                pending.push((power.get_base(), child_depth));
                pending.push((power.get_exp(), child_depth));
            }
            AtomView::Mul(product) => {
                pending.extend(product.iter().map(|child| (child, child_depth)))
            }
            AtomView::Add(sum) => pending.extend(sum.iter().map(|child| (child, child_depth))),
            AtomView::Num(_) | AtomView::Var(_) => {}
        }
    }
    Ok(count)
}

const MAX_SYMBOLICA_POLYNOMIAL_EXPONENT: usize = i32::MAX as usize - 1;

pub(super) struct ExactComparisonBudget {
    used: usize,
    limit: usize,
    diagnostics_enabled: bool,
}

impl ExactComparisonBudget {
    pub(super) const fn new(limit: usize, diagnostics_enabled: bool) -> Self {
        Self {
            used: 0,
            limit,
            diagnostics_enabled,
        }
    }

    pub(super) fn equals(
        &mut self,
        left: AtomView<'_>,
        right: AtomView<'_>,
    ) -> Result<bool, ScalarNumeratorError> {
        self.used = self.used.saturating_add(1);
        check_limit(
            "loop-momentum exact subtree checks",
            self.used,
            self.limit,
            self.diagnostics_enabled,
        )?;
        Ok(left == right)
    }
}

pub(super) fn validate_scalar_syntax(
    numerator: AtomView<'_>,
    dot_head: Symbol,
    loop_momenta: &[Atom],
    comparisons: &mut ExactComparisonBudget,
) -> Result<(), ScalarNumeratorError> {
    fn visit<'a>(
        atom: AtomView<'a>,
        dot_head: Symbol,
        loop_momenta: &[Atom],
        comparisons: &mut ExactComparisonBudget,
        pending: &mut Vec<AtomView<'a>>,
    ) -> Result<(), ScalarNumeratorError> {
        if let Some(momentum) = resolve_loop(atom, loop_momenta, comparisons)? {
            return Err(ScalarNumeratorError::LoopMomentumOutsideScalarProduct {
                momentum: loop_momenta[momentum].clone(),
            });
        }
        match atom {
            AtomView::Var(variable) if variable.get_symbol() == dot_head => {
                Err(ScalarNumeratorError::MalformedScalarProduct { actual_arity: None })
            }
            AtomView::Fun(function) if function.get_symbol() == dot_head => {
                if function.get_nargs() != 2 {
                    return Err(ScalarNumeratorError::MalformedScalarProduct {
                        actual_arity: Some(function.get_nargs()),
                    });
                }
                let mut arguments = function.iter();
                let left_argument = arguments
                    .next()
                    .expect("validated scalar product has a left argument");
                let right_argument = arguments
                    .next()
                    .expect("validated scalar product has a right argument");
                if contains_head(left_argument, dot_head)?
                    || contains_head(right_argument, dot_head)?
                {
                    return Err(ScalarNumeratorError::NestedScalarProductArgument {
                        expression: atom.to_owned(),
                    });
                }
                let left = resolve_loop(left_argument, loop_momenta, comparisons)?;
                let right = resolve_loop(right_argument, loop_momenta, comparisons)?;
                match (left, right) {
                    (Some(_), Some(_)) | (None, None) => {
                        let mut hidden_loop = false;
                        if left.is_none() {
                            'arguments: for argument in [left_argument, right_argument] {
                                for momentum in loop_momenta {
                                    if contains_exact_atom(
                                        argument,
                                        momentum.as_view(),
                                        comparisons,
                                    )? {
                                        hidden_loop = true;
                                        break 'arguments;
                                    }
                                }
                            }
                        }
                        if hidden_loop {
                            Err(ScalarNumeratorError::NestedScalarProductArgument {
                                expression: atom.to_owned(),
                            })
                        } else {
                            Ok(())
                        }
                    }
                    _ => Err(ScalarNumeratorError::MixedLoopScalarProduct {
                        expression: atom.to_owned(),
                    }),
                }
            }
            AtomView::Fun(function) => {
                for argument in function.iter() {
                    pending.push(argument);
                }
                Ok(())
            }
            AtomView::Pow(power) => {
                pending.push(power.get_exp());
                pending.push(power.get_base());
                Ok(())
            }
            AtomView::Mul(product) => {
                for factor in product.iter() {
                    pending.push(factor);
                }
                Ok(())
            }
            AtomView::Add(sum) => {
                for term in sum.iter() {
                    pending.push(term);
                }
                Ok(())
            }
            AtomView::Num(_) | AtomView::Var(_) => Ok(()),
        }
    }

    let mut pending = vec![numerator];
    while let Some(atom) = pending.pop() {
        visit(atom, dot_head, loop_momenta, comparisons, &mut pending)?;
    }
    Ok(())
}

/// Admit an already expanded polynomial shape before asking Symbolica to
/// collect coefficients. This prevents a compact `(s1+s2)^N` input from
/// creating an unbounded transient expansion inside the CAS.
pub(super) fn preflight_polynomial_shape(
    numerator: AtomView<'_>,
    scalar_products: &[Atom],
    limits: ScalarNumeratorLimits,
    comparisons: &mut ExactComparisonBudget,
) -> Result<(), ScalarNumeratorError> {
    let term_count = match numerator {
        AtomView::Add(sum) => sum.iter().count(),
        _ => 1,
    };
    check_limit(
        "scalar-numerator input terms",
        term_count,
        limits.max_input_terms,
        limits.diagnostics_enabled,
    )?;
    match numerator {
        AtomView::Add(sum) => {
            for term in sum.iter() {
                preflight_term(term, scalar_products, limits, comparisons)?;
            }
        }
        term => preflight_term(term, scalar_products, limits, comparisons)?,
    }
    Ok(())
}

fn preflight_term(
    term: AtomView<'_>,
    scalar_products: &[Atom],
    limits: ScalarNumeratorLimits,
    comparisons: &mut ExactComparisonBudget,
) -> Result<(), ScalarNumeratorError> {
    let factor_count = match term {
        AtomView::Mul(product) => product.iter().count(),
        _ => 1,
    };
    check_limit(
        "scalar-numerator factors per term",
        factor_count,
        limits.max_factors_per_term,
        limits.diagnostics_enabled,
    )?;
    let mut degree = 0usize;
    match term {
        AtomView::Mul(product) => {
            for factor in product.iter() {
                preflight_factor(factor, scalar_products, &mut degree, comparisons)?;
            }
        }
        factor => preflight_factor(factor, scalar_products, &mut degree, comparisons)?,
    }
    check_limit(
        "scalar-product degree",
        degree,
        limits.max_scalar_product_degree,
        limits.diagnostics_enabled,
    )
}

fn preflight_factor(
    factor: AtomView<'_>,
    scalar_products: &[Atom],
    degree: &mut usize,
    comparisons: &mut ExactComparisonBudget,
) -> Result<(), ScalarNumeratorError> {
    if matches_any_exact(factor, scalar_products, comparisons)? {
        add_scalar_degree(degree, 1)?;
        return Ok(());
    }
    if let AtomView::Pow(power) = factor
        && matches_any_exact(power.get_base(), scalar_products, comparisons)?
    {
        let exponent = Rational::try_from(power.get_exp()).map_err(|_| {
            ScalarNumeratorError::NonPolynomialScalarProducts {
                detail: format!("noninteger exponent in {factor}"),
            }
        })?;
        if !exponent.is_integer() || exponent.is_negative() {
            return Err(ScalarNumeratorError::NonPolynomialScalarProducts {
                detail: format!("negative or noninteger exponent in {factor}"),
            });
        }
        let exponent = exponent
            .numerator()
            .to_i64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(ScalarNumeratorError::ScalarProductExponentOverflow)?;
        add_scalar_degree(degree, exponent)?;
        return Ok(());
    }
    for candidate in scalar_products {
        if contains_exact_atom(factor, candidate.as_view(), comparisons)? {
            return Err(ScalarNumeratorError::NonPolynomialScalarProducts {
                detail: format!("loop scalar product occurs inside an unexpanded factor {factor}"),
            });
        }
    }
    Ok(())
}

fn add_scalar_degree(degree: &mut usize, increment: usize) -> Result<(), ScalarNumeratorError> {
    *degree = checked_add("scalar-product degree", *degree, increment)?;
    if *degree > MAX_SYMBOLICA_POLYNOMIAL_EXPONENT {
        return Err(ScalarNumeratorError::ScalarProductExponentOverflow);
    }
    Ok(())
}

fn matches_any_exact(
    atom: AtomView<'_>,
    candidates: &[Atom],
    comparisons: &mut ExactComparisonBudget,
) -> Result<bool, ScalarNumeratorError> {
    for candidate in candidates {
        if comparisons.equals(atom, candidate.as_view())? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn contains_head(
    atom: AtomView<'_>,
    head: Symbol,
) -> Result<bool, ScalarNumeratorError> {
    let mut pending = vec![atom];
    while let Some(part) = pending.pop() {
        symbolica::utils::resources::checkpoint()
            .map_err(crate::algebra::ExactAlgebraError::Resource)?;
        match part {
            AtomView::Var(variable) if variable.get_symbol() == head => return Ok(true),
            AtomView::Fun(function) => {
                if function.get_symbol() == head {
                    return Ok(true);
                }
                pending.extend(function.iter());
            }
            AtomView::Pow(power) => {
                pending.push(power.get_base());
                pending.push(power.get_exp());
            }
            AtomView::Mul(product) => pending.extend(product.iter()),
            AtomView::Add(sum) => pending.extend(sum.iter()),
            AtomView::Num(_) | AtomView::Var(_) => {}
        }
    }
    Ok(false)
}

fn resolve_loop(
    momentum: AtomView<'_>,
    loop_momenta: &[Atom],
    comparisons: &mut ExactComparisonBudget,
) -> Result<Option<usize>, ScalarNumeratorError> {
    for (index, candidate) in loop_momenta.iter().enumerate() {
        if comparisons.equals(candidate.as_view(), momentum)? {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

pub(super) fn contains_exact_atom(
    atom: AtomView<'_>,
    needle: AtomView<'_>,
    comparisons: &mut ExactComparisonBudget,
) -> Result<bool, ScalarNumeratorError> {
    let mut pending = vec![atom];
    while let Some(part) = pending.pop() {
        if comparisons.equals(part, needle)? {
            return Ok(true);
        }
        match part {
            AtomView::Fun(function) => pending.extend(function.iter()),
            AtomView::Pow(power) => {
                pending.push(power.get_base());
                pending.push(power.get_exp());
            }
            AtomView::Mul(product) => pending.extend(product.iter()),
            AtomView::Add(sum) => pending.extend(sum.iter()),
            AtomView::Num(_) | AtomView::Var(_) => {}
        }
    }
    Ok(false)
}

/// Evaluate an algebraic syntax tree without consuming the Rust call stack.
/// Function arguments may be left opaque by the owning shape analysis.
pub(super) fn fold_expression<'a, T>(
    atom: AtomView<'a>,
    children: impl Fn(AtomView<'a>) -> Vec<AtomView<'a>>,
    mut evaluate: impl FnMut(AtomView<'a>, Vec<T>) -> Result<T, ScalarNumeratorError>,
) -> Result<T, ScalarNumeratorError> {
    let mut pending = vec![(atom, None)];
    let mut values = Vec::new();
    while let Some((part, child_count)) = pending.pop() {
        symbolica::utils::resources::checkpoint()
            .map_err(crate::algebra::ExactAlgebraError::Resource)?;
        if let Some(child_count) = child_count {
            let start = values
                .len()
                .checked_sub(child_count)
                .expect("postorder children have completed");
            let children = values.split_off(start);
            values.push(evaluate(part, children)?);
        } else {
            let next = children(part);
            pending.push((part, Some(next.len())));
            pending.extend(next.into_iter().rev().map(|child| (child, None)));
        }
    }
    Ok(values
        .pop()
        .expect("the nonempty input has one folded value"))
}
