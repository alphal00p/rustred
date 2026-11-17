//! Lane procfix7 round 9 (gate13 G13-M2): the scope of EVERY ended attempt is read by its exact unit name, its state is
//! RECORDED in the ledger (note `scope_result`) BEFORE a failed one is reset (`systemctl --user reset-failed --
//! <unit>.scope`), WHATEVER ended the job: also the ends killed by our own memory sampler (kill reason MemoryCap; host
//! step 1's nine killed_memory ends had failed oom-kill scopes that the round-8b code never reset). A unit that is not in
//! a final state is never reset; it is looked at again at the exit.
//!
//! Inside a private pid namespace there is no systemd: the test-build failpoint `fake_systemd:1` lets the unit queries
//! run there against a FAKE `systemctl` first on the supervisor's PATH. The fake logs every call; `show` prints the
//! state of the case; on `reset-failed` it counts the `scope_result` notes already in the ledger and logs
//! `recorded-before-reset` when there is one for every reset so far (else `NOT-RECORDED-before-reset`). The failpoint
//! `scope_pending:1` gives every job the requested unit `rr-fp-pending`.
//!
//! Spawns processes and sends signals: ONLY inside a private pid namespace (`LIGHT_PIDNS=1 light_build.sh ...`, rule L10).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::{Event, JobEvent};
use rustred_campaign::state::FinishClass;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn in_ns() -> bool {
    if rustred_campaign::launch::in_private_namespace() {
        return true;
    }
    eprintln!(
        "SKIPPED: spawns and signals processes: only inside a private pid namespace (rule L10)"
    );
    false
}

fn tool(name: &str) -> PathBuf {
    std::env::var_os("PATH")
        .and_then(|p| {
            std::env::split_paths(&p)
                .map(|d| d.join(name))
                .find(|f| f.is_file())
        })
        .unwrap_or_else(|| panic!("{name} not on PATH"))
}

/// The fake `systemctl` in `<tmp>/fakebin`; returns that directory. `state`: the `show` answer.
fn fake_systemctl(t: &Tmp, cdir: &Path, state: &str) -> PathBuf {
    let bin = t.path.join("fakebin");
    std::fs::create_dir_all(&bin).unwrap();
    let log = t.path.join("systemctl.log");
    let st = t.path.join("state");
    std::fs::write(&st, state).unwrap();
    let (sh, cat, grep, tr) = (tool("sh"), tool("cat"), tool("grep"), tool("tr"));
    let ledger = cdir.join("ledger");
    let script = format!(
        r#"#!{sh}
# fake systemctl (procfix13_scope_settle): the supervisor clears the environment, hence absolute paths
echo "$*" >> {log}
case "$2" in
  show) {cat} {st} ;;
  reset-failed)
    notes=$({cat} {ledger}/segment-*.jsonl | {grep} -c 'scope_result ')
    resets=$({grep} -c '^--user reset-failed' {log} | {tr} -d ' ')
    if [ "$notes" -ge "$resets" ]; then echo "recorded-before-reset $4 notes=$notes resets=$resets" >> {log};
    else echo "NOT-RECORDED-before-reset $4 notes=$notes resets=$resets" >> {log}; fi ;;
esac
exit 0
"#,
        sh = sh.display(),
        log = log.display(),
        cat = cat.display(),
        st = st.display(),
        grep = grep.display(),
        ledger = ledger.display(),
        tr = tr.display(),
    );
    let p = bin.join("systemctl");
    std::fs::write(&p, script).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

fn finished_classes(d: &Path) -> Vec<FinishClass> {
    records(d)
        .into_iter()
        .filter_map(|r| match r.event {
            Event::Job {
                ev: JobEvent::Finished { class, .. },
                ..
            } => Some(class),
            _ => None,
        })
        .collect()
}

fn run_case(tag: &str, state: &str, synthetic: serde_json::Value) -> (i32, PathBuf, Tmp, String) {
    let t = Tmp::new(tag);
    let d = t.cdir("c");
    let bin = fake_systemctl(&t, &d, state);
    let mut v = spec(&t, synthetic, serde_json::json!({"max_jobs": 2}));
    v["synthetic"]["mem_cap_mb"] = serde_json::json!(40);
    let sp = write_spec(&t, "spec.json", &v);
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut c = start(
        &sp,
        &d,
        &[],
        &[
            ("RR_CAMPAIGN_FAILPOINT", "scope_pending:1,fake_systemd:1"),
            ("PATH", &path),
        ],
    );
    let code = wait(&mut c, Duration::from_secs(600));
    let log = std::fs::read_to_string(t.path.join("systemctl.log")).unwrap_or_default();
    (code, d, t, log)
}

/// Failed scopes: every ended attempt's state is recorded, then reset, whatever ended the job: two campaigns, one whose
/// every job our sampler kills (kill reason MemoryCap: the case the round-8b code skipped) and one whose every job ends
/// normally. A SIGKILL end with no kill reason of ours and result oom-kill would be killed_memory (note
/// `scope_oom_kill`; not exercised: inside a namespace our sampler kills first).
#[test]
fn every_ended_attempts_failed_scope_is_recorded_then_reset_whatever_ended_it() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    for (case, p_mem, want) in [
        ("ours-memorycap", 1.0, FinishClass::KilledMemory),
        ("normal", 0.0, FinishClass::RuleFound),
    ] {
        let (code, d, t, log) = run_case(
            &format!("pf13-settle-failed-{case}"),
            "LoadState=loaded\nActiveState=failed\nResult=oom-kill\n",
            serde_json::json!({"items": 3, "nodes": 1, "rungs": 1, "seed": 5, "case_ms": [300, 600],
                "p_crash": 0.0, "p_mem": p_mem, "p_norule": 0.0, "deps": 0.0}),
        );
        let classes = finished_classes(&d);
        let results = notes(&d, "scope_result ");
        let resets: Vec<&str> = log
            .lines()
            .filter(|l| l.starts_with("--user reset-failed"))
            .collect();
        eprintln!(
            "PF13 settle failed {case}: exit {code}; finished {classes:?}; {} scope_result notes; {} resets; scope_oom_kill {}; log tail:\n{}",
            results.len(),
            resets.len(),
            notes(&d, "scope_oom_kill ").len(),
            log.lines().rev().take(4).collect::<Vec<_>>().join("\n")
        );
        assert_eq!(code, 0, "{case}: {}", log_tail(&d));
        assert_eq!(classes.len(), 3, "{case}: {classes:?}");
        assert!(classes.iter().all(|c| *c == want), "{case}: {classes:?}");
        assert!(
            notes(&d, "scope_oom_kill ").is_empty(),
            "{case}: no end without a kill reason of ours"
        );
        assert_eq!(
            results.len(),
            classes.len(),
            "{case}: one scope_result per ended attempt: {results:?}"
        );
        assert!(
            results.iter().all(|n| n.contains("\"active\":\"failed\"")
                && n.contains("\"result\":\"oom-kill\"")
                && n.contains("\"unit\":\"rr-fp-pending.scope\"")
                && n.contains("\"when\":\"ended\"")),
            "{case}: {results:?}"
        );
        assert_eq!(resets.len(), classes.len(), "{case}: {log}");
        assert!(
            resets
                .iter()
                .all(|l| *l == "--user reset-failed -- rr-fp-pending.scope"),
            "{case}: exact unit name only: {resets:?}"
        );
        assert_eq!(
            log.lines()
                .filter(|l| l.starts_with("recorded-before-reset"))
                .count(),
            classes.len(),
            "{case}: {log}"
        );
        assert!(!log.contains("NOT-RECORDED"), "{case}: {log}");
        assert!(notes(&d, "scope_reset_error ").is_empty());
        assert_no_marked(&t.campaign);
    }
}

/// A unit that never reaches a final state is never reset: read again for up to 1 s per attempt, then at the exit
/// (bounded), and left alone.
#[test]
fn a_scope_not_in_a_final_state_is_never_reset_and_looked_at_again_at_the_exit() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let (code, d, t, log) = run_case(
        "pf13-settle-deact",
        "LoadState=loaded\nActiveState=deactivating\nResult=success\n",
        serde_json::json!({"items": 2, "nodes": 1, "rungs": 1, "seed": 5, "case_ms": [300, 600],
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0, "deps": 0.0}),
    );
    let classes = finished_classes(&d);
    let sup_log = std::fs::read_to_string(d.with_extension("log")).unwrap_or_default();
    eprintln!(
        "PF13 settle deactivating: exit {code}; finished {classes:?}; {} show calls",
        log.lines().filter(|l| l.starts_with("--user show")).count()
    );
    assert_eq!(code, 0, "{}", log_tail(&d));
    assert!(!log.contains("reset-failed"), "{log}");
    assert!(notes(&d, "scope_result ").is_empty());
    assert!(
        sup_log.contains("still deactivating at the exit: left alone"),
        "{}",
        log_tail(&d)
    );
    assert_eq!(classes.len(), 2);
    assert_no_marked(&t.campaign);
}
