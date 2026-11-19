//! gate10 audit (WS/notes/implementer.gate10.md, letter (a)(2)): the fsx site labels that procfix7 round 5 states as
//! NEVER REACHED by its synthetic crash/resume campaign (adapters.outcome, closeloop.input, closeloop.output, ledger.dir,
//! ledger.snapshot, progress, real.program, supervisor.outcome, supervisor.sentinel, tui), each reached here with an
//! injected error (`RR_CAMPAIGN_FAILPOINT=fsx_site:<k>[,fsx_errno:<e>]`, test builds) and its typed end checked.
//!
//! Library-level cases run the test binary itself as a child process (the fault layer reads the variable once per
//! process): `GATE10_INNER=1` selects the inner half of the same test.
//!
//! Spawns processes and sends signals: ONLY inside a private pid namespace (rule L10; `in_ns()` skips otherwise).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

mod closeloop_support;
#[path = "sup_common/mod.rs"]
mod common;
mod ledger_support;

use common::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const INTERNAL: i32 = 70;
const RESOURCES: i32 = 71;

fn in_ns() -> bool {
    if rustred_campaign::launch::in_private_namespace() {
        return true;
    }
    eprintln!(
        "SKIPPED: spawns and signals processes: only inside a private pid namespace (rule L10)"
    );
    false
}

fn site_index(name: &str) -> usize {
    rustred_campaign::fsx::SITES
        .iter()
        .position(|s| *s == name)
        .unwrap_or_else(|| panic!("no fsx site {name}"))
        + 1
}

fn fp_site(name: &str, errno: i32) -> String {
    format!("fsx_site:{},fsx_errno:{errno}", site_index(name))
}

fn run_env(sp: &Path, d: &Path, env: &[(&str, &str)], limit: Duration) -> i32 {
    let mut c = start(sp, d, &["--plain"], env);
    wait(&mut c, limit)
}

fn small_spec(t: &Tmp, items: u64) -> PathBuf {
    let v = spec(
        t,
        serde_json::json!({"items": items, "nodes": 1, "rungs": 1, "seed": 7, "case_ms": [300, 600],
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0, "deps": 0.0}),
        serde_json::json!({"max_jobs": 1, "unproven_end_limit_s": 30}),
    );
    write_spec(t, "spec.json", &v)
}

/// Run the named test of THIS binary as a child with GATE10_INNER=1 and `env`; true when it passed.
fn inner(test: &str, env: &[(&str, &str)]) -> bool {
    let exe = std::env::current_exe().unwrap();
    let mut c = Command::new(exe);
    c.args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env("GATE10_INNER", "1")
        .env_remove("RR_CAMPAIGN_FAILPOINT");
    for (k, v) in env {
        c.env(k, v);
    }
    let out = c.output().unwrap();
    let text =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    eprintln!(
        "--- inner {test} {env:?}:\n{}",
        text.lines()
            .filter(|l| l.contains("G10"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    out.status.success() && text.contains("1 passed")
}

fn is_inner() -> bool {
    std::env::var_os("GATE10_INNER").is_some()
}

// ------------------------------------------------------------------------------------------------ ledger.dir

/// A FRESH start whose ledger-directory check fails: typed stop (EIO 70, EMFILE 71), nothing created; then a clean run
/// finishes.
#[test]
fn g10_site_ledger_dir_fresh_start_is_typed_and_creates_nothing() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("g10-ldir");
    let sp = small_spec(&t, 2);
    let d = t.cdir("c");
    for (errno, want) in [(libc::EIO, INTERNAL), (libc::EMFILE, RESOURCES)] {
        let fp = fp_site("ledger.dir", errno);
        let code = run_env(
            &sp,
            &d,
            &[("RR_CAMPAIGN_FAILPOINT", &fp)],
            Duration::from_secs(120),
        );
        eprintln!("G10 ledger.dir errno {errno}: exit {code}");
        assert_eq!(code, want, "errno {errno}: {}", log_tail(&d));
        let ldir = rustred_campaign::ledger::ledger_dir(&d);
        assert!(
            !ldir.join("MANIFEST.json").exists() && !ldir.join("segment-000001.jsonl").exists(),
            "errno {errno}: a ledger was created"
        );
    }
    let code = run_env(&sp, &d, &[], Duration::from_secs(300));
    assert_eq!(code, 0, "{}", log_tail(&d));
}

// ------------------------------------------------------------------------------------------------ real.program

/// The program's digest check cannot stat the program: typed stop (70), no job launched.
#[test]
fn g10_site_real_program_is_typed() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("g10-prog");
    let sp = small_spec(&t, 2);
    let d = t.cdir("c");
    let fp = fp_site("real.program", libc::EIO);
    let code = run_env(
        &sp,
        &d,
        &[("RR_CAMPAIGN_FAILPOINT", &fp)],
        Duration::from_secs(120),
    );
    eprintln!("G10 real.program: exit {code}");
    assert_ne!(code, 0, "{}", log_tail(&d));
    assert!(
        code == INTERNAL || code == RESOURCES || code == 1,
        "untyped end {code}: {}",
        log_tail(&d)
    );
    let started = count(&d, |e| {
        matches!(
            e,
            rustred_campaign::ledger::Event::Job {
                ev: rustred_campaign::ledger::JobEvent::Started { .. },
                ..
            }
        )
    });
    assert_eq!(
        started, 0,
        "a job was started although the program could not be checked"
    );
}

// ------------------------------------------------------------------------------------------------ supervisor.outcome / .sentinel

/// Session 1 aborts right after the spawn (behind the spawn barrier: the program never ran); session 2's recovery
/// cannot rule out an OUTCOME (injected stat error): EXECUTION UNKNOWN (exit 8), never "not spawned". Control: the same
/// crash recovered cleanly is NotSpawned + requeued, exit 0.
#[test]
fn g10_site_supervisor_outcome_unconfirmed_barrier_launch() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    for inject in [true, false] {
        let t = Tmp::new(&format!("g10-out-{inject}"));
        let sp = small_spec(&t, 2);
        let d = t.cdir("c");
        let c1 = run_env(
            &sp,
            &d,
            &[("RR_CAMPAIGN_FAILPOINT", "after_spawn:1")],
            Duration::from_secs(120),
        );
        assert_ne!(c1, 0, "session 1 must abort");
        let fp = fp_site("supervisor.outcome", libc::EIO);
        let env: Vec<(&str, &str)> = if inject {
            vec![("RR_CAMPAIGN_FAILPOINT", &fp)]
        } else {
            vec![]
        };
        let code = run_env(&sp, &d, &env, Duration::from_secs(300));
        let not_spawned = count(&d, |e| {
            matches!(
                e,
                rustred_campaign::ledger::Event::Job {
                    ev: rustred_campaign::ledger::JobEvent::NotSpawned { .. },
                    ..
                }
            )
        });
        eprintln!(
            "G10 supervisor.outcome inject {inject}: session1 {c1}, exit {code}, not_spawned {not_spawned}"
        );
        if inject {
            assert_eq!(code, ATTENTION, "{}", log_tail(&d));
            assert_eq!(
                not_spawned, 0,
                "an unreadable OUTCOME check decided 'not spawned'"
            );
            assert!(log_tail(&d).contains("yet an OUTCOME exists") || !notes(&d, "").is_empty());
        } else {
            assert_eq!(code, 0, "{}", log_tail(&d));
            assert_eq!(not_spawned, 1, "control: the barrier launch is NotSpawned");
        }
    }
}

/// Session 1 launches WITHOUT the barrier (an older binary's launch: `no_spawn_barrier`) and aborts after the spawn;
/// session 2 cannot check the spawn sentinel (injected stat error): EXECUTION UNKNOWN (exit 8), never "not spawned".
#[test]
fn g10_site_supervisor_sentinel_unconfirmed_nobarrier_launch() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("g10-sent");
    let sp = small_spec(&t, 2);
    let d = t.cdir("c");
    let c1 = run_env(
        &sp,
        &d,
        &[("RR_CAMPAIGN_FAILPOINT", "no_spawn_barrier:1,after_spawn:1")],
        Duration::from_secs(120),
    );
    assert_ne!(c1, 0, "session 1 must abort");
    let fp = fp_site("supervisor.sentinel", libc::EIO);
    let code = run_env(
        &sp,
        &d,
        &[("RR_CAMPAIGN_FAILPOINT", &fp)],
        Duration::from_secs(300),
    );
    let not_spawned = count(&d, |e| {
        matches!(
            e,
            rustred_campaign::ledger::Event::Job {
                ev: rustred_campaign::ledger::JobEvent::NotSpawned { .. },
                ..
            }
        )
    });
    let tail = log_tail(&d);
    eprintln!(
        "G10 supervisor.sentinel: session1 {c1}, exit {code}, not_spawned {not_spawned}, sentinel-unreadable {}",
        tail.contains("spawn sentinel unreadable")
    );
    assert_eq!(code, ATTENTION, "{tail}");
    assert_eq!(
        not_spawned, 0,
        "an unreadable sentinel decided 'not spawned'"
    );
    assert!(
        tail.contains("spawn sentinel unreadable"),
        "the site was not reached: {tail}"
    );
}

// ------------------------------------------------------------------------------------------------ progress

/// `rr_campaign verify` of a finished campaign whose progress files cannot be listed / read: never "verified".
#[test]
fn g10_site_progress_verify_never_passes_on_a_failed_read() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("g10-prog-verify");
    let sp = small_spec(&t, 2);
    let d = t.cdir("c");
    assert_eq!(
        run_env(&sp, &d, &[], Duration::from_secs(300)),
        0,
        "{}",
        log_tail(&d)
    );
    let run_verify = |env: &[(&str, &str)]| -> (i32, String) {
        let mut c = Command::new(SUP);
        c.arg("verify").arg(&d).env_remove("RR_CAMPAIGN_FAILPOINT");
        for (k, v) in env {
            c.env(k, v);
        }
        let o = c.output().unwrap();
        (
            o.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr),
        )
    };
    let (c0, _) = run_verify(&[]);
    assert_eq!(c0, 0, "control verify");
    for errno in [libc::EIO, libc::EMFILE, libc::EACCES] {
        let fp = fp_site("progress", errno);
        let (c, out) = run_verify(&[("RR_CAMPAIGN_FAILPOINT", &fp)]);
        eprintln!(
            "G10 progress verify errno {errno}: exit {c}: {}",
            out.lines().last().unwrap_or("")
        );
        assert_ne!(
            c, 0,
            "verify passed although the progress files could not be read: {out}"
        );
    }
}

// ------------------------------------------------------------------------------------------------ ledger.snapshot

/// Rotation with old snapshots to prune while every snapshot proof read fails: NOTHING is removed (an unprovable
/// snapshot is kept), the ledger stays readable. Control: old snapshots are pruned.
#[test]
fn g10_site_ledger_snapshot_prune_keeps_what_it_cannot_prove() {
    if is_inner() {
        use ledger_support::*;
        use rustred_campaign::state::EvidenceLevel;
        let d = scratch("g10snap");
        let _c = Cleanup(d.clone());
        let clk = clock();
        let cfg = rustred_campaign::ledger::LedgerConfig {
            rotate_bytes: 6 * 1024,
            keep_old_snapshots: 0,
            ..rustred_campaign::ledger::LedgerConfig::default()
        };
        let mut l = new_ledger(&d, as_dyn(&clk), cfg.clone());
        l.append_all(setup_events("N1")).unwrap();
        for k in 0..60 {
            for e in item_life(
                "N1",
                k,
                (k % 3) as u8,
                EvidenceLevel::IdentityTested,
                100 + k as u64,
            ) {
                l.append(e).unwrap();
            }
        }
        let rotations = l.manifest().sealed.len();
        drop(l);
        let snaps = std::fs::read_dir(rustred_campaign::ledger::ledger_dir(&d))
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("snapshot-")
            })
            .count();
        let (l2, _) = open_ledger(&d, as_dyn(&clk), cfg).unwrap();
        let ok = rustred_campaign::ledger::verify_chain(&d).is_ok();
        println!(
            "G10 ledger.snapshot: rotations {rotations}, snapshot files {snaps}, reopen ok, chain ok {ok}, seq {}",
            l2.seq()
        );
        let injected = std::env::var("RR_CAMPAIGN_FAILPOINT").is_ok();
        assert!(rotations >= 3, "rotations {rotations}");
        assert!(ok);
        if injected {
            assert!(
                snaps >= rotations,
                "a snapshot was removed without its proof: {snaps} < {rotations}"
            );
        } else {
            assert!(snaps <= 2, "control: old snapshots pruned ({snaps})");
        }
        return;
    }
    let fp = fp_site("ledger.snapshot", libc::EIO);
    assert!(inner(
        "g10_site_ledger_snapshot_prune_keeps_what_it_cannot_prove",
        &[("RR_CAMPAIGN_FAILPOINT", &fp)]
    ));
    assert!(inner(
        "g10_site_ledger_snapshot_prune_keeps_what_it_cannot_prove",
        &[]
    ));
}

// ------------------------------------------------------------------------------------------------ tui

/// The dashboard's directory source of a finished campaign while every tui-labelled read / listing fails: never a
/// clean picture ("no nodes", no note).
#[test]
fn g10_site_tui_never_shows_a_failed_read_as_nothing() {
    if is_inner() {
        use rustred_campaign::tui::source::{DirSource, FileRead, ProgressSource};
        let d = PathBuf::from(std::env::var("GATE10_CDIR").unwrap());
        let snap = DirSource::new(&d).read();
        let injected = std::env::var("RR_CAMPAIGN_FAILPOINT").is_ok();
        println!(
            "G10 tui injected {injected}: campaign ok {}, nodes {}, notes {:?}, mtime {:?}",
            matches!(snap.campaign, FileRead::Ok(_)),
            snap.nodes.len(),
            snap.notes,
            snap.campaign_mtime_s
        );
        // In this layout (campaign.json names its node files) the tui label governs the heartbeat mtime and the
        // ledger size: a failed stat is shown as UNKNOWN (None), never as a value.
        if injected {
            assert!(
                snap.campaign_mtime_s.is_none(),
                "a failed stat shown as an mtime: {:?}",
                snap.campaign_mtime_s
            );
        } else {
            assert!(matches!(snap.campaign, FileRead::Ok(_)));
            assert!(snap.campaign_mtime_s.is_some());
        }
        return;
    }
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("g10-tui");
    let sp = small_spec(&t, 2);
    let d = t.cdir("c");
    assert_eq!(
        run_env(&sp, &d, &[], Duration::from_secs(300)),
        0,
        "{}",
        log_tail(&d)
    );
    let ds = d.display().to_string();
    let fp = fp_site("tui", libc::EIO);
    assert!(inner(
        "g10_site_tui_never_shows_a_failed_read_as_nothing",
        &[("RR_CAMPAIGN_FAILPOINT", &fp), ("GATE10_CDIR", &ds)]
    ));
    assert!(inner(
        "g10_site_tui_never_shows_a_failed_read_as_nothing",
        &[("GATE10_CDIR", &ds)]
    ));
}

// ------------------------------------------------------------------------------------------------ adapters.outcome

/// The legacy wrapper's refusal record cannot be read: reported as unreadable, never "no refusal" (None).
#[test]
fn g10_site_adapters_outcome_refusal_record() {
    if is_inner() {
        let d = PathBuf::from(std::env::var("GATE10_RUN").unwrap());
        let layout = rustred_campaign_contract::layout::RunLayout::new(&d);
        let r = rustred_campaign::adapters::legacy::refusal(&layout);
        let injected = std::env::var("RR_CAMPAIGN_FAILPOINT").is_ok();
        println!("G10 adapters.outcome injected {injected}: refusal {r:?}");
        if injected {
            assert!(
                r.is_some(),
                "an unreadable refusal record read as 'no refusal'"
            );
        } else {
            assert_eq!(r.as_deref(), Some("refused: test\n"));
        }
        return;
    }
    let t = Tmp::new("g10-adapt");
    let run = t.path.join("run");
    std::fs::create_dir_all(run.join("sup")).unwrap();
    std::fs::write(run.join("sup/wrapper-refusal.txt"), "refused: test\n").unwrap();
    let rs = run.display().to_string();
    let fp = fp_site("adapters.outcome", libc::EIO);
    assert!(inner(
        "g10_site_adapters_outcome_refusal_record",
        &[("RR_CAMPAIGN_FAILPOINT", &fp), ("GATE10_RUN", &rs)]
    ));
    assert!(inner(
        "g10_site_adapters_outcome_refusal_record",
        &[("GATE10_RUN", &rs)]
    ));
}

// ------------------------------------------------------------------------------------------------ closeloop.input / .output

/// The closing loop's toy node under every closeloop.input (resp. .output) call failing: never exit 0 with the node
/// closed. Control: the same toy closes (exit 0).
#[test]
fn g10_site_closeloop_input_and_output_never_close_on_a_failed_read() {
    if !in_ns() {
        return;
    }
    use closeloop_support::*;
    let _g = serial();
    for (label, errno) in [
        ("closeloop.input", libc::EIO),
        ("closeloop.output", libc::EIO),
        ("", 0),
    ] {
        let t = Tmp::new(&format!("g10-cl-{}", label.replace('.', "-")));
        let files = toy(&t.path.join("material"), &Toy::default(), FAKE);
        // the hand statement (comparison only), as closeloop_fake writes it before a run
        let hand: Vec<serde_json::Value> = [
            ([5, 1, 0], true, "base", "A#0"),
            ([4, 1, 0], false, "recipe", "R1"),
            ([3, 1, 0], false, "recipe", "R1"),
            ([2, 1, 0], false, "recipe", "R1"),
            ([1, 1, 0], false, "recipe", "default-recipe"),
            ([3, 2, 0], true, "recipe", "R2"),
            ([2, 2, 0], false, "recipe", "R2"),
            ([1, 2, 0], false, "recipe", "R2"),
        ]
        .iter()
        .map(|(k, b, l, r)| serde_json::json!([format!("I({},{},{})", k[0], k[1], k[2]), b, l, r]))
        .collect();
        std::fs::write(
            &files.hand,
            hand.iter().map(|v| format!("{v}\n")).collect::<String>(),
        )
        .unwrap();
        let spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
        let sp = write_spec(&t, "spec.json", &spec);
        let d = t.cdir("cdir");
        let fp = if label.is_empty() {
            String::new()
        } else {
            fp_site(label, errno)
        };
        let env: Vec<(&str, &str)> = if label.is_empty() {
            vec![]
        } else {
            vec![("RR_CAMPAIGN_FAILPOINT", &fp)]
        };
        let code = run_env(&sp, &d, &env, Duration::from_secs(900));
        let st = state(&d);
        let closed = st
            .nodes
            .get("toy")
            .is_some_and(|n| n.state.phase == rustred_campaign::state::NodePhase::Closed);
        eprintln!("G10 {label:?}: exit {code}, toy closed {closed}");
        if label.is_empty() {
            assert_eq!(code, 0, "control: {}", log_tail(&d));
            assert!(closed, "control: the toy closes");
        } else {
            assert!(
                !(code == 0 && closed),
                "{label}: exit 0 with the node closed under a failed read: {}",
                log_tail(&d)
            );
            assert_ne!(
                code,
                0,
                "{label}: exit 0 under a failed read: {}",
                log_tail(&d)
            );
        }
    }
}
