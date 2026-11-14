//! I1 (plan 6.5; brief deliverable 4): a synthetic campaign run uninterrupted gives the reference; then the SAME
//! campaign is interrupted at seeded random moments by SIGINT once (drain), SIGINT twice (force), SIGTERM, or SIGKILL
//! of the supervisor, and resumed with the same command until finished. For every run: the accepted set (canonical
//! digests), the item dispositions and the terminal list equal the reference (user ruling U-1: identical accepted
//! tables given the same ledger decisions); no item is accepted twice; re-runs are bounded by interruptions; the ledger
//! replays and `verify` is clean; admission stayed within the core budget and the memory reservation; no marked
//! process survives. Runs: RR_SUP_I1_RUNS (default 52 = 13 per kind). The summary line is printed for the report.

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use std::time::Duration;

const KINDS: [&str; 4] = ["sigint", "sigint2", "sigterm", "sigkill"];

#[test]
fn i1_random_interrupts_give_the_reference_result() {
    let _g = serial();
    let t = Tmp::new("i1");
    let runs: u64 = std::env::var("RR_SUP_I1_RUNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(52);
    let res = serde_json::json!({"mem_total": "300M"});
    let syn = serde_json::json!({"items": 40});
    let spec_v = spec(&t, syn, res);
    let spec_p = write_spec(&t, "spec.json", &spec_v);
    // Reference.
    let reference_dir = t.cdir("reference");
    let codes = run_to_end(&spec_p, &reference_dir, &[]);
    assert_eq!(codes, vec![0], "reference must run uninterrupted");
    let reference = result(&reference_dir);
    let rf = ledger_facts(&reference_dir);
    assert_eq!(rf.interrupted, 0);
    assert!(
        !reference.accepted.is_empty() && !reference.terminals.is_empty(),
        "{reference:?}"
    );
    assert_eq!(verify(&reference_dir), 0);
    let mut per_kind = [(0u32, 0u32); 4];
    let mut sessions_total = 0;
    let mut interrupts_total = 0;
    // RR_SUP_I1_FIRST=k replays from run k (the seeds depend on the run index only): reproductions of one run.
    let first: u64 = std::env::var("RR_SUP_I1_FIRST")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    for run in first..runs {
        let kind = (run % 4) as usize;
        let seed = mix(0x5eed_0000 + run);
        let dir = t.cdir(&format!("run{run:03}"));
        // One or two interrupted sessions, then resume to the end.
        let n_interrupts = 1 + (seed % 2) as usize;
        let mut codes = Vec::new();
        for k in 0..n_interrupts {
            let delay = 300 + mix(seed ^ (k as u64 + 1)) % 3500;
            let mut c = start(&spec_p, &dir, &[], &[]);
            std::thread::sleep(Duration::from_millis(delay));
            if c.try_wait().unwrap().is_some() {
                codes.push(wait(&mut c, Duration::from_secs(1)));
                break; // finished before the interrupt
            }
            match KINDS[kind] {
                "sigint" => signal(&c, libc::SIGINT),
                "sigint2" => {
                    signal(&c, libc::SIGINT);
                    std::thread::sleep(Duration::from_millis(50 + mix(seed ^ 77) % 400));
                    signal(&c, libc::SIGINT);
                }
                "sigterm" => signal(&c, libc::SIGTERM),
                _ => signal(&c, libc::SIGKILL),
            }
            interrupts_total += 1;
            let code = wait(&mut c, Duration::from_secs(120));
            // ATTENTION (8): the session ended by itself for an operator decision (an attempt of unknown execution
            // left by an earlier SIGKILL between spawn and `job.started`, procfix3 item 6); run_to_end resolves it.
            match KINDS[kind] {
                "sigint" | "sigint2" => {
                    assert!(code == 130 || code == 0 || code == ATTENTION, "exit {code}")
                }
                "sigterm" => assert!(code == 143 || code == 0 || code == ATTENTION, "exit {code}"),
                _ => assert!(
                    code == 128 + libc::SIGKILL || code == ATTENTION,
                    "exit {code}"
                ),
            }
            codes.push(code);
            if code == 0 {
                break;
            }
        }
        if codes.last() != Some(&0) {
            codes.extend(run_to_end(&spec_p, &dir, &[]));
        }
        sessions_total += codes.len();
        let got = result(&dir);
        let same = got == reference;
        if same {
            per_kind[kind].0 += 1;
        } else {
            per_kind[kind].1 += 1;
            eprintln!(
                "run {run} ({}) DIFFERS: codes {codes:?}\n{}",
                KINDS[kind],
                log_tail(&dir)
            );
            for (k, v) in &reference.accepted {
                if got.accepted.get(k) != Some(v) {
                    eprintln!(
                        "  accepted {k}: reference {v:?} got {:?}",
                        got.accepted.get(k)
                    );
                }
            }
        }
        let f = ledger_facts(&dir);
        assert_eq!(
            f.requeues, f.interrupted,
            "run {run}: re-runs must equal interrupted attempts"
        );
        assert!(f.max_cores <= 8, "run {run}: {} cores", f.max_cores);
        assert!(
            f.max_reserved <= 300_000_000,
            "run {run}: reserved {}",
            f.max_reserved
        );
        assert_eq!(verify(&dir), 0, "run {run}: verify");
        assert_no_marked(&t.campaign);
        let _ = std::fs::remove_dir_all(&dir); // keep the temp dir small
    }
    eprintln!(
        "I1 SUMMARY: {runs} runs, {interrupts_total} interrupts, {sessions_total} sessions; per kind (identical, different): {}",
        KINDS
            .iter()
            .zip(per_kind)
            .map(|(k, (a, b))| format!("{k} ({a}, {b})"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    assert!(
        per_kind.iter().all(|(_, b)| *b == 0),
        "some runs differ from the reference"
    );
}
