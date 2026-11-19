//! Lane procfix7 round 9 (gate13 G13-M1): the evidence a job that dies before its first check is recorded from, checked
//! against the REAL systemd-run of the host (systemd 261.1 here; gate13 found its start acknowledgment is
//! `Running as unit: <unit>.scope; invocation ID: <id>`, which the round-8b code never matched).
//!
//! HOST ONLY, ignored by default, never part of a suite. It runs `rr_campaign scope-check --unit <exact name>` (the host
//! step's pre-flight; `RR_SCOPE_CHECK_BIN` may name another rr_campaign binary, e.g. a frozen release): ONE transient
//! scope `systemd-run --user --scope --unit=<name> -p MemoryMax=64M -p MemorySwapMax=0 -p OOMPolicy=kill -- true`, the
//! kernel's record of the cgroup of its process at the exit (read from the unreaped zombie: at once, and after systemd
//! removed the unit, when it must read ` (deleted)`), systemd-run's acknowledgment in a documented form naming the exact
//! unit, nothing left loaded. What runs outside a pid namespace: this test binary (this one test only), the rr_campaign
//! `scope-check` verb (no supervisor, launcher, sampler or sweep; its code path holds no signal call), `systemctl --user
//! show` reads, `systemd-run --user --scope` and `true`. Nothing can be signalled: no kill / killpg / pidfd_send_signal
//! is reached; a scope that hangs fails the check and is left to end by itself. Rule L10 exception (the brief of round
//! 9), described in WS/notes/implementer.procfix7.md before it was run.
//!
//! Run exactly once, from the lane worktree, WITHOUT LIGHT_PIDNS:
//!   light_build.sh procfix7 test -p rustred-campaign --test procfix13_real_systemd -- --ignored --exact \
//!     real_systemd_start_acknowledgment_and_kernel_record_name_the_exact_unit
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use std::path::PathBuf;
use std::process::Command;

#[test]
#[ignore = "host only: starts ONE transient systemd scope around `true` (run by hand, see the module doc)"]
fn real_systemd_start_acknowledgment_and_kernel_record_name_the_exact_unit() {
    if rustred_campaign::launch::in_private_namespace() {
        eprintln!("SKIPPED: inside a private namespace there is no systemd scope (host only)");
        return;
    }
    let bin = std::env::var_os("RR_SCOPE_CHECK_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_rr_campaign")));
    let unit = format!(
        "procfix7-ack-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );
    let out = Command::new(&bin)
        .args(["scope-check", "--unit", &unit])
        .env_remove("RR_CAMPAIGN_JOB")
        .output()
        .expect("rr_campaign scope-check");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    eprintln!(
        "{} scope-check --unit {unit}: {}\n{text}{}",
        bin.display(),
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "scope-check failed:\n{text}");
    let lines: Vec<&str> = text.lines().collect();
    let has = |p: &str| lines.iter().any(|l| l.contains(p));
    assert!(has(": PASS"), "{text}");
    assert!(
        has(&format!(
            "start acknowledgment naming exactly {unit}.scope: form "
        )),
        "{text}"
    );
    assert!(
        has(&format!("{unit}.scope (deleted)\" -> Inside")),
        "the kernel's record after the unit was removed names it, (deleted): {text}"
    );
    assert!(
        has(&format!(
            "{unit}.scope after the check: Ok(ScopeUnitState {{ load: \"not-found\""
        )),
        "{text}"
    );
}
