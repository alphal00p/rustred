//! Black-box driver: univariate reconstruction of many functions that one
//! probe evaluates together (FireFly, arXiv:1904.00009 §2 and App. B;
//! Kira 2, arXiv:2008.06494 §2.2, where the probe is a numeric solve and the
//! functions are the master coefficients of the requested integrals).
//!
//! Per function:
//! 1. **First prime** (or after a shape change): Newton and Thiele raced
//!    with early termination ([`Interpolator`]); the normalised image fixes
//!    the degrees ([`Shape`]).
//! 2. **Later primes**: the degrees are known; `unknowns + check_points`
//!    probes suffice ([`fit_known_shape`]; FireFly §2.3). Functions already
//!    accepted are dropped from the prime ("pruning"), so a prime costs the
//!    maximum over the open functions only.
//! 3. **Lifting**: CRT plus Wang/MQRR race ([`Lift`]); accepted once a guess
//!    is unchanged over two consecutive primes. A function whose lifting
//!    stays pending at three or more primes is retried with each single
//!    prime left out (one unlucky or corrupted image must not poison the
//!    CRT).
//! 4. **Verification**: every accepted function is compared with the
//!    verifier's probe at fresh points of a fresh prime (FireFly Alg. 3);
//!    failed functions are reset and reconstructed again.
//!
//! Probes come in bunches of points that share one black-box traversal
//! (FireFly 2, arXiv:2004.01463 §3.1); bunches of one round run in
//! parallel (Rayon). A probe failure at a point (an unlucky point) discards
//! that point only; a new point is drawn. Probe values can be logged and
//! replayed ([`ProbeLog`]): points are drawn from a deterministic stream
//! per prime, so a resumed run re-reads exactly the probes it needs. A failed
//! point is logged as failed and stays failed on resume, so black boxes must
//! report only deterministic failures (poles), never transient ones.
//!
//! Acceptance strength: a wrong function passes the final check with
//! probability at most about `((dN + dD + dN' + dD') / p)^verify_points`
//! (degrees of the true and of the wrong function, `p` the fresh prime,
//! Schwartz-Zippel), plus an order-`1/p` chance that the fresh prime divides
//! every coefficient difference. With the default 2 points at a prime near
//! `2^62` this is negligible for degrees in the hundreds.
//!
//! Contracts checked at run time (typed errors, never a panic or a hang):
//! the black box and the verifier report the same number of outputs and
//! return one result per point and one value per output; caller primes are
//! primes of at least 16 bits and each is used once; the point budget of a
//! prime is capped by its `p - 1` residues.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::time::Instant;

use rand::{Rng, SeedableRng, rngs::StdRng};
use rayon::prelude::*;
use symbolica::domains::RingOps;
use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::rational::Rational;
use symbolica::prelude::Ring;

use super::Zp;
use super::interpolate::{DEFAULT_CONFIRMATIONS, Interpolator};
use super::known::{Shape, fit_known_shape};
use super::lift::{Lift, LiftStatus};
use super::poly::{from_u64, to_u64};
use super::rational::{RationalFunction, rational_image};

/// Why a probe at one point is unusable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProbeFailure(pub &'static str);

/// Values of every function at one point, or the failure.
pub type ProbeResult = Result<Vec<u64>, ProbeFailure>;

/// A black box evaluating all functions at points of one free variable.
pub trait BlackBox: Sync {
    type Prime: PrimeBlackBox;
    /// Number of functions (fixed order).
    fn outputs(&self) -> usize;
    /// Prepare one prime; `None` when the prime is unusable (for example
    /// an input coefficient has a pole modulo it).
    fn at_prime(&self, prime: u64) -> Option<Self::Prime>;
}

/// The black box at one prime.
pub trait PrimeBlackBox: Sync {
    /// Evaluate one bunch of distinct points (canonical residues).
    fn probe(&self, xs: &[u64]) -> Vec<ProbeResult>;
}

/// Records and replays probe values (checkpoints).
pub trait ProbeLog {
    /// Probes recorded earlier at `prime`: `(x, values)`, `None` for a
    /// failed point.
    /// A log that belongs to another black box, or a corrupted one, is an
    /// error: it must never be mixed in.
    fn load(&mut self, prime: u64) -> std::io::Result<Vec<(u64, Option<Vec<u64>>)>>;
    /// Append new probes at `prime`.
    fn record(&mut self, prime: u64, probes: &[(u64, Option<Vec<u64>>)]) -> std::io::Result<()>;
}

/// Settings of [`reconstruct_univariate`].
#[derive(Clone, Debug)]
pub struct UnivariateOptions {
    /// Agreeing extra points before an interpolation terminates.
    pub confirmations: u32,
    /// Points per black-box call.
    pub bunch: usize,
    /// Bunches evaluated in parallel per round.
    pub parallel_bunches: usize,
    /// Probe points evaluated per prime at most (incl. failed ones).
    pub max_points_per_prime: usize,
    /// Primes used for lifting at most.
    ///
    /// A bad image of the same shape as the good ones (a corrupted probe) is
    /// dropped only by the leave-one-prime-out retry, which first runs when
    /// a function holds 8 images (see `retry_due`). With `max_primes < 8`
    /// such a function therefore ends in
    /// [`ReconstructionError::OutOfPrimes`]: a loud failure, never a wrong
    /// result.
    pub max_primes: usize,
    /// Extra points of a known-shape prime.
    pub check_points: usize,
    /// Fresh points of the fresh verification prime.
    pub verify_points: usize,
    /// Verification rounds before giving up.
    pub max_verifications: usize,
    pub seed: u64,
    /// Prime list, used in order; `None` for [`reconstruction_primes`].
    pub primes: Option<Vec<u64>>,
    /// Test hook: the first points of the first prime (for example a
    /// known pole). They are probed like any other point.
    #[doc(hidden)]
    pub forced_points: Vec<Rational>,
    /// Denominator first: the group of each function (for example the
    /// master coefficients of one integral). The least common denominator
    /// of a group is lifted to Q like a function; once known, later primes
    /// fit the polynomial `f * denominator` of known degree instead of the
    /// rational function, whenever that needs fewer points (Kira 2,
    /// arXiv:2008.06494 §3.8, known denominators divided out of the
    /// probes).
    pub groups: Option<Vec<usize>>,
}

impl Default for UnivariateOptions {
    fn default() -> Self {
        Self {
            confirmations: DEFAULT_CONFIRMATIONS,
            bunch: 8,
            parallel_bunches: 1,
            max_points_per_prime: 4096,
            max_primes: 16,
            check_points: 1,
            verify_points: 2,
            max_verifications: 3,
            seed: 0x7265_636f_6e73_7472,
            primes: None,
            forced_points: Vec::new(),
            groups: None,
        }
    }
}

/// `count` primes below `2^62`, descending, skipping `2^61 - 1` (the
/// structure-sample prime of the LiteRed lane, `solver::litered::DEFAULT_PRIME`):
/// FireFly-style large word-size primes for which the sum of two residues
/// fits in 64 bits. The same list as `solver::litered::reconstruction_primes`.
pub fn reconstruction_primes(count: usize) -> Vec<u64> {
    let mut primes = Vec::with_capacity(count);
    let mut candidate = (1u64 << 62) - 1;
    while primes.len() < count {
        if candidate != (1u64 << 61) - 1
            && symbolica::domains::integer::Integer::from(candidate).is_prime(0)
        {
            primes.push(candidate);
        }
        candidate -= 2;
    }
    primes
}

/// Smallest caller prime accepted (16 bits): smaller primes have too few
/// residues for useful point budgets and too high unlucky-point rates.
pub const MIN_PRIME: u64 = 1 << 16;

/// Per-prime statistics.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrimeReport {
    pub prime: u64,
    /// Functions still open at the start of the prime.
    pub open: usize,
    /// Of those, interpolated without known degrees.
    pub interpolated: usize,
    /// Points evaluated (incl. failed and replayed ones).
    pub points: usize,
    pub good_points: usize,
    /// Replayed from the probe log.
    pub replayed: usize,
    pub failures: BTreeMap<String, usize>,
    /// Functions accepted after this prime.
    pub accepted: usize,
    /// Known-shape fits that failed.
    pub misfits: usize,
    /// Interpolations that did not terminate within the point budget.
    pub unfinished: usize,
    /// Known-degree fits done on `f * group denominator` (polynomials).
    pub polynomial_fits: usize,
}

/// Verification statistics of one fresh prime.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VerificationReport {
    pub prime: u64,
    pub points: usize,
    pub failed_points: usize,
    pub failed_functions: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReconstructionReport {
    pub functions: usize,
    pub zero_functions: usize,
    pub primes: Vec<PrimeReport>,
    /// Primes the black box refused.
    pub skipped_primes: Vec<u64>,
    pub verifications: Vec<VerificationReport>,
    /// Functions recovered by leaving one prime out.
    pub recovered: usize,
    pub shape_changes: usize,
    pub max_numerator_degree: usize,
    pub max_denominator_degree: usize,
    /// Most primes any function needed.
    pub max_primes_per_function: usize,
    /// Groups whose common denominator was lifted to Q.
    pub known_denominators: usize,
    pub probe_seconds: f64,
    pub reconstruction_seconds: f64,
    pub verification_seconds: f64,
}

/// Result of [`reconstruct_univariate`].
#[derive(Clone, Debug)]
pub struct Reconstruction {
    pub functions: Vec<RationalFunction>,
    pub report: ReconstructionReport,
}

#[derive(Clone, Debug)]
pub enum ReconstructionError {
    /// `open` functions were not accepted within the prime budget.
    OutOfPrimes {
        open: usize,
        primes: usize,
    },
    /// Verification kept failing.
    Verification {
        rounds: usize,
        failed: usize,
    },
    /// No usable verification point (every fresh point failed).
    VerificationPoints {
        prime: u64,
    },
    Log(String),
    /// The black box or the verifier broke its contract (output count,
    /// results per point, values per output).
    BlackBox(String),
    /// Invalid options (for example a caller prime that is not a prime).
    InvalidOptions(String),
}

impl fmt::Display for ReconstructionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfPrimes { open, primes } => {
                write!(
                    f,
                    "{open} functions not reconstructed within {primes} primes"
                )
            }
            Self::Verification { rounds, failed } => write!(
                f,
                "fresh-prime verification failed {rounds} times ({failed} functions in the last round)"
            ),
            Self::VerificationPoints { prime } => {
                write!(f, "no usable verification point at prime {prime}")
            }
            Self::Log(message) => write!(f, "probe log: {message}"),
            Self::BlackBox(message) => write!(f, "black-box contract: {message}"),
            Self::InvalidOptions(message) => write!(f, "invalid options: {message}"),
        }
    }
}

impl std::error::Error for ReconstructionError {}

#[derive(Clone, Debug, Default)]
struct FunctionState {
    shape: Option<Shape>,
    lift: Lift,
    /// `(prime, numerator, denominator)` images accumulated in `lift`.
    images: Vec<(u64, Vec<u64>, Vec<u64>)>,
    done: bool,
    misfits: u32,
    relearn: bool,
}

impl FunctionState {
    /// The lift of every image but `skip`, accepted (`Stable`) when the
    /// guesses at the last two moduli agree. The CRT runs over all kept
    /// images, but the reconstruction race only twice (before and after the
    /// last kept image), so one rebuild costs two reconstructions, not one
    /// per image.
    fn rebuild(&self, skip: usize) -> Option<Lift> {
        let kept: Vec<&(u64, Vec<u64>, Vec<u64>)> = self
            .images
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != skip)
            .map(|(_, image)| image)
            .collect();
        let (last, rest) = kept.split_last()?;
        let mut lift = Lift::default();
        let accumulate =
            |lift: &mut Lift, (prime, numerator, denominator): &(u64, Vec<u64>, Vec<u64>)| {
                let field = Zp64::new(*prime);
                lift.accumulate(
                    &field,
                    &from_u64(&field, numerator),
                    &from_u64(&field, denominator),
                )
            };
        for image in rest {
            if accumulate(&mut lift, image) != LiftStatus::Pending {
                return None;
            }
        }
        lift.race();
        if accumulate(&mut lift, last) != LiftStatus::Pending {
            return None;
        }
        (lift.race() == LiftStatus::Stable).then_some(lift)
    }

    fn consistent(&self, candidate: &RationalFunction, skip: usize) -> bool {
        self.images
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != skip)
            .all(|(_, (prime, numerator, denominator))| {
                let field = Zp64::new(*prime);
                Lift::matches(
                    candidate,
                    &field,
                    &from_u64(&field, numerator),
                    &from_u64(&field, denominator),
                )
            })
    }
}

/// Common denominator of a group of functions (denominator first).
#[derive(Clone, Debug, Default)]
struct GroupState {
    members: Vec<usize>,
    lift: Lift,
    /// The least common denominator over Q, normalised (lowest nonzero
    /// coefficient one), once stable.
    known: Option<Vec<Rational>>,
}

impl GroupState {
    /// Image of the known denominator; `None` for an unlucky prime.
    fn image(&self, field: &Zp64) -> Option<Vec<Zp>> {
        let known = self.known.as_ref()?;
        let image: Option<Vec<Zp>> = known
            .iter()
            .map(|value| rational_image(value, field))
            .collect();
        let image = image?;
        // The image must keep the degree and the normalisation.
        let low = super::poly::low_degree(field, &image);
        let expected_low = known.iter().position(|value| !value.is_zero()).unwrap_or(0);
        (field.is_one(&image[low]) && low == expected_low && !field.is_zero(image.last()?))
            .then_some(image)
    }
}

/// Points and probe values gathered at one prime.
struct PrimeSampler<'a, P: PrimeBlackBox> {
    black_box: &'a P,
    outputs: usize,
    field: Zp64,
    rng: StdRng,
    forced: Vec<u64>,
    used: std::collections::HashSet<u64>,
    cache: HashMap<u64, Option<Vec<u64>>>,
    xs: Vec<Zp>,
    values: Vec<Vec<u64>>,
    report: PrimeReport,
}

impl<P: PrimeBlackBox> PrimeSampler<'_, P> {
    /// A point not used before at this prime; `None` once every nonzero
    /// residue is used (a small prime), so drawing always terminates.
    fn draw(&mut self) -> Option<u64> {
        let prime = self.field.get_prime();
        if self.used.len() as u64 >= prime - 1 {
            return None;
        }
        loop {
            let x = if self.forced.is_empty() {
                self.rng.random_range(1..prime)
            } else {
                self.forced.remove(0)
            };
            if x != 0 && x < prime && self.used.insert(x) {
                return Some(x);
            }
        }
    }

    /// Evaluate up to `count` new points (at least one); `false` when no new
    /// point is left at this prime.
    fn sample(
        &mut self,
        count: usize,
        options: &UnivariateOptions,
        log: &mut Option<&mut dyn ProbeLog>,
        seconds: &mut f64,
    ) -> Result<bool, ReconstructionError> {
        let bunch = options.bunch.max(1);
        let count = count.max(1);
        let points: Vec<u64> = (0..count).map_while(|_| self.draw()).collect();
        let drawn = !points.is_empty();
        let fresh: Vec<u64> = points
            .iter()
            .copied()
            .filter(|x| !self.cache.contains_key(x))
            .collect();
        let started = Instant::now();
        let black_box = self.black_box;
        let outputs = self.outputs;
        let results: Vec<ProbeResult> = fresh
            .par_chunks(bunch)
            .map(|chunk| {
                let results = black_box.probe(chunk);
                if results.len() != chunk.len() {
                    return Err(ReconstructionError::BlackBox(format!(
                        "{} results for {} points",
                        results.len(),
                        chunk.len()
                    )));
                }
                if let Some(Ok(values)) = results
                    .iter()
                    .find(|result| result.as_ref().is_ok_and(|values| values.len() != outputs))
                {
                    return Err(ReconstructionError::BlackBox(format!(
                        "{} values for {outputs} outputs",
                        values.len()
                    )));
                }
                Ok(results)
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();
        *seconds += started.elapsed().as_secs_f64();
        let mut records = Vec::with_capacity(fresh.len());
        for (x, result) in fresh.iter().zip(results) {
            let entry = match result {
                Ok(values) => Some(values),
                Err(failure) => {
                    *self
                        .report
                        .failures
                        .entry(failure.0.to_string())
                        .or_default() += 1;
                    None
                }
            };
            records.push((*x, entry));
        }
        if let Some(log) = log.as_deref_mut()
            && !records.is_empty()
        {
            log.record(self.field.get_prime(), &records)
                .map_err(|error| ReconstructionError::Log(error.to_string()))?;
        }
        self.cache.extend(records);
        self.report.replayed += points.len() - fresh.len();
        // Every point is drawn once per prime: consume its cache entry.
        for x in points {
            self.report.points += 1;
            if let Some(values) = self.cache.remove(&x).flatten() {
                self.report.good_points += 1;
                self.xs.push(self.field.to_element(x));
                self.values.push(values);
            }
        }
        Ok(drawn)
    }
}

/// A function's new image at one prime: `(function, normalised image or
/// None, interpolated (true) or fitted to a known shape (false))`.
type NewImage = (usize, Option<(Vec<Zp>, Vec<Zp>)>, bool);

/// Image counts at which a pending function tries leave-one-prime-out:
/// 8, 16, 32, ... A same-shape bad image (a corrupted probe) is rare (the
/// five-loop production run recovered none), and one retry at `k` images
/// costs `2k` reconstruction races, so retries are kept to `O(log k)` per
/// function, starting when a function needs clearly more primes than usual.
/// Consequence: a corrupted image is recovered from only once 8 images are
/// in, so a run with `max_primes < 8` fails with `OutOfPrimes` instead.
fn retry_due(images: usize) -> bool {
    images >= 8 && images.is_power_of_two()
}

fn prime_rng(seed: u64, prime: u64, salt: u64) -> StdRng {
    let mut z = seed ^ prime.rotate_left(17) ^ salt;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    StdRng::seed_from_u64(z ^ (z >> 31))
}

/// Reconstruct every function of `black_box` as a univariate rational
/// function over Q; verify with `verifier` (which may be `black_box`).
pub fn reconstruct_univariate<B: BlackBox, V: BlackBox>(
    black_box: &B,
    verifier: &V,
    options: &UnivariateOptions,
    mut log: Option<&mut dyn ProbeLog>,
) -> Result<Reconstruction, ReconstructionError> {
    let outputs = black_box.outputs();
    if verifier.outputs() != outputs {
        return Err(ReconstructionError::BlackBox(format!(
            "the verifier has {} outputs, the black box {outputs}",
            verifier.outputs()
        )));
    }
    if let Some(primes) = &options.primes
        && let Some(&bad) = primes
            .iter()
            .find(|&&p| p < MIN_PRIME || !symbolica::domains::integer::Integer::from(p).is_prime(0))
    {
        return Err(ReconstructionError::InvalidOptions(format!(
            "caller prime {bad} is not a prime of at least 16 bits"
        )));
    }
    let mut report = ReconstructionReport {
        functions: outputs,
        ..Default::default()
    };
    let primes = options.primes.clone().unwrap_or_else(|| {
        reconstruction_primes(options.max_primes + 8 * options.max_verifications + 8)
    });
    // Every prime is used once: for reconstruction or, later in the same
    // stream, for verification, so a verification prime is never a
    // reconstruction prime (also for a caller list with repetitions).
    let mut seen = std::collections::HashSet::new();
    let mut primes = primes.into_iter().filter(move |&prime| seen.insert(prime));
    let mut states = vec![FunctionState::default(); outputs];
    let mut groups: Vec<GroupState> = Vec::new();
    let mut group_of: Vec<Option<usize>> = vec![None; outputs];
    if let Some(ids) = &options.groups {
        let mut index: HashMap<usize, usize> = HashMap::new();
        for (function, &id) in ids.iter().enumerate().take(outputs) {
            let next = index.len();
            let group = *index.entry(id).or_insert(next);
            if group == groups.len() {
                groups.push(GroupState::default());
            }
            groups[group].members.push(function);
            group_of[function] = Some(group);
        }
    }
    let mut used_primes = 0usize;
    let mut forced: Vec<Rational> = options.forced_points.clone();
    let mut verifications = 0usize;
    loop {
        while states.iter().any(|state| !state.done) {
            let open = states.iter().filter(|state| !state.done).count();
            if used_primes >= options.max_primes {
                return Err(ReconstructionError::OutOfPrimes {
                    open,
                    primes: used_primes,
                });
            }
            let Some(prime) = primes.next() else {
                return Err(ReconstructionError::OutOfPrimes {
                    open,
                    primes: used_primes,
                });
            };
            let Some(at_prime) = black_box.at_prime(prime) else {
                report.skipped_primes.push(prime);
                continue;
            };
            used_primes += 1;
            let field = Zp64::new(prime);
            let forced_here: Vec<u64> = forced
                .drain(..)
                .filter_map(|value| rational_image(&value, &field))
                .map(|value| field.from_element(&value))
                .collect();
            let prime_report = run_prime(
                &at_prime,
                field,
                &mut states,
                (&mut groups, &group_of),
                options,
                forced_here,
                &mut log,
                &mut report,
            )?;
            report.primes.push(prime_report);
        }
        // Fresh prime, fresh points, the verifier's probe (FireFly Alg. 3).
        let started = Instant::now();
        let functions: Vec<RationalFunction> = states
            .iter()
            .map(|state| state.lift.result().cloned().expect("accepted"))
            .collect();
        let mut chosen = None;
        for prime in primes.by_ref() {
            let field = Zp64::new(prime);
            if functions
                .iter()
                .any(|function| function.image(&field).is_none())
            {
                report.skipped_primes.push(prime);
                continue;
            }
            match verifier.at_prime(prime) {
                Some(at_prime) => {
                    chosen = Some((prime, at_prime));
                    break;
                }
                None => report.skipped_primes.push(prime),
            }
        }
        let Some((prime, at_prime)) = chosen else {
            return Err(ReconstructionError::OutOfPrimes {
                open: 0,
                primes: used_primes,
            });
        };
        let field = Zp64::new(prime);
        let images: Vec<(Vec<Zp>, Vec<Zp>)> = functions
            .iter()
            .map(|function| function.image(&field).expect("checked"))
            .collect();
        let mut rng = prime_rng(options.seed, prime, 0x7665_7269_6679);
        let mut verification = VerificationReport {
            prime,
            ..Default::default()
        };
        let mut failed: Vec<bool> = vec![false; outputs];
        let mut attempts = 0usize;
        let wanted = options.verify_points.max(1);
        while verification.points < wanted {
            attempts += 1;
            if attempts > 64 * wanted {
                return Err(ReconstructionError::VerificationPoints { prime });
            }
            let x = rng.random_range(1..prime);
            let point = field.to_element(x);
            // Skip poles of any accepted function.
            let mut predicted = Vec::with_capacity(outputs);
            let mut pole = false;
            for (numerator, denominator) in &images {
                let d = super::poly::horner(&field, denominator, &point);
                match field.try_inv(&d) {
                    Some(inverse) => predicted
                        .push(field.mul(&super::poly::horner(&field, numerator, &point), &inverse)),
                    None => {
                        pole = true;
                        break;
                    }
                }
            }
            if pole {
                continue;
            }
            let mut results = at_prime.probe(&[x]);
            if results.len() != 1 {
                return Err(ReconstructionError::BlackBox(format!(
                    "the verifier returned {} results for 1 point",
                    results.len()
                )));
            }
            let probe = results.pop().expect("one result");
            let Ok(values) = probe else {
                verification.failed_points += 1;
                continue;
            };
            if values.len() != outputs {
                return Err(ReconstructionError::BlackBox(format!(
                    "the verifier returned {} values for {outputs} outputs",
                    values.len()
                )));
            }
            verification.points += 1;
            for (index, (value, expected)) in values.iter().zip(&predicted).enumerate() {
                if *value != field.from_element(expected) {
                    failed[index] = true;
                }
            }
        }
        verification.failed_functions = failed.iter().filter(|&&f| f).count();
        report.verification_seconds += started.elapsed().as_secs_f64();
        let failures = verification.failed_functions;
        report.verifications.push(verification);
        verifications += 1;
        if failures == 0 {
            break;
        }
        if verifications >= options.max_verifications {
            return Err(ReconstructionError::Verification {
                rounds: verifications,
                failed: failures,
            });
        }
        for (state, failed) in states.iter_mut().zip(failed) {
            if failed {
                *state = FunctionState::default();
            }
        }
    }
    let functions: Vec<RationalFunction> = states
        .iter()
        .map(|state| state.lift.result().cloned().expect("verified"))
        .collect();
    for (function, state) in functions.iter().zip(&states) {
        let (n, d) = function.degrees();
        report.max_numerator_degree = report.max_numerator_degree.max(n);
        report.max_denominator_degree = report.max_denominator_degree.max(d);
        report.max_primes_per_function = report
            .max_primes_per_function
            .max(state.lift.primes().len());
        if function.is_zero() {
            report.zero_functions += 1;
        }
    }
    report.known_denominators = groups.iter().filter(|group| group.known.is_some()).count();
    Ok(Reconstruction { functions, report })
}

#[allow(clippy::too_many_arguments)]
fn run_prime<P: PrimeBlackBox>(
    black_box: &P,
    field: Zp64,
    states: &mut [FunctionState],
    (groups, group_of): (&mut Vec<GroupState>, &Vec<Option<usize>>),
    options: &UnivariateOptions,
    forced: Vec<u64>,
    log: &mut Option<&mut dyn ProbeLog>,
    report: &mut ReconstructionReport,
) -> Result<PrimeReport, ReconstructionError> {
    let prime = field.get_prime();
    let open: Vec<usize> = (0..states.len()).filter(|&i| !states[i].done).collect();
    let interpolate: Vec<usize> = open
        .iter()
        .copied()
        .filter(|&i| states[i].shape.is_none() || states[i].relearn)
        .collect();
    let known: Vec<usize> = open
        .iter()
        .copied()
        .filter(|&i| states[i].shape.is_some() && !states[i].relearn)
        .collect();
    // Denominator first: images of the known group denominators, and the
    // polynomial degree of `f * denominator` where that needs fewer points.
    let group_images: Vec<Option<Vec<Zp>>> =
        groups.iter().map(|group| group.image(&field)).collect();
    let polynomial_degree = |index: usize| -> Option<usize> {
        let shape = states[index].shape?;
        if shape.zero {
            return None;
        }
        let denominator = group_images[group_of[index]?].as_ref()?;
        let degree = shape.numerator + (denominator.len() - 1).checked_sub(shape.denominator)?;
        (degree + 1 < shape.unknowns()).then_some(degree)
    };
    let modes: HashMap<usize, usize> = known
        .iter()
        .filter_map(|&index| polynomial_degree(index).map(|degree| (index, degree)))
        .collect();
    let need_known = known
        .iter()
        .map(|&i| {
            modes.get(&i).map_or_else(
                || states[i].shape.expect("known").unknowns(),
                |degree| degree + 1,
            ) + options.check_points.max(1)
        })
        .max()
        .unwrap_or(0);
    let cache: HashMap<u64, Option<Vec<u64>>> = match log.as_deref_mut() {
        Some(log) => log
            .load(prime)
            .map_err(|error| ReconstructionError::Log(error.to_string()))?
            .into_iter()
            .collect(),
        None => HashMap::new(),
    };
    let mut sampler = PrimeSampler {
        black_box,
        outputs: states.len(),
        field: field.clone(),
        rng: prime_rng(options.seed, prime, 0x7072_6f62_6573),
        forced,
        used: Default::default(),
        cache,
        xs: Vec::new(),
        values: Vec::new(),
        report: PrimeReport {
            prime,
            open: open.len(),
            interpolated: interpolate.len(),
            ..Default::default()
        },
    };
    let mut interpolators: Vec<(usize, Interpolator)> = interpolate
        .iter()
        .map(|&i| (i, Interpolator::new(options.confirmations)))
        .collect();
    let mut fed = 0usize;
    let round = options.bunch.max(1) * options.parallel_bunches.max(1);
    // A prime has `p - 1` usable points at most.
    let budget = options
        .max_points_per_prime
        .min(usize::try_from(prime - 1).unwrap_or(usize::MAX));
    loop {
        let interpolating = interpolators
            .iter()
            .any(|(_, interpolator)| !interpolator.done());
        if !interpolating && sampler.xs.len() >= need_known {
            break;
        }
        if sampler.report.points >= budget {
            break;
        }
        let want = if interpolating {
            round
        } else {
            need_known - sampler.xs.len()
        };
        let want = want.min(budget - sampler.report.points);
        let mut probe_seconds = 0.0;
        if !sampler.sample(want, options, log, &mut probe_seconds)? {
            break;
        }
        report.probe_seconds += probe_seconds;
        let started = Instant::now();
        let xs = &sampler.xs[fed..];
        let values = &sampler.values[fed..];
        let field_ref = &field;
        interpolators
            .par_iter_mut()
            .for_each(|(index, interpolator)| {
                for (x, values) in xs.iter().zip(values) {
                    if interpolator.done() {
                        break;
                    }
                    let y = field_ref.to_element(values[*index]);
                    interpolator.feed(field_ref, x, &y);
                }
            });
        fed = sampler.xs.len();
        report.reconstruction_seconds += started.elapsed().as_secs_f64();
    }
    let started = Instant::now();
    let mut prime_report = sampler.report;
    // New images: from the interpolators, then from known-shape fits.
    let mut images: Vec<NewImage> = interpolators
        .par_iter()
        .map(|(index, interpolator)| {
            let image = (interpolator.done() && !interpolator.failed())
                .then(|| interpolator.rational(&field))
                .flatten();
            (*index, image, true)
        })
        .collect();
    let xs = &sampler.xs;
    let values = &sampler.values;
    // Denominator values per group and point (polynomial mode).
    let denominator_values: Vec<Option<Vec<Zp>>> = group_images
        .iter()
        .map(|image| {
            image.as_ref().map(|image| {
                xs.iter()
                    .map(|x| super::poly::horner(&field, image, x))
                    .collect()
            })
        })
        .collect();
    let fitted: Vec<NewImage> = known
        .par_iter()
        .map(|&index| {
            let shape = states[index].shape.expect("known");
            let ys: Vec<Zp> = values
                .iter()
                .map(|values| field.to_element(values[index]))
                .collect();
            if let Some(&degree) = modes.get(&index) {
                let group = group_of[index].expect("grouped");
                let denominator = group_images[group].as_ref().expect("known image");
                let at = denominator_values[group].as_ref().expect("known image");
                let zs: Vec<Zp> = ys.iter().zip(at).map(|(y, d)| field.mul(y, d)).collect();
                let polynomial = Shape {
                    numerator: degree,
                    denominator: 0,
                    low: 0,
                    zero: false,
                };
                let image = fit_known_shape(&field, &polynomial, xs, &zs)
                    .and_then(|(p, _)| super::poly::normalise(&field, &p, denominator))
                    .filter(|(n, d)| Shape::of(&field, n, d) == shape);
                return (index, image, false);
            }
            (index, fit_known_shape(&field, &shape, xs, &ys), false)
        })
        .collect();
    prime_report.polynomial_fits += modes.len();
    images.extend(fitted);
    let mut prime_images: HashMap<usize, Vec<Zp>> = HashMap::new();
    for (index, image, interpolated) in images {
        let state = &mut states[index];
        let Some((numerator, denominator)) = image else {
            if interpolated {
                prime_report.unfinished += 1;
            } else {
                prime_report.misfits += 1;
                state.misfits += 1;
                if state.misfits >= 2 {
                    state.relearn = true;
                }
            }
            continue;
        };
        if interpolated {
            let shape = Shape::of(&field, &numerator, &denominator);
            if state.shape.is_some_and(|old| old != shape) {
                // A larger shape in a new prime: the earlier prime(s) were
                // unlucky, or a smaller one: this prime is. Keep the larger.
                let old = state.shape.expect("checked");
                report.shape_changes += 1;
                if shape.unknowns() < old.unknowns() {
                    state.relearn = false;
                    prime_report.misfits += 1;
                    continue;
                }
                state.lift = Lift::default();
                state.images.clear();
            }
            state.shape = Some(shape);
            state.relearn = false;
            state.misfits = 0;
        } else {
            state.misfits = 0;
        }
        if group_of[index].is_some() {
            prime_images.insert(index, denominator.clone());
        }
        state.images.push((
            prime,
            to_u64(&field, &numerator),
            to_u64(&field, &denominator),
        ));
        match state.lift.add(&field, &numerator, &denominator) {
            LiftStatus::Stable => {
                state.done = true;
                prime_report.accepted += 1;
            }
            LiftStatus::ShapeMismatch | LiftStatus::DuplicatePrime => {
                state.images.pop();
                state.relearn = true;
            }
            // The accumulated images had a smaller shape: keep this one only.
            LiftStatus::Restarted => {
                let image = state.images.pop().expect("pushed above");
                state.images = vec![image];
            }
            LiftStatus::Pending => {
                // Leave-one-prime-out (one unlucky or corrupted image must
                // not poison the CRT), only at stall checkpoints of the image
                // count (`retry_due`): O(k) races per function over k primes
                // instead of O(k^3) for a retry at every pending prime.
                if retry_due(state.images.len()) {
                    for skip in 0..state.images.len() {
                        let Some(lift) = state.rebuild(skip) else {
                            continue;
                        };
                        let candidate = lift.result().cloned().expect("stable");
                        if state.consistent(&candidate, skip) {
                            state.images.remove(skip);
                            state.lift = lift;
                            state.done = true;
                            report.recovered += 1;
                            prime_report.accepted += 1;
                            break;
                        }
                    }
                }
            }
        }
    }
    // Lift the least common denominator of every group whose members all
    // have an image in this prime (accepted members from their result).
    let states: &[FunctionState] = states;
    let prime_images = &prime_images;
    let field_ref = &field;
    groups
        .par_iter_mut()
        .filter(|group| group.known.is_none())
        .for_each(|group| {
            let mut common = vec![field_ref.one()];
            let mut seen: Vec<Vec<Zp>> = Vec::new();
            for &member in &group.members {
                let denominator = match prime_images.get(&member) {
                    Some(denominator) => denominator.clone(),
                    None => {
                        let state = &states[member];
                        let image = state
                            .lift
                            .result()
                            .filter(|_| state.done)
                            .and_then(|function| function.image(field_ref));
                        match image {
                            Some((_, denominator)) => denominator,
                            None => return,
                        }
                    }
                };
                // Normalised denominators: a constant one is `[1]`.
                if denominator.len() == 1 || seen.contains(&denominator) {
                    continue;
                }
                common = super::poly::lcm(field_ref, &common, &denominator);
                seen.push(denominator);
            }
            match group.lift.add(field_ref, &common, &[field_ref.one()]) {
                LiftStatus::Stable => {
                    group.known = group
                        .lift
                        .result()
                        .map(|function| function.numerator.clone());
                }
                LiftStatus::ShapeMismatch => group.lift = Lift::default(),
                // A larger common denominator restarted the accumulation.
                LiftStatus::Restarted => {}
                _ => {}
            }
        });
    report.reconstruction_seconds += started.elapsed().as_secs_f64();
    Ok(prime_report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::reconstruct::poly::horner;
    use symbolica::domains::integer::Integer;

    /// Functions `f_k(x) = (x + k)^k / (x - 2)` for k = 0..5, plus zero,
    /// with an optional corrupted prime and a pole at x = 2.
    struct Toy {
        corrupt: Option<u64>,
    }

    struct ToyPrime {
        field: Zp64,
        corrupt: bool,
    }

    impl BlackBox for Toy {
        type Prime = ToyPrime;
        fn outputs(&self) -> usize {
            7
        }
        fn at_prime(&self, prime: u64) -> Option<ToyPrime> {
            Some(ToyPrime {
                field: Zp64::new(prime),
                corrupt: self.corrupt == Some(prime),
            })
        }
    }

    impl PrimeBlackBox for ToyPrime {
        fn probe(&self, xs: &[u64]) -> Vec<ProbeResult> {
            let field = &self.field;
            xs.iter()
                .map(|&x| {
                    let x = field.to_element(x);
                    let pole = field.sub(&x, &field.to_element(2));
                    let inverse = field.try_inv(&pole).ok_or(ProbeFailure("pole"))?;
                    let mut out = Vec::new();
                    for k in 0..6u64 {
                        let base = field.add(&x, &field.to_element(k));
                        let mut value = field.mul(&field.pow(&base, k), &inverse);
                        if self.corrupt && k == 3 {
                            value = field.add(&value, &value);
                        }
                        out.push(field.from_element(&value));
                    }
                    out.push(0);
                    Ok(out)
                })
                .collect()
        }
    }

    fn check(functions: &[RationalFunction]) {
        let q = |n: i64| Rational::from(Integer::from(n));
        for k in 0..6i64 {
            for x in [3i64, 5, -7] {
                let expected = &q((x + k).pow(k as u32)) / &q(x - 2);
                assert_eq!(
                    functions[k as usize].evaluate_rational(&q(x)),
                    Some(expected),
                    "k = {k}"
                );
            }
        }
        assert!(functions[6].is_zero());
    }

    #[test]
    fn reconstructs_and_verifies_with_a_forced_pole() {
        let options = UnivariateOptions {
            forced_points: vec![Rational::from(Integer::from(2))],
            ..Default::default()
        };
        let result = reconstruct_univariate(
            &Toy { corrupt: None },
            &Toy { corrupt: None },
            &options,
            None,
        )
        .unwrap();
        check(&result.functions);
        assert_eq!(result.report.primes[0].failures.get("pole"), Some(&1));
        assert_eq!(result.report.verifications.len(), 1);
        assert!(result.report.primes.len() >= 2);
        // Later primes need only the known-degree points.
        assert!(result.report.primes[1].points < result.report.primes[0].points);
        let _ = horner;
    }

    /// Denominator first: functions sharing the denominator `(x - 2)^3`
    /// need only their numerator degree plus a check point in the primes
    /// after the common denominator is known.
    struct Shared;
    struct SharedPrime(Zp64);
    impl BlackBox for Shared {
        type Prime = SharedPrime;
        fn outputs(&self) -> usize {
            4
        }
        fn at_prime(&self, prime: u64) -> Option<SharedPrime> {
            Some(SharedPrime(Zp64::new(prime)))
        }
    }
    impl PrimeBlackBox for SharedPrime {
        fn probe(&self, xs: &[u64]) -> Vec<ProbeResult> {
            let field = &self.0;
            xs.iter()
                .map(|&x| {
                    let x = field.to_element(x);
                    let t = field.sub(&x, &field.to_element(2));
                    let inverse = field
                        .try_inv(&field.pow(&t, 3))
                        .ok_or(ProbeFailure("pole"))?;
                    // Numerators x^3 + k * 1000003 x + 7, k = 0..3: large
                    // coefficients force several primes.
                    Ok((0..4u64)
                        .map(|k| {
                            let n = field.add(
                                &field.add(
                                    &field.pow(&x, 3),
                                    &field.mul(&field.to_element(1_000_003 * k + 99_999_989), &x),
                                ),
                                &field.to_element(7),
                            );
                            field.from_element(&field.mul(&n, &inverse))
                        })
                        .collect())
                })
                .collect()
        }
    }

    #[test]
    fn denominator_first_fits_numerators_in_later_primes() {
        // Small primes (31 bits) so that lifting takes several primes.
        let mut primes = Vec::new();
        let mut candidate = (1u64 << 31) - 1;
        while primes.len() < 24 {
            if Integer::from(candidate).is_prime(0) {
                primes.push(candidate);
            }
            candidate -= 2;
        }
        let plain = UnivariateOptions {
            primes: Some(primes.clone()),
            max_primes: 20,
            ..Default::default()
        };
        let grouped = UnivariateOptions {
            groups: Some(vec![0; 4]),
            ..plain.clone()
        };
        let a = reconstruct_univariate(&Shared, &Shared, &plain, None).unwrap();
        let b = reconstruct_univariate(&Shared, &Shared, &grouped, None).unwrap();
        assert_eq!(a.functions, b.functions);
        assert_eq!(b.report.known_denominators, 1);
        let fits: usize = b.report.primes.iter().map(|p| p.polynomial_fits).sum();
        assert!(fits > 0, "{:?}", b.report.primes);
        // Rational fits need 3 + 3 + 1 + 1 points, polynomial ones 3 + 1 + 1.
        let points = |r: &Reconstruction| r.report.primes.last().unwrap().points;
        assert!(points(&b) < points(&a), "{} vs {}", points(&b), points(&a));
    }

    #[test]
    fn a_corrupted_prime_is_left_out_and_never_accepted() {
        let primes = reconstruction_primes(24);
        let options = UnivariateOptions {
            primes: Some(primes.clone()),
            ..Default::default()
        };
        let result = reconstruct_univariate(
            &Toy {
                corrupt: Some(primes[0]),
            },
            &Toy { corrupt: None },
            &options,
            None,
        )
        .unwrap();
        check(&result.functions);
        assert!(result.report.recovered >= 1 || result.report.shape_changes >= 1);
    }
}
