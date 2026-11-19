//! gate9 audit (WS/notes/implementer.gate9.md), BLOCKER G9-B2: a MARKED process whose main thread has exited (a zombie
//! leader) and whose threads keep replacing themselves is judged "a zombie without a live thread" by every strict scan:
//! `resources::live_thread_at` answers NoneAlive when every thread of ONE listing reads dead or gone, and a thread created
//! after the listing is never examined (also `guard::read_facts` zombie_without_threads, `read_proc_facts` dead). After a
//! supervisor crash the resumed session's recovery and final sweeps are "proven": FINISHED, exit 0, the process alive.
//!
//! Without help the evasion is timing-dependent (2 of 2 survived in the auditor's first batch, 0 of 84 later). Here it is
//! made DETERMINISTIC without failing any read: the resumed supervisor's opens of the churner's `/proc/<pid>/task/<tid>/*`
//! files are DELAYED by 2 ms (the gate9 LD_PRELOAD injector `gate9_g9fault.c`, mode `delay`: a descheduled supervisor), so
//! each listed thread really exits before it is read (6 of 6 rc 0 with the churner alive in the auditor's runs, even with
//! 500 us thread lives; a churner whose main thread lives is killed).
//!
//! Ignored (needs a C compiler: `gate9_churn.c` = implementer-scratch/gate9/src/g9churn.c, `gate9_g9fault.c`): FAILS on
//! 9876cba2.
//! Run explicitly, ONLY inside a private pid namespace:
//! `LIGHT_PIDNS=1 light_build.sh gate9 test -p rustred-campaign --test gate9_churn -- --ignored`.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

fn path_env() -> std::ffi::OsString {
    std::env::var_os("PATH").unwrap_or_default()
}

/// Live: the process exists and has a thread besides its (zombie) leader. procfix7 round 6 (gate10 m10-7, the one
/// unexplained failure of this test): it counted the entries of ONE readdir of /proc/<pid>/task, which misses the live
/// thread of a churner whose threads replace themselves (measured, S/exp/churn: < 2 entries in 2214 of 1.16 M samples at
/// 0 us thread life, 1946 / 1.26 M at 50 us, 472 / 1.25 M at 500 us, while the kernel's Threads: count was >= 2 in every
/// one of them): the precondition "the churner lives" failed spuriously (and the survivor check could miss a survivor).
/// Now the kernel's ATOMIC count (status `Threads:`, signal->nr_threads), the same proof the product uses.
fn alive_threads(pid: u32) -> usize {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("Threads:"))
                .and_then(|v| v.trim().parse().ok())
        })
        .unwrap_or(0)
}

fn kill_pid(pid: u32) {
    let _ = Command::new("sh")
        .args(["-c", &format!("kill -KILL {pid} 2>/dev/null")])
        .env("PATH", path_env())
        .status();
}

#[test]
// procfix7 round 5: un-ignored as FIXED (f8999046); FAILS on 9876cba2 (G9-B2); skips without a C compiler
fn g9_b2_zombie_leader_thread_churn_never_survives_an_exit_0() {
    if !rustred_campaign::launch::in_private_namespace() {
        eprintln!(
            "SKIPPED: spawns and signals processes: only inside a private pid namespace (rule L10)"
        );
        return;
    }
    let _g = serial();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/gate9_churn.c");
    let probe = Tmp::new("g9-churn-cc");
    let bin = probe.path.join("g9churn");
    let built = Command::new("cc")
        .args(["-O1", "-o"])
        .arg(&bin)
        .arg(&src)
        .arg("-lpthread")
        .env("PATH", path_env())
        .status();
    let so = probe.path.join("g9fault.so");
    let built2 = Command::new("cc")
        .args(["-shared", "-fPIC", "-O1", "-o"])
        .arg(&so)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/gate9_g9fault.c"))
        .arg("-ldl")
        .arg("-lpthread")
        .env("PATH", path_env())
        .status();
    if !built.is_ok_and(|s| s.success()) || !built2.is_ok_and(|s| s.success()) {
        eprintln!("SKIPPED: no C compiler for the churner / the injector");
        return;
    }
    let so_s = so.to_str().unwrap().to_string();
    for life_us in [500u32, 50, 0] {
        let t = Tmp::new(&format!("g9-churn-{life_us}"));
        let rec = t.path.join("churn.pid");
        let wrap = t.path.join("wrap.sh");
        std::fs::write(
            &wrap,
            format!(
                "#!/bin/sh\nif [ \"$1\" = job ] && [ ! -e {rec} ]; then {bin} {rec} {life_us}; fi\nexec {fake} \"$@\"\n",
                rec = rec.display(),
                bin = bin.display(),
                fake = FAKE
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&wrap, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut v = spec(
            &t,
            serde_json::json!({"items": 4, "nodes": 1, "rungs": 1, "seed": 5, "case_ms": [1500, 2500],
                "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0, "deps": 0.0}),
            serde_json::json!({"max_jobs": 2}),
        );
        v["programs"]["rr_fake_child"]["path"] = serde_json::json!(wrap);
        let sp = write_spec(&t, "spec.json", &v);
        let d = t.cdir("c");
        let mut s1 = start(&sp, &d, &["--plain"], &[]);
        wait_until("the churner", Duration::from_secs(60), || {
            std::fs::read_to_string(&rec).is_ok_and(|s| s.ends_with('\n'))
        });
        std::thread::sleep(Duration::from_millis(500));
        signal(&s1, libc::SIGKILL);
        let _ = s1.wait();
        let cp: u32 = std::fs::read_to_string(&rec)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(alive_threads(cp) >= 2, "precondition: the churner lives");
        let m = format!("/proc/{cp}/task/");
        let env = [
            ("LD_PRELOAD", so_s.as_str()),
            ("G8_MATCH", m.as_str()),
            ("G8_OPS", "open"),
            ("G8_ERRNO", "delay"),
            ("G8_DELAY_US", "2000"),
            ("G8_FROM", "1"),
        ];
        let mut s2 = start(&sp, &d, &["--plain"], &env);
        let code = wait(&mut s2, Duration::from_secs(900));
        let survivor = alive_threads(cp) >= 2;
        kill_pid(cp);
        assert!(
            !(code == 0 && survivor),
            "life {life_us} us: exit 0 with the crashed session's live marked churner {cp}: {}",
            log_tail(&d)
        );
    }
}
