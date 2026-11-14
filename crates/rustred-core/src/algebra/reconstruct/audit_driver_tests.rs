//! Regression tests from the adversarial audit of the driver and the probe
//! log (review of #24): verifier and black-box contracts, small and
//! composite caller primes, unlucky primes, point budgets, zero functions,
//! many-prime lifting within a time budget, resume with other bunching, and
//! renamed log files. Adapted from the auditor's scratch tests (the exact
//! black box is theirs), with the assertions turned into the contract the
//! fixes establish.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::RingOps;
use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;
use symbolica::prelude::Ring;

use super::Zp;
use super::checkpoint::FileProbeLog;
use super::driver::{
    BlackBox, PrimeBlackBox, ProbeFailure, ProbeResult, ReconstructionError, UnivariateOptions,
    reconstruct_univariate, reconstruction_primes,
};
use super::rational::RationalFunction;

fn q(v: i64) -> Rational {
    Rational::from(Integer::from(v))
}
fn qi(v: Integer) -> Rational {
    Rational::from(v)
}
// ---------------------------------------------------------------- generic exact black box
/// Black box evaluating exact functions; `refuse` = return None from at_prime when an image
/// does not exist (a coefficient denominator vanishes), else evaluate the "integer form":
/// N_int(x)/D_int(x) mod p where N_int, D_int are the primitive integer numerator/denominator.
struct Exact {
    functions: Vec<RationalFunction>,
    /// For each function, the integer-coefficient (cleared) numerator and denominator.
    int_form: Vec<(Vec<Integer>, Vec<Integer>)>,
    /// Override: number of values returned (verifier-side truncation test).
    truncate_to: Option<usize>,
    /// Corrupt function k (add 1 to its value) at every prime.
    corrupt_fn: Option<usize>,
    /// Corrupt function k at the first prime only (a same-shape bad image).
    corrupt_first_prime: Option<usize>,
    primes_seen: AtomicUsize,
    calls: AtomicUsize,
}

fn clear(values: &[Rational]) -> Vec<Integer> {
    let mut l = Integer::one();
    for v in values {
        l = l.lcm(&v.denominator());
    }
    values
        .iter()
        .map(|v| {
            let t = v * &qi(l.clone());
            t.numerator()
        })
        .collect()
}

impl Exact {
    fn new(functions: Vec<RationalFunction>) -> Self {
        let int_form = functions
            .iter()
            .map(|f| {
                // N/D with D normalised: clear both with a common factor.
                let all: Vec<Rational> =
                    f.numerator.iter().chain(&f.denominator).cloned().collect();
                let c = clear(&all);
                let (n, d) = c.split_at(f.numerator.len());
                (n.to_vec(), d.to_vec())
            })
            .collect();
        Self {
            functions,
            int_form,
            truncate_to: None,
            corrupt_fn: None,
            corrupt_first_prime: None,
            primes_seen: AtomicUsize::new(0),
            calls: AtomicUsize::new(0),
        }
    }
}

struct ExactPrime<'a> {
    field: Zp64,
    images: Vec<(Vec<Zp>, Vec<Zp>)>,
    truncate_to: Option<usize>,
    corrupt_fn: Option<usize>,
    calls: &'a AtomicUsize,
}

impl<'a> BlackBox for &'a Exact {
    type Prime = ExactPrime<'a>;
    fn outputs(&self) -> usize {
        self.functions.len()
    }
    fn at_prime(&self, prime: u64) -> Option<ExactPrime<'a>> {
        let field = Zp64::new(prime);
        let m = |v: &Integer| -> Zp {
            let r = v.clone() % &Integer::from(prime);
            let r = if r < Integer::zero() {
                r + Integer::from(prime)
            } else {
                r
            };
            let r: u64 = r.to_string().parse().unwrap();
            field.to_element(r)
        };
        let images = self
            .int_form
            .iter()
            .map(|(n, d)| (n.iter().map(m).collect(), d.iter().map(m).collect()))
            .collect();
        Some(ExactPrime {
            field,
            images,
            truncate_to: self.truncate_to,
            corrupt_fn: if self.primes_seen.fetch_add(1, Ordering::Relaxed) == 0 {
                self.corrupt_fn.or(self.corrupt_first_prime)
            } else {
                self.corrupt_fn
            },
            calls: &self.calls,
        })
    }
}

impl PrimeBlackBox for ExactPrime<'_> {
    fn probe(&self, xs: &[u64]) -> Vec<ProbeResult> {
        self.calls.fetch_add(xs.len(), Ordering::Relaxed);
        let f = &self.field;
        let horner = |c: &[Zp], x: &Zp| {
            let mut v = f.zero();
            for k in c.iter().rev() {
                v = f.add(&f.mul(&v, x), k);
            }
            v
        };
        xs.iter()
            .map(|&x| {
                let x = f.to_element(x);
                let mut out = Vec::new();
                for (k, (n, d)) in self.images.iter().enumerate() {
                    let inv = f.try_inv(&horner(d, &x)).ok_or(ProbeFailure("pole"))?;
                    let mut v = f.mul(&horner(n, &x), &inv);
                    if self.corrupt_fn == Some(k) {
                        v = f.add(&v, &f.one());
                    }
                    out.push(f.from_element(&v));
                }
                if let Some(t) = self.truncate_to {
                    out.truncate(t);
                }
                Ok(out)
            })
            .collect()
    }
}

/// Normalise an exact N/D: lowest nonzero denominator coefficient one (no gcd step: inputs are coprime).
fn exact(n: Vec<Rational>, d: Vec<Rational>) -> RationalFunction {
    let low = d.iter().position(|v| !v.is_zero()).unwrap();
    let c = d[low].clone();
    RationalFunction {
        numerator: n.iter().map(|v| v / &c).collect(),
        denominator: d.iter().map(|v| v / &c).collect(),
    }
}

fn opts() -> UnivariateOptions {
    UnivariateOptions::default()
}

/// D1 (review #24 finding 1, MAJOR): a verifier whose probe returns fewer
/// values than outputs used to verify only a prefix and return a WRONG f2 as
/// verified. Now an error.
#[test]
fn d1_a_short_verifier_vector_is_an_error() {
    let f1 = RationalFunction {
        numerator: vec![q(1), q(2)],
        denominator: vec![q(1), q(3)],
    };
    let f2 = RationalFunction {
        numerator: vec![q(5), q(0), q(1)],
        denominator: vec![q(1)],
    };
    let mut bb = Exact::new(vec![f1.clone(), f2.clone()]);
    bb.corrupt_fn = Some(1);
    let mut verifier = Exact::new(vec![f1, f2]);
    verifier.truncate_to = Some(1);
    let result = reconstruct_univariate(&&bb, &&verifier, &opts(), None);
    assert!(
        matches!(result, Err(ReconstructionError::BlackBox(_))),
        "{result:?}"
    );
}

/// D1b: a verifier with another output count is refused before any work.
#[test]
fn d1b_a_verifier_with_another_output_count_is_refused() {
    let f = RationalFunction {
        numerator: vec![q(1), q(2)],
        denominator: vec![q(1), q(3)],
    };
    let bb = Exact::new(vec![f.clone(), f.clone()]);
    let verifier = Exact::new(vec![f]);
    let result = reconstruct_univariate(&&bb, &&verifier, &opts(), None);
    assert!(matches!(result, Err(ReconstructionError::BlackBox(_))));
    assert_eq!(bb.calls.load(Ordering::Relaxed), 0);
}

/// D2 (finding 4): tiny caller primes used to hang in `draw()`. They are
/// refused now; and a prime just above the minimum terminates (its point
/// budget is capped by its residues) instead of looping.
#[test]
fn d2_small_primes_never_hang() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let f = RationalFunction {
            numerator: (0..=10).map(|i| q(i + 1)).collect(),
            denominator: (0..=10).map(|i| q(2 * i + 1)).collect(),
        };
        let bb = Exact::new(vec![f]);
        let tiny = UnivariateOptions {
            primes: Some(vec![13, 17, 19, 23, 29, 31, 37, 41]),
            ..opts()
        };
        let refused = matches!(
            reconstruct_univariate(&&bb, &&bb, &tiny, None),
            Err(ReconstructionError::InvalidOptions(_))
        );
        // 65537 has 65536 residues, more than enough; the budget cap is the
        // point: a budget above p - 1 must not spin.
        let small = UnivariateOptions {
            primes: Some(vec![
                65537, 65539, 65543, 65551, 65557, 65563, 65579, 65581, 65587, 65599,
            ]),
            max_points_per_prime: 100_000,
            ..opts()
        };
        let finished = reconstruct_univariate(&&bb, &&bb, &small, None).is_ok();
        let _ = tx.send((refused, finished));
    });
    let (refused, finished) = rx.recv_timeout(Duration::from_secs(60)).expect("no hang");
    assert!(refused);
    assert!(finished);
}

/// D3 (finding 4): a composite caller "prime" is refused.
#[test]
fn d3_a_composite_caller_prime_is_refused() {
    let f = RationalFunction {
        numerator: vec![q(1), q(2)],
        denominator: vec![q(1), q(3)],
    };
    let bb = Exact::new(vec![f]);
    let composite = ((1u64 << 31) - 1) * ((1u64 << 31) + 11);
    let mut primes = vec![composite];
    primes.extend(reconstruction_primes(30));
    let options = UnivariateOptions {
        primes: Some(primes),
        ..opts()
    };
    let result = reconstruct_univariate(&&bb, &&bb, &options, None);
    assert!(matches!(
        result,
        Err(ReconstructionError::InvalidOptions(_))
    ));
}

/// D4: a reconstruction prime divides a coefficient denominator of the
/// normalised function (degree drop, `low` change, numerator drop at the
/// first prime): exact through relearning.
#[test]
fn d4_a_prime_dividing_a_coefficient_denominator_is_survived() {
    let p0 = reconstruction_primes(1)[0];
    let p0q = qi(Integer::from(p0));
    let f1 = exact(vec![q(1), q(1)], vec![p0q.clone(), q(1)]);
    let f2 = exact(vec![q(1), q(1)], vec![q(1), p0q.clone()]);
    let f3 = exact(vec![q(2), q(1), q(0), p0q], vec![q(1), q(0), q(1)]);
    let fs = vec![f1, f2, f3];
    let bb = Exact::new(fs.clone());
    let r = reconstruct_univariate(&&bb, &&bb, &opts(), None).unwrap();
    assert_eq!(r.functions, fs);
}

/// D6: a degree beyond the point budget fails loudly.
#[test]
fn d6_a_degree_beyond_the_point_budget_is_out_of_primes() {
    let f = RationalFunction {
        numerator: (0..=60).map(|i| q(i % 7 + 1)).collect(),
        denominator: (0..=60).map(|i| q(i % 5 + 1)).collect(),
    };
    let bb = Exact::new(vec![f]);
    let options = UnivariateOptions {
        max_points_per_prime: 80,
        ..opts()
    };
    let r = reconstruct_univariate(&&bb, &&bb, &options, None);
    assert!(matches!(r, Err(ReconstructionError::OutOfPrimes { .. })));
}

/// D7: zero and constant functions only.
#[test]
fn d7_zero_and_constant_functions() {
    let fs = vec![
        RationalFunction::zero(),
        RationalFunction::constant(&q(5) / &q(7)),
        RationalFunction::constant(q(-123456789)),
    ];
    let bb = Exact::new(fs.clone());
    let r = reconstruct_univariate(&&bb, &&bb, &opts(), None).unwrap();
    assert_eq!(r.functions, fs);
    assert_eq!(r.report.zero_functions, 1);
}

/// D8 (finding 2, MAJOR): an 8/8 function with ~600 and ~1200-bit
/// coefficients needs 18 and 33 primes. The leave-one-prime-out retry used
/// to run at every pending prime (O(k^3)): 1.37 s and 15.3 s in a debug
/// build (auditor's measurement), against 0.03 s and 0.09 s without it. It
/// now runs at stall checkpoints only, with two reconstructions per rebuild.
#[test]
fn d8_many_prime_lifting_stays_within_a_time_budget() {
    let mut rng = StdRng::seed_from_u64(77);
    let big = |rng: &mut StdRng, words: usize| {
        let mut v = Integer::one();
        for _ in 0..words {
            v = &(&v * &Integer::from(1u64 << 60))
                + &Integer::from(rng.random_range(1u64..(1 << 60)));
        }
        v
    };
    for (words, budget) in [(10usize, 1.0f64), (20, 3.0)] {
        let n: Vec<Rational> = (0..=8)
            .map(|_| &qi(big(&mut rng, words)) / &qi(big(&mut rng, 2)))
            .collect();
        let d: Vec<Rational> = (0..=8).map(|_| qi(big(&mut rng, words / 2))).collect();
        let f = exact(n, d);
        let bb = Exact::new(vec![f.clone()]);
        let options = UnivariateOptions {
            max_primes: 80,
            primes: Some(reconstruction_primes(120)),
            ..opts()
        };
        let started = Instant::now();
        let r = reconstruct_univariate(&&bb, &&bb, &options, None).unwrap();
        let seconds = started.elapsed().as_secs_f64();
        eprintln!(
            "d8: ~{} bits: primes {} reconstruction {:.3} s wall {seconds:.3} s",
            60 * words,
            r.report.max_primes_per_function,
            r.report.reconstruction_seconds
        );
        assert_eq!(r.functions[0], f);
        assert!(r.report.max_primes_per_function >= 15);
        assert!(
            r.report.reconstruction_seconds < budget,
            "{} s",
            r.report.reconstruction_seconds
        );
    }
}

/// D10 (finding 5): a black box returning a short vector used to panic.
#[test]
fn d10_a_short_black_box_vector_is_an_error() {
    let f1 = RationalFunction {
        numerator: vec![q(1), q(2)],
        denominator: vec![q(1), q(3)],
    };
    let mut bb = Exact::new(vec![f1.clone(), f1]);
    bb.truncate_to = Some(1);
    let result = reconstruct_univariate(&&bb, &&bb, &opts(), None);
    assert!(
        matches!(result, Err(ReconstructionError::BlackBox(_))),
        "{result:?}"
    );
}

/// D11: a resume with other bunching gives the same functions.
#[test]
fn d11_resume_with_other_bunching_is_identical() {
    let fs: Vec<RationalFunction> = (1..6)
        .map(|k| RationalFunction {
            numerator: (0..=k).map(|i| q(i * 31 - 7)).collect(),
            denominator: (0..=k).map(|i| q(2 * i + 1)).collect(),
        })
        .collect();
    let bb = Exact::new(fs.clone());
    let dir = std::env::temp_dir().join(format!("rr-audit-bunch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut log = FileProbeLog::new(&dir, [9; 32], fs.len()).unwrap();
    let a = reconstruct_univariate(
        &&bb,
        &&bb,
        &UnivariateOptions { bunch: 3, ..opts() },
        Some(&mut log),
    )
    .unwrap();
    drop(log);
    let mut log = FileProbeLog::new(&dir, [9; 32], fs.len()).unwrap();
    let b = reconstruct_univariate(
        &&bb,
        &&bb,
        &UnivariateOptions {
            bunch: 16,
            parallel_bunches: 3,
            ..opts()
        },
        Some(&mut log),
    )
    .unwrap();
    assert_eq!(a.functions, b.functions);
    let replayed: usize = b.report.primes.iter().map(|p| p.replayed).sum();
    assert!(replayed > 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// D12 (finding 7): swapped per-prime log files are refused (the prime is
/// in the header now), instead of being replayed at the wrong prime.
#[test]
fn d12_swapped_log_files_are_refused() {
    let f = RationalFunction {
        numerator: vec![q(1), q(2), q(5)],
        denominator: vec![q(1), q(3)],
    };
    let bb = Exact::new(vec![f]);
    let dir = std::env::temp_dir().join(format!("rr-audit-rename-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut log = FileProbeLog::new(&dir, [4; 32], 1).unwrap();
    let a = reconstruct_univariate(&&bb, &&bb, &opts(), Some(&mut log)).unwrap();
    drop(log);
    let (p0, p1) = (a.report.primes[0].prime, a.report.primes[1].prime);
    let name = |p: u64| {
        std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|path| path.to_string_lossy().ends_with(&format!("-{p}.bin")))
            .unwrap()
    };
    let (f0, f1) = (name(p0), name(p1));
    let tmp = dir.join("tmp.bin");
    std::fs::rename(&f0, &tmp).unwrap();
    std::fs::rename(&f1, &f0).unwrap();
    std::fs::rename(&tmp, &f1).unwrap();
    let mut log = FileProbeLog::new(&dir, [4; 32], 1).unwrap();
    let b = reconstruct_univariate(&&bb, &&bb, &opts(), Some(&mut log));
    assert!(matches!(b, Err(ReconstructionError::Log(_))), "{b:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// D13 (re-audit of #24): a corrupted image of the same shape at the first
/// prime is dropped only by the leave-one-prime-out retry, which first runs
/// at 8 images. With `max_primes < 8` the run fails loudly with
/// `OutOfPrimes`; with more primes the image is dropped and the result is
/// exact.
#[test]
fn d13_a_corrupted_first_prime_is_recovered_only_from_eight_images() {
    let f = exact(vec![q(1), q(2), q(5)], vec![q(1), q(3)]);
    let fs = vec![f];
    let verifier = Exact::new(fs.clone());
    let corrupted = || {
        let mut bb = Exact::new(fs.clone());
        bb.corrupt_first_prime = Some(0);
        bb
    };
    let short = UnivariateOptions {
        max_primes: 7,
        ..opts()
    };
    let r = reconstruct_univariate(&&corrupted(), &&verifier, &short, None);
    assert!(
        matches!(r, Err(ReconstructionError::OutOfPrimes { .. })),
        "{:?}",
        r.err()
    );
    let long = UnivariateOptions {
        max_primes: 16,
        ..opts()
    };
    let r = reconstruct_univariate(&&corrupted(), &&verifier, &long, None).unwrap();
    assert_eq!(r.functions, fs);
    // The retry dropped the corrupted image: at least 8 primes were
    // drawn, and the accepted lift keeps the others.
    assert_eq!(r.report.recovered, 1);
    assert!(r.report.primes.len() >= 8);
}
