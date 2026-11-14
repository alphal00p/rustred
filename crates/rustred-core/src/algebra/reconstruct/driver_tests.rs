//! Property tests of [`reconstruct_univariate`] over a black box of random
//! rational functions (degrees up to 170/170) with poles at probe points:
//! exact results, verification at a prime outside the reconstruction
//! primes, identical results and reports for 1 and 4 Rayon threads, and an
//! interrupted run resumed from its probe log giving the identical result.

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};

use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::RingOps;
use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;
use symbolica::prelude::Ring;

use super::checkpoint::FileProbeLog;
use super::driver::{
    BlackBox, PrimeBlackBox, ProbeFailure, ProbeLog, ProbeResult, Reconstruction,
    UnivariateOptions, reconstruct_univariate,
};
use super::rational::RationalFunction;

fn q(value: i64) -> Rational {
    Rational::from(Integer::from(value))
}

/// Random functions with integer coefficients in `[-1000, 1000]`; every
/// denominator also vanishes at `x = 3` mod any prime (a pole at a probe
/// point is an unlucky point the driver must discard).
struct Random {
    functions: Vec<RationalFunction>,
    calls: AtomicUsize,
}

impl Random {
    fn new(seed: u64, degrees: &[(usize, usize)]) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut coefficient = |nonzero: bool| loop {
            let value = rng.random_range(-1000i64..=1000);
            if value != 0 || !nonzero {
                break q(value);
            }
        };
        let functions = degrees
            .iter()
            .map(|&(n, d)| {
                let numerator: Vec<Rational> = (0..=n).map(|i| coefficient(i == n)).collect();
                // (x - 3) * (random polynomial of degree d - 1, constant != 0).
                let mut denominator = vec![Rational::one()];
                if d > 0 {
                    let base: Vec<Rational> =
                        (0..d).map(|i| coefficient(i == 0 || i == d - 1)).collect();
                    denominator = vec![Rational::zero(); d + 1];
                    for (i, c) in base.iter().enumerate() {
                        denominator[i] = &denominator[i] - &(&q(3) * c);
                        denominator[i + 1] = &denominator[i + 1] + c;
                    }
                }
                RationalFunction {
                    numerator,
                    denominator,
                }
            })
            .collect();
        Self {
            functions,
            calls: AtomicUsize::new(0),
        }
    }
}

struct RandomPrime<'a> {
    field: Zp64,
    images: Vec<(Vec<u64>, Vec<u64>)>,
    calls: &'a AtomicUsize,
}

impl<'a> BlackBox for &'a Random {
    type Prime = RandomPrime<'a>;
    fn outputs(&self) -> usize {
        self.functions.len()
    }
    fn at_prime(&self, prime: u64) -> Option<RandomPrime<'a>> {
        let field = Zp64::new(prime);
        let images = self
            .functions
            .iter()
            .map(|f| {
                let (n, d) = f.image(&field).expect("integer coefficients");
                let raw = |p: &[super::Zp]| -> Vec<u64> {
                    p.iter().map(|c| field.from_element(c)).collect()
                };
                (raw(&n), raw(&d))
            })
            .collect();
        Some(RandomPrime {
            field,
            images,
            calls: &self.calls,
        })
    }
}

impl PrimeBlackBox for RandomPrime<'_> {
    fn probe(&self, xs: &[u64]) -> Vec<ProbeResult> {
        self.calls.fetch_add(xs.len(), Ordering::Relaxed);
        let field = &self.field;
        let horner = |coefficients: &[u64], x: &_| {
            let mut value = field.zero();
            for c in coefficients.iter().rev() {
                value = field.add(&field.mul(&value, x), &field.to_element(*c));
            }
            value
        };
        xs.iter()
            .map(|&x| {
                let x = field.to_element(x);
                self.images
                    .iter()
                    .map(|(n, d)| {
                        let inverse = field.try_inv(&horner(d, &x)).ok_or(ProbeFailure("pole"))?;
                        Ok(field.from_element(&field.mul(&horner(n, &x), &inverse)))
                    })
                    .collect()
            })
            .collect()
    }
}

const DEGREES: &[(usize, usize)] = &[(0, 0), (3, 0), (0, 2), (12, 9), (60, 61), (170, 170)];

fn options() -> UnivariateOptions {
    UnivariateOptions {
        // The pole of every function: discarded, then a new point is drawn.
        forced_points: vec![q(3)],
        bunch: 16,
        parallel_bunches: 4,
        max_points_per_prime: 1024,
        ..Default::default()
    }
}

fn check(black_box: &Random, result: &Reconstruction) {
    let mut rng = StdRng::seed_from_u64(11);
    for (f, g) in black_box.functions.iter().zip(&result.functions) {
        for _ in 0..3 {
            let x = &q(rng.random_range(-500..500)) / &q(rng.random_range(1..500));
            assert_eq!(f.evaluate_rational(&x), g.evaluate_rational(&x));
        }
    }
    let report = &result.report;
    assert_eq!(report.primes[0].failures.get("pole"), Some(&1));
    assert_eq!(report.max_numerator_degree, 170);
    assert_eq!(report.max_denominator_degree, 170);
    // Verification runs at primes never used for reconstruction.
    assert!(!report.verifications.is_empty());
    for verification in &report.verifications {
        assert!(
            report.primes.iter().all(|p| p.prime != verification.prime),
            "verification prime {} was a reconstruction prime",
            verification.prime
        );
    }
}

fn in_pool<T: Send>(threads: usize, run: impl FnOnce() -> T + Send) -> T {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap()
        .install(run)
}

#[test]
fn random_high_degree_functions_are_exact_and_thread_count_invariant() {
    let black_box = Random::new(0x6472_6976_6572, DEGREES);
    let one = in_pool(1, || {
        reconstruct_univariate(&&black_box, &&black_box, &options(), None).unwrap()
    });
    check(&black_box, &one);
    let four = in_pool(4, || {
        reconstruct_univariate(&&black_box, &&black_box, &options(), None).unwrap()
    });
    assert_eq!(one.functions, four.functions);
    assert_eq!(one.report.primes, four.report.primes);
    assert_eq!(one.report.verifications, four.report.verifications);
}

/// A probe log whose appends fail after `budget` records: the run stops
/// with a log error, as a killed process would stop after its last append.
struct Interrupting {
    inner: FileProbeLog,
    budget: usize,
}

impl ProbeLog for Interrupting {
    fn load(&mut self, prime: u64) -> io::Result<Vec<(u64, Option<Vec<u64>>)>> {
        self.inner.load(prime)
    }
    fn record(&mut self, prime: u64, probes: &[(u64, Option<Vec<u64>>)]) -> io::Result<()> {
        if self.budget < probes.len() {
            return Err(io::Error::other("interrupted"));
        }
        self.budget -= probes.len();
        self.inner.record(prime, probes)
    }
}

#[test]
fn an_interrupted_run_resumes_from_its_probe_log_with_an_identical_result() {
    let black_box = Random::new(0x7265_7375_6d65, &DEGREES[..5]);
    let dir = std::env::temp_dir().join(format!("rr-reconstruct-resume-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let key = [0x5a; 32];
    let outputs = black_box.functions.len();
    let reference = reconstruct_univariate(&&black_box, &&black_box, &options(), None).unwrap();
    let mut interrupted = Interrupting {
        inner: FileProbeLog::new(&dir, key, outputs).unwrap(),
        budget: 150,
    };
    let stopped =
        reconstruct_univariate(&&black_box, &&black_box, &options(), Some(&mut interrupted));
    assert!(stopped.is_err(), "the interrupted run must stop");
    let written = interrupted.inner.written;
    // One writer per directory and key: release the first log's lock.
    drop(interrupted);
    let mut log = FileProbeLog::new(&dir, key, outputs).unwrap();
    black_box.calls.store(0, Ordering::Relaxed);
    let resumed =
        reconstruct_univariate(&&black_box, &&black_box, &options(), Some(&mut log)).unwrap();
    assert_eq!(resumed.functions, reference.functions);
    let replayed: usize = resumed.report.primes.iter().map(|p| p.replayed).sum();
    assert!(
        written > 0 && replayed >= written,
        "replayed {replayed}, written {written}"
    );
    let points: usize = reference.report.primes.iter().map(|p| p.points).sum();
    assert!(black_box.calls.load(Ordering::Relaxed) < points);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A caller prime list with repetitions: each prime is still used once, so
/// no reconstruction prime repeats and no verification prime is one of
/// them.
#[test]
fn repeated_caller_primes_are_used_once() {
    let black_box = Random::new(0x7072_696d_6573, &DEGREES[..4]);
    let base = super::driver::reconstruction_primes(12);
    let primes: Vec<u64> = base.iter().flat_map(|&p| [p, p, base[0]]).collect();
    let options = UnivariateOptions {
        primes: Some(primes),
        ..options()
    };
    let result = reconstruct_univariate(&&black_box, &&black_box, &options, None).unwrap();
    let used: Vec<u64> = result.report.primes.iter().map(|p| p.prime).collect();
    let mut distinct = used.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), used.len(), "{used:?}");
    for verification in &result.report.verifications {
        assert!(!used.contains(&verification.prime));
    }
}
