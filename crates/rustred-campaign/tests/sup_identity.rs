//! The campaign id as the only proof of ownership (gate re-audit C-1), the supervisor's lack of a signal path of its
//! own (M-1, grep tripwire over the supervisor's sources), and the pidfd probe that never falls back on a transient
//! failure (m-2). Spawning tests run inside the private pid namespace (rule L10).

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::{Event, JobEvent};
use rustred_campaign::state::FinishClass;
use rustred_campaign::supervisor::identity;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

#[test]
fn copied_campaign_directory_signals_nothing_of_the_original() {
    let _g = serial();
    let t = Tmp::new("copy");
    let syn = serde_json::json!({"items": 8, "case_ms": [6000, 9000], "seed": 5, "p_mem": 0.0, "p_crash": 0.0});
    let spec_p = write_spec(&t, "spec.json", &spec(&t, syn, serde_json::json!({})));
    let a_dir = t.cdir("orig");
    let mut a = start(&spec_p, &a_dir, &[], &[]);
    wait_until("original's jobs running", Duration::from_secs(120), || {
        a_dir.join("ledger").exists()
            && count(&a_dir, |e| {
                matches!(
                    e,
                    Event::Job {
                        ev: JobEvent::Started { .. },
                        ..
                    }
                )
            }) >= 3
    });
    // Copy the live directory (as a backup / restore or an operator's cp -a would).
    let b_dir = t.cdir("copy");
    let st = Command::new("cp")
        .arg("-a")
        .arg(&a_dir)
        .arg(&b_dir)
        .status()
        .unwrap();
    assert!(st.success());
    let a_id = identity::recorded_id(&state(&a_dir)).unwrap();
    let a_live_before: Vec<u32> = marked(&t.campaign);
    assert!(!a_live_before.is_empty());
    // 1) The copied LOCK.json names the live original: refused by the lock (exit 1), nothing signalled.
    let mut b = start(&spec_p, &b_dir, &[], &[]);
    assert_eq!(
        wait(&mut b, Duration::from_secs(60)),
        1,
        "{}",
        log_tail(&b_dir)
    );
    // 2) Without the lock record (a restore without it): the marker id differs -> refused, exit 7.
    let _ = std::fs::remove_file(b_dir.join("LOCK.json"));
    let mut b = start(&spec_p, &b_dir, &[], &[]);
    assert_eq!(
        wait(&mut b, Duration::from_secs(60)),
        7,
        "{}",
        log_tail(&b_dir)
    );
    assert!(
        log_tail(&b_dir).contains("copied, moved or restored"),
        "{}",
        log_tail(&b_dir)
    );
    let b_id = identity::campaign_marker_id(&b_dir, &t.campaign).unwrap();
    assert_ne!(a_id, b_id);
    // 3) With --adopt-moved-campaign: the recorded live attempts become lost (not signalled) and are redone here.
    let mut b = start(&spec_p, &b_dir, &["--adopt-moved-campaign"], &[]);
    wait_until("copy's recovery", Duration::from_secs(60), || {
        count(
            &b_dir,
            |e| matches!(e, Event::Job { ev: JobEvent::Requeue { reason }, .. } if reason.contains("moved")),
        ) >= 1
    });
    // The original's children are untouched: still alive (cases last 6-9 s; checked right after the recovery).
    for p in &a_live_before {
        let alive = std::fs::read_to_string(format!("/proc/{p}/stat"))
            .map(|s| !s[s.rfind(')').unwrap() + 2..].starts_with('Z'))
            .unwrap_or(false);
        let still_a = std::fs::read(format!("/proc/{p}/environ"))
            .map(|e| e.windows(a_id.len()).any(|w| w == a_id.as_bytes()))
            .unwrap_or(false);
        assert!(!still_a || alive, "original's process {p} was disturbed");
    }
    assert_eq!(
        wait(&mut a, Duration::from_secs(600)),
        0,
        "{}",
        log_tail(&a_dir)
    );
    assert_eq!(
        wait(&mut b, Duration::from_secs(600)),
        0,
        "{}",
        log_tail(&b_dir)
    );
    // No attempt of the original ended by a signal it did not expect (a kill by the copy would show as signal 9).
    let killed = count(&a_dir, |e| {
        matches!(
            e,
            Event::Job {
                ev: JobEvent::Finished {
                    signal: Some(_),
                    ..
                },
                ..
            }
        ) || matches!(
            e,
            Event::Job {
                ev: JobEvent::Finished {
                    class: FinishClass::Lost,
                    ..
                },
                ..
            }
        )
    });
    assert_eq!(killed, 0, "{}", log_tail(&a_dir));
    assert_eq!(result(&a_dir).accepted, result(&b_dir).accepted);
    assert_eq!(identity::recorded_id(&state(&b_dir)).unwrap(), b_id);
    assert_eq!(verify(&a_dir), 0);
    assert_eq!(verify(&b_dir), 0);
    assert_no_marked(&t.campaign);
}

#[test]
fn bad_campaign_id_is_refused_at_start() {
    // Process layer: an invalid id is a hard error with try_with_config (never a silent ad-hoc id).
    let sampler = std::sync::Arc::new(
        rustred_campaign::resources::Sampler::spawn(rustred_campaign::resources::SamplerConfig {
            interval: Duration::from_millis(500),
            collect_foreign_pinning: false,
        })
        .unwrap(),
    );
    for bad in ["bad@id", "", "a b", &"x".repeat(65)] {
        let cfg = rustred_campaign::launch::RealConfig {
            campaign_id: bad.to_string(),
            ..Default::default()
        };
        assert!(
            rustred_campaign::launch::RealLauncher::try_with_config(sampler.clone(), cfg).is_err(),
            "{bad:?} accepted"
        );
    }
    // The supervisor derives a valid id (charset, length) from any spec campaign name and the directory.
    let t = Tmp::new("badid");
    let d = t.cdir("c");
    std::fs::create_dir_all(&d).unwrap();
    let id = identity::campaign_marker_id(&d, &"y".repeat(90)).unwrap();
    rustred_campaign::resources::validate_campaign_id(&id).unwrap();
    assert_eq!(
        id,
        identity::campaign_marker_id(&d, &"y".repeat(90)).unwrap(),
        "stable across starts"
    );
    // A spec whose campaign name is outside the charset is refused at start (exit 20).
    let mut v = spec(&t, serde_json::json!({"items": 2}), serde_json::json!({}));
    v["campaign"] = "bad@id".into();
    let p = write_spec(&t, "bad.json", &v);
    let (code, _) = cli(&[
        "run",
        "--spec",
        &p.to_string_lossy(),
        "--dir",
        &d.to_string_lossy(),
    ]);
    assert_eq!(code, 20);
}

/// Gate re-audit M-1: the supervisor, CLI, adapters and the rr_campaign binary contain no signal-capable call and no
/// catch-all reaping (assumption 4): everything goes through the process layer.
#[test]
fn supervisor_sources_have_no_signal_path_of_their_own() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![src.join("bin/rr_campaign.rs")];
    for d in ["supervisor", "cli", "adapters"] {
        for e in std::fs::read_dir(src.join(d)).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "rs") {
                files.push(p);
            }
        }
    }
    let forbidden = [
        "libc::kill",
        "kill(",
        "killpg",
        "tgkill",
        "tkill",
        "SYS_kill",
        "pidfd_send_signal",
        ".kill()",
        "raise(",
        "cgroup.kill",
        "waitpid",
        "wait4",
        "waitid",
        "SIGCHLD",
        "SIG_IGN",
        "\"pkill",
        "\"kill\"",
        "systemctl",
        "\"timeout\"",
    ];
    let mut report = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap();
        for (i, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for pat in forbidden {
                // `kill_orphan(` and `kill_now(` are the process layer's verified API.
                let cleaned = code.replace("kill_orphan(", "").replace("kill_now(", "");
                if cleaned.contains(pat) {
                    report.push(format!(
                        "{}:{}: [{pat}] {}",
                        f.display(),
                        i + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    for r in &report {
        println!("GREP {r}");
    }
    println!("GREP checked {} files", files.len());
    assert!(
        report.is_empty(),
        "signal-capable or reaping calls outside the process layer"
    );
}

/// Gate re-audit m-2: with file descriptors exhausted the pidfd probe reports a TRANSIENT failure (not cached), and a
/// verified signal attempt refuses (Unverified) instead of falling back to kill; afterwards pidfd works again.
#[test]
fn pidfd_probe_transient_failure_never_falls_back() {
    if std::env::var("RR_SUP_HELPER").as_deref() == Ok("nofile") {
        nofile_helper();
        return;
    }
    let exe = std::env::current_exe().unwrap();
    let out = Command::new(exe)
        .args([
            "--exact",
            "pidfd_probe_transient_failure_never_falls_back",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("RR_SUP_HELPER", "nofile")
        .output()
        .unwrap();
    let text =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{text}");
    assert!(
        text.contains("HELPER transient=yes refused=yes recovered=yes"),
        "{text}"
    );
}

fn nofile_helper() {
    use rustred_campaign::launch::{PidfdProbe, pidfd_probe};
    // SAFETY: plain getrlimit / setrlimit on this helper process only.
    unsafe {
        let mut r: libc::rlimit = std::mem::zeroed();
        libc::getrlimit(libc::RLIMIT_NOFILE, &mut r);
        r.rlim_cur = 64.min(r.rlim_max);
        libc::setrlimit(libc::RLIMIT_NOFILE, &r);
    }
    let mut hold = Vec::new();
    while let Ok(f) = std::fs::File::open("/dev/null") {
        hold.push(f);
    }
    let transient = matches!(pidfd_probe(), PidfdProbe::Transient(_));
    // A verified signal now must be refused, not sent by plain kill (target: our own child, which must survive).
    let refused = {
        let v = rustred_campaign::launch::kill_orphan(
            "nobody.0000000000000000",
            &rustred_campaign::resources::boot_id().unwrap_or_default(),
            &rustred_campaign::resources::MarkedProcess {
                pid: 2,
                start_ticks: 0,
                attempt_id: "x".into(),
                ppid: 0,
                pgrp: 0,
                session: 0,
                comm: String::new(),
                state: 'S',
                rss_bytes: 0,
                cpu_ticks: 0,
            },
            0,
        );
        !v.delivered()
    };
    drop(hold);
    let recovered = pidfd_probe() == PidfdProbe::Supported;
    println!(
        "HELPER transient={} refused={} recovered={}",
        if transient { "yes" } else { "no" },
        if refused { "yes" } else { "no" },
        if recovered { "yes" } else { "no" }
    );
}
