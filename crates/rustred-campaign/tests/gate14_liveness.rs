//! gate14 item 7 (LIVENESS COST of round 10's inconclusive passes): one launcher (one supervisor), many concurrent HONEST
//! fork-heavy jobs; measures jobs per minute, the proof latency (report time minus the leader's wall time) and the
//! unproven ends. Same file builds on 362e932f (before round 10) and on 639c26ce, so the two can be compared.
//!
//! Workloads (RR_G14L_KIND): `pipe` (shell pipelines, every child waited for: nothing is re-parented), `orphan` (each
//! job leaves short-lived background children: they are re-parented to the supervisor and end while OTHER jobs' proofs
//! run), `mix` (half and half). RR_G14L_JOBS (default 96), RR_G14L_CONC (default 8), RR_G14L_LOOPS (default 30).
//!
//! Spawns processes: ONLY inside a private pid namespace (rule L10): refuses unless it is pid 1.
#![allow(clippy::disallowed_methods)]

#[path = "proc_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::launch::{
    CapPreference, ChildHandle, ChildLauncher, ChildProgress, RealChild, RealConfig, RealLauncher,
    RetireEvent, TrackingPreference,
};
use std::time::{Duration, Instant};

fn env_or(k: &str, d: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| d.to_string())
}

fn pct(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() as f64 - 1.0) * p).round() as usize]
}

#[test]
#[ignore = "gate14 audit measurement: run explicitly (--ignored --exact) inside a private pid namespace"]
fn g14_liveness_honest_fork_heavy_jobs() {
    if std::process::id() != 1 {
        eprintln!("SKIPPED: spawns processes: only inside a private pid namespace (rule L10)");
        return;
    }
    let _g = serial();
    let dir = TempDir::new("g14-live");
    let kind = env_or("RR_G14L_KIND", "mix");
    let jobs: usize = env_or("RR_G14L_JOBS", "96").parse().unwrap();
    let conc: usize = env_or("RR_G14L_CONC", "8").parse().unwrap();
    let loops: usize = env_or("RR_G14L_LOOPS", "30").parse().unwrap();
    let mut l = RealLauncher::with_config(
        sampler(),
        RealConfig {
            cap_preference: CapPreference::NoSystemd,
            tracking: TrackingPreference::ProcOnly,
            verify_timeout: Duration::from_secs(10),
            rehunt_every: Duration::from_millis(300),
            ..RealConfig::default()
        },
    );
    let pipe = format!(
        "for i in $(seq {loops}); do echo $i | cat | tr 0-9 a-j | wc -c > /dev/null; done; date +%s.%N > \"$G14_END\"; exit 0"
    );
    let orphan = format!(
        "for i in $(seq {loops}); do ( sh -c 'sleep 0.0$((i % 9 + 1))' & ); echo $i | cat > /dev/null; done; sleep 0.2; date +%s.%N > \"$G14_END\"; exit 0"
    );
    let t0 = Instant::now();
    let mut running: Vec<(RealChild, std::path::PathBuf)> = Vec::new();
    let mut launched = 0usize;
    let mut lat: Vec<f64> = Vec::new();
    let mut verified = 0usize;
    let mut unverified = 0usize;
    let mut reasons: Vec<String> = Vec::new();
    let mut cleared_after: Vec<f64> = Vec::new();
    let mut quarantined = 0usize;
    while launched < jobs || !running.is_empty() {
        while launched < jobs && running.len() < conc {
            let script = match kind.as_str() {
                "pipe" => &pipe,
                "orphan" => &orphan,
                _ if launched % 2 == 0 => &pipe,
                _ => &orphan,
            };
            let end = dir.path().join(format!("j{launched}.end"));
            let mut spec = sh(dir.path(), &format!("j{launched}"), script);
            spec.env.push(("G14_END".into(), end.clone().into()));
            let h = l.launch(&spec).unwrap();
            running.push((h, end));
            launched += 1;
        }
        let mut i = 0;
        while i < running.len() {
            match running[i].0.poll() {
                Ok(ChildProgress::Finished(r)) => {
                    let (h, end) = running.swap_remove(i);
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs_f64();
                    let e: f64 = std::fs::read_to_string(&end)
                        .ok()
                        .and_then(|t| t.trim().parse().ok())
                        .unwrap_or(f64::NAN);
                    lat.push(now - e);
                    if r.verification.is_verified() {
                        verified += 1;
                    } else {
                        unverified += 1;
                        if let rustred_campaign::launch::Verification::Unverified { reason, .. } =
                            &r.verification
                            && reasons.len() < 5
                        {
                            reasons.push(reason.chars().take(200).collect());
                        }
                        assert!(l.retire(h, None).is_none());
                        quarantined += 1;
                    }
                }
                _ => i += 1,
            }
        }
        for ev in l.tick() {
            if let RetireEvent::Cleared { after, .. } = ev {
                cleared_after.push(after.as_secs_f64());
                quarantined -= 1;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let wall = t0.elapsed().as_secs_f64();
    let tq = Instant::now();
    while quarantined > 0 && tq.elapsed() < Duration::from_secs(120) {
        for ev in l.tick() {
            if let RetireEvent::Cleared { after, .. } = ev {
                cleared_after.push(after.as_secs_f64());
                quarantined -= 1;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let n = lat.len();
    let (p50, p90, pmax) = (pct(&mut lat, 0.5), pct(&mut lat, 0.9), pct(&mut lat, 1.0));
    eprintln!(
        "G14L kind {kind} jobs {jobs} conc {conc} loops {loops}: wall {wall:.1} s, {:.1} jobs/min; proof latency \
         p50 {p50:.3} s p90 {p90:.3} s max {pmax:.3} s (n {n}); verified {verified}, unverified {unverified}, \
         cleared later {} (max after {:.2} s), never cleared within 120 s {quarantined}; reasons {reasons:?}",
        jobs as f64 / wall * 60.0,
        cleared_after.len(),
        cleared_after.iter().cloned().fold(0.0, f64::max)
    );
    assert_eq!(quarantined, 0, "jobs never proven");
}
