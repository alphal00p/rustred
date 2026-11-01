//! Exact rebuild of the generic denominator polynomial for leaf-separable input.
//!
//! The generic lane converts the translated numerator with Symbolica's
//! `to_polynomial_in_vars` over `AtomField`, whose coefficients are general
//! expressions with *statistical* zero tests. That conversion never expands
//! scalar coefficients: every term `q * L * D` (rational `q`, product `L` of
//! the factors without denominator coordinates, polynomial part `D` in the
//! coordinates) contributes `normalize(q * c * L * M)` to a monomial, where
//! `c * M` is a coefficient of `D` (`M` a power of the common mass's
//! variable), and equal non-numeric parts merge by native normalization.
//!
//! This lane groups the terms by `L`, converts each group's polynomial part
//! exactly over Q (the rational lane), and rebuilds every coefficient as the
//! normalized sum of `c * L * M`. The rebuilt polynomial is the generic one,
//! coefficient for coefficient (the same Atoms, not only the same values),
//! provided that
//! - the common mass is a rational number or a rational multiple of a power
//!   of one plain variable, the polynomial parts contain only rational
//!   numbers, coordinates and that variable, and every sum in them is
//!   homogeneous in (coordinate, mass) degree. Then a term contributes one
//!   mass power per coordinate monomial, so each generic coefficient of a
//!   term is a single product. (With two mass variables a homogeneous sum
//!   can mix `m*n` and `m^2`: the generic lane keeps `x*(m*n+m^2)`, this
//!   lane would split it.)
//! - no part `L * M` that normalizes to a bare sum `P` collects two
//!   contributions. Native normalization flattens `1*P` into the terms of
//!   `P` but keeps `c*P` as a product, so the generic sequential sums give
//!   `P + 2*P` as `fx+fy+2*(fx+fy)`, not `3*(fx+fy)`. Such a part must come
//!   from a single term of a single leaf group, and within a term the
//!   generic lane must never add products whose leaf prefix (the leaf
//!   factors met so far, in the term's factor order) may be a bare sum.
//! - the distinct non-numeric parts `L * M` are linearly independent over Q,
//!   proven by a nonsingular matrix of their values at pseudo-random points
//!   modulo a large prime. Then a normalized sum vanishes in value only when
//!   it is literally zero, so the generic statistical zero tests and the
//!   literal tests used here agree;
//! - likewise the terms of every sum that the generic lane rebuilds inside a
//!   leaf, so that its rebuilt leaf is the original one.
//!
//! Anything else returns `None`, and the caller keeps the generic lane.
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use symbolica::{
    atom::{Atom, AtomCore, AtomView, Symbol},
    domains::{atom::AtomField, rational::RationalField},
    poly::{PolyVariable, polynomial::MultivariatePolynomial},
    prelude::{Q, Rational},
};

use crate::scalar_numerator::service::PreparedScalarProducts;

/// Common-mass variable and its degree; numeric masses have none.
struct Mass {
    variables: Vec<Symbol>,
    degree: u64,
}

impl Mass {
    /// A rational number, or a rational multiple of a positive integer power
    /// of one plain variable. Products of several variables (`m*n`) keep the
    /// generic lane: homogeneity no longer fixes the mass monomial of a
    /// coordinate monomial (see the module documentation).
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
        if variables.len() > 1 {
            return None;
        }
        let degree = variables
            .values()
            .try_fold(0u64, |sum, power| sum.checked_add(*power))?;
        Some(Self {
            variables: variables.into_keys().collect(),
            degree,
        })
    }

    /// A power of the mass variable (`m`, `m^-2`), as a leaf factor.
    fn is_power(&self, view: AtomView<'_>) -> bool {
        let base = match view {
            AtomView::Pow(power) => match rational(power.get_exp()) {
                Some(exponent) if exponent.is_integer() => power.get_base(),
                _ => return false,
            },
            view => view,
        };
        matches!(base, AtomView::Var(variable) if self.variables.contains(&variable.get_symbol()))
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

/// Arithmetic modulo the Mersenne prime `2^61 - 1` for the independence
/// proofs. Values of rational functions with rational coefficients reduce
/// to residues as long as no denominator is divisible by the prime, and a
/// matrix of residues with full rank has full rank over Q (its determinant
/// is nonzero modulo the prime, so nonzero).
mod residue {
    use symbolica::domains::integer::Integer;
    use symbolica::prelude::Rational;

    pub(super) const PRIME: u64 = (1 << 61) - 1;

    fn reduce(value: u128) -> u64 {
        let prime = u128::from(PRIME);
        let folded = (value & prime) + (value >> 61);
        let folded = ((folded & prime) + (folded >> 61)) as u64;
        if folded >= PRIME {
            folded - PRIME
        } else {
            folded
        }
    }

    pub(super) fn add(left: u64, right: u64) -> u64 {
        let sum = left + right;
        if sum >= PRIME { sum - PRIME } else { sum }
    }

    pub(super) fn sub(left: u64, right: u64) -> u64 {
        if left >= right {
            left - right
        } else {
            left + PRIME - right
        }
    }

    pub(super) fn mul(left: u64, right: u64) -> u64 {
        reduce(u128::from(left) * u128::from(right))
    }

    pub(super) fn pow(mut base: u64, mut exponent: u64) -> u64 {
        let mut result = 1;
        while exponent > 0 {
            if exponent & 1 == 1 {
                result = mul(result, base);
            }
            base = mul(base, base);
            exponent >>= 1;
        }
        result
    }

    pub(super) fn inverse(value: u64) -> Option<u64> {
        (value != 0).then(|| pow(value, PRIME - 2))
    }

    fn integer(value: &Integer) -> u64 {
        let residue = value % &Integer::from(PRIME as i64);
        u64::try_from(residue.to_i64().expect("a residue below 2^61")).expect("nonnegative")
    }

    /// The residue of a rational, `None` if the prime divides its denominator.
    pub(super) fn rational(value: &Rational) -> Option<u64> {
        Some(mul(
            integer(&value.numerator()),
            inverse(integer(&value.denominator()))?,
        ))
    }

    /// A deterministic pseudo-random residue for (symbol, sample point):
    /// independent values per symbol and point, so a nonzero combination
    /// that vanishes at every point is an accident of probability about
    /// degree / 2^61, never a structural property of the sample.
    pub(super) fn sample(symbol: u64, point: u64) -> u64 {
        // splitmix64 of a seed mixing both indices.
        let mut state = symbol
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .wrapping_add(point.wrapping_mul(0xd1b5_4a32_d192_ed03))
            .wrapping_add(0x2545_f491_4f6c_dd1d);
        state = (state ^ (state >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        state = (state ^ (state >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        state ^= state >> 31;
        state % PRIME
    }
}

/// Value modulo [`residue::PRIME`] of a rational function of plain
/// variables, `None` at a pole or for anything else (functions, non-rational
/// numbers or exponents).
fn evaluate(view: AtomView<'_>, point: &HashMap<Symbol, u64>) -> Option<u64> {
    match view {
        AtomView::Num(_) => residue::rational(&rational(view)?),
        AtomView::Var(variable) => point.get(&variable.get_symbol()).copied(),
        AtomView::Fun(_) => None,
        AtomView::Add(sum) => sum.iter().try_fold(0, |total, term| {
            Some(residue::add(total, evaluate(term, point)?))
        }),
        AtomView::Mul(product) => product.iter().try_fold(1, |total, factor| {
            Some(residue::mul(total, evaluate(factor, point)?))
        }),
        AtomView::Pow(power) => {
            let exponent = rational(power.get_exp())?;
            if !exponent.is_integer() {
                return None;
            }
            let exponent = exponent.numerator().to_i64()?;
            let mut base = evaluate(power.get_base(), point)?;
            if exponent < 0 {
                base = residue::inverse(base)?;
            }
            Some(residue::pow(base, exponent.unsigned_abs()))
        }
    }
}

/// Largest set of expressions an independence proof is attempted for (the
/// elimination costs `count^3` modular operations); larger sets keep the
/// generic lane.
const MAX_PROOF_EXPRESSIONS: usize = 1024;

/// Exact proof that the expressions are linearly independent over Q: a
/// matrix of their values at pseudo-random points modulo a large prime with
/// full rank. A pole, a prime-divisible denominator or an unlucky sample only
/// fails the proof.
fn linearly_independent(expressions: &[&Atom]) -> bool {
    let count = expressions.len();
    if count > MAX_PROOF_EXPRESSIONS {
        return false;
    }
    let symbols = expressions
        .iter()
        .flat_map(|expression| expression.get_all_symbols(false))
        .collect::<BTreeSet<_>>();
    let columns = count + 2;
    let mut rows = vec![Vec::with_capacity(columns); count];
    // Two extra points tolerate an unlucky singular square sample.
    for point in 0..columns as u64 {
        let values = symbols
            .iter()
            .enumerate()
            .map(|(index, symbol)| (*symbol, residue::sample(index as u64, point)))
            .collect::<HashMap<_, _>>();
        for (row, expression) in rows.iter_mut().zip(expressions) {
            let Some(value) = evaluate(expression.as_view(), &values) else {
                return false;
            };
            row.push(value);
        }
    }
    let mut rank = 0;
    for column in 0..columns {
        if rank == count {
            break;
        }
        let Some(pivot) = (rank..count).find(|row| rows[*row][column] != 0) else {
            continue;
        };
        rows.swap(rank, pivot);
        let pivot = rows[rank].clone();
        let inverse = residue::inverse(pivot[column]).expect("a nonzero pivot");
        for row in rows.iter_mut().skip(rank + 1) {
            if row[column] == 0 {
                continue;
            }
            let factor = residue::mul(row[column], inverse);
            for (entry, value) in row.iter_mut().zip(&pivot).skip(column) {
                *entry = residue::sub(*entry, residue::mul(factor, *value));
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

/// Whether the generic lane, multiplying a term's factors in order, adds two
/// products while its leaf prefix (the leaf factors met so far, other than
/// mass powers) is one sum `P`: a coefficient `P + c*P` is then flattened,
/// not merged. Products of the coordinate factors met so far meet on a
/// monomial exactly when their exponents add up equally (exact supports: the
/// generic lane drops zero coefficients too). `None` if a factor does not
/// convert.
fn products_collide(
    grammar: &Grammar<'_>,
    mass: &Mass,
    factors: &[AtomView<'_>],
    convert: &impl Fn(Vec<Atom>) -> Option<MultivariatePolynomial<RationalField, u32>>,
) -> Option<bool> {
    let mut product = None::<MultivariatePolynomial<RationalField, u32>>;
    let mut prefix = 0usize;
    let mut prefix_sum = false;
    for &factor in factors {
        if let AtomView::Num(_) = factor {
            continue;
        }
        if !grammar.contains_coordinate(factor) {
            if !mass.is_power(factor) {
                prefix += 1;
                if prefix == 1 {
                    prefix_sum = matches!(factor, AtomView::Add(_));
                }
            }
            continue;
        }
        let polynomial = convert(vec![factor.to_owned()])?;
        product = Some(match product {
            None => polynomial,
            Some(product) => {
                if prefix == 1 && prefix_sum {
                    let mut seen = BTreeSet::new();
                    for left in product.exponents_iter() {
                        for right in polynomial.exponents_iter() {
                            let sum = left.iter().zip(right).map(|(a, b)| a + b);
                            if !seen.insert(sum.collect::<Vec<_>>()) {
                                return Some(true);
                            }
                        }
                    }
                }
                &product * &polynomial
            }
        });
    }
    Some(false)
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
    let coordinates = prepared.scalar_product_variables.len();
    let mut variables = prepared.scalar_product_variables.as_ref().clone();
    variables.extend(
        mass.variables
            .iter()
            .map(|symbol| PolyVariable::Symbol(*symbol)),
    );
    let variables = Arc::new(variables);
    let convert = |terms: Vec<Atom>| -> Option<MultivariatePolynomial<RationalField, u32>> {
        let polynomial: MultivariatePolynomial<_, u32> = Atom::add_many(terms)
            .try_to_polynomial(&Q, variables.clone())
            .ok()?;
        // Symbolica may add unknown expressions as variables; never accept that.
        (polynomial.variables().as_ref() == variables.as_ref()).then_some(polynomial)
    };
    let monomial = |leaf: &Atom, exponents: &[u32]| {
        let mut monomial = leaf.clone();
        for (symbol, power) in mass.variables.iter().zip(&exponents[coordinates..]) {
            if *power != 0 {
                monomial = monomial * Atom::var(*symbol).pow(Atom::num(i64::from(*power)));
            }
        }
        monomial
    };
    // Polynomial parts grouped by their exact leaf product, in first
    // appearance order (the order only affects the work, never the result).
    // A group is marked when its leaf product times a mass power may
    // normalize to a bare sum.
    let mut index = HashMap::<Atom, usize>::new();
    let mut groups = Vec::<(Atom, Vec<Atom>, bool)>::new();
    for term in terms {
        let factors = match term {
            AtomView::Mul(product) => product.iter().collect::<Vec<_>>(),
            factor => vec![factor],
        };
        let mut number = Rational::one();
        let mut leaves = Vec::new();
        let mut part = Vec::new();
        // The generic lane multiplies the factors in this order. While the
        // leaf factors met so far, other than mass powers, are one sum `P`,
        // its products are `c*P`; adding two of them is not exact here.
        let mut prefix = 0usize;
        let mut prefix_sum = false;
        let mut emulate = false;
        for &factor in &factors {
            if let AtomView::Num(_) = factor {
                number = &number * &rational(factor)?;
            } else if grammar.contains_coordinate(factor) {
                grammar.degree(factor)?;
                emulate |= prefix == 1 && prefix_sum && !part.is_empty();
                part.push(factor.to_owned());
            } else {
                if !mass.is_power(factor) {
                    prefix += 1;
                    if prefix == 1 {
                        prefix_sum = matches!(factor, AtomView::Add(_));
                    }
                }
                leaves.push(factor.to_owned());
            }
        }
        if emulate && products_collide(&grammar, &mass, &factors, &convert)? {
            return None;
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
            groups.push((leaf, Vec::new(), prefix == 1 && prefix_sum));
            groups.len() - 1
        });
        groups[slot].1.push(part);
    }
    // Rebuilt coefficients: coordinate exponents -> products `c * L * M`.
    let mut coefficients = BTreeMap::<Vec<u32>, Vec<Atom>>::new();
    let mut parts = BTreeSet::<Atom>::new();
    let mut leaves = Vec::with_capacity(groups.len());
    // Generic contributions of bare-sum parts, by coordinate exponents.
    let mut bare = BTreeSet::<(Vec<u32>, Atom)>::new();
    for (leaf, terms, may_be_bare) in groups {
        if may_be_bare {
            // Every term of the group is one generic contribution per
            // monomial (cancelling ones included); a bare-sum part must
            // receive at most one.
            for term in &terms {
                for term in &convert(vec![term.clone()])? {
                    let part = monomial(&leaf, term.exponents);
                    if matches!(part.as_view(), AtomView::Add(_))
                        && !bare.insert((term.exponents[..coordinates].to_vec(), part))
                    {
                        return None;
                    }
                }
            }
        }
        for term in &convert(terms)? {
            let part = monomial(&leaf, term.exponents);
            coefficients
                .entry(term.exponents[..coordinates].to_vec())
                .or_default()
                .push(Atom::num(term.coefficient.clone()) * &part);
            if !mass.variables.is_empty() {
                parts.insert(part);
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
