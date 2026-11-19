//! Lane procfix7 round 5 (gate9 minor, "mutations not caught"): taking an UNREADABLE lock owner as dead was not caught
//! by any test. The rule (lock.rs `liveness`, procfix8): only ENOENT / ESRCH of the owner's stat (or another start time,
//! another boot) is "dead"; an unreadable stat is `Liveness::Unknown` and `acquire` refuses typed (LockError::Io), never
//! a takeover. This test makes EVERY stat read of the recorded owner fail (procfs fault layer, test builds) for a LIVE
//! owner process: `acquire` must refuse with the liveness reason; a mutation that maps Unknown to Dead turns it into a
//! takeover and fails here. Its own test binary (the fault layer reads RR_CAMPAIGN_FAILPOINT once per process).
//! Spawns a process: ONLY inside a private pid namespace (`LIGHT_PIDNS=1`, rule L10).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use rustred_campaign::lock::{self, LockError, OwnerRecord, ProcessIdentity};
use std::process::{Command, Stdio};

#[test]
fn an_unreadable_owner_is_never_taken_over() {
    if !rustred_campaign::launch::in_private_namespace() {
        eprintln!("SKIPPED: spawns a process: only inside a private pid namespace (rule L10)");
        return;
    }
    let dir = std::env::temp_dir().join(format!("pf9-lockowner-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut owner = Command::new("sleep")
        .arg("30")
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let pid = owner.id();
    // test code reads /proc directly: the owner's real identity (alive, this boot)
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    let start_ticks: u64 = stat[stat.rfind(')').unwrap() + 2..]
        .split_whitespace()
        .nth(19)
        .unwrap()
        .parse()
        .unwrap();
    let boot_id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .unwrap()
        .trim()
        .to_string();
    let rec = OwnerRecord {
        schema: lock::LOCK_SCHEMA.into(),
        identity: ProcessIdentity {
            pid,
            start_ticks,
            boot_id,
            host: lock::hostname().unwrap(),
        },
        argv: vec!["recorded-owner".into()],
        started_utc: "2026-11-16T00:00:00Z".into(),
        token: "0123456789abcdef0123456789abcdef".into(),
    };
    std::fs::write(
        dir.join(lock::OWNER_FILE),
        serde_json::to_vec(&rec).unwrap(),
    )
    .unwrap();
    // SAFETY: set before the first /proc read of the crate in this process (one test in this binary)
    unsafe {
        std::env::set_var(
            "RR_CAMPAIGN_FAILPOINT",
            format!("proc_stat_eio:1,proc_pid:{pid}"),
        )
    };
    let r = lock::acquire(
        &dir,
        &rustred_campaign::clock::SystemClock::new(),
        &["successor".into()],
    );
    let _ = owner.kill();
    let _ = owner.wait();
    match r {
        Err(LockError::Io { source, .. }) => assert!(
            source.to_string().contains("cannot be read"),
            "the refusal names the unreadable liveness: {source}"
        ),
        Err(e) => panic!("expected the typed liveness refusal, got {e}"),
        Ok((_, out)) => panic!("an owner whose liveness cannot be read was TAKEN OVER: {out:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}
