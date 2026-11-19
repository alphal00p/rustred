//! Lane procfix7 round 5 (gate9 minor, lock.rs:457-461): the takeover's token scan treated ANY environ read error as
//! "not a holder" (the original ledger_lock test failed 1/200 on a child read in its exec window; the amended one hid
//! it). The rule now (`lock::scan_token_holders`): a candidate of our real uid whose environ cannot be read is re-read
//! across an exec window, then reported as a POSSIBLE holder, never dropped; a failed listing is an error.
//!
//! Deterministic: the procfs fault layer (test builds) makes EVERY environ read of the token-carrying child fail (EIO);
//! the scan must report it in `possible`, not in `holders`, and never drop it. Its own test binary: the fault layer
//! reads RR_CAMPAIGN_FAILPOINT once per process, set here before the first /proc read of this process.
//! Spawns a process: ONLY inside a private pid namespace (`LIGHT_PIDNS=1`, rule L10).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn an_unreadable_environ_is_a_possible_token_holder_never_dropped() {
    if !rustred_campaign::launch::in_private_namespace() {
        eprintln!("SKIPPED: spawns a process: only inside a private pid namespace (rule L10)");
        return;
    }
    let token = "procfix9tokentest0123456789abcdef";
    let mut child = Command::new("sleep")
        .arg("30")
        .env(rustred_campaign::lock::TOKEN_ENV, token)
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let pid = child.id();
    // test code reads /proc directly (not through the crate): the child exec'd and its environ carries the token
    let t0 = Instant::now();
    while !std::fs::read(format!("/proc/{pid}/environ"))
        .is_ok_and(|b| b.windows(token.len()).any(|w| w == token.as_bytes()))
    {
        assert!(
            t0.elapsed() < Duration::from_secs(10),
            "the child never carried the token"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    // SAFETY: set before the first /proc read of the crate in this process (the fault layer reads it once); this test
    // binary has one test, so no other thread reads the environment concurrently.
    unsafe {
        std::env::set_var(
            "RR_CAMPAIGN_FAILPOINT",
            format!("proc_environ_eio:1,proc_pid:{pid}"),
        )
    };
    let scan = rustred_campaign::lock::scan_token_holders(token).expect("the listing works");
    let _ = child.kill();
    let _ = child.wait();
    assert!(
        !scan.holders.contains(&pid),
        "an environ that was never read cannot prove a holder: {scan:?}"
    );
    assert!(
        scan.possible
            .iter()
            .any(|(p, why)| *p == pid && why.contains("injected")),
        "the child whose environ cannot be read is a POSSIBLE holder (never dropped): {scan:?}"
    );
}
