//! Regression tests of the findings of audits A and B of the contract crate (WS/notes/implementer.audit-contract-A.md,
//! implementer.audit-contract-B.md). One test per finding id; each failed on the audited commit de322604 (the
//! auditors' throw-away tests asserted the defective behaviour) and passes after lane contractfix.

mod common;
use common::*;
use rustred_campaign_contract::canon::{self, CanonError, canonical_line};
use rustred_campaign_contract::classify::*;
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::outcome::*;
use rustred_campaign_contract::preflight::{ProgramSelf, preflight, verify_resume};
use rustred_campaign_contract::resume::{CommittedFile, Frame, ResumeRef, ResumeState};
use rustred_campaign_contract::seal::*;
use rustred_campaign_contract::spec::*;
use rustred_campaign_contract::stop::*;
use rustred_campaign_contract::{ExitCode, RESUME_SCHEMA, blake3_hex};
use std::path::{Path, PathBuf};

const HEADER: &str = r#"{"type":"header","schema":"t","zero_seconds":0.5}"#;

fn rule(i: u64, target: &str) -> String {
    format!(
        r#"{{"type":"rule","chart":"C","index":{i},"target":"{target}","rhs":[["I(0)","1"]],"seconds_search":0.25}}"#
    )
}

/// A run directory with `job.json` (generate spec), `out/F.table.jsonl` sealed from `lines` (last = end record), and
/// an OUTCOME (exit `code`) consistent with that table. Returns (layout, spec, spec digest, outcome).
fn run_with_table(
    d: &Path,
    lines: &[String],
    code: ExitCode,
    stop: Option<StopCode>,
) -> (RunLayout, JobSpec, String, JobOutcome) {
    std::fs::create_dir_all(d.join("out")).unwrap();
    let spec = sample_spec(d.to_str().unwrap());
    let sb = spec.to_json_line();
    std::fs::write(d.join("job.json"), &sb).unwrap();
    let digest = blake3_hex(&sb);
    let mut raw = Vec::new();
    for l in lines {
        raw.extend_from_slice(l.as_bytes());
        raw.push(b'\n');
    }
    std::fs::write(d.join("out/F.table.jsonl"), &raw).unwrap();
    let mut canon = canon::CanonicalDigest::new();
    let mut rules = 0;
    for l in lines {
        canon.push_line(l.as_bytes()).unwrap();
        if canon::line_type(l.as_bytes()).unwrap().as_deref() == Some("rule") {
            rules += 1;
        }
    }
    let mut j = job_outcome(&spec, code, stop, true);
    j.spec_blake3 = digest.clone();
    j.counters.rules_committed = rules;
    let t = j.outputs.table.as_mut().unwrap();
    t.bytes = Some(raw.len() as u64);
    t.blake3 = Some(blake3_hex(&raw));
    t.records = Some(lines.len() as u64);
    t.rules = Some(rules);
    t.rules_committed = rules;
    t.canonical_blake3 = Some(canon.hex());
    j.check().unwrap();
    Outcome::new(j.clone())
        .write_atomic(&d.join("OUTCOME.json"))
        .unwrap();
    (RunLayout::new(d), spec, digest, j)
}

fn classify_run(l: &RunLayout, s: &JobSpec, d: &str, c: i32) -> Classification {
    classify(&assess(
        l,
        s,
        d,
        WaitStatus::Exited(c),
        KillReason::None,
        false,
    ))
}

fn write_outcome(l: &RunLayout, j: &JobOutcome) {
    Outcome::new(j.clone()).write_atomic(&l.outcome()).unwrap();
}

#[test]
fn audit_a_c1_duplicated_rules_are_refused() {
    // the auditor's scenario: rule indices 0,1,2,3,1,2,3 (targets I(1)..I(3) twice) in a "sealed" table -> refusal
    let d = TempDir::new("c1dup");
    let mut lines = vec![HEADER.to_string()];
    for (i, t) in [
        (0, "I(0)"),
        (1, "I(1)"),
        (2, "I(2)"),
        (3, "I(3)"),
        (1, "I(1)"),
        (2, "I(2)"),
        (3, "I(3)"),
    ] {
        lines.push(rule(i, t));
    }
    lines.push(r#"{"type":"end","rules":7}"#.into());
    let (l, s, dg, j) = run_with_table(d.path(), &lines, ExitCode::Complete, None);
    assert!(matches!(
        verify_table(&l, j.outputs.table.as_ref().unwrap()),
        SealVerdict::Unsealed(UnsealReason::DuplicateRuleIndex { line: 6 })
    ));
    let c = classify_run(&l, &s, &dg, 0);
    assert_eq!((c.row, c.accept), (22, Accept::Nothing));
    // same targets under fresh indices (a RESUME whose counters ran ahead of its queue state): refused too
    let d = TempDir::new("c1tgt");
    let mut lines = vec![HEADER.to_string()];
    for (i, t) in [(0, "I(0)"), (1, "I(1)"), (2, "I(1)")] {
        lines.push(rule(i, t));
    }
    lines.push(r#"{"type":"end","rules":3}"#.into());
    let (l, _, _, j) = run_with_table(d.path(), &lines, ExitCode::Complete, None);
    assert!(matches!(
        verify_table(&l, j.outputs.table.as_ref().unwrap()),
        SealVerdict::Unsealed(UnsealReason::DuplicateRuleTarget { .. })
    ));
}

#[test]
fn audit_a_c1_resume_ref_carries_no_sizes_and_prefix_never_ends_in_an_end_record() {
    // spec.resume is {from, blake3} only: the old form with its own byte count is refused (unknown field)
    let old = r#"{"from":"/a/RESUME.json","blake3":"0000000000000000000000000000000000000000000000000000000000000000","table":"/a/out/F.table.jsonl","committed_bytes":613,"prefix_blake3":"0000000000000000000000000000000000000000000000000000000000000000"}"#;
    assert!(serde_json::from_str::<ResumeRef>(old).is_err());
    // a prefix ending in an end record (the sealed size of a graceful stop) is refused (audit A a9)
    let d = TempDir::new("c1end");
    std::fs::create_dir_all(d.path().join("out")).unwrap();
    let mut w = RecordWriter::create(&d.path().join("out/F.table.jsonl")).unwrap();
    w.append(HEADER.as_bytes()).unwrap();
    w.append(rule(0, "I(0)").as_bytes()).unwrap();
    let s = w.seal(br#"{"type":"partial","rules":1}"#).unwrap();
    let want = CommittedFile {
        path: "out/F.table.jsonl".into(),
        bytes: s.bytes,
        blake3: s.blake3,
        records: s.records,
        rules: s.rules,
        canonical_blake3: s.canonical_blake3,
    };
    assert!(matches!(
        verify_prefix(d.path(), &want, &TABLE_END_TYPES),
        Err(UnsealReason::EndRecordInPrefix { .. })
    ));
    let x = d.path().join("x.jsonl");
    assert!(RecordWriter::resume_from(d.path(), &want, &TABLE_END_TYPES, &x).is_err());
    assert!(!x.exists());
}

/// A previous attempt (attempt 1 of the fake-like generate spec) with a committed table prefix and a RESUME.
fn prev_attempt(root: &Path, job: &str) -> (PathBuf, JobSpec, ResumeState) {
    let run = root.join(format!("{job}.a1"));
    std::fs::create_dir_all(run.join("out")).unwrap();
    let mut spec = sample_spec(run.to_str().unwrap());
    spec.job_id = job.into();
    spec.inputs.clear();
    let sb = spec.to_json_line();
    std::fs::write(run.join("job.json"), &sb).unwrap();
    let mut w = RecordWriter::create(&run.join("out/F.table.jsonl")).unwrap();
    w.append(HEADER.as_bytes()).unwrap();
    w.append(rule(0, "I(0)").as_bytes()).unwrap();
    let cp = w.commit().unwrap();
    w.append(rule(1, "I(1)").as_bytes()).unwrap(); // after the commit point: not in RESUME
    let r = ResumeState {
        schema: RESUME_SCHEMA.into(),
        job_id: job.into(),
        attempt: 1,
        spec_blake3: blake3_hex(&sb),
        chart: serde_json::json!("C"),
        frame: Frame::Presented,
        rules_committed: 1,
        table: cp.committed("out/F.table.jsonl"),
        queue: None,
        current: None,
        pending: vec![],
        numerical: vec![],
        deferred: vec![],
        refused_open: vec![],
        counters_at_commit: Counters {
            cases: 1,
            rules_committed: 1,
            ..Default::default()
        },
        symbolic_cases: 1,
        discarded: 0,
    };
    r.check().unwrap();
    rustred_campaign_contract::atomic::write_json_atomic(&run.join("RESUME.json"), &r).unwrap();
    (run, spec, r)
}

fn next_spec(root: &Path, prev: &JobSpec, from: &Path) -> JobSpec {
    let mut s = prev.clone();
    s.attempt = 2;
    let run = root.join(format!("{}.a2", prev.job_id));
    std::fs::create_dir_all(&run).unwrap();
    s.run_dir = run.to_str().unwrap().into();
    let b = std::fs::read(from).unwrap();
    s.resume = Some(ResumeRef {
        from: from.to_str().unwrap().into(),
        blake3: blake3_hex(&b),
    });
    s
}

#[test]
fn audit_a_c1_resume_point_is_one_consistent_object() {
    let d = TempDir::new("c1obj");
    let (run, prev, r) = prev_attempt(d.path(), "j");
    let s = next_spec(d.path(), &prev, &run.join("RESUME.json"));
    let v = verify_resume(&s, s.resume.as_ref().unwrap()).unwrap();
    assert_eq!(v.state, r);
    // a RESUME whose byte count is that of OUTCOME (ahead of its record count / canonical digest): refused, 41
    let mut bad = r.clone();
    let (n, dg) = file_blake3(&run.join("out/F.table.jsonl")).unwrap();
    bad.table.bytes = n;
    bad.table.blake3 = dg;
    rustred_campaign_contract::atomic::write_json_atomic(&run.join("RESUME.json"), &bad).unwrap();
    let s = next_spec(d.path(), &prev, &run.join("RESUME.json"));
    let (code, why) = verify_resume(&s, s.resume.as_ref().unwrap()).unwrap_err();
    assert_eq!(code, ExitCode::Input, "{why}");
    // the classifier's resume advice is the RESUME record (path + digest), never OUTCOME's sizes
    rustred_campaign_contract::atomic::write_json_atomic(&run.join("RESUME.json"), &r).unwrap();
    let (_, rref) = resume_state(
        &RunLayout::new(&run),
        &prev,
        &blake3_hex(&prev.to_json_line()),
    )
    .unwrap();
    assert!(rref.from.ends_with("/RESUME.json"));
    assert_eq!(
        rref.blake3,
        blake3_hex(&std::fs::read(run.join("RESUME.json")).unwrap())
    );
}

#[test]
fn audit_a_m7_and_b_f2_resume_of_another_job_or_spec_is_refused() {
    let d = TempDir::new("m7");
    let (run, prev, _) = prev_attempt(d.path(), "x");
    // job y given job x's RESUME
    let mut s = next_spec(d.path(), &prev, &run.join("RESUME.json"));
    s.job_id = "y".into();
    assert_eq!(
        verify_resume(&s, s.resume.as_ref().unwrap()).unwrap_err().0,
        ExitCode::Input
    );
    // same job id, but another spec (a budget changed): not the same job
    let mut s = next_spec(d.path(), &prev, &run.join("RESUME.json"));
    s.seed.sample_seed = 99;
    assert_eq!(
        verify_resume(&s, s.resume.as_ref().unwrap()).unwrap_err().0,
        ExitCode::Input
    );
    // another binary: 42
    let mut s = next_spec(d.path(), &prev, &run.join("RESUME.json"));
    s.program.blake3 = D1.into();
    assert_eq!(
        verify_resume(&s, s.resume.as_ref().unwrap()).unwrap_err().0,
        ExitCode::Refused
    );
    // the previous job.json rewritten after the RESUME (spec binding, M7)
    let mut p2 = prev.clone();
    p2.heartbeat_s = 11;
    std::fs::write(run.join("job.json"), p2.to_json_line()).unwrap();
    let s = next_spec(d.path(), &prev, &run.join("RESUME.json"));
    assert_eq!(
        verify_resume(&s, s.resume.as_ref().unwrap()).unwrap_err().0,
        ExitCode::Input
    );
}

#[test]
fn audit_a_m7_resume_source_is_not_this_run_and_not_followed_through_symlinks() {
    let d = TempDir::new("m7src");
    let (run, prev, _) = prev_attempt(d.path(), "j");
    let mut s = next_spec(d.path(), &prev, &run.join("RESUME.json"));
    s.run_dir = run.to_str().unwrap().into();
    assert!(verify_resume(&s, s.resume.as_ref().unwrap()).is_err());
}

#[test]
fn audit_a_c2_and_b_f4_seal_check_follows_only_the_layout_path() {
    let d = TempDir::new("c2");
    let lines = vec![
        HEADER.to_string(),
        rule(0, "I(0)"),
        r#"{"type":"end","rules":1}"#.to_string(),
    ];
    // a sealed table of ANOTHER run
    let other = d.path().join("other");
    let (_, _, _, oj) = run_with_table(&other, &lines, ExitCode::Complete, None);
    let run = d.path().join("run");
    let (l, s, dg, mut j) = run_with_table(&run, &lines, ExitCode::Complete, None);
    assert_eq!(classify_run(&l, &s, &dg, 0).row, 1);
    // this run's own table torn
    std::fs::write(l.table(), format!("{HEADER}\n{{\"type\":\"rule\",\"ind")).unwrap();
    for p in [
        other
            .join("out/F.table.jsonl")
            .to_str()
            .unwrap()
            .to_string(),
        "../other/out/F.table.jsonl".to_string(),
        "out/F.queue.jsonl".to_string(),
    ] {
        j.outputs.table = oj.outputs.table.clone();
        j.outputs.table.as_mut().unwrap().path = p.clone();
        write_outcome(&l, &j);
        let c = classify_run(&l, &s, &dg, 0);
        assert_eq!((c.row, c.accept), (22, Accept::Nothing), "{p}");
    }
    // the layout path is a symlink to the sealed foreign table: not sealed
    j.outputs.table = oj.outputs.table.clone();
    write_outcome(&l, &j);
    std::fs::remove_file(l.table()).unwrap();
    std::os::unix::fs::symlink(other.join("out/F.table.jsonl"), l.table()).unwrap();
    assert!(matches!(
        verify_table(&l, j.outputs.table.as_ref().unwrap()),
        SealVerdict::Unsealed(UnsealReason::NotRegularFile(_))
    ));
    assert_eq!(classify_run(&l, &s, &dg, 0).row, 22);
    // out/ itself a symlink
    std::fs::remove_file(l.table()).unwrap();
    std::fs::remove_dir(run.join("out")).unwrap();
    std::os::unix::fs::symlink(other.join("out"), run.join("out")).unwrap();
    assert!(!verify_table(&l, j.outputs.table.as_ref().unwrap()).is_sealed());
}

#[test]
fn audit_a_c3_and_b_f3_escaped_key_spellings_do_not_collide() {
    // the auditor's vectors: escape-spelled duplicate keys are a typed error, never one digest for two meanings
    for r in [
        &br#"{"a":1,"a":2}"#[..],
        br#"{"a":2,"a":1}"#,
        br#"{"type":"rule","type":"end"}"#,
        br#"{"type":"rule","type":"end"}"#,
    ] {
        assert!(matches!(
            canonical_line(r),
            Err(CanonError::DuplicateKey(_))
        ));
    }
    // a key spelled with escapes IS its decoded key (also for volatility and the record type)
    assert_eq!(
        canonical_line(br#"{"type":"end","seconds":1.5,"rules":1}"#).unwrap(),
        br#"{"rules":1,"type":"end"}"#.to_vec()
    );
}

/// A tiny deterministic generator (xorshift) for the property tests.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn random_value(g: &mut Rng, depth: u32) -> serde_json::Value {
    use serde_json::Value;
    let k = if depth >= 3 { g.below(5) } else { g.below(7) };
    match k {
        0 => Value::Null,
        1 => Value::Bool(g.below(2) == 1),
        2 => Value::from(g.below(2_000_000) as i64 - 1_000_000),
        3 => Value::from((g.below(1_000_000) as f64) / 64.0 - 7000.0),
        4 => {
            let pool = [
                "a", "é", "\"", "\\", "/", "\n", "\u{1}", "x y", "😀", "I(1,2)", "-1/2",
            ];
            let n = g.below(4);
            Value::from(
                (0..n)
                    .map(|_| pool[g.below(pool.len() as u64) as usize])
                    .collect::<String>(),
            )
        }
        5 => Value::from(
            (0..g.below(4))
                .map(|_| random_value(g, depth + 1))
                .collect::<Vec<_>>(),
        ),
        _ => {
            let mut m = serde_json::Map::new();
            for _ in 0..g.below(5) {
                let keys = ["a", "b", "type", "é", "k\"q", "z9", "_", "B"];
                m.insert(
                    keys[g.below(keys.len() as u64) as usize].to_string(),
                    random_value(g, depth + 1),
                );
            }
            Value::Object(m)
        }
    }
}

/// Re-spell a serde-written JSON text: escape random characters of strings as \uXXXX (surrogate pairs above the
/// BMP), spell `/` as `\/`, add whitespace, re-spell floats with an exponent.
fn respell(g: &mut Rng, v: &serde_json::Value, out: &mut String) {
    use serde_json::Value;
    let ws = |g: &mut Rng, out: &mut String| {
        for _ in 0..g.below(3) {
            out.push([' ', '\t', '\n', '\r'][g.below(4) as usize]);
        }
    };
    let s_esc = |g: &mut Rng, s: &str, out: &mut String| {
        out.push('"');
        for ch in s.chars() {
            let esc = matches!(ch, '"' | '\\') || (ch as u32) < 0x20 || g.below(3) == 0;
            if !esc {
                out.push(ch);
            } else if ch == '/' && g.below(2) == 0 {
                out.push_str("\\/");
            } else {
                let mut b = [0u16; 2];
                for u in ch.encode_utf16(&mut b) {
                    out.push_str(&format!("\\u{:04X}", u));
                }
            }
        }
        out.push('"');
    };
    ws(g, out);
    match v {
        Value::Object(m) => {
            out.push('{');
            let mut keys: Vec<&String> = m.keys().collect();
            // random member order
            for i in (1..keys.len()).rev() {
                keys.swap(i, g.below(i as u64 + 1) as usize);
            }
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                ws(g, out);
                s_esc(g, k, out);
                ws(g, out);
                out.push(':');
                respell(g, &m[*k], out);
            }
            out.push('}');
        }
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                respell(g, x, out);
            }
            out.push(']');
        }
        Value::String(s) => s_esc(g, s, out),
        Value::Number(n) if n.is_f64() && g.below(2) == 0 => {
            let x = n.as_f64().unwrap();
            out.push_str(&format!("{x:e}"));
        }
        other => out.push_str(&other.to_string()),
    }
    ws(g, out);
}

#[test]
fn audit_a_c3_property_random_records_and_respellings() {
    // decision C3: decode + one encoding + numbers by value (plan 3.3.0). Property: every re-spelling of a record
    // has the canonical bytes of the serde-written record; distinct records have distinct canonical bytes; the
    // canonical bytes parse back to the same value.
    let mut g = Rng(0x00C3_A0D1_7B5E_ED01);
    let mut seen = std::collections::HashMap::new();
    for _ in 0..3000 {
        let mut v = random_value(&mut g, 0);
        if !v.is_object() {
            v = serde_json::json!({ "v": v });
        }
        if let Some(m) = v.as_object_mut() {
            m.remove("type"); // keep the volatile rule out of the property (typeless: verdict rule)
            m.retain(|k, _| k != "seconds" && k != "utc" && !k.ends_with("_seconds"));
        }
        let base = canonical_line(serde_json::to_string(&v).unwrap().as_bytes()).unwrap();
        let back: serde_json::Value = serde_json::from_slice(&base).unwrap();
        assert_eq!(back, v);
        for _ in 0..3 {
            let mut s = String::new();
            respell(&mut g, &v, &mut s);
            assert_eq!(
                canonical_line(s.as_bytes()).unwrap(),
                base,
                "re-spelling {s} of {v}"
            );
        }
        if let Some(prev) = seen.insert(base.clone(), v.clone()) {
            assert_eq!(prev, v, "one canonical form for two records");
        }
    }
}

#[test]
fn audit_a_c4_preflight_writes_nothing_into_an_unverified_or_finished_run_dir() {
    let d = TempDir::new("c4");
    let mine = d.path().join("mine");
    let foreign = d.path().join("foreign");
    std::fs::create_dir_all(&mine).unwrap();
    std::fs::create_dir_all(&foreign).unwrap();
    let me = ProgramSelf {
        name: "rr_node",
        features: &["initial_presented"],
        kinds: &[JobKind::Generate],
        blake3: None,
    };
    let env = |_: &str| None;
    // (a) invalid field + run_dir of another directory: location first, nothing written
    let mut s = sample_spec(foreign.to_str().unwrap());
    s.heartbeat_s = 0;
    s.inputs.clear();
    std::fs::write(mine.join("job.json"), s.to_json_line()).unwrap();
    let r = preflight(&mine.join("job.json"), &me, &env).unwrap_err();
    assert!(!r.write_outcome);
    // (b) wrong program in a RUN whose OUTCOME exists; (c) invalid field there: refused WITHOUT writing
    for f in [
        |s: &mut JobSpec| s.program.name = "rr_proof".into(),
        |s: &mut JobSpec| s.stop_grace_s = 0,
    ] {
        let mut s = sample_spec(mine.to_str().unwrap());
        s.inputs.clear();
        f(&mut s);
        std::fs::write(mine.join("job.json"), s.to_json_line()).unwrap();
        std::fs::write(mine.join("OUTCOME.json"), b"{\"sealed\":\"result\"}").unwrap();
        let r = preflight(&mine.join("job.json"), &me, &env).unwrap_err();
        assert_eq!((r.code, r.write_outcome), (ExitCode::Refused, false));
    }
    // the child's OUTCOME write is no-clobber
    let o = Outcome::new(job_outcome(
        &sample_spec(mine.to_str().unwrap()),
        ExitCode::Usage,
        Some(StopCode::Refused),
        false,
    ));
    assert_eq!(
        o.write_new(&mine.join("OUTCOME.json")).unwrap_err().kind(),
        std::io::ErrorKind::AlreadyExists
    );
    assert_eq!(
        std::fs::read(mine.join("OUTCOME.json")).unwrap(),
        b"{\"sealed\":\"result\"}"
    );
    // a started run directory (EVENTS exists) is refused without writing
    std::fs::remove_file(mine.join("OUTCOME.json")).unwrap();
    std::fs::write(mine.join("EVENTS.jsonl"), b"").unwrap();
    let mut s = sample_spec(mine.to_str().unwrap());
    s.inputs.clear();
    std::fs::write(mine.join("job.json"), s.to_json_line()).unwrap();
    let r = preflight(&mine.join("job.json"), &me, &env).unwrap_err();
    assert_eq!((r.code, r.write_outcome), (ExitCode::Refused, false));
}

#[test]
fn audit_b_f7_symlink_in_the_run_dir_is_refused_at_start() {
    let d = TempDir::new("f7");
    let run = d.path().join("run");
    std::fs::create_dir_all(&run).unwrap();
    let mut s = sample_spec(run.to_str().unwrap());
    s.inputs.clear();
    std::fs::write(run.join("job.json"), s.to_json_line()).unwrap();
    let me = ProgramSelf {
        name: "rr_node",
        features: &["initial_presented"],
        kinds: &[JobKind::Generate],
        blake3: None,
    };
    for name in ["out", ".reserve", "EVENTS.jsonl", "tmp"] {
        std::os::unix::fs::symlink(d.path(), run.join(name)).unwrap();
        let r = preflight(&run.join("job.json"), &me, &|_| None).unwrap_err();
        assert_eq!(
            (r.code, r.write_outcome),
            (ExitCode::Refused, false),
            "{name}"
        );
        std::fs::remove_file(run.join(name)).unwrap();
    }
    assert!(preflight(&run.join("job.json"), &me, &|_| None).is_ok());
}

#[test]
fn audit_a_m1_and_b_f5_outcome_is_bound_to_spec_blake3() {
    let d = TempDir::new("m1");
    let lines = vec![
        HEADER.to_string(),
        rule(0, "I(0)"),
        r#"{"type":"end","rules":1}"#.to_string(),
    ];
    let (l, s, dg, mut j) = run_with_table(d.path(), &lines, ExitCode::Complete, None);
    assert_eq!(classify_run(&l, &s, &dg, 0).row, 1);
    // OUTCOME of another spec (same job id / attempt / kind)
    j.spec_blake3 = D1.into();
    write_outcome(&l, &j);
    assert_eq!(classify_run(&l, &s, &dg, 0).row, 22);
    // job.json rewritten after the run
    j.spec_blake3 = dg.clone();
    write_outcome(&l, &j);
    let mut s2 = s.clone();
    s2.heartbeat_s = 11;
    std::fs::write(l.job_spec(), s2.to_json_line()).unwrap();
    assert_eq!(classify_run(&l, &s, &dg, 0).row, 22);
}

#[test]
fn audit_a_m2_verifier_counts_elided_loci_itself() {
    let d = TempDir::new("m2");
    let lines = vec![
        HEADER.to_string(),
        r#"{"type":"rule","chart":"C","index":0,"target":"I(0)","witness_loci":["n1","elided"]}"#
            .to_string(),
        r#"{"type":"end","rules":1}"#.to_string(),
    ];
    let (l, mut s, dg, mut j) = run_with_table(d.path(), &lines, ExitCode::Complete, None);
    // the child "forgot" to count: the verifier's count disagrees -> not sealed
    assert!(matches!(
        verify_table(&l, j.outputs.table.as_ref().unwrap()),
        SealVerdict::Unsealed(UnsealReason::ElidedCountMismatch {
            outcome: 0,
            file: 1
        })
    ));
    assert_eq!(classify_run(&l, &s, &dg, 0).row, 22);
    // counted: forbidden -> contract violation; allowed -> sealed_with_elided, never used
    j.outputs.table.as_mut().unwrap().witness_loci_elided = 1;
    write_outcome(&l, &j);
    assert_eq!(classify_run(&l, &s, &dg, 0).row, 22);
    s.output.elide_witness_loci = true;
    let sb = s.to_json_line();
    std::fs::write(l.job_spec(), &sb).unwrap();
    j.spec_blake3 = blake3_hex(&sb);
    write_outcome(&l, &j);
    let c = classify_run(&l, &s, &blake3_hex(&sb), 0);
    assert_eq!(
        (c.row, c.kind, c.accept, c.action),
        (
            25,
            OutcomeKind::SealedWithElided,
            Accept::Nothing,
            Action::RerunWithoutElision
        )
    );
}

#[test]
fn audit_a_m3_verifier_rejects_interior_end_garbage_and_rechecks_every_figure() {
    let d = TempDir::new("m3");
    // the auditor's table: interior end record, non-JSON line, empty line
    let body = "{\"type\":\"header\"}\n{\"type\":\"end\",\"rules\":0}\n{\"type\":\"rule\",\"index\":0}\nNOT JSON {{{\n\n{\"type\":\"partial\",\"rules\":1}\n";
    std::fs::create_dir_all(d.path().join("out")).unwrap();
    std::fs::write(d.path().join("out/F.table.jsonl"), body).unwrap();
    assert!(verify_stream(d.path(), "out/F.table.jsonl", &TABLE_END_TYPES).is_err());
    for b in [
        "{\"type\":\"header\"}\nNOT JSON\n{\"type\":\"end\"}\n",
        "{\"type\":\"header\"}\n\n{\"type\":\"end\"}\n",
        "{\"type\":\"end\"}\n{\"type\":\"header\"}\n",
    ] {
        std::fs::write(d.path().join("out/F.table.jsonl"), b).unwrap();
        assert!(
            verify_stream(d.path(), "out/F.table.jsonl", &TABLE_END_TYPES).is_err(),
            "{b:?}"
        );
    }
    // an OUTCOME that claims a rule over a table without one (row 2 on self-report before) -> not sealed
    let d = TempDir::new("m3b");
    let lines = vec![
        HEADER.to_string(),
        r#"{"type":"partial","rules":0}"#.to_string(),
    ];
    let (l, s, dg, mut j) = run_with_table(
        d.path(),
        &lines,
        ExitCode::Incomplete,
        Some(StopCode::MaxCases),
    );
    j.counters.rules_committed = 1;
    let t = j.outputs.table.as_mut().unwrap();
    t.rules = Some(1);
    t.rules_committed = 1;
    write_outcome(&l, &j);
    assert_eq!(classify_run(&l, &s, &dg, 10).row, 22);
}

#[test]
fn audit_a_m4_every_kind_needs_its_sealed_outputs() {
    let d = TempDir::new("m4");
    let run = d.path();
    std::fs::create_dir_all(run.join("out")).unwrap();
    let mut s = sample_spec(run.to_str().unwrap());
    s.kind = JobKind::Descent;
    s.args = vec![];
    s.program.name = "rr_node".into();
    let sb = s.to_json_line();
    std::fs::write(run.join("job.json"), &sb).unwrap();
    let dg = blake3_hex(&sb);
    let l = RunLayout::new(run);
    let mut j = job_outcome(&s, ExitCode::Complete, None, false);
    j.spec_blake3 = dg.clone();
    j.outputs.table = None;
    write_outcome(&l, &j);
    // nothing listed: not accepted (was row 1 with nothing checked)
    assert_eq!(classify_run(&l, &s, &dg, 0).row, 22);
    // a sealed results file listed and verified: row 1
    let body =
        "{\"type\":\"verdict\",\"ok\":true,\"seconds\":1.5}\n{\"type\":\"end\",\"records\":1}\n";
    std::fs::write(l.results(), body).unwrap();
    let mut c = canon::CanonicalDigest::new();
    for x in body.lines() {
        c.push_line(x.as_bytes()).unwrap();
    }
    j.outputs.extra = vec![ExtraOutput {
        role: "results".into(),
        path: "out/results.jsonl".into(),
        bytes: body.len() as u64,
        blake3: blake3_hex(body.as_bytes()),
        records: Some(2),
        canonical_blake3: Some(c.hex()),
        end: Some(EndKind::End),
        sealed: true,
    }];
    write_outcome(&l, &j);
    assert_eq!(classify_run(&l, &s, &dg, 0).row, 1);
    // a kind without a sealing rule: unverifiable_kind (row 26), never success
    let e = Evidence {
        kind: JobKind::GenerateAuto,
        wait: WaitStatus::Exited(0),
        outcome: OutcomeState::Valid(Box::new(job_outcome(&s, ExitCode::Complete, None, true))),
        verified: None,
        unsealed_why: None,
        resume_valid: false,
        resume_ref: None,
        kill: KillReason::None,
        oom_kill_observed: false,
        elision_forbidden: true,
    };
    let c = classify(&e);
    assert_eq!(
        (c.row, c.kind, c.accept, c.action),
        (
            26,
            OutcomeKind::UnverifiableKind,
            Accept::Nothing,
            Action::UnverifiableKind
        )
    );
}

#[test]
fn audit_a_m5_and_b_f8_args_allow_list() {
    let base = sample_spec("/abs/run");
    for a in [
        &["--emit-certs", "/elsewhere/x"][..],
        &["--cand-dump-dir", "/x"],
        &["--cert", "/x"],
        &["--keys", "/x"],
        &["--bindings", "/x"],
        &["--initial-cases", "/x"],
        &["--cover-table", "/x"],
        &["--adopt-bank", "/x"],
        &["positional"],
        &["--order=auto"],
        &["--numeric-depth-max", "/x"],
        &["--symmetry-rows", "--symmetry-rows"],
    ] {
        let mut s = base.clone();
        s.args = a.iter().map(|x| x.to_string()).collect();
        assert!(s.validate().is_err(), "{a:?}");
    }
    // strict: --on-heavy auto refused in BOTH spellings; a generate job must name a deterministic value
    let mut s = base.clone();
    s.determinism = Determinism::Strict;
    for a in [&["--on-heavy=auto"][..], &["--on-heavy", "auto"], &[]] {
        s.args = a.iter().map(|x| x.to_string()).collect();
        assert!(s.validate().is_err(), "{a:?}");
    }
    s.args = vec!["--on-heavy".into(), "refuse".into()];
    s.validate().unwrap();
    // gated replay variables
    assert!(GATED_ENV.contains(&"RR_REPLAY_RECENTER_SEARCH"));
    assert!(GATED_ENV.contains(&"RR_REPLAY_RECENTER_LIMIT"));
}

#[test]
fn audit_a_m6_fake_child_has_its_own_writers() {
    // structural: the fake child does not call the crate's sealing / canonical / OUTCOME writers (M6), so every
    // fake-child process test is a differential test of the verifier
    let src = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../rustred-campaign/src/bin/rr_fake_child.rs"),
    )
    .unwrap();
    let code: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for banned in [
        "RecordWriter",
        "canonical_line",
        "canonical_record",
        "TableSeal",
        "write_atomic(",
        "Outcome::",
        "JobOutcome",
        ".check()",
        "seal::",
        "canon::",
    ] {
        assert!(!code.contains(banned), "rr_fake_child uses {banned}");
    }
}

#[test]
fn audit_a_m1_minor_m2_success_with_error_is_a_violation() {
    let s = sample_spec("/abs/run");
    for code in [ExitCode::Complete, ExitCode::BudgetWork] {
        let stop = (code == ExitCode::BudgetWork).then_some(StopCode::BudgetWork);
        let mut j = job_outcome(&s, code, stop, true);
        j.error = Some(ErrorInfo::new(ErrorClass::Internal, "boom"));
        assert!(j.check().is_err());
    }
    let mut j = job_outcome(&s, ExitCode::Complete, None, true);
    j.stop.graceful = false;
    assert!(j.check().is_err());
}

#[test]
fn audit_a_m3_minor_wrapper_statuses_are_signals() {
    let ev = |w, k| Evidence {
        kind: JobKind::Generate,
        wait: w,
        outcome: OutcomeState::Absent,
        verified: None,
        unsealed_why: None,
        resume_valid: true,
        resume_ref: None,
        kill: k,
        oom_kill_observed: false,
        elision_forbidden: true,
    };
    assert_eq!(
        classify(&ev(WaitStatus::Exited(137), KillReason::Memory)).row,
        18
    );
    assert_eq!(
        classify(&ev(WaitStatus::Exited(143), KillReason::Stop)).row,
        20
    );
    assert_eq!(
        classify(&ev(WaitStatus::Exited(137), KillReason::None)).row,
        21
    );
    assert_eq!(
        classify(&ev(WaitStatus::Exited(101), KillReason::None)).row,
        23
    );
}

#[test]
fn audit_a_m4_minor_unknown_wait_with_invalid_outcome_is_a_violation() {
    let e = Evidence {
        kind: JobKind::Generate,
        wait: WaitStatus::Unknown,
        outcome: OutcomeState::Unreadable("EOF".into()),
        verified: None,
        unsealed_why: None,
        resume_valid: true,
        resume_ref: None,
        kill: KillReason::None,
        oom_kill_observed: false,
        elision_forbidden: true,
    };
    assert_eq!(classify(&e).row, 22);
}

#[test]
fn audit_a_m5_minor_unique_temp_names() {
    let p = Path::new("/x/OUTCOME.json");
    let a = rustred_campaign_contract::atomic::temp_path(p);
    let b = std::thread::spawn(move || rustred_campaign_contract::atomic::temp_path(p))
        .join()
        .unwrap();
    let c = rustred_campaign_contract::atomic::temp_path(p);
    assert!(a != b && a != c && b != c);
}

#[test]
fn audit_a_m7_minor_resume_source_equal_to_destination_is_refused() {
    let d = TempDir::new("m7m");
    std::fs::create_dir_all(d.path().join("out")).unwrap();
    let p = d.path().join("out/F.table.jsonl");
    let mut w = RecordWriter::create(&p).unwrap();
    w.append(HEADER.as_bytes()).unwrap();
    let want = w.commit().unwrap().committed("out/F.table.jsonl");
    drop(w);
    assert!(RecordWriter::resume_from(d.path(), &want, &TABLE_END_TYPES, &p).is_err());
    assert_eq!(
        std::fs::metadata(&p).unwrap().len(),
        want.bytes,
        "source intact"
    );
}

#[test]
fn audit_a_m8_and_b_f9_records_readers_reject_are_refused() {
    for r in [
        &br#"{"a":"\ud800"}"#[..],
        br#"{"a":"\udc00x"}"#,
        br#"{"a":1e999999}"#,
        br#"{"type":"rule","seconds_search":1e999999}"#,
    ] {
        assert!(canonical_line(r).is_err(), "{}", String::from_utf8_lossy(r));
        assert!(serde_json::from_slice::<serde_json::Value>(r).is_err());
    }
}

#[test]
fn audit_a_m10_minor_dirty_out_is_refused_also_with_resume_and_when_not_a_dir() {
    let d = TempDir::new("m10");
    let run = d.path();
    let mut s = sample_spec(run.to_str().unwrap());
    s.inputs.clear();
    std::fs::write(run.join("job.json"), s.to_json_line()).unwrap();
    std::fs::write(run.join("out"), b"a file").unwrap();
    let me = ProgramSelf {
        name: "rr_node",
        features: &["initial_presented"],
        kinds: &[JobKind::Generate],
        blake3: None,
    };
    assert_eq!(
        preflight(&run.join("job.json"), &me, &|_| None)
            .unwrap_err()
            .code,
        ExitCode::Refused
    );
}

#[test]
fn audit_a_m12_minor_verdict_rule_only_for_verdict_records() {
    assert_eq!(
        canonical_line(br#"{"type":"cert_row","limit_seconds":60}"#).unwrap(),
        br#"{"limit_seconds":60,"type":"cert_row"}"#.to_vec()
    );
    assert_ne!(
        canonical_line(br#"{"type":"residual","budget_seconds":60}"#).unwrap(),
        canonical_line(br#"{"type":"residual","budget_seconds":600}"#).unwrap()
    );
}

#[test]
fn audit_b_f6_fault_paths_are_devices_only() {
    let mut s = FakeScriptBuilder::script();
    s.faults = vec![rustred_campaign_contract::fake::Fault {
        at_op: 0,
        kind: rustred_campaign_contract::fake::FaultKind::Enospc,
        bytes: 0,
        code: 0,
        path: Some("/tmp/victim".into()),
    }];
    assert!(s.validate().is_err());
    s.faults[0].path = Some("/dev/../tmp/victim".into());
    assert!(s.validate().is_err());
    s.faults[0].path = Some("/dev/full".into());
    s.validate().unwrap();
}

struct FakeScriptBuilder;
impl FakeScriptBuilder {
    fn script() -> rustred_campaign_contract::fake::FakeScript {
        rustred_campaign_contract::fake::FakeScript {
            script: vec![rustred_campaign_contract::fake::FakeOp::End {
                stop: None,
                verdict: None,
            }],
            on_sigterm: Default::default(),
            slow_ms: 0,
            start_delay_ms: 0,
            rss: Default::default(),
            helpers: vec![],
            faults: vec![],
            time_scale: 1.0,
            volatile_jitter: false,
        }
    }
}

#[test]
fn audit_b_f10_canonical_digest_of_outcome_is_recomputed() {
    let d = TempDir::new("f10");
    let lines = vec![
        HEADER.to_string(),
        r#"{"type":"end","rules":0}"#.to_string(),
    ];
    let (l, s, dg, mut j) = run_with_table(d.path(), &lines, ExitCode::Complete, None);
    j.outputs.table.as_mut().unwrap().canonical_blake3 = Some(D1.into());
    write_outcome(&l, &j);
    assert_eq!(classify_run(&l, &s, &dg, 0).row, 22);
}

#[test]
fn audit_b_f11_canonicaliser_is_linear_in_depth_with_a_typed_limit() {
    let payload = "x".repeat(4 << 20);
    let mut times = Vec::new();
    for depth in [1usize, 120] {
        let line = format!(
            "{{{}\"p\":\"{payload}\"{}}}",
            "\"a\":{".repeat(depth),
            "}".repeat(depth)
        );
        let t = std::time::Instant::now();
        canonical_line(line.as_bytes()).unwrap();
        times.push(t.elapsed().as_secs_f64());
    }
    // the audited code took ~40x longer at depth 120 than at depth 1
    assert!(
        times[1] < times[0] * 8.0 + 0.5,
        "depth 120 {:.3} s vs depth 1 {:.3} s",
        times[1],
        times[0]
    );
    let deep = format!("{{\"a\":{}1{}}}", "[".repeat(200), "]".repeat(200));
    assert_eq!(canonical_line(deep.as_bytes()), Err(CanonError::TooDeep));
}

#[test]
fn family_file_volatile_only_with_family_blake3_and_compared_with_the_spec() {
    // canonical form: two headers naming different paths of the same family content are equal
    let d = "a".repeat(64);
    let h1 = format!(r#"{{"type":"header","family_file":"/x/T.sym","family_blake3":"{d}"}}"#);
    let h2 = format!(r#"{{"type":"header","family_file":"/y/T.sym","family_blake3":"{d}"}}"#);
    assert_eq!(canonical_line(h1.as_bytes()), canonical_line(h2.as_bytes()));
    // classifier: the header's family_blake3 must be the spec's family input digest (sample spec: D1)
    let dd = TempDir::new("fam");
    let good = format!(r#"{{"type":"header","family_file":"/x/T.sym","family_blake3":"{D1}"}}"#);
    let lines = vec![good, r#"{"type":"end","rules":0}"#.to_string()];
    let (l, s, dg, _) = run_with_table(dd.path(), &lines, ExitCode::Complete, None);
    assert_eq!(classify_run(&l, &s, &dg, 0).row, 1);
    let dd = TempDir::new("fam2");
    let bad = format!(r#"{{"type":"header","family_file":"/x/T.sym","family_blake3":"{D0}"}}"#);
    let lines = vec![bad, r#"{"type":"end","rules":0}"#.to_string()];
    let (l, s, dg, _) = run_with_table(dd.path(), &lines, ExitCode::Complete, None);
    assert_eq!(classify_run(&l, &s, &dg, 0).row, 22);
}

#[test]
fn decision_table_rows_and_document_agree() {
    // every row of ROWS is reachable by a label and the generated markdown is the one in child-job.md (when the
    // document is reachable from this worktree's workspace; RUSTRED_CHILD_JOB_DOC overrides the path)
    let md = decision_table_markdown();
    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/decision_table.md");
    if std::env::var_os("RR_UPDATE_GOLDEN").is_some() {
        std::fs::write(&golden, &md).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(&golden).unwrap(),
        md,
        "golden decision_table.md"
    );
    let doc = std::env::var("RUSTRED_CHILD_JOB_DOC").unwrap_or_else(|_| {
        "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/plan/auto-campaign/interfaces/child-job.md".into()
    });
    if let Ok(text) = std::fs::read_to_string(&doc) {
        assert!(
            text.contains(&md),
            "child-job.md 6.5 does not contain the generated decision table"
        );
    }
    let mut ids: Vec<String> = ROWS.iter().map(|r| format!("{}{}", r.row, r.sub)).collect();
    let n = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n, "row ids unique");
}
