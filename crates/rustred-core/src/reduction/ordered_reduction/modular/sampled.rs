//! Image source of `ModularSampled`: numeric replays of the recorded trace at
//! sample points, and per-prime Thiele interpolation of every terminal.
//!
//! The terminal key set and the guards are fixed by the single recording
//! pass, so the structure of every image is `()` and only the degrees vary
//! between primes. Sampling is sized by need:
//!
//! - the first usable prime discovers the degrees adaptively: half of
//!   `batch_points` points, then doubling the total until every terminal's
//!   interpolation is confirmed by `verification_points` consecutive fresh
//!   points;
//! - every later prime draws exactly the points needed (the most Thiele
//!   nodes of any terminal plus the confirming points), topped up only when
//!   an interpolation did not converge (poles, or a larger degree);
//! - points are split into replay batches of at most `batch_points` (and at
//!   least an eighth of that) so that the batches of a round fill whole waves
//!   of the threads; several primes share a round only when one prime's
//!   points cannot occupy the threads;
//! - a verification prime replays `verification_points` fresh points and
//!   evaluates every candidate there.
//!
//! The points of a prime come from a generator seeded by the prime, and their
//! order of use never depends on the batching, so images and statistics
//! (except replay timings and speculative images) are independent of the
//! thread count.
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::{Field, Ring, RingOps};

use super::executor::Executor;
use super::field::{self, Fe, Poly};
use super::lift::{Candidate, ImageSource, PrimeImages, Verification, image_from};
use super::replay::{ReplayBatch, ReplayError, replay};
use super::{ModularStatistics, Trace, error};
use crate::reduction::ReductionError;
use crate::reduction::ordered_reduction::arithmetic::SampledOptions;

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
    result: Option<(Poly, Poly)>,
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
                        self.result = Some(self.reduced(f));
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

    /// The converged convergent as coprime numerator and monic denominator.
    fn reduced(&self, f: &Zp64) -> (Poly, Poly) {
        if self.p.is_empty() {
            return (vec![], vec![f.one()]);
        }
        let g = field::gcd(f, &self.p, &self.q);
        let (mut numerator, _) = field::quot_rem(f, &self.p, &g);
        let (mut denominator, _) = field::quot_rem(f, &self.q, &g);
        let lead = f.inv(denominator.last().expect("nonzero Thiele denominator"));
        for c in numerator.iter_mut().chain(denominator.iter_mut()) {
            *c = f.mul(&*c, &lead);
        }
        field::trim(f, &mut numerator);
        (numerator, denominator)
    }
}

/// Values of all output terminals at the good points of one prime.
struct PrimeSamples {
    field: Zp64,
    xs: Vec<Fe>,
    ys: Vec<Vec<Fe>>,
    used: HashSet<u64>,
    rng: StdRng,
    /// Points drawn, including bad ones.
    drawn: usize,
}

impl PrimeSamples {
    fn new(prime: u64, seed: u64, terminals: usize) -> Self {
        Self {
            field: Zp64::new(prime),
            xs: vec![],
            ys: vec![vec![]; terminals],
            used: HashSet::new(),
            rng: StdRng::seed_from_u64(seed ^ prime.rotate_left(17)),
            drawn: 0,
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
        self.drawn += count;
        points
    }

    fn absorb(
        &mut self,
        points: &[Fe],
        batch: &ReplayBatch,
        slots: &[usize],
        statistics: &mut ModularStatistics,
    ) {
        let width = points.len();
        for (j, x) in points.iter().enumerate() {
            if !batch.good[j] {
                statistics.bad_points += 1;
                continue;
            }
            self.xs.push(*x);
            for (ys, &slot) in self.ys.iter_mut().zip(slots) {
                ys.push(batch.values[slot * width + j]);
            }
        }
    }
}

/// One replay batch: points of the prime `owner` of the round.
struct Job {
    owner: usize,
    field: Zp64,
    points: Vec<Fe>,
}

/// Narrowest batch worth replaying: per-operation overheads (one frontier
/// row per trace operation) dominate below an eighth of `batch_points`.
fn min_width(options: &SampledOptions) -> usize {
    (options.batch_points / 8).max(1)
}

/// Batches for `points` points of one prime when `primes` primes share a
/// round: at most `batch_points` and at least `min_width` points each, filling
/// whole waves of the threads when possible.
fn batches(options: &SampledOptions, points: usize, primes: usize, threads: usize) -> usize {
    let at_least = points.div_ceil(options.batch_points).max(1);
    let at_most = (points / min_width(options)).max(at_least);
    let waves = (at_least * primes).div_ceil(threads);
    (waves * threads).div_ceil(primes).clamp(at_least, at_most)
}

/// Seed salt of the verification points.
const VERIFICATION_POINTS_SALT: u64 = 0x7665_7269_6670;

pub(super) struct SampledSource<'a> {
    trace: &'a Trace,
    /// Terminal accumulator slot of every output terminal.
    slots: &'a [usize],
    structure: Arc<()>,
    options: SampledOptions,
    seed: u64,
    /// Points per prime once the first image fixed the degrees.
    needed: Option<usize>,
}

impl<'a> SampledSource<'a> {
    pub fn new(trace: &'a Trace, slots: &'a [usize], options: SampledOptions, seed: u64) -> Self {
        Self {
            trace,
            slots,
            structure: Arc::new(()),
            options,
            seed,
            needed: None,
        }
    }

    /// The structure of every image: fixed by the recording.
    pub fn structure(&self) -> &Arc<()> {
        &self.structure
    }

    fn checks(&self) -> usize {
        self.options.verification_points
    }

    fn min_width(&self) -> usize {
        min_width(&self.options)
    }

    fn points_budget(&self, samples: &PrimeSamples, count: usize) -> Result<(), ReductionError> {
        if samples.drawn + count > self.options.max_points_per_prime {
            return Err(error(format!(
                "modular reconstruction needs more than {} points per prime",
                self.options.max_points_per_prime
            )));
        }
        Ok(())
    }

    /// Replay `jobs`; `None` marks a prime dividing a constant denominator.
    fn replay(
        &self,
        jobs: Vec<Job>,
        executor: &Executor,
        statistics: &mut ModularStatistics,
    ) -> Result<Vec<(usize, Vec<Fe>, Option<ReplayBatch>)>, ReductionError> {
        let started = Instant::now();
        let trace = self.trace;
        let results = executor.map(jobs, |job| {
            let begun = Instant::now();
            let result = replay(trace, &job.field, &job.points);
            (job.owner, job.points, result, begun.elapsed().as_secs_f64())
        });
        statistics.replay_seconds += started.elapsed().as_secs_f64();
        let mut output = Vec::with_capacity(results.len());
        for (owner, points, result, cpu) in results {
            statistics.replay_cpu_seconds += cpu;
            statistics.batches += 1;
            statistics.points_replayed += points.len();
            match result {
                Ok(batch) => {
                    statistics.cancel_checks += batch.cancel_checks;
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

    /// Sample every prime of `requests` (`(prime index, points)`), feed the
    /// interpolations and return the primes whose interpolation is not done.
    fn sample_round(
        &self,
        samples: &mut [PrimeSamples],
        states: &mut [Thiele],
        usable: &mut [bool],
        requests: &[(usize, usize)],
        executor: &Executor,
        statistics: &mut ModularStatistics,
    ) -> Result<Vec<usize>, ReductionError> {
        let terminals = self.slots.len();
        let mut jobs = vec![];
        for &(owner, count) in requests {
            self.points_budget(&samples[owner], count)?;
            let mut points = samples[owner].fresh_points(count);
            let batches = batches(&self.options, count, requests.len(), executor.threads());
            let width = count.div_ceil(batches);
            while !points.is_empty() {
                let rest = points.split_off(width.min(points.len()));
                jobs.push(Job {
                    owner,
                    field: samples[owner].field.clone(),
                    points: std::mem::replace(&mut points, rest),
                });
            }
        }
        for (owner, points, batch) in self.replay(jobs, executor, statistics)? {
            match batch {
                Some(batch) if usable[owner] => {
                    samples[owner].absorb(&points, &batch, self.slots, statistics)
                }
                Some(_) => {}
                None => usable[owner] = false,
            }
        }
        let active: Vec<bool> = (0..samples.len())
            .map(|i| usable[i] && requests.iter().any(|(owner, _)| *owner == i))
            .collect();
        let checks = self.checks();
        let interpolation = Instant::now();
        let samples = &*samples;
        let done = executor.map_mut(states, |index, state| {
            let (owner, t) = (index / terminals, index % terminals);
            if !active[owner] {
                return true;
            }
            let sample = &samples[owner];
            state.feed(&sample.field, &sample.xs, &sample.ys[t], checks)
        });
        statistics.interpolation_seconds += interpolation.elapsed().as_secs_f64();
        Ok((0..samples.len())
            .filter(|&owner| {
                active[owner] && done[owner * terminals..(owner + 1) * terminals].contains(&false)
            })
            .collect())
    }

    fn images_of(
        &self,
        samples: &[PrimeSamples],
        states: Vec<Thiele>,
        usable: &[bool],
    ) -> Vec<Option<PrimeImages<()>>> {
        let terminals = self.slots.len();
        let mut states = states.into_iter();
        samples
            .iter()
            .zip(usable)
            .map(|(sample, usable)| {
                let images: Vec<_> = states
                    .by_ref()
                    .take(terminals)
                    .map(|state| {
                        let (numerator, denominator) = state.result.unwrap_or_default();
                        image_from(&sample.field, &numerator, &denominator)
                    })
                    .collect();
                usable.then(|| PrimeImages {
                    structure: self.structure.clone(),
                    terminals: images,
                })
            })
            .collect()
    }

    /// Adaptive degree discovery on one prime.
    fn discover(
        &mut self,
        prime: u64,
        executor: &Executor,
        statistics: &mut ModularStatistics,
    ) -> Result<Option<PrimeImages<()>>, ReductionError> {
        let terminals = self.slots.len();
        let mut samples = [PrimeSamples::new(prime, self.seed, terminals)];
        let mut states: Vec<Thiele> = (0..terminals)
            .map(|_| Thiele::new(&samples[0].field))
            .collect();
        let mut usable = [true];
        // Without terminals the replays only certify the recorded erasures.
        let start = if terminals == 0 {
            self.checks()
        } else {
            self.options.batch_points / 2
        };
        let mut count = start.min(self.options.max_points_per_prime);
        loop {
            let open = self.sample_round(
                &mut samples,
                &mut states,
                &mut usable,
                &[(0, count)],
                executor,
                statistics,
            )?;
            if !usable[0] {
                return Ok(None);
            }
            if open.is_empty() {
                break;
            }
            // Double the points drawn so far, within the budget.
            count = samples[0]
                .drawn
                .min(self.options.max_points_per_prime - samples[0].drawn)
                .max(1);
        }
        let nodes = states
            .iter()
            .map(|state| state.nodes.len())
            .max()
            .unwrap_or(0);
        self.needed = Some((nodes + self.checks()).max(1));
        statistics.points_first_prime = samples[0].drawn;
        statistics.points_per_prime = self.needed.unwrap_or(0);
        Ok(self
            .images_of(&samples, states, &usable)
            .pop()
            .expect("one prime"))
    }

    /// Exactly the needed points for each prime, topped up when necessary.
    fn sample_known(
        &mut self,
        primes: &[u64],
        executor: &Executor,
        statistics: &mut ModularStatistics,
    ) -> Result<Vec<Option<PrimeImages<()>>>, ReductionError> {
        let terminals = self.slots.len();
        let needed = self.needed.expect("degrees discovered");
        let mut samples: Vec<PrimeSamples> = primes
            .iter()
            .map(|prime| PrimeSamples::new(*prime, self.seed, terminals))
            .collect();
        let mut states: Vec<Thiele> = samples
            .iter()
            .flat_map(|sample| (0..terminals).map(|_| Thiele::new(&sample.field)))
            .collect();
        let mut usable = vec![true; primes.len()];
        let mut requests: Vec<(usize, usize)> = (0..primes.len()).map(|i| (i, needed)).collect();
        let top_up = (needed / 8).max(self.checks() + 2);
        while !requests.is_empty() {
            let open = self.sample_round(
                &mut samples,
                &mut states,
                &mut usable,
                &requests,
                executor,
                statistics,
            )?;
            requests = open.into_iter().map(|owner| (owner, top_up)).collect();
        }
        // A prime needing more nodes than the first one sets the new need.
        let nodes = states
            .iter()
            .map(|state| state.nodes.len())
            .max()
            .unwrap_or(0);
        if nodes + self.checks() > needed {
            self.needed = Some(nodes + self.checks());
            statistics.points_per_prime = statistics.points_per_prime.max(nodes + self.checks());
        }
        Ok(self.images_of(&samples, states, &usable))
    }
}

impl ImageSource for SampledSource<'_> {
    type Structure = ();
    type Error = ReductionError;

    fn images(
        &mut self,
        primes: &[u64],
        executor: &Executor,
        statistics: &mut ModularStatistics,
    ) -> Result<Vec<Option<PrimeImages<()>>>, ReductionError> {
        let mut images = Vec::with_capacity(primes.len());
        let mut rest = primes;
        while self.needed.is_none() {
            let Some((first, tail)) = rest.split_first() else {
                break;
            };
            images.push(self.discover(*first, executor, statistics)?);
            rest = tail;
        }
        if !rest.is_empty() {
            images.extend(self.sample_known(rest, executor, statistics)?);
        }
        Ok(images)
    }

    /// One prime per round while its points can occupy every thread.
    fn round_capacity(&self, threads: usize) -> usize {
        match self.needed {
            None => 1,
            Some(needed) => threads.div_ceil((needed / self.min_width()).max(1)),
        }
    }

    fn verify(
        &mut self,
        primes: &[u64],
        _: &(),
        candidates: &[&Candidate],
        executor: &Executor,
        statistics: &mut ModularStatistics,
    ) -> Result<Vec<Verification<()>>, ReductionError> {
        let count = self.checks();
        let jobs: Vec<Job> = primes
            .iter()
            .enumerate()
            .map(|(owner, &prime)| {
                let field = Zp64::new(prime);
                let mut rng = StdRng::seed_from_u64(
                    self.seed ^ VERIFICATION_POINTS_SALT ^ prime.rotate_left(29),
                );
                let mut used = HashSet::new();
                let mut points = Vec::with_capacity(count);
                while points.len() < count {
                    let value = rng.random_range(2..prime);
                    if used.insert(value) {
                        points.push(field.to_element(value));
                    }
                }
                Job {
                    owner,
                    field,
                    points,
                }
            })
            .collect();
        let slots = self.slots;
        let mut outcomes = vec![];
        for (owner, points, batch) in self.replay(jobs, executor, statistics)? {
            let Some(batch) = batch else {
                outcomes.push(Verification::Unusable);
                continue;
            };
            let field = Zp64::new(primes[owner]);
            let good: Vec<usize> = (0..points.len()).filter(|&j| batch.good[j]).collect();
            statistics.bad_points += points.len() - good.len();
            // Too few finite points, or the prime divides a lifted
            // denominator: draw another prime.
            let width = points.len();
            let checked =
                executor.map(candidates.iter().enumerate().collect(), |(t, candidate)| {
                    let (numerator, denominator) = candidate.reduce(&field)?;
                    Some(good.iter().all(|&j| {
                        let x = points[j];
                        let den = field::horner(&field, &denominator, x);
                        !field.is_zero(&den)
                            && field.mul(&batch.values[slots[t] * width + j], &den)
                                == field::horner(&field, &numerator, x)
                    }))
                });
            if good.len() < count || checked.iter().any(Option::is_none) {
                outcomes.push(Verification::Unusable);
                continue;
            }
            outcomes.push(Verification::Checked {
                disagreeing: checked
                    .iter()
                    .enumerate()
                    .filter(|(_, agrees)| **agrees == Some(false))
                    .map(|(t, _)| t)
                    .collect(),
            });
        }
        Ok(outcomes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batches_fill_whole_waves_within_the_width_bounds() {
        let options = SampledOptions::default();
        // 5L zero momentum: 906 points per prime, one wave when possible.
        assert_eq!(batches(&options, 906, 1, 1), 8);
        assert_eq!(batches(&options, 906, 1, 4), 8);
        assert_eq!(batches(&options, 906, 1, 10), 10);
        assert_eq!(batches(&options, 906, 1, 12), 12);
        assert_eq!(batches(&options, 906, 1, 16), 16);
        assert_eq!(batches(&options, 906, 2, 64), 32);
        // Small requests are never split below an eighth of batch_points.
        assert_eq!(batches(&options, 33, 1, 10), 2);
        assert_eq!(batches(&options, 10, 1, 10), 1);
        assert_eq!(batches(&options, 64, 3, 4), 2);
    }
}
