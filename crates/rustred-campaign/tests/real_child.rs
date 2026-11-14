//! Conformance of the REAL child `rr_node job` with the child-job contract `rustred.child-job.v1` (WP-I8 lane eng2,
//! item E8): run directories written by the engine are classified with the contract crate's verifier and decision
//! table (`classify::assess` + `classify::classify`), exactly as the supervisor will, and compared with the fake
//! child's run directories of the same job shapes (files present, OUTCOME members).
//!
//! Every test is `#[ignore]`: it needs
//! - `RR_NODE_BIN`: an rr_node with the `job` subcommand;
//! - `RUSTRED_CAMPAIGN_FIXTURES`: the campaign fixture root (families of `small/m2-slice/`);
//! - the Symbolica licence environment (`source WS/ops/env.sh`);
//! - `LIGHT_PIDNS=1` for the tests that send signals (each test binary then runs in a private pid namespace);
//! - optionally `RR_REAL_OUT`: where the run directories are kept (default: the system temp dir).
//!
//! `cargo test -p rustred-campaign --test real_child -- --ignored --test-threads 1`

use rustred_campaign_contract::classify::{Classification, KillReason, WaitStatus, assess, classify};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::outcome::Outcome;
use rustred_campaign_contract::spec::*;
use rustred_campaign_contract::{ExitCode, StopCode};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

static N: AtomicU32 = AtomicU32::new(0);

fn bin() -> PathBuf {
    PathBuf::from(std::env::var("RR_NODE_BIN").expect("RR_NODE_BIN"))
}

fn family(node: &str) -> PathBuf {
    let root = std::env::var("RUSTRED_CAMPAIGN_FIXTURES").expect("RUSTRED_CAMPAIGN_FIXTURES");
    let p = Path::new(&root).join("small/m2-slice").join(node).join("family.sym");
    assert!(p.is_file(), "{}", p.display());
    p
}

fn b3(p: &Path) -> String {
    rustred_campaign_contract::blake3_hex(&std::fs::read(p).unwrap())
}

/// A fresh root for one test.
fn root(tag: &str) -> PathBuf {
    let base = std::env::var("RR_REAL_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    let p = base.join(format!(
        "real-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// A generate job spec for `node` with `args` (an explicit order among them).
fn spec(root: &Path, job: &str, attempt: u32, node: &str, args: &[&str]) -> JobSpec {
    let run = rustred_campaign_contract::layout::attempt_dir(root, job, attempt);
    std::fs::create_dir_all(&run).unwrap();
    let fam = family(node);
    JobSpec {
        schema: rustred_campaign_contract::CONTRACT.into(),
        job_id: job.into(),
        attempt,
        kind: JobKind::Generate,
        run_dir: run.to_str().unwrap().into(),
        program: ProgramRef {
            name: "rr_node".into(),
            blake3: b3(&bin()),
            features_required: vec!["job".into(), "no_elision".into()],
        },
        inputs: vec![InputRef {
            role: "family".into(),
            path: fam.to_str().unwrap().into(),
            blake3: b3(&fam),
            upto: None,
        }],
        args: args.iter().map(|s| s.to_string()).collect(),
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

/// Write job.json (read-only) and spawn the child in its own session, as the supervisor does (6.1).
fn spawn(spec: &JobSpec) -> Child {
    let layout = RunLayout::new(&spec.run_dir);
    let p = layout.job_spec();
    std::fs::write(&p, spec.to_json_line()).unwrap();
    let mut perm = std::fs::metadata(&p).unwrap().permissions();
    perm.set_readonly(true);
    std::fs::set_permissions(&p, perm).unwrap();
    let out = std::fs::File::create(layout.stdout_log()).unwrap();
    let err = std::fs::File::create(layout.stderr_log()).unwrap();
    let mut cmd = Command::new(bin());
    cmd.arg("job")
        .arg(&p)
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .env("RAYON_NUM_THREADS", "1")
        .env("OMP_NUM_THREADS", "1")
        .env("SYMBOLICA_HIDE_BANNER", "1")
        .env("TMPDIR", layout.tmp_dir());
    // SAFETY: setsid is async-signal-safe.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    cmd.spawn().unwrap()
}

fn wait(c: &mut Child) -> WaitStatus {
    let s = c.wait().unwrap();
    match (s.code(), s.signal()) {
        (Some(c), _) => WaitStatus::Exited(c),
        (None, Some(sig)) => WaitStatus::Signaled(sig),
        _ => WaitStatus::Unknown,
    }
}

fn signal(c: &Child, sig: i32) {
    // SAFETY: our own child, inside the test's private pid namespace (LIGHT_PIDNS=1).
    unsafe {
        libc::kill(c.id() as i32, sig);
    }
}

fn classified(spec: &JobSpec, w: WaitStatus, k: KillReason) -> Classification {
    let layout = RunLayout::new(&spec.run_dir);
    let digest = rustred_campaign_contract::spec::spec_blake3(&std::fs::read(layout.job_spec()).unwrap());
    let ev = assess(&layout, spec, &digest, w, k, false);
    let c = classify(&ev);
    eprintln!("{}: wait {w:?} -> row {} {:?} ({})", spec.run_dir, c.row, c.kind, c.why);
    c
}

fn outcome(spec: &JobSpec) -> Outcome {
    Outcome::read(&RunLayout::new(&spec.run_dir).outcome())
        .unwrap()
        .unwrap()
}

fn run_to_end(spec: &JobSpec) -> (WaitStatus, Duration) {
    let t = Instant::now();
    let mut c = spawn(spec);
    (wait(&mut c), t.elapsed())
}

/// Wait until the job has committed at least `rules` rules (EVENTS `case_finished`), or panic after `limit`.
fn wait_for_rules(spec: &JobSpec, rules: u64, limit: Duration) {
    let ev = RunLayout::new(&spec.run_dir).events();
    let t = Instant::now();
    loop {
        if let Ok(text) = std::fs::read_to_string(&ev) {
            let n = text
                .lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .filter(|v| v["kind"] == "case_finished")
                .filter_map(|v| v["rules_committed"].as_u64())
                .max()
                .unwrap_or(0);
            if n >= rules {
                return;
            }
        }
        assert!(t.elapsed() < limit, "no {rules} rules within {limit:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn canonical(spec: &JobSpec) -> String {
    outcome(spec).job.outputs.table.unwrap().canonical_blake3.unwrap()
}

#[test]
#[ignore]
fn complete_l4a_twice_canonically_identical() {
    let r = root("complete");
    let a = spec(&r, "l4a", 1, "L4a", &["--order", "id"]);
    let (w, t) = run_to_end(&a);
    assert_eq!(w, WaitStatus::Exited(0), "{t:?}");
    let c = classified(&a, w, KillReason::None);
    assert_eq!(c.row, 1);
    let b = spec(&r, "l4a-again", 1, "L4a", &["--order", "id"]);
    let (w2, _) = run_to_end(&b);
    assert_eq!(w2, WaitStatus::Exited(0));
    assert_eq!(canonical(&a), canonical(&b), "same spec, same binary: canonical tables equal");
    let o = outcome(&a);
    assert_eq!(o.job.exit_code, ExitCode::Complete);
    assert!(o.job.effective.is_some());
    assert_eq!(o.job.outputs.table.as_ref().unwrap().witness_loci_elided, 0);
    assert!(o.legacy.contains_key("complete"), "legacy OUTCOME fields kept");
}

#[test]
#[ignore]
fn one_case_job_stops_with_max_cases() {
    let r = root("maxcases");
    let mut s = spec(&r, "l4a-1", 1, "L4a", &["--order", "id"]);
    s.budgets.work.cases = Some(1);
    let (w, _) = run_to_end(&s);
    assert_eq!(w, WaitStatus::Exited(10));
    let o = outcome(&s);
    assert_eq!(o.job.stop.code, Some(StopCode::MaxCases));
    let c = classified(&s, w, KillReason::None);
    assert!(c.row == 2 || c.row == 24 || c.row == 3, "row {}", c.row);
    assert!(RunLayout::new(&s.run_dir).resume().is_file());
}

#[test]
#[ignore]
fn refusals_write_nothing_or_an_error_outcome() {
    let r = root("refuse");
    // unknown flag: allow-list -> 40 with OUTCOME
    let s = spec(&r, "bad-flag", 1, "L4a", &["--order", "id", "--frobnicate"]);
    let (w, _) = run_to_end(&s);
    assert_eq!(w, WaitStatus::Exited(40));
    assert_eq!(classified(&s, w, KillReason::None).row, 12);
    assert_eq!(outcome(&s).job.exit_code, ExitCode::Usage);
    // a work budget this build does not enforce: refused (fail closed), 42 with OUTCOME
    let mut s = spec(&r, "budget", 1, "L4a", &["--order", "id"]);
    s.budgets.work.seeds_per_case = Some(10);
    let (w, _) = run_to_end(&s);
    assert_eq!(w, WaitStatus::Exited(42));
    assert_eq!(classified(&s, w, KillReason::None).row, 14);
    // a finished run directory is final: 42, OUTCOME untouched
    let before = std::fs::read(RunLayout::new(&s.run_dir).outcome()).unwrap();
    let mut c = Command::new(bin())
        .arg("job")
        .arg(RunLayout::new(&s.run_dir).job_spec())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    assert_eq!(wait(&mut c), WaitStatus::Exited(42));
    assert_eq!(std::fs::read(RunLayout::new(&s.run_dir).outcome()).unwrap(), before);
    // out/ not empty: 42, nothing written
    let s = spec(&r, "dirty", 1, "L4a", &["--order", "id"]);
    std::fs::create_dir_all(RunLayout::new(&s.run_dir).out_dir()).unwrap();
    std::fs::write(RunLayout::new(&s.run_dir).out_dir().join("x"), b"x").unwrap();
    let (w, _) = run_to_end(&s);
    assert_eq!(w, WaitStatus::Exited(42));
    assert!(!RunLayout::new(&s.run_dir).outcome().exists());
    assert!(!RunLayout::new(&s.run_dir).events().exists());
}

/// T281 (arity 20, ~85 s, ~0.6 GB): graceful SIGTERM after some rules.
#[test]
#[ignore]
fn sigterm_is_a_graceful_stop_with_a_sealed_partial_table() {
    let r = root("graceful");
    let s = spec(&r, "t281-term", 1, "T281-b6", &["--order", "id"]);
    let mut c = spawn(&s);
    wait_for_rules(&s, 20, Duration::from_secs(120));
    let t = Instant::now();
    signal(&c, libc::SIGTERM);
    let w = wait(&mut c);
    let took = t.elapsed();
    eprintln!("graceful stop took {took:?}");
    assert_eq!(w, WaitStatus::Exited(30));
    let o = outcome(&s);
    assert!(o.job.stop.graceful);
    assert_eq!(o.job.stop.code, Some(StopCode::Interrupted));
    assert!(o.job.outputs.table.as_ref().unwrap().sealed);
    let cl = classified(&s, w, KillReason::Stop);
    assert_eq!(cl.row, 11);
    assert!(cl.resume_from.is_some(), "RESUME valid");
}

#[test]
#[ignore]
fn second_signal_is_the_hard_safe_stop() {
    let r = root("hard");
    let s = spec(&r, "t281-hard", 1, "T281-b6", &["--order", "id"]);
    let mut c = spawn(&s);
    wait_for_rules(&s, 10, Duration::from_secs(120));
    let t = Instant::now();
    signal(&c, libc::SIGTERM);
    signal(&c, libc::SIGTERM);
    let w = wait(&mut c);
    eprintln!("stop took {:?}", t.elapsed());
    assert_eq!(w, WaitStatus::Exited(30));
    let o = outcome(&s);
    assert_eq!(o.job.stop.code, Some(StopCode::Interrupted));
    let cl = classified(&s, w, KillReason::Stop);
    assert_eq!(cl.row, 11);
}

#[test]
#[ignore]
fn sigkill_mid_case_leaves_a_valid_resume() {
    let r = root("kill");
    let s = spec(&r, "t281-kill", 1, "T281-b6", &["--order", "id"]);
    let mut c = spawn(&s);
    wait_for_rules(&s, 15, Duration::from_secs(120));
    std::thread::sleep(Duration::from_millis(137));
    signal(&c, libc::SIGKILL);
    let w = wait(&mut c);
    assert_eq!(w, WaitStatus::Signaled(9));
    let cl = classified(&s, w, KillReason::None);
    assert_eq!(cl.row, 21);
    assert!(cl.resume_from.is_some(), "RESUME of the last commit point is valid");
}

#[test]
#[ignore]
fn job_wall_budget_is_exit_21() {
    let r = root("wall");
    let mut s = spec(&r, "t281-wall", 1, "T281-b6", &["--order", "id"]);
    s.budgets.wall.job_s = Some(8);
    let (w, t) = run_to_end(&s);
    assert_eq!(w, WaitStatus::Exited(21), "{t:?}");
    let o = outcome(&s);
    assert!(o.job.time_decided);
    assert_eq!(classified(&s, w, KillReason::None).row, 9);
}

/// Q14 / plan E-5: is `generate` deterministic at fixed inputs (two uninterrupted runs, canonical tables)?
#[test]
#[ignore]
fn t281_twice_canonically_identical() {
    let r = root("det");
    let a = spec(&r, "t281-a", 1, "T281-b6", &["--order", "id"]);
    let b = spec(&r, "t281-b", 1, "T281-b6", &["--order", "id"]);
    let (wa, ta) = run_to_end(&a);
    let (wb, tb) = run_to_end(&b);
    eprintln!("runs {ta:?} {tb:?}");
    assert_eq!(wa, WaitStatus::Exited(0));
    assert_eq!(wb, WaitStatus::Exited(0));
    assert_eq!(classified(&a, wa, KillReason::None).row, 1);
    assert_eq!(canonical(&a), canonical(&b));
}

/// Diagnostic: verify the table and cert of the run directory in `RR_VERIFY_DIR` with the contract verifier.
#[test]
#[ignore]
fn verify_dir() {
    let d = PathBuf::from(std::env::var("RR_VERIFY_DIR").expect("RR_VERIFY_DIR"));
    for (rel, ends) in [
        ("out/F.table.jsonl", &rustred_campaign_contract::seal::TABLE_END_TYPES),
        ("out/F.cert.jsonl", &rustred_campaign_contract::seal::CERT_END_TYPES),
    ] {
        match rustred_campaign_contract::seal::verify_stream(&d, rel, ends) {
            Ok(s) => eprintln!("{rel}: sealed bytes {} records {} rules {} end {}", s.bytes, s.records, s.rules, s.end),
            Err(e) => eprintln!("{rel}: NOT sealed: {e}"),
        }
        let mut sd = rustred_campaign_contract::seal::StreamDigest::new(ends);
        for c in std::fs::read(d.join(rel)).unwrap().chunks(8192) {
            sd.feed(c);
        }
        eprintln!("{rel}: StreamDigest seal {:?} failed {:?} boundary {}", sd.seal().is_some(), sd.failed(), sd.at_boundary());
    }
}

/// The next attempt of `prev`, resuming from its RESUME.json.
fn resume_spec(root: &Path, prev: &JobSpec) -> JobSpec {
    let attempt = prev.attempt + 1;
    let run = rustred_campaign_contract::layout::attempt_dir(root, &prev.job_id, attempt);
    std::fs::create_dir_all(&run).unwrap();
    let from = RunLayout::new(&prev.run_dir).resume();
    let mut s = prev.clone();
    s.attempt = attempt;
    s.run_dir = run.to_str().unwrap().into();
    s.resume = Some(rustred_campaign_contract::resume::ResumeRef {
        from: from.to_str().unwrap().into(),
        blake3: b3(&from),
    });
    s
}

/// E6: interrupt T281 at a safe point (graceful SIGTERM) and by SIGKILL mid-case, resume, and compare the final sealed
/// table with an uninterrupted run (canonical digest).
#[test]
#[ignore]
fn interrupted_and_resumed_equals_uninterrupted() {
    let r = root("resume");
    let full = spec(&r, "t281-full", 1, "T281-b6", &["--order", "id"]);
    let (w, t) = run_to_end(&full);
    assert_eq!(w, WaitStatus::Exited(0), "{t:?}");
    let want = canonical(&full);
    for (k, (rules, sig)) in [(7u64, libc::SIGTERM), (150, libc::SIGKILL), (300, libc::SIGTERM)].into_iter().enumerate() {
        let a = spec(&r, &format!("t281-r{k}"), 1, "T281-b6", &["--order", "id"]);
        let mut c = spawn(&a);
        wait_for_rules(&a, rules, Duration::from_secs(300));
        std::thread::sleep(Duration::from_millis(37 * k as u64));
        signal(&c, sig);
        let w = wait(&mut c);
        let kill = if sig == libc::SIGKILL { KillReason::None } else { KillReason::Stop };
        let cl = classified(&a, w, kill);
        assert!(cl.resume_from.is_some(), "attempt 1 resumable (row {})", cl.row);
        let b = resume_spec(&r, &a);
        let (w2, t2) = run_to_end(&b);
        eprintln!("resume {k}: interrupted after >= {rules} rules by signal {sig}, resumed run {t2:?} -> {w2:?}");
        assert_eq!(w2, WaitStatus::Exited(0));
        assert_eq!(classified(&b, w2, KillReason::None).row, 1);
        assert!(outcome(&b).job.resumed);
        assert_eq!(canonical(&b), want, "resumed table canonically identical to the uninterrupted one");
    }
}

/// `--descend open`: the job solves its own case(s) and returns the sub-cases (sealed F.subcases.jsonl); a second job
/// takes some of them back as PRESENTED cases.
#[test]
#[ignore]
fn descend_open_returns_subcases_that_are_accepted_back() {
    let r = root("descend");
    let a = spec(&r, "l4a-top", 1, "L4a", &["--order", "id", "--descend", "open"]);
    let (w, _) = run_to_end(&a);
    assert_eq!(w, WaitStatus::Exited(10));
    let o = outcome(&a);
    assert_eq!(o.job.stop.code, Some(StopCode::MaxCases));
    assert_eq!(o.job.counters.cases, 1, "only the job's own case was searched");
    let cl = classified(&a, w, KillReason::None);
    assert!(cl.row == 2 || cl.row == 24, "row {}", cl.row);
    let sub = std::fs::read_to_string(RunLayout::new(&a.run_dir).subcases()).unwrap();
    let cases: Vec<serde_json::Value> = sub
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .filter(|v| v["type"] == "subcase")
        .collect();
    eprintln!("{} sub-cases returned", cases.len());
    assert!(!cases.is_empty());
    let input = r.join("cases.jsonl");
    let lines: Vec<String> = cases
        .iter()
        .filter(|v| v["kind"] == "case")
        .take(3)
        .map(|v| v["case"].to_string())
        .collect();
    std::fs::write(&input, lines.join("\n") + "\n").unwrap();
    let mut b = spec(&r, "l4a-children", 1, "L4a", &["--order", "id", "--descend", "open"]);
    b.inputs.push(InputRef {
        role: "cases".into(),
        path: input.to_str().unwrap().into(),
        blake3: b3(&input),
        upto: None,
    });
    let (w, _) = run_to_end(&b);
    assert!(w == WaitStatus::Exited(10) || w == WaitStatus::Exited(0), "{w:?}");
    let ob = outcome(&b);
    assert!(ob.job.counters.cases <= lines.len() as u64);
    let clb = classified(&b, w, KillReason::None);
    assert!(matches!(clb.row, 1 | 2 | 3 | 24), "row {}", clb.row);
}

/// Differential check against the fake child: the same job shape (a one-case stop with max_cases) leaves the same
/// top-level files and the same OUTCOME `job` members.
#[test]
#[ignore]
fn same_files_and_outcome_members_as_the_fake_child() {
    use rustred_campaign_contract::fake::*;
    let r = root("diff");
    let mut real = spec(&r, "real", 1, "L4a", &["--order", "id"]);
    real.budgets.work.cases = Some(1);
    let (w, _) = run_to_end(&real);
    assert_eq!(w, WaitStatus::Exited(10));
    let run = rustred_campaign_contract::layout::attempt_dir(&r, "fake", 1);
    std::fs::create_dir_all(&run).unwrap();
    let mut fake = real.clone();
    fake.job_id = "fake".into();
    fake.run_dir = run.to_str().unwrap().into();
    fake.kind = JobKind::Fake;
    fake.args = vec![];
    fake.program = ProgramRef {
        name: "rr_fake_child".into(),
        blake3: "0".repeat(64),
        features_required: vec!["fake".into()],
    };
    fake.fake = Some(FakeScript {
        script: vec![
            FakeOp::Case {
                ordinal: 0,
                ms: 10,
                work: Default::default(),
                result: rustred_campaign_contract::stop::CaseResult::Rule,
                rule: Some(FakeRule {
                    record: serde_json::json!({"chart":"1","case":0,"target":"I(1)","rhs":[]}),
                    loci: None,
                }),
                children: vec![],
            },
            FakeOp::End { stop: Some(StopCode::MaxCases), verdict: None },
        ],
        on_sigterm: OnSigterm::Graceful,
        slow_ms: 0,
        start_delay_ms: 0,
        rss: RssPlan::default(),
        helpers: vec![],
        faults: vec![],
        time_scale: 1.0,
        volatile_jitter: false,
    });
    let p = RunLayout::new(&fake.run_dir).job_spec();
    std::fs::write(&p, fake.to_json_line()).unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_rr_fake_child"))
        .arg("job")
        .arg(&p)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    assert_eq!(wait(&mut c), WaitStatus::Exited(10));
    let names = |d: &str| -> std::collections::BTreeSet<String> {
        std::fs::read_dir(d)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .filter(|n| !n.ends_with(".log"))
            .collect()
    };
    assert_eq!(names(&real.run_dir), names(&fake.run_dir), "top-level files");
    let keys = |s: &JobSpec| -> Vec<String> {
        let v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(RunLayout::new(&s.run_dir).outcome()).unwrap()).unwrap();
        let mut k: Vec<String> = v["job"].as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    };
    assert_eq!(keys(&real), keys(&fake), "OUTCOME job members");
    let cr = classified(&real, WaitStatus::Exited(10), KillReason::None);
    let cf = classified(&fake, WaitStatus::Exited(10), KillReason::None);
    assert_eq!(cr.row, cf.row, "same decision-table row");
}
