//! procfix3 regressions of the process layer (run ONLY as pid 1 of a private pid namespace: LIGHT_PIDNS=1, rule L10).
//!
//! gate-A M-1: a MARKED job process whose parent is a live job process that LOST the marker (exec'd with an empty or a
//! marker-free environment, own session, optionally `cd /`). Before procfix3 the end-of-job hunt never looked behind
//! the marker-less parent and reported `Verified` while the marked process lived. Now the walk descends through
//! marker-less processes (never signalling them) and the proof passes carry a full-scan marker belt: the marked process
//! is killed through the guard, the marker-less parent is reported (unattributed) and survives, then `Verified`.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "proc_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::launch::{
    CapPreference, ChildLauncher, RealConfig, RealLauncher, TrackingPreference, Verification,
};
use rustred_campaign::resources::{MARKER_VAR, read_stat};
use std::fs;
use std::path::Path;
use std::time::Duration;

fn require_namespace() {
    assert_eq!(
        std::process::id(),
        1,
        "run as pid 1 of a private pid namespace"
    );
}

fn launcher_ns() -> RealLauncher {
    RealLauncher::with_config(
        sampler(),
        RealConfig {
            cap_preference: CapPreference::NoSystemd,
            tracking: TrackingPreference::ProcOnly,
            verify_timeout: Duration::from_secs(30),
            rehunt_every: Duration::from_millis(300),
            ..RealConfig::default()
        },
    )
}

fn read_pid(p: &Path) -> u32 {
    wait_until("pid file", Duration::from_secs(30), || {
        fs::read_to_string(p).is_ok_and(|s| s.trim().parse::<u32>().is_ok())
    });
    fs::read_to_string(p).unwrap().trim().parse().unwrap()
}

fn marker_of(pid: u32) -> Option<String> {
    let b = fs::read(format!("/proc/{pid}/environ")).ok()?;
    b.split(|&c| c == 0)
        .filter_map(|e| std::str::from_utf8(e).ok())
        .find_map(|e| {
            e.strip_prefix(&format!("{MARKER_VAR}="))
                .map(str::to_string)
        })
}

fn live(pid: u32) -> bool {
    read_stat(pid).ok().is_some_and(|s| !s.is_dead())
}

fn kill_own(pid: u32) {
    if live(pid) {
        // SAFETY: our own descendant inside our private pid namespace (we are pid 1 here; checked above).
        unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    }
}

fn run_variant(name: &str, exec_env: &str, cd: &str) {
    let _g = serial();
    require_namespace();
    let d = TempDir::new(name);
    let dir = d.path().to_path_buf();
    let mut l = launcher_ns();
    // A = setsid'd shell; it starts B (marked, inherits the marker) and then execs itself with a marker-less env.
    let script = format!(
        "setsid sh -c '{cd} sleep 300 & echo $! > {b}; echo $$ > {a}; exec {exec_env} sleep 300' & sleep 1; exit 0",
        b = dir.join("b.pid").display(),
        a = dir.join("a.pid").display(),
    );
    let spec = sh(&dir, &format!("pf3-{name}:a1"), &script);
    let mut h = l.launch(&spec).unwrap();
    let a = read_pid(&dir.join("a.pid"));
    let b = read_pid(&dir.join("b.pid"));
    std::thread::sleep(Duration::from_millis(300));
    assert!(marker_of(b).is_some(), "B carries the marker");
    let r = drive(&mut h);
    let (a_alive, b_alive) = (live(a), live(b));
    println!(
        "PF3 {name}: verification={:?} unattributed={:?} killed={:?} A {a} alive={a_alive} B {b} alive={b_alive}",
        r.verification,
        r.unattributed_children
            .iter()
            .map(|u| (u.pid, u.comm.clone()))
            .collect::<Vec<_>>(),
        r.leftovers_killed.iter().map(|k| k.pid).collect::<Vec<_>>()
    );
    kill_own(b);
    kill_own(a);
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        matches!(r.verification, Verification::Verified { .. }),
        "{name}: {:?}",
        r.verification
    );
    assert!(
        !b_alive,
        "{name}: the MARKED process {b} survived a Verified end"
    );
    assert!(
        r.leftovers_killed.iter().any(|k| k.pid == b),
        "{name}: B {b} must be reported killed: {:?}",
        r.leftovers_killed
    );
    assert!(
        a_alive,
        "{name}: the marker-less process {a} is never signalled"
    );
    assert!(
        r.unattributed_children.iter().any(|u| u.pid == a),
        "{name}: A {a} reported unattributed"
    );
}

#[test]
fn m1_marked_grandchild_behind_empty_environ_parent_is_killed_before_verified() {
    run_variant("g1empty", "env -i", "");
}

#[test]
fn m1_marked_grandchild_behind_marker_free_parent_cwd_root_is_killed_before_verified() {
    run_variant("g1unset", &format!("env -u {MARKER_VAR}"), "cd /;");
}

/// gate-A m-5: the tripwire extended beyond the signal syscalls (those stay in proc_reaudit_regressions::ra_m5): every
/// REAP form (waitid / wait4 / waitpid / `.wait()` / `try_wait` / `.status()` / `.output()`), SIGSTOP / SIGCONT,
/// cgroup.freeze / cgroup.kill, `raise(`, PR_SET_PDEATHSIG, ANY `Command::new(` (also with a variable path), the `nix` /
/// `rustix` crates, and SIGCHLD dispositions (SIG_IGN, SA_NOCLDWAIT) appear only in the files named here (with the
/// reason). Scanning stops only at a `#[cfg(test)]` that introduces a `mod` block (not at any `#[cfg(test)]` line).
#[test]
fn m5_extended_tripwire_reap_stop_spawn_forms_only_where_allowed() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let allowed: &[(&str, &[&str])] = &[
        // guard: waitid(P_PIDFD) of exact zombies (sys), SIGCHLD checks; real: peek_exit (WNOWAIT), the systemd probe
        // (own child, own session), try_wait under the registry lock; cli: the pre-flight version probe (separate
        // process, no launcher); the fake child (test double): wait4 of its own helpers.
        ("waitid", &["launch/guard.rs", "launch/real.rs"]),
        ("wait4", &["bin/rr_fake_child.rs"]),
        ("waitpid", &[]),
        (".wait()", &[]),
        ("try_wait", &["launch/real.rs", "cli/mod.rs"]),
        (".status()", &["launch/real.rs"]),
        (".output()", &[]),
        // stop / continue: only through the guard, from the hunt, the sweep and the launcher's terminate.
        // procfix6 (gate5 m1): the guard itself names SIGSTOP (it makes the stop hold of a delivered stop) and SIGCONT
        // (the stop hold allows SIGKILL / SIGCONT only); the sweep and the hunt continue through `guard::continue_held`.
        (
            "SIGSTOP",
            &["launch/hunt.rs", "launch/sweep.rs", "launch/guard.rs"],
        ),
        ("SIGCONT", &["launch/real.rs", "launch/guard.rs"]),
        ("cgroup.freeze", &["launch/cgroup.rs"]),
        ("cgroup.kill", &["launch/cgroup.rs"]),
        // the fake child's legacy rr_proof emulation (a TEST program, lane closeloop2) raises a signal on itself
        ("raise(", &["bin/rr_fake_child/proof.rs"]),
        (
            "PR_SET_PDEATHSIG",
            &["launch/real.rs", "bin/rr_fake_child.rs"],
        ),
        ("Command::new(", &["launch/real.rs", "cli/mod.rs"]),
        ("rustix::", &[]),
        ("SIG_IGN", &["launch/guard.rs", "bin/rr_fake_child.rs"]),
        ("SA_NOCLDWAIT", &["launch/guard.rs"]),
    ];
    let mut files = Vec::new();
    let mut stack = vec![src.clone()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                files.push(p);
            }
        }
    }
    let mut hits = Vec::new();
    for f in &files {
        let rel = f.strip_prefix(&src).unwrap().display().to_string();
        let text = fs::read_to_string(f).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.trim_start().starts_with("#[cfg(test)]")
                && lines[i + 1..]
                    .iter()
                    .find(|l| !l.trim().is_empty())
                    .is_some_and(|l| l.trim_start().starts_with("mod "))
            {
                break;
            }
            let code = line.split("//").next().unwrap_or("");
            for (pat, ok) in allowed {
                if code.contains(pat) && !ok.contains(&rel.as_str()) {
                    hits.push(format!("{rel}:{}: [{pat}] {}", i + 1, line.trim()));
                }
            }
            // `nix::` as a path start (not `unix::`).
            if code.match_indices("nix::").any(|(k, _)| {
                k == 0
                    || !code[..k]
                        .chars()
                        .last()
                        .is_some_and(|c| c.is_alphanumeric() || c == '_')
            }) {
                hits.push(format!("{rel}:{}: [nix::] {}", i + 1, line.trim()));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "forms outside their allowed files: {hits:#?}"
    );
    // Self-test: a planted snippet is caught.
    let planted =
        "fn f() { let _ = std::process::Command::new(p).status(); unsafe { libc::raise(9) }; }";
    assert!(allowed.iter().filter(|(p, _)| planted.contains(p)).count() >= 3);
}

/// The freezing campaign sweep never touches a DECOY: unmarked processes, another campaign id, prefix / suffix / case
/// variants of the id, the marker text in another variable, the exact marker on a process started BEFORE the bound,
/// and a marked process the caller's narrowing refuses (reported in `left`, alive). Every true member dies, including a
/// marked chain that re-forks into a new session every 0.1 s.
#[test]
fn sweep_kills_only_marked_members_and_never_a_decoy() {
    use rustred_campaign::launch::sweep::{SweepSpec, sweep};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    let _g = serial();
    require_namespace();
    let d = TempDir::new("sweep-decoys");
    let id = "pf3sweep.0123456789abcdef";
    let spawn = |env: &[(&str, String)], script: &str| {
        let mut c = std::process::Command::new("sh");
        c.args(["-c", script])
            .current_dir(d.path())
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default());
        for (k, v) in env {
            c.env(k, v);
        }
        c.spawn().unwrap()
    };
    // An OLD process with the exact marker (started before the bound below).
    let mut old = spawn(&[(MARKER_VAR, format!("{id}:job-t0:a1"))], "exec sleep 600");
    std::thread::sleep(Duration::from_millis(50));
    let bound = read_stat(std::process::id())
        .unwrap()
        .start_ticks
        .max(read_stat(old.id()).unwrap().start_ticks + 1);
    std::thread::sleep(Duration::from_millis(30));
    let mut decoys = vec![
        spawn(&[], "exec sleep 600"),
        spawn(
            &[(MARKER_VAR, "other.0123456789abcdef:job-t0:a1".into())],
            "exec sleep 600",
        ),
        spawn(
            &[(MARKER_VAR, format!("{id}0:job-t0:a1"))],
            "exec sleep 600",
        ),
        spawn(
            &[(MARKER_VAR, format!("x{id}:job-t0:a1"))],
            "exec sleep 600",
        ),
        spawn(
            &[(MARKER_VAR, id.to_uppercase() + ":job-t0:a1")],
            "exec sleep 600",
        ),
        spawn(
            &[("FOO", format!("{MARKER_VAR}={id}:job-t0:a1"))],
            "exec sleep 600",
        ),
        spawn(
            &[(&format!("X{MARKER_VAR}"), format!("{id}:job-t0:a1"))],
            "exec sleep 600",
        ),
    ];
    // True members: plain sleepers, a setsid'd one, and a marked re-fork chain (new session per hop).
    let relay = d.path().join("relay.sh");
    fs::write(&relay, "#!/bin/sh\nsleep 0.1\nsetsid \"$0\" &\nexit 0\n").unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&relay, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let m = |a: &str| vec![(MARKER_VAR, format!("{id}:{a}"))];
    let mut members = [
        spawn(&m("job-t0:a1"), "exec sleep 600"),
        spawn(&m("job-t1:a2"), "setsid sleep 600 & exec sleep 600"),
        spawn(
            &m("job-t2:a1"),
            &format!("{} & exec sleep 600", relay.display()),
        ),
    ];
    // A member the narrowing refuses (cwd /).
    let mut refused = {
        let mut c = std::process::Command::new("sleep");
        c.arg("600")
            .current_dir("/")
            .env_clear()
            .env(MARKER_VAR, format!("{id}:job-t3:a1"));
        c.spawn().unwrap()
    };
    std::thread::sleep(Duration::from_millis(500));
    let root = d.path().to_path_buf();
    let narrowing: rustred_campaign::launch::Narrowing = Arc::new(move |p| {
        let cwd = fs::read_link(format!("/proc/{}/cwd", p.pid)).map_err(|e| e.to_string())?;
        if cwd.starts_with(&root) {
            Ok(())
        } else {
            Err(format!("cwd {} outside", cwd.display()))
        }
    });
    let r = sweep(
        &SweepSpec {
            campaign_id: id.into(),
            only_attempt: None,
            not_before_ticks: bound,
            budget: Duration::from_secs(20),
            pass_gap: Duration::from_millis(20),
            narrowing: Some(narrowing),
        },
        &AtomicBool::new(false),
    );
    println!(
        "PF3 sweep: proven={} reason={:?} killed={} left={:?}",
        r.proven,
        r.reason,
        r.killed.len(),
        r.left
    );
    assert!(r.proven, "{r:?}");
    assert!(
        r.killed.len() >= 5,
        "members, the setsid'd sleeper, chain hops: {}",
        r.killed.len()
    );
    assert_eq!(r.left.len(), 1, "{:?}", r.left);
    assert_eq!(r.left[0].process.pid, refused.id());
    // No live process of ours carries a marker of the campaign any more, except the refused one and the old one.
    let marked_now: Vec<u32> = fs::read_dir("/proc")
        .unwrap()
        .flatten()
        .filter_map(|e| e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()))
        .filter(|&p| live(p) && marker_of(p).is_some_and(|m| m.starts_with(&format!("{id}:"))))
        .collect();
    let mut expect = vec![refused.id(), old.id()];
    expect.sort_unstable();
    let mut got = marked_now.clone();
    got.sort_unstable();
    assert_eq!(
        got, expect,
        "only the refused and the too-old marked processes are left"
    );
    for dcy in &mut decoys {
        assert!(
            dcy.try_wait().unwrap().is_none(),
            "decoy {} was signalled",
            dcy.id()
        );
    }
    assert!(
        old.try_wait().unwrap().is_none(),
        "the process older than the bound was signalled"
    );
    assert!(
        refused.try_wait().unwrap().is_none(),
        "the refused process was signalled"
    );
    for c in decoys.iter_mut().chain(members.iter_mut()) {
        kill_own(c.id());
        let _ = c.wait();
    }
    kill_own(old.id());
    let _ = old.wait();
    kill_own(refused.id());
    let _ = refused.wait();
}
