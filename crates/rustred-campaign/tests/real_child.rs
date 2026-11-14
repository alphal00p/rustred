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

use rustred_campaign_contract::classify::{
    Classification, KillReason, WaitStatus, assess, classify,
};
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
    let p = Path::new(&root)
        .join("small/m2-slice")
        .join(node)
        .join("family.sym");
    assert!(p.is_file(), "{}", p.display());
    p
}

fn b3(p: &Path) -> String {
    rustred_campaign_contract::blake3_hex(&std::fs::read(p).unwrap())
}

/// A fresh root for one test.
fn root(tag: &str) -> PathBuf {
    // lane int5: a NEW directory unique on the host (never the pid alone; an existing one is never removed)
    let u = unique_temp_dir(&format!("real-{tag}"));
    let _ = N.fetch_add(1, Ordering::Relaxed);
    match std::env::var("RR_REAL_OUT") {
        Ok(b) => {
            let p = PathBuf::from(b).join(u.file_name().unwrap());
            let _ = std::fs::remove_dir(&u);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::create_dir(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            p
        }
        Err(_) => u,
    }
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
    spawn_with(spec, &["job"])
}

/// [`spawn`] with the program words before the spec path (`job`, or the alias `solve-recipe --job`).
fn spawn_with(spec: &JobSpec, words: &[&str]) -> Child {
    let layout = RunLayout::new(&spec.run_dir);
    let p = layout.job_spec();
    std::fs::write(&p, spec.to_json_line()).unwrap();
    let mut perm = std::fs::metadata(&p).unwrap().permissions();
    perm.set_readonly(true);
    std::fs::set_permissions(&p, perm).unwrap();
    let out = std::fs::File::create(layout.stdout_log()).unwrap();
    let err = std::fs::File::create(layout.stderr_log()).unwrap();
    let mut cmd = Command::new(bin());
    cmd.args(words)
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
    let digest =
        rustred_campaign_contract::spec::spec_blake3(&std::fs::read(layout.job_spec()).unwrap());
    let ev = assess(&layout, spec, &digest, w, k, false);
    let c = classify(&ev);
    eprintln!(
        "{}: wait {w:?} -> row {} {:?} ({})",
        spec.run_dir, c.row, c.kind, c.why
    );
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
    outcome(spec)
        .job
        .outputs
        .table
        .unwrap()
        .canonical_blake3
        .unwrap()
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
    assert_eq!(
        canonical(&a),
        canonical(&b),
        "same spec, same binary: canonical tables equal"
    );
    let o = outcome(&a);
    assert_eq!(o.job.exit_code, ExitCode::Complete);
    assert!(o.job.effective.is_some());
    assert_eq!(o.job.outputs.table.as_ref().unwrap().witness_loci_elided, 0);
    assert!(
        o.legacy.contains_key("complete"),
        "legacy OUTCOME fields kept"
    );
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
    assert_eq!(
        std::fs::read(RunLayout::new(&s.run_dir).outcome()).unwrap(),
        before
    );
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
    // two DIFFERENT signals back to back: they cannot merge into one pending signal (two SIGTERMs sent at once can),
    // so the child receives two signals, normally inside one 20 ms watchdog poll
    signal(&c, libc::SIGTERM);
    signal(&c, libc::SIGINT);
    let w = wait(&mut c);
    eprintln!("stop took {:?}", t.elapsed());
    assert_eq!(w, WaitStatus::Exited(30));
    let o = outcome(&s);
    assert_eq!(o.job.stop.code, Some(StopCode::Interrupted));
    // (lane engfix3, audit-engjob m3) two signals inside one watchdog poll are a second signal: hard-safe stop
    assert!(!o.job.stop.graceful, "hard-safe stop");
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
    assert!(
        cl.resume_from.is_some(),
        "RESUME of the last commit point is valid"
    );
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
        (
            "out/F.table.jsonl",
            &rustred_campaign_contract::seal::TABLE_END_TYPES,
        ),
        (
            "out/F.cert.jsonl",
            &rustred_campaign_contract::seal::CERT_END_TYPES,
        ),
    ] {
        match rustred_campaign_contract::seal::verify_stream(&d, rel, ends) {
            Ok(s) => eprintln!(
                "{rel}: sealed bytes {} records {} rules {} end {}",
                s.bytes, s.records, s.rules, s.end
            ),
            Err(e) => eprintln!("{rel}: NOT sealed: {e}"),
        }
        let mut sd = rustred_campaign_contract::seal::StreamDigest::new(ends);
        for c in std::fs::read(d.join(rel)).unwrap().chunks(8192) {
            sd.feed(c);
        }
        eprintln!(
            "{rel}: StreamDigest seal {:?} failed {:?} boundary {}",
            sd.seal().is_some(),
            sd.failed(),
            sd.at_boundary()
        );
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
    for (k, (rules, sig)) in [
        (7u64, libc::SIGTERM),
        (150, libc::SIGKILL),
        (300, libc::SIGTERM),
    ]
    .into_iter()
    .enumerate()
    {
        let a = spec(&r, &format!("t281-r{k}"), 1, "T281-b6", &["--order", "id"]);
        let mut c = spawn(&a);
        wait_for_rules(&a, rules, Duration::from_secs(300));
        std::thread::sleep(Duration::from_millis(37 * k as u64));
        signal(&c, sig);
        let w = wait(&mut c);
        let kill = if sig == libc::SIGKILL {
            KillReason::None
        } else {
            KillReason::Stop
        };
        let cl = classified(&a, w, kill);
        assert!(
            cl.resume_from.is_some(),
            "attempt 1 resumable (row {})",
            cl.row
        );
        let b = resume_spec(&r, &a);
        let (w2, t2) = run_to_end(&b);
        eprintln!(
            "resume {k}: interrupted after >= {rules} rules by signal {sig}, resumed run {t2:?} -> {w2:?}"
        );
        assert_eq!(w2, WaitStatus::Exited(0));
        assert_eq!(classified(&b, w2, KillReason::None).row, 1);
        assert!(outcome(&b).job.resumed);
        assert_eq!(
            canonical(&b),
            want,
            "resumed table canonically identical to the uninterrupted one"
        );
    }
}

/// `--descend open`: the job solves its own case(s) and returns the sub-cases (sealed F.subcases.jsonl); a second job
/// takes some of them back as PRESENTED cases.
#[test]
#[ignore]
fn descend_open_returns_subcases_that_are_accepted_back() {
    let r = root("descend");
    let a = spec(
        &r,
        "l4a-top",
        1,
        "L4a",
        &["--order", "id", "--descend", "open"],
    );
    let (w, _) = run_to_end(&a);
    assert_eq!(w, WaitStatus::Exited(10));
    let o = outcome(&a);
    assert_eq!(o.job.stop.code, Some(StopCode::MaxCases));
    assert_eq!(
        o.job.counters.cases, 1,
        "only the job's own case was searched"
    );
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
    let mut b = spec(
        &r,
        "l4a-children",
        1,
        "L4a",
        &["--order", "id", "--descend", "open"],
    );
    b.inputs.push(InputRef {
        role: "cases".into(),
        path: input.to_str().unwrap().into(),
        blake3: b3(&input),
        upto: None,
    });
    let (w, _) = run_to_end(&b);
    assert!(
        w == WaitStatus::Exited(10) || w == WaitStatus::Exited(0),
        "{w:?}"
    );
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
            FakeOp::End {
                stop: Some(StopCode::MaxCases),
                verdict: None,
            },
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
    assert_eq!(
        names(&real.run_dir),
        names(&fake.run_dir),
        "top-level files"
    );
    let keys = |s: &JobSpec| -> Vec<String> {
        let v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(RunLayout::new(&s.run_dir).outcome()).unwrap())
                .unwrap();
        let mut k: Vec<String> = v["job"].as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    };
    assert_eq!(keys(&real), keys(&fake), "OUTCOME job members");
    let cr = classified(&real, WaitStatus::Exited(10), KillReason::None);
    let cf = classified(&fake, WaitStatus::Exited(10), KillReason::None);
    assert_eq!(cr.row, cf.row, "same decision-table row");
}

/// The table header, the queue log and OUTCOME of a job (lane engbase helpers).
fn table_lines(spec: &JobSpec) -> Vec<serde_json::Value> {
    std::fs::read_to_string(RunLayout::new(&spec.run_dir).table())
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .collect()
}

/// (lane engbase) the engine line's seed options (coord-ec1-seed-centre.md v2) pass the allow-list, reach the engine
/// through the legacy parser and are recorded (header node_options, queue seed_recipe, rule seed_setting, OUTCOME
/// seed_recipe); the job ends through the contract (sealed table, a decision-table row of a producing kind).
#[test]
#[ignore]
fn engine_line_seed_options_reach_the_engine_in_job_mode() {
    let r = root("seedopts");
    let mut s = spec(
        &r,
        "l4a-seed",
        1,
        "L4a",
        &[
            "--order",
            "id",
            "--max-depth",
            "3",
            "--seed-layer-cap",
            "2,2,1",
            "--seed-centre-offset",
            "0:-1",
            "--seed-face-floor",
            "1",
            "--symmetry-symbolic",
        ],
    );
    s.budgets.work.cases = Some(2);
    let (w, t) = run_to_end(&s);
    assert!(
        w == WaitStatus::Exited(0) || w == WaitStatus::Exited(10),
        "{w:?} {t:?}"
    );
    let c = classified(&s, w, KillReason::None);
    assert!(
        matches!(c.row, 1 | 2 | 3 | 24 | 4 | 5 | 27),
        "row {}",
        c.row
    );
    let lines = table_lines(&s);
    let opts = &lines[0]["node_options"];
    assert_eq!(lines[0]["type"], "header");
    assert_eq!(
        opts["seed_layer_cap"],
        serde_json::json!([2, 2, 1]),
        "{opts}"
    );
    assert_eq!(opts["seed_face_floor"], 1);
    assert_eq!(opts["max_depth"], 3);
    assert_eq!(opts["symmetry_symbolic"], true);
    assert_eq!(opts["seed_centre_offset"][0], -1);
    let settings: Vec<&serde_json::Value> = lines
        .iter()
        .filter(|v| v["type"] == "rule")
        .filter_map(|v| v.get("seed_setting"))
        .collect();
    eprintln!(
        "{} rules, {} with seed_setting",
        lines.iter().filter(|v| v["type"] == "rule").count(),
        settings.len()
    );
    for st in &settings {
        assert_eq!(st["layer_cap"], serde_json::json!([2, 2, 1]), "{st}");
        assert_eq!(st["face_floor"], 1, "{st}");
    }
    let queue = std::fs::read_to_string(RunLayout::new(&s.run_dir).queue()).unwrap();
    assert!(queue.contains("\"type\":\"seed_recipe\""));
    let o = outcome(&s);
    assert!(
        o.legacy.contains_key("seed_recipe"),
        "legacy OUTCOME seed_recipe"
    );
    assert!(o.job.outputs.table.as_ref().unwrap().sealed);
}

/// (lane engbase) a PRESENTED case of the cases input may carry `seed_centre_offset` (feature case_seed_centres): it
/// becomes the engine line's per-case seed centre (header node_options.case_seed_centres); an ORIGINAL case with a
/// centre is refused (42).
#[test]
#[ignore]
fn presented_case_seed_centres_are_accepted() {
    let r = root("casecentre");
    let mut off = vec![0i64; 14];
    off[0] = -1;
    let line = serde_json::json!({"frame": "PRESENTED", "fixed": vec![serde_json::Value::Null; 14],
        "equations": [], "seed_centre_offset": off});
    let input = r.join("cases.jsonl");
    std::fs::write(&input, format!("{line}\n")).unwrap();
    let mut s = spec(
        &r,
        "l4a-centre",
        1,
        "L4a",
        &["--order", "id", "--max-depth", "2"],
    );
    s.budgets.work.cases = Some(1);
    s.program.features_required.push("case_seed_centres".into());
    s.inputs.push(InputRef {
        role: "cases".into(),
        path: input.to_str().unwrap().into(),
        blake3: b3(&input),
        upto: None,
    });
    let (w, _) = run_to_end(&s);
    assert!(
        w == WaitStatus::Exited(0) || w == WaitStatus::Exited(10),
        "{w:?}"
    );
    let c = classified(&s, w, KillReason::None);
    assert!(matches!(c.row, 1 | 2 | 3 | 24 | 4), "row {}", c.row);
    let lines = table_lines(&s);
    let centres = &lines[0]["node_options"]["case_seed_centres"];
    assert_eq!(centres.as_array().map(Vec::len), Some(1), "{centres}");
    assert_eq!(centres[0]["seed_centre_offset"][0], -1);
    // ORIGINAL frame with a centre: refused
    let line = serde_json::json!({"frame": "ORIGINAL", "fixed": vec![serde_json::Value::Null; 14],
        "equations": [], "seed_centre_offset": off});
    let input = r.join("cases-original.jsonl");
    std::fs::write(&input, format!("{line}\n")).unwrap();
    let mut s = spec(&r, "l4a-centre-orig", 1, "L4a", &["--order", "id"]);
    s.inputs.push(InputRef {
        role: "cases".into(),
        path: input.to_str().unwrap().into(),
        blake3: b3(&input),
        upto: None,
    });
    let (w, _) = run_to_end(&s);
    assert_eq!(w, WaitStatus::Exited(42));
    assert_eq!(classified(&s, w, KillReason::None).row, 14);
}

/// (lane engbase) `--symmetry` (sym-gen `sym` / `sym_zero` case records, no rule in the contract) is refused (42).
#[test]
#[ignore]
fn symmetry_case_records_are_refused_in_job_mode() {
    let r = root("symrefuse");
    let s = spec(&r, "l4a-sym", 1, "L4a", &["--order", "id", "--symmetry"]);
    let (w, _) = run_to_end(&s);
    assert_eq!(w, WaitStatus::Exited(42));
    assert_eq!(classified(&s, w, KillReason::None).row, 14);
    let o = outcome(&s);
    assert_eq!(o.job.exit_code, ExitCode::Refused);
    assert!(o.job.outputs.table.is_none() || !o.job.outputs.table.as_ref().unwrap().sealed);
}

/// A NEW directory `<TMPDIR>/<prefix>-<token>` unique on the host (lane int5, procfix3 item 0): never derived from the
/// pid alone (inside the private pid namespaces of rule L10 every test binary is pid 1, and two lanes may share a temp
/// root), and created with `create_dir` (a directory it did not create is never reused or removed).
#[allow(dead_code)]
pub fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let base = std::env::temp_dir();
    std::fs::create_dir_all(&base).unwrap();
    let ns = std::fs::read_link("/proc/self/ns/pid")
        .map(|l| l.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    loop {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let token = blake3::hash(format!("{ns}|{}|{stat}|{t}|{n}", std::process::id()).as_bytes())
            .to_hex()[..12]
            .to_string();
        let p = base.join(format!("{prefix}-{token}"));
        match std::fs::create_dir(&p) {
            Ok(()) => return p,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("{}: {e}", p.display()),
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------
// lane engfix3: regression tests of the engine child-job audit (WS/notes/implementer.audit-engjob.md)

/// A generate job spec with a `cases` input made of `lines` (written once per job id under the test root, so every
/// attempt of the job names the same file and digest).
fn spec_with_cases(
    root: &Path,
    job: &str,
    attempt: u32,
    node: &str,
    args: &[&str],
    lines: &[serde_json::Value],
) -> JobSpec {
    let input = root.join(format!("{job}.cases.jsonl"));
    if !input.exists() {
        let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
        std::fs::write(&input, text).unwrap();
    }
    let mut s = spec(root, job, attempt, node, args);
    s.inputs.push(InputRef {
        role: "cases".into(),
        path: input.to_str().unwrap().into(),
        blake3: b3(&input),
        upto: None,
    });
    s
}

fn events_of(spec: &JobSpec) -> Vec<serde_json::Value> {
    std::fs::read_to_string(RunLayout::new(&spec.run_dir).events())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .collect()
}

/// Wait until `pred` holds on the job's EVENTS (polled every 2 ms), or panic after `limit`.
fn wait_events(
    spec: &JobSpec,
    limit: Duration,
    what: &str,
    pred: impl Fn(&[serde_json::Value]) -> bool,
) {
    let t = Instant::now();
    loop {
        if pred(&events_of(spec)) {
            return;
        }
        assert!(t.elapsed() < limit, "{what}: not within {limit:?}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn count_kind(
    ev: &[serde_json::Value],
    kind: &str,
    f: impl Fn(&serde_json::Value) -> bool,
) -> usize {
    ev.iter().filter(|v| v["kind"] == kind && f(v)).count()
}

fn resume_json(spec: &JobSpec) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(RunLayout::new(&spec.run_dir).resume()).unwrap()).unwrap()
}

/// Canonical digest of a stream of the run (`out/<name>`), None when absent.
fn canonical_of(spec: &JobSpec, name: &str) -> Option<String> {
    let p = RunLayout::new(&spec.run_dir).out_dir().join(name);
    p.exists().then(|| {
        rustred_campaign_contract::canon::canonical_digest_file(&p)
            .unwrap()
            .0
    })
}

/// The C1 numeric-endpoint fixture: L3a (arity 9, chart 8 lines + 1 numerator) with 8 fully fixed ORIGINAL cases
/// (order id: presented = original) under `--numeric-depth 0 --numeric-depth-max 3`. Every endpoint exhausts the
/// depth-0 search; seven get a per-point rule at depth 1; the master I(1^8,0) stays exhausted through depth 3 (its retries make it the slow endpoint) and is
/// bulk-solved after the loop (one residual). The engine's numeric order puts the master 7th of 8, so a stop at the
/// 8th endpoint, or after the loop, has an exhausted endpoint waiting for the shared solve (audit-engjob C1).
const L3A_POINTS: [[i64; 9]; 8] = [
    [1, 1, 1, 1, 1, 1, 1, 1, 0],
    [2, 1, 1, 1, 1, 1, 1, 1, 0],
    [1, 2, 1, 1, 1, 1, 1, 1, 0],
    [1, 1, 1, 2, 1, 1, 1, 1, 0],
    [1, 1, 1, 1, 1, 1, 1, 1, -1],
    [1, 1, 1, 1, 1, 2, 1, 1, 0],
    [1, 1, 1, 1, 1, 1, 1, 2, 0],
    [2, 1, 1, 1, 1, 1, 1, 1, -1],
];

fn numeric_spec(root: &Path, job: &str) -> JobSpec {
    let lines: Vec<serde_json::Value> = L3A_POINTS
        .iter()
        .map(|p| serde_json::json!({"frame": "ORIGINAL", "fixed": p, "equations": []}))
        .collect();
    spec_with_cases(
        root,
        job,
        1,
        "L3a",
        &[
            "--order",
            "id",
            "--numeric-depth",
            "0",
            "--numeric-depth-max",
            "3",
        ],
        &lines,
    )
}

/// audit-engjob C1 (REQUIRED TEST): interrupt the NUMERIC stage at several endpoints (graceful SIGTERM after the
/// k-th numeric `case_started`, and a SIGKILL), resume, and require the final sealed table (and cert) to be
/// canonically identical to the uninterrupted run. At least one interruption must land after the exhausted master
/// (RESUME `engine.numeric_stage.exhausted` non-empty): the case the audit found broken. `RR_DRILL_CANONICAL_ONLY=1`
/// skips the checks of the new RESUME form (to run the drill against a binary from before the fix).
#[test]
#[ignore]
fn numeric_stage_interrupted_and_resumed_equals_uninterrupted() {
    let r = root("numeric");
    let full = numeric_spec(&r, "l3a-num-full");
    let (w, t) = run_to_end(&full);
    assert_eq!(w, WaitStatus::Exited(0), "{t:?}");
    assert_eq!(classified(&full, w, KillReason::None).row, 1);
    let o = outcome(&full);
    assert_eq!(o.legacy["numerical_cases"], 8, "8 numeric endpoints");
    assert_eq!(
        o.legacy["numeric_bulk_fallback"], 1,
        "one endpoint bulk-solved"
    );
    assert_eq!(o.legacy["residuals"], 1);
    let want = canonical(&full);
    let want_cert = canonical_of(&full, "F.cert.jsonl");
    let numeric_started =
        |ev: &[serde_json::Value]| count_kind(ev, "case_started", |v| v["numerical"] == 1);
    let mut landed: Vec<(usize, usize, i32)> = Vec::new();
    for (k, (started, sig)) in [
        (1usize, libc::SIGTERM),
        (3, libc::SIGTERM),
        (5, libc::SIGKILL),
        (6, libc::SIGTERM),
        (7, libc::SIGTERM),
        (8, libc::SIGTERM),
    ]
    .into_iter()
    .enumerate()
    {
        let a = numeric_spec(&r, &format!("l3a-num-r{k}"));
        let mut c = spawn(&a);
        wait_events(&a, Duration::from_secs(120), "numeric case_started", |ev| {
            numeric_started(ev) >= started
        });
        if sig == libc::SIGKILL {
            std::thread::sleep(Duration::from_millis(40));
        }
        signal(&c, sig);
        let w = wait(&mut c);
        if w == WaitStatus::Exited(0) {
            eprintln!(
                "drill {k}: the job ended before the stop (started {started}): no interruption"
            );
            continue;
        }
        let kill = if sig == libc::SIGKILL {
            KillReason::None
        } else {
            KillReason::Stop
        };
        let cl = classified(&a, w, kill);
        assert!(
            cl.resume_from.is_some(),
            "drill {k}: RESUME valid (row {})",
            cl.row
        );
        let res = resume_json(&a);
        let stage = &res["engine"]["numeric_stage"];
        if std::env::var_os("RR_DRILL_CANONICAL_ONLY").is_some() {
            let b = resume_spec(&r, &a);
            let (w2, _) = run_to_end(&b);
            let same = w2 == WaitStatus::Exited(0) && canonical(&b) == want;
            eprintln!(
                "drill {k} (canonical only): RESUME current {} numerical {}; resumed {w2:?}; canonical identical: {same}",
                res["current"],
                res["numerical"].as_array().map_or(0, Vec::len)
            );
            landed.push((usize::from(same), 0, sig));
            continue;
        }
        assert!(
            stage.is_object(),
            "drill {k}: a numeric-stage RESUME carries engine.numeric_stage: {res}"
        );
        assert_eq!(stage["numerical_cases"], 8);
        assert!(
            res["current"].is_null(),
            "drill {k}: a numeric endpoint is never `current`"
        );
        assert!(
            res["pending"].as_array().unwrap().is_empty(),
            "drill {k}: no symbolic case"
        );
        let (left, ex) = (
            res["numerical"].as_array().unwrap().len(),
            stage["exhausted"].as_array().unwrap().len(),
        );
        // a graceful stop (a safe point within stop_grace_s) seals F.subcases; a stop during a search longer than the
        // grace is the hard-safe stop (RESUME of the last commit point, no sealed output)
        if sig == libc::SIGTERM && outcome(&a).job.stop.graceful {
            assert_eq!(w, WaitStatus::Exited(30));
            // F.subcases of the stopped attempt: every open endpoint (not yet searched or exhausted) as kind numeric
            let sub = std::fs::read_to_string(RunLayout::new(&a.run_dir).subcases()).unwrap();
            let kinds: Vec<serde_json::Value> = sub
                .lines()
                .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
                .filter(|v| v["type"] == "subcase")
                .map(|v| v["kind"].clone())
                .collect();
            assert_eq!(kinds.len(), left + ex, "drill {k}: {sub}");
            assert!(
                kinds.iter().all(|x| x == "numeric"),
                "drill {k} (m7): {sub}"
            );
        }
        landed.push((left, ex, sig));
        let b = resume_spec(&r, &a);
        let (w2, t2) = run_to_end(&b);
        eprintln!(
            "drill {k}: stop after {started} numeric starts by signal {sig}: RESUME numerical {left}, exhausted {ex}; resumed run {t2:?} -> {w2:?}"
        );
        assert_eq!(w2, WaitStatus::Exited(0));
        assert_eq!(classified(&b, w2, KillReason::None).row, 1);
        let ob = outcome(&b);
        assert!(ob.job.resumed);
        assert_eq!(ob.legacy["numerical_cases"], 8);
        assert_eq!(
            canonical(&b),
            want,
            "drill {k}: resumed table canonically identical"
        );
        assert_eq!(
            canonical_of(&b, "F.cert.jsonl"),
            want_cert,
            "drill {k}: cert"
        );
    }
    eprintln!("landed (numerical left, exhausted, signal): {landed:?}");
    if std::env::var_os("RR_DRILL_CANONICAL_ONLY").is_some() {
        assert!(
            landed.iter().all(|&(same, _, _)| same == 1),
            "resumed tables differ from the uninterrupted one: {landed:?}"
        );
        return;
    }
    assert!(
        landed.len() >= 4,
        "at least four interruptions inside the numeric stage: {landed:?}"
    );
    assert!(
        landed.iter().any(|&(_, ex, _)| ex >= 1),
        "an interruption after the exhausted endpoint (C1): {landed:?}"
    );
}

/// The PRESENTED case of rule `index` of a frozen fixture table (generated with order id).
fn fixture_case(node: &str, index: u64) -> serde_json::Value {
    let root = std::env::var("RUSTRED_CAMPAIGN_FIXTURES").expect("RUSTRED_CAMPAIGN_FIXTURES");
    let p = Path::new(&root)
        .join("small/m2-slice")
        .join(node)
        .join("table.jsonl");
    let text = std::fs::read_to_string(&p).unwrap();
    let v = text
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .find(|v| v["type"] == "rule" && v["index"] == index)
        .unwrap_or_else(|| panic!("{node} rule {index}"));
    serde_json::json!({"frame": "PRESENTED", "fixed": v["fixed"], "equations": v["equations"]})
}

fn subcase_lines(spec: &JobSpec) -> Vec<serde_json::Value> {
    std::fs::read_to_string(RunLayout::new(&spec.run_dir).subcases())
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .filter(|v| v["type"] == "subcase")
        .collect()
}

/// Own cases of the C2 test: four T281-b6 cases with exceptional children (rules 363, 367, 370, 380 of the
/// frozen table, 0.6-1.2 s each).
fn descend_spec(root: &Path, job: &str) -> JobSpec {
    let lines: Vec<serde_json::Value> = [363, 367, 370, 380]
        .iter()
        .map(|&i| fixture_case("T281-b6", i))
        .collect();
    spec_with_cases(
        root,
        job,
        1,
        "T281-b6",
        &["--order", "id", "--descend", "open"],
        &lines,
    )
}

/// audit-engjob C2 (REQUIRED TEST): `--descend open` with resume. Interrupted (a) during the own cases and (b) during
/// the LAST own case (the stop is then honoured at the queue end, after every not-own case was returned: C3 with
/// returned cases), the resumed attempt solves exactly the job's OWN cases (never
/// a returned sub-case: counters.cases and the table equal the uninterrupted run's) and returns the same sub-cases
/// (F.subcases.jsonl canonically identical), the ones returned before the stop included (RESUME
/// `engine.descend_open.returned`).
#[test]
#[ignore]
fn descend_open_resume_solves_only_own_cases_and_returns_the_rest() {
    let r = root("descend-resume");
    let full = descend_spec(&r, "t281-desc-full");
    let (w, t) = run_to_end(&full);
    eprintln!("uninterrupted: {w:?} in {t:?}");
    assert_eq!(w, WaitStatus::Exited(10));
    let of = outcome(&full);
    assert_eq!(of.job.stop.code, Some(StopCode::MaxCases));
    assert_eq!(
        of.job.counters.cases, 4,
        "only the four own cases are searched"
    );
    let want = canonical(&full);
    let want_sub = canonical_of(&full, "F.subcases.jsonl").unwrap();
    let n_sub = subcase_lines(&full).len();
    assert!(n_sub > 0, "sub-cases returned");
    let started = |ev: &[serde_json::Value]| count_kind(ev, "case_started", |_| true);
    let mut returned_before_stop = 0usize;
    for (k, phase) in ["own", "returned"].into_iter().enumerate() {
        let a = descend_spec(&r, &format!("t281-desc-r{k}"));
        let mut c = spawn(&a);
        if phase == "own" {
            // during the second own case
            wait_events(&a, Duration::from_secs(300), "2nd own case", |ev| {
                started(ev) >= 2
            });
        } else {
            // during the LAST own case: the following not-own cases are returned (no work) even with the stop
            // pending, and the stop is honoured at the queue end (C3) with all of them in RESUME `returned`
            wait_events(&a, Duration::from_secs(300), "4th own case", |ev| {
                started(ev) >= 4
            });
            std::thread::sleep(Duration::from_millis(300));
        }
        signal(&c, libc::SIGTERM);
        let w = wait(&mut c);
        assert_eq!(
            w,
            WaitStatus::Exited(30),
            "{phase}: interrupted (never exit 0 / 10 with a pending stop)"
        );
        let cl = classified(&a, w, KillReason::Stop);
        assert_eq!(cl.row, 11);
        let res = resume_json(&a);
        let d = &res["engine"]["descend_open"];
        assert_eq!(
            d["own"].as_array().map(Vec::len),
            Some(4),
            "{phase}: own cases in RESUME: {res}"
        );
        let ret = d["returned"].as_array().map_or(0, Vec::len);
        if phase == "returned" {
            returned_before_stop = ret;
            assert!(res["current"].is_null(), "stopped at the queue end: {res}");
            assert!(res["pending"].as_array().unwrap().is_empty(), "{res}");
        }
        let b = resume_spec(&r, &a);
        let (w2, t2) = run_to_end(&b);
        eprintln!(
            "{phase}: RESUME returned {ret}, pending {}; resumed {t2:?} -> {w2:?}",
            res["pending"].as_array().map_or(0, Vec::len)
        );
        assert_eq!(w2, WaitStatus::Exited(10));
        let ob = outcome(&b);
        assert_eq!(
            ob.job.counters.cases, 4,
            "{phase}: no returned case was solved"
        );
        assert_eq!(canonical(&b), want, "{phase}: table");
        assert_eq!(
            canonical_of(&b, "F.subcases.jsonl").unwrap(),
            want_sub,
            "{phase}: F.subcases"
        );
        let clb = classified(&b, w2, KillReason::None);
        assert!(matches!(clb.row, 2 | 24), "row {}", clb.row);
    }
    assert!(
        returned_before_stop >= 1,
        "the returned-phase stop recorded returned cases in RESUME ({returned_before_stop})"
    );
}

/// audit-engjob C3 (REQUIRED TEST): `--descend open`, a stop requested during the LAST own case (T281-b6 rule 389's
/// case: about 3 s, no exceptional child, so no case boundary follows it) is honoured at the end of the queue:
/// exit 30 with a valid RESUME (it used to end with exit 0, the stop taken as handled); the resumed attempt ends
/// like the uninterrupted job (same table, same F.subcases).
#[test]
#[ignore]
fn descend_open_stop_during_the_last_own_case_is_honoured() {
    let r = root("descend-last");
    let lines = [fixture_case("T281-b6", 389)];
    let mk = |job: &str| {
        spec_with_cases(
            &r,
            job,
            1,
            "T281-b6",
            &["--order", "id", "--descend", "open"],
            &lines,
        )
    };
    let full = mk("t281-last-full");
    let (w, _) = run_to_end(&full);
    assert!(
        w == WaitStatus::Exited(0) || w == WaitStatus::Exited(10),
        "{w:?}"
    );
    let want = canonical(&full);
    let want_sub = canonical_of(&full, "F.subcases.jsonl");
    let a = mk("t281-last");
    let mut c = spawn(&a);
    wait_events(&a, Duration::from_secs(300), "own case started", |ev| {
        count_kind(ev, "case_started", |_| true) >= 1
    });
    std::thread::sleep(Duration::from_millis(500));
    signal(&c, libc::SIGTERM);
    let w1 = wait(&mut c);
    let ev = events_of(&a);
    let committed = count_kind(&ev, "rule_committed", |_| true);
    eprintln!("stop during the last own case: {w1:?}, rules committed {committed}");
    assert_eq!(w1, WaitStatus::Exited(30), "the stop is not ignored");
    let o = outcome(&a);
    assert_eq!(o.job.stop.code, Some(StopCode::Interrupted));
    assert!(o.job.stop.graceful);
    let cl = classified(&a, w1, KillReason::Stop);
    assert_eq!(cl.row, 11);
    assert!(cl.resume_from.is_some());
    let res = resume_json(&a);
    assert!(res["current"].is_null(), "stopped at the queue end: {res}");
    let b = resume_spec(&r, &a);
    let (w2, _) = run_to_end(&b);
    assert_eq!(w2, w, "resumed attempt ends as the uninterrupted job");
    assert_eq!(canonical(&b), want);
    assert_eq!(canonical_of(&b, "F.subcases.jsonl"), want_sub);
}

/// engbase open point: `--open-on-refusal` with resume. Rules above 400 coefficient monomials are refused
/// (`--max-rule-monomials 400 --fallback-rule-monomials 400 --no-refused-bank`: a deterministic refusal, no clock),
/// their cases become open obligations (RESUME `refused_open`); a run interrupted mid-way and resumed ends with the
/// table and F.subcases of the uninterrupted run (stop open_obligations, exit 10).
#[test]
#[ignore]
fn open_on_refusal_interrupted_and_resumed_equals_uninterrupted() {
    let r = root("refusal-resume");
    let args = [
        "--order",
        "id",
        "--open-on-refusal",
        "--max-rule-monomials",
        "400",
        "--fallback-rule-monomials",
        "400",
        "--no-refused-bank",
    ];
    let full = spec(&r, "t281-ref-full", 1, "T281-b6", &args);
    let (w, t) = run_to_end(&full);
    eprintln!("uninterrupted: {w:?} in {t:?}");
    assert_eq!(w, WaitStatus::Exited(10));
    let of = outcome(&full);
    assert_eq!(of.job.stop.code, Some(StopCode::OpenObligations));
    let refused = subcase_lines(&full)
        .iter()
        .filter(|v| v["kind"] == "refused")
        .count();
    assert!(refused >= 1, "refused cases listed");
    let want = canonical(&full);
    let want_sub = canonical_of(&full, "F.subcases.jsonl").unwrap();
    let a = spec(&r, "t281-ref-r", 1, "T281-b6", &args);
    let mut c = spawn(&a);
    // after the second refused case (refusals start late in this node): the first is then in RESUME refused_open
    wait_events(&a, Duration::from_secs(900), "two refused cases", |ev| {
        count_kind(ev, "case_finished", |v| v["result"] == "refused") >= 2
    });
    signal(&c, libc::SIGTERM);
    let w1 = wait(&mut c);
    assert_eq!(w1, WaitStatus::Exited(30));
    let res = resume_json(&a);
    let open_before = res["refused_open"].as_array().map_or(0, Vec::len);
    let b = resume_spec(&r, &a);
    let (w2, _) = run_to_end(&b);
    eprintln!("refused open in RESUME: {open_before} of {refused}; resumed -> {w2:?}");
    assert_eq!(w2, WaitStatus::Exited(10));
    assert_eq!(canonical(&b), want);
    assert_eq!(canonical_of(&b, "F.subcases.jsonl").unwrap(), want_sub);
    assert!(
        open_before >= 1,
        "the interruption came after a refusal (RESUME refused_open non-empty)"
    );
}

/// audit-engjob M5: a clock decision is recorded. `--native-try 0` makes every native guard try time out at once,
/// the content route takes over (the table is complete), and OUTCOME says `time_decided` with one `guard_route`
/// decision per guarded rule; the contract's OUTCOME check passes (row 1, flag carried).
#[test]
#[ignore]
fn clock_decisions_are_recorded_as_time_decisions() {
    let r = root("timedec");
    let s = spec(
        &r,
        "l4a-native0",
        1,
        "L4a",
        &["--order", "id", "--native-try", "0"],
    );
    let (w, _) = run_to_end(&s);
    assert_eq!(w, WaitStatus::Exited(0));
    let cl = classified(&s, w, KillReason::None);
    assert_eq!(cl.row, 1);
    assert!(cl.time_decided);
    let o = outcome(&s);
    assert!(o.job.time_decided);
    let n = o
        .job
        .time_decisions
        .iter()
        .filter(|d| d.what == rustred_campaign_contract::outcome::TimeDecisionWhat::GuardRoute)
        .count();
    eprintln!(
        "{} time decisions, {n} guard_route",
        o.job.time_decisions.len()
    );
    assert!(n >= 1);
    let ev = events_of(&s);
    assert_eq!(
        count_kind(&ev, "time_decision", |_| true),
        o.job.time_decisions.len()
    );
    // the default run of the same node: no clock decided anything
    let d = spec(&r, "l4a-default", 1, "L4a", &["--order", "id"]);
    let (w, _) = run_to_end(&d);
    assert_eq!(w, WaitStatus::Exited(0));
    let od = outcome(&d);
    eprintln!("default run: time decisions {:?}", od.job.time_decisions);
    assert!(!od.job.time_decided);
}

/// audit-engjob m6: `rule_committed` events carry the rule's RHS term count, trace rows and monomials, every rule
/// of the sealed table has exactly one (also those written after the last safe point), and the job counters
/// `rhs_terms` / `lift_monomials` are their sums.
#[test]
#[ignore]
fn rule_committed_events_cover_every_rule_with_sizes() {
    let r = root("rulecommitted");
    let s = numeric_spec(&r, "l3a-num-events");
    let (w, _) = run_to_end(&s);
    assert_eq!(w, WaitStatus::Exited(0));
    let o = outcome(&s);
    let rules = o.job.outputs.table.as_ref().unwrap().rules.unwrap();
    let ev = events_of(&s);
    let rc: Vec<&serde_json::Value> = ev
        .iter()
        .filter(|v| v["kind"] == "rule_committed")
        .collect();
    assert_eq!(rc.len() as u64, rules, "one rule_committed per rule");
    let idx: Vec<u64> = rc.iter().map(|v| v["index"].as_u64().unwrap()).collect();
    assert_eq!(idx, (0..rules).collect::<Vec<_>>());
    let rhs: u64 = rc.iter().map(|v| v["rhs"].as_u64().unwrap()).sum();
    assert!(rc.iter().all(|v| v["rhs"].as_u64().unwrap() > 0));
    assert_eq!(o.job.counters.rhs_terms, rhs);
    assert_eq!(o.legacy["rhs_terms"].as_u64().unwrap(), rhs);
    let mono: u64 = rc.iter().map(|v| v["monomials"].as_u64().unwrap()).sum();
    assert_eq!(o.job.counters.lift_monomials, mono);
}

/// audit-engjob M1: a not-lower rule is never in a sealed table that an accepting row can reach. No small fixture
/// produces a not-lower rule, so the test injects one through the path a resumed attempt takes: a valid RESUME of
/// an interrupted numeric-stage attempt gets `engine.not_lower` (and a new digest in the resume reference). The
/// resumed attempt completes its work, yet ends with stop `not_lower`, exit 10, decision row 6 (crashed,
/// attention, NOTHING accepted), and its end record lists the key; without the key it ends complete (row 1).
#[test]
#[ignore]
fn not_lower_rule_maps_to_row_6_before_any_other_end() {
    let r = root("notlower");
    let a = numeric_spec(&r, "l3a-notlower");
    let mut c = spawn(&a);
    wait_events(&a, Duration::from_secs(120), "numeric start", |ev| {
        count_kind(ev, "case_started", |v| v["numerical"] == 1) >= 2
    });
    signal(&c, libc::SIGTERM);
    assert_eq!(wait(&mut c), WaitStatus::Exited(30));
    // control: the untouched RESUME completes (row 1)
    let ok = resume_spec(&r, &a);
    let (w, _) = run_to_end(&ok);
    assert_eq!(w, WaitStatus::Exited(0));
    assert_eq!(classified(&ok, w, KillReason::None).row, 1);
    // the same RESUME with a not-lower key (rewritten in place after the control used it; new digest)
    let layout = RunLayout::new(&a.run_dir);
    let mut res = resume_json(&a);
    res["engine"]["not_lower"] = serde_json::json!(["I(1,1,1,1,1,1,1,1,0)"]);
    let injected = layout.resume();
    std::fs::write(&injected, serde_json::to_vec(&res).unwrap()).unwrap();
    let mut b = resume_spec(&r, &a);
    b.attempt = 3;
    b.run_dir = rustred_campaign_contract::layout::attempt_dir(&r, &a.job_id, 3)
        .to_str()
        .unwrap()
        .into();
    std::fs::create_dir_all(&b.run_dir).unwrap();
    b.resume = Some(rustred_campaign_contract::resume::ResumeRef {
        from: injected.to_str().unwrap().into(),
        blake3: b3(&injected),
    });
    let (w, _) = run_to_end(&b);
    let o = outcome(&b);
    eprintln!("injected not_lower: {w:?} stop {:?}", o.job.stop.code);
    assert_eq!(w, WaitStatus::Exited(10));
    assert_eq!(o.job.stop.code, Some(StopCode::NotLower));
    assert!(o.job.error.is_none());
    let cl = classified(&b, w, KillReason::None);
    assert_eq!(cl.row, 6, "not_lower: attention, nothing accepted");
    assert_eq!(
        cl.accept,
        rustred_campaign_contract::classify::Accept::Nothing
    );
    let lines = table_lines(&b);
    let end = lines.last().unwrap();
    assert_eq!(end["type"], "partial");
    assert_eq!(end["native_descent_not_lower"][0], "I(1,1,1,1,1,1,1,1,0)");
}

/// audit-engjob M2: with `--strict-numeric-search`, a stop honoured after an exhausted endpoint (SIGTERM during the
/// master's search; the next safe point is the endpoint after it) is `interrupted` (exit 30, row 11: nothing accepted,
/// resume), never overwritten by the strict refusal `numeric_search_refused` (exit 10, row 4: committed rules
/// accepted from an interrupted attempt). The uninterrupted strict job ends with that refusal.
#[test]
#[ignore]
fn strict_numeric_search_stop_is_interrupted_not_refused() {
    let r = root("strict");
    let lines: Vec<serde_json::Value> = L3A_POINTS
        .iter()
        .map(|p| serde_json::json!({"frame": "ORIGINAL", "fixed": p, "equations": []}))
        .collect();
    let args = [
        "--order",
        "id",
        "--numeric-depth",
        "0",
        "--numeric-depth-max",
        "3",
        "--strict-numeric-search",
    ];
    let full = spec_with_cases(&r, "l3a-strict-full", 1, "L3a", &args, &lines);
    let (w, _) = run_to_end(&full);
    assert_eq!(w, WaitStatus::Exited(10));
    assert_eq!(
        outcome(&full).job.stop.code,
        Some(StopCode::NumericSearchRefused)
    );
    let a = spec_with_cases(&r, "l3a-strict", 1, "L3a", &args, &lines);
    let mut c = spawn(&a);
    // the master is the 7th endpoint and takes about 0.5 s
    wait_events(&a, Duration::from_secs(120), "7th endpoint", |ev| {
        count_kind(ev, "case_started", |v| v["numerical"] == 1) >= 7
    });
    std::thread::sleep(Duration::from_millis(100));
    signal(&c, libc::SIGTERM);
    let w = wait(&mut c);
    let o = outcome(&a);
    eprintln!(
        "strict + stop after the exhausted master: {w:?} {:?}",
        o.job.stop.code
    );
    assert_eq!(w, WaitStatus::Exited(30));
    assert_eq!(o.job.stop.code, Some(StopCode::Interrupted));
    assert_eq!(classified(&a, w, KillReason::Stop).row, 11);
    let res = resume_json(&a);
    assert_eq!(
        res["engine"]["numeric_stage"]["exhausted"]
            .as_array()
            .map(Vec::len),
        Some(1),
        "{res}"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// job kind solve-recipe through the ONE job entry (lane engbase2). Test data: RR_RECIPE_FIXTURES (default
// /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/fixtures/recipe-solve: a family, recipes, key lists,
// an order and the hand reference results of each recipe; see its README.md and MANIFEST.sha256).

fn recipe_fixtures() -> PathBuf {
    let p = std::env::var("RR_RECIPE_FIXTURES").map(PathBuf::from).unwrap_or_else(|_| {
        PathBuf::from("/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/fixtures/recipe-solve")
    });
    assert!(p.join("order.json").is_file(), "{}", p.display());
    p
}

/// The key lines of a recipe's key list.
fn recipe_keys(rid: &str) -> Vec<serde_json::Value> {
    std::fs::read_to_string(recipe_fixtures().join("keys").join(format!("{rid}.jsonl")))
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// A solve-recipe job spec: the fixture family, recipe `rid`, `keys` written to RUN/in/keys.jsonl (input path relative
/// to the run directory), args `--order <fixture order> --threads 1` + `extra`.
fn recipe_spec(
    root: &Path,
    job: &str,
    rid: &str,
    keys: &[serde_json::Value],
    extra: &[&str],
) -> JobSpec {
    recipe_spec_rule(root, job, rid, keys, extra, None)
}

/// [`recipe_spec`] with the recipe's v1.2 `relation_rule` set to `rule` (a copy written to RUN/in/recipe.json).
fn recipe_spec_rule(
    root: &Path,
    job: &str,
    rid: &str,
    keys: &[serde_json::Value],
    extra: &[&str],
    rule: Option<&str>,
) -> JobSpec {
    let run = rustred_campaign_contract::layout::attempt_dir(root, job, 1);
    std::fs::create_dir_all(run.join("in")).unwrap();
    let fx = recipe_fixtures();
    let fam = fx.join("T178.sym");
    let mut rec = fx.join("recipes").join(format!("{rid}.json"));
    if let Some(rule) = rule {
        let mut r: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&rec).unwrap()).unwrap();
        r["relation_rule"] = serde_json::json!(rule);
        rec = run.join("in/recipe.json");
        std::fs::write(&rec, serde_json::to_string(&r).unwrap()).unwrap();
    }
    let kp = run.join("in/keys.jsonl");
    let text: String = keys.iter().map(|k| format!("{k}\n")).collect();
    std::fs::write(&kp, text).unwrap();
    let order = std::fs::read_to_string(fx.join("order.json")).unwrap();
    let mut args: Vec<String> = vec!["--order".into(), order, "--threads".into(), "1".into()];
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

/// (key, verdict, relation digest) of every relation record of a results file.
fn relations_of(p: &Path) -> Vec<(serde_json::Value, String, Option<String>)> {
    std::fs::read_to_string(p)
        .unwrap()
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["type"] == "relation")
        .map(|v| {
            (
                v["key"].clone(),
                v["verdict"].as_str().unwrap_or("").to_string(),
                v["relation_digest"].as_str().map(str::to_string),
            )
        })
        .collect()
}

/// (key, verdict, rhs as [key, coefficient] terms) of every relation record: relations compared across digest
/// versions (v1 records have no top/sub mark; v2 digests include the relation rule).
fn rhs_of(p: &Path) -> Vec<(serde_json::Value, String, serde_json::Value)> {
    std::fs::read_to_string(p)
        .unwrap()
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["type"] == "relation")
        .map(|v| {
            (
                v["key"].clone(),
                v["verdict"].as_str().unwrap_or("").to_string(),
                v["rhs"]
                    .as_array()
                    .map(|t| {
                        t.iter()
                            .map(|x| serde_json::json!([x[0], x[1]]))
                            .collect::<Vec<_>>()
                    })
                    .into(),
            )
        })
        .collect()
}

/// A member of every relation record.
fn member_of(p: &Path, m: &str) -> Vec<serde_json::Value> {
    std::fs::read_to_string(p)
        .unwrap()
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["type"] == "relation")
        .map(|v| v[m].clone())
        .collect()
}

fn results_canonical(spec: &JobSpec) -> String {
    outcome(spec).job.outputs.extra[0]
        .canonical_blake3
        .clone()
        .unwrap()
}

/// v1.2: `relation_rule: first_admissible` gives the v1 relation: the pass a1 hand reference (v1 digests are not
/// comparable with v2, so the relations are compared by their right-hand sides), recorded in the records and OUTCOME;
/// the alias `solve-recipe --job` is the same dispatcher (canonically identical results).
#[test]
#[ignore]
fn solve_recipe_job_first_admissible_matches_the_v1_reference_and_alias() {
    let r = root("recipe-ref");
    let fx = recipe_fixtures();
    let mut canon_job = None;
    for (n, rid) in ["results_47_fix10m1_n10_M1_json", "results_13_x8_C_json"]
        .iter()
        .enumerate()
    {
        let keys = recipe_keys(rid);
        let s = recipe_spec_rule(
            &r,
            &format!("sr{n}"),
            rid,
            &keys,
            &[],
            Some("first_admissible"),
        );
        let (w, t) = run_to_end(&s);
        let c = classified(&s, w, KillReason::None);
        assert_eq!(w, WaitStatus::Exited(0), "{rid}: {}", c.why);
        assert_eq!(c.row, 1, "{rid}: {}", c.why);
        let o = outcome(&s);
        assert_eq!(o.job.kind, JobKind::SolveRecipe);
        assert_eq!(o.job.stop.code, None);
        assert_eq!(o.job.counters.rules_committed, keys.len() as u64);
        assert!(!o.job.time_decided);
        assert_eq!(
            o.legacy["result"]["schema"],
            "rustred.solve-recipe.outcome.v0"
        );
        assert_eq!(o.legacy["relation_rule"], "first_admissible");
        assert_eq!(
            o.job.effective.as_ref().unwrap().node_options["relation_rule"],
            "first_admissible"
        );
        let res = RunLayout::new(&s.run_dir).results();
        let reference = fx.join("reference").join(format!("{rid}.results.jsonl"));
        assert_eq!(
            rhs_of(&res),
            rhs_of(&reference),
            "{rid}: relations differ from the v1 hand reference"
        );
        for m in ["relation_rule", "relation_rule_taken"] {
            assert!(
                member_of(&res, m).iter().all(|v| v == "first_admissible"),
                "{rid}: {m}"
            );
        }
        let ev = events_of(&s);
        assert_eq!(
            count_kind(&ev, "engine", |_| true),
            keys.len(),
            "{rid}: one RecipeKey event per key"
        );
        assert_eq!(ev.last().unwrap()["kind"], "job_finished");
        eprintln!(
            "{rid}: {} keys SOLVED first_admissible, rhs = v1 reference, {t:?}",
            keys.len()
        );
        if n == 0 {
            canon_job = Some(results_canonical(&s));
        }
    }
    // the engine line's spelling `solve-recipe --job` is the same dispatcher: same classification and results
    let rid = "results_47_fix10m1_n10_M1_json";
    let s = recipe_spec_rule(
        &r,
        "sr-alias",
        rid,
        &recipe_keys(rid),
        &[],
        Some("first_admissible"),
    );
    let mut ch = spawn_with(&s, &["solve-recipe", "--job"]);
    let w = wait(&mut ch);
    assert_eq!(classified(&s, w, KillReason::None).row, 1);
    assert_eq!(
        Some(results_canonical(&s)),
        canon_job,
        "alias run canonically identical to the job run"
    );
}

/// v1.2 default `relation_rule: m1_first` against the coordinator's M1-first reference (frozen engine-line4, same keys):
/// verdicts, relation digests, taken rule and m1 steps equal; the rule recorded in OUTCOME. g026 has symmetry rows (the
/// trace-row identity fix ef617e98).
#[test]
#[ignore]
fn solve_recipe_job_m1_first_matches_the_coordinator_reference() {
    let r = root("recipe-m1");
    let fx = recipe_fixtures();
    for (n, rid) in ["g026_d2_ccomb_fon_sfirst", "g038_d2_cL1_fon_sfirst"]
        .iter()
        .enumerate()
    {
        let keys = recipe_keys(rid);
        let s = recipe_spec(&r, &format!("m1-{n}"), rid, &keys, &[]);
        let (w, t) = run_to_end(&s);
        let c = classified(&s, w, KillReason::None);
        assert_eq!((w, c.row), (WaitStatus::Exited(0), 1), "{rid}: {}", c.why);
        let o = outcome(&s);
        assert_eq!(o.legacy["relation_rule"], "m1_first");
        assert_eq!(
            o.job.effective.as_ref().unwrap().node_options["relation_rule"],
            "m1_first"
        );
        let res = RunLayout::new(&s.run_dir).results();
        let reference = fx.join("reference").join(format!("{rid}.results.jsonl"));
        assert_eq!(
            relations_of(&res),
            relations_of(&reference),
            "{rid}: digests differ from the M1-first reference"
        );
        for m in ["relation_rule", "relation_rule_taken", "m1_steps"] {
            assert_eq!(member_of(&res, m), member_of(&reference, m), "{rid}: {m}");
        }
        eprintln!(
            "{rid}: {} keys, digests = M1-first reference, taken {:?}, {t:?}",
            keys.len(),
            member_of(&res, "relation_rule_taken")
        );
    }
}

#[test]
#[ignore]
fn solve_recipe_job_with_an_off_domain_key_is_row_28() {
    // first_admissible: the exit paths, not the relation rule, are under test (m1_first costs ~2 min per key here)
    let r = root("recipe-28");
    let rid = "results_47_fix10m1_n10_M1_json";
    let mut keys = recipe_keys(rid);
    keys.truncate(2);
    // the recipe's domain fixes slot 10 at -1: the same key with -2 there is outside it (typed NOT_IN_DOMAIN)
    let mut off = keys[1].clone();
    off["key"][10] = serde_json::json!(-2);
    off["tag"] = serde_json::json!("off-domain");
    keys.push(off);
    let s = recipe_spec_rule(&r, "sr28", rid, &keys, &[], Some("first_admissible"));
    let (w, _) = run_to_end(&s);
    let c = classified(&s, w, KillReason::None);
    assert_eq!(w, WaitStatus::Exited(10), "{}", c.why);
    assert_eq!(c.row, 28, "{}", c.why);
    let o = outcome(&s);
    assert_eq!(o.job.stop.code, Some(StopCode::KeysUnsolved));
    assert_eq!(o.job.counters.rules_committed, 2);
    let rel = relations_of(&RunLayout::new(&s.run_dir).results());
    assert_eq!(rel[2].1, "NOT_IN_DOMAIN");
    // only the off-domain key: row 29 (no accepted relation)
    let s = recipe_spec_rule(&r, "sr29", rid, &keys[2..], &[], Some("first_admissible"));
    let (w, _) = run_to_end(&s);
    let c = classified(&s, w, KillReason::None);
    assert_eq!((w, c.row), (WaitStatus::Exited(10), 29), "{}", c.why);
}

/// Wait until the job's EVENTS hold `n` RecipeKey engine events.
fn wait_recipe_keys(spec: &JobSpec, n: usize, limit: Duration) {
    let t = Instant::now();
    while count_kind(&events_of(spec), "engine", |_| true) < n {
        assert!(t.elapsed() < limit, "no {n} solved keys within {limit:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
#[ignore]
fn solve_recipe_job_signals_graceful_then_hard() {
    let r = root("recipe-sig");
    let rid = "results_47_fix10m1_n10_M1_json";
    let keys = recipe_keys(rid);
    // first signal: running key finishes, the rest INTERRUPTED, sealed partial, exit 30 (row 11)
    let s = recipe_spec_rule(&r, "srsig", rid, &keys, &[], Some("first_admissible"));
    let mut ch = spawn(&s);
    wait_recipe_keys(&s, 1, Duration::from_secs(600));
    signal(&ch, libc::SIGTERM);
    let w = wait(&mut ch);
    let c = classified(&s, w, KillReason::None);
    assert_eq!((w, c.row), (WaitStatus::Exited(30), 11), "{}", c.why);
    let o = outcome(&s);
    assert_eq!(o.job.stop.code, Some(StopCode::Interrupted));
    assert!(o.job.stop.graceful);
    assert_eq!(o.job.outputs.extra.len(), 1);
    let rel = relations_of(&RunLayout::new(&s.run_dir).results());
    assert!(rel.iter().any(|x| x.1 == "INTERRUPTED"), "{rel:?}");
    assert!(rel.iter().any(|x| x.1 == "SOLVED"), "{rel:?}");
    // two distinct signals: the hard-safe stop (no sealed output listed), exit 30
    let s = recipe_spec_rule(&r, "srhard", rid, &keys, &[], Some("first_admissible"));
    let mut ch = spawn(&s);
    wait_recipe_keys(&s, 1, Duration::from_secs(600));
    signal(&ch, libc::SIGTERM);
    signal(&ch, libc::SIGINT);
    let w = wait(&mut ch);
    let c = classified(&s, w, KillReason::None);
    assert_eq!((w, c.row), (WaitStatus::Exited(30), 11), "{}", c.why);
    let o = outcome(&s);
    assert!(!o.job.stop.graceful);
    assert!(o.job.outputs.extra.is_empty());
}

#[test]
#[ignore]
fn solve_recipe_job_strict_spec_checks_refuse() {
    let r = root("recipe-refuse");
    let rid = "results_13_x8_C_json";
    let keys = recipe_keys(rid);
    // an arg outside the kind's allow-list (the verb's --no-proof): usage, 40
    let s = recipe_spec(&r, "srno", rid, &keys, &["--no-proof"]);
    let (w, _) = run_to_end(&s);
    assert_eq!(w, WaitStatus::Exited(40));
    assert_eq!(classified(&s, w, KillReason::None).row, 12);
    // a job wall budget (an L1 kind is bounded by the supervisor's cap): 40
    let mut s = recipe_spec(&r, "srwall", rid, &keys, &[]);
    s.budgets.wall.job_s = Some(60);
    let (w, _) = run_to_end(&s);
    assert_eq!(w, WaitStatus::Exited(40));
    // a keys input changed after the spec was written: input, 41
    let s = recipe_spec(&r, "srdig", rid, &keys, &[]);
    std::fs::write(Path::new(&s.run_dir).join("in/keys.jsonl"), "[1]\n").unwrap();
    let (w, _) = run_to_end(&s);
    assert_eq!(w, WaitStatus::Exited(41));
    assert_eq!(classified(&s, w, KillReason::None).row, 13);
}
