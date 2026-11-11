//! S2: exact integer emptiness of one AND branch from residues modulo a small
//! prime on the chart lattice.
//!
//! Sound and all-integer. An integer point of the branch reduces modulo `p`
//! to a residue vector that satisfies the chart's fixed coordinates and its
//! primitive integer rows `[A | b]` (`A n = b`) modulo `p`, and at which every
//! branch equation (integer coefficients, interpreted as zero) vanishes modulo
//! `p`. If no residue vector of the chart lattice is a common zero, the branch
//! has no integer point. Sector signs and numerator rank are ignored (the test
//! runs on a superset), an equation that contains a non-index variable is not
//! used (dropping a conjunct enlarges the set), and nothing is sampled. A
//! `None` answer proves nothing, in particular never that a point exists.

use std::collections::BTreeMap;

use symbolica::prelude::Integer;

use crate::algebra::CoefficientPolynomial;

use super::super::Case;
use super::engine::checkpoint;
use super::{CaseIntersectionFailure, ModularCheck};

/// The primes tried, in order. The first one proving emptiness is reported.
pub(super) const PRIMES: [u32; 4] = [2, 3, 5, 7];
/// Residue vectors enumerated by one test; a test with `p^k` above it is skipped.
pub(super) const MAX_RESIDUES: u64 = 100_000;
/// Residue vectors times reduced monomials evaluated by one test.
pub(super) const MAX_EVALUATIONS: u64 = 20_000_000;

/// One monomial of an equation reduced modulo `p`: coefficient in `1..p` and
/// `(axis, exponent)` pairs with Fermat-reduced exponents in `1..p`.
type Term = (u32, Vec<(usize, u32)>);

enum Reduced {
    /// The equation contains a non-index variable: not used.
    Unusable,
    /// The equation vanishes identically modulo `p`: no constraint.
    Zero,
    /// A nonzero constant modulo `p`: no integer point satisfies it.
    NonzeroConstant,
    Terms(Vec<Term>),
}

fn residue(value: &Integer, prime: u32) -> u32 {
    let modulus = Integer::from(u64::from(prime));
    let remainder = value % &modulus;
    let remainder = remainder
        .to_i64()
        .expect("a remainder below a small prime fits in i64");
    remainder.rem_euclid(i64::from(prime)) as u32
}

fn reduce(equation: &CoefficientPolynomial, axis_of: &[Option<usize>], prime: u32) -> Reduced {
    let mut terms = BTreeMap::<Vec<(usize, u32)>, u32>::new();
    for (coefficient, exponents) in equation.coefficients.iter().zip(equation.exponents_iter()) {
        let mut monomial = Vec::new();
        for (position, &power) in exponents.iter().enumerate() {
            if power == 0 {
                continue;
            }
            let Some(axis) = axis_of.get(position).copied().flatten() else {
                return Reduced::Unusable;
            };
            // x^e = x^((e-1) mod (p-1) + 1) for every residue x when e >= 1.
            let power = (u32::from(power) - 1) % (prime - 1) + 1;
            monomial.push((axis, power));
        }
        monomial.sort_unstable();
        let value = residue(coefficient, prime);
        let entry = terms.entry(monomial).or_insert(0);
        *entry = (*entry + value) % prime;
    }
    terms.retain(|_, value| *value != 0);
    if terms.is_empty() {
        return Reduced::Zero;
    }
    if terms.len() == 1 && terms.keys().next().is_some_and(Vec::is_empty) {
        return Reduced::NonzeroConstant;
    }
    Reduced::Terms(
        terms
            .into_iter()
            .map(|(monomial, value)| (value, monomial))
            .collect(),
    )
}

fn inverse(value: u32, prime: u32) -> u32 {
    // Fermat: value^(p-2), value != 0 modulo p.
    let mut result = 1u32;
    for _ in 0..prime - 2 {
        result = result * value % prime;
    }
    result
}

/// The residues of the chart lattice modulo `p`: `point + span(directions)`.
struct ResidueSpace {
    point: Vec<u32>,
    directions: Vec<Vec<u32>>,
}

/// `None`: the chart rows are inconsistent modulo `p`, so the chart (and the
/// branch) has no integer point at all.
// Row elimination over two rows at once: index loops are the clear form here.
#[allow(clippy::needless_range_loop)]
fn residue_space<const N: usize>(parent: &Case<N>, prime: u32) -> Option<ResidueSpace> {
    let mut rows: Vec<Vec<u32>> = Vec::new();
    for (axis, value) in parent.fixed().iter().enumerate() {
        if let Some(value) = value {
            let mut row = vec![0; N + 1];
            row[axis] = 1;
            row[N] = i64::from(*value).rem_euclid(i64::from(prime)) as u32;
            rows.push(row);
        }
    }
    if let Some(affine) = parent.affine() {
        for source in affine.primitive_matrix().row_iter() {
            debug_assert_eq!(source.len(), N + 1);
            rows.push(source.iter().map(|value| residue(value, prime)).collect());
        }
    }
    // Reduced row echelon form over GF(p).
    let mut pivots = Vec::<usize>::new();
    let mut rank = 0;
    for column in 0..N {
        let Some(found) = (rank..rows.len()).find(|&row| rows[row][column] != 0) else {
            continue;
        };
        rows.swap(rank, found);
        let scale = inverse(rows[rank][column], prime);
        for entry in rows[rank].iter_mut() {
            *entry = *entry * scale % prime;
        }
        for row in 0..rows.len() {
            if row == rank || rows[row][column] == 0 {
                continue;
            }
            let factor = rows[row][column];
            for entry in 0..=N {
                let subtract = factor * rows[rank][entry] % prime;
                rows[row][entry] = (rows[row][entry] + prime - subtract) % prime;
            }
        }
        pivots.push(column);
        rank += 1;
    }
    if rows[rank..].iter().any(|row| row[N] != 0) {
        return None;
    }
    let mut point = vec![0; N];
    for (row, &column) in pivots.iter().enumerate() {
        point[column] = rows[row][N];
    }
    let mut directions = Vec::new();
    for free in (0..N).filter(|column| !pivots.contains(column)) {
        let mut direction = vec![0; N];
        direction[free] = 1;
        for (row, &column) in pivots.iter().enumerate() {
            direction[column] = (prime - rows[row][free]) % prime;
        }
        directions.push(direction);
    }
    Some(ResidueSpace { point, directions })
}

/// Whether the reduced equations have a common zero on the residue space.
/// `None`: the test is above the residue or evaluation budget.
#[allow(clippy::needless_range_loop, clippy::type_complexity)]
fn common_zero(
    space: &ResidueSpace,
    equations: &[&Vec<Term>],
    prime: u32,
) -> Result<Option<bool>, CaseIntersectionFailure> {
    // Only the axes of these equations matter: project the space onto them
    // and keep an independent spanning set of the projected directions.
    let mut axes: Vec<usize> = equations
        .iter()
        .flat_map(|terms| terms.iter())
        .flat_map(|(_, monomial)| monomial.iter().map(|&(axis, _)| axis))
        .collect();
    axes.sort_unstable();
    axes.dedup();
    let width = axes.len();
    let mut basis: Vec<Vec<u32>> = space
        .directions
        .iter()
        .map(|direction| axes.iter().map(|&axis| direction[axis]).collect())
        .collect();
    let mut rank = 0;
    for column in 0..width {
        let Some(found) = (rank..basis.len()).find(|&row| basis[row][column] != 0) else {
            continue;
        };
        basis.swap(rank, found);
        let scale = inverse(basis[rank][column], prime);
        for entry in basis[rank].iter_mut() {
            *entry = *entry * scale % prime;
        }
        for row in 0..basis.len() {
            if row == rank || basis[row][column] == 0 {
                continue;
            }
            let factor = basis[row][column];
            for entry in 0..width {
                let subtract = factor * basis[rank][entry] % prime;
                basis[row][entry] = (basis[row][entry] + prime - subtract) % prime;
            }
        }
        rank += 1;
    }
    basis.truncate(rank);
    let Some(residues) = u64::from(prime).checked_pow(rank as u32) else {
        return Ok(None);
    };
    let terms: u64 = equations.iter().map(|terms| terms.len() as u64).sum();
    if residues > MAX_RESIDUES || residues.saturating_mul(terms) > MAX_EVALUATIONS {
        return Ok(None);
    }
    // Equations in local axis positions; powers[value][exponent].
    let local: Vec<Vec<(u32, Vec<(usize, u32)>)>> = equations
        .iter()
        .map(|terms| {
            terms
                .iter()
                .map(|(value, monomial)| {
                    let monomial = monomial
                        .iter()
                        .map(|&(axis, power)| (axes.binary_search(&axis).unwrap(), power))
                        .collect();
                    (*value, monomial)
                })
                .collect()
        })
        .collect();
    let powers: Vec<Vec<u32>> = (0..prime)
        .map(|value| {
            let mut row = vec![1u32; prime as usize];
            for exponent in 1..prime as usize {
                row[exponent] = row[exponent - 1] * value % prime;
            }
            row
        })
        .collect();
    let origin: Vec<u32> = axes.iter().map(|&axis| space.point[axis]).collect();
    let mut digits = vec![0u32; rank];
    let mut values = vec![0u32; width];
    for visited in 0..residues {
        if visited % 4096 == 0 {
            checkpoint()?;
        }
        values.copy_from_slice(&origin);
        for (digit, direction) in digits.iter().zip(&basis) {
            if *digit == 0 {
                continue;
            }
            for (value, component) in values.iter_mut().zip(direction) {
                *value = (*value + digit * component) % prime;
            }
        }
        let all_vanish = local.iter().all(|terms| {
            terms.iter().fold(0u32, |sum, (coefficient, monomial)| {
                let product = monomial
                    .iter()
                    .fold(*coefficient, |product, &(axis, power)| {
                        product * powers[values[axis] as usize][power as usize] % prime
                    });
                (sum + product) % prime
            }) == 0
        });
        if all_vanish {
            return Ok(Some(true));
        }
        for digit in digits.iter_mut() {
            *digit += 1;
            if *digit < prime {
                break;
            }
            *digit = 0;
        }
    }
    Ok(Some(false))
}

/// The first prime of [`PRIMES`] proving the AND branch `parent` and
/// `equations` integer-empty, or `None` when no test applies or proves it.
pub(super) fn integer_empty_prime<const N: usize>(
    parent: &Case<N>,
    equations: &[CoefficientPolynomial],
    indices: &[usize; N],
) -> Result<Option<u32>, CaseIntersectionFailure> {
    let Some(first) = equations.first() else {
        return Ok(None);
    };
    let mut axis_of = vec![None; first.nvars()];
    for (axis, &position) in indices.iter().enumerate() {
        if let Some(slot) = axis_of.get_mut(position) {
            *slot = Some(axis);
        }
    }
    for prime in PRIMES {
        checkpoint()?;
        let mut reduced = Vec::new();
        for equation in equations {
            if equation.nvars() != axis_of.len() {
                return Ok(None);
            }
            match reduce(equation, &axis_of, prime) {
                Reduced::Unusable | Reduced::Zero => {}
                Reduced::NonzeroConstant => return Ok(Some(prime)),
                Reduced::Terms(terms) => reduced.push(terms),
            }
        }
        let Some(space) = residue_space(parent, prime) else {
            return Ok(Some(prime));
        };
        if reduced.is_empty() {
            continue;
        }
        let all: Vec<&Vec<Term>> = reduced.iter().collect();
        match common_zero(&space, &all, prime)? {
            Some(false) => return Ok(Some(prime)),
            // A joint residue zero is a residue zero of every conjunct.
            Some(true) => continue,
            None => {
                for terms in &reduced {
                    if common_zero(&space, &[terms], prime)? == Some(false) {
                        return Ok(Some(prime));
                    }
                }
            }
        }
    }
    Ok(None)
}

/// Diagnostic twin of [`integer_empty_prime`] for the S4 record: the outcome
/// at EVERY prime of [`PRIMES`] instead of the first proof, with the same
/// reductions, lattice and budgets. Decides nothing.
pub(super) fn residue_checks<const N: usize>(
    parent: &Case<N>,
    equations: &[CoefficientPolynomial],
    indices: &[usize; N],
) -> Result<Vec<(u32, ModularCheck)>, CaseIntersectionFailure> {
    let Some(first) = equations.first() else {
        return Ok(Vec::new());
    };
    let mut axis_of = vec![None; first.nvars()];
    for (axis, &position) in indices.iter().enumerate() {
        if let Some(slot) = axis_of.get_mut(position) {
            *slot = Some(axis);
        }
    }
    if equations
        .iter()
        .any(|equation| equation.nvars() != axis_of.len())
    {
        return Ok(Vec::new());
    }
    let mut checks = Vec::new();
    'primes: for prime in PRIMES {
        checkpoint()?;
        let mut reduced = Vec::new();
        for equation in equations {
            match reduce(equation, &axis_of, prime) {
                Reduced::Unusable | Reduced::Zero => {}
                Reduced::NonzeroConstant => {
                    checks.push((prime, ModularCheck::ProvedEmpty));
                    continue 'primes;
                }
                Reduced::Terms(terms) => reduced.push(terms),
            }
        }
        let Some(space) = residue_space(parent, prime) else {
            checks.push((prime, ModularCheck::ProvedEmpty));
            continue;
        };
        if reduced.is_empty() {
            checks.push((prime, ModularCheck::NoConstraint));
            continue;
        }
        let all: Vec<&Vec<Term>> = reduced.iter().collect();
        let check = match common_zero(&space, &all, prime)? {
            Some(false) => ModularCheck::ProvedEmpty,
            Some(true) => ModularCheck::CommonZero,
            None => {
                let mut check = ModularCheck::ZeroPerEquation;
                for terms in &reduced {
                    match common_zero(&space, &[terms], prime)? {
                        Some(false) => {
                            check = ModularCheck::ProvedEmpty;
                            break;
                        }
                        Some(true) => {}
                        None => check = ModularCheck::OverBudget,
                    }
                }
                check
            }
        };
        checks.push((prime, check));
    }
    Ok(checks)
}

#[cfg(test)]
pub(super) fn residue_space_dimension<const N: usize>(
    parent: &Case<N>,
    prime: u32,
) -> Option<usize> {
    residue_space(parent, prime).map(|space| space.directions.len())
}
