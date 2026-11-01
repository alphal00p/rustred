//! Exact rebuild of the generic denominator polynomial for leaf-separable input.
//!
//! The generic lane converts the translated numerator with Symbolica's
//! `to_polynomial_in_vars` over `AtomField`, whose coefficients are general
//! expressions with *statistical* zero tests. That conversion never expands
//! scalar coefficients: every term `q * L * D` (rational `q`, product `L` of
//! the factors without denominator coordinates, polynomial part `D` in the
//! coordinates) contributes `normalize(q * c * L * M)` to a monomial, where
//! `c * M` is a coefficient of `D` (`M` a power product of the common mass's
//! variables), and equal non-numeric parts merge by native normalization.
//!
//! This lane groups the terms by `L`, converts each group's polynomial part
//! exactly over Q (the rational lane), and rebuilds every coefficient as the
//! normalized sum of `c * L * M`. The rebuilt polynomial is the generic one,
//! coefficient for coefficient, provided that
//! - the polynomial parts contain only rational numbers, coordinates and the
//!   mass variables, and every sum in them is homogeneous in (coordinate,
//!   mass) degree, so each generic coefficient of a term is a single product;
//! - the distinct non-numeric parts `L * M` are linearly independent over Q,
//!   proven by a nonsingular matrix of exact values at rational points. Then a
//!   normalized sum vanishes in value only when it is literally zero, so the
//!   generic statistical zero tests and the literal tests used here agree;
//! - likewise the terms of every sum that the generic lane rebuilds inside a
//!   leaf, so that its rebuilt leaf is the original one.
//!
//! Anything else returns `None`, and the caller keeps the generic lane.
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use symbolica::{
    atom::{Atom, AtomCore, AtomView, Symbol},
    domains::atom::AtomField,
    poly::{PolyVariable, polynomial::MultivariatePolynomial},
    prelude::{Q, Rational},
};

use crate::scalar_numerator::service::PreparedScalarProducts;

/// Common-mass variables and their total degree; numeric masses have none.
struct Mass {
    variables: Vec<Symbol>,
    degree: u64,
}

impl Mass {
    /// A rational number, or a rational multiple of a power product of plain
    /// variables with positive integer exponents.
    fn new(mass: AtomView<'_>) -> Option<Self> {
        let factors = match mass {
            AtomView::Mul(product) => product.iter().collect::<Vec<_>>(),
            factor => vec![factor],
        };
        let mut variables = BTreeMap::<Symbol, u64>::new();
        for factor in factors {
            match factor {
                AtomView::Num(_) => {
                    rational(factor)?;
                }
                AtomView::Var(variable) => {
                    *variables.entry(variable.get_symbol()).or_default() += 1
                }
                AtomView::Pow(power) => {
                    let AtomView::Var(variable) = power.get_base() else {
                        return None;
                    };
                    let exponent = positive_integer(power.get_exp())?;
                    *variables.entry(variable.get_symbol()).or_default() += exponent;
                }
                _ => return None,
            }
        }
        let degree = variables
            .values()
            .try_fold(0u64, |sum, power| sum.checked_add(*power))?;
        Some(Self {
            variables: variables.into_keys().collect(),
            degree,
        })
    }
}

fn rational(view: AtomView<'_>) -> Option<Rational> {
    Rational::try_from(view).ok()
}

fn positive_integer(view: AtomView<'_>) -> Option<u64> {
    let exponent = rational(view)?;
    if !exponent.is_integer() || exponent.is_negative() || exponent.is_zero() {
        return None;
    }
    u64::try_from(exponent.numerator().to_i64()?).ok()
}

struct Grammar<'a> {
    prepared: &'a PreparedScalarProducts,
    mass: &'a Mass,
}

impl Grammar<'_> {
    fn is_coordinate(&self, view: AtomView<'_>) -> bool {
        matches!(view, AtomView::Fun(function) if function.get_symbol() == self.prepared.dot_head)
            && self
                .prepared
                .scalar_products
                .iter()
                .any(|coordinate| coordinate.as_view() == view)
    }

    fn contains_coordinate(&self, view: AtomView<'_>) -> bool {
        let mut found = false;
        view.visitor(&mut |part| {
            if !found && self.is_coordinate(part) {
                found = true;
            }
            !found
        });
        found
    }

    /// Joint degree of a polynomial part, with weight `mass.degree` per
    /// coordinate and one per mass variable (zero weights for a numeric
    /// mass), or `None` outside the grammar or for an inhomogeneous sum.
    fn degree(&self, view: AtomView<'_>) -> Option<u64> {
        let numeric = self.mass.variables.is_empty();
        match view {
            AtomView::Num(_) => rational(view).map(|_| 0),
            AtomView::Fun(_) => {
                self.is_coordinate(view)
                    .then_some(if numeric { 0 } else { self.mass.degree })
            }
            AtomView::Var(variable) => {
                (!numeric && self.mass.variables.contains(&variable.get_symbol())).then_some(1)
            }
            AtomView::Add(sum) => {
                let mut degree = None;
                for term in sum.iter() {
                    let term = self.degree(term)?;
                    if degree.is_some_and(|degree| degree != term) {
                        return None;
                    }
                    degree = Some(term);
                }
                degree
            }
            AtomView::Mul(product) => product
                .iter()
                .try_fold(0u64, |sum, factor| sum.checked_add(self.degree(factor)?)),
            AtomView::Pow(power) => {
                let exponent = rational(power.get_exp())?;
                if !exponent.is_integer() || exponent.is_negative() {
                    return None;
                }
                let exponent = u64::try_from(exponent.numerator().to_i64()?).ok()?;
                self.degree(power.get_base())?.checked_mul(exponent)
            }
        }
    }
}

/// Exact value of a rational function of plain variables, `None` at a pole
/// or for anything else (functions, non-rational numbers or exponents).
fn evaluate(view: AtomView<'_>, point: &HashMap<Symbol, Rational>) -> Option<Rational> {
    match view {
        AtomView::Num(_) => rational(view),
        AtomView::Var(variable) => point.get(&variable.get_symbol()).cloned(),
        AtomView::Fun(_) => None,
        AtomView::Add(sum) => sum.iter().try_fold(Rational::zero(), |total, term| {
            Some(&total + &evaluate(term, point)?)
        }),
        AtomView::Mul(product) => product.iter().try_fold(Rational::one(), |total, factor| {
            Some(&total * &evaluate(factor, point)?)
        }),
        AtomView::Pow(power) => {
            let exponent = rational(power.get_exp())?;
            if !exponent.is_integer() {
                return None;
            }
            let exponent = exponent.numerator().to_i64()?;
            let base = evaluate(power.get_base(), point)?;
            if exponent < 0 && base.is_zero() {
                return None;
            }
            let value = base.pow(exponent.unsigned_abs());
            Some(if exponent < 0 { value.inv() } else { value })
        }
    }
}

/// Exact proof that the expressions are linearly independent over Q: a
/// nonsingular matrix of their values at deterministic rational points.
/// A pole or an unlucky singular sample only fails the proof.
fn linearly_independent(expressions: &[&Atom]) -> bool {
    let count = expressions.len();
    let symbols = expressions
        .iter()
        .flat_map(|expression| expression.get_all_symbols(false))
        .collect::<BTreeSet<_>>();
    let mut rows = vec![Vec::with_capacity(count + 2); count];
    // Two extra points tolerate an unlucky singular square sample.
    for point in 0..count as i64 + 2 {
        let values = symbols
            .iter()
            .enumerate()
            .map(|(index, symbol)| {
                let index = index as i64;
                // A large prime denominator avoids small rational poles.
                let numerator = 7919 * (index + 1)
                    + 104_729 * (point + 1)
                    + (index + 3) * (point + 5) * (point + 7);
                (*symbol, Rational::new(numerator, 1_000_003))
            })
            .collect::<HashMap<_, _>>();
        for (row, expression) in rows.iter_mut().zip(expressions) {
            let Some(value) = evaluate(expression.as_view(), &values) else {
                return false;
            };
            row.push(value);
        }
    }
    let columns = count + 2;
    let mut rank = 0;
    for column in 0..columns {
        if rank == count {
            break;
        }
        let Some(pivot) = (rank..count).find(|row| !rows[*row][column].is_zero()) else {
            continue;
        };
        rows.swap(rank, pivot);
        let pivot = rows[rank].clone();
        for row in rows.iter_mut().skip(rank + 1) {
            if row[column].is_zero() {
                continue;
            }
            let factor = &row[column] / &pivot[column];
            for (entry, value) in row.iter_mut().zip(&pivot).skip(column) {
                *entry = &*entry - &(&factor * value);
            }
        }
        rank += 1;
    }
    rank == count
}

/// The generic lane rebuilds every sum it reaches inside a leaf (not under a
/// negative power or a function) by sequential native additions with
/// statistical zero tests. That rebuild is the original sum when its terms
/// are linearly independent over Q (no partial sum vanishes in value).
fn rebuilt_sums_are_exact(view: AtomView<'_>) -> bool {
    match view {
        AtomView::Num(_) | AtomView::Var(_) | AtomView::Fun(_) => true,
        AtomView::Add(sum) => {
            let terms = sum.iter().map(|term| term.to_owned()).collect::<Vec<_>>();
            linearly_independent(&terms.iter().collect::<Vec<_>>())
                && sum.iter().all(rebuilt_sums_are_exact)
        }
        AtomView::Mul(product) => product.iter().all(rebuilt_sums_are_exact),
        AtomView::Pow(power) => match rational(power.get_exp()) {
            Some(exponent) if exponent.is_integer() && !exponent.is_negative() => {
                rebuilt_sums_are_exact(power.get_base())
            }
            // Negative or symbolic powers stay opaque generic constants.
            _ => true,
        },
    }
}

/// The generic `to_polynomial_in_vars` result for leaf-separable input, built
/// without statistical zero tests, or `None` if the input is outside the
/// grammar of the module documentation.
pub(super) fn collect(
    prepared: &PreparedScalarProducts,
    atom: &Atom,
    mass: &Atom,
) -> Option<MultivariatePolynomial<AtomField, u32>> {
    let mass = Mass::new(mass.as_view())?;
    let grammar = Grammar {
        prepared,
        mass: &mass,
    };
    let terms = match atom.as_view() {
        AtomView::Add(sum) => sum.iter().collect::<Vec<_>>(),
        term => vec![term],
    };
    // Polynomial parts grouped by their exact leaf product, in first
    // appearance order (the order only affects the work, never the result).
    let mut index = HashMap::<Atom, usize>::new();
    let mut groups = Vec::<(Atom, Vec<Atom>)>::new();
    for term in terms {
        let factors = match term {
            AtomView::Mul(product) => product.iter().collect::<Vec<_>>(),
            factor => vec![factor],
        };
        let mut number = Rational::one();
        let mut leaves = Vec::new();
        let mut part = Vec::new();
        for factor in factors {
            if let AtomView::Num(_) = factor {
                number = &number * &rational(factor)?;
            } else if grammar.contains_coordinate(factor) {
                grammar.degree(factor)?;
                part.push(factor.to_owned());
            } else {
                leaves.push(factor.to_owned());
            }
        }
        let leaf = if leaves.is_empty() {
            Atom::one()
        } else {
            Atom::mul_many(leaves)
        };
        let part = part
            .into_iter()
            .fold(Atom::num(number), |product, factor| product * factor);
        let slot = *index.entry(leaf.clone()).or_insert_with(|| {
            groups.push((leaf, Vec::new()));
            groups.len() - 1
        });
        groups[slot].1.push(part);
    }
    let coordinates = prepared.scalar_product_variables.len();
    let mut variables = prepared.scalar_product_variables.as_ref().clone();
    variables.extend(
        mass.variables
            .iter()
            .map(|symbol| PolyVariable::Symbol(*symbol)),
    );
    let variables = Arc::new(variables);
    // Rebuilt coefficients: coordinate exponents -> products `c * L * M`.
    let mut coefficients = BTreeMap::<Vec<u32>, Vec<Atom>>::new();
    let mut parts = BTreeSet::<Atom>::new();
    let mut leaves = Vec::with_capacity(groups.len());
    for (leaf, terms) in groups {
        let polynomial: MultivariatePolynomial<_, u32> = Atom::add_many(terms)
            .try_to_polynomial(&Q, variables.clone())
            .ok()?;
        // Symbolica may add unknown expressions as variables; never accept that.
        if polynomial.variables().as_ref() != variables.as_ref() {
            return None;
        }
        for term in &polynomial {
            let mut monomial = leaf.clone();
            for (symbol, power) in mass.variables.iter().zip(&term.exponents[coordinates..]) {
                if *power != 0 {
                    monomial = monomial * Atom::var(*symbol).pow(Atom::num(i64::from(*power)));
                }
            }
            coefficients
                .entry(term.exponents[..coordinates].to_vec())
                .or_default()
                .push(Atom::num(term.coefficient.clone()) * &monomial);
            if !mass.variables.is_empty() {
                parts.insert(monomial);
            }
        }
        leaves.push(leaf);
    }
    // Distinct non-numeric parts must be linearly independent. With the mass
    // variables absent from every leaf, that follows from the leaves alone.
    let disjoint = mass.variables.is_empty()
        || leaves.iter().all(|leaf| {
            let symbols = leaf.get_all_symbols(true);
            mass.variables
                .iter()
                .all(|variable| !symbols.contains(variable))
        });
    let independent = if disjoint {
        linearly_independent(&leaves.iter().collect::<Vec<_>>())
    } else {
        linearly_independent(&parts.iter().collect::<Vec<_>>())
    };
    if !independent
        || !leaves
            .iter()
            .all(|leaf| rebuilt_sums_are_exact(leaf.as_view()))
    {
        return None;
    }
    let mut field = AtomField::new();
    field.statistical_zero_test = false;
    let mut polynomial = MultivariatePolynomial::new(
        &field,
        Some(coefficients.len()),
        prepared.scalar_product_variables.clone(),
    );
    for (exponents, terms) in coefficients {
        // Native normalization merges equal non-numeric parts exactly as the
        // generic coefficient sums do; a literal zero is a value zero here.
        polynomial.append_monomial(Atom::add_many(terms), &exponents);
    }
    Some(polynomial)
}
