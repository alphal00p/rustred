//! gate11 audit (WS/notes/implementer.gate11.md), letter (i): the COST of round 6's completeness proof at scale. Spawns
//! G11_N (default 5000) sleeping processes in the private namespace (G11_FANOUT shells with N / FANOUT `sleep` children
//! each: a two-level tree under pid 1), then times, each REPEAT times: the proc root listing (`pids`), the listing with
//! its completeness proof (`pids_proven`: the children walk from pid 1 + the listing + the per-pid checks), the strict
//! marker scan (`scan_campaign`, every process's stat / status / environ) and `hidden_descendants`. Prints one line per
//! measurement (milliseconds: min / median / max). Read-only apart from the sleeps it starts and kills.
//!
//! Ignored (a measurement, not a check). Run ONLY inside a private pid namespace:
//! `G11_N=5000 LIGHT_PIDNS=1 light_build.sh gate11 test -p rustred-campaign --test gate11_cost -- --ignored --nocapture`.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use rustred_campaign::resources::procfs::ProcRoot;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn env_usize(k: &str, d: usize) -> usize {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

fn stats(name: &str, n: usize, mut v: Vec<f64>) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    eprintln!(
        "G11COST n={n} {name}: min {:.1} ms, median {:.1} ms, max {:.1} ms ({} runs)",
        v[0],
        v[v.len() / 2],
        v[v.len() - 1],
        v.len()
    );
}

#[test]
#[ignore = "gate11 (i): a measurement"]
fn g11_cost_of_the_completeness_proof_at_scale() {
    if !rustred_campaign::launch::in_private_namespace() {
        eprintln!("SKIPPED: spawns processes: only inside a private pid namespace (rule L10)");
        return;
    }
    let n = env_usize("G11_N", 5000);
    let fan = env_usize("G11_FANOUT", 50).max(1);
    let rep = env_usize("G11_REPEAT", 7).max(1);
    let path = std::env::var_os("PATH").unwrap_or_default();
    let per = n / fan;
    // procfix-independent harness note (gate11): a process spawned by a TEST THREAD is on that thread's children list,
    // which the completeness walk does not read (G11-M1). Each fan shell is therefore started by a subshell that exits at
    // once (`( sh -c ... & )`): the shell is re-parented to the namespace's init LEADER (this test binary's main thread),
    // so the walk reaches the whole tree and its cost is measured.
    let mut shells: Vec<Child> = Vec::new();
    let c = Command::new("sh")
        .args([
            "-c",
            &format!(
                "j=0; while [ $j -lt {fan} ]; do ( sh -c 'i=0; while [ $i -lt {per} ]; do sleep 900 & i=$((i+1)); done; wait' & ); j=$((j+1)); done"
            ),
        ])
        .env("PATH", &path)
        .stdin(Stdio::null())
        .spawn()
        .expect("sh");
    shells.push(c);
    let want = fan * per + fan;
    let t0 = Instant::now();
    loop {
        let have = ProcRoot::system().pids().map(|v| v.len()).unwrap_or(0);
        if have >= want || t0.elapsed() > Duration::from_secs(600) {
            eprintln!(
                "G11COST spawned: {have} processes in the namespace (wanted >= {want}) after {:?}",
                t0.elapsed()
            );
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    std::thread::sleep(Duration::from_secs(1));
    let total = ProcRoot::system().pids().unwrap().len();
    let time = |f: &mut dyn FnMut()| -> Vec<f64> {
        (0..rep)
            .map(|_| {
                let t = Instant::now();
                f();
                t.elapsed().as_secs_f64() * 1e3
            })
            .collect()
    };
    stats(
        "pids (listing, two enumerations)",
        total,
        time(&mut || {
            ProcRoot::system().pids().unwrap();
        }),
    );
    stats(
        "pids_proven (walk + listing + checks)",
        total,
        time(&mut || {
            ProcRoot::system().pids_proven(&[]).unwrap();
        }),
    );
    let live = std::collections::BTreeSet::new();
    stats(
        "scan_campaign (strict marker scan)",
        total,
        time(&mut || {
            rustred_campaign::resources::scan_campaign("gate11cost.0123456789abcdef", 0, &live)
                .unwrap();
        }),
    );
    stats(
        "hidden_descendants",
        total,
        time(&mut || {
            rustred_campaign::resources::hidden_descendants().unwrap();
        }),
    );
    let reached: usize = {
        // the walk's reach (leader lists from pid 1 and from this process), for the record
        let mut seen = std::collections::BTreeSet::new();
        let mut stack = vec![1u32, std::process::id()];
        while let Some(p) = stack.pop() {
            if let Ok(s) = std::fs::read_to_string(format!("/proc/{p}/task/{p}/children")) {
                for w in s.split_whitespace() {
                    if let Ok(c) = w.parse::<u32>()
                        && seen.insert(c)
                    {
                        stack.push(c);
                    }
                }
            }
        }
        seen.len()
    };
    eprintln!("G11COST n={total}: the leader-list walk reaches {reached} processes");
    for mut c in shells {
        let _ = c.kill();
        let _ = c.wait();
    }
    // the sleeps and their shells (re-parented to this pid 1) end with the namespace
}
