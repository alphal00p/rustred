//! Lane procfix7 round 19 (found by the landing tree's wireflow_seedmin_fake: a trial "undecided:refused" in a resumed
//! minimisation): launchers of ONE process share the id lock (`idlock::acquire`'s registry). When the last launcher of an
//! id is DROPPED while another thread builds one, the builder could find the registry entry dead (strong count 0) while
//! the dropping lock still held its descriptor: its own open + try_lock then met that flock -> `IdLock(InUse)`: a
//! spurious refusal of `RealLauncher::new` (seedmin's and termmin's job threads each build one per job). Before round 19
//! `with_config` ran such a launcher silently WITHOUT the lock instead.
//!
//! Here: 8 threads, each building and dropping ad-hoc launchers of this process in a loop; not one may be refused.
//!
//! Builds launchers: ONLY inside a private pid namespace (`LIGHT_PIDNS=1 light_build.sh`, rule L10). One test function
//! (the registry is process-wide).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use rustred_campaign::launch::{CapPreference, RealConfig, RealLauncher, TrackingPreference};
use rustred_campaign::resources::{Sampler, SamplerConfig};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[test]
fn concurrent_launchers_of_one_process_never_refuse_each_other() {
    if !rustred_campaign::launch::in_private_namespace() {
        eprintln!("SKIPPED: builds launchers: only inside a private pid namespace (rule L10)");
        return;
    }
    let sampler = Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(500),
            collect_foreign_pinning: false,
            foreign_pin_max_cpus: usize::MAX,
        })
        .unwrap(),
    );
    let built = Arc::new(AtomicU64::new(0));
    let refused = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let t0 = Instant::now();
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let (sampler, built, refused) = (sampler.clone(), built.clone(), refused.clone());
            std::thread::spawn(move || {
                while t0.elapsed() < Duration::from_secs(8) {
                    // the process's ad-hoc id, as RealLauncher::new (systemd and cgroup probes off: the lock is the subject)
                    let c = RealConfig {
                        cap_preference: CapPreference::NoSystemd,
                        tracking: TrackingPreference::ProcOnly,
                        ..RealConfig::default()
                    };
                    match RealLauncher::try_with_config(sampler.clone(), c) {
                        Ok(l) => {
                            built.fetch_add(1, Ordering::Relaxed);
                            drop(l);
                        }
                        Err(e) => refused.lock().unwrap().push(e.to_string()),
                    }
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    let refused = refused.lock().unwrap();
    eprintln!(
        "PF19R built {} launchers in 8 threads, refused {}{}",
        built.load(Ordering::Relaxed),
        refused.len(),
        refused
            .first()
            .map(|e| format!(" (first: {e})"))
            .unwrap_or_default()
    );
    assert!(
        built.load(Ordering::Relaxed) > 100,
        "too few launchers built"
    );
    assert!(
        refused.is_empty(),
        "{} refused: {:?}",
        refused.len(),
        &refused[..refused.len().min(3)]
    );
}
