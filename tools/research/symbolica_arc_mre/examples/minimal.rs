//! Minimal version of src/main.rs (no pinning, no layout control, no variants beyond the two below).
//! K threads repeatedly split and specialize polynomials that all share ONE context (`shared`), or
//! first copy them onto a per-thread context (`private`), and report thread CPU time per operation.
//!   cargo run --release --example minimal -- <threads> shared|private [ops per thread]
//! Run it under e.g. `taskset -c 0-95` so that the threads spread over several L3 domains.
use std::sync::Arc;
use std::time::Instant;

use symbolica::domains::integer::{Integer, IntegerRing, Z};
use symbolica::license::LicenseManager;
use symbolica::poly::{PolyVariable, polynomial::MultivariatePolynomial};
use symbolica::symbol;

type Poly = MultivariatePolynomial<IntegerRing, u16>;

fn thread_cpu_s() -> f64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: valid pointer to a local timespec.
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    ts.tv_sec as f64 + ts.tv_nsec as f64 * 1e-9
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let threads: usize = args
        .get(1)
        .and_then(|a| a.parse().ok())
        .expect("usage: minimal <threads> shared|private [ops]");
    let private = args.get(2).map(String::as_str) == Some("private");
    let ops: usize = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(20_000);
    assert!(
        LicenseManager::is_licensed(),
        "multi-threaded Symbolica needs SYMBOLICA_LICENSE"
    );

    // 4096 polynomials in 16 variables with 8-64 terms, all created from one template: one context.
    let vars: Arc<Vec<PolyVariable>> = Arc::new(
        (0..16)
            .map(|i| symbol!(format!("min_x{i}").as_str()).into())
            .collect(),
    );
    let template = Poly::new(&Z, None, vars.clone());
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut rnd = |n: u64| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state % n
    };
    let shared: Vec<Poly> = (0..4096)
        .map(|_| {
            let mut p = template.zero();
            for _ in 0..8 + rnd(57) {
                let mut e = [0u16; 16];
                e[rnd(16) as usize] += 1 + rnd(3) as u16;
                e[rnd(16) as usize] += 1;
                p.append_monomial(Integer::from(1 + rnd(1000) as i64), &e);
            }
            p
        })
        .collect();

    let wall = Instant::now();
    let cpu: f64 = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|t| {
                let (shared, vars) = (&shared, &vars);
                s.spawn(move || {
                    let own: Vec<Poly>;
                    let set = if private {
                        // A fresh context and variable map for this thread, same polynomials.
                        let mine = Poly::new(&Z, None, Arc::new(vars.as_ref().clone()));
                        own = shared
                            .iter()
                            .map(|p| {
                                let mut q = mine.zero_with_capacity(p.nterms());
                                for (c, e) in p.coefficients.iter().zip(p.exponents_iter()) {
                                    q.append_monomial_back(c.clone(), e);
                                }
                                q
                            })
                            .collect();
                        &own
                    } else {
                        shared
                    };
                    let c0 = thread_cpu_s();
                    let mut acc = 0usize;
                    for i in 0..ops {
                        let p = &set[(i * 7919 + t * 104_729) % set.len()];
                        // Each output of the split clones the context; replace() creates a new
                        // context and clones the variable map; every drop decrements the counts.
                        acc += p.to_multivariate_polynomial_list(&[0, 1, 2, 3], true).len();
                        acc += p.clone().replace(4 + i % 12, &Integer::from(2)).nterms();
                    }
                    std::hint::black_box(acc);
                    thread_cpu_s() - c0
                })
            })
            .collect();
        workers.into_iter().map(|w| w.join().unwrap()).sum()
    });
    println!(
        "{} threads, {}: {:.0} ns CPU per operation, {:.2} s wall",
        threads,
        if private { "private" } else { "shared" },
        cpu * 1e9 / (threads * ops) as f64,
        wall.elapsed().as_secs_f64()
    );
}
