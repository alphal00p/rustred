//! Lane procfix5 (the fourth gate audit's findings, WS/notes/implementer.gate4.md), at the supervisor level (real
//! `rr_campaign` + `rr_fake_child`). Spawns processes and sends signals: run ONLY inside a private pid namespace
//! (`LIGHT_PIDNS=1 light_build.sh ...`, rule L10).
//! - M1: every signal sent is in the ledger after a crash between its durable intent and the send, or between the send
//!   and its completion line (the gate4 E3 check: daemons that only a signal can end);
//! - M2: every verb refuses to start from an environment carrying the campaign marker (gate4 E11);
//! - m1: a child that deletes `RUN/sup/SPAWN` and escapes cannot act before its spawn is in the ledger (gate4 E4);
//! - m2: `ulimit -f` ends the supervisor with exit 75 (disk full), never by SIGXFSZ;
//! - letter (a): real children are refused without scope containment unless the spec says `containment: marker_only`.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

mod closeloop_support;
#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::{Event, JobEvent};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// A contract program that, for `job`, first starts a daemon carrying the job's marker in its own session (only a signal
/// ends it: `sleep 600`), records its pid and start time, then runs the fake child.
fn daemon_wrapper(dir: &Path) -> PathBuf {
    let p = dir.join("wrap.sh");
    let pids = dir.join("daemons.txt");
    std::fs::write(
        &p,
        format!(
            "#!/bin/sh\nif [ \"$1\" = job ]; then\n  setsid {sleep} 600 </dev/null >/dev/null 2>&1 &\n  d=$!\n  \
             echo \"$d $(cut -d' ' -f22 /proc/$d/stat)\" >> {pids}\nfi\nexec {fake} \"$@\"\n",
            sleep = sleep_bin().display(),
            pids = pids.display(),
            fake = FAKE
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

fn alive_with(pid: u32, start: u64) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| {
            let rest = &s[s.rfind(')')? + 2..];
            let f: Vec<&str> = rest.split_whitespace().collect();
            Some(f.first() != Some(&"Z") && f.get(19)?.parse::<u64>().ok()? == start)
        })
        .unwrap_or(false)
}

/// (pid, start_ticks, signal, by) of every `signal_sent` record.
fn signals(d: &Path) -> Vec<(u32, Option<u64>, i32, String)> {
    records(d)
        .into_iter()
        .filter_map(|r| match r.event {
            Event::SignalSent {
                pid,
                start_ticks,
                signal,
                by,
                ..
            } => Some((pid, start_ticks, signal, by)),
            _ => None,
        })
        .collect()
}

/// gate4 M1 / E3: a crash at `site` (right after a durable intent, or right after the send); the restart records the
/// intent(s) of the dead session; every daemon that vanished (only a signal ends it) has a SIGKILL record with its
/// exact pid and start time; the crash intent itself is recorded as POSSIBLY SENT.
fn crash_around_a_send(site: &str) {
    let _g = serial();
    let t = Tmp::new(&format!("pf5-m1-{}", site.replace([':', '_'], "")));
    let wrap = daemon_wrapper(&t.path);
    let syn = serde_json::json!({"items": 8, "case_ms": [300, 900], "seed": 7, "p_mem": 0.0, "p_crash": 0.0});
    let mut v = spec(&t, syn, serde_json::json!({}));
    v["programs"]["rr_fake_child"]["path"] = serde_json::json!(wrap);
    let spec_p = write_spec(&t, "spec.json", &v);
    let d = t.cdir("c");
    let mut c = start(&spec_p, &d, &[], &[("RR_CAMPAIGN_FAILPOINT", site)]);
    assert_eq!(
        wait(&mut c, Duration::from_secs(300)),
        134,
        "{}",
        log_tail(&d)
    );
    // the dead session's last intent has no completion line
    let log = d.join("log").join("signal-intents.jsonl");
    let (intents, _) = rustred_campaign::launch::read_intent_log(&log).unwrap();
    let open: Vec<_> = intents.iter().filter(|i| i.result.is_none()).collect();
    assert!(
        !open.is_empty(),
        "the crash left an intent without completion"
    );
    let crash_id = open.last().unwrap().line.id.clone();
    let codes = run_to_end(&spec_p, &d, &[]);
    assert_eq!(codes.last(), Some(&0), "{codes:?} {}", log_tail(&d));
    let sig = signals(&d);
    assert!(
        sig.iter().any(|(_, _, _, by)| by.contains(&format!(
            "[intent {crash_id}; recovered at restart: POSSIBLY SENT"
        ))),
        "the crash intent {crash_id} is recorded as possibly sent: {sig:?}"
    );
    assert!(
        notes(&d, "signal_intents_reconciled").len() == 1,
        "one reconciliation note"
    );
    // E3: every vanished daemon has its SIGKILL in the ledger (pid AND start time)
    let daemons = std::fs::read_to_string(t.path.join("daemons.txt")).unwrap_or_default();
    let mut n = 0;
    for l in daemons.lines() {
        let mut w = l.split_whitespace();
        let (Some(p), Some(s)) = (w.next(), w.next()) else {
            continue;
        };
        let (pid, st): (u32, u64) = (p.parse().unwrap(), s.parse().unwrap());
        n += 1;
        assert!(!alive_with(pid, st), "daemon {pid} still alive");
        assert!(
            sig.iter()
                .any(|(p, s, g, _)| *p == pid && *s == Some(st) && *g == libc::SIGKILL),
            "daemon {pid} (start {st}) vanished without a SIGKILL record: {sig:?}"
        );
    }
    assert!(n >= 2, "daemons started: {n}");
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}

#[test]
fn m1_crash_between_the_durable_intent_and_the_send() {
    crash_around_a_send("signal_after_intent:3");
}

#[test]
fn m1_crash_between_the_send_and_its_completion() {
    crash_around_a_send("signal_after_send:3");
}

/// gate4 M2 / E11: every verb refuses to start (exit 1, a message naming the variable) when the marker variable is in
/// its own environment; nothing is created.
#[test]
fn m2_every_verb_refuses_a_marked_environment() {
    let _g = serial();
    let t = Tmp::new("pf5-m2");
    let syn = serde_json::json!({"items": 2});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let d = t.cdir("c");
    let ds = d.to_str().unwrap();
    let sp = spec_p.to_str().unwrap();
    let verbs: Vec<Vec<&str>> = vec![
        vec!["run", "--spec", sp, "--dir", ds, "--plain"],
        vec!["preflight", "--spec", sp, "--dir", ds],
        vec!["watch", ds, "--plain"],
        vec!["dashboard", ds],
        vec!["status", ds, "--once"],
        vec!["control", ds, "stop"],
        vec!["verify", ds],
        vec!["result", ds],
        vec!["rebuild-manifest", ds],
        vec!["help"],
        vec![],
    ];
    for args in verbs {
        for value in ["sup-x-1234567890abcdef:N0.i0000.L0-t0:a1", ""] {
            let o = Command::new(SUP)
                .args(&args)
                .env("RR_CAMPAIGN_JOB", value)
                .stdin(Stdio::null())
                .output()
                .unwrap();
            let err = String::from_utf8_lossy(&o.stderr);
            assert_eq!(o.status.code(), Some(1), "{args:?}: {err}");
            assert!(
                err.contains("refused") && err.contains("RR_CAMPAIGN_JOB"),
                "{args:?}: {err}"
            );
        }
    }
    assert!(!d.exists(), "nothing created");
    // the same commands without the variable work as before
    let o = Command::new(SUP)
        .arg("help")
        .env_remove("RR_CAMPAIGN_JOB")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
}

/// gate4 m1 / E4: the job's program deletes `RUN/sup/SPAWN`, drops its marker and re-runs itself in a new session. A
/// crash right after the spawn (before `job.started`): behind the spawn barrier the program NEVER ran (no escapee, the
/// sentinel intact); the restart proves "never ran" from the ledger (no `job.started`), records `not_spawned` + requeue,
/// and the second attempt runs alone.
#[test]
fn m1_a_child_cannot_act_before_its_spawn_is_recorded() {
    let _g = serial();
    let t = Tmp::new("pf5-m1b");
    let esc = t.path.join("escapees.txt");
    let wrap = t.path.join("wrap.sh");
    std::fs::write(
        &wrap,
        format!(
            "#!/bin/sh\nif [ \"$1\" = job ]; then\n  case \"$2\" in */a1/job.json|*.a1/job.json)\n    \
             rm -f \"$(dirname \"$2\")/sup/SPAWN\"\n    echo \"$2\" >> {esc}\n    \
             setsid env -u RR_CAMPAIGN_JOB {fake} \"$@\" >/dev/null 2>&1 </dev/null &\n    exit 0;;\n  esac\nfi\n\
             exec {fake} \"$@\"\n",
            esc = esc.display(),
            fake = FAKE
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&wrap, std::fs::Permissions::from_mode(0o755)).unwrap();
    let syn = serde_json::json!({"items": 1, "case_ms": [2000, 2000], "seed": 7, "p_mem": 0.0, "p_crash": 0.0, "p_norule": 0.0});
    let mut v = spec(&t, syn.clone(), serde_json::json!({}));
    let spec_ref = write_spec(&t, "ref.json", &v);
    let r = t.cdir("reference");
    assert_eq!(run_to_end(&spec_ref, &r, &[]), vec![0]);
    v["programs"]["rr_fake_child"]["path"] = serde_json::json!(wrap);
    let spec_p = write_spec(&t, "spec.json", &v);
    let d = t.cdir("c");
    let mut c = start(
        &spec_p,
        &d,
        &[],
        &[("RR_CAMPAIGN_FAILPOINT", "after_spawn:1")],
    );
    assert_eq!(
        wait(&mut c, Duration::from_secs(120)),
        134,
        "{}",
        log_tail(&d)
    );
    std::thread::sleep(Duration::from_secs(1));
    assert!(
        !esc.exists(),
        "the program ran before its spawn was recorded: {:?}",
        std::fs::read_to_string(&esc)
    );
    let st = state(&d);
    let (job, jr) = st.jobs.iter().next().unwrap();
    let run = PathBuf::from(jr.run_dir.clone().unwrap());
    assert_eq!(
        std::fs::read_to_string(run.join("sup").join("SPAWN")).unwrap(),
        "barrier v1\n"
    );
    let codes = run_to_end(&spec_p, &d, &[]);
    assert_eq!(codes, vec![0], "{}", log_tail(&d));
    assert_eq!(
        count(
            &d,
            |e| matches!(e, Event::Job { job: j, attempt: 1, ev: JobEvent::NotSpawned { .. } } if j == job)
        ),
        1
    );
    assert_eq!(
        count(
            &d,
            |e| matches!(e, Event::Job { job: j, attempt: 1, ev: JobEvent::Started { .. } } if j == job)
        ),
        0
    );
    // only attempt 2 ran (its program is the plain fake child: the wrapper misbehaves for a1 only)
    assert!(!esc.exists(), "attempt 1 never ran");
    assert_eq!(result(&d), result(&r));
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}

/// gate4 m2: under `ulimit -f` the supervisor ends with exit 75 (EFBIG = disk full), never killed by SIGXFSZ (153);
/// the same command without the limit finishes equal to the reference.
#[test]
fn m2_file_size_limit_is_disk_full_never_sigxfsz() {
    let _g = serial();
    let t = Tmp::new("pf5-m2f");
    let syn = serde_json::json!({"items": 30, "seed": 3});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let r = t.cdir("reference");
    let ref_codes = run_to_end(&spec_p, &r, &[]);
    let d = t.cdir("c");
    let log = std::fs::File::create(t.path.join("limited.log")).unwrap();
    let mut cmd = Command::new(SUP);
    cmd.args(["run", "--spec"])
        .arg(&spec_p)
        .arg("--dir")
        .arg(&d)
        .arg("--plain")
        .env_remove("RR_CAMPAIGN_FAILPOINT")
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log);
    use std::os::unix::process::CommandExt;
    // SAFETY: setrlimit is async-signal-safe; plain integers.
    unsafe {
        cmd.pre_exec(|| {
            let r = libc::rlimit {
                rlim_cur: 48 << 10,
                rlim_max: 48 << 10,
            };
            libc::setrlimit(libc::RLIMIT_FSIZE, &r);
            Ok(())
        });
    }
    let mut c = cmd.spawn().unwrap();
    let code = wait(&mut c, Duration::from_secs(600));
    let txt = std::fs::read_to_string(t.path.join("limited.log")).unwrap_or_default();
    assert_ne!(code, 128 + libc::SIGXFSZ, "killed by SIGXFSZ: {txt}");
    assert_eq!(code, 75, "{txt}");
    let codes = run_to_end(&spec_p, &d, &[]);
    assert_eq!(codes.last(), ref_codes.last());
    assert_eq!(result(&d), result(&r));
    assert_eq!(verify(&d), 0);
    assert_no_marked(&t.campaign);
}

/// gate4 letter (a), item 5: inside a pid namespace there is no systemd scope; a campaign whose program is NOT the fake
/// child (its `version --json` names another program) is refused (exit 1) unless `resources.containment` is
/// `"marker_only"`; an unknown containment value is refused. (The toy closeloop campaign; its program wraps the fake child
/// for jobs.)
#[test]
fn real_children_need_scope_containment_unless_marker_only() {
    if !rustred_campaign::launch::in_private_namespace() {
        eprintln!("SKIPPED: only meaningful inside a private namespace (no systemd scope there)");
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf5-cont");
    let real = t.path.join("realish.sh");
    std::fs::write(
        &real,
        format!(
            "#!/bin/sh\nif [ \"$1\" = version ]; then echo '{{\"program\":\"rr_node_stand_in\"}}'; exit 0; fi\nexec {} \"$@\"\n",
            FAKE
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o755)).unwrap();
    let r = real.to_str().unwrap();
    let files = closeloop_support::toy(&t.path.join("m"), &closeloop_support::Toy::default(), r);
    let base = closeloop_support::campaign_spec(&t.campaign, &files, r, &cpus());
    // default (scope): refused, nothing launched
    let p1 = write_spec(&t, "s1.json", &base);
    let d1 = t.cdir("refused");
    let mut c = start(&p1, &d1, &["--plain"], &[]);
    assert_eq!(
        wait(&mut c, Duration::from_secs(120)),
        1,
        "{}",
        log_tail(&d1)
    );
    assert!(
        log_tail(&d1).contains("real children need scope containment"),
        "{}",
        log_tail(&d1)
    );
    assert!(
        !d1.join("jobs").exists() || std::fs::read_dir(d1.join("jobs")).unwrap().next().is_none(),
        "nothing launched"
    );
    // an unknown value: refused
    let mut bad = base.clone();
    bad["resources"]["containment"] = serde_json::json!("cgroupish");
    let p2 = write_spec(&t, "s2.json", &bad);
    let d2 = t.cdir("bad");
    let mut c = start(&p2, &d2, &["--plain"], &[]);
    assert_eq!(
        wait(&mut c, Duration::from_secs(120)),
        1,
        "{}",
        log_tail(&d2)
    );
    // explicit marker_only: runs (warning logged)
    let mut ok = base.clone();
    ok["resources"]["containment"] = serde_json::json!("marker_only");
    let p3 = write_spec(&t, "s3.json", &ok);
    let d3 = t.cdir("marker_only");
    let codes = run_to_end(&p3, &d3, &["--plain"]);
    assert!(
        finished(*codes.last().unwrap()),
        "{codes:?} {}",
        log_tail(&d3)
    );
    assert!(
        std::fs::read_to_string(d3.join("log").join("supervisor.log"))
            .unwrap()
            .contains("containment: marker only"),
        "the operator is told"
    );
    assert_no_marked(&t.campaign);
}
