//! Lane procfix7 (gate6 MJ-A): the parent-chain classification of `resources::lineage` on REAL processes of a private
//! pid namespace (rule L10: refuses to run outside one). The walk reads only `stat` / `status`, which stay readable for
//! non-dumpable processes, so no capability games are needed here.

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::resources::lineage::{Hidden, LineageRules, classify_hidden_at};
use std::path::{Path, PathBuf};
use std::process::Command;
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

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

/// Test code: SIGKILL one exact pid of ours (the shell's builtin).
fn kill_pid(pid: u32) {
    let _ = Command::new("sh")
        .args(["-c", &format!("kill -KILL {pid} 2>/dev/null")])
        .env("PATH", path_env())
        .status();
}

fn read_pid(p: &Path) -> u32 {
    wait_until("a pid file", Duration::from_secs(30), || {
        p.exists() && std::fs::read_to_string(p).is_ok_and(|s| s.ends_with('\n'))
    });
    std::fs::read_to_string(p).unwrap().trim().parse().unwrap()
}

/// MJ-A on REAL processes of the namespace (the walk reads only `stat` / `status`, which stay readable): a child of
/// an old process is not ours, a child (and a re-parented orphan) of the live supervisor is possibly ours, a process
/// below a nested pid-namespace init is possibly ours, and a recorded ancestor that died and whose pid was REUSED by a
/// younger process is recognised by its start time (implementer condition 1).
#[test]
fn mj_a_parent_chains_of_real_processes() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf7-chain");
    let go = t.path.join("go");
    let pidf = t.path.join("old-child.pid");
    // O: old; after `go` it starts a child.
    let o = script(
        &t.path,
        "old.sh",
        &format!(
            "#!/bin/sh\nwhile [ ! -e {go} ]; do sleep 0.02; done\n{sleep} 600 &\necho $! > {pidf}\nwait\n",
            go = go.display(),
            sleep = sleep_bin().display(),
            pidf = pidf.display()
        ),
    );
    let old = Command::new(&o).env("PATH", path_env()).spawn().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    // S: the stand-in "live supervisor" with a child and a double-forked orphan (re-parented to pid 1 here).
    let spids = t.path.join("s.pids");
    let s = script(
        &t.path,
        "sup.sh",
        &format!(
            "#!/bin/sh\n{sleep} 600 &\nc=$!\n( {sleep} 600 & echo $! > {spids}.orphan ) \necho $c > {spids}\nwait\n",
            sleep = sleep_bin().display(),
            spids = spids.display()
        ),
    );
    let sup = Command::new(&s).env("PATH", path_env()).spawn().unwrap();
    std::fs::write(&go, b"").unwrap();
    let oc = read_pid(&pidf);
    let sc = read_pid(&spids);
    let orphan = read_pid(&spids.with_extension("pids.orphan"));
    let sup_start = start_ticks(sup.id());
    let me = (1u32, start_ticks(1));
    let live_only = LineageRules {
        live: (sup.id(), sup_start),
        targets: Default::default(),
        earliest: sup_start,
    };
    let root = Path::new("/proc");
    let c = |pid: u32, r: &LineageRules| classify_hidden_at(root, pid, start_ticks(pid), r);
    assert!(
        matches!(c(oc, &live_only), Hidden::NotOurs(_)),
        "{:?}",
        c(oc, &live_only)
    );
    assert!(matches!(c(sc, &live_only), Hidden::Possible(w) if w.contains("this supervisor")));
    // The orphan's real parent is pid 1 (the namespace init, older than the session, no target): not ours under the
    // live-only rule, possibly ours once pid 1 is a recorded ancestor of an earlier (dead) session.
    assert_eq!(
        rustred_campaign::resources::read_stat(orphan)
            .unwrap()
            .unwrap()
            .ppid,
        1
    );
    assert!(matches!(c(orphan, &live_only), Hidden::NotOurs(_)));
    let dead = LineageRules {
        targets: [me].into_iter().collect(),
        ..live_only.clone()
    };
    assert!(matches!(c(orphan, &dead), Hidden::Possible(w) if w.contains("re-parent target")));
    assert!(matches!(c(oc, &dead), Hidden::NotOurs(_)));
    // A nested pid namespace below an old process: its init is a stop.
    let nested = Command::new("unshare")
        .args(["-Upf", "--kill-child", "--", "sh", "-c"])
        .arg(format!("{} 600 & wait", sleep_bin().display()))
        .env("PATH", path_env())
        .spawn()
        .unwrap();
    let inner = {
        let t0 = Instant::now();
        loop {
            let found = std::fs::read_dir("/proc").unwrap().flatten().find_map(|e| {
                let pid: u32 = e.file_name().to_str()?.parse().ok()?;
                let st = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
                let ns = st.lines().find_map(|l| l.strip_prefix("NSpid:"))?;
                (ns.split_whitespace().count() == 2 && ns.split_whitespace().nth(1) == Some("2"))
                    .then_some(pid)
            });
            if let Some(p) = found {
                break p;
            }
            assert!(
                t0.elapsed() < Duration::from_secs(20),
                "nested namespace did not come up"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    assert!(
        matches!(c(inner, &live_only), Hidden::Possible(w) if w.contains("nested pid namespace"))
    );
    // A reused pid of a recorded ancestor: A (recorded) dies, a younger process takes its pid (ns_last_pid), a child of
    // the younger one is NOT matched against the recorded (pid, start) and is decided by the old link above.
    let mut a = Command::new(sleep_bin()).arg("600").spawn().unwrap();
    let (a_pid, a_start) = (a.id(), start_ticks(a.id()));
    let _ = a.kill();
    let _ = a.wait();
    std::thread::sleep(Duration::from_millis(1100)); // a later clock tick for the reuser
    std::fs::write("/proc/sys/kernel/ns_last_pid", format!("{}", a_pid - 1)).unwrap();
    let reuser = script(
        &t.path,
        "reuser.sh",
        &format!(
            "#!/bin/sh\n{sleep} 600 &\necho $! > {f}\nwait\n",
            sleep = sleep_bin().display(),
            f = t.path.join("reuser.child").display()
        ),
    );
    let r = Command::new(&reuser)
        .env("PATH", path_env())
        .spawn()
        .unwrap();
    if r.id() == a_pid {
        assert_ne!(start_ticks(a_pid), a_start);
        let rc = read_pid(&t.path.join("reuser.child"));
        let with_dead_a = LineageRules {
            targets: [(a_pid, a_start)].into_iter().collect(),
            ..live_only.clone()
        };
        assert!(
            matches!(c(rc, &with_dead_a), Hidden::NotOurs(_)),
            "{:?}",
            c(rc, &with_dead_a)
        );
        // the same chain with the CURRENT identity recorded is a stop
        let with_live_a = LineageRules {
            targets: [(a_pid, start_ticks(a_pid))].into_iter().collect(),
            ..live_only.clone()
        };
        assert!(matches!(c(rc, &with_live_a), Hidden::Possible(_)));
        kill_pid(rc);
    } else {
        panic!("pid reuse not obtained ({} != {a_pid})", r.id());
    }
    for p in [oc, sc, orphan] {
        kill_pid(p);
    }
    for mut ch in [old, sup, nested, r] {
        let _ = ch.kill();
        let _ = ch.wait();
    }
}
