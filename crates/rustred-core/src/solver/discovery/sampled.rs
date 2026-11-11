//! Sampled exact backend (p5l-lift-sampled, hypothesis H2): reconstruct ONLY the normalized target row of the fixed
//! selected trace from modular replays, without any characteristic-zero elimination.
//!
//! Every black-box call replays the unchanged source trace over Z/p at a point of the active coefficient variables with
//! Symbolica's native sparse GPLU (as the semi-numerical backend does) and stops at the target pivot. A point is usable
//! only if every source denominator is nonzero, the pivot trace equals the generic one and no column outside the generic
//! support appears. Reconstruction (degree probe, common denominator, shared numerator box, CRT + rational
//! reconstruction) is in `recon.rs`. Authority: the reconstructed row is accepted only if it reproduces the replayed row
//! (every column, including the absent ones) at 6 fresh points of a prime not used for reconstruction, at 3 distinct
//! values of d, and a mutated copy (one numerator coefficient + 1) is rejected at those points. This is a modular
//! (probabilistic) certificate, the P61 x 3 d numeric-replay standard of the 5L repair ladder, not a char-0 proof.

#![allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "oracle plumbing of the sampled backend"
)]

use symbolica::domains::finite_field::{FiniteFieldCore, ToFiniteField, Zp64};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::domains::{Field, Ring, SelfRing};
use symbolica::poly::polynomial::MultivariatePolynomial;
use symbolica::prelude::{IntegerRing, Z};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::algebra::Coefficient;

use super::variables::FrameVariables;
use super::{ExactRow, Integral, IntegralOrder, MaterializationError, MaterializationEvent, Term};

mod recon;
pub(super) use recon::MAX_BOX;
use recon::{Fp, Oracle, Rng};

struct Frame {
    rows: Vec<Vec<(u32, Coefficient)>>,
    columns_with_sentinel: u32,
    target: u32,
    nvars: usize,
}

impl Frame {
    /// Replay mod p: the target U row (column, value) and the pivot trace, or None at a pole / missing target.
    fn replay(&self, p: u64, point: &[u64]) -> Option<(Vec<(u32, u64)>, Vec<Option<u32>>)> {
        let field = Zp64::new(p);
        let pt: Vec<_> = point.iter().map(|&x| field.to_element(x)).collect();
        let mut reducer =
            SparseRowReducer::new(self.columns_with_sentinel, field.clone(), LuLMode::None);
        let mut values = Vec::new();
        let mut ids = Vec::new();
        let mut pivots = Vec::new();
        for row in &self.rows {
            values.clear();
            ids.clear();
            for (column, coefficient) in row {
                let numerator = coefficient.numerator.evaluate_with_coeff_map(
                    |v| v.to_finite_field(&field),
                    &pt,
                    &field,
                );
                let denominator = coefficient.denominator.evaluate_with_coeff_map(
                    |v| v.to_finite_field(&field),
                    &pt,
                    &field,
                );
                if field.is_zero(&denominator) {
                    return None;
                }
                let value = field.div(&numerator, &denominator);
                if !field.is_zero(&value) {
                    ids.push(*column);
                    values.push(value);
                }
            }
            let pivot = reducer.add_row(&values, &ids);
            pivots.push(pivot);
            if pivot == Some(self.target) {
                let upper = reducer.u();
                let r = upper.nrows() as usize - 1;
                let (s, e) = (upper.row_ptrs()[r], upper.row_ptrs()[r + 1]);
                let row = upper.col_idcs()[s..e]
                    .iter()
                    .copied()
                    .zip(upper.values()[s..e].iter().map(|v| field.from_element(v)))
                    .collect();
                return Some((row, pivots));
            }
        }
        None
    }
}

struct FrameOracle<'a> {
    frame: &'a Frame,
    support: Vec<u32>,
    pivots: Vec<Option<u32>>,
}

impl Oracle for FrameOracle<'_> {
    fn nvars(&self) -> usize {
        self.frame.nvars
    }
    fn ncols(&self) -> usize {
        self.support.len()
    }
    fn eval(&self, p: u64, point: &[u64]) -> Option<Vec<u64>> {
        let (row, pivots) = self.frame.replay(p, point)?;
        if pivots != self.pivots {
            return None;
        }
        let mut out = vec![0u64; self.support.len()];
        for (column, value) in row {
            out[self.support.binary_search(&column).ok()?] = value;
        }
        Some(out)
    }
}

fn err(message: impl Into<String>) -> MaterializationError {
    MaterializationError::SemiNumericalReconstruction(format!("sampled: {}", message.into()))
}

fn integer_poly(
    template: &MultivariatePolynomial<IntegerRing, u16>,
    mons: &[Vec<u16>],
    coeffs: &[Rational],
    scale: &Integer,
) -> MultivariatePolynomial<IntegerRing, u16> {
    let mut poly = template.zero();
    for (m, q) in mons.iter().zip(coeffs) {
        if q.is_zero() {
            continue;
        }
        // q * scale is an integer by construction of `scale` (lcm of the denominators)
        let value = &(scale / &q.denominator()) * &q.numerator();
        poly.append_monomial(value, m);
    }
    poly
}

fn lcm_denominators(coeffs: &[Rational]) -> Integer {
    coeffs.iter().fold(Integer::one(), |acc, q| {
        if q.is_zero() {
            acc
        } else {
            acc.lcm(&q.denominator())
        }
    })
}

pub(super) fn materialize<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    variables: &FrameVariables,
    threads: usize,
    seed: u64,
    mut observe: impl FnMut(MaterializationEvent<N>),
) -> Result<ExactRow<N>, MaterializationError> {
    let start = std::time::Instant::now();
    if variables.active_len() == 0 {
        // a constant frame is cheap for the ordinary sparse authority
        return super::sparse_materialize(rows, columns, order, target_column, variables, observe);
    }
    observe(MaterializationEvent::SemiNumericalStarted {
        rows: rows.len(),
        columns: columns.len(),
        variables: variables.active_len(),
    });
    let columns_with_sentinel = u32::try_from(columns.len())
        .ok()
        .and_then(|n| n.checked_add(1))
        .ok_or(MaterializationError::TooManyColumns)?;
    let mapped: Vec<Vec<(u32, Coefficient)>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|term| {
                    let column = columns
                        .binary_search_by(|candidate| order.compare(candidate, &term.integral))
                        .map_err(|_| MaterializationError::TargetAbsent)?;
                    Ok((column as u32, variables.map_coefficient(&term.coefficient)?))
                })
                .collect::<Result<_, MaterializationError>>()
        })
        .collect::<Result<_, _>>()?;
    let template = mapped
        .iter()
        .flatten()
        .next()
        .ok_or(MaterializationError::TargetAbsent)?
        .1
        .numerator
        .clone();
    let frame = Frame {
        rows: mapped,
        columns_with_sentinel,
        target: target_column as u32,
        nvars: variables.active_len(),
    };
    let primes = recon::primes_below_2_62(14, 0);
    let mut rng = Rng(seed | 1);
    // generic support and pivot branch: majority over 5 random points of the first prime
    let mut branches: Vec<((Vec<u32>, Vec<Option<u32>>), usize)> = Vec::new();
    for _ in 0..5 {
        let point: Vec<u64> = (0..frame.nvars).map(|_| rng.element(primes[0])).collect();
        if let Some((row, pivots)) = frame.replay(primes[0], &point) {
            let key = (row.iter().map(|x| x.0).collect::<Vec<_>>(), pivots);
            match branches.iter_mut().find(|(k, _)| *k == key) {
                Some(entry) => entry.1 += 1,
                None => branches.push((key, 1)),
            }
        }
    }
    let ((mut support, pivots), _) = branches
        .into_iter()
        .max_by_key(|x| x.1)
        .ok_or_else(|| err("no usable generic point"))?;
    support.sort_unstable();
    let oracle = FrameOracle {
        frame: &frame,
        support: support.clone(),
        pivots,
    };
    let f0 = Fp { p: primes[0] };
    let info = recon::probe_degrees(&oracle, &f0, rng.next(), threads, 4000).map_err(err)?;
    let boxes = recon::boxes(&info);
    eprintln!(
        "SAMPLED degrees rows={} support={} vars={} common_den_total={} F_num_total={} max_total=({},{}) boxes L={} NF={} P={} probe_points={} t={:.1}s",
        rows.len(),
        support.len(),
        frame.nvars,
        info.f_total.1,
        info.f_total.0,
        info.total.iter().map(|x| x.0).max().unwrap_or(0),
        info.total.iter().map(|x| x.1).max().unwrap_or(0),
        boxes.l_mons.len(),
        boxes.nf_mons.len(),
        boxes.p_mons.len(),
        info.points,
        start.elapsed().as_secs_f64()
    );
    let unknowns = (boxes.l_mons.len() + boxes.nf_mons.len()).max(boxes.p_mons.len());
    if unknowns > recon::MAX_BOX {
        // The relation itself is large (dense box beyond the cubic-solve limit): the sampled route cannot help, so the
        // ordinary exact lift keeps its chance (it closes such lifts when the elimination does not swell, e.g. T163 1504 rows).
        eprintln!(
            "SAMPLED fallback: dense box {unknowns} > {} -> targetonlyfactorized exact lift",
            recon::MAX_BOX
        );
        return super::target_only::materialize_factorized(
            rows,
            columns,
            order,
            target_column,
            variables,
            observe,
        );
    }
    let mut lift = recon::Lift::new();
    let mut norm = None;
    let mut confirmed = false;
    for (k, &p) in primes.iter().take(12).enumerate() {
        symbolica::utils::resources::checkpoint().map_err(MaterializationError::Resources)?;
        let f = Fp { p };
        let weights: Vec<u64> = info.weights.iter().map(|&w| w % p).collect();
        match recon::prime_image(
            &oracle,
            &f,
            &boxes,
            &weights,
            &mut norm,
            rng.next(),
            threads,
        ) {
            Ok(image) => {
                confirmed = lift.add(p, &image);
                eprintln!(
                    "SAMPLED prime {k} points={} bad={} candidate={} confirmed={confirmed} t={:.1}s",
                    image.points,
                    image.bad_points,
                    lift.candidate.is_some(),
                    start.elapsed().as_secs_f64()
                );
            }
            Err(e) => eprintln!("SAMPLED prime {k} error: {e}"),
        }
        if confirmed {
            break;
        }
    }
    if !confirmed {
        return Err(err(format!(
            "no confirmed reconstruction after {} primes",
            lift.primes.len()
        )));
    }
    let coeffs = lift.candidate.clone().expect("confirmed candidate");
    // authority: fresh prime, 3 values of d, 2 points each; mutation control
    let vp = recon::primes_below_2_62(1, 40)[0];
    let fv = Fp { p: vp };
    let dpos = variables
        .active_variables()
        .iter()
        .position(|v| v.to_string() == "d");
    let mut points = Vec::new();
    for _ in 0..3 {
        let dval = rng.element(vp);
        for _ in 0..2 {
            let mut x: Vec<u64> = (0..frame.nvars).map(|_| rng.element(vp)).collect();
            if let Some(dp) = dpos {
                x[dp] = dval;
            }
            points.push(x);
        }
    }
    let replays = recon::eval_many(&oracle, vp, &points, threads);
    let pass = points
        .iter()
        .zip(&replays)
        .filter(|(x, y)| matches!((y, recon::eval_rational_row(&fv, &boxes, &coeffs, x)), (Some(y), Some(z)) if *y == z))
        .count();
    let nl = boxes.l_mons.len();
    let mut mutated = coeffs.clone();
    let pos = (nl..mutated.len())
        .find(|&i| !mutated[i].is_zero())
        .ok_or_else(|| err("empty row"))?;
    mutated[pos] = &mutated[pos] + &Rational::from(1);
    let caught = points
        .iter()
        .zip(&replays)
        .filter(
            |(x, y)| match (y, recon::eval_rational_row(&fv, &boxes, &mutated, x)) {
                (Some(y), Some(z)) => *y != z,
                _ => true,
            },
        )
        .count();
    eprintln!(
        "SAMPLED verify prime={vp} points={} pass={pass} mutation_caught={caught} primes_used={} t={:.1}s",
        points.len(),
        lift.primes.len(),
        start.elapsed().as_secs_f64()
    );
    if pass != points.len() || caught != points.len() {
        return Err(err(format!(
            "verification failed: {pass}/{} pass, mutation caught {caught}",
            points.len()
        )));
    }
    // exact row: c_i = P_i / L over the active variables, then the original variable map
    let l_coeffs = &coeffs[..nl];
    let l_scale = lcm_denominators(l_coeffs);
    let l_poly = integer_poly(&template, &boxes.l_mons, l_coeffs, &l_scale);
    let np = boxes.p_mons.len();
    let mut output = Vec::with_capacity(support.len());
    for (i, &column) in support.iter().enumerate() {
        let p_coeffs = &coeffs[nl + i * np..nl + (i + 1) * np];
        if p_coeffs.iter().all(|q| q.is_zero()) {
            continue;
        }
        let p_scale = lcm_denominators(p_coeffs);
        // c = (P*p_scale / p_scale) / (L*l_scale / l_scale) = (P*p_scale * l_scale) / (L*l_scale * p_scale)
        let num =
            integer_poly(&template, &boxes.p_mons, p_coeffs, &p_scale).mul_coeff(l_scale.clone());
        let den = l_poly.clone().mul_coeff(p_scale);
        let active = Coefficient::from_num_den(num, den, &Z, true);
        output.push(Term {
            integral: columns[column as usize],
            coefficient: variables.restore_coefficient(&active)?,
        });
    }
    if output
        .iter()
        .find(|t| t.integral == columns[target_column])
        .is_none_or(|t| !t.coefficient.is_one())
    {
        return Err(err("reconstructed target coefficient is not one"));
    }
    observe(MaterializationEvent::SemiNumericalFinished {
        output_terms: output.len(),
    });
    eprintln!(
        "SAMPLED done terms={} t={:.1}s",
        output.len(),
        start.elapsed().as_secs_f64()
    );
    Ok(output)
}

/// Lift pre-screen (rr-consolidate): a cheap modular estimate of the target row of a fixed trace BEFORE any exact
/// lift. Replays the trace mod p (no characteristic-zero arithmetic): the generic support size gives the RHS term
/// count, one Thiele line through a random point in a random direction gives the total (numerator, denominator)
/// degree of every column and of a random combination F (whose reduced denominator is the common denominator).
/// The line is capped at `max_points` replays: no convergence within the cap means a total degree beyond about
/// `max_points / 2` (`converged = false`). The measurement is diagnostic; the policy belongs to
/// [`crate::node::screen::LiftScreen`].
pub(crate) fn screen<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
    variables: &FrameVariables,
    max_points: usize,
    seed: u64,
) -> Result<crate::node::screen::ScreenMeasure, MaterializationError> {
    let start = std::time::Instant::now();
    let mut measure = crate::node::screen::ScreenMeasure {
        rows: rows.len(),
        columns: columns.len(),
        variables: variables.active_len(),
        target_block: target_column + 1,
        ..Default::default()
    };
    let columns_with_sentinel = u32::try_from(columns.len())
        .ok()
        .and_then(|n| n.checked_add(1))
        .ok_or(MaterializationError::TooManyColumns)?;
    let mapped: Vec<Vec<(u32, Coefficient)>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|term| {
                    let column = columns
                        .binary_search_by(|candidate| order.compare(candidate, &term.integral))
                        .map_err(|_| MaterializationError::TargetAbsent)?;
                    Ok((column as u32, variables.map_coefficient(&term.coefficient)?))
                })
                .collect::<Result<_, MaterializationError>>()
        })
        .collect::<Result<_, _>>()?;
    let frame = Frame {
        rows: mapped,
        columns_with_sentinel,
        target: target_column as u32,
        nvars: variables.active_len(),
    };
    let p = recon::primes_below_2_62(1, 3)[0];
    let mut rng = Rng(seed | 1);
    let mut branches: Vec<((Vec<u32>, Vec<Option<u32>>), usize)> = Vec::new();
    for _ in 0..3 {
        let point: Vec<u64> = (0..frame.nvars).map(|_| rng.element(p)).collect();
        measure.points += 1;
        if let Some((row, pivots)) = frame.replay(p, &point) {
            let key = (row.iter().map(|x| x.0).collect::<Vec<_>>(), pivots);
            match branches.iter_mut().find(|(k, _)| *k == key) {
                Some(entry) => entry.1 += 1,
                None => branches.push((key, 1)),
            }
        }
    }
    let Some(((mut support, pivots), _)) = branches.into_iter().max_by_key(|x| x.1) else {
        measure.note = "no usable generic point".into();
        measure.seconds = start.elapsed().as_secs_f64();
        return Ok(measure);
    };
    support.sort_unstable();
    measure.rhs_terms = support.len().saturating_sub(1);
    if frame.nvars == 0 {
        measure.converged = true;
        measure.seconds = start.elapsed().as_secs_f64();
        return Ok(measure);
    }
    let oracle = FrameOracle {
        frame: &frame,
        support,
        pivots,
    };
    let f = Fp { p };
    let weights: Vec<u64> = (0..oracle.ncols()).map(|_| rng.element(p)).collect();
    let base: Vec<u64> = (0..frame.nvars).map(|_| rng.element(p)).collect();
    let dir: Vec<u64> = (0..frame.nvars).map(|_| rng.element(p)).collect();
    let (mut used, mut bad) = (0usize, 0usize);
    match recon::probe_line(
        &oracle, &f, &base, &dir, &weights, &mut rng, 1, max_points, &mut used, &mut bad,
    ) {
        Ok((cols, fd)) => {
            measure.converged = true;
            measure.max_num_degree = cols.iter().map(|x| x.0).max().unwrap_or(0);
            measure.max_den_degree = cols.iter().map(|x| x.1).max().unwrap_or(0);
            measure.common_den_degree = fd.1;
        }
        Err(e) => measure.note = e,
    }
    measure.points += used;
    measure.bad_points = bad;
    measure.seconds = start.elapsed().as_secs_f64();
    measure.replay_seconds = measure.seconds / measure.points.max(1) as f64;
    Ok(measure)
}
