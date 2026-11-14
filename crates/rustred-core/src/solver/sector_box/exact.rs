//! Exact lane and box-system certificates.
//!
//! **Exact lane.** The selected rows (re-instantiated from provenance, see
//! [`super::system`]) are replayed over Q(free parameters), with the fixed
//! parameters substituted exactly, by Symbolica's `SparseRowReducer` with
//! `LuLMode::None`. Every row must reproduce its modular pivot (otherwise
//! the fixed point is special and the replay is refused); back
//! substitution is restricted to the closure of the requested integrals.
//! This is the authority the finite-field lane is compared against.
//!
//! **Certificates.** A reduction `T = sum_m c_m M_m` is certified by showing
//! that `route(T) - sum_m c_m M_m` lies in the span of the recorded rows
//! (IBP identities at the recorded seeds, the router's automorphism
//! relations and routes), by forward reduction against an echelon form of
//! those rows:
//! - *exact*: over Q(free) with the exact rows ([`certify_exact`]);
//! - *probabilistic*: modulo an independent prime at an independent point,
//!   with the rows produced by the Zp template compiler of the modular pass
//!   at that point ([`certify_probabilistic`]), i.e. independently of the
//!   exact instantiation, of the probe kernel and of the reconstruction.
//!   The session refuses a certificate prime that the reconstruction
//!   lifted over or was verified at (it would not be independent).
//!
//! Like `certify_laporta`, this verifies the reduction; it does not prove
//! that the masters are independent.

use std::collections::HashMap;

use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::rational::RationalField;
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::domains::{Field, RingOps};
use symbolica::poly::polynomial::MultivariatePolynomial;
use symbolica::prelude::{IntegerRing, Q, Rational, Ring, Z};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::algebra::Coefficient;
use crate::algebra::reconstruct::{RationalFunction, rational_image};

use super::super::SolverError;
use super::probe::LaneRequest;
use super::rows::Zp;
use super::system::ExactField;

/// Substitute the fixed parameter values into an exact coefficient; `None`
/// at a pole.
fn substitute(coefficient: &Coefficient, fixed: &[Option<Rational>]) -> Option<Coefficient> {
    if fixed.iter().all(Option::is_none) {
        return Some(coefficient.clone());
    }
    let variables = coefficient.numerator.variables().clone();
    let restrict = |polynomial: &crate::algebra::CoefficientPolynomial| {
        let mut out: MultivariatePolynomial<RationalField, u16> =
            MultivariatePolynomial::new(&Q, Some(polynomial.nterms()), variables.clone());
        for term in polynomial {
            let mut value = Rational::from(term.coefficient.clone());
            let mut exponents = term.exponents.to_vec();
            for (variable, exponent) in exponents.iter_mut().enumerate() {
                if *exponent == 0 {
                    continue;
                }
                if let Some(Some(fixed)) = fixed.get(variable) {
                    value = &value * &fixed.pow(u64::from(*exponent));
                    *exponent = 0;
                }
            }
            if !value.is_zero() {
                out.append_monomial(value, &exponents);
            }
        }
        out
    };
    let numerator = restrict(&coefficient.numerator);
    let denominator = restrict(&coefficient.denominator);
    if denominator.is_zero() {
        return None;
    }
    Some(<Coefficient as FromNumeratorAndDenominator<
        RationalField,
        IntegerRing,
        u16,
    >>::from_num_den(numerator, denominator, &Z, true))
}

/// Insert `rows` in order; row `i` must have pivot `expected[i]`. Returns
/// the index of the first row that does not.
fn replay<F: Field>(
    field: &F,
    ncols: usize,
    rows: &[Vec<(u32, F::Element)>],
    expected: &[u32],
) -> Result<SparseRowReducer<F>, usize> {
    let mut reducer = SparseRowReducer::new(ncols as u32, field.clone(), LuLMode::None);
    let mut values = Vec::new();
    let mut ids = Vec::new();
    for (index, (row, expected)) in rows.iter().zip(expected).enumerate() {
        values.clear();
        ids.clear();
        for (column, value) in row {
            if !field.is_zero(value) {
                ids.push(*column);
                values.push(value.clone());
            }
        }
        if reducer.add_row(&values, &ids) != Some(*expected) {
            return Err(index);
        }
    }
    Ok(reducer)
}

fn u_row<F: Field>(reducer: &SparseRowReducer<F>, row: u32) -> (&[u32], &[F::Element]) {
    let u = reducer.u();
    let (start, end) = (u.row_ptrs()[row as usize], u.row_ptrs()[row as usize + 1]);
    (&u.col_idcs()[start..end], &u.values()[start..end])
}

/// Reduced rows of `targets` over the non-pivot columns of their closure:
/// `column -> [(candidate, weight)]` with `x_column = sum weight * x_candidate`.
fn reduce_closure<F: Field>(
    reducer: &SparseRowReducer<F>,
    field: &F,
    targets: &[u32],
) -> HashMap<u32, Vec<(u32, F::Element)>> {
    let pivots = reducer.pivots();
    let mut seen = vec![false; pivots.len()];
    let mut stack: Vec<u32> = Vec::new();
    for &column in targets {
        if !seen[column as usize] {
            seen[column as usize] = true;
            stack.push(column);
        }
    }
    let mut closure_pivots = Vec::new();
    while let Some(column) = stack.pop() {
        if let Some(row) = pivots[column as usize] {
            closure_pivots.push(column);
            for &next in &u_row(reducer, row).0[1..] {
                if !seen[next as usize] {
                    seen[next as usize] = true;
                    stack.push(next);
                }
            }
        }
    }
    closure_pivots.sort_unstable();
    let mut reduced: HashMap<u32, Vec<(u32, F::Element)>> = HashMap::new();
    for &column in closure_pivots.iter().rev() {
        let row = pivots[column as usize].expect("closure pivot");
        let (columns, values) = u_row(reducer, row);
        debug_assert_eq!(columns[0], column);
        let inverse = field.inv(&values[0]);
        let mut accumulator: HashMap<u32, F::Element> = HashMap::new();
        for (&next, value) in columns[1..].iter().zip(&values[1..]) {
            let scale = field.neg(&field.mul(value, &inverse));
            match pivots[next as usize] {
                None => {
                    let entry = accumulator.entry(next).or_insert_with(|| field.zero());
                    field.add_assign(entry, &scale);
                }
                Some(_) => {
                    for (candidate, weight) in &reduced[&next] {
                        let entry = accumulator
                            .entry(*candidate)
                            .or_insert_with(|| field.zero());
                        field.add_assign(entry, &field.mul(&scale, weight));
                    }
                }
            }
        }
        let mut row: Vec<(u32, F::Element)> = accumulator
            .into_iter()
            .filter(|(_, value)| !field.is_zero(value))
            .collect();
        row.sort_unstable_by_key(|(candidate, _)| *candidate);
        reduced.insert(column, row);
    }
    for &column in targets {
        if pivots[column as usize].is_none() {
            reduced.insert(column, vec![(column, field.one())]);
        }
    }
    reduced
}

/// Whether `vector` (column -> value) lies in the row span: forward
/// reduction against the echelon rows must leave nothing.
fn in_span<F: Field>(
    reducer: &SparseRowReducer<F>,
    field: &F,
    vector: Vec<(u32, F::Element)>,
) -> bool {
    let pivots = reducer.pivots();
    let mut dense: std::collections::BTreeMap<u32, F::Element> = std::collections::BTreeMap::new();
    for (column, value) in vector {
        let entry = dense.entry(column).or_insert_with(|| field.zero());
        field.add_assign(entry, &value);
    }
    while let Some((&column, value)) = dense.iter().next() {
        let value = value.clone();
        dense.remove(&column);
        if field.is_zero(&value) {
            continue;
        }
        let Some(row) = pivots.get(column as usize).copied().flatten() else {
            return false;
        };
        let (columns, values) = u_row(reducer, row);
        let factor = field.div(&value, &values[0]);
        for (&next, entry) in columns[1..].iter().zip(&values[1..]) {
            let slot = dense.entry(next).or_insert_with(|| field.zero());
            field.sub_mul_assign(slot, &factor, entry);
        }
    }
    true
}

/// The exact selected rows with the fixed values substituted.
fn exact_rows<const N: usize>(
    request: &LaneRequest<N>,
) -> Result<Vec<Vec<(u32, Coefficient)>>, SolverError> {
    let mut rows = Vec::with_capacity(request.system.rows.len());
    for (index, row) in request.system.rows.iter().enumerate() {
        let mut out = Vec::with_capacity(row.len());
        for (column, coefficient) in row {
            let value = substitute(coefficient, &request.fixed).ok_or_else(|| {
                SolverError::ExactReplay(format!(
                    "row {index}: a coefficient has a pole at the fixed parameter values"
                ))
            })?;
            if !value.is_zero() {
                out.push((*column, value));
            }
        }
        rows.push(out);
    }
    Ok(rows)
}

/// Result of the exact lane.
pub(crate) struct ExactResult {
    /// `targets x masters`, exact in the free parameters.
    pub coefficients: Vec<Coefficient>,
    pub rows: usize,
    pub seconds: f64,
}

/// The exact lane: replay over Q(free) and back substitution on the
/// closure of the targets.
pub(crate) fn solve_exact<const N: usize>(
    request: &LaneRequest<N>,
) -> Result<ExactResult, SolverError> {
    let started = std::time::Instant::now();
    let system = &request.system;
    let field = ExactField::new(Z);
    let rows = exact_rows(request)?;
    let reducer = replay(&field, system.columns.len(), &rows, &system.pivots).map_err(|index| {
        SolverError::ExactReplay(format!(
            "row {index} does not reproduce its modular pivot at the fixed parameter values"
        ))
    })?;
    let mut demand: Vec<u32> = request
        .routes
        .iter()
        .flat_map(|route| route.iter().map(|(column, _)| *column))
        .collect();
    demand.sort_unstable();
    demand.dedup();
    let reduced = reduce_closure(&reducer, &field, &demand);
    let masters = &system.masters;
    let mut coefficients = vec![field.zero(); request.routes.len() * masters.len()];
    for (target, route) in request.routes.iter().enumerate() {
        for (column, factor) in route {
            let factor = substitute(factor, &request.fixed).ok_or_else(|| {
                SolverError::ExactReplay(
                    "a route coefficient has a pole at the fixed values".into(),
                )
            })?;
            for (candidate, weight) in &reduced[column] {
                let position = masters.binary_search(candidate).map_err(|_| {
                    SolverError::ExactReplay(format!(
                        "target {target} has a nonzero coefficient on the non-master column {:?}",
                        system.columns[*candidate as usize]
                    ))
                })?;
                let slot = &mut coefficients[target * masters.len() + position];
                field.add_assign(slot, &field.mul(&factor, weight));
            }
        }
    }
    Ok(ExactResult {
        coefficients,
        rows: rows.len(),
        seconds: started.elapsed().as_secs_f64(),
    })
}

/// How a certificate checked the reduction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertificateMode {
    /// Over Q(free parameters) with the exact rows.
    Exact,
    /// Modulo an independent prime at a random point.
    Probabilistic,
}

impl CertificateMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Probabilistic => "probabilistic",
        }
    }
}

/// What a box certificate checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoxCertificate {
    pub mode: CertificateMode,
    /// The prime of a probabilistic certificate (never a prime the
    /// reconstruction lifted over or was verified at).
    pub prime: Option<u64>,
    /// Recorded rows replayed (all reproduced their modular pivots).
    pub rows: usize,
    /// Reductions shown to lie in the span of the rows.
    pub rules: usize,
}

/// `coefficients` must be `targets x masters`.
fn check_shape<T>(
    coefficients: &[Vec<T>],
    targets: usize,
    masters: usize,
) -> Result<(), SolverError> {
    if coefficients.len() != targets || coefficients.iter().any(|row| row.len() != masters) {
        return Err(SolverError::InvalidInput(format!(
            "a certified solution needs {targets} x {masters} coefficients"
        )));
    }
    Ok(())
}

/// Exact certificate of `coefficients[t][m]` (exact in the free
/// parameters, as returned by the exact lane or converted from the
/// finite-field lane).
pub(crate) fn certify_exact<const N: usize>(
    request: &LaneRequest<N>,
    coefficients: &[Vec<Coefficient>],
) -> Result<BoxCertificate, SolverError> {
    let system = &request.system;
    check_shape(coefficients, request.routes.len(), system.masters.len())?;
    let field = ExactField::new(Z);
    let rows = exact_rows(request)?;
    let reducer = replay(&field, system.columns.len(), &rows, &system.pivots).map_err(|index| {
        SolverError::Certification(format!("row {index} does not reproduce its modular pivot"))
    })?;
    for (target, route) in request.routes.iter().enumerate() {
        let mut vector = Vec::with_capacity(route.len() + system.masters.len());
        for (column, factor) in route {
            let factor = substitute(factor, &request.fixed)
                .ok_or_else(|| SolverError::Certification("route coefficient pole".into()))?;
            vector.push((*column, factor));
        }
        for (position, &column) in system.masters.iter().enumerate() {
            let coefficient = &coefficients[target][position];
            if !coefficient.is_zero() {
                vector.push((column, field.neg(coefficient)));
            }
        }
        if !in_span(&reducer, &field, vector) {
            return Err(SolverError::Certification(format!(
                "the reduction of target {:?} is not in the span of the recorded rows",
                request.targets[target]
            )));
        }
    }
    Ok(BoxCertificate {
        mode: CertificateMode::Exact,
        prime: None,
        rows: rows.len(),
        rules: request.routes.len(),
    })
}

/// Probabilistic certificate at `prime` and the free-parameter value `x`:
/// `rows` are the recorded rows produced independently at that point (the
/// Zp template compiler), `routes` the Zp routes of the targets onto the
/// system's columns.
pub(crate) fn certify_probabilistic<const N: usize>(
    request: &LaneRequest<N>,
    field: &Zp64,
    rows: &[Vec<(u32, Zp)>],
    routes: &[Vec<(u32, Zp)>],
    coefficients: &[Vec<RationalFunction>],
    x: &Rational,
) -> Result<BoxCertificate, SolverError> {
    let system = &request.system;
    check_shape(coefficients, routes.len(), system.masters.len())?;
    let reducer = replay(field, system.columns.len(), rows, &system.pivots).map_err(|index| {
        SolverError::Certification(format!(
            "row {index} does not reproduce its modular pivot at the certificate point"
        ))
    })?;
    let point = rational_image(x, field)
        .ok_or_else(|| SolverError::Certification("certificate point pole".into()))?;
    for (target, route) in routes.iter().enumerate() {
        let mut vector = route.clone();
        for (position, &column) in system.masters.iter().enumerate() {
            let function = &coefficients[target][position];
            if function.is_zero() {
                continue;
            }
            let value = function.evaluate(field, &point).ok_or_else(|| {
                SolverError::Certification(
                    "a coefficient has a pole at the certificate point".into(),
                )
            })?;
            vector.push((column, field.neg(&value)));
        }
        if !in_span(&reducer, field, vector) {
            return Err(SolverError::Certification(format!(
                "the reduction of target {:?} fails the probabilistic span check at prime {}",
                request.targets[target],
                field.get_prime()
            )));
        }
    }
    Ok(BoxCertificate {
        mode: CertificateMode::Probabilistic,
        prime: Some(field.get_prime()),
        rows: rows.len(),
        rules: routes.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbolica::domains::finite_field::FiniteFieldCore;

    #[test]
    fn span_and_closure_over_a_small_field_system() {
        let field = Zp64::new(1_000_003);
        let e = |v: u64| field.to_element(v);
        // Columns 0..4 (0 hardest). Rows: x0 = 2 x2 + x3, x1 = x2 - x3.
        let rows = vec![
            vec![(0u32, e(1)), (2, field.neg(&e(2))), (3, field.neg(&e(1)))],
            vec![(1u32, e(1)), (2, field.neg(&e(1))), (3, e(1))],
        ];
        let reducer = replay(&field, 4, &rows, &[0, 1]).unwrap();
        assert!(replay(&field, 4, &rows, &[1, 0]).is_err());
        let reduced = reduce_closure(&reducer, &field, &[0, 1, 3]);
        assert_eq!(reduced[&0], vec![(2, e(2)), (3, e(1))]);
        assert_eq!(reduced[&1], vec![(2, e(1)), (3, field.neg(&e(1)))]);
        assert_eq!(reduced[&3], vec![(3, e(1))]);
        // x0 - x1 - x2 - 2 x3 = 0 is in the span; x0 - x2 is not.
        assert!(in_span(
            &reducer,
            &field,
            vec![
                (0, e(1)),
                (1, field.neg(&e(1))),
                (2, field.neg(&e(1))),
                (3, field.neg(&e(2)))
            ]
        ));
        assert!(!in_span(
            &reducer,
            &field,
            vec![(0, e(1)), (2, field.neg(&e(1)))]
        ));
    }
}
