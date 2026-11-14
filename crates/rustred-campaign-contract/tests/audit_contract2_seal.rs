//! Independent re-audit (lane audit-contract2, 2026-11-14): acceptance only from a sealed table bound to THIS
//! attempt; lying / crashed children; path fields; time-of-check / time-of-use; version negotiation.
//!
//! Every test builds a real run directory (job.json, out/F.table.jsonl, OUTCOME.json, RESUME.json) with the crate's
//! own writers where an honest child would use them, then makes the child lie, and runs the supervisor's real entry
//! points `classify::assess` + `classify::classify` (and `resume_state`). Tests named in a FINDING carry
//! `#[ignore = "FINDING ..."]` and assert the CORRECT behaviour: they fail with `--include-ignored` until fixed.

mod common;
use common::*;
use rustred_campaign_contract::canon::{CanonicalDigest, canonical_line};
use rustred_campaign_contract::classify::{
    self, Accept, Classification, Evidence, KillReason, OutcomeState, WaitStatus,
};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::outcome::{EndKind, ExtraOutput, JobOutcome, Outcome, StopInfo, TableOutput};
use rustred_campaign_contract::resume::{CommittedFile, Frame, ResumeRef, ResumeState};
use rustred_campaign_contract::seal::{self, RecordWriter, TableSeal};
use rustred_campaign_contract::spec::{Counters, JobKind, JobSpec};
use rustred_campaign_contract::stop::StopCode;
use rustred_campaign_contract::{ExitCode, RESUME_SCHEMA, blake3_hex};
use std::path::Path;

// ------------------------------------------------------------------------------------------------ fixtures

struct Run {
    _d: TempDir,
    layout: RunLayout,
    spec: JobSpec,
    digest: String,
}

fn new_run(tag: &str, attempt: u32) -> Run {
    let d = TempDir::new(tag);
    let root = d.path().join(format!("J.a{attempt}"));
    for s in ["out", "in", "tmp", "sup"] {
        std::fs::create_dir_all(root.join(s)).unwrap();
    }
    let mut spec = sample_spec(root.to_str().unwrap());
    spec.attempt = attempt;
    let bytes = spec.to_json_line();
    std::fs::write(root.join("job.json"), &bytes).unwrap();
    let digest = rustred_campaign_contract::spec::spec_blake3(&bytes);
    Run {
        _d: d,
        layout: RunLayout::new(root),
        spec,
        digest,
    }
}

const HEADER: &str = r#"{"type":"header","family_file":"/abs/T162.sym","family_blake3":"1111111111111111111111111111111111111111111111111111111111111111","zero_seconds":0.25}"#;

fn rule(i: u64) -> String {
    format!(
        r#"{{"type":"rule","chart":"11","index":{i},"case":"c{i}","target":"I({i})","rhs":[["I(0)","-1/2"]],"seconds_search":0.5,"witness_loci":["n1+1"]}}"#
    )
}

/// An honest table: header, `n` rules, an end record of type `end_type`.
fn write_table(run: &Run, n: u64, end_type: &str) -> TableSeal {
    let mut w = RecordWriter::create(&run.layout.table()).unwrap();
    w.append(HEADER.as_bytes()).unwrap();
    for i in 0..n {
        w.append(rule(i).as_bytes()).unwrap();
    }
    w.seal(format!(r#"{{"type":"{end_type}","rules":{n},"seconds":1.5}}"#).as_bytes())
        .unwrap()
}

fn outcome_for(run: &Run, code: ExitCode, stop: Option<StopCode>, seal: Option<&TableSeal>, rules: u64) -> JobOutcome {
    let mut j = JobOutcome::for_spec(
        &run.spec,
        &run.digest,
        program_info(),
        code,
        StopInfo {
            code: stop,
            detail: serde_json::Value::Null,
            source: None,
            graceful: true,
        },
    );
    let mut t = TableOutput {
        path: "out/F.table.jsonl".into(),
        rules_committed: rules,
        ..Default::default()
    };
    if let Some(s) = seal {
        s.apply(&mut t);
    }
    j.outputs.table = Some(t);
    j.counters.rules_committed = rules;
    j
}

fn write_outcome(run: &Run, j: &JobOutcome) {
    let _ = std::fs::remove_file(run.layout.outcome());
    Outcome::new(j.clone()).write_new(&run.layout.outcome()).unwrap();
}

fn judge(run: &Run, wait: WaitStatus) -> (Evidence, Classification) {
    let ev = classify::assess(&run.layout, &run.spec, &run.digest, wait, KillReason::None, false);
    let c = classify::classify(&ev);
    (ev, c)
}

/// An honest complete run: 3 rules, exit 0, sealed.
fn honest(tag: &str) -> (Run, TableSeal) {
    let run = new_run(tag, 1);
    let s = write_table(&run, 3, "end");
    write_outcome(&run, &outcome_for(&run, ExitCode::Complete, None, Some(&s), 3));
    (run, s)
}

/// Figures of an arbitrary byte string as OUTCOME would report them (bytes, raw digest, records, canonical digest),
/// computed independently of the verifier; `None` when a line has no canonical form.
fn figures(b: &[u8]) -> Option<(u64, String, u64, String)> {
    let mut d = CanonicalDigest::new();
    let mut n = 0;
    for l in b.split_inclusive(|&c| c == b'\n') {
        d.push_line(l).ok()?;
        n += 1;
    }
    Some((b.len() as u64, blake3_hex(b), n, d.hex()))
}

fn lie_about(run: &Run, j: &mut JobOutcome, b: &[u8], rules: u64, end: EndKind) {
    let t = j.outputs.table.as_mut().unwrap();
    if let Some((bytes, raw, records, canon)) = figures(b) {
        t.bytes = Some(bytes);
        t.blake3 = Some(raw);
        t.records = Some(records);
        t.canonical_blake3 = Some(canon);
    }
    t.rules = Some(rules);
    t.rules_committed = rules;
    t.end = Some(end);
    t.sealed = true;
    j.counters.rules_committed = rules;
    write_outcome(run, j);
}

// ------------------------------------------------------------------------------------------------ acceptance

#[test]
fn ac2_seal_baseline_honest_run_is_accepted() {
    let (run, _) = honest("base");
    let (ev, c) = judge(&run, WaitStatus::Exited(0));
    assert_eq!((c.row, c.accept), (1, Accept::Canonical), "{c:?} {:?}", ev.unsealed_why);
    assert_eq!(ev.verified.unwrap().rules, 3);
}

/// A child that claims success over a table truncated at EVERY byte length (OUTCOME still names the full seal),
/// and a child that lies CONSISTENTLY (OUTCOME recomputed for the truncated bytes): nothing is ever accepted
/// except the full table.
#[test]
fn ac2_seal_truncated_table_claiming_success_never_accepted() {
    let (run, s) = honest("trunc");
    let full = std::fs::read(run.layout.table()).unwrap();
    let mut accepted = Vec::new();
    for k in 0..full.len() {
        std::fs::write(run.layout.table(), &full[..k]).unwrap();
        // stale claim
        write_outcome(&run, &outcome_for(&run, ExitCode::Complete, None, Some(&s), 3));
        let (_, c) = judge(&run, WaitStatus::Exited(0));
        if c.accept != Accept::Nothing {
            accepted.push(format!("stale k={k}"));
        }
        // consistent lie (rules = rule lines contained)
        let rules = full[..k]
            .split_inclusive(|&b| b == b'\n')
            .filter(|l| l.ends_with(b"\n") && l.starts_with(br#"{"type":"rule""#))
            .count() as u64;
        let mut j = outcome_for(&run, ExitCode::Complete, None, Some(&s), rules);
        lie_about(&run, &mut j, &full[..k], rules, EndKind::End);
        let (_, c) = judge(&run, WaitStatus::Exited(0));
        if c.accept != Accept::Nothing {
            accepted.push(format!("consistent k={k}"));
        }
    }
    eprintln!("ac2_seal_truncated_table_claiming_success_never_accepted: {} lengths x 2 lies", full.len());
    assert!(accepted.is_empty(), "accepted truncated tables: {accepted:?}");
}

/// Writes after sealing: a record appended after the end record (stale OUTCOME, and OUTCOME rewritten to match the
/// new bytes) is never accepted.
#[test]
fn ac2_seal_write_after_seal_never_accepted() {
    let (run, s) = honest("after");
    let mut f = std::fs::OpenOptions::new().append(true).open(run.layout.table()).unwrap();
    use std::io::Write;
    f.write_all(format!("{}\n", rule(9)).as_bytes()).unwrap();
    drop(f);
    let (_, c) = judge(&run, WaitStatus::Exited(0));
    assert_eq!((c.row, c.accept), (22, Accept::Nothing), "stale OUTCOME: {c:?}");
    let b = std::fs::read(run.layout.table()).unwrap();
    let mut j = outcome_for(&run, ExitCode::Complete, None, Some(&s), 4);
    lie_about(&run, &mut j, &b, 4, EndKind::End);
    let (ev, c) = judge(&run, WaitStatus::Exited(0));
    assert_eq!((c.row, c.accept), (22, Accept::Nothing), "{c:?} {:?}", ev.unsealed_why);
}

/// OUTCOME of another attempt / another spec, and a child that replaces job.json to match a forged OUTCOME.
#[test]
fn ac2_seal_outcome_bound_to_this_attempt_and_spec() {
    let (run, s) = honest("bind");
    // (a) attempt 2's OUTCOME (job_id equal) inside attempt 1's run directory
    let mut spec2 = run.spec.clone();
    spec2.attempt = 2;
    let d2 = rustred_campaign_contract::spec::spec_blake3(&spec2.to_json_line());
    let mut j = outcome_for(&run, ExitCode::Complete, None, Some(&s), 3);
    j.attempt = 2;
    j.spec_blake3 = d2.clone();
    write_outcome(&run, &j);
    assert_eq!(judge(&run, WaitStatus::Exited(0)).1.accept, Accept::Nothing, "(a)");
    // (b) same job / attempt, spec digest of a spec differing only in run_dir
    let mut spec3 = run.spec.clone();
    spec3.run_dir = "/elsewhere/J.a1".into();
    let mut j = outcome_for(&run, ExitCode::Complete, None, Some(&s), 3);
    j.spec_blake3 = rustred_campaign_contract::spec::spec_blake3(&spec3.to_json_line());
    write_outcome(&run, &j);
    assert_eq!(judge(&run, WaitStatus::Exited(0)).1.accept, Accept::Nothing, "(b)");
    // (c) the child rewrites job.json to spec3 and binds OUTCOME to it: the supervisor's own digest decides
    let _ = std::fs::remove_file(run.layout.job_spec());
    std::fs::write(run.layout.job_spec(), spec3.to_json_line()).unwrap();
    assert_eq!(judge(&run, WaitStatus::Exited(0)).1.accept, Accept::Nothing, "(c)");
    // (d) the right OUTCOME again but job.json still replaced
    write_outcome(&run, &outcome_for(&run, ExitCode::Complete, None, Some(&s), 3));
    let (ev, c) = judge(&run, WaitStatus::Exited(0));
    assert_eq!(c.accept, Accept::Nothing, "(d) {:?}", ev.outcome);
    // (e) exit-code lie: OUTCOME says 0, the process exited 10
    std::fs::write(run.layout.job_spec(), run.spec.to_json_line()).unwrap();
    let (_, c) = judge(&run, WaitStatus::Exited(0));
    assert_eq!(c.accept, Accept::Canonical, "restored");
    let (_, c) = judge(&run, WaitStatus::Exited(10));
    assert_eq!((c.row, c.accept), (22, Accept::Nothing), "(e)");
    // (f) supervisor restart (W unknown) with a valid OUTCOME: the table applies with c = OUTCOME's exit code
    let (_, c) = judge(&run, WaitStatus::Unknown);
    assert_eq!((c.row, c.accept), (1, Accept::Canonical), "(f) 6.6");
}

// ------------------------------------------------------------------------------------------------ path fields

#[cfg(unix)]
#[test]
fn ac2_seal_symlinks_in_the_run_are_refused() {
    use std::os::unix::fs::symlink;
    // (a) out/ is a symlink to a directory holding a perfect table
    let (run, _) = honest("lnk-out");
    let elsewhere = run.layout.root.parent().unwrap().join("elsewhere");
    std::fs::rename(run.layout.out_dir(), &elsewhere).unwrap();
    symlink(&elsewhere, run.layout.out_dir()).unwrap();
    assert_eq!(judge(&run, WaitStatus::Exited(0)).1.accept, Accept::Nothing, "(a) out/ symlink");
    // (b) the table itself is a symlink
    let (run, _) = honest("lnk-table");
    let real = run.layout.root.parent().unwrap().join("real.jsonl");
    std::fs::rename(run.layout.table(), &real).unwrap();
    symlink(&real, run.layout.table()).unwrap();
    assert_eq!(judge(&run, WaitStatus::Exited(0)).1.accept, Accept::Nothing, "(b) table symlink");
    // (c) OUTCOME is a symlink
    let (run, _) = honest("lnk-outcome");
    let real = run.layout.root.parent().unwrap().join("o.json");
    std::fs::rename(run.layout.outcome(), &real).unwrap();
    symlink(&real, run.layout.outcome()).unwrap();
    let (ev, c) = judge(&run, WaitStatus::Exited(0));
    assert_eq!(c.accept, Accept::Nothing, "(c) {:?}", ev.outcome);
    // (d) RESUME is a symlink: never valid
    let (run, _) = honest("lnk-resume");
    let real = run.layout.root.parent().unwrap().join("r.json");
    std::fs::write(&real, b"{}").unwrap();
    symlink(&real, run.layout.resume()).unwrap();
    assert!(classify::resume_state(&run.layout, &run.spec, &run.digest).is_err());
    // (e) a FIFO as the table (lstat says not regular): refused without blocking
    let (run, _) = honest("fifo");
    std::fs::remove_file(run.layout.table()).unwrap();
    let st = std::process::Command::new("mkfifo").arg(run.layout.table()).status();
    if st.is_ok_and(|s| s.success()) {
        assert_eq!(judge(&run, WaitStatus::Exited(0)).1.accept, Accept::Nothing, "(e) fifo");
    }
}

/// Observation ac2-n3: a HARD link to a file outside the run directory passes every check ((dev, inode) equals the
/// directory entry's). Not a wrong acceptance by itself (the content must still match OUTCOME's seal, which the
/// child could equally have copied), but "the file lives inside RUN" is not what the verifier establishes.
#[test]
fn ac2_seal_hard_link_from_outside_the_run_passes() {
    let (run, _) = honest("hard");
    let outside = run.layout.root.parent().unwrap().join("outside.jsonl");
    std::fs::rename(run.layout.table(), &outside).unwrap();
    std::fs::hard_link(&outside, run.layout.table()).unwrap();
    assert_eq!(judge(&run, WaitStatus::Exited(0)).1.accept, Accept::Canonical);
}

/// FINDING ac2-m3 (minor): `seal::open_in_run` checks every path component with lstat and then opens the joined path
/// with O_NOFOLLOW, which protects only the LAST component. A process that can write into RUN after the child's
/// exit (an escaped helper is still alive while the supervisor settles: supervisor/mod.rs settles BEFORE the
/// attempt's marker sweep) can swap `out/` for a symlink between the lstat and the open: the verifier then reads
/// and certifies a file OUTSIDE the run directory, and the post-open (dev, ino) check passes because `entry` is
/// re-read through the same swapped path. Swapping in a FIFO between the lstat and the open makes `open` block
/// the supervisor (no O_NONBLOCK). Smallest fix: walk with `openat(dirfd, comp, O_NOFOLLOW|O_DIRECTORY)` per
/// component (rustix / nix are not allowed by R-a; std has no openat: open each directory with O_NOFOLLOW |
/// O_DIRECTORY through `/proc/self/fd/<dirfd>/<comp>`), open the file with O_NOFOLLOW | O_NONBLOCK, and compare
/// (dev, ino) of each opened directory with its lstat.
#[test]
#[ignore = "FINDING ac2-m3: fails (escape observed) until open_in_run opens component by component"]
fn ac2_seal_race_symlink_swap_of_out_dir_escapes_the_run() {
    use std::os::unix::fs::symlink;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    let (run, own) = honest("race");
    // a foreign directory with ANOTHER perfectly sealed table (5 rules)
    let foreign_run = new_run("race-foreign", 1);
    let foreign = write_table(&foreign_run, 5, "end");
    let root = run.layout.root.clone();
    let lnk = root.join("out.lnk");
    symlink(foreign_run.layout.out_dir(), &lnk).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let st = stop.clone();
    let r2 = root.clone();
    let flipper = std::thread::spawn(move || {
        let (out, real, lnk) = (r2.join("out"), r2.join("out.real"), r2.join("out.lnk"));
        while !st.load(Ordering::Relaxed) {
            let _ = std::fs::rename(&out, &real);
            let _ = std::fs::rename(&lnk, &out);
            let _ = std::fs::rename(&out, &lnk);
            let _ = std::fs::rename(&real, &out);
        }
    });
    let t0 = std::time::Instant::now();
    let (mut tries, mut own_ok, mut escaped) = (0u64, 0u64, 0u64);
    while t0.elapsed() < std::time::Duration::from_secs(8) && escaped == 0 {
        tries += 1;
        if let Ok(v) = seal::verify_stream(&root, "out/F.table.jsonl", &seal::TABLE_END_TYPES) {
            if v.blake3 == foreign.blake3 {
                escaped += 1;
            } else if v.blake3 == own.blake3 {
                own_ok += 1;
            }
        }
    }
    stop.store(true, Ordering::Relaxed);
    flipper.join().unwrap();
    eprintln!(
        "ac2_seal_race_symlink_swap_of_out_dir_escapes_the_run: {tries} verifications, {own_ok} own table, {escaped} FOREIGN table certified ({:.1} s)",
        t0.elapsed().as_secs_f64()
    );
    assert_eq!(escaped, 0, "the verifier certified a table outside the run directory");
}

// ------------------------------------------------------------------------------------------------ TOCTOU in the consumer

/// The supervisor's re-read of an accepted table, copied VERBATIM from crates/rustred-campaign/src/supervisor/mod.rs
/// `settle` (lines 1007-1018 at b5ba6b33): what the ledger's accepted record is built from.
fn settle_reread(layout: &RunLayout) -> (Vec<String>, Option<String>) {
    let p = layout.table();
    let mut lines = Vec::new();
    let mut canon = None;
    if let Ok(text) = std::fs::read(&p) {
        let mut d = CanonicalDigest::new();
        for l in text.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
            if let Ok(cl) = canonical_line(l) {
                lines.push(String::from_utf8_lossy(&cl).into_owned());
            }
            let _ = d.push_line(l);
        }
        canon = Some(d.hex());
    }
    (lines, canon)
}

/// FINDING ac2-M1 (major, consumer side): `settle` classifies with `assess` (read 1: verified), then computes the
/// ledger FileRef with `blake3_file` (read 2) and builds the accepted canonical lines and canonical digest from a
/// THIRD read (`std::fs::read`, follows symlinks), none of them compared with the verified stream
/// (`Evidence.verified.streams[0]`: dev, ino, bytes, blake3, canonical digest). Lines without a canonical form are
/// silently dropped from both the lines and the digest. Anything that writes into RUN between the reads (an
/// escaped helper is alive then: the marker sweep runs AFTER settle) makes the ledger accept records that were
/// never verified, under a sealed classification. Child-job.md 6.4 requires "never accept a record from a RUN
/// whose files changed after classification (the supervisor stores the digests it read)". Reproduction: honest
/// run, assess + classify (row 1, canonical), then (a) the table is replaced by another sealed table, (b) garbage
/// is appended; the settle re-read differs from the verified stream in (a) and silently hides (b).
/// Smallest fix: `verify_stream` returns the verified canonical lines (or `settle` re-opens with `open_in_run`,
/// hashes while reading, and compares bytes + blake3 + (dev, ino) with `ev.verified.streams[0]` BEFORE using a
/// line; any mismatch = FinishClass::Crashed, nothing accepted); run the attempt's marker sweep before `settle`.
#[test]
#[ignore = "FINDING ac2-M1: fails until the supervisor binds the accepted bytes to the verified stream"]
fn ac2_toctou_settle_reread_is_not_bound_to_the_verified_stream() {
    let (run, _) = honest("toctou");
    let (ev, c) = judge(&run, WaitStatus::Exited(0));
    assert_eq!((c.row, c.accept), (1, Accept::Canonical));
    let verified = ev.verified.as_ref().unwrap().streams[0].clone();
    // (b) garbage appended after classification: invisible to the re-read digest
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(run.layout.table()).unwrap();
        f.write_all(b"{\"type\":\"rule\",\"index\":99,\"rhs\":[[\"I(5)\",\"7\"]],\"target\":\"I(9)\"} trailing garbage\n")
            .unwrap();
    }
    let (_, canon_b) = settle_reread(&run.layout);
    let raw_b = blake3_hex(&std::fs::read(run.layout.table()).unwrap());
    eprintln!(
        "(b) appended garbage: settle canonical digest == verified: {}; raw digest == verified: {}",
        canon_b.as_deref() == Some(verified.canonical_blake3.as_str()),
        raw_b == verified.blake3
    );
    // (a) the table replaced (atomic rename) by another sealed table with different rules
    let other = new_run("toctou-other", 1);
    let _ = write_table(&other, 2, "end");
    std::fs::rename(other.layout.table(), run.layout.table()).unwrap();
    let (lines_a, canon_a) = settle_reread(&run.layout);
    eprintln!(
        "(a) replaced table: settle accepts {} canonical lines, canonical digest == verified: {}",
        lines_a.len(),
        canon_a.as_deref() == Some(verified.canonical_blake3.as_str())
    );
    assert_eq!(
        canon_a.as_deref(),
        Some(verified.canonical_blake3.as_str()),
        "the supervisor accepts records from bytes it never verified"
    );
}

// ------------------------------------------------------------------------------------------------ RESUME

fn resume_state_for(run: &Run, table: CommittedFile, attempt: u32, spec_blake3: &str) -> ResumeState {
    ResumeState {
        schema: RESUME_SCHEMA.into(),
        job_id: run.spec.job_id.clone(),
        attempt,
        spec_blake3: spec_blake3.into(),
        chart: serde_json::json!("11"),
        frame: Frame::Presented,
        rules_committed: table.rules,
        table: table.clone(),
        queue: None,
        current: None,
        pending: vec![],
        numerical: vec![],
        deferred: vec![],
        refused_open: vec![],
        counters_at_commit: Counters {
            rules_committed: table.rules,
            ..Default::default()
        },
        symbolic_cases: 0,
        discarded: 0,
        engine: Default::default(),
    }
}

/// A run stopped at a commit point: table prefix of `n` rules (no end record) and a RESUME for it.
fn stopped_run(tag: &str, n: u64) -> (Run, ResumeState) {
    let run = new_run(tag, 1);
    let mut w = RecordWriter::create(&run.layout.table()).unwrap();
    w.append(HEADER.as_bytes()).unwrap();
    for i in 0..n {
        w.append(rule(i).as_bytes()).unwrap();
    }
    let cp = w.commit().unwrap();
    let r = resume_state_for(&run, cp.committed("out/F.table.jsonl"), 1, &run.digest.clone());
    std::fs::write(run.layout.resume(), serde_json::to_vec(&r).unwrap()).unwrap();
    (run, r)
}

#[test]
fn ac2_resume_one_record_bound_to_this_attempt() {
    let (run, r) = stopped_run("res", 2);
    assert!(classify::resume_state(&run.layout, &run.spec, &run.digest).is_ok(), "honest RESUME");
    let ok = std::fs::read(run.layout.resume()).unwrap();
    let mut bad: Vec<(&str, Vec<u8>)> = Vec::new();
    // written twice (two objects in the file; appended instead of replaced)
    bad.push(("two objects", [ok.clone(), ok.clone()].concat()));
    bad.push(("two objects, newline", [ok.clone(), b"\n".to_vec(), ok.clone()].concat()));
    // duplicate keys inside the one object
    let s = String::from_utf8(ok.clone()).unwrap();
    bad.push(("duplicate table key", s.replacen("{", &format!("{{\"table\":{},", serde_json::to_string(&r.table).unwrap()), 1).into_bytes()));
    bad.push(("duplicate attempt key", s.replacen("{", "{\"attempt\":1,", 1).into_bytes()));
    // another attempt / another spec
    let mut x = r.clone();
    x.attempt = 2;
    bad.push(("other attempt", serde_json::to_vec(&x).unwrap()));
    let mut x = r.clone();
    x.spec_blake3 = D0.into();
    bad.push(("other spec digest", serde_json::to_vec(&x).unwrap()));
    // path fields
    for p in ["out/../out/F.table.jsonl", "/abs/out/F.table.jsonl", "out/F.queue.jsonl", "./out/F.table.jsonl", "out//F.table.jsonl", "job.json"] {
        let mut x = r.clone();
        x.table.path = p.into();
        bad.push(("table path", serde_json::to_vec(&x).unwrap()));
    }
    // prefix claims beyond / inside the file
    let mut x = r.clone();
    x.table.bytes += 1;
    bad.push(("bytes + 1", serde_json::to_vec(&x).unwrap()));
    let mut x = r.clone();
    x.table.bytes -= 1;
    bad.push(("bytes - 1 (mid line)", serde_json::to_vec(&x).unwrap()));
    // unknown major version
    let mut x = r.clone();
    x.schema = "rustred.child-job.resume.v2".into();
    bad.push(("schema v2", serde_json::to_vec(&x).unwrap()));
    let mut accepted = Vec::new();
    for (what, b) in &bad {
        std::fs::write(run.layout.resume(), b).unwrap();
        if classify::resume_state(&run.layout, &run.spec, &run.digest).is_ok() {
            accepted.push(*what);
        }
    }
    assert!(accepted.is_empty(), "invalid RESUME accepted: {accepted:?}");
}

#[test]
fn ac2_resume_ref_path_validation() {
    let ok = |f: &str| ResumeRef { from: f.into(), blake3: D0.into() }.validate().is_ok();
    assert!(ok("/w/jobs/J.a1/RESUME.json"));
    for bad in [
        "w/jobs/J.a1/RESUME.json",
        "/w/jobs/../J.a1/RESUME.json",
        "/w/jobs/J.a1/OUTCOME.json",
        "/w/jobs/J.a1/RESUME.json.tmp",
        "",
        "/",
    ] {
        assert!(!ok(bad), "{bad}");
    }
    // `.` components are normalised away by Path::components (harmless: same file)
    assert!(ok("/w/./jobs/J.a1/RESUME.json"));
}

// ------------------------------------------------------------------------------------------------ duplicate keys / versions

/// Plan 3.3 general rule: "duplicate JSON keys are refused". OUTCOME is read with serde derive + a flattened legacy
/// map: duplicates of a struct field are refused, duplicates of a legacy key or inside `Value` fields are not.
/// FINDING ac2-m4 (minor): OUTCOME (and the job spec's `Value`-typed fields, the cases input) accept duplicate keys
/// where the reader keeps the last value; another reader (validate.py, json.loads) may keep a different one.
/// Smallest fix: read every contract document through `canon::canonical_record` first (it refuses decoded
/// duplicate keys at any depth) and parse the canonical bytes.
#[test]
#[ignore = "FINDING ac2-m4: fails until OUTCOME refuses duplicate keys at every depth"]
fn ac2_outcome_duplicate_keys_refused() {
    let (run, s) = honest("dup");
    let good = String::from_utf8(std::fs::read(run.layout.outcome()).unwrap()).unwrap();
    let _ = s;
    let cases: Vec<(&str, String)> = vec![
        ("duplicate job", good.replacen("{", &format!("{{\"job\":{},", serde_json::to_string(&serde_json::from_str::<serde_json::Value>(&good).unwrap()["job"]).unwrap()), 1)),
        ("duplicate legacy key", good.replacen("{", "{\"complete\":true,\"complete\":false,", 1)),
        ("duplicate in stop.detail", good.replacen("\"detail\": null", "\"detail\": {\"a\":1,\"a\":2}", 1)),
    ];
    let mut accepted = Vec::new();
    for (what, b) in &cases {
        let parsed = Outcome::from_slice(b.as_bytes());
        eprintln!("{what}: from_slice {}", if parsed.is_ok() { "ACCEPTS" } else { "refuses" });
        if parsed.is_ok() {
            accepted.push(*what);
        }
    }
    assert!(accepted.is_empty(), "duplicate keys accepted in OUTCOME: {accepted:?}");
}

#[test]
fn ac2_versions_and_unknown_fields() {
    // job spec: unknown major version and unknown fields are refused (40)
    let (run, s) = honest("ver");
    let mut v: serde_json::Value = serde_json::from_slice(&run.spec.to_json_line()).unwrap();
    v["schema"] = "rustred.child-job.v2".into();
    assert!(JobSpec::from_json(&serde_json::to_vec(&v).unwrap()).is_err(), "spec v2");
    let mut v: serde_json::Value = serde_json::from_slice(&run.spec.to_json_line()).unwrap();
    v["budgets"]["work"]["seeds_per_cas"] = 5.into();
    assert!(JobSpec::from_json(&serde_json::to_vec(&v).unwrap()).is_err(), "misspelt budget");
    let mut v: serde_json::Value = serde_json::from_slice(&run.spec.to_json_line()).unwrap();
    v["kind"] = "generate2".into();
    assert!(JobSpec::from_json(&serde_json::to_vec(&v).unwrap()).is_err(), "unknown kind");
    // OUTCOME: unknown major version -> invalid (row 22); unknown exit code / stop code / status -> unreadable
    let mut j = outcome_for(&run, ExitCode::Complete, None, Some(&s), 3);
    j.schema = "rustred.child-job.outcome.v2".into();
    write_outcome(&run, &j);
    let (ev, c) = judge(&run, WaitStatus::Exited(0));
    assert!(matches!(ev.outcome, OutcomeState::Invalid(_)), "{:?}", ev.outcome);
    assert_eq!(c.row, 22);
    let good = Outcome::new(outcome_for(&run, ExitCode::Complete, None, Some(&s), 3)).to_bytes();
    let good = String::from_utf8(good).unwrap();
    for (from, to) in [
        ("\"exit_code\": 0", "\"exit_code\": 13"),
        ("\"status\": \"complete\"", "\"status\": \"completed\""),
        ("\"code\": null", "\"code\": \"max_case\""),
        ("\"kind\": \"generate\"", "\"kind\": \"generate2\""),
        ("\"end\": \"end\"", "\"end\": \"ended\""),
    ] {
        assert!(good.contains(from), "{from}");
        let bad = good.replacen(from, to, 1);
        let _ = std::fs::remove_file(run.layout.outcome());
        std::fs::write(run.layout.outcome(), bad.as_bytes()).unwrap();
        let (ev, c) = judge(&run, WaitStatus::Exited(0));
        assert!(matches!(ev.outcome, OutcomeState::Unreadable(_)), "{to}: {:?}", ev.outcome);
        assert_eq!((c.row, c.accept), (22, Accept::Nothing), "{to}");
    }
}

/// Observation ac2-n4: OUTCOME readers tolerate unknown fields (plan 3.3), so a field the writer MISSPELLS takes its
/// default. Every acceptance-relevant default fails closed (seal figures, rules_committed, elided count), EXCEPT the
/// pair `time_decisions` / `time_decided`: misspelling both makes a time-decided attempt look deterministic.
#[test]
fn ac2_misspelt_time_decided_pair_reads_as_deterministic() {
    let (run, s) = honest("time");
    let mut v = serde_json::to_value(Outcome::new(outcome_for(&run, ExitCode::Complete, None, Some(&s), 3))).unwrap();
    v["job"]["time_decisionz"] = serde_json::json!([{"what":"guard_route","limit_s":60.0,"measured_s":61.0,"taken":"content"}]);
    v["job"]["time_decidedd"] = true.into();
    let _ = std::fs::remove_file(run.layout.outcome());
    std::fs::write(run.layout.outcome(), serde_json::to_vec(&v).unwrap()).unwrap();
    let (_, c) = judge(&run, WaitStatus::Exited(0));
    assert_eq!((c.row, c.accept, c.time_decided), (1, Accept::Canonical, false));
}

// ------------------------------------------------------------------------------------------------ outputs beyond the table

/// FINDING ac2-m5 (minor): `verify_outputs` checks only the FIRST `outputs.extra` entry of a role; a second entry of
/// the same role (any path under out/, `sealed: false`, wrong figures) is ignored and the attempt is accepted.
/// No consumer reads `outputs.extra` today [read: grep of crates/rustred-campaign/src], so latent. Smallest fix:
/// refuse duplicate roles in `JobOutcome::check`.
#[test]
#[ignore = "FINDING ac2-m5: fails until duplicate outputs.extra roles are refused"]
fn ac2_outputs_extra_duplicate_role_refused() {
    let (run, s) = honest("extra");
    // a sealed subcases file
    let mut w = RecordWriter::create(&run.layout.out_dir().join("F.subcases.jsonl")).unwrap();
    w.append(br#"{"type":"subcase","kind":"case","case":{"frame":"PRESENTED","fixed":[1]}}"#).unwrap();
    let sub = w.seal(br#"{"type":"end","subcases":1}"#).unwrap();
    let mut j = outcome_for(&run, ExitCode::Complete, None, Some(&s), 3);
    j.outputs.extra.push(ExtraOutput {
        role: "subcases".into(),
        path: "out/F.subcases.jsonl".into(),
        bytes: sub.bytes,
        blake3: sub.blake3.clone(),
        records: Some(sub.records),
        canonical_blake3: Some(sub.canonical_blake3.clone()),
        end: Some(EndKind::End),
        sealed: true,
    });
    j.outputs.extra.push(ExtraOutput {
        role: "subcases".into(),
        path: "out/F.subcases.other.jsonl".into(),
        bytes: 1,
        blake3: D0.into(),
        records: None,
        canonical_blake3: None,
        end: None,
        sealed: false,
    });
    write_outcome(&run, &j);
    let (ev, c) = judge(&run, WaitStatus::Exited(0));
    assert_eq!(c.accept, Accept::Nothing, "{c:?} {:?}", ev.unsealed_why);
}

/// Observation ac2-n5: the cert stream is verified only when OUTCOME lists it; a generate child that omits
/// `outputs.cert` over a torn out/F.cert.jsonl is accepted. Nothing reads the cert in the supervisor today; a
/// consumer of certificates must not read out/F.cert.jsonl unless OUTCOME listed and the verifier sealed it.
#[test]
fn ac2_cert_is_verified_only_when_listed() {
    let (run, s) = honest("cert");
    std::fs::write(run.layout.cert(), b"{\"type\":\"cert_header\"}\n{\"type\":\"basis\",\"x\":").unwrap();
    let (_, c) = judge(&run, WaitStatus::Exited(0));
    assert_eq!(c.accept, Accept::Canonical, "omitted cert is not looked at");
    let mut j = outcome_for(&run, ExitCode::Complete, None, Some(&s), 3);
    j.outputs.cert = Some(rustred_campaign_contract::outcome::StreamOutput {
        path: "out/F.cert.jsonl".into(),
        bytes: Some(10),
        blake3: Some(D0.into()),
        sealed: true,
    });
    write_outcome(&run, &j);
    let (_, c) = judge(&run, WaitStatus::Exited(0));
    assert_eq!(c.accept, Accept::Nothing, "a listed torn cert blocks acceptance");
}

/// FINDING ac2-m6 (minor): the family binding (table header `family_blake3` = spec family input) is checked only
/// for a header on the FIRST line. A table whose first line is not a header, with a header carrying a FOREIGN
/// family digest further down, is accepted; and that header's `family_file` is volatile (any 64-hex
/// `family_blake3` in any header makes it so), so the foreign path does not even reach the canonical digest.
/// Smallest fix: in the verifier, require at most one header and, when present, on line 1; compare every header's
/// `family_blake3`.
#[test]
#[ignore = "FINDING ac2-m6: fails until a header off line 1 (or a second header) is refused"]
fn ac2_family_binding_only_checks_a_first_line_header() {
    let run = new_run("fam", 1);
    let mut w = RecordWriter::create(&run.layout.table()).unwrap();
    w.append(rule(0).as_bytes()).unwrap();
    w.append(
        br#"{"type":"header","family_file":"/other/T999.sym","family_blake3":"9999999999999999999999999999999999999999999999999999999999999999"}"#,
    )
    .unwrap();
    let s = w.seal(br#"{"type":"end","rules":1}"#).unwrap();
    write_outcome(&run, &outcome_for(&run, ExitCode::Complete, None, Some(&s), 1));
    let (_, c) = judge(&run, WaitStatus::Exited(0));
    assert_eq!(c.accept, Accept::Nothing, "a foreign family digest was accepted: {c:?}");
}

/// Observation ac2-n6: rule identity. (chart, index) is unique; (chart, target, case) only when `target` is present:
/// rules WITHOUT a target repeat freely. Negative and arbitrarily large integer indices pass.
#[test]
fn ac2_rule_identity_edges() {
    let run = new_run("ident", 1);
    let mut w = RecordWriter::create(&run.layout.table()).unwrap();
    w.append(br#"{"type":"rule","chart":"1","index":0,"case":"c","rhs":[]}"#).unwrap();
    w.append(br#"{"type":"rule","chart":"1","index":1,"case":"c","rhs":[]}"#).unwrap();
    w.append(br#"{"type":"rule","chart":"1","index":-7,"case":"d","target":"I(1)","rhs":[]}"#).unwrap();
    w.append(br#"{"type":"rule","chart":"1","index":123456789012345678901234567890,"case":"e","target":"I(2)","rhs":[]}"#).unwrap();
    let s = w.seal(br#"{"type":"end","rules":4}"#).unwrap();
    write_outcome(&run, &outcome_for(&run, ExitCode::Complete, None, Some(&s), 4));
    let (_, c) = judge(&run, WaitStatus::Exited(0));
    assert_eq!(c.accept, Accept::Canonical);
}

/// Checker kinds: their results file is the sealed output; a checker never accepts on 10 / 20, a producer never on
/// 11 / 12 (exercised end to end through assess).
#[test]
fn ac2_checker_results_sealing_end_to_end() {
    let d = TempDir::new("checker");
    let root = d.path().join("C.a1");
    std::fs::create_dir_all(root.join("out")).unwrap();
    let mut spec = sample_spec(root.to_str().unwrap());
    spec.kind = JobKind::Descent;
    spec.args = vec![];
    spec.budgets = Default::default();
    spec.program.features_required = vec![];
    let bytes = spec.to_json_line();
    std::fs::write(root.join("job.json"), &bytes).unwrap();
    let digest = rustred_campaign_contract::spec::spec_blake3(&bytes);
    let layout = RunLayout::new(&root);
    let mut w = RecordWriter::create(&root.join("out/results.jsonl")).unwrap();
    w.append(br#"{"type":"verdict","verdict":"PASS","seconds":1.0}"#).unwrap();
    let s = w.seal(br#"{"type":"end"}"#).unwrap();
    let mut j = JobOutcome::for_spec(&spec, &digest, program_info(), ExitCode::Complete, StopInfo::none());
    j.outputs.extra.push(ExtraOutput {
        role: "results".into(),
        path: "out/results.jsonl".into(),
        bytes: s.bytes,
        blake3: s.blake3.clone(),
        records: Some(s.records),
        canonical_blake3: Some(s.canonical_blake3.clone()),
        end: Some(EndKind::End),
        sealed: true,
    });
    Outcome::new(j.clone()).write_new(&layout.outcome()).unwrap();
    let ev = classify::assess(&layout, &spec, &digest, WaitStatus::Exited(0), KillReason::None, false);
    let c = classify::classify(&ev);
    assert_eq!((c.row, c.accept), (1, Accept::Canonical), "{:?} {:?}", ev.outcome, ev.unsealed_why);
    // the same OUTCOME with exit 20 (budget_work) is a contract violation for a checker
    let mut j20 = j.clone();
    j20.exit_code = ExitCode::BudgetWork;
    j20.status = rustred_campaign_contract::stop::JobStatus::Budget;
    j20.stop.code = Some(StopCode::BudgetWork);
    std::fs::remove_file(layout.outcome()).unwrap();
    Outcome::new(j20).write_new(&layout.outcome()).unwrap();
    let ev = classify::assess(&layout, &spec, &digest, WaitStatus::Exited(20), KillReason::None, false);
    assert_eq!(classify::classify(&ev).accept, Accept::Nothing);
    let _ = Path::new("");
}
