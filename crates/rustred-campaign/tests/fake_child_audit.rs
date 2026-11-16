//! Process-level regression tests of audits A and B (lane contractfix), each citing its finding id. Run them in a
//! private pid namespace: `LIGHT_PIDNS=1 light_build.sh <lane> test -p rustred-campaign --test fake_child_audit`.

mod fake_child_common;
use fake_child_common::*;
use rustred_campaign_contract::ExitCode;
use rustred_campaign_contract::classify::{Accept, KillReason, WaitStatus, resume_ref_for};
use rustred_campaign_contract::fake::*;
use rustred_campaign_contract::resume::ResumeState;
use rustred_campaign_contract::spec::InputRef;
use rustred_campaign_contract::stop::CaseResult;
use std::time::Duration;

const T: Duration = Duration::from_secs(20);

fn one_rule_case(i: u64, ms: u64) -> FakeOp {
    FakeOp::Case {
        ordinal: i,
        ms,
        work: Default::default(),
        result: CaseResult::Rule,
        rule: Some(FakeRule {
            record: serde_json::json!({"chart":"C","target":format!("I({i})"),"rhs":[[format!("I({i}-1)"),"1"]]}),
            loci: None,
        }),
        children: vec![],
    }
}

/// The auditor's script: 4 one-rule cases, then a long covered case.
fn dup_script(long_ms: u64) -> FakeScript {
    let mut ops: Vec<FakeOp> = (0..4).map(|i| one_rule_case(i, 0)).collect();
    ops.push(FakeOp::Case {
        ordinal: 4,
        ms: long_ms,
        work: Default::default(),
        result: CaseResult::Covered,
        rule: None,
        children: vec![],
    });
    ops.push(end(None));
    script(ops)
}

/// Rule targets of a run's table, in order.
fn targets(r: &Running) -> Vec<String> {
    std::fs::read_to_string(r.layout.table())
        .unwrap()
        .lines()
        .filter_map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).ok()?;
            (v["type"] == "rule").then(|| v["target"].as_str().unwrap().to_string())
        })
        .collect()
}

/// Attempt 1 of the auditor's scenario: RESUME lags (resume_every_s 30), a slow SIGTERM reaction, two SIGTERMs ->
/// hard-safe stop with OUTCOME's commit point AHEAD of RESUME.
fn stopped_attempt(root: &mut Root, job: &str) -> Running {
    let dir = root.run_dir(job, 1);
    let mut sc = dup_script(60_000);
    sc.on_sigterm = OnSigterm::Slow;
    sc.slow_ms = 600_000;
    let mut s = spec(&dir, job, 1, sc);
    s.resume_every_s = 30;
    s.stop_grace_s = 60;
    let mut r = spawn(root, &s);
    r.wait_event(T, |e| {
        matches!(
            e,
            rustred_campaign_contract::events::Event::CaseStarted { ordinal: 4, .. }
        )
    });
    r.signal_pid(libc::SIGTERM);
    std::thread::sleep(Duration::from_millis(200));
    r.signal_pid(libc::SIGTERM);
    let w = r.wait(T);
    assert_eq!(w, WaitStatus::Exited(30));
    let o = r.outcome();
    let t = o.job.outputs.table.unwrap();
    let res: ResumeState =
        serde_json::from_slice(&std::fs::read(r.layout.resume()).unwrap()).unwrap();
    assert!(
        t.committed_bytes > res.table.bytes,
        "OUTCOME is ahead of RESUME"
    );
    assert_eq!(t.rules_committed, 4);
    assert_eq!(res.rules_committed, 1);
    r
}

#[test]
fn audit_a_c1_resume_from_the_last_resume_record_reproduces_the_uninterrupted_table() {
    let mut root = Root::new("c1ok");
    let r1 = stopped_attempt(&mut root, "dup");
    // the classifier's advice is the RESUME record
    let c = r1.classify(WaitStatus::Exited(30), KillReason::Stop);
    assert_eq!(c.row, 11);
    let advice = c.resume_from.clone().expect("resume advice");
    let mut s2 = resume_spec(&root, &r1, 2);
    assert_eq!(s2.resume.as_ref(), Some(&advice));
    s2.fake = Some(dup_script(0));
    let (r2, w) = run_to_end(&mut root, &s2, T);
    assert_eq!(w, WaitStatus::Exited(0), "{}", r2.stderr());
    assert_eq!(r2.classify(w, KillReason::None).row, 1);
    assert_eq!(targets(&r2), ["I(0)", "I(1)", "I(2)", "I(3)"]);
    // uninterrupted reference
    let d = root.run_dir("ref", 1);
    let (rr, w) = run_to_end(&mut root, &spec(&d, "ref", 1, dup_script(0)), T);
    assert_eq!(w, WaitStatus::Exited(0));
    assert_eq!(canonical_of(&r2), canonical_of(&rr));
    root.assert_no_process_left();
}

#[test]
fn audit_a_c1_a_resume_record_using_outcome_sizes_is_refused_before_any_table_write() {
    let mut root = Root::new("c1bad");
    let r1 = stopped_attempt(&mut root, "dup");
    let o = r1.outcome().job.outputs.table.unwrap();
    // RESUME rewritten with OUTCOME's (newer) byte count and prefix digest, counts left at the old commit point
    let mut res: ResumeState =
        serde_json::from_slice(&std::fs::read(r1.layout.resume()).unwrap()).unwrap();
    res.table.bytes = o.committed_bytes;
    res.table.blake3 = o.prefix_blake3.clone().unwrap();
    rustred_campaign_contract::atomic::write_json_atomic(&r1.layout.resume(), &res).unwrap();
    let mut s2 = resume_spec(&root, &r1, 2);
    s2.fake = Some(dup_script(0));
    let (r2, w) = run_to_end(&mut root, &s2, T);
    assert_eq!(w, WaitStatus::Exited(41), "{}", r2.stderr());
    assert!(!r2.layout.table().exists(), "nothing written to the tables");
    assert_eq!(r2.classify(w, KillReason::None).row, 13);
    // a fully consistent RESUME of the newer prefix but with the OLD queue state (cases done = 1): the resumed run
    // repeats cases 1..3 -> duplicate rule targets -> refused by the verifier (the auditor's 0,1,2,3,1,2,3 table)
    let prefix = &std::fs::read(r1.layout.table()).unwrap()[..o.committed_bytes as usize];
    let (canon, records) =
        rustred_campaign_contract::canon::canonical_digest_reader(prefix).unwrap();
    res.table.records = records;
    res.table.rules = 4;
    res.table.canonical_blake3 = canon;
    res.rules_committed = 4;
    res.counters_at_commit.rules_committed = 4;
    res.check().unwrap();
    rustred_campaign_contract::atomic::write_json_atomic(&r1.layout.resume(), &res).unwrap();
    let mut s3 = resume_spec(&root, &r1, 3);
    s3.fake = Some(dup_script(0));
    let b = std::fs::read(r1.layout.resume()).unwrap();
    assert_eq!(s3.resume, Some(resume_ref_for(&r1.layout, &b)));
    let (r3, w) = run_to_end(&mut root, &s3, T);
    let c = r3.classify(w, KillReason::None);
    assert_eq!((c.row, c.accept), (22, Accept::Nothing), "{c:?}");
    root.assert_no_process_left();
}

/// xorshift for the seeded stop points.
struct Rng(u64);
impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }
}

#[test]
fn audit_a_c1_seeded_stop_points_resume_to_the_uninterrupted_canonical_table() {
    let mut root = Root::new("c1seed");
    let mk = |dir: &std::path::Path, job: &str, a: u32| {
        let mut sc = script(
            (0..6)
                .map(|i| rule_case(i, 120))
                .chain([end(None)])
                .collect(),
        );
        sc.volatile_jitter = true;
        let mut s = spec(dir, job, a, sc);
        s.resume_every_s = 30; // RESUME lags the commit points (default cadence)
        s
    };
    let d = root.run_dir("ref", 1);
    let (rr, w) = run_to_end(&mut root, &mk(&d, "ref", 1), T);
    assert_eq!(w, WaitStatus::Exited(0));
    let want = canonical_of(&rr);
    let mut g = Rng(0xC1_5EED_0001);
    for seed in 0..8 {
        let job = format!("s{seed}");
        let mut attempt = 1;
        let mut s = mk(&root.run_dir(&job, 1), &job, 1);
        loop {
            let mut r = spawn(&mut root, &s);
            let kill = attempt <= 2;
            let w = if kill {
                std::thread::sleep(Duration::from_millis(g.below(900)));
                if g.below(2) == 0 {
                    r.signal_group(libc::SIGKILL);
                } else {
                    r.signal_pid(libc::SIGTERM);
                }
                r.wait(T)
            } else {
                r.wait(T)
            };
            let k = match w {
                WaitStatus::Signaled(_) => KillReason::Stop,
                _ => KillReason::None,
            };
            let c = r.classify(w, k);
            if c.accept != Accept::Nothing {
                assert_eq!(c.row, 1, "seed {seed}: {c:?}");
                assert_eq!(canonical_of(&r), want, "seed {seed} attempt {attempt}");
                break;
            }
            assert!(attempt < 6, "seed {seed}: no completion");
            attempt += 1;
            let dir = root.run_dir(&job, attempt);
            s = mk(&dir, &job, attempt);
            if let Some(rf) = c.resume_from {
                s.resume = Some(rf);
            }
        }
    }
    root.assert_no_process_left();
}

#[test]
fn audit_a_c4_no_write_into_a_finished_or_foreign_run_dir() {
    let mut root = Root::new("c4");
    let dir = root.run_dir("f", 1);
    let s = spec(&dir, "f", 1, script(vec![rule_case(0, 0), end(None)]));
    let (r, w) = run_to_end(&mut root, &s, T);
    assert_eq!(w, WaitStatus::Exited(0));
    let before = std::fs::read(r.layout.outcome()).unwrap();
    // a wrong program spawned into the finished run directory: refused, OUTCOME untouched
    let mut s2 = s.clone();
    s2.program.name = "rr_node".into();
    std::fs::remove_file(r.layout.job_spec()).ok();
    let mut r2 = spawn(&mut root, &s2);
    assert_eq!(r2.wait(T), WaitStatus::Exited(42));
    assert_eq!(std::fs::read(r.layout.outcome()).unwrap(), before);
    // an invalid spec whose run_dir is another directory: nothing written there
    let foreign = root.run_dir("foreign", 1);
    let mine = root.run_dir("mine", 1);
    let mut s3 = spec(&foreign, "mine", 1, script(vec![end(None)]));
    s3.heartbeat_s = 0;
    s3.run_dir = foreign.to_str().unwrap().into();
    std::fs::write(mine.join("job.json"), s3.to_json_line()).unwrap();
    let st = std::process::Command::new(BIN)
        .arg("job")
        .arg(mine.join("job.json"))
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert_eq!(st.code(), Some(42));
    assert_eq!(std::fs::read_dir(&foreign).unwrap().count(), 0);
    assert!(!mine.join("OUTCOME.json").exists());
    root.assert_no_process_left();
}

#[test]
fn audit_b_f6_enospc_fault_never_writes_a_regular_file() {
    let mut root = Root::new("f6");
    let victim = root.path.join("victim.txt");
    std::fs::write(&victim, b"precious").unwrap();
    let dir = root.run_dir("f", 1);
    let mut sc = script(vec![rule_case(0, 0), end(None)]);
    sc.faults = vec![Fault {
        at_op: 1,
        kind: FaultKind::Enospc,
        bytes: 0,
        code: 0,
        path: Some(victim.to_str().unwrap().into()),
    }];
    let (r, w) = run_to_end(&mut root, &spec(&dir, "f", 1, sc), T);
    assert_eq!(w, WaitStatus::Exited(40), "{}", r.stderr());
    assert_eq!(std::fs::read(&victim).unwrap(), b"precious");
    root.assert_no_process_left();
}

#[test]
fn audit_b_f7_planted_symlinks_redirect_nothing() {
    let mut root = Root::new("f7");
    let outside = root.path.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("file"), b"keep").unwrap();
    for name in ["out", ".reserve", "EVENTS.jsonl"] {
        let job = format!("j{}", name.len());
        let dir = root.run_dir(&job, 1);
        let target = if name == "out" {
            outside.clone()
        } else {
            outside.join("file")
        };
        std::os::unix::fs::symlink(&target, dir.join(name)).unwrap();
        let (r, w) = run_to_end(&mut root, &spec(&dir, &job, 1, script(vec![end(None)])), T);
        assert_eq!(w, WaitStatus::Exited(42), "{name}: {}", r.stderr());
        assert!(!r.layout.outcome().exists());
    }
    assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 1);
    assert_eq!(std::fs::read(outside.join("file")).unwrap(), b"keep");
    root.assert_no_process_left();
}

#[test]
fn audit_a_m6_differential_family_digest_and_volatile_family_path() {
    // the fake writes family_file + family_blake3 with its OWN canonical form; the crate's verifier recomputes it;
    // two runs naming the same family content by different paths have the same canonical table
    let mut root = Root::new("fam");
    let mut digests = Vec::new();
    for (k, name) in ["a.sym", "b.sym"].iter().enumerate() {
        let fam = root.path.join(name);
        std::fs::write(&fam, b"family content").unwrap();
        let job = format!("fam{k}");
        let dir = root.run_dir(&job, 1);
        let mut s = spec(&dir, &job, 1, script(vec![rule_case(0, 0), end(None)]));
        s.inputs = vec![InputRef {
            role: "family".into(),
            path: fam.to_str().unwrap().into(),
            blake3: rustred_campaign_contract::blake3_hex(b"family content"),
            upto: None,
        }];
        let (r, w) = run_to_end(&mut root, &s, T);
        assert_eq!(w, WaitStatus::Exited(0), "{}", r.stderr());
        assert_eq!(r.classify(w, KillReason::None).row, 1);
        let ev = r.evidence(w, KillReason::None);
        assert_eq!(
            ev.verified.unwrap().streams[0].family_blake3.as_deref(),
            Some(rustred_campaign_contract::blake3_hex(b"family content").as_str())
        );
        digests.push(canonical_of(&r));
    }
    assert_eq!(digests[0], digests[1]);
    assert_eq!(ExitCode::Complete.code(), 0);
    root.assert_no_process_left();
}
