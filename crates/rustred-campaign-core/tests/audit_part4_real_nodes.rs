//! Re-audit part 4 (lane audit-part4): Q2 / Q3 on the REAL nodes: what `economy::prove_coverage_preserved` (the
//! minting function, default `ProofWork`) answers on T162, T178 order B (case exports and the 38 raw tables) and T266,
//! how long it takes and why it refuses. Every test is `#[ignore]` (fixtures; release; run under the 2 GB scope):
//! `light_build.sh <lane> test -p rustred-campaign-core --release --test audit_part4_real_nodes -- --ignored
//! --nocapture --test-threads 1`. Memory: the scope's MemoryMax kills the binary beyond 2 GB (that is the Q2 check).

mod inputs;
mod partition_common;

use std::time::Instant;

use partition_common::*;
use rustred_campaign_core::cover::RuleRef;
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::economy::{ProofPreconditions, ProofWork, prove_coverage_preserved};
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::partition::{
    DispatchOrderStatement, DispatcherSource, DomainList, DomainSource,
};
use rustred_campaign_core::poly::Poly;

fn rss_mb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|x| x.parse::<u64>().ok())
        })
        .map_or(0, |kb| kb / 1024)
}

fn mint(tag: &str, src: &dyn DomainSource, frame: &NodeFrame, work: &ProofWork) {
    let pre = ProofPreconditions::check(src, frame);
    println!(
        "{tag}: {} records; preconditions: {:?}",
        src.len(),
        pre.as_ref().map(|_| "ok").map_err(|e| e.clone())
    );
    let t0 = Instant::now();
    let r = prove_coverage_preserved(src, src, frame, work, &DispatchOrderStatement::SameOrder);
    let secs = t0.elapsed().as_secs_f64();
    match r {
        Ok(p) => println!(
            "{tag}: PROOF MINTED in {secs:.1} s (peak RSS {} MB): {}",
            rss_mb(),
            p.artefact_sha256()
        ),
        Err(e) => {
            let short: String = e.chars().take(600).collect();
            println!(
                "{tag}: REFUSED in {secs:.1} s (peak RSS {} MB): {short}",
                rss_mb()
            );
        }
    }
}

#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES; release"]
fn p4_q3_t162_minting() {
    let Some(fx) = fx() else { return };
    let disp = t162_dispatcher(&fx);
    mint(
        "T162 bank (as loaded)",
        &DispatcherSource::new(&disp),
        &t162_frame(),
        &ProofWork::default(),
    );
    // the same domains as a plain list (no attestation basis): what the partition itself answers
    let src = DispatcherSource::new(&disp);
    let mut l = DomainList::new(src.arity());
    for i in 0..src.len() {
        l.push(RuleRef::bare(0, i, i as i64), src.domain(i).clone());
    }
    mint(
        "T162 domains as a list",
        &l,
        &t162_frame(),
        &ProofWork::default(),
    );
}

#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES; release"]
fn p4_q3_t178_minting() {
    let Some(fx) = fx() else { return };
    for raw in [false, true] {
        let (disp, _) = t178_dispatcher(&fx, raw);
        mint(
            &format!("T178B raw={raw}"),
            &DispatcherSource::new(&disp),
            &t178_frame(),
            &ProofWork::default(),
        );
    }
}

fn t266(dir: &std::path::Path) -> (DomainList, NodeFrame) {
    let v: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("h11b/out/rules266_nf.json")).unwrap(),
    )
    .unwrap();
    let e = v["E"].as_u64().unwrap() as usize;
    let n = 20; // T266 arity (test data)
    let frame = NodeFrame::without_header_lines_first(n, e);
    let mask: Vec<bool> = (0..n).map(|i| i < e).collect();
    let mut l = DomainList::new(n);
    for r in v["rules"].as_array().unwrap() {
        let mut d = CaseDomain::free(n, Some(mask.clone()));
        let p = |s: &serde_json::Value| {
            Poly::parse(
                &s.as_str().unwrap().replace("**", "^").replace(' ', ""),
                n,
                false,
            )
            .ok()
        };
        let eq: Option<Vec<Poly>> = r["EQ"].as_array().unwrap().iter().map(p).collect();
        let bad: Option<Vec<Vec<Poly>>> = r["BAD"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b.as_array().unwrap().iter().map(p).collect())
            .collect();
        let (Some(eq), Some(bad)) = (eq, bad) else {
            continue;
        };
        d.equations = eq;
        d.exceptions = bad;
        let k = r["k"].as_u64().unwrap() as usize;
        l.push(RuleRef::bare(0, k, k as i64), d);
    }
    (l, frame)
}

/// Q2: T266 (arity 20, 33,660 records) with the DEFAULT minting budget (20,000,000 units, memo 100,000): must return
/// (refuse) within the budget and stay under the 2 GB scope.
#[test]
#[ignore = "needs the checker's T266 sources (read-only); release; 2 GB scope"]
fn p4_q2_t266_minting_default_work() {
    let dir = std::path::PathBuf::from(
        std::env::var("RUSTRED_T266_SOURCES").unwrap_or("/common/ben/sym-investigation".into()),
    );
    if !dir.join("h11b/out/rules266_nf.json").is_file() {
        inputs::missing(format_args!("SKIP: T266 sources not found"));
        return;
    }
    let (l, frame) = t266(&dir);
    println!("T266 loaded: {} domains, peak RSS {} MB", l.len(), rss_mb());
    let w = std::env::var("AUDIT_P4_T266_UNITS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_000_000u64);
    mint(
        "T266",
        &l,
        &frame,
        &ProofWork {
            max_units: w,
            ..ProofWork::default()
        },
    );
}

/// Q2 (cost per work unit): one recursion node is ONE unit, but a node can evaluate a multi-slot linear literal's
/// status with `exact::linear_feasible`, whose bounded-sum DP may cost up to 40 x 4e6 steps and is NOT charged to the
/// meter. Arity 4, a catch-all whose exception branches are a 4-slot literal L and the planes n_s = 61 j (j = 1..K),
/// each plane / L re-covered by a patch (the set has NO gap). Reports the time per unit for a small unit budget and
/// for a wall-clock budget (env AUDIT_P4_UNITS, default 20,000; AUDIT_P4_WALL_S, default 120).
#[test]
#[ignore = "timing; release"]
fn p4_q2_time_per_work_unit_with_unmetered_linear_dp() {
    let n = 4;
    let k: i64 = std::env::var("AUDIT_P4_K")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    let mask = vec![true; n];
    let frame = NodeFrame::without_header_lines_first(n, n);
    let lit = |s: &str| Poly::parse_d_free_exact(s, n).unwrap();
    let l = "1000*n0+1001*n1+1002*n2+1003*n3-123457";
    let mut set = DomainList::new(n);
    let mut i = 0usize;
    let mut push = |set: &mut DomainList, d: CaseDomain| {
        set.push(RuleRef::bare(0, i, i as i64), d);
        i += 1;
    };
    for s in 0..n {
        for j in 1..=k {
            let mut d = CaseDomain::free(n, Some(mask.clone()));
            d.fixed[s] = Some((61 * j) as i32);
            push(&mut set, d);
        }
    }
    let mut pl = CaseDomain::free(n, Some(mask.clone()));
    pl.equations = vec![lit(l)];
    push(&mut set, pl);
    let mut c = CaseDomain::free(n, Some(mask.clone()));
    c.exceptions.push(vec![lit(l)]);
    for s in 0..n {
        for j in 1..=k {
            c.exceptions.push(vec![lit(&format!("n{s}-{}", 61 * j))]);
        }
    }
    push(&mut set, c);
    let units: u64 = std::env::var("AUDIT_P4_UNITS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_000);
    let wall: u64 = std::env::var("AUDIT_P4_WALL_S")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);
    for (tag, w) in [
        (
            "units only",
            ProofWork {
                max_units: units,
                wall: None,
                ..ProofWork::default()
            },
        ),
        (
            "default units + wall",
            ProofWork {
                wall: Some(std::time::Duration::from_secs(wall)),
                ..ProofWork::default()
            },
        ),
    ] {
        let t0 = Instant::now();
        let r =
            prove_coverage_preserved(&set, &set, &frame, &w, &DispatchOrderStatement::SameOrder);
        let secs = t0.elapsed().as_secs_f64();
        let msg: String = match &r {
            Ok(_) => "PROOF".into(),
            Err(e) => e.chars().take(300).collect(),
        };
        println!(
            "Q2 per-unit ({tag}, K = {k}, {} records): {secs:.1} s, peak RSS {} MB: {msg}",
            set.len(),
            rss_mb()
        );
    }
}
