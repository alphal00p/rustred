//! Per-prime Thiele interpolation of all terminal accumulators, CRT and
//! maximal-quotient rational reconstruction over Q, with stability and
//! fresh-prime verification.
use std::collections::HashSet;
use std::time::Instant;

use rand::{Rng, SeedableRng, rngs::StdRng};
use rayon::prelude::*;
use symbolica::domains::finite_field::{FiniteFieldCore, PrimeIteratorU64, Zp64};
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::domains::{Field, Ring, RingOps};
use symbolica::prelude::{Integer, Rational, Z};

use super::field::{self, Fe, Poly};
use super::replay::{ReplayBatch, ReplayError, replay};
use super::{ModularStatistics, OpTag, Trace, error};
use crate::algebra::{Coefficient, CoefficientContext};
use crate::reduction::ReductionError;
use crate::reduction::ordered_reduction::arithmetic::ModularWeightOptions;

/// Incremental univariate Thiele interpolation over one prime field.
struct Thiele {
    nodes: Vec<Fe>,
    differences: Vec<Fe>,
    p_prev: Poly,
    q_prev: Poly,
    p: Poly,
    q: Poly,
    checks: usize,
    consumed: usize,
    result: Option<Image>,
}

/// A reduced modular image `num/den` with monic denominator; zero has an
/// empty numerator.
#[derive(Clone)]
struct Image {
    numerator: Poly,
    denominator: Poly,
    /// Interpolation nodes used (excluding checks).
    nodes: usize,
}

impl Image {
    fn degrees(&self) -> Option<(usize, usize)> {
        (!self.numerator.is_empty()).then(|| (self.numerator.len() - 1, self.denominator.len() - 1))
    }
}

impl Thiele {
    fn new(f: &Zp64) -> Self {
        Self {
            nodes: vec![],
            differences: vec![],
            p_prev: vec![f.one()],
            q_prev: vec![],
            p: vec![],
            q: vec![f.one()],
            checks: 0,
            consumed: 0,
            result: None,
        }
    }

    /// Consume new samples; true once `required` consecutive fresh samples
    /// agree with the current convergent.
    fn feed(&mut self, f: &Zp64, xs: &[Fe], ys: &[Fe], required: usize) -> bool {
        if self.result.is_some() {
            return true;
        }
        while self.consumed < xs.len() {
            let t = xs[self.consumed];
            let y = ys[self.consumed];
            self.consumed += 1;
            if !self.nodes.is_empty() {
                let dv = field::eval(f, &self.q, t);
                if !f.is_zero(&dv) && f.mul(&y, &dv) == field::eval(f, &self.p, t) {
                    self.checks += 1;
                    if self.checks >= required {
                        self.result = Some(self.image(f));
                        return true;
                    }
                    continue;
                }
            }
            self.checks = 0;
            let mut numerator = y;
            let mut denominator = f.one();
            let mut valid = true;
            for (x, a) in self.nodes.iter().zip(&self.differences) {
                let den = f.sub(&numerator, &f.mul(a, &denominator));
                if f.is_zero(&den) {
                    valid = false;
                    break;
                }
                numerator = f.mul(&f.sub(&t, x), &denominator);
                denominator = den;
            }
            if !valid {
                continue;
            }
            let b = f.div(&numerator, &denominator);
            if self.nodes.is_empty() {
                self.p = vec![b];
                field::trim(f, &mut self.p);
            } else {
                let c = *self.nodes.last().unwrap();
                let next_p = field::thiele_step(f, &self.p, b, c, &self.p_prev);
                let next_q = field::thiele_step(f, &self.q, b, c, &self.q_prev);
                self.p_prev = std::mem::replace(&mut self.p, next_p);
                self.q_prev = std::mem::replace(&mut self.q, next_q);
            }
            self.nodes.push(t);
            self.differences.push(b);
        }
        false
    }

    fn image(&self, f: &Zp64) -> Image {
        let nodes = self.nodes.len();
        if self.p.is_empty() {
            return Image {
                numerator: vec![],
                denominator: vec![f.one()],
                nodes,
            };
        }
        let g = field::gcd(f, &self.p, &self.q);
        let (mut numerator, _) = field::quot_rem(f, &self.p, &g);
        let (mut denominator, _) = field::quot_rem(f, &self.q, &g);
        let lead = f.inv(denominator.last().expect("nonzero Thiele denominator"));
        for c in numerator.iter_mut().chain(denominator.iter_mut()) {
            *c = f.mul(&*c, &lead);
        }
        field::trim(f, &mut numerator);
        Image {
            numerator,
            denominator,
            nodes,
        }
    }
}

/// CRT accumulation of one terminal's normalized coefficients.
struct Lift {
    degrees: Option<Option<(usize, usize)>>,
    residues: Vec<Integer>,
    modulus: Integer,
    images: usize,
    candidate: Option<Coefficient>,
    /// Normalized (monic-denominator) rational coefficients of `candidate`.
    rationals: Option<Vec<Rational>>,
    stable: bool,
}

enum ImageOrder {
    Same,
    Lower,
    Higher,
    Incomparable,
}

fn compare(image: Option<(usize, usize)>, reference: Option<(usize, usize)>) -> ImageOrder {
    match (image, reference) {
        (None, None) => ImageOrder::Same,
        (None, Some(_)) => ImageOrder::Lower,
        (Some(_), None) => ImageOrder::Higher,
        (Some((a, b)), Some((c, d))) => {
            if (a, b) == (c, d) {
                ImageOrder::Same
            } else if a <= c && b <= d {
                ImageOrder::Lower
            } else if a >= c && b >= d {
                ImageOrder::Higher
            } else {
                ImageOrder::Incomparable
            }
        }
    }
}

impl Lift {
    fn new() -> Self {
        Self {
            degrees: None,
            residues: vec![],
            modulus: Integer::one(),
            images: 0,
            candidate: None,
            rationals: None,
            stable: false,
        }
    }

    /// Returns false when the image was rejected as unlucky.
    fn add(&mut self, f: &Zp64, image: &Image, context: &CoefficientContext) -> bool {
        let degrees = image.degrees();
        let reset = match self.degrees {
            None => true,
            Some(reference) => match compare(degrees, reference) {
                ImageOrder::Same => false,
                ImageOrder::Higher => true,
                ImageOrder::Lower | ImageOrder::Incomparable => return false,
            },
        };
        let values: Vec<Integer> = image
            .numerator
            .iter()
            .chain(
                image
                    .denominator
                    .iter()
                    .take(image.denominator.len().saturating_sub(1)),
            )
            .map(|c| field::integer_from_fe(f, c))
            .collect();
        let prime = Integer::from(f.get_prime());
        // A candidate built from earlier primes that also matches this image
        // is confirmed by a prime it was not built from (no new lifting).
        let confirmed = !reset
            && self.candidate.is_some()
            && self.rationals.as_ref().is_some_and(|rationals| {
                rationals.iter().zip(&values).all(|(r, v)| {
                    let n = field::reduce(f, r.numerator_ref());
                    let d = field::reduce(f, r.denominator_ref());
                    let v = field::reduce(f, v);
                    !f.is_zero(&d) && n == f.mul(&v, &d)
                })
            });
        if reset {
            self.degrees = Some(degrees);
            self.residues = values;
            self.modulus = prime;
            self.images = 1;
            self.candidate = None;
            self.rationals = None;
            self.stable = false;
        } else {
            for (residue, value) in self.residues.iter_mut().zip(values) {
                *residue = Integer::chinese_remainder(
                    value,
                    std::mem::replace(residue, Integer::zero()),
                    prime.clone(),
                    self.modulus.clone(),
                );
            }
            self.modulus = &self.modulus * &prime;
            self.images += 1;
        }
        if confirmed {
            self.stable = true;
            return true;
        }
        let rationals = self.rational_coefficients();
        let candidate = rationals
            .as_ref()
            .and_then(|r| self.rational_candidate(context, r));
        self.stable = candidate.is_some() && candidate == self.candidate && self.images >= 2;
        self.candidate = candidate;
        self.rationals = rationals;
        true
    }

    fn rational_coefficients(&self) -> Option<Vec<Rational>> {
        self.residues
            .iter()
            .map(|residue| {
                Rational::maximal_quotient_reconstruction(residue, &self.modulus, None).ok()
            })
            .collect()
    }

    fn rational_candidate(
        &self,
        context: &CoefficientContext,
        rationals: &[Rational],
    ) -> Option<Coefficient> {
        let degrees = self.degrees?;
        let Some((numerator_degree, denominator_degree)) = degrees else {
            return Some(context.zero());
        };
        let mut lcm = Integer::one();
        for r in rationals {
            lcm = lcm.lcm(r.denominator_ref());
        }
        let scale = |r: &Rational| -> Integer {
            let (quotient, _) = lcm.quot_rem(r.denominator_ref());
            r.numerator_ref() * &quotient
        };
        let template = &context.template().numerator;
        let mut numerator = template.zero();
        for (degree, r) in rationals[..=numerator_degree].iter().enumerate() {
            if !r.is_zero() {
                numerator.append_monomial(scale(r), &exponent(context, degree)?);
            }
        }
        let mut denominator = template.zero();
        for (degree, r) in rationals[numerator_degree + 1..].iter().enumerate() {
            if !r.is_zero() {
                denominator.append_monomial(scale(r), &exponent(context, degree)?);
            }
        }
        denominator.append_monomial(lcm.clone(), &exponent(context, denominator_degree)?);
        Some(Coefficient::from_num_den(numerator, denominator, &Z, true))
    }
}

fn exponent(context: &CoefficientContext, degree: usize) -> Option<Vec<u16>> {
    if context.variables().is_empty() {
        (degree == 0).then(Vec::new)
    } else {
        u16::try_from(degree).ok().map(|d| vec![d])
    }
}

fn evaluate_candidate(f: &Zp64, candidate: &Coefficient, x: Fe) -> Option<Fe> {
    let dense = super::field::DenseRational::from_coefficient(candidate);
    let reduce = |v: &Vec<Integer>| -> Vec<Fe> { v.iter().map(|c| field::reduce(f, c)).collect() };
    let den = field::horner(f, &reduce(&dense.denominator), x);
    if f.is_zero(&den) {
        return None;
    }
    Some(f.div(&field::horner(f, &reduce(&dense.numerator), x), &den))
}

/// Values of all output terminals at the good points of one prime.
struct PrimeSamples {
    field: Zp64,
    xs: Vec<Fe>,
    ys: Vec<Vec<Fe>>,
    used: HashSet<u64>,
    rng: StdRng,
    batches: usize,
}

impl PrimeSamples {
    fn new(prime: u64, seed: u64, terminals: usize) -> Self {
        Self {
            field: Zp64::new(prime),
            xs: vec![],
            ys: vec![vec![]; terminals],
            used: HashSet::new(),
            rng: StdRng::seed_from_u64(seed ^ prime.rotate_left(17)),
            batches: 0,
        }
    }

    fn fresh_points(&mut self, count: usize) -> Vec<Fe> {
        let p = self.field.get_prime();
        let mut points = Vec::with_capacity(count);
        while points.len() < count {
            let value = self.rng.random_range(2..p);
            if self.used.insert(value) {
                points.push(self.field.to_element(value));
            }
        }
        points
    }

    fn absorb(
        &mut self,
        points: &[Fe],
        batch: &ReplayBatch,
        slots: &[usize],
        stats: &mut ModularStatistics,
    ) {
        let width = points.len();
        self.batches += 1;
        for (j, x) in points.iter().enumerate() {
            if !batch.good[j] {
                stats.bad_points += 1;
                continue;
            }
            self.xs.push(*x);
            for (ys, &slot) in self.ys.iter_mut().zip(slots) {
                ys.push(batch.values[slot * width + j]);
            }
        }
    }
}

struct Driver<'a> {
    trace: &'a Trace,
    slots: &'a [usize],
    options: &'a ModularWeightOptions,
    pool: rayon::ThreadPool,
    primes: PrimeIteratorU64,
}

impl Driver<'_> {
    fn next_prime(&mut self) -> Result<u64, ReductionError> {
        self.primes
            .next()
            .ok_or_else(|| error("exhausted 64-bit primes"))
    }

    /// Replay batches in parallel. `None` marks a prime dividing a constant.
    fn run(
        &self,
        jobs: Vec<(usize, Zp64, Vec<Fe>)>,
        stats: &mut ModularStatistics,
    ) -> Result<Vec<(usize, Vec<Fe>, Option<ReplayBatch>)>, ReductionError> {
        let started = Instant::now();
        let trace = self.trace;
        let results: Vec<_> = self.pool.install(|| {
            jobs.into_par_iter()
                .map(|(owner, f, points)| {
                    let begun = Instant::now();
                    let result = replay(trace, &f, &points);
                    (owner, points, result, begun.elapsed().as_secs_f64())
                })
                .collect()
        });
        stats.replay_seconds += started.elapsed().as_secs_f64();
        let mut output = Vec::with_capacity(results.len());
        for (owner, points, result, cpu) in results {
            stats.replay_cpu_seconds += cpu;
            stats.batches += 1;
            match result {
                Ok(batch) => {
                    stats.cancel_checks += batch.cancel_checks;
                    output.push((owner, points, Some(batch)));
                }
                Err(ReplayError::BadPrime) => output.push((owner, points, None)),
                Err(ReplayError::ZeroDecision) => {
                    return Err(error(
                        "a key erased as exactly zero in the recording pass is nonzero in a replay",
                    ));
                }
            }
        }
        Ok(output)
    }

    /// Fill `samples` with `count` more points (in `batch_points` batches).
    fn extend(
        &mut self,
        samples: &mut [PrimeSamples],
        requests: &[(usize, usize, usize)],
        stats: &mut ModularStatistics,
    ) -> Result<Vec<bool>, ReductionError> {
        let mut jobs = vec![];
        for &(owner, count, width) in requests {
            for _ in 0..count.div_ceil(width) {
                let points = samples[owner].fresh_points(width);
                jobs.push((owner, samples[owner].field.clone(), points));
            }
        }
        let mut bad = vec![false; samples.len()];
        for (owner, points, batch) in self.run(jobs, stats)? {
            match batch {
                Some(batch) => samples[owner].absorb(&points, &batch, self.slots, stats),
                None => bad[owner] = true,
            }
        }
        Ok(bad)
    }
}

/// Reconstruct the terminal accumulators `slots` of `trace` over Q.
pub(super) fn reconstruct(
    trace: &Trace,
    slots: &[usize],
    context: &CoefficientContext,
    options: &ModularWeightOptions,
    stats: &mut ModularStatistics,
) -> Result<Vec<Coefficient>, ReductionError> {
    let terminals = slots.len();
    let has_cancellations = trace
        .ops
        .iter()
        .any(|op| matches!(op.tag(), OpTag::Cancel | OpTag::TermCancel));
    if terminals == 0 && !has_cancellations {
        return Ok(vec![]);
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(options.threads)
        .build()
        .map_err(|e| error(format!("replay thread pool: {e}")))?;
    let mut rng = StdRng::seed_from_u64(options.seed ^ 0x6c69_6674);
    let start = (1u64 << 62) + (rng.random::<u64>() >> 3);
    let mut driver = Driver {
        trace,
        slots,
        options,
        pool,
        primes: PrimeIteratorU64::new(start),
    };
    if terminals == 0 {
        // Everything cancelled: certify the recorded zero erasures at a
        // fresh prime (each replay checks every erased key at every point).
        for _ in 0..options.verification_primes {
            let prime = driver.next_prime()?;
            let mut samples = vec![PrimeSamples::new(prime, options.seed, 0)];
            driver.extend(
                &mut samples,
                &[(0, options.batch_points, options.batch_points)],
                stats,
            )?;
        }
        return Ok(vec![]);
    }
    let checks = options.verification_points;
    let seed = options.seed;
    let threads = options.threads;
    let width = options.batch_points;

    // First prime: adaptive number of points until every terminal converges.
    let first = loop {
        let prime = driver.next_prime()?;
        let mut samples = vec![PrimeSamples::new(prime, seed, terminals)];
        let mut thiele: Vec<Thiele> = (0..terminals)
            .map(|_| Thiele::new(&samples[0].field))
            .collect();
        let mut bad_prime = false;
        loop {
            if driver.extend(&mut samples, &[(0, threads * width, width)], stats)?[0] {
                bad_prime = true;
                break;
            }
            let interpolation = Instant::now();
            let sample = &samples[0];
            let done = driver.pool.install(|| {
                thiele
                    .par_iter_mut()
                    .enumerate()
                    .map(|(t, state)| state.feed(&sample.field, &sample.xs, &sample.ys[t], checks))
                    .collect::<Vec<_>>()
            });
            stats.interpolation_seconds += interpolation.elapsed().as_secs_f64();
            if done.iter().all(|d| *d) {
                break;
            }
            if sample.xs.len() >= options.max_points_per_prime {
                return Err(error(format!(
                    "modular reconstruction needs more than {} points per prime",
                    options.max_points_per_prime
                )));
            }
        }
        if bad_prime {
            stats.unlucky_images += 1;
            continue;
        }
        let samples = samples.pop().unwrap();
        stats.points_first_prime = samples.xs.len();
        break (samples, thiele);
    };
    let (first_samples, first_thiele) = first;
    let images: Vec<Image> = first_thiele
        .into_iter()
        .map(|t| t.result.expect("converged"))
        .collect();
    let needed = images.iter().map(|i| i.nodes).max().unwrap_or(1) + checks;
    stats.points_per_prime = needed;
    let mut lifts: Vec<Lift> = (0..terminals).map(|_| Lift::new()).collect();
    let lift_started = Instant::now();
    for (lift, image) in lifts.iter_mut().zip(&images) {
        if !lift.add(&first_samples.field, image, context) {
            stats.unlucky_images += 1;
        }
    }
    stats.lift_seconds += lift_started.elapsed().as_secs_f64();
    stats.primes = 1;
    // Later primes: fill all workers with about `batch_points` per batch.
    let primes_per_round = ((threads * width) as f64 / needed as f64)
        .round()
        .clamp(1.0, threads as f64) as usize;
    let batches_per_prime = (threads / primes_per_round).max(1);
    let prime_width = needed.div_ceil(batches_per_prime).max(1);
    loop {
        if lifts.iter().all(|l| l.stable) {
            // Fresh primes and fresh points, never used for the lift.
            let mut failures = 0;
            for _ in 0..options.verification_primes {
                let prime = driver.next_prime()?;
                let mut samples = vec![PrimeSamples::new(prime, seed ^ 0x7665_7269, terminals)];
                if driver.extend(&mut samples, &[(0, width, width)], stats)?[0] {
                    stats.unlucky_images += 1;
                    continue;
                }
                let sample = &samples[0];
                let f = &sample.field;
                let failed: Vec<usize> = driver.pool.install(|| {
                    lifts
                        .par_iter()
                        .enumerate()
                        .filter_map(|(t, lift)| {
                            let candidate = lift.candidate.as_ref().expect("stable candidate");
                            sample
                                .xs
                                .iter()
                                .zip(&sample.ys[t])
                                .any(|(x, y)| {
                                    evaluate_candidate(f, candidate, *x).is_some_and(|v| v != *y)
                                })
                                .then_some(t)
                        })
                        .collect()
                });
                for t in &failed {
                    lifts[*t].stable = false;
                }
                failures += failed.len();
            }
            if failures == 0 {
                break;
            }
            stats.verification_failures += failures;
        }
        if stats.primes >= options.max_primes {
            return Err(error(format!(
                "modular reconstruction did not stabilize within {} primes",
                options.max_primes
            )));
        }
        let mut samples = vec![];
        for _ in 0..primes_per_round {
            samples.push(PrimeSamples::new(driver.next_prime()?, seed, terminals));
        }
        let requests: Vec<_> = (0..samples.len())
            .map(|i| (i, needed, prime_width))
            .collect();
        let bad = driver.extend(&mut samples, &requests, stats)?;
        for (index, sample) in samples.iter_mut().enumerate() {
            if bad[index] {
                stats.unlucky_images += 1;
                continue;
            }
            // Interpolate; top up the rare prime whose samples did not suffice.
            let mut thiele: Vec<Thiele> =
                (0..terminals).map(|_| Thiele::new(&sample.field)).collect();
            loop {
                let interpolation = Instant::now();
                let current = &*sample;
                let done = driver.pool.install(|| {
                    thiele
                        .par_iter_mut()
                        .enumerate()
                        .map(|(t, state)| {
                            state.feed(&current.field, &current.xs, &current.ys[t], checks)
                        })
                        .collect::<Vec<_>>()
                });
                stats.interpolation_seconds += interpolation.elapsed().as_secs_f64();
                if done.iter().all(|d| *d) {
                    break;
                }
                if sample.xs.len() >= options.max_points_per_prime {
                    return Err(error(
                        "modular reconstruction exceeded the per-prime point budget",
                    ));
                }
                let mut single = vec![std::mem::replace(sample, PrimeSamples::new(3, 0, 0))];
                driver.extend(&mut single, &[(0, prime_width, prime_width)], stats)?;
                *sample = single.pop().unwrap();
            }
            let lift_started = Instant::now();
            let field = sample.field.clone();
            let rejected: usize = driver.pool.install(|| {
                lifts
                    .par_iter_mut()
                    .zip(thiele.into_par_iter())
                    .map(|(lift, state)| {
                        let image = state.result.expect("converged");
                        usize::from(!lift.add(&field, &image, context))
                    })
                    .sum()
            });
            stats.unlucky_images += rejected;
            stats.lift_seconds += lift_started.elapsed().as_secs_f64();
            stats.primes += 1;
        }
    }
    let mut results = Vec::with_capacity(terminals);
    for lift in lifts {
        let candidate = lift.candidate.expect("verified candidate");
        if let Some((n, d)) = lift.degrees.flatten() {
            stats.max_numerator_degree = stats.max_numerator_degree.max(n);
            stats.max_denominator_degree = stats.max_denominator_degree.max(d);
        }
        for c in candidate
            .numerator
            .coefficients
            .iter()
            .chain(candidate.denominator.coefficients.iter())
        {
            stats.max_coefficient_bits = stats
                .max_coefficient_bits
                .max(c.abs().significant_bits() as usize);
        }
        results.push(candidate);
    }
    Ok(results)
}
