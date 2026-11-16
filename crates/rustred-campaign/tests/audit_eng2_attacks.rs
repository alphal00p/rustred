//! Lane audit-eng2: attacks on the engine side of the child-job contract with the REAL child (`rr_node job`):
//! lying / stale RESUME files, a full disk (tmpfs inside the test's private namespaces), helper processes around
//! signals, and the solve-recipe dispatcher. Each test prints what it found; a test FAILS when the engine accepts
//! something the audit treats as a defect (the assertion message names the finding).
//!
//! Needs (all `#[ignore]`): `RR_NODE_BIN`, `RUSTRED_CAMPAIGN_FIXTURES`, the licence environment, `LIGHT_PIDNS=1`
//! (private user + pid + mount namespace: signals reach only our children; tmpfs mounts stay private), `RR_REAL_OUT`.

mod audit_eng2_common;
use audit_eng2_common::*;

use rustred_campaign_contract::classify::{
    Accept, Evidence, KillReason, OutcomeState, WaitStatus, classify,
};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::resume::ResumeRef;
use rustred_campaign_contract::spec::{JobKind, JobSpec};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn l4a() -> Fx {
    Fx {
        tag: "l4a",
        node: "L4a",
        args: vec!["--order".into(), "id".into()],
        cases: vec![],
        resume_every: vec![0],
        stop_grace_s: 5,
        numeric_units: false,
    }
}

fn run_to_end(spec: &JobSpec) -> WaitStatus {
    let mut c = spawn(spec);
    wait(&mut c)
}

/// Spawn, wait for `k` case starts (numeric or all), SIGTERM, wait: the graceful stop of an attempt.
fn stop_after(spec: &JobSpec, k: usize, numeric: bool) -> WaitStatus {
    let mut c = spawn(spec);
    let t = Instant::now();
    while units(&events_of(spec), numeric) < k && t.elapsed() < Duration::from_secs(300) {
        if c.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    signal(&c, libc::SIGTERM);
    wait(&mut c)
}

fn table_canon(spec: &JobSpec) -> Option<String> {
    canonical_of(spec, "F.table.jsonl")
}

fn rules_in(spec: &JobSpec) -> usize {
    std::fs::read_to_string(RunLayout::new(&spec.run_dir).table())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["type"] == "rule")
        .count()
}

fn read_json(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

/// A spec of job `prev.job_id`, attempt `attempt`, in run directory `<root>/<dir>`, resuming from `from`.
fn attempt_in(
    root: &Path,
    prev: &JobSpec,
    attempt: u32,
    dir: &str,
    from: Option<ResumeRef>,
) -> JobSpec {
    let run = root.join(dir);
    std::fs::create_dir_all(&run).unwrap();
    let mut s = prev.clone();
    s.attempt = attempt;
    s.run_dir = run.to_str().unwrap().into();
    s.resume = from;
    s
}

fn rref(p: &Path) -> ResumeRef {
    ResumeRef {
        from: p.to_str().unwrap().into(),
        blake3: b3(p),
    }
}

/// Write `v` as a RESUME.json into a fresh directory that also holds a copy of the previous attempt's job.json and
/// out/ (so the RESUME's run directory checks pass only if the engine does not bind them): returns the RESUME path.
fn rewrite_resume_in_place(prev: &JobSpec, v: &Value) -> PathBuf {
    let p = RunLayout::new(&prev.run_dir).resume();
    std::fs::write(&p, serde_json::to_vec(v).unwrap()).unwrap();
    p
}

/// The committed-prefix figures of `bytes` (a table or queue prefix) as RESUME's `CommittedFile` JSON.
fn committed_json(path_rel: &str, bytes: &[u8], scratch: &Path) -> Value {
    std::fs::write(scratch, bytes).unwrap();
    let (canon, _) = rustred_campaign_contract::canon::canonical_digest_file(scratch).unwrap();
    let lines: Vec<Value> = bytes
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_slice(l).unwrap())
        .collect();
    json!({"path": path_rel, "bytes": bytes.len(), "blake3": rustred_campaign_contract::blake3_hex(bytes),
           "records": lines.len(), "rules": lines.iter().filter(|v| v["type"] == "rule").count(),
           "canonical_blake3": canon})
}

/// Finding log line (stderr + RR_AUDIT_LOG).
fn note(v: Value) {
    eprintln!("FINDING-PROBE {v}");
    if let Ok(p) = std::env::var("RR_AUDIT_LOG") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
        {
            let _ = writeln!(f, "{v}");
        }
    }
}

/// Item 2: A lying or stale RESUME. Every refusal the contract promises is probed; then RESUME files that are
/// SELF-CONSISTENT (re-digested by whoever wrote them) but lie about the queue or the committed rules: the engine
/// is expected to refuse or to redo the work; accepting a sealed table that skips work or carries a not-lower rule
/// is reported.
#[test]
#[ignore]
fn audit_eng2_lying_resume_l4a() {
    let r = root("lying");
    let fx = l4a();
    // the uninterrupted reference
    let full = job_spec(&r, "ref", 1, &fx, 0);
    assert_eq!(run_to_end(&full), WaitStatus::Exited(0));
    let want = table_canon(&full).unwrap();
    let want_rules = rules_in(&full);
    // attempt 1 of job A, stopped gracefully after 30 cases
    let a1 = job_spec(&r, "A", 1, &fx, 0);
    let w = stop_after(&a1, 30, false);
    assert_eq!(w, WaitStatus::Exited(30), "attempt 1 interrupted");
    let cl = classified(&a1, w, KillReason::Stop);
    let good = cl.resume_from.clone().expect("valid RESUME");
    let res_path = RunLayout::new(&a1.run_dir).resume();
    let original = std::fs::read(&res_path).unwrap();
    let res: Value = serde_json::from_slice(&original).unwrap();
    let mut results: Vec<(String, Value)> = Vec::new();
    let mut n = 2u32;
    let mut probe = |name: &str,
                     spec: JobSpec,
                     expect_refusal: bool|
     -> (WaitStatus, Option<String>) {
        let w = run_to_end(&spec);
        let cl = classified(&spec, w, KillReason::None);
        let canon = table_canon(&spec);
        let rules = rules_in(&spec);
        let v = json!({"probe": name, "wait": format!("{w:?}"), "row": cl.row, "accept": format!("{:?}", cl.accept),
                       "rules": rules, "want_rules": want_rules, "canonical_equal": canon.as_deref() == Some(want.as_str()),
                       "expect_refusal": expect_refusal});
        note(v.clone());
        results.push((name.to_string(), v));
        (w, canon)
    };
    // (a) wrong digest in the resume reference
    let mut bad = good.clone();
    bad.blake3 = "0".repeat(64);
    let s = attempt_in(&r, &a1, 2, &format!("A.a2-{n}"), Some(bad));
    n += 1;
    let (w, _) = probe("wrong_digest", s, true);
    assert_eq!(w, WaitStatus::Exited(41), "wrong RESUME digest must be 41");
    // (b) another job resuming from A's RESUME (digest right)
    let mut other = a1.clone();
    other.job_id = "B".into();
    let s = attempt_in(&r, &other, 2, "B.a2", Some(good.clone()));
    let (w, _) = probe("other_job", s, true);
    assert_eq!(
        w,
        WaitStatus::Exited(41),
        "RESUME of another job must be 41"
    );
    // (c) another spec (different args) resuming from A's RESUME
    let mut other = a1.clone();
    other.args = vec![
        "--order".into(),
        "id".into(),
        "--max-depth".into(),
        "3".into(),
    ];
    let s = attempt_in(&r, &other, 2, &format!("A.a2-{n}"), Some(good.clone()));
    n += 1;
    let (w, _) = probe("other_spec", s, true);
    assert_eq!(
        w,
        WaitStatus::Exited(41),
        "RESUME of another spec must be 41"
    );
    // (d) edited without re-digest (pending emptied, old digest)
    let mut ed = res.clone();
    ed["pending"] = json!([]);
    ed["current"] = Value::Null;
    std::fs::write(&res_path, serde_json::to_vec(&ed).unwrap()).unwrap();
    let s = attempt_in(&r, &a1, 2, &format!("A.a2-{n}"), Some(good.clone()));
    n += 1;
    let (w, _) = probe("edited_old_digest", s, true);
    assert_eq!(w, WaitStatus::Exited(41));
    // (e) truncated, re-digested
    std::fs::write(&res_path, &original[..original.len() / 2]).unwrap();
    let s = attempt_in(&r, &a1, 2, &format!("A.a2-{n}"), Some(rref(&res_path)));
    n += 1;
    let (w, _) = probe("truncated_redigested", s, true);
    assert_eq!(w, WaitStatus::Exited(41));
    // (f) spec_blake3 inside RESUME changed, re-digested
    let mut ed = res.clone();
    ed["spec_blake3"] = json!("1".repeat(64));
    let p = rewrite_resume_in_place(&a1, &ed);
    let s = attempt_in(&r, &a1, 2, &format!("A.a2-{n}"), Some(rref(&p)));
    let (w, _) = probe("spec_digest_redigested", s, true);
    assert_eq!(w, WaitStatus::Exited(41));
    // restore the original RESUME (its digest = `good`)
    std::fs::write(&res_path, &original).unwrap();
    assert_eq!(b3(&res_path), good.blake3);
    // (g) the normal resume (attempt 2) and a REPLAY of the same RESUME by attempt 3
    let a2 = attempt_in(&r, &a1, 2, "A.a2", Some(good.clone()));
    let (w, c2) = probe("normal_resume", a2.clone(), false);
    assert_eq!(w, WaitStatus::Exited(0));
    assert_eq!(c2.as_deref(), Some(want.as_str()));
    let a3 = attempt_in(&r, &a1, 3, "A.a3-replay", Some(good.clone()));
    let (w, c3) = probe("replay_same_resume_twice", a3, false);
    note(
        json!({"probe": "replay_same_resume_twice", "note": "accepted by the engine (attempt 3 from attempt 1's RESUME after attempt 2 used it)", "wait": format!("{w:?}"), "equal": c3.as_deref() == Some(want.as_str())}),
    );
    // (h) a LATER attempt's RESUME used by an earlier attempt number: attempt 2 (another dir) from attempt 2's own
    // RESUME (A.a2 is complete; its final RESUME exists)
    let a2_res = RunLayout::new(&a2.run_dir).resume();
    if a2_res.exists() {
        let s = attempt_in(&r, &a2, 2, "A.a2-again", Some(rref(&a2_res)));
        let (w, _) = probe("not_earlier_attempt", s, true);
        assert_eq!(
            w,
            WaitStatus::Exited(41),
            "RESUME of a non-earlier attempt must be 41"
        );
    }
    // ---- self-consistent lies (re-digested): the contract's binding is the digest the supervisor reads; the engine
    // is the last line of defence
    // (i) the open queue dropped: pending [] and current null
    let mut lie = res.clone();
    lie["pending"] = json!([]);
    lie["current"] = Value::Null;
    let p = rewrite_resume_in_place(&a1, &lie);
    let s = attempt_in(&r, &a1, 4, "A.a4-dropped-queue", Some(rref(&p)));
    let (w, c) = probe("lie_pending_dropped", s.clone(), false);
    let cl = classified(&s, w, KillReason::None);
    let accepted_short = cl.accept != Accept::Nothing && c.as_deref() != Some(want.as_str());
    note(
        json!({"probe": "lie_pending_dropped", "accepted_table_missing_rules": accepted_short, "row": cl.row,
                "rules": rules_in(&s), "want_rules": want_rules}),
    );
    // (j) a committed rule made NOT LOWER (its own target added to its RHS) in the previous attempt's table prefix,
    // every figure of RESUME recomputed, engine.not_lower absent
    std::fs::write(&res_path, &original).unwrap();
    let t_path = RunLayout::new(&a1.run_dir).table();
    let table = std::fs::read(&t_path).unwrap();
    let pre_len = res["table"]["bytes"].as_u64().unwrap() as usize;
    let mut out = Vec::new();
    let mut done = false;
    for line in table[..pre_len].split_inclusive(|&b| b == b'\n') {
        let mut v: Value = serde_json::from_slice(line).unwrap();
        if !done && v["type"] == "rule" && v["index"] == 3 {
            let t = v["target"].clone();
            v["rhs"].as_array_mut().unwrap().push(json!([t, "1"]));
            done = true;
            out.extend_from_slice(serde_json::to_string(&v).unwrap().as_bytes());
            out.push(b'\n');
        } else {
            out.extend_from_slice(line);
        }
    }
    assert!(done, "rule 3 in the committed prefix");
    let mut planted = a1.clone();
    let pdir = r.join("A.a1-planted");
    // a copy of attempt 1's run directory with the planted table (the original is kept for the other probes)
    copy_dir(Path::new(&a1.run_dir), &pdir);
    planted.run_dir = pdir.to_str().unwrap().into();
    // the copied job.json names the old run_dir: the planted run must carry its own job.json (spec digest changes)
    let jp = RunLayout::new(&planted.run_dir).job_spec();
    let mut perm = std::fs::metadata(&jp).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perm.set_readonly(false);
    std::fs::set_permissions(&jp, perm).unwrap();
    std::fs::write(&jp, planted.to_json_line()).unwrap();
    let mut whole = out.clone();
    whole.extend_from_slice(&table[pre_len..]);
    std::fs::write(RunLayout::new(&planted.run_dir).table(), &whole).unwrap();
    let mut lie = res.clone();
    lie["table"] = committed_json("out/F.table.jsonl", &out, &r.join("scratch.prefix"));
    lie["spec_blake3"] = json!(rustred_campaign_contract::spec::spec_blake3(
        &std::fs::read(&jp).unwrap()
    ));
    let p = rewrite_resume_in_place(&planted, &lie);
    let s = attempt_in(&r, &planted, 5, "A.a5-not-lower", Some(rref(&p)));
    let (w, _) = probe("lie_not_lower_rule_in_prefix", s.clone(), false);
    let cl = classified(&s, w, KillReason::None);
    let o = outcome(&s);
    note(
        json!({"probe": "lie_not_lower_rule_in_prefix", "row": cl.row, "accept": format!("{:?}", cl.accept),
                "stop": o.as_ref().map(|o| format!("{:?}", o.job.stop.code)),
                "native_descent_not_lower": std::fs::read_to_string(RunLayout::new(&s.run_dir).table()).unwrap_or_default().lines().last().map(|l| serde_json::from_str::<Value>(l).map(|v| v["native_descent_not_lower"].clone()).unwrap_or(Value::Null)) }),
    );
    let summary: Vec<Value> = results.iter().map(|(_, v)| v.clone()).collect();
    eprintln!("SUMMARY {}", Value::from(summary));
    assert!(
        !accepted_short,
        "FINDING: a self-consistent RESUME with the open queue dropped yields an ACCEPTED sealed table missing rules"
    );
    assert_eq!(
        cl.accept,
        Accept::Nothing,
        "FINDING: a not-lower rule carried in a re-digested RESUME's committed prefix reaches an accepting row ({})",
        cl.row
    );
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let t = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &t);
        } else {
            std::fs::copy(e.path(), &t).unwrap();
        }
    }
}

/// C1 family: a self-consistent RESUME of the numeric stage that forgets the exhausted master
/// (`engine.numeric_stage.exhausted` emptied): does the resumed attempt notice, or does it seal a complete table?
#[test]
#[ignore]
fn audit_eng2_lying_resume_numeric_stage() {
    let r = root("lying-num");
    let cases: Vec<Value> = [
        [1, 1, 1, 1, 1, 1, 1, 1, 0],
        [2, 1, 1, 1, 1, 1, 1, 1, 0],
        [1, 2, 1, 1, 1, 1, 1, 1, 0],
        [1, 1, 1, 2, 1, 1, 1, 1, 0],
        [1, 1, 1, 1, 1, 1, 1, 1, -1],
        [1, 1, 1, 1, 1, 2, 1, 1, 0],
        [1, 1, 1, 1, 1, 1, 1, 2, 0],
        [2, 1, 1, 1, 1, 1, 1, 1, -1],
    ]
    .iter()
    .map(|p| json!({"frame": "ORIGINAL", "fixed": p, "equations": []}))
    .collect();
    let fx = Fx {
        tag: "l3anum",
        node: "L3a",
        args: [
            "--order",
            "id",
            "--numeric-depth",
            "0",
            "--numeric-depth-max",
            "3",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        cases,
        resume_every: vec![0],
        stop_grace_s: 5,
        numeric_units: true,
    };
    let full = job_spec(&r, "ref", 1, &fx, 0);
    assert_eq!(run_to_end(&full), WaitStatus::Exited(0));
    let want = table_canon(&full).unwrap();
    // stop at the 8th numeric endpoint (after the exhausted master, 7th) until RESUME has exhausted >= 1
    let mut got = None;
    for k in 0..12 {
        let a = job_spec(&r, &format!("N{k}"), 1, &fx, 0);
        let w = stop_after(&a, 8, true);
        if w != WaitStatus::Exited(30) {
            continue;
        }
        let res = read_json(&RunLayout::new(&a.run_dir).resume());
        if res["engine"]["numeric_stage"]["exhausted"]
            .as_array()
            .is_some_and(|x| !x.is_empty())
        {
            got = Some((a, res));
            break;
        }
    }
    let Some((a, res)) = got else {
        note(
            json!({"probe": "lie_numeric_exhausted", "skipped": "no stop landed after the exhausted master"}),
        );
        return;
    };
    let mut lie = res.clone();
    lie["engine"]["numeric_stage"]["exhausted"] = json!([]);
    let p = rewrite_resume_in_place(&a, &lie);
    let s = attempt_in(&r, &a, 2, "N.a2-lie", Some(rref(&p)));
    let w = run_to_end(&s);
    let cl = classified(&s, w, KillReason::None);
    let canon = table_canon(&s);
    let v = json!({"probe": "lie_numeric_exhausted_dropped", "wait": format!("{w:?}"), "row": cl.row,
                   "accept": format!("{:?}", cl.accept), "canonical_equal": canon.as_deref() == Some(want.as_str()),
                   "residuals": outcome(&s).map(|o| o.legacy.get("residuals").cloned())});
    note(v);
    assert!(
        !(cl.accept != Accept::Nothing && canon.as_deref() != Some(want.as_str())),
        "FINDING: a self-consistent numeric-stage RESUME without its exhausted endpoint yields an ACCEPTED table that differs from the uninterrupted one (row {})",
        cl.row
    );
}

// ---------------------------------------------------------------------------------------------------------------
// full disk (M3, M6): a tmpfs mounted inside the test's private mount namespace is the campaign directory; a filler
// takes every free byte at a random point of the run; then the space is freed and the job resumed

fn mount_tmpfs(dir: &Path, bytes: u64) -> bool {
    std::fs::create_dir_all(dir).unwrap();
    let src = std::ffi::CString::new("tmpfs").unwrap();
    let d = std::ffi::CString::new(dir.to_str().unwrap()).unwrap();
    let opts = std::ffi::CString::new(format!("size={bytes},mode=0700")).unwrap();
    // SAFETY: plain mount(2) of a tmpfs inside the test's private mount namespace (LIGHT_PIDNS=1: unshare -Urpf
    // --mount-proc); it disappears with the namespace.
    let rc = unsafe {
        libc::mount(
            src.as_ptr(),
            d.as_ptr(),
            src.as_ptr(),
            0,
            opts.as_ptr() as *const libc::c_void,
        )
    };
    if rc != 0 {
        eprintln!("mount tmpfs: {}", std::io::Error::last_os_error());
    }
    rc == 0
}

/// Fill the file system of `dir` completely (a file of zeros written until ENOSPC).
fn fill(dir: &Path) -> PathBuf {
    use std::io::Write;
    let p = dir.join("FILLER");
    let mut f = std::fs::File::create(&p).unwrap();
    let block = vec![0u8; 1 << 16];
    while f.write_all(&block).is_ok() {}
    let small = [0u8; 512];
    while f.write_all(&small).is_ok() {}
    let one = [0u8; 1];
    while f.write_all(&one).is_ok() {}
    let _ = f.sync_all();
    p
}

/// M3 / M6 / 3.10: ENOSPC at random points of real jobs. Required: exit 50 (or a normal end if nothing more had to be
/// written), never a panic code / 70 / an unexplained code; an OUTCOME, when written, consistent and never claiming a
/// sealed output the verifier refuses; nothing accepted; after freeing the space the job resumes (or restarts) to the
/// canonical table of an uninterrupted run.
#[test]
#[ignore]
fn audit_eng2_full_disk_generate() {
    let r = root("enospc");
    let mnt = r.join("mnt");
    assert!(
        mount_tmpfs(&mnt, 48 << 20),
        "tmpfs mount inside the namespace (run with LIGHT_PIDNS=1)"
    );
    let fxs = [
        l4a(),
        Fx {
            tag: "t281desc",
            node: "T281-b6",
            args: ["--order", "id", "--descend", "open"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            cases: [363u64, 367, 370, 380]
                .iter()
                .map(|&i| {
                    let p = fixtures().join("small/m2-slice/T281-b6/table.jsonl");
                    let text = std::fs::read_to_string(&p).unwrap();
                    let v = text
                        .lines()
                        .map(|l| serde_json::from_str::<Value>(l).unwrap())
                        .find(|v| v["type"] == "rule" && v["index"] == i)
                        .unwrap();
                    json!({"frame": "PRESENTED", "fixed": v["fixed"], "equations": v["equations"]})
                })
                .collect(),
            resume_every: vec![0],
            stop_grace_s: 5,
            numeric_units: false,
        },
    ];
    let mut rng = Rng(env_u64("RR_AUDIT_SEED", 20261114) ^ 0xD15C);
    let mut problems: Vec<String> = Vec::new();
    let mut ran = 0;
    for fx in &fxs {
        let refd = job_spec(&mnt, &format!("{}-ref", fx.tag), 1, fx, 0);
        let t0 = Instant::now();
        let wref = run_to_end(&refd);
        let wall = t0.elapsed();
        let want_t = table_canon(&refd);
        let want_s = canonical_of(&refd, "F.subcases.jsonl");
        let units_ref = units(&events_of(&refd), false);
        eprintln!(
            "[{}] reference {wref:?} in {wall:?}, {units_ref} cases",
            fx.tag
        );
        let drills = env_u64("RR_AUDIT_DISK_DRILLS", if fx.tag == "l4a" { 10 } else { 3 });
        for d in 0..drills {
            let job = format!("{}-disk{d}", fx.tag);
            let a = job_spec(&mnt, &job, 1, fx, 0);
            let mut c = spawn(&a);
            let k = 1 + rng.below(units_ref.max(1) as u64) as usize;
            let t = Instant::now();
            while units(&events_of(&a), false) < k
                && t.elapsed() < wall * 3 + Duration::from_secs(30)
            {
                if c.try_wait().unwrap().is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            let filler = fill(&mnt);
            let w = wait(&mut c);
            ran += 1;
            let cl = classified(&a, w, KillReason::None);
            let o = outcome(&a);
            let v = json!({"probe": "enospc", "fixture": fx.tag, "drill": d, "after_cases": k, "wait": format!("{w:?}"),
                           "row": cl.row, "accept": format!("{:?}", cl.accept), "why": cl.why,
                           "outcome": o.as_ref().map(|o| json!({"exit": o.job.exit_code.code(), "stop": format!("{:?}", o.job.stop.code), "error": o.job.error.as_ref().map(|e| format!("{:?}: {}", e.class, e.message))})),
                           "resume_valid": cl.resume_from.is_some()});
            note(v);
            std::fs::remove_file(&filler).unwrap();
            match w {
                WaitStatus::Exited(50) => {}
                x if x == wref && cl.row != 22 => {}
                _ => problems.push(format!(
                    "[{job}] ENOSPC after {k} cases: {w:?} row {} ({})",
                    cl.row, cl.why
                )),
            }
            if w != wref && cl.accept != Accept::Nothing {
                problems.push(format!("[{job}] ENOSPC attempt accepted: row {}", cl.row));
            }
            if w == WaitStatus::Exited(50) && cl.row != 15 {
                problems.push(format!(
                    "[{job}] exit 50 classified row {} ({})",
                    cl.row, cl.why
                ));
            }
            if w == wref {
                continue;
            }
            // resume after the space came back (RESUME valid: from it; else from scratch)
            let b = next_attempt(&mnt, &a, cl.resume_from.clone());
            let w2 = run_to_end(&b);
            let equal_t = table_canon(&b) == want_t;
            let equal_s = canonical_of(&b, "F.subcases.jsonl") == want_s;
            note(
                json!({"probe": "enospc_resume", "fixture": fx.tag, "drill": d, "resumed": b.resume.is_some(),
                        "wait": format!("{w2:?}"), "table_equal": equal_t, "subcases_equal": equal_s}),
            );
            if w2 != wref || !equal_t || !equal_s {
                problems.push(format!(
                    "[{job}] after ENOSPC + free space: resumed {w2:?} (want {wref:?}), table equal {equal_t}, subcases equal {equal_s}"
                ));
            }
            // keep the tmpfs small: drop finished attempts
            let _ = std::fs::remove_dir_all(&a.run_dir);
            let _ = std::fs::remove_dir_all(&b.run_dir);
        }
    }
    eprintln!("ENOSPC drills: {ran}; problems: {}", problems.len());
    for p in &problems {
        eprintln!("DIFF {p}");
    }
    assert!(problems.is_empty(), "{} ENOSPC problems", problems.len());
}

// ---------------------------------------------------------------------------------------------------------------
// helpers (forks) around signals

/// Children (pids) of `pid` (all its threads).
fn children(pid: u32) -> Vec<i32> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(format!("/proc/{pid}/task")) {
        for t in rd.flatten() {
            if let Ok(s) = std::fs::read_to_string(t.path().join("children")) {
                out.extend(s.split_whitespace().filter_map(|x| x.parse::<i32>().ok()));
            }
        }
    }
    out
}

/// (state, pgid, sid) of a process, None when gone.
fn stat(pid: i32) -> Option<(char, i32, i32)> {
    let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = &s[s.rfind(')')? + 2..];
    let f: Vec<&str> = rest.split_whitespace().collect();
    Some((f[0].chars().next()?, f[2].parse().ok()?, f[3].parse().ok()?))
}

/// Every live process of the namespace except ourselves (pid 1 = the test binary under LIGHT_PIDNS=1).
fn live_others() -> Vec<(i32, char)> {
    let me = std::process::id() as i32;
    std::fs::read_dir("/proc")
        .unwrap()
        .flatten()
        .filter_map(|e| e.file_name().to_str().and_then(|s| s.parse::<i32>().ok()))
        .filter(|&p| p != me)
        .filter_map(|p| stat(p).map(|(s, _, _)| (p, s)))
        .filter(|&(_, s)| s != 'Z')
        .collect()
}

fn reap_zombies() {
    loop {
        // SAFETY: reaping our own (adopted) children in our private pid namespace.
        let r = unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) };
        if r <= 0 {
            break;
        }
    }
}

/// 3.4.6 / m4: a helper forked by the engine (native guard try) stays in the job's process group, ignores a group
/// SIGTERM, and is dead within 1 s after (a) a SIGKILL of the job alone (PDEATHSIG), (b) the hard-safe stop
/// (SIGTERM + SIGINT), (c) a group SIGTERM (graceful stop: the job kills its helper itself).
#[test]
#[ignore]
fn audit_eng2_helpers_die_with_the_job() {
    assert_eq!(
        std::process::id(),
        1,
        "run with LIGHT_PIDNS=1 (pid 1 of a private namespace)"
    );
    let r = root("helpers");
    let fx = Fx {
        tag: "t281h",
        node: "T281-b6",
        args: ["--order", "id"].iter().map(|s| s.to_string()).collect(),
        cases: vec![],
        resume_every: vec![0],
        stop_grace_s: 5,
        numeric_units: false,
    };
    let mut problems = Vec::new();
    let mut caught = 0;
    for (k, how) in [
        "kill_job_only",
        "term_then_int",
        "group_term",
        "kill_job_only",
        "group_term",
        "term_then_int",
    ]
    .iter()
    .enumerate()
    {
        let a = job_spec(&r, &format!("h{k}"), 1, &fx, 0);
        let mut c = spawn(&a);
        let pid = c.id();
        // wait (polling 1 ms) for a helper of the job
        let t = Instant::now();
        let mut helper = None;
        while t.elapsed() < Duration::from_secs(240) {
            if c.try_wait().unwrap().is_some() {
                break;
            }
            if let Some(&h) = children(pid).first() {
                helper = Some(h);
                break;
            }
            std::thread::sleep(Duration::from_micros(500));
        }
        let Some(h) = helper else {
            signal(&c, libc::SIGKILL);
            let _ = wait(&mut c);
            note(json!({"probe": "helper", "how": how, "skipped": "no helper seen"}));
            continue;
        };
        caught += 1;
        let hs = stat(h);
        let js = stat(pid as i32);
        let same_group = matches!((hs, js), (Some((_, hg, _)), Some((_, jg, _))) if hg == jg);
        if !same_group {
            problems.push(format!(
                "[{how}] helper {h} not in the job's group: {hs:?} vs {js:?}"
            ));
        }
        match *how {
            "kill_job_only" => signal(&c, libc::SIGKILL),
            "term_then_int" => {
                signal(&c, libc::SIGTERM);
                std::thread::sleep(Duration::from_millis(30));
                signal(&c, libc::SIGINT);
            }
            _ => {
                // SAFETY: the job's process group (it is a session leader, pgid = its pid) inside our namespace.
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGTERM);
                }
            }
        }
        let w = wait(&mut c);
        std::thread::sleep(Duration::from_millis(1000));
        reap_zombies();
        let alive = live_others();
        note(
            json!({"probe": "helper", "how": how, "helper": h, "same_group": same_group, "job_exit": format!("{w:?}"),
                    "survivors": format!("{alive:?}")}),
        );
        if !alive.is_empty() {
            problems.push(format!(
                "[{how}] survivors 1 s after the job ended: {alive:?}"
            ));
            for (p, _) in alive {
                // SAFETY: a survivor inside our private namespace.
                unsafe {
                    libc::kill(p, libc::SIGKILL);
                }
            }
            std::thread::sleep(Duration::from_millis(200));
            reap_zombies();
        }
        if *how != "kill_job_only" {
            let cl = classified(&a, w, KillReason::Stop);
            if cl.accept != Accept::Nothing {
                problems.push(format!("[{how}] stopped job accepted (row {})", cl.row));
            }
        }
    }
    eprintln!("helpers caught: {caught}; problems: {}", problems.len());
    for p in &problems {
        eprintln!("DIFF {p}");
    }
    assert!(problems.is_empty(), "{} helper problems", problems.len());
}

// ---------------------------------------------------------------------------------------------------------------
// solve-recipe dispatcher

fn recipe_fixtures() -> PathBuf {
    std::env::var("RR_RECIPE_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(
                "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/fixtures/recipe-solve",
            )
        })
}

fn recipe_keys(rid: &str) -> Vec<Value> {
    std::fs::read_to_string(recipe_fixtures().join("keys").join(format!("{rid}.jsonl")))
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn recipe_spec(
    root: &Path,
    job: &str,
    rid: &str,
    keys: &[Value],
    threads: u32,
    extra: &[&str],
    rule: Option<&str>,
) -> JobSpec {
    use rustred_campaign_contract::spec::*;
    let run = rustred_campaign_contract::layout::attempt_dir(root, job, 1);
    std::fs::create_dir_all(run.join("in")).unwrap();
    let fx = recipe_fixtures();
    let fam = fx.join("T178.sym");
    let mut rec = fx.join("recipes").join(format!("{rid}.json"));
    if let Some(rule) = rule {
        let mut r: Value = serde_json::from_str(&std::fs::read_to_string(&rec).unwrap()).unwrap();
        r["relation_rule"] = json!(rule);
        rec = run.join("in/recipe.json");
        std::fs::write(&rec, serde_json::to_string(&r).unwrap()).unwrap();
    }
    let kp = run.join("in/keys.jsonl");
    let text: String = keys.iter().map(|k| format!("{k}\n")).collect();
    std::fs::write(&kp, text).unwrap();
    let order = std::fs::read_to_string(fx.join("order.json")).unwrap();
    let mut args: Vec<String> = vec![
        "--order".into(),
        order,
        "--threads".into(),
        threads.to_string(),
    ];
    args.extend(extra.iter().map(|s| s.to_string()));
    let input = |role: &str, path: String, file: &Path| InputRef {
        role: role.into(),
        path,
        blake3: b3(file),
        upto: None,
    };
    JobSpec {
        schema: rustred_campaign_contract::CONTRACT.into(),
        job_id: job.into(),
        attempt: 1,
        kind: JobKind::SolveRecipe,
        run_dir: run.to_str().unwrap().into(),
        program: ProgramRef {
            name: "rr_node".into(),
            blake3: b3(&bin()),
            features_required: vec!["job".into(), "solve_recipe".into()],
        },
        inputs: vec![
            input("family", fam.to_str().unwrap().into(), &fam),
            input("recipe", rec.to_str().unwrap().into(), &rec),
            input("keys", "in/keys.jsonl".into(), &kp),
        ],
        args,
        seed: SeedSpec { sample_seed: 0 },
        budgets: Budgets::default(),
        on_case_budget: OnCaseBudget::Stop,
        determinism: Determinism::Recorded,
        output: OutputOptions::default(),
        heartbeat_s: 1,
        stop_grace_s: 5,
        resume_every_s: 0,
        resume: None,
        env_allow: vec![],
        fake: None,
    }
}

fn results_canon(spec: &JobSpec) -> Option<String> {
    canonical_of(spec, "results.jsonl")
}

/// Item 4: Determinism (same job twice; 1 vs 4 threads), duplicate keys, a key outside the request, and the exit-70-without
/// OUTCOME classification.
#[test]
#[ignore]
fn audit_eng2_recipe_determinism_and_keys() {
    let r = root("recipe");
    let rid = std::env::var("RR_AUDIT_RECIPE")
        .unwrap_or_else(|_| "results_47_fix10m1_n10_M1_json".into());
    let rule = std::env::var("RR_AUDIT_RULE").ok();
    let keys = recipe_keys(&rid);
    let keys: Vec<Value> = keys
        .into_iter()
        .take(env_u64("RR_AUDIT_RECIPE_KEYS", 3) as usize)
        .collect();
    let mut problems = Vec::new();
    let mut canon = Vec::new();
    for (k, threads) in [1u32, 1, 4].iter().enumerate() {
        let s = recipe_spec(
            &r,
            &format!("det{k}"),
            &rid,
            &keys,
            *threads,
            &[],
            rule.as_deref(),
        );
        let t = Instant::now();
        let w = run_to_end(&s);
        let cl = classified(&s, w, KillReason::None);
        let c = results_canon(&s);
        note(
            json!({"probe": "recipe_determinism", "threads": threads, "wait": format!("{w:?}"), "row": cl.row,
                    "accept": format!("{:?}", cl.accept), "canonical": c, "seconds": t.elapsed().as_secs_f64()}),
        );
        canon.push(c);
    }
    if canon.iter().any(|c| c != &canon[0]) {
        problems.push(format!(
            "results not canonically identical across runs / threads: {canon:?}"
        ));
    }
    // duplicate key: the same key twice in the request
    let mut dup = keys.clone();
    dup.push(keys[0].clone());
    let s = recipe_spec(&r, "dup", &rid, &dup, 1, &[], rule.as_deref());
    let w = run_to_end(&s);
    let cl = classified(&s, w, KillReason::None);
    let recs: Vec<Value> =
        std::fs::read_to_string(RunLayout::new(&s.run_dir).out_dir().join("results.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|v| v["type"] == "relation")
            .collect();
    let first = recs.first().map(|v| v["relation_digest"].clone());
    let last = recs.last().map(|v| v["relation_digest"].clone());
    note(
        json!({"probe": "recipe_duplicate_key", "wait": format!("{w:?}"), "row": cl.row, "accept": format!("{:?}", cl.accept),
                "records": recs.len(), "first_digest": first, "dup_digest": last}),
    );
    if recs.len() == dup.len() && first != last {
        problems.push("duplicate key: two different relations".into());
    }
    // exit 70 without OUTCOME (the verb's own exit paths after an I/O error): the supervisor's classification
    let ev = Evidence {
        kind: JobKind::SolveRecipe,
        wait: WaitStatus::Exited(70),
        outcome: OutcomeState::Absent,
        verified: None,
        unsealed_why: None,
        resume_valid: false,
        resume_ref: None,
        kill: KillReason::None,
        oom_kill_observed: false,
        elision_forbidden: true,
    };
    let cl = classify(&ev);
    note(
        json!({"probe": "recipe_exit70_no_outcome", "row": cl.row, "accept": format!("{:?}", cl.accept), "action": format!("{:?}", cl.action)}),
    );
    if cl.accept != Accept::Nothing {
        problems.push(format!("exit 70 without OUTCOME accepted (row {})", cl.row));
    }
    for p in &problems {
        eprintln!("DIFF {p}");
    }
    assert!(problems.is_empty(), "{} recipe problems", problems.len());
}

/// Item 4: The legacy flag form of solve-recipe pointed at a run directory (`--out RUN` and `--out RUN/out` with a
/// job.json in RUN): does it write anything the supervisor would classify as an accepted job result?
#[test]
#[ignore]
fn audit_eng2_recipe_legacy_form_is_not_a_job_result() {
    let r = root("recipe-legacy");
    let rid = "results_47_fix10m1_n10_M1_json";
    let keys: Vec<Value> = recipe_keys(rid).into_iter().take(1).collect();
    let mut problems = Vec::new();
    for (k, sub) in ["", "out"].iter().enumerate() {
        let s = recipe_spec(
            &r,
            &format!("leg{k}"),
            rid,
            &keys,
            1,
            &[],
            Some("first_admissible"),
        );
        let layout = RunLayout::new(&s.run_dir);
        std::fs::write(layout.job_spec(), s.to_json_line()).unwrap();
        let out_dir = if sub.is_empty() {
            PathBuf::from(&s.run_dir)
        } else {
            layout.out_dir()
        };
        let fx = recipe_fixtures();
        let order = std::fs::read_to_string(fx.join("order.json")).unwrap();
        let st = std::process::Command::new(bin())
            .args(["solve-recipe", "--family"])
            .arg(fx.join("T178.sym"))
            .arg("--order")
            .arg(order.trim())
            .arg("--recipe")
            .arg(RunLayout::new(&s.run_dir).root.join("in/recipe.json"))
            .arg("--keys")
            .arg(layout.root.join("in/keys.jsonl"))
            .arg("--out")
            .arg(&out_dir)
            .arg("--threads")
            .arg("1")
            .env("RAYON_NUM_THREADS", "1")
            .env("SYMBOLICA_HIDE_BANNER", "1")
            .stdout(std::process::Stdio::null())
            .stderr(std::fs::File::create(r.join(format!("leg{k}.stderr"))).unwrap())
            .status()
            .unwrap();
        let w = match st.code() {
            Some(c) => WaitStatus::Exited(c),
            None => WaitStatus::Unknown,
        };
        let cl = classified(&s, w, KillReason::None);
        note(
            json!({"probe": "recipe_legacy_form", "out": out_dir, "wait": format!("{w:?}"), "row": cl.row,
                    "accept": format!("{:?}", cl.accept), "why": cl.why,
                    "outcome_at_run_root": layout.outcome().exists()}),
        );
        if cl.accept != Accept::Nothing {
            problems.push(format!(
                "legacy solve-recipe with --out {} classified row {} {:?}",
                out_dir.display(),
                cl.row,
                cl.accept
            ));
        }
    }
    assert!(problems.is_empty(), "{problems:?}");
}

/// 3.10 for kind solve-recipe: the disk fills while keys are solved (tmpfs inside the namespace, filler at the
/// `case_search` phase). The contract asks for exit 50 (stop nospace, row 15: pause launches); anything accepted is a
/// defect; another non-accepting code is reported as a deviation.
#[test]
#[ignore]
fn audit_eng2_full_disk_solve_recipe() {
    let r = root("enospc-recipe");
    let mnt = r.join("mnt");
    assert!(
        mount_tmpfs(&mnt, 32 << 20),
        "tmpfs mount inside the namespace (run with LIGHT_PIDNS=1)"
    );
    let rid = "results_47_fix10m1_n10_M1_json";
    let keys: Vec<Value> = recipe_keys(rid).into_iter().take(2).collect();
    let mut problems = Vec::new();
    for k in 0..env_u64("RR_AUDIT_DISK_DRILLS", 2) {
        let s = recipe_spec(
            &mnt,
            &format!("sr{k}"),
            rid,
            &keys,
            1,
            &[],
            Some("first_admissible"),
        );
        let mut c = spawn(&s);
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(300) {
            if c.try_wait().unwrap().is_some() {
                break;
            }
            if events_of(&s)
                .iter()
                .any(|v| v["kind"] == "phase" && v["phase"] == "case_search")
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let filler = fill(&mnt);
        let w = wait(&mut c);
        let cl = classified(&s, w, KillReason::None);
        let o = outcome(&s);
        note(
            json!({"probe": "enospc_recipe", "drill": k, "wait": format!("{w:?}"), "row": cl.row, "accept": format!("{:?}", cl.accept),
                    "action": format!("{:?}", cl.action), "why": cl.why,
                    "outcome": o.as_ref().map(|o| json!({"exit": o.job.exit_code.code(), "stop": format!("{:?}", o.job.stop.code),
                        "error": o.job.error.as_ref().map(|e| format!("{:?}: {}", e.class, e.message))}))}),
        );
        std::fs::remove_file(&filler).unwrap();
        if cl.accept != Accept::Nothing {
            problems.push(format!(
                "ENOSPC solve-recipe attempt accepted (row {})",
                cl.row
            ));
        }
        // lane engfix4 (re-audit MJ-2 closed): contract 3.10 strictly: exit 50, an OUTCOME with stop nospace, row 15
        let typed = w == WaitStatus::Exited(50)
            && cl.row == 15
            && o.as_ref().is_some_and(|o| {
                o.job.exit_code.code() == 50
                    && o.job.stop.code == Some(rustred_campaign_contract::stop::StopCode::Nospace)
            });
        if !typed {
            problems.push(format!(
                "ENOSPC solve-recipe must end 50 with an OUTCOME stop nospace (row 15): {w:?}, row {}, OUTCOME {}",
                cl.row,
                o.is_some()
            ));
        }
        let _ = std::fs::remove_dir_all(&s.run_dir);
    }
    assert!(problems.is_empty(), "{problems:?}");
}
