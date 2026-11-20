//! Image source of `ModularSampled`: numeric replays of the recorded trace at
//! sample points, and per-prime Thiele interpolation of every terminal.
//!
//! The terminal key set and the guards are fixed by the single recording
//! pass, so the structure of every image is `()` and only the degrees vary
//! between primes. Sampling is sized by need:
//!
//! - the first usable prime discovers the degrees adaptively: half of
//!   `batch_points` points, then growing the total by a quarter (at least an
//!   eighth of `batch_points`) until every terminal's
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

use super::compose::{PrimeProgram, Program};

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
pub(super) struct Thiele {
    nodes: Vec<Fe>,
    differences: Vec<Fe>,
    p_prev: Poly,
    q_prev: Poly,
    p: Poly,
    q: Poly,
    checks: usize,
    consumed: usize,
    pub(super) result: Option<(Poly, Poly)>,
}

impl Thiele {
    pub(super) fn new(f: &Zp64) -> Self {
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
    pub(super) fn feed(&mut self, f: &Zp64, xs: &[Fe], ys: &[Fe], required: usize) -> bool {
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

/// Values of all output terminals (or composed outputs) at the good points
/// of one prime.
struct PrimeSamples<'p> {
    field: Zp64,
    xs: Vec<Fe>,
    ys: Vec<Vec<Fe>>,
    used: HashSet<u64>,
    rng: StdRng,
    /// Points drawn, including bad ones.
    drawn: usize,
    /// The terminal composition modulo this prime, when composing.
    program: Option<Arc<PrimeProgram<'p>>>,
    /// The prime reduces a composition coefficient's denominator to zero.
    unusable: bool,
}

impl<'p> PrimeSamples<'p> {
    fn new(prime: u64, seed: u64, terminals: usize, compose: Option<&'p Program>) -> Self {
        let field = Zp64::new(prime);
        let (program, unusable) = match compose.map(|program| program.reduce(&field)) {
            None => (None, false),
            Some(Some(program)) => (Some(Arc::new(program)), false),
            Some(None) => (None, true),
        };
        Self {
            field,
            xs: vec![],
            ys: vec![vec![]; terminals],
            used: HashSet::new(),
            rng: StdRng::seed_from_u64(seed ^ prime.rotate_left(17)),
            drawn: 0,
            program,
            unusable,
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
        export_batch(&self.field, points, batch, slots);
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

/// Astra A/B (A): `RUSTRED_SAMPLED_EXPORT=<file>` appends one JSON line per
/// replay batch: prime, points, finite mask, the cancellation checks of the
/// batch and, per output terminal, its values at the points (u64 residues).
fn export_batch(field: &Zp64, points: &[Fe], batch: &ReplayBatch, slots: &[usize]) {
    let Some(path) = std::env::var_os("RUSTRED_SAMPLED_EXPORT") else {
        return;
    };
    let width = points.len();
    let to = |x: &Fe| super::field::to_u64(field, x);
    let line = serde_json::json!({
        "prime": field.get_prime(),
        "points": points.iter().map(to).collect::<Vec<_>>(),
        "finite": batch.good,
        "cancel_checks": batch.cancel_checks,
        "values": slots
            .iter()
            .map(|&slot| (0..width).map(|j| to(&batch.values[slot * width + j])).collect::<Vec<_>>())
            .collect::<Vec<_>>(),
    });
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

/// The first `count` reconstruction points drawn at `prime` (test hook).
#[cfg(test)]
pub(super) fn first_points(seed: u64, prime: u64, count: usize) -> Vec<u64> {
    let mut samples = PrimeSamples::new(prime, seed, 0, None);
    let field = samples.field.clone();
    samples
        .fresh_points(count)
        .iter()
        .map(|x| super::field::to_u64(&field, x))
        .collect()
}

/// One replay batch: points of the prime `owner` of the round.
struct Job<'p> {
    owner: usize,
    field: Zp64,
    points: Vec<Fe>,
    /// The terminal composition modulo the job's prime, when composing.
    program: Option<Arc<PrimeProgram<'p>>>,
}

/// Narrowest batch worth replaying: per-operation overheads (one frontier
/// row per trace operation) dominate below an eighth of `batch_points`.
fn min_width(options: &SampledOptions) -> usize {
    // Astra A/B (E): never below the 8-point SIMD packet (a width-8 pass
    // costs about the wall of a width-2 pass), unless the batch itself is
    // narrower.
    (options.batch_points / 8)
        .max(SIMD_POINTS.min(options.batch_points))
        .max(1)
}

/// `RUSTRED_SAMPLED_FIRST` (0 / unset: half of batch_points).
fn first_round() -> usize {
    static N: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *N.get_or_init(|| {
        std::env::var("RUSTRED_SAMPLED_FIRST")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    })
}

/// The points one vector instruction of the replay kernel carries.
const SIMD_POINTS: usize = 8;

/// Astra A/B (E): `RUSTRED_SAMPLED_WIDE=<points>`: when the first round of the
/// first prime does not converge, the next round draws at least this many
/// points (wide speculation; cheap low-degree keys converge in the first round
/// and never pay it). 0 / unset: the quarter growth only.
fn wide_floor() -> usize {
    static N: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *N.get_or_init(|| {
        std::env::var("RUSTRED_SAMPLED_WIDE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    })
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

/// Resumable sampled replay: every completed replay batch of a request with a
/// resume directory is appended to `<dir>/samples.jsonl` (prime, points,
/// finite mask, cancellation checks, the value of every output column), each
/// line bound to the request's final saved state (`binding`: BLAKE3 of the
/// state's own digest and the seed) and carrying its own BLAKE3. A restarted
/// replay re-imports the points instead of replaying them: a job whose points
/// are all stored is not replayed; the values enter the interpolation exactly
/// as fresh ones (and the lift's verification prime stays). A line of another
/// binding, or an altered line, is refused (hard error naming the file).
///
/// Failure handling (third gate): a write or fsync error of the store stops
/// the replay loudly, naming the file, with the store and the saved state
/// kept (a restart re-imports what was stored); so does a low disk
/// (`RUSTRED_RESUME_DISK_MIN_GB`, default 50, or the `RUSTRED_DISK_LOW_FLAG`
/// file) before a batch is appended. A final line without its newline (a
/// torn write) is dropped at open, logged, and cut from the file.
///
/// Limits: re-import is per point, but a replay job is re-imported only when
/// ALL its points are stored, so a restart with another batch size or thread
/// count replays the partially stored jobs in full. The line digests are
/// integrity checks against damage, not authentication: a forged value with
/// a recomputed digest is used (the lift's verification prime still checks
/// the result).
pub(super) struct PointStore {
    path: std::path::PathBuf,
    binding: String,
    /// (prime, x) -> (finite, value per output column).
    points: std::collections::HashMap<(u64, u64), (bool, Vec<u64>)>,
    pub(super) imported: usize,
    /// Batches appended by this process (test hook).
    appended: usize,
    /// Low-disk rule, read at open: minimum free GB and the flag file.
    disk_min_gb: u64,
    disk_flag: Option<std::path::PathBuf>,
}

impl PointStore {
    pub(super) fn open(dir: &std::path::Path, binding: [u8; 32]) -> Result<Self, ReductionError> {
        let path = dir.join("samples.jsonl");
        let binding = blake3::Hash::from_bytes(binding).to_hex().to_string();
        let mut points = std::collections::HashMap::new();
        // A missing store is empty; any other read error is refused (a store
        // that exists but cannot be read is never taken as empty).
        let read = match std::fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                let message = format!("sample store {}: cannot be read: {e}", path.display());
                super::super::resume::storage_stop(&message);
                return Err(super::error(message));
            }
        };
        if let Some(bytes) = read {
            // A torn tail (a short write: the final line lacks its newline)
            // is dropped and cut from the file before new lines are appended.
            let complete = bytes.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
            if complete < bytes.len() {
                eprintln!(
                    "rustred replay: sample store {}: dropped a torn final line ({} bytes without a newline)",
                    path.display(),
                    bytes.len() - complete
                );
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(&path)
                    .and_then(|file| {
                        file.set_len(complete as u64)?;
                        file.sync_data()
                    })
                    .map_err(|e| {
                        let message = format!(
                            "sample store {}: cannot cut its torn final line: {e}",
                            path.display()
                        );
                        super::super::resume::storage_stop(&message);
                        super::error(message)
                    })?;
            }
            let text = String::from_utf8_lossy(&bytes[..complete]);
            for (n, line) in text.lines().enumerate() {
                let refuse = |why: &str| {
                    super::error(format!(
                        "sample store {} refused at line {}: {why}",
                        path.display(),
                        n + 1
                    ))
                };
                let Some((body, digest)) = line.rsplit_once("\t") else {
                    return Err(refuse("no line digest"));
                };
                if blake3::hash(body.as_bytes()).to_hex().as_str() != digest {
                    return Err(refuse("altered (line digest mismatch)"));
                }
                let record: serde_json::Value =
                    serde_json::from_str(body).map_err(|e| refuse(&e.to_string()))?;
                if record["binding"].as_str() != Some(binding.as_str()) {
                    return Err(refuse("written for another record (binding mismatch)"));
                }
                let prime = record["prime"].as_u64().ok_or_else(|| refuse("prime"))?;
                let xs: Vec<u64> = serde_json::from_value(record["points"].clone())
                    .map_err(|e| refuse(&e.to_string()))?;
                let finite: Vec<bool> = serde_json::from_value(record["finite"].clone())
                    .map_err(|e| refuse(&e.to_string()))?;
                let values: Vec<Vec<u64>> = serde_json::from_value(record["values"].clone())
                    .map_err(|e| refuse(&e.to_string()))?;
                for (j, &x) in xs.iter().enumerate() {
                    let column: Vec<u64> = values.iter().map(|row| row[j]).collect();
                    points.insert((prime, x), (finite[j], column));
                }
            }
        }
        Ok(Self {
            path,
            binding,
            points,
            imported: 0,
            appended: 0,
            disk_min_gb: std::env::var("RUSTRED_RESUME_DISK_MIN_GB")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50),
            disk_flag: std::env::var_os("RUSTRED_DISK_LOW_FLAG").map(std::path::PathBuf::from),
        })
    }

    /// The batch of `points` if every point is stored.
    fn lookup(&mut self, field: &Zp64, points: &[Fe], columns: &[usize]) -> Option<ReplayBatch> {
        let prime = field.get_prime();
        let width = points.len();
        let rows = columns.iter().max().map_or(0, |m| m + 1);
        let mut values = vec![field.zero(); rows * width];
        let mut good = vec![true; width];
        for (j, x) in points.iter().enumerate() {
            let (finite, column) = self.points.get(&(prime, field::to_u64(field, x)))?;
            good[j] = *finite;
            for (&slot, &value) in columns.iter().zip(column) {
                values[slot * width + j] = field.to_element(value);
            }
        }
        self.imported += width;
        Some(ReplayBatch {
            values,
            good,
            cancel_checks: 0,
        })
    }

    fn append(
        &mut self,
        field: &Zp64,
        points: &[Fe],
        batch: &ReplayBatch,
        columns: &[usize],
    ) -> Result<(), ReductionError> {
        if let Some(dir) = self.path.parent()
            && store_disk_low(dir, self.disk_min_gb, self.disk_flag.as_deref())
        {
            let message = format!(
                "sample store {}: stopped on low disk before appending (RUSTRED_RESUME_DISK_MIN_GB / \
                 RUSTRED_DISK_LOW_FLAG); the store and the saved state are kept, restart to resume",
                self.path.display()
            );
            super::super::resume::storage_stop(&message);
            return Err(super::error(message));
        }
        let width = points.len();
        let prime = field.get_prime();
        let xs: Vec<u64> = points.iter().map(|x| field::to_u64(field, x)).collect();
        let values: Vec<Vec<u64>> = columns
            .iter()
            .map(|&slot| {
                (0..width)
                    .map(|j| field::to_u64(field, &batch.values[slot * width + j]))
                    .collect()
            })
            .collect();
        let body = serde_json::json!({
            "binding": self.binding, "prime": prime, "points": xs, "finite": batch.good,
            "cancel_checks": batch.cancel_checks, "values": values,
        })
        .to_string();
        for (j, &x) in xs.iter().enumerate() {
            self.points.insert(
                (prime, x),
                (batch.good[j], values.iter().map(|row| row[j]).collect()),
            );
        }
        let line = format!("{body}\t{}\n", blake3::hash(body.as_bytes()).to_hex());
        use std::io::Write;
        let created = !self.path.exists();
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .and_then(|mut file| {
                file.write_all(line.as_bytes())?;
                file.sync_data()?;
                // a new store's directory entry is made durable too
                if created && let Some(dir) = self.path.parent() {
                    std::fs::File::open(dir)?.sync_all()?;
                }
                Ok(())
            })
            .map_err(|e| {
                let message = format!(
                    "sample store {}: write failed: {e}; the store and the saved state are kept \
                     (a torn final line is dropped at restart), restart to resume",
                    self.path.display()
                );
                super::super::resume::storage_stop(&message);
                super::error(message)
            })
    }
}

/// The store's filesystem has less than `RUSTRED_RESUME_DISK_MIN_GB` (default
/// 50) GB available, or `RUSTRED_DISK_LOW_FLAG` names an existing file (the
/// same rule as memfix's `resume::disk_low`; to be merged into it).
fn store_disk_low(dir: &std::path::Path, min_gb: u64, flag: Option<&std::path::Path>) -> bool {
    if flag.is_some_and(std::path::Path::exists) {
        return true;
    }
    let Ok(path) = std::ffi::CString::new(dir.as_os_str().as_encoded_bytes()) else {
        return false;
    };
    // SAFETY: statvfs on a NUL-terminated path into a zeroed struct.
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(path.as_ptr(), &mut stat) } != 0 {
        return false;
    }
    (stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64) < min_gb.saturating_mul(1 << 30)
}

pub(super) struct SampledSource<'a> {
    /// Resumable replay (requests with a resume directory).
    store: Option<std::sync::Mutex<PointStore>>,
    trace: &'a Trace,
    /// Terminal accumulator slot of every output terminal.
    slots: &'a [usize],
    structure: Arc<()>,
    options: SampledOptions,
    seed: u64,
    /// Points per prime once the first image fixed the degrees.
    needed: Option<usize>,
    /// Terminal composition applied at every sample point before the
    /// interpolation: the outputs are then the composed outputs.
    compose: Option<&'a Program>,
    /// `0..outputs`: the value rows of composed replay batches.
    identity: Vec<usize>,
}

impl<'a> SampledSource<'a> {
    pub fn new(
        trace: &'a Trace,
        slots: &'a [usize],
        options: SampledOptions,
        seed: u64,
        compose: Option<&'a Program>,
        store: Option<PointStore>,
    ) -> Self {
        Self {
            store: store.map(std::sync::Mutex::new),
            trace,
            slots,
            structure: Arc::new(()),
            options,
            seed,
            needed: None,
            compose,
            identity: compose.map_or(vec![], |program| (0..program.outputs()).collect()),
        }
    }

    /// Interpolated outputs: the terminals, or the composed outputs.
    fn outputs(&self) -> usize {
        self.compose.map_or(self.slots.len(), Program::outputs)
    }

    /// Row of every output in the value tables of the replay batches.
    fn columns(&self) -> &[usize] {
        if self.compose.is_some() {
            &self.identity
        } else {
            self.slots
        }
    }

    fn prime_samples(&self, prime: u64) -> PrimeSamples<'a> {
        PrimeSamples::new(prime, self.seed, self.outputs(), self.compose)
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
    /// Composing jobs return the composed outputs' values (rows `0..outputs`)
    /// and mark the poles of composition coefficients as bad points.
    fn replay(
        &self,
        jobs: Vec<Job<'a>>,
        executor: &Executor,
        statistics: &mut ModularStatistics,
    ) -> Result<Vec<(usize, Vec<Fe>, Option<ReplayBatch>)>, ReductionError> {
        let started = Instant::now();
        let trace = self.trace;
        let slots = self.slots;
        let results = executor.map(jobs, |job| {
            let begun = Instant::now();
            let mut result = replay(trace, &job.field, &job.points);
            let replayed = begun.elapsed().as_secs_f64();
            if let (Ok(batch), Some(program)) = (&mut result, &job.program) {
                let width = job.points.len();
                let values = &batch.values;
                let composed = program.apply(
                    &job.points,
                    |t| &values[slots[t] * width..(slots[t] + 1) * width],
                    &mut batch.good,
                );
                batch.values = composed;
            }
            let composed = begun.elapsed().as_secs_f64() - replayed;
            (job.owner, job.points, result, replayed, composed)
        })?;
        statistics.replay_seconds += started.elapsed().as_secs_f64();
        let mut output = Vec::with_capacity(results.len());
        for (owner, points, result, cpu, composed) in results {
            statistics.replay_cpu_seconds += cpu;
            statistics.composition_cpu_seconds += composed;
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
                Err(ReplayError::Stopped(stopped)) => return Err(stopped.into()),
            }
        }
        Ok(output)
    }

    /// Sample every prime of `requests` (`(prime index, points)`), feed the
    /// interpolations and return the primes whose interpolation is not done.
    fn sample_round(
        &self,
        samples: &mut [PrimeSamples<'a>],
        states: &mut [Thiele],
        usable: &mut [bool],
        requests: &[(usize, usize)],
        executor: &Executor,
        statistics: &mut ModularStatistics,
    ) -> Result<Vec<usize>, ReductionError> {
        let terminals = self.outputs();
        let mut jobs = vec![];
        for &(owner, count) in requests {
            if samples[owner].unusable {
                usable[owner] = false;
                continue;
            }
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
                    program: samples[owner].program.clone(),
                });
            }
        }
        // Resumable replay: stored batches are re-imported, not replayed.
        let mut imported = vec![];
        if let Some(store) = &self.store {
            let mut store = store
                .lock()
                .map_err(|_| super::error("sample store poisoned"))?;
            let columns = self.columns();
            jobs.retain(|job| match store.lookup(&job.field, &job.points, columns) {
                Some(batch) => {
                    imported.push((job.owner, job.points.clone(), Some(batch)));
                    false
                }
                None => true,
            });
        }
        let mut replayed = self.replay(jobs, executor, statistics)?;
        if let Some(store) = &self.store {
            let mut store = store
                .lock()
                .map_err(|_| super::error("sample store poisoned"))?;
            for (owner, points, batch) in &replayed {
                if let Some(batch) = batch {
                    store.append(&samples[*owner].field, points, batch, self.columns())?;
                    store.appended += 1;
                }
            }
            // Test hook: a stop after `n` newly stored batches (as a memory
            // bound or a host stop would), the store kept for the restart.
            if let Some(n) = std::env::var("RUSTRED_SAMPLED_STOP_AFTER")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                && store.appended >= n
            {
                return Err(super::error(format!(
                    "stopped after {} stored replay batches (RUSTRED_SAMPLED_STOP_AFTER)",
                    store.appended
                )));
            }
        }
        if !imported.is_empty() {
            eprintln!(
                "rustred replay: {} stored batches ({} points) re-imported, {} replayed",
                imported.len(),
                imported
                    .iter()
                    .map(|(_, points, _)| points.len())
                    .sum::<usize>(),
                replayed.len()
            );
        }
        imported.append(&mut replayed);
        for (owner, points, batch) in imported {
            match batch {
                Some(batch) if usable[owner] => {
                    samples[owner].absorb(&points, &batch, self.columns(), statistics)
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
        })?;
        statistics.interpolation_seconds += interpolation.elapsed().as_secs_f64();
        Ok((0..samples.len())
            .filter(|&owner| {
                active[owner] && done[owner * terminals..(owner + 1) * terminals].contains(&false)
            })
            .collect())
    }

    fn images_of(
        &self,
        samples: &[PrimeSamples<'a>],
        states: Vec<Thiele>,
        usable: &[bool],
    ) -> Vec<Option<PrimeImages<()>>> {
        let terminals = self.outputs();
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
        let terminals = self.outputs();
        let mut samples = [self.prime_samples(prime)];
        if samples[0].unusable {
            return Ok(None);
        }
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
        // RUSTRED_SAMPLED_FIRST=<points>: an explicitly wide first round for a
        // key known to be hard (T241: ~120 points on the first prime).
        let start = start.max(first_round());
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
            // Grow by a quarter of the points drawn so far (at least an eighth
            // of a batch), within the budget: a terminal that needs a few
            // points more than the first batch is not replayed at twice its
            // need (vk5l-physrun: 828's channels need ~69 points, doubling
            // replayed 128). The points come from the same seeded generator
            // in the same order, so the converged interpolants are the same.
            // Row E: the wide speculation replaces only the SECOND round (the
            // first one did not converge: the key is not cheap); later rounds
            // grow by a quarter again, so the overshoot stays bounded.
            let wide = if samples[0].drawn == start.min(self.options.max_points_per_prime) {
                wide_floor()
            } else {
                0
            };
            count = (samples[0].drawn / 4)
                .max(self.options.batch_points / 8)
                .max(wide)
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
        let terminals = self.outputs();
        let needed = self.needed.expect("degrees discovered");
        let mut samples: Vec<PrimeSamples<'a>> = primes
            .iter()
            .map(|prime| self.prime_samples(*prime))
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

impl<'a> ImageSource for SampledSource<'a> {
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
        let mut unusable = vec![false; primes.len()];
        let jobs: Vec<Job<'a>> = primes
            .iter()
            .enumerate()
            .filter_map(|(owner, &prime)| {
                let field = Zp64::new(prime);
                let program = match self.compose.map(|program| program.reduce(&field)) {
                    None => None,
                    Some(Some(program)) => Some(Arc::new(program)),
                    Some(None) => {
                        unusable[owner] = true;
                        return None;
                    }
                };
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
                Some(Job {
                    owner,
                    field,
                    points,
                    program,
                })
            })
            .collect();
        let slots = self.columns();
        let mut outcomes: Vec<Option<Verification<()>>> = unusable
            .iter()
            .map(|&unusable| unusable.then_some(Verification::Unusable))
            .collect();
        for (owner, points, batch) in self.replay(jobs, executor, statistics)? {
            let Some(batch) = batch else {
                outcomes[owner] = Some(Verification::Unusable);
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
                })?;
            if good.len() < count || checked.iter().any(Option::is_none) {
                outcomes[owner] = Some(Verification::Unusable);
                continue;
            }
            outcomes[owner] = Some(Verification::Checked {
                disagreeing: checked
                    .iter()
                    .enumerate()
                    .filter(|(_, agrees)| **agrees == Some(false))
                    .map(|(t, _)| t)
                    .collect(),
            });
        }
        outcomes
            .into_iter()
            .map(|outcome| outcome.ok_or_else(|| error("a verification prime has no outcome")))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("rr-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn one_batch(field: &Zp64) -> (Vec<Fe>, ReplayBatch) {
        let points: Vec<Fe> = (3..7).map(|x| field.to_element(x)).collect();
        let values = (0..2 * points.len())
            .map(|v| field.to_element(v as u64 + 11))
            .collect();
        let batch = ReplayBatch {
            values,
            good: vec![true; points.len()],
            cancel_checks: 0,
        };
        (points, batch)
    }

    /// Third gate (a): ENOSPC on the store's write is a loud error naming the
    /// file (the store is `/dev/full`).
    #[test]
    fn a_full_disk_stops_the_store_loudly() {
        let _serial = super::super::super::resume::STORAGE_STOP_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = super::super::super::resume::take_storage_stop();
        let dir = store_dir("enospc");
        let mut store = PointStore::open(&dir, [7; 32]).unwrap();
        // Writes to /dev/full fail with ENOSPC (opened only for the append:
        // reading it never ends).
        std::os::unix::fs::symlink("/dev/full", dir.join("samples.jsonl")).unwrap();
        store.disk_min_gb = 0;
        store.disk_flag = None;
        let field = Zp64::new(4611686018427387847);
        let (points, batch) = one_batch(&field);
        let error = store
            .append(&field, &points, &batch, &[0, 1])
            .expect_err("ENOSPC must be an error")
            .to_string();
        assert!(
            error.contains("samples.jsonl") && error.contains("write failed"),
            "{error}"
        );
        // R-B2: a storage stop (reduce-files keeps the chunk retryable)
        assert!(
            super::super::super::resume::take_storage_stop()
                .is_some_and(|s| s.contains("samples.jsonl"))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Gate note: a store that exists but cannot be read is refused at open,
    /// never taken as empty (here `samples.jsonl` is a directory).
    #[test]
    fn an_unreadable_store_is_refused_not_empty() {
        let _serial = super::super::super::resume::STORAGE_STOP_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = super::super::super::resume::take_storage_stop();
        let dir = store_dir("unreadable");
        std::fs::create_dir(dir.join("samples.jsonl")).unwrap();
        let error = PointStore::open(&dir, [7; 32])
            .err()
            .expect("an unreadable store must be refused")
            .to_string();
        assert!(error.contains("cannot be read"), "{error}");
        // R-B2: a storage stop (reduce-files keeps the chunk retryable)
        assert!(
            super::super::super::resume::take_storage_stop()
                .is_some_and(|s| s.contains("samples.jsonl"))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Third gate (b): a torn final line is dropped (and cut), the complete
    /// lines are kept, and later appends start on a clean line.
    #[test]
    fn a_torn_final_line_is_dropped_not_refused() {
        let dir = store_dir("torn");
        let field = Zp64::new(4611686018427387847);
        let (points, batch) = one_batch(&field);
        let mut store = PointStore::open(&dir, [7; 32]).unwrap();
        (store.disk_min_gb, store.disk_flag) = (0, None);
        store.append(&field, &points, &batch, &[0, 1]).unwrap();
        let path = dir.join("samples.jsonl");
        let whole = std::fs::read(&path).unwrap();
        // A short write of a second line.
        let mut torn = whole.clone();
        torn.extend_from_slice(&whole[..whole.len() / 2]);
        std::fs::write(&path, &torn).unwrap();
        let mut store = PointStore::open(&dir, [7; 32]).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), whole, "torn tail cut");
        assert!(
            store.lookup(&field, &points, &[0, 1]).is_some(),
            "complete line kept"
        );
        let other: Vec<Fe> = (8..12).map(|x| field.to_element(x)).collect();
        (store.disk_min_gb, store.disk_flag) = (0, None);
        store.append(&field, &other, &batch, &[0, 1]).unwrap();
        let mut store = PointStore::open(&dir, [7; 32]).unwrap();
        assert!(store.lookup(&field, &other, &[0, 1]).is_some());
        assert!(store.lookup(&field, &points, &[0, 1]).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The low-disk flag stops the store before it appends.
    #[test]
    fn a_low_disk_flag_stops_the_store_before_appending() {
        let _serial = super::super::super::resume::STORAGE_STOP_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = super::super::super::resume::take_storage_stop();
        let dir = store_dir("lowdisk");
        let flag = dir.join("DISK_LOW");
        std::fs::write(&flag, "").unwrap();
        let field = Zp64::new(4611686018427387847);
        let (points, batch) = one_batch(&field);
        let mut store = PointStore::open(&dir, [7; 32]).unwrap();
        store.disk_flag = Some(flag);
        let error = store
            .append(&field, &points, &batch, &[0, 1])
            .expect_err("low disk must stop")
            .to_string();
        assert!(error.contains("low disk"), "{error}");
        // R-B2: a storage stop (reduce-files keeps the chunk retryable)
        assert!(
            super::super::super::resume::take_storage_stop()
                .is_some_and(|s| s.contains("samples.jsonl"))
        );
        assert!(!dir.join("samples.jsonl").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

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
