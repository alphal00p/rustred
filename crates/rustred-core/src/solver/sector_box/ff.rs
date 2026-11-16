//! Finite-field lane: univariate rational reconstruction of every requested
//! `(target, master)` coefficient in the one free parameter.
//!
//! The black box is the bunch kernel of [`super::probe`] (the selected rows
//! compiled at a prime and eliminated along the modular pivots, `K` points
//! per traversal; Rayon runs over bunches). Reconstruction is
//! [`crate::algebra::reconstruct::reconstruct_univariate`] (FireFly
//! arXiv:1904.00009 §2): Thiele/Newton with early termination in the first
//! prime, known degrees later, CRT with Wang/MQRR, and a check at a fresh
//! prime and fresh points. The check uses the independent reference probe
//! (Symbolica's sparse row reducer with pivot search), not the bunch kernel.
//!
//! A lane that does not reproduce the modular pivots or master set (a
//! [`super::LaneFailure`]) is a failed point: it is counted and a new point
//! is drawn, never used. Fault injection ([`FaultInjection`]) forces such
//! points, discards good lanes and corrupts one prime image, so that tests
//! can show that the result does not change.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use symbolica::domains::RingOps;
use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::rational::Rational;

use crate::algebra::reconstruct::{
    BlackBox, FileProbeLog, PrimeBlackBox, ProbeFailure, ProbeLog, ProbeResult, RationalFunction,
    ReconstructionError, ReconstructionReport, UnivariateOptions, rational_image,
    reconstruct_univariate, reconstruction_primes,
};

use super::super::SolverError;
use super::BoxPhase;
use super::probe::{Compiled, LaneRequest, reference_probe};
use super::rows::Zp;

/// Fault injection for tests and acceptance runs; never set in production.
/// The result of a run with injected faults must equal the clean result.
/// A test hook: hidden from the documentation.
#[doc(hidden)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FaultInjection {
    /// First probe points of the first prime, as values of the free
    /// parameter (for example a known pole): they must be detected as
    /// unlucky, or give correct values.
    pub special_points: Vec<Rational>,
    /// `(prime ordinal, entry)`: at the prime with this position in the
    /// prime stream ([`reconstruction_primes`]), every probe value of
    /// `entry` (row-major `target * masters + master`) is doubled, i.e. a
    /// corrupted prime image that the lifting must reject. It is rejected
    /// by the leave-one-prime-out retry at 8 images, so this needs
    /// `max_primes >= 8`.
    pub corrupt_prime: Option<(usize, usize)>,
    /// Discard this many otherwise good probe lanes as if unlucky (counted
    /// as `injected`). With parallel bunches, which lanes are discarded
    /// depends on scheduling; the result does not.
    pub discard_lanes: usize,
}

/// Finite-field lane settings.
#[derive(Clone, Debug)]
pub struct FfOptions {
    /// Points per traversal: 1, 2, 4, 8 or 16.
    pub bunch: usize,
    /// Random stream of the probe points.
    pub seed: u64,
    /// Reconstruction primes at most (a typed error when exceeded). A
    /// corrupted prime image is recovered from only once a function holds 8
    /// images (see
    /// [`crate::algebra::reconstruct::UnivariateOptions::max_primes`]), so
    /// with fewer than 8 the run fails loudly instead.
    pub max_primes: usize,
    /// Probe points per prime at most, failed ones included.
    pub max_points_per_prime: usize,
    /// Fresh points at the fresh verification prime.
    pub verify_points: usize,
    /// Verification rounds (fresh prime each) before giving up; functions
    /// that fail a round are reconstructed again.
    pub max_verifications: usize,
    /// Agreeing extra points before an interpolation terminates.
    pub confirmations: u32,
    /// Probe values per prime are appended here after every round of
    /// bunches and replayed on resume. Files are bound to a key over the
    /// system, the request and the point stream.
    pub checkpoint_dir: Option<PathBuf>,
    /// Test hook (fault injection); `None` in production.
    #[doc(hidden)]
    pub fault: Option<FaultInjection>,
    /// Denominator first: lift each target's common denominator and fit
    /// numerators only in later primes (see
    /// [`crate::algebra::reconstruct::UnivariateOptions::groups`]).
    pub denominator_first: bool,
}

impl Default for FfOptions {
    fn default() -> Self {
        Self {
            bunch: 8,
            seed: 0x4646_5f6c_616e_6531,
            max_primes: 16,
            max_points_per_prime: 4096,
            verify_points: 2,
            max_verifications: 3,
            confirmations: 2,
            checkpoint_dir: None,
            fault: None,
            denominator_first: false,
        }
    }
}

/// Statistics of one finite-field materialisation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FfReport {
    pub rows: usize,
    pub terms: usize,
    pub columns: usize,
    pub targets: usize,
    pub masters: usize,
    /// `targets x masters`.
    pub entries: usize,
    pub nonzero_entries: usize,
    /// Probe lanes discarded, by reason (all primes, incl. verification).
    pub unlucky: BTreeMap<String, usize>,
    pub reconstruction: ReconstructionReport,
    /// Probe records read back from the checkpoint directory.
    pub resumed_probes: usize,
    pub build_seconds: f64,
    pub seconds: f64,
}

impl<const N: usize> LaneRequest<N> {
    /// `targets x masters`.
    pub(crate) fn entries(&self) -> usize {
        self.routes.len() * self.system.masters.len()
    }

    /// Binds probe logs to the system, the request, the point stream and
    /// any fault injection (injected values must never be replayed into a
    /// clean run).
    pub(crate) fn key(
        &self,
        session_key: &[u8],
        seed: u64,
        fault: Option<&FaultInjection>,
    ) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"rustred-sector-box-ff-probes-v1\0");
        hasher.update(session_key);
        for target in &self.targets {
            for value in target {
                hasher.update(&value.to_le_bytes());
            }
        }
        for value in &self.fixed {
            hasher.update(format!("{value:?};").as_bytes());
        }
        hasher.update(&seed.to_le_bytes());
        if let Some(fault) = fault {
            hasher.update(format!("fault:{fault:?};").as_bytes());
        }
        *hasher.finalize().as_bytes()
    }
}

/// The bunch kernel as a reconstruction black box.
struct KernelBox<'a, const N: usize> {
    request: &'a LaneRequest<N>,
    bunch: usize,
    corrupt: Option<(u64, usize)>,
    discard: AtomicUsize,
    unlucky: std::sync::Mutex<BTreeMap<String, usize>>,
}

struct KernelPrime<'a, const N: usize> {
    owner: &'a KernelBox<'a, N>,
    compiled: Compiled,
    corrupt: Option<usize>,
}

impl<'a, const N: usize> BlackBox for &'a KernelBox<'a, N> {
    type Prime = KernelPrime<'a, N>;

    fn outputs(&self) -> usize {
        self.request.entries()
    }

    fn at_prime(&self, prime: u64) -> Option<KernelPrime<'a, N>> {
        let &owner = self;
        Some(KernelPrime {
            owner,
            compiled: owner.request.compile(prime)?,
            corrupt: owner
                .corrupt
                .and_then(|(bad, entry)| (bad == prime).then_some(entry)),
        })
    }
}

impl<const N: usize> PrimeBlackBox for KernelPrime<'_, N> {
    fn probe(&self, xs: &[u64]) -> Vec<ProbeResult> {
        let field = self.compiled.field();
        let free = self.owner.request.free().len();
        let points: Vec<Zp> = xs.iter().map(|&x| field.to_element(x)).collect();
        let system = &self.owner.request.system;
        let results = match self.owner.bunch {
            1 => self.compiled.probe_points::<1, N>(system, &points, free),
            2 => self.compiled.probe_points::<2, N>(system, &points, free),
            4 => self.compiled.probe_points::<4, N>(system, &points, free),
            16 => self.compiled.probe_points::<16, N>(system, &points, free),
            _ => self.compiled.probe_points::<8, N>(system, &points, free),
        };
        let mut unlucky = self.owner.unlucky.lock().expect("unlucky lock");
        results
            .into_iter()
            .map(|result| {
                // Injected discards hit lanes that would have succeeded, so
                // genuine failures are still detected and counted.
                let discard = result.is_ok()
                    && self
                        .owner
                        .discard
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
                        .is_ok();
                match result {
                    Ok(_) if discard => {
                        *unlucky.entry("injected".to_string()).or_default() += 1;
                        Err(ProbeFailure("injected"))
                    }
                    Ok(mut values) => {
                        if let Some(entry) = self.corrupt {
                            values[entry] = field.add(&values[entry], &values[entry]);
                        }
                        Ok(values
                            .iter()
                            .map(|value| field.from_element(value))
                            .collect())
                    }
                    Err(failure) => {
                        *unlucky.entry(failure.label().to_string()).or_default() += 1;
                        Err(ProbeFailure(failure.label()))
                    }
                }
            })
            .collect()
    }
}

/// The reference probe as the verification black box.
struct ReferenceBox<'a, const N: usize> {
    request: &'a LaneRequest<N>,
    unlucky: std::sync::Mutex<BTreeMap<String, usize>>,
}

struct ReferencePrime<'a, const N: usize> {
    owner: &'a ReferenceBox<'a, N>,
    field: Zp64,
    fixed: Vec<Option<Zp>>,
}

impl<'a, const N: usize> BlackBox for &'a ReferenceBox<'a, N> {
    type Prime = ReferencePrime<'a, N>;

    fn outputs(&self) -> usize {
        self.request.entries()
    }

    fn at_prime(&self, prime: u64) -> Option<ReferencePrime<'a, N>> {
        let field = Zp64::new(prime);
        let mut fixed = Vec::with_capacity(self.request.fixed.len());
        for value in &self.request.fixed {
            fixed.push(match value {
                Some(value) => Some(rational_image(value, &field)?),
                None => None,
            });
        }
        let &owner = self;
        Some(ReferencePrime {
            owner,
            field,
            fixed,
        })
    }
}

impl<const N: usize> PrimeBlackBox for ReferencePrime<'_, N> {
    fn probe(&self, xs: &[u64]) -> Vec<ProbeResult> {
        xs.iter()
            .map(|&x| {
                let x = self.field.to_element(x);
                let point: Vec<Zp> = self.fixed.iter().map(|value| value.unwrap_or(x)).collect();
                match reference_probe(self.owner.request, &self.field, &point) {
                    Ok(values) => Ok(values
                        .iter()
                        .map(|value| self.field.from_element(value))
                        .collect()),
                    Err(failure) => {
                        *self
                            .owner
                            .unlucky
                            .lock()
                            .expect("unlucky lock")
                            .entry(format!("verify-{}", failure.label()))
                            .or_default() += 1;
                        Err(ProbeFailure(failure.label()))
                    }
                }
            })
            .collect()
    }
}

fn reconstruction_error(error: ReconstructionError) -> SolverError {
    match error {
        ReconstructionError::VerificationPoints { .. } => SolverError::UnluckyBoxSample {
            phase: BoxPhase::Replay,
        },
        ReconstructionError::InvalidOptions(message) => {
            SolverError::InvalidInput(format!("finite-field options: {message}"))
        }
        other => SolverError::Certification(format!("finite-field reconstruction: {other}")),
    }
}

/// Univariate reconstruction of every `(target, master)` coefficient in
/// the single free parameter. Must run inside the session's Rayon pool.
pub(crate) fn reconstruct<const N: usize>(
    request: &LaneRequest<N>,
    options: &FfOptions,
    workers: usize,
    session_key: &[u8],
) -> Result<(Vec<RationalFunction>, FfReport), SolverError> {
    let started = std::time::Instant::now();
    let free = request.free();
    if free.len() != 1 {
        return Err(SolverError::InvalidInput(format!(
            "the univariate finite-field lane needs exactly one free parameter, got {}",
            free.len()
        )));
    }
    if ![1, 2, 4, 8, 16].contains(&options.bunch) {
        return Err(SolverError::InvalidInput(format!(
            "finite-field bunch size {} is not one of 1, 2, 4, 8, 16",
            options.bunch
        )));
    }
    if options.verify_points == 0 || options.max_verifications == 0 {
        return Err(SolverError::InvalidInput(
            "the finite-field lane verifies at a fresh prime: verify_points and max_verifications must be positive".into(),
        ));
    }
    let fault = options.fault.clone().unwrap_or_default();
    // The prime stream of `reconstruct_univariate` (each prime used once,
    // reconstruction first, verification later), materialised here so that
    // fault injection can name a prime by its position.
    let primes = reconstruction_primes(options.max_primes + 8 * options.max_verifications + 8);
    if let Some((ordinal, entry)) = fault.corrupt_prime
        && (ordinal >= primes.len() || entry >= request.entries())
    {
        return Err(SolverError::InvalidInput(format!(
            "fault injection: no prime {ordinal} or entry {entry} to corrupt"
        )));
    }
    let kernel = KernelBox {
        request,
        bunch: options.bunch,
        corrupt: fault
            .corrupt_prime
            .and_then(|(ordinal, entry)| primes.get(ordinal).map(|&prime| (prime, entry))),
        discard: AtomicUsize::new(fault.discard_lanes),
        unlucky: Default::default(),
    };
    let reference = ReferenceBox {
        request,
        unlucky: Default::default(),
    };
    let reconstruct_options = UnivariateOptions {
        confirmations: options.confirmations,
        bunch: options.bunch,
        parallel_bunches: workers.max(1),
        max_points_per_prime: options.max_points_per_prime,
        max_primes: options.max_primes,
        // One extra point checks every known-shape fit.
        check_points: 1,
        verify_points: options.verify_points,
        max_verifications: options.max_verifications,
        seed: options.seed,
        primes: Some(primes),
        forced_points: fault.special_points.clone(),
        groups: options.denominator_first.then(|| {
            let masters = request.system.masters.len().max(1);
            (0..request.entries())
                .map(|entry| entry / masters)
                .collect()
        }),
    };
    let key = request.key(session_key, options.seed, options.fault.as_ref());
    let mut log = match &options.checkpoint_dir {
        Some(dir) => Some(
            FileProbeLog::new(dir, key, request.entries())
                .map_err(|error| SolverError::InvalidInput(format!("checkpoint dir: {error}")))?,
        ),
        None => None,
    };
    let result = reconstruct_univariate(
        &&kernel,
        &&reference,
        &reconstruct_options,
        log.as_mut().map(|log| log as &mut dyn ProbeLog),
    )
    .map_err(reconstruction_error)?;
    let mut unlucky = kernel.unlucky.into_inner().expect("unlucky lock");
    for (reason, count) in reference.unlucky.into_inner().expect("unlucky lock") {
        *unlucky.entry(reason).or_default() += count;
    }
    let report = FfReport {
        rows: request.system.rows.len(),
        terms: request.system.terms(),
        columns: request.system.columns.len(),
        targets: request.targets.len(),
        masters: request.system.masters.len(),
        entries: request.entries(),
        nonzero_entries: result.functions.iter().filter(|f| !f.is_zero()).count(),
        unlucky,
        reconstruction: result.report,
        resumed_probes: log.as_ref().map_or(0, |log| log.loaded),
        build_seconds: 0.0,
        seconds: started.elapsed().as_secs_f64(),
    };
    // The reconstruction's contract: one function per (target, master).
    if result.functions.len() != request.entries() {
        return Err(SolverError::Certification(format!(
            "finite-field reconstruction returned {} functions for {} entries",
            result.functions.len(),
            request.entries()
        )));
    }
    Ok((result.functions, report))
}
