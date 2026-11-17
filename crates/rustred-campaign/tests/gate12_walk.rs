//! gate12 audit (WS/notes/implementer.gate12.md), item 1: is gate11 G11-M1 closed? `ProcRoot::children_all_threads_proven`
//! (resources/procfs.rs, procfix7 round 7) must answer, for a process whose threads come and go and whose children are
//! RE-PARENTED between its threads while the walk reads them, either the complete set of its children or Unreadable:
//! never Ok with a child that was there all the time missing. And its retry must be BOUNDED under permanent churn.
//!
//! - `g12_walk_reparent_stream_*`: scripts/gate12/src/g12stream.c: every MS ms a new thread forks a child and exits at once
//!   (the child is re-parented to the leader, or with `z` to the live non-leader thread W0 of a zombie leader). The test
//!   calls `children_all_threads_proven` in a loop: every Ok answer must contain every child that was reported before the
//!   call started and is still the same live child of the helper after it.
//!   IGNORED: FAILS on 4435f6db (G12-M1): at 1 ms (~700-900 live children, a children file of 4.8-6 KB > one seq_file
//!   page) 40-60 of ~100-130 Ok answers lack a live child; at 10 ms (one page) none. Run with `--include-ignored`.
//!   procfix7 round 8: UN-IGNORED: `ProcRoot::children` reads every non-empty list until two consecutive reads agree
//!   (bounded, else Unreadable "children list unstable", which the walk retries with its back-off; the kernel's by-index
//!   continuation also skips inside ONE buffer: 1 violation in ~1,000 Ok at 10 ms with a single-buffer rule), and
//!   `children_all_threads_proven` refuses a pass during which a thread entered its exit state (its children moved).
//! - `g12_walk_permanent_churn_is_bounded`: scripts/gate12/src/g12thr.c `churn 0` (4 threads created back to back for
//!   ever; the child's parent thread lives): each call ends within a bound, Ok containing the child or Unreadable.
//!
//! Run ONLY inside a private pid namespace: `LIGHT_PIDNS=1 light_build.sh gate12 test -p rustred-campaign --test gate12_walk`.
//! Needs a C compiler (skips otherwise).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use rustred_campaign::resources::procfs::{ProcError, ProcRoot};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn in_ns() -> bool {
    if rustred_campaign::launch::in_private_namespace() {
        return true;
    }
    eprintln!(
        "SKIPPED: spawns and signals processes: only inside a private pid namespace (rule L10)"
    );
    false
}

fn path_env() -> std::ffi::OsString {
    std::env::var_os("PATH").unwrap_or_default()
}

fn tmp() -> PathBuf {
    let d = std::env::temp_dir().join(format!("g12walk-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn cc(name: &str) -> Option<PathBuf> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/gate12/src")
        .join(name);
    let out = tmp().join(name.trim_end_matches(".c"));
    let ok = Command::new("cc")
        .args(["-O1", "-pthread", "-o"])
        .arg(&out)
        .arg(&src)
        .env("PATH", path_env())
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        eprintln!("SKIPPED: no C compiler for {name}");
        return None;
    }
    Some(out)
}

/// (ppid, start ticks) of a live (non-zombie) process.
fn ppid_start(pid: u32) -> Option<(u32, u64)> {
    let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = &s[s.rfind(')')? + 2..];
    let f: Vec<&str> = rest.split_whitespace().collect();
    if f.first() == Some(&"Z") || f.first() == Some(&"X") {
        return None;
    }
    Some((f.get(1)?.parse().ok()?, f.get(19)?.parse().ok()?))
}

/// Test code: SIGKILL one exact pid we started (the shell's builtin), then reap it.
fn stop(mut c: Child) {
    let _ = Command::new("sh")
        .args(["-c", &format!("kill -KILL {} 2>/dev/null", c.id())])
        .env("PATH", path_env())
        .status();
    let _ = c.wait();
}

/// This test binary is pid 1 of its namespace: the helpers' orphans come to it. Wait until they ended, then reap every
/// zombie child (the tests of this file run one at a time: SERIAL; no other child of ours exists then).
fn reap_orphans(settle: Duration) {
    std::thread::sleep(settle);
    loop {
        // SAFETY: waitpid with a null status pointer and WNOHANG only reaps an exited child of this process.
        let r = unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) };
        if r <= 0 {
            break;
        }
    }
}

static SERIAL: Mutex<()> = Mutex::new(());

/// One reparent-stream configuration: returns (Ok answers, Unreadable answers, violations).
fn stream_case(bin: &Path, ms: u32, z: bool, secs: u64) -> (u64, u64, Vec<String>) {
    // the children's life: bounded so that at most ~1,500 live at once
    let life_ms: u64 = match ms {
        0 => 300,
        1 => 1000,
        _ => 3000,
    };
    let mut cmd = Command::new(bin);
    cmd.arg(ms.to_string()).arg(life_ms.to_string());
    if z {
        cmd.arg("z");
    }
    let mut child = cmd.stdout(Stdio::piped()).spawn().unwrap();
    let hp = child.id();
    let out = child.stdout.take().unwrap();
    // reported children: pid -> start ticks (read when the report arrives: the child exists, it is ours)
    let reported: Arc<Mutex<BTreeMap<u32, u64>>> = Arc::default();
    let r2 = reported.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines() {
            let Ok(l) = line else { break };
            if let Ok(p) = l.trim().parse::<u32>()
                && let Some((pp, st)) = ppid_start(p)
                && pp == hp
            {
                r2.lock().unwrap().insert(p, st);
            }
        }
    });
    std::thread::sleep(Duration::from_millis(300));
    let root = ProcRoot::system();
    let (mut ok, mut unr, mut bad) = (0u64, 0u64, Vec::new());
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(secs) {
        let before: BTreeMap<u32, u64> = reported.lock().unwrap().clone();
        let ans = root.children_all_threads_proven(hp);
        match ans {
            Ok(v) => {
                ok += 1;
                let got: BTreeSet<u32> = v.into_iter().collect();
                for (&p, &st) in &before {
                    if !got.contains(&p) && ppid_start(p) == Some((hp, st)) {
                        // diagnostic: the size of every thread's children file now (a seq_file page is 4096 bytes)
                        let mut sizes = Vec::new();
                        if let Ok(rd) = std::fs::read_dir(format!("/proc/{hp}/task")) {
                            for e in rd.flatten() {
                                let n =
                                    std::fs::read(e.path().join("children")).map_or(0, |b| b.len());
                                if n > 0 {
                                    sizes.push(n);
                                }
                            }
                        }
                        bad.push(format!(
                            "child {p} of {hp} (ms {ms}, z {z}) live and ours before and after an Ok answer ({} children) that lacks it; children-file bytes now {sizes:?}",
                            got.len()
                        ));
                    }
                }
            }
            Err(ProcError::Unreadable(_)) => unr += 1,
            Err(e) => panic!("unexpected answer for the live helper {hp}: {e:?}"),
        }
        // forget the children that are gone (they live 3 s)
        reported
            .lock()
            .unwrap()
            .retain(|p, st| ppid_start(*p).is_some_and(|(_, s)| s == *st));
    }
    stop(child);
    reap_orphans(Duration::from_millis(life_ms + 500));
    (ok, unr, bad)
}

#[test]
// procfix7 round 8: un-ignored (G12-M1 fixed: every non-empty children list read until two reads agree; exits during a pass refused)
fn g12_walk_reparent_stream_never_answers_ok_with_a_child_missing() {
    if !in_ns() {
        return;
    }
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let Some(bin) = cc("g12stream.c") else { return };
    let secs: u64 = std::env::var("G12_STREAM_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    let mut all_bad = Vec::new();
    for z in [false, true] {
        for ms in [0u32, 1, 10] {
            let (ok, unr, bad) = stream_case(&bin, ms, z, secs);
            eprintln!(
                "G12STREAM ms={ms} z={z} secs={secs}: Ok {ok}, Unreadable {unr}, violations {}",
                bad.len()
            );
            assert!(ok + unr > 0, "no answer at all");
            all_bad.extend(bad.into_iter().take(5));
        }
    }
    assert!(all_bad.is_empty(), "{all_bad:#?}");
}

#[test]
fn g12_walk_permanent_churn_is_bounded() {
    if !in_ns() {
        return;
    }
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let Some(bin) = cc("g12thr.c") else { return };
    let pf = tmp().join("churn.pids");
    for ms in [0u32, 1, 10] {
        let _ = std::fs::remove_file(&pf);
        let child = Command::new(&bin)
            .arg(&pf)
            .arg("unused:marker")
            .args(["churn", &ms.to_string()])
            .env_clear()
            .spawn()
            .unwrap();
        let t0 = Instant::now();
        while !pf.exists() && t0.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(20));
        }
        let s = std::fs::read_to_string(&pf).unwrap();
        let mut it = s.split_whitespace().map(|w| w.parse::<u32>().unwrap());
        let (m, hp) = (it.next().unwrap(), it.next().unwrap());
        assert_eq!(hp, child.id());
        let root = ProcRoot::system();
        let (mut ok, mut unr, mut worst) = (0u32, 0u32, Duration::ZERO);
        for _ in 0..60 {
            let t = Instant::now();
            let ans = root.children_all_threads_proven(hp);
            worst = worst.max(t.elapsed());
            match ans {
                Ok(v) => {
                    ok += 1;
                    assert!(v.contains(&m), "Ok without the live child {m}: {v:?}");
                }
                Err(ProcError::Unreadable(w)) => {
                    unr += 1;
                    if unr == 1 {
                        eprintln!("G12CHURN ms={ms}: first Unreadable: {w}");
                    }
                }
                Err(e) => panic!("{e:?}"),
            }
        }
        eprintln!("G12CHURN ms={ms}: Ok {ok}, Unreadable {unr}, worst {worst:?}");
        // 16 tries with a back-off of at most ~0.1 s in all, plus the reads; generous for a loaded host
        assert!(worst < Duration::from_secs(2), "unbounded: {worst:?}");
        let _ = Command::new("sh")
            .args(["-c", &format!("kill -KILL {m} 2>/dev/null")])
            .env("PATH", path_env())
            .status();
        stop(child);
        reap_orphans(Duration::from_millis(300));
    }
}

/// gate12 item 3 (the host step): how often does the completeness proof (`pids_proven`, the round-7 walk over EVERY thread
/// of EVERY process from pid 1) succeed on the WHOLE host /proc? READ-ONLY: it lists and reads /proc only; it spawns and
/// signals nothing (no namespace needed; run explicitly: `--ignored --exact g12_hostwalk_measure`). G12_HOSTWALK_N calls.
#[test]
#[ignore = "measurement on the host /proc (read-only); run explicitly"]
fn g12_hostwalk_measure() {
    let n: usize = std::env::var("G12_HOSTWALK_N")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let root = ProcRoot::system();
    let (mut ok, mut why) = (0usize, BTreeMap::<String, usize>::new());
    let mut times = Vec::new();
    for _ in 0..n {
        let t = Instant::now();
        let r = root.pids_proven(&[]);
        times.push(t.elapsed());
        match r {
            Ok(v) => {
                ok += 1;
                eprintln!("G12HOSTWALK Ok {} pids in {:?}", v.len(), t.elapsed());
            }
            Err(e) => {
                let w = format!("{e:?}");
                eprintln!(
                    "G12HOSTWALK Err in {:?}: {}",
                    t.elapsed(),
                    &w[..w.len().min(300)]
                );
                // the kind of failure, without pids
                let k: String = w
                    .chars()
                    .map(|c| if c.is_ascii_digit() { 'N' } else { c })
                    .take(120)
                    .collect();
                *why.entry(k).or_default() += 1;
            }
        }
    }
    times.sort();
    eprintln!(
        "G12HOSTWALK SUMMARY n={n} ok={ok} median={:?} max={:?} failures={why:#?}",
        times[times.len() / 2],
        times.last().unwrap()
    );
}
