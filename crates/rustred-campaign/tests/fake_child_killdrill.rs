//! Ported from audit-contract-B (t1_kill_resume.rs; lane contractfix, regression baseline: 0 attempts wrongly
//! accepted, every resumed table canonically identical). Ignored by default; run in release in a pid namespace:
//! `LIGHT_PIDNS=1 light_build.sh <lane> test --release -p rustred-campaign --test fake_child_killdrill -- --ignored`.
//! Byte identity after resume is counted, not required (user ruling R40 U-1).
//!
//! audit-contract-B experiment 1: KILL AT EVERY POINT, then resume to completion; compare with an uninterrupted run.
//! (a) SIGKILL at seeded random times; (b) deterministic death at byte N of the table via RLIMIT_FSIZE (SIGXFSZ);
//! (c) same with SIGXFSZ ignored (EFBIG -> the child's own I/O-error path); (d) torn_write fault at every op/byte.
//! Every intermediate attempt is classified; any `accept != Nothing` for an attempt that did not finish is CRITICAL.
//! Env: AUDIT_KILLS (default 240), AUDIT_SEED (default 0xA0D17B).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

mod fake_child_common;
use fake_child_common::drill::*;
use rustred_campaign_contract::classify::{Accept, KillReason, WaitStatus};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::outcome::Outcome;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

fn script(big: bool, jitter: bool) -> Value {
    let loci = if big { Some((4u32, 100_000u64)) } else { None };
    let mut ops = vec![json!({"op":"prepare","ms":10})];
    for i in 0..8u64 {
        if i % 3 == 2 {
            ops.push(covered_case(i, 15));
        } else {
            ops.push(rule_case(i, 15, loci));
        }
    }
    ops.push(json!({"op":"end"}));
    json!({"script":ops,"volatile_jitter":jitter})
}

struct Ref {
    canon: Vec<Vec<u8>>,
    digest: String,
    raw: Vec<u8>,
    wall: Duration,
}

fn reference(tag: &str, big: bool, jitter: bool) -> Ref {
    let root = fresh(tag);
    let rd = rustred_campaign_contract::layout::attempt_dir(&root, "j", 1);
    write_spec(&rd, &spec_json(&rd, "j", 1, script(big, jitter)));
    let t0 = Instant::now();
    let w = run_to_end(&rd, &SpawnOpt::default(), Duration::from_secs(30));
    let wall = t0.elapsed();
    assert_eq!(w, WaitStatus::Exited(0));
    let (_, c) = classify_dir(&rd, w, KillReason::None);
    assert_eq!(c.row, 1);
    let l = RunLayout::new(&rd);
    let (d, _) = rustred_campaign_contract::canon::canonical_digest_file(&l.table()).unwrap();
    Ref {
        canon: canon_lines(&l.table()),
        digest: d,
        raw: std::fs::read(l.table()).unwrap(),
        wall,
    }
}

#[derive(Default, Debug)]
struct Tally {
    rows: BTreeMap<String, u64>,
    critical: Vec<String>,
    resumed_ok: u64,
    attempts: u64,
    byte_identical: u64,
}

/// Classify an attempt; CRITICAL if anything is accepted unless the process exited 0 AND its table equals the
/// uninterrupted reference canonically. Returns whether the attempt is row 1 (done).
fn check_attempt(t: &mut Tally, rd: &Path, w: WaitStatus, label: &str, rf: &Ref) -> bool {
    let genuine = w == WaitStatus::Exited(0)
        && std::fs::read(RunLayout::new(rd).table())
            .ok()
            .map(|_| canon_lines_opt(&RunLayout::new(rd).table()) == Some(rf.canon.clone()))
            .unwrap_or(false);
    let mut done = false;
    for k in [
        KillReason::None,
        KillReason::Stop,
        KillReason::Time,
        KillReason::Memory,
        KillReason::Unresponsive,
    ] {
        let (_, c) = classify_dir(rd, w, k);
        if k == KillReason::None {
            *t.rows
                .entry(format!("{w:?} -> row {} {:?}", c.row, c.kind))
                .or_default() += 1;
            done = c.row == 1;
        }
        if !genuine && c.accept != Accept::Nothing {
            t.critical.push(format!(
                "{label}: {w:?} K={k:?} -> row {} accept {:?} ({})",
                c.row,
                c.accept,
                rd.display()
            ));
        }
    }
    done
}

fn canon_lines_opt(p: &Path) -> Option<Vec<Vec<u8>>> {
    let b = std::fs::read(p).ok()?;
    if !b.ends_with(b"\n") {
        return None;
    }
    b.split(|&c| c == b'\n')
        .filter(|l| !l.is_empty())
        .map(|l| rustred_campaign_contract::canon::canonical_line(l).ok())
        .collect()
}

/// Resume chain from attempt `a` (already run and classified) to completion. `kill_rng`: kill resumed attempts too.
#[allow(clippy::too_many_arguments)]
fn resume_chain(
    t: &mut Tally,
    root: &Path,
    mut last: u32,
    rf: &Ref,
    big: bool,
    jitter: bool,
    rng: &mut Rng,
    label: &str,
) {
    let mut a = last + 1;
    loop {
        assert!(a < 40, "{label}: resume chain does not end");
        // the most recent attempt with a valid RESUME
        let mut from = None;
        for b in (1..=last).rev() {
            let pd = rustred_campaign_contract::layout::attempt_dir(root, "j", b);
            if let Some(r) = resume_ref_from(&pd) {
                from = Some(r);
                break;
            }
        }
        let rd = rustred_campaign_contract::layout::attempt_dir(root, "j", a);
        let mut s = spec_json(&rd, "j", a, script(big, jitter));
        if let Some(r) = from {
            s["resume"] = r;
        }
        write_spec(&rd, &s);
        let kill_it = rng.chance(1, 3);
        let mut ch = spawn(&rd, &SpawnOpt::default());
        let w = if kill_it {
            let at = Duration::from_micros(rng.below(rf.wall.as_micros() as u64 + 1));
            let t0 = Instant::now();
            while t0.elapsed() < at {
                if ch.try_wait().unwrap().is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_micros(300));
            }
            kill_group(ch.id() as i32, libc::SIGKILL);
            wait_timeout(&mut ch, Duration::from_secs(10)).0
        } else {
            let (w, to) = wait_timeout(&mut ch, Duration::from_secs(30));
            assert!(!to);
            w
        };
        t.attempts += 1;
        let fin = check_attempt(t, &rd, w, label, rf);
        if fin {
            let l = RunLayout::new(&rd);
            let got = canon_lines(&l.table());
            if got != rf.canon {
                t.critical.push(format!("{label}: final canonical records differ from the uninterrupted run ({}): {} vs {} records", rd.display(), got.len(), rf.canon.len()));
            }
            let o = Outcome::read(&l.outcome()).unwrap().unwrap();
            let cd = o
                .job
                .outputs
                .table
                .as_ref()
                .and_then(|x| x.canonical_blake3.clone())
                .unwrap_or_default();
            if cd != rf.digest {
                t.critical.push(format!(
                    "{label}: OUTCOME canonical_blake3 differs ({})",
                    rd.display()
                ));
            }
            if std::fs::read(l.table()).unwrap() == rf.raw {
                t.byte_identical += 1;
            }
            t.resumed_ok += 1;
            return;
        }
        last = a;
        a += 1;
    }
}

fn report(name: &str, t: &Tally) {
    let out = scratch().join("results").join(format!("{name}.txt"));
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    let s = format!("{t:#?}\n");
    std::fs::write(&out, &s).unwrap();
    eprintln!("{name}: {s}");
}

#[test]
#[ignore = "drill: release + LIGHT_PIDNS"]
fn a_random_sigkill_then_resume() {
    let kills: u64 = std::env::var("AUDIT_KILLS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(240);
    let seed: u64 = std::env::var("AUDIT_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0xA0D17B);
    let rf = reference("t1ref", false, true);
    let mut t = Tally::default();
    let mut rng = Rng(seed);
    let span = rf.wall.as_micros() as u64 * 12 / 10 + 1;
    for k in 0..kills {
        let root = fresh("t1a");
        let rd = rustred_campaign_contract::layout::attempt_dir(&root, "j", 1);
        write_spec(&rd, &spec_json(&rd, "j", 1, script(false, true)));
        let at = Duration::from_micros(rng.below(span));
        let mut ch = spawn(&rd, &SpawnOpt::default());
        let t0 = Instant::now();
        while t0.elapsed() < at {
            if ch.try_wait().unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_micros(200));
        }
        kill_group(ch.id() as i32, libc::SIGKILL);
        let (w, _) = wait_timeout(&mut ch, Duration::from_secs(10));
        t.attempts += 1;
        if check_attempt(&mut t, &rd, w, &format!("kill#{k}@{at:?}"), &rf) {
            t.resumed_ok += 1;
            if canon_lines(&RunLayout::new(&rd).table()) != rf.canon {
                t.critical.push(format!("kill#{k}: unkilled run differs"));
            }
        } else {
            resume_chain(
                &mut t,
                &root,
                1,
                &rf,
                false,
                true,
                &mut rng,
                &format!("kill#{k}"),
            );
        }
        assert!(
            survivors(&root).is_empty(),
            "survivor under {}",
            root.display()
        );
        let _ = std::fs::remove_dir_all(&root);
    }
    report("t1a_random_sigkill", &t);
    assert!(t.critical.is_empty(), "CRITICAL: {:#?}", t.critical);
}

fn line_ends(raw: &[u8]) -> Vec<u64> {
    raw.iter()
        .enumerate()
        .filter(|(_, b)| **b == b'\n')
        .map(|(i, _)| i as u64 + 1)
        .collect()
}

fn fsize_sweep(name: &str, ignore: bool) {
    // jitter off so that the resumed table must be byte-identical too
    let rf = reference("t1bref", true, false);
    let size = rf.raw.len() as u64;
    let floor = 1u64 << 20; // the 1 MiB .reserve must fit
    assert!(size > floor + 100_000, "table too small: {size}");
    let mut points: Vec<u64> = (floor..size).step_by(7919).collect();
    for e in line_ends(&rf.raw) {
        for d in [-1i64, 0, 1] {
            let p = e as i64 + d;
            if p as u64 >= floor && (p as u64) < size + 2 {
                points.push(p as u64);
            }
        }
    }
    points.sort();
    points.dedup();
    let mut t = Tally::default();
    let mut rng = Rng(0xF5122E ^ ignore as u64);
    for &n in &points {
        let root = fresh(if ignore { "t1c" } else { "t1b" });
        let rd = rustred_campaign_contract::layout::attempt_dir(&root, "j", 1);
        write_spec(&rd, &spec_json(&rd, "j", 1, script(true, false)));
        let w = run_to_end(
            &rd,
            &SpawnOpt {
                fsize: Some(n),
                ignore_xfsz: ignore,
            },
            Duration::from_secs(30),
        );
        t.attempts += 1;
        let tsize = std::fs::metadata(RunLayout::new(&rd).table())
            .map(|m| m.len())
            .unwrap_or(0);
        if !check_attempt(&mut t, &rd, w, &format!("fsize={n} table={tsize}"), &rf) {
            resume_chain(
                &mut t,
                &root,
                1,
                &rf,
                true,
                false,
                &mut rng,
                &format!("fsize={n}"),
            );
        } else {
            t.resumed_ok += 1;
        }
        assert!(survivors(&root).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
    report(name, &t);
    assert!(t.critical.is_empty(), "CRITICAL: {:#?}", t.critical);
}

#[test]
#[ignore = "drill: release + LIGHT_PIDNS"]
fn b_fsize_death_at_byte_n_then_resume() {
    fsize_sweep("t1b_fsize_sigxfsz", false);
}

#[test]
#[ignore = "drill: release + LIGHT_PIDNS"]
fn c_fsize_efbig_io_path_then_resume() {
    fsize_sweep("t1c_fsize_efbig", true);
}

#[test]
#[ignore = "drill: release + LIGHT_PIDNS"]
fn d_torn_write_every_op_every_byte() {
    let rf = reference("t1dref", false, true);
    let mut t = Tally::default();
    let mut rng = Rng(0x7031);
    let nops = 10usize; // prepare + 8 cases + end
    for at_op in 0..=nops {
        for bytes in [1u64, 2, 5, 17, 40, 63, 64, 65, 1000] {
            let root = fresh("t1d");
            let rd = rustred_campaign_contract::layout::attempt_dir(&root, "j", 1);
            let mut sc = script(false, true);
            sc["faults"] = json!([{"at_op":at_op,"kind":"torn_write","bytes":bytes}]);
            write_spec(&rd, &spec_json(&rd, "j", 1, sc));
            let w = run_to_end(&rd, &SpawnOpt::default(), Duration::from_secs(30));
            t.attempts += 1;
            if !check_attempt(
                &mut t,
                &rd,
                w,
                &format!("torn op={at_op} bytes={bytes}"),
                &rf,
            ) {
                resume_chain(
                    &mut t,
                    &root,
                    1,
                    &rf,
                    false,
                    true,
                    &mut rng,
                    &format!("torn op={at_op} b={bytes}"),
                );
            }
            let _ = std::fs::remove_dir_all(&root);
        }
    }
    // kill_before_outcome and outcome_unsealed
    for kind in ["kill_before_outcome", "outcome_unsealed"] {
        let root = fresh("t1d");
        let rd = rustred_campaign_contract::layout::attempt_dir(&root, "j", 1);
        let mut sc = script(false, true);
        sc["faults"] = json!([{"at_op":nops,"kind":kind}]);
        write_spec(&rd, &spec_json(&rd, "j", 1, sc));
        let w = run_to_end(&rd, &SpawnOpt::default(), Duration::from_secs(30));
        t.attempts += 1;
        if !check_attempt(&mut t, &rd, w, kind, &rf) {
            resume_chain(&mut t, &root, 1, &rf, false, true, &mut rng, kind);
        }
        let _ = std::fs::remove_dir_all(&root);
    }
    report("t1d_torn_write", &t);
    assert!(t.critical.is_empty(), "CRITICAL: {:#?}", t.critical);
}
