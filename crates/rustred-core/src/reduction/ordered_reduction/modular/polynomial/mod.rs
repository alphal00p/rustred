//! Image source of `ModularPolynomial`: replays of the recorded trace whose
//! weights are rational functions in `Z_p(d)`, carried for several primes
//! (lanes) at once.
//!
//! The recording pass is the one of `ModularSampled` (`ModularRecorder`): the
//! scheduler and the adapters' checked steps run exactly once, and their
//! weight-independent program is the trace. Each replay job evaluates that
//! program for `lanes` primes with one weight representation:
//!
//! - `factorized` (default): partial fractions over the request's factor
//!   basis of step-coefficient denominators (`fraction.rs`); sums are
//!   coefficient-wise, so merging needs no common denominator and no gcd;
//! - `dense`: one reduced numerator/denominator pair per lane, normalized by
//!   a gcd after every operation (`dense.rs`, a reference).
//!
//! A job yields, for every lane, the reduced image of every output terminal;
//! the shared driver (`lift.rs`) lifts them over Q. A prime that is bad for
//! the basis or divides a coefficient denominator (`basis.rs`) is unusable.
//! Every recorded erasure is checked to be exactly zero in every lane: a
//! nonzero lane proves the recorded zero decision wrong and fails the request.
//! An unlucky prime (a terminal whose numerator or denominator degree drops,
//! or which acquires a common factor, modulo `p`) only changes the degrees
//! of its image and is outvoted by the driver's signature majority.
//!
//! Images depend only on their prime, never on the other lanes of a job, on
//! the grouping of primes into jobs or on the thread count.
mod basis;
mod dense;
mod fraction;
mod lane;

use std::sync::Arc;
use std::time::Instant;

use symbolica::utils::resources;

use self::basis::LaneTables;
pub(super) use self::basis::{Basis, BasisBuilder};
use self::dense::Dense;
use self::fraction::Factorized;
use super::compose::{Program, compose_images};
use super::executor::Executor;
use super::lift::{Image, ImageSource, PrimeImages};
use super::{CoefRef, ModularStatistics, OpTag, Trace, error, statistics_requested};
use crate::reduction::ReductionError;
use crate::reduction::ordered_reduction::arithmetic::{PolynomialDenominators, PolynomialOptions};

/// Weight arithmetic of one replay job (all lanes at once).
pub(super) trait Representation {
    type Weight: Default;
    /// An input weight: the coefficient itself.
    fn coefficient(&mut self, c: CoefRef) -> Result<Self::Weight, ReductionError>;
    /// The popped weight becomes the factor of the following products.
    fn activate(&mut self, active: &mut Self::Weight);
    fn product(
        &mut self,
        active: &Self::Weight,
        c: CoefRef,
    ) -> Result<Self::Weight, ReductionError>;
    fn add_product(
        &mut self,
        destination: &mut Self::Weight,
        active: &Self::Weight,
        c: CoefRef,
    ) -> Result<(), ReductionError>;
    /// Zero in every lane.
    fn is_zero(&self, weight: &Self::Weight) -> bool;
    /// Resident field elements (memory census).
    fn words(&self, weight: &Self::Weight) -> usize;
    /// Reduced image of one lane.
    fn image(&mut self, weight: &Self::Weight, lane: usize) -> Image;
}

/// Result of one replay job.
struct Replayed {
    /// Per lane, the images of the output terminals.
    images: Vec<Vec<Image>>,
    peak_words: usize,
    cancel_checks: usize,
    /// Seconds per trace segment: before the first phase, then one per
    /// recorded phase start (`Trace::phase_starts`).
    segment_seconds: Vec<f64>,
    /// Per segment, the resident words of the active weight summed over the
    /// products (`New`/`Add`), per lane: the replay's operation volume (0
    /// unless statistics are requested).
    segment_volume: Vec<f64>,
}

/// Trace operations between two resource checkpoints of a replay: every
/// operation combines polynomial weights of all lanes (microseconds or more
/// each), so the checkpoint's cost is negligible.
const CHECKPOINT_OPS: usize = 64;

/// Evaluate the trace with `representation` and return the images of the
/// terminal accumulators `outputs` for each of its `lanes`.
fn replay<R: Representation>(
    trace: &Trace,
    outputs: &[usize],
    lanes: usize,
    representation: &mut R,
) -> Result<Replayed, ReductionError> {
    let mut weights: Vec<R::Weight> = (0..trace.slots).map(|_| R::Weight::default()).collect();
    let mut terminals: Vec<R::Weight> = (0..trace.terminal_slots)
        .map(|_| R::Weight::default())
        .collect();
    let mut active = R::Weight::default();
    let (mut words, mut peak_words, mut cancel_checks) = (0usize, 0usize, 0usize);
    // The memory census (peak resident words) and the per-phase operation
    // volume only feed the requested statistics; otherwise skip their
    // per-op reads of the weights.
    let census = statistics_requested();
    // Phase timing (diagnostics): one clock read per phase boundary.
    let mut segment_seconds = vec![0.0; trace.phase_starts.len() + 1];
    let mut segment_volume = vec![0usize; trace.phase_starts.len() + 1];
    let mut segment = 0usize;
    let mut next_boundary = trace
        .phase_starts
        .first()
        .map_or(usize::MAX, |start| start.1);
    let mut clock = Instant::now();
    for (index, op) in trace.ops.iter().enumerate() {
        if index % CHECKPOINT_OPS == 0 {
            resources::checkpoint()?;
        }
        while index == next_boundary {
            let now = Instant::now();
            segment_seconds[segment] += now.duration_since(clock).as_secs_f64();
            clock = now;
            segment += 1;
            next_boundary = trace
                .phase_starts
                .get(segment)
                .map_or(usize::MAX, |start| start.1);
        }
        let tag = op.tag();
        let slot = op.slot();
        let target = match tag {
            OpTag::Init | OpTag::Pop | OpTag::New | OpTag::Add | OpTag::Cancel => {
                &mut weights[slot]
            }
            OpTag::TermNew | OpTag::TermAdd | OpTag::TermCancel => &mut terminals[slot],
        };
        let before = if census {
            representation.words(target)
        } else {
            0
        };
        match tag {
            OpTag::Pop => {
                active = std::mem::take(target);
                representation.activate(&mut active);
            }
            OpTag::Cancel | OpTag::TermCancel => {
                cancel_checks += 1;
                if !representation.is_zero(target) {
                    return Err(error(
                        "a key erased as exactly zero in the recording pass is nonzero \
                         in a polynomial replay",
                    ));
                }
                *target = R::Weight::default();
            }
            OpTag::Init => *target = representation.coefficient(op.coefficient)?,
            OpTag::New | OpTag::TermNew => {
                if census {
                    segment_volume[segment] += representation.words(&active);
                }
                *target = representation.product(&active, op.coefficient)?
            }
            OpTag::Add | OpTag::TermAdd => {
                if census {
                    segment_volume[segment] += representation.words(&active);
                }
                representation.add_product(target, &active, op.coefficient)?
            }
        }
        if census {
            words = words - before + representation.words(target);
            peak_words = peak_words.max(words);
        }
    }
    segment_seconds[segment] += clock.elapsed().as_secs_f64();
    let images = (0..lanes)
        .map(|lane| {
            outputs
                .iter()
                .map(|&slot| representation.image(&terminals[slot], lane))
                .collect()
        })
        .collect();
    Ok(Replayed {
        images,
        peak_words,
        cancel_checks,
        segment_seconds,
        segment_volume: segment_volume
            .into_iter()
            .map(|words| words as f64 / lanes.max(1) as f64)
            .collect(),
    })
}

/// Split `primes` usable primes into jobs of at most `lanes` primes, at
/// least one job per thread while primes remain: contiguous job sizes.
fn job_sizes(primes: usize, threads: usize, lanes: usize) -> Vec<usize> {
    if primes == 0 {
        return vec![];
    }
    let jobs = primes
        .min(threads.max(1))
        .max(primes.div_ceil(lanes.max(1)));
    let (base, extra) = (primes / jobs, primes % jobs);
    (0..jobs)
        .map(|job| base + usize::from(job < extra))
        .collect()
}

pub(super) struct PolynomialSource<'a> {
    trace: &'a Trace,
    /// Terminal accumulator slot of every output terminal.
    outputs: &'a [usize],
    basis: &'a Basis,
    options: PolynomialOptions,
    structure: Arc<()>,
    /// Terminal composition applied to every prime's terminal images in
    /// `Z_p(d)` (inside the replay job) before the lift.
    compose: Option<&'a Program>,
    seed: u64,
}

impl<'a> PolynomialSource<'a> {
    pub fn new(
        trace: &'a Trace,
        outputs: &'a [usize],
        basis: &'a Basis,
        options: PolynomialOptions,
        compose: Option<&'a Program>,
        seed: u64,
    ) -> Self {
        Self {
            trace,
            outputs,
            basis,
            options,
            structure: Arc::new(()),
            compose,
            seed,
        }
    }

    /// The structure of every image: fixed by the recording.
    pub fn structure(&self) -> &Arc<()> {
        &self.structure
    }
}

impl ImageSource for PolynomialSource<'_> {
    type Structure = ();
    type Error = ReductionError;

    fn images(
        &mut self,
        primes: &[u64],
        executor: &Executor,
        statistics: &mut ModularStatistics,
    ) -> Result<Vec<Option<PrimeImages<()>>>, ReductionError> {
        let (trace, basis, outputs, options) = (self.trace, self.basis, self.outputs, self.options);
        let prepared = Instant::now();
        let tables = executor.map(primes.to_vec(), |prime| {
            LaneTables::new(prime, trace, basis)
        })?;
        statistics.prepare_seconds += prepared.elapsed().as_secs_f64();
        let mut usable: Vec<(usize, LaneTables)> = tables
            .into_iter()
            .enumerate()
            .filter_map(|(index, tables)| tables.map(|tables| (index, tables)))
            .collect();
        let mut jobs = vec![];
        for size in job_sizes(usable.len(), executor.threads(), options.lanes)
            .into_iter()
            .rev()
        {
            jobs.push(usable.split_off(usable.len() - size));
        }
        jobs.reverse();
        let started = Instant::now();
        let (compose, seed) = (self.compose, self.seed);
        let results = executor.map(jobs, |job| {
            let begun = Instant::now();
            let (indices, tables): (Vec<usize>, Vec<LaneTables>) = job.into_iter().unzip();
            let replayed = match options.denominators {
                PolynomialDenominators::Factorized => replay(
                    trace,
                    outputs,
                    tables.len(),
                    &mut Factorized::new(&tables, basis, options.max_degree),
                ),
                PolynomialDenominators::Dense => replay(
                    trace,
                    outputs,
                    tables.len(),
                    &mut Dense::new(&tables, basis, options.max_degree),
                ),
            };
            let replay_seconds = begun.elapsed().as_secs_f64();
            // Compose every lane's terminal images (None: unusable prime).
            let composed = match (compose, &replayed) {
                (Some(program), Ok(replayed)) => Some(
                    indices
                        .iter()
                        .zip(&replayed.images)
                        .map(|(&index, images)| {
                            compose_images(program, primes[index], images, seed)
                        })
                        .collect::<Result<Vec<_>, _>>(),
                ),
                _ => None,
            };
            let compose_seconds = begun.elapsed().as_secs_f64() - replay_seconds;
            (indices, replayed, composed, replay_seconds, compose_seconds)
        })?;
        statistics.replay_seconds += started.elapsed().as_secs_f64();
        let mut images: Vec<Option<PrimeImages<()>>> = primes.iter().map(|_| None).collect();
        for (indices, replayed, composed, seconds, compose_seconds) in results {
            let replayed = replayed?;
            statistics.replay_cpu_seconds += seconds;
            statistics.composition_cpu_seconds += compose_seconds;
            let composed = composed.transpose()?;
            statistics.passes += 1;
            statistics.lanes_replayed += indices.len();
            statistics.cancel_checks += replayed.cancel_checks * indices.len();
            statistics.peak_weight_words = statistics.peak_weight_words.max(replayed.peak_words);
            let labels = std::iter::once(None)
                .chain(trace.phase_starts.iter().map(|&(phase, _)| Some(phase)));
            for ((label, seconds), volume) in labels
                .zip(&replayed.segment_seconds)
                .zip(&replayed.segment_volume)
            {
                match statistics
                    .replay_phase_cpu_seconds
                    .iter_mut()
                    .find(|(known, _)| *known == label)
                {
                    Some((_, total)) => *total += seconds,
                    None => statistics.replay_phase_cpu_seconds.push((label, *seconds)),
                }
                // The volume per lane is the same in every job: keep one.
                if !statistics
                    .replay_phase_volume
                    .iter()
                    .any(|(known, _)| *known == label)
                {
                    statistics.replay_phase_volume.push((label, *volume));
                }
            }
            match composed {
                None => {
                    for (index, terminals) in indices.into_iter().zip(replayed.images) {
                        images[index] = Some(PrimeImages {
                            structure: self.structure.clone(),
                            terminals,
                        });
                    }
                }
                Some(composed) => {
                    for (index, (terminals, points)) in indices.into_iter().zip(composed) {
                        statistics.composition_points += points.drawn;
                        statistics.bad_points += points.bad;
                        images[index] = terminals.map(|terminals| PrimeImages {
                            structure: self.structure.clone(),
                            terminals,
                        });
                    }
                }
            }
        }
        Ok(images)
    }

    /// One wave of jobs: a replay's cost grows with its lanes, so a round of
    /// more primes than threads would take as long as several rounds, which
    /// can stop early. Few threads share one replay among up to `lanes`
    /// primes.
    fn round_capacity(&self, threads: usize) -> usize {
        threads.max(self.options.lanes)
    }

    /// The first round already occupies every thread with one prime.
    fn round_minimum(&self, threads: usize) -> usize {
        threads
    }

    /// A verification is a full replay: computing it within a
    /// reconstruction round saves a round in which one thread works.
    fn prefetch_verification(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests;
