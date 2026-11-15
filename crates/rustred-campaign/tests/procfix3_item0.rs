//! procfix3 item 0: the sup_i1 failure "run 46 (sigterm) DIFFERS: codes [143, 143, 0]", every accepted item missing,
//! `verify` 4, an EMPTY log tail. Run inside a private pid namespace (rule L10): LIGHT_PIDNS=1.
//!
//! `campaign_directory_removed_during_a_resume_is_never_exit_0` replays the mechanism found: a second test process
//! with the same temp path (every namespaced test binary is pid 1) runs `remove_dir_all` on the directory of a
//! campaign whose supervisor is running (the first thing its `Tmp::new` did). The supervisor must end as an internal
//! error (70), never 0, whatever survives of the directory.
//!
//! `observe_removal_during_resume` (ignored; RR_ITEM0_DELAYS_MS=a,b,c) prints what happens for several deletion
//! moments (exit code, what `open_read_only` gives, accepted items, verify, log tail length): the reproduction record.

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use std::path::Path;
use std::time::Duration;

fn sup_spec(t: &Tmp) -> std::path::PathBuf {
    let res = serde_json::json!({"mem_total": "300M"});
    let syn = serde_json::json!({"items": 40});
    write_spec(t, "spec.json", &spec(t, syn, res))
}

/// What a campaign directory looks like afterwards (no panics: the directory may be half gone).
#[derive(Debug)]
#[allow(dead_code)] // printed with {:?}
struct Seen {
    code: i32,
    ledger: Result<usize, String>,
    verify: i32,
    log_bytes: usize,
}

fn look(cdir: &Path, code: i32) -> Seen {
    let ledger = rustred_campaign::ledger::open_read_only(cdir)
        .map(|v| {
            rustred_campaign::supervisor::result::final_result(&v.state)
                .accepted
                .len()
        })
        .map_err(|e| e.to_string());
    let log_bytes = std::fs::read(cdir.with_extension("log"))
        .map(|b| b.len())
        .unwrap_or(0);
    Seen {
        code,
        ledger,
        verify: cli(&["verify", cdir.to_str().unwrap()]).0,
        log_bytes,
    }
}

/// Two SIGTERM-interrupted sessions (delays as in sup_i1 run 46 would be on a fast host), then a resume during which
/// `remove_dir_all(temp path)` + `create_dir_all(temp path)` runs `remove_after` into the session (what the colliding
/// `Tmp::new` did).
fn removal_during_resume(t: &Tmp, spec_p: &Path, remove_after: Duration) -> Seen {
    let dir = t.cdir(&format!("run-{}", remove_after.as_millis()));
    for delay in [700u64, 900] {
        let mut c = start(spec_p, &dir, &[], &[]);
        std::thread::sleep(Duration::from_millis(delay));
        if c.try_wait().unwrap().is_none() {
            signal(&c, libc::SIGTERM);
        }
        let _ = wait(&mut c, Duration::from_secs(120));
    }
    let mut c = start(spec_p, &dir, &[], &[]);
    std::thread::sleep(remove_after);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(dir.with_extension("log"));
    let code = wait(&mut c, Duration::from_secs(600));
    look(&dir, code)
}

#[test]
fn campaign_directory_removed_during_a_resume_is_never_exit_0() {
    let _g = serial();
    let t = Tmp::new("rm0");
    let spec_p = sup_spec(&t);
    for ms in [400u64, 1500] {
        let s = removal_during_resume(&t, &spec_p, Duration::from_millis(ms));
        eprintln!("removal after {ms} ms: {s:?}");
        assert_ne!(s.code, 0, "removal after {ms} ms: {s:?}");
        assert_eq!(s.code, 70, "removal after {ms} ms: {s:?}");
        assert_no_marked(&t.campaign);
    }
}

/// Uninterrupted control: the invariant passes on a healthy run (exit 0, the disk ledger equals the live state).
#[test]
fn healthy_run_passes_the_final_invariant() {
    let _g = serial();
    let t = Tmp::new("ok0");
    let spec_p = sup_spec(&t);
    let d = t.cdir("ref");
    // procfix5 (gate4 m3/m4): the exit code follows the items (a synthetic item that crashed twice is in attention: 10)
    let codes = run_to_end(&spec_p, &d, &[]);
    assert_eq!(codes, vec![finish_code_by_items(&d)], "{}", log_tail(&d));
    assert_eq!(verify(&d), 0);
    assert!(!result(&d).accepted.is_empty());
}

#[test]
#[ignore = "reproduction record; RR_ITEM0_DELAYS_MS=a,b,c"]
fn observe_removal_during_resume() {
    let _g = serial();
    let t = Tmp::new("obs0");
    let spec_p = sup_spec(&t);
    let delays: Vec<u64> = std::env::var("RR_ITEM0_DELAYS_MS")
        .unwrap_or_else(|_| "100,400,1000,2500,5000".into())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    for ms in delays {
        let s = removal_during_resume(&t, &spec_p, Duration::from_millis(ms));
        eprintln!("ITEM0 removal after {ms} ms: {s:?}");
    }
}

/// The collision as it happened, without guessing the interleaving: TWO instances of this test, started in two private
/// pid namespaces with the same RR_ITEM0_SHARED path, behave like two sup_i1 binaries of the OLD harness (both pid 1,
/// hence the same `/tmp/rr-sup-i1-1-0`): `remove_dir_all` + `create_dir_all` of the shared path at start, a reference
/// run, then the sup_i1 loop over RR_SUP_I1_FIRST..RR_SUP_I1_RUNS with the sup_i1 seeds, printing what sup_i1 printed.
#[test]
#[ignore = "two concurrent instances; RR_ITEM0_SHARED=<path>"]
fn collide_like_two_i1_runs() {
    use std::time::Duration;
    let Ok(shared) = std::env::var("RR_ITEM0_SHARED") else {
        return;
    };
    let path = std::path::PathBuf::from(shared);
    let _ = std::fs::remove_dir_all(&path); // the OLD Tmp::new
    std::fs::create_dir_all(&path).unwrap();
    let t = Tmp {
        path: path.clone(),
        campaign: "sup-i1-1-0".into(),
    };
    let spec_p = sup_spec(&t);
    let reference_dir = t.cdir("reference");
    let reference = match std::panic::catch_unwind(|| {
        let codes = run_to_end(&spec_p, &reference_dir, &[]);
        (codes, result(&reference_dir))
    }) {
        Ok((codes, r)) => {
            eprintln!(
                "ITEM0 reference codes {codes:?}, {} accepted",
                r.accepted.len()
            );
            r
        }
        Err(_) => {
            eprintln!("ITEM0 reference FAILED (directory disturbed)");
            std::mem::forget(t);
            return;
        }
    };
    let first: u64 = std::env::var("RR_SUP_I1_FIRST")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(40);
    let runs: u64 = std::env::var("RR_SUP_I1_RUNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(52);
    const KINDS: [&str; 4] = ["sigint", "sigint2", "sigterm", "sigkill"];
    for run in first..runs {
        let kind = (run % 4) as usize;
        let seed = mix(0x5eed_0000 + run);
        let dir = t.cdir(&format!("run{run:03}"));
        let n_interrupts = 1 + (seed % 2) as usize;
        let mut codes = Vec::new();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for k in 0..n_interrupts {
                let delay = 300 + mix(seed ^ (k as u64 + 1)) % 3500;
                let mut c = start(&spec_p, &dir, &[], &[]);
                std::thread::sleep(Duration::from_millis(delay));
                if c.try_wait().unwrap().is_some() {
                    codes.push(wait(&mut c, Duration::from_secs(1)));
                    break;
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
                let code = wait(&mut c, Duration::from_secs(120));
                codes.push(code);
                if finished(code) {
                    break;
                }
            }
            if !codes.last().is_some_and(|c| finished(*c)) {
                codes.extend(run_to_end(&spec_p, &dir, &[]));
            }
        }));
        let seen = look(&dir, *codes.last().unwrap_or(&-1));
        let got = rustred_campaign::ledger::open_read_only(&dir)
            .map(|v| rustred_campaign::supervisor::result::final_result(&v.state));
        let same = got.as_ref().is_ok_and(|g| *g == reference);
        eprintln!(
            "ITEM0 run {run} ({}) codes {codes:?} panicked {} same {same} missing_accepted {} {seen:?}",
            KINDS[kind],
            outcome.is_err(),
            got.as_ref()
                .map(|g| reference
                    .accepted
                    .keys()
                    .filter(|k| !g.accepted.contains_key(*k))
                    .count())
                .unwrap_or(usize::MAX),
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
    std::mem::forget(t); // the shared path is the other instance's too
}
