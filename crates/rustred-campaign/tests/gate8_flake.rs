//! gate8 audit (WS/notes/implementer.gate8.md), letter (i): the cause of the intermittent failure of
//! ledger_lock::crashed_supervisor_with_live_child_is_taken_over_and_recorded ("the child carries the token": [] for a
//! live child). std's `Command::spawn` returns when the child's CLOEXEC error pipe closes, i.e. at the close-on-exec
//! step of execve, BEFORE the kernel has set the new image's environment bounds (create_elf_tables): in that window
//! `/proc/<pid>/environ` reads EMPTY although the process is alive and will carry the token. Under a CPU quota the
//! window stretches. This test spawns children exactly as the lock helper does and counts how often the token scan
//! right after `spawn()` misses a live child (and finds it a moment later). Spawns processes: run ONLY inside a private
//! pid namespace (LIGHT_PIDNS=1). Ignored: run with `--ignored` under a CPU quota (e.g. 30 %).
//! `GATE8_TRIALS` (default 2000) sets the number of spawns. It asserts only that every missed child was found later
//! (the window closes): a miss is the mechanism, not a failure.

use rustred_campaign::lock;
use std::process::{Command, Stdio};
use std::time::Duration;

#[test]
#[ignore = "gate8 (i): exec-window probe; spawns processes (namespace only), run under a CPU quota"]
fn g8_token_scan_right_after_spawn_can_miss_a_live_child() {
    let n: usize = std::env::var("GATE8_TRIALS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);
    let (mut empty_at_once, mut missed, mut found_later) = (0usize, 0usize, 0usize);
    for i in 0..n {
        let tok = format!("g8tok{}x{i}", std::process::id());
        let mut c = Command::new("sleep")
            .arg("30")
            .env(lock::TOKEN_ENV, &tok)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let pid = c.id();
        if std::fs::read(format!("/proc/{pid}/environ")).is_ok_and(|b| b.is_empty()) {
            empty_at_once += 1;
        }
        if !lock::scan_token_holders(&tok).contains(&pid) {
            missed += 1;
            std::thread::sleep(Duration::from_millis(50));
            if lock::scan_token_holders(&tok).contains(&pid) {
                found_later += 1;
            }
        }
        let _ = c.kill();
        let _ = c.wait();
    }
    println!(
        "gate8 exec window: trials {n}, environ empty right after spawn {empty_at_once}, token scan missed the live child {missed}, found 50 ms later {found_later}"
    );
    assert_eq!(missed, found_later, "a missed child never showed its token");
}
