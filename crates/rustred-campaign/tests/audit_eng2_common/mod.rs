//! Shared helpers of the lane audit-eng2 tests (real child `rr_node job`, classified with the contract crate).
#![allow(dead_code)]

use rustred_campaign_contract::classify::{
    Classification, KillReason, WaitStatus, assess, classify,
};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::outcome::Outcome;
use rustred_campaign_contract::resume::ResumeRef;
use rustred_campaign_contract::spec::*;
use serde_json::Value;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

static N: AtomicU32 = AtomicU32::new(0);

pub fn bin() -> PathBuf {
    PathBuf::from(std::env::var("RR_NODE_BIN").expect("RR_NODE_BIN"))
}

pub fn fixtures() -> PathBuf {
    PathBuf::from(std::env::var("RUSTRED_CAMPAIGN_FIXTURES").expect("RUSTRED_CAMPAIGN_FIXTURES"))
}

pub fn family(node: &str) -> PathBuf {
    let p = fixtures()
        .join("small/m2-slice")
        .join(node)
        .join("family.sym");
    assert!(p.is_file(), "{}", p.display());
    p
}

pub fn b3(p: &Path) -> String {
    rustred_campaign_contract::blake3_hex(&std::fs::read(p).unwrap())
}

pub fn env_u64(k: &str, d: u64) -> u64 {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

pub fn root(tag: &str) -> PathBuf {
    let base = std::env::var("RR_REAL_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    let p = base.join(format!(
        "ae2-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// xorshift64* (deterministic per seed; the timing makes each drill land where it lands anyway).
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
    pub fn chance(&mut self, p: f64) -> bool {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64 <= p
    }
}

/// A drill fixture: a generate job shape.
#[derive(Clone)]
pub struct Fx {
    pub tag: &'static str,
    pub node: &'static str,
    pub args: Vec<String>,
    pub cases: Vec<Value>,
    /// `resume_every_s` choices (0 = RESUME at every commit point; 30 = the contract default).
    pub resume_every: Vec<u32>,
    pub stop_grace_s: u32,
    /// Target units: `case_started` events (all, or numeric ones only).
    pub numeric_units: bool,
}

pub fn job_spec(root: &Path, job: &str, attempt: u32, fx: &Fx, resume_every_s: u32) -> JobSpec {
    let run = rustred_campaign_contract::layout::attempt_dir(root, job, attempt);
    std::fs::create_dir_all(&run).unwrap();
    let fam = family(fx.node);
    let mut inputs = vec![InputRef {
        role: "family".into(),
        path: fam.to_str().unwrap().into(),
        blake3: b3(&fam),
        upto: None,
    }];
    if !fx.cases.is_empty() {
        let input = root.join(format!("{}.cases.jsonl", fx.tag));
        if !input.exists() {
            let text: String = fx.cases.iter().map(|l| format!("{l}\n")).collect();
            std::fs::write(&input, text).unwrap();
        }
        inputs.push(InputRef {
            role: "cases".into(),
            path: input.to_str().unwrap().into(),
            blake3: b3(&input),
            upto: None,
        });
    }
    JobSpec {
        schema: rustred_campaign_contract::CONTRACT.into(),
        job_id: job.into(),
        attempt,
        kind: JobKind::Generate,
        run_dir: run.to_str().unwrap().into(),
        program: ProgramRef {
            name: "rr_node".into(),
            blake3: b3(&bin()),
            features_required: vec!["job".into(), "no_elision".into(), "resume".into()],
        },
        inputs,
        args: fx.args.clone(),
        seed: SeedSpec { sample_seed: 0 },
        budgets: Budgets::default(),
        on_case_budget: OnCaseBudget::Stop,
        determinism: Determinism::Recorded,
        output: OutputOptions::default(),
        heartbeat_s: 1,
        stop_grace_s: fx.stop_grace_s,
        resume_every_s,
        resume: None,
        env_allow: vec![],
        fake: None,
    }
}

/// The next attempt of `prev`: from the classifier's resume reference, or from scratch (None).
pub fn next_attempt(root: &Path, prev: &JobSpec, from: Option<ResumeRef>) -> JobSpec {
    let attempt = prev.attempt + 1;
    let run = rustred_campaign_contract::layout::attempt_dir(root, &prev.job_id, attempt);
    std::fs::create_dir_all(&run).unwrap();
    let mut s = prev.clone();
    s.attempt = attempt;
    s.run_dir = run.to_str().unwrap().into();
    s.resume = from;
    s
}

pub fn spawn(spec: &JobSpec) -> Child {
    let layout = RunLayout::new(&spec.run_dir);
    let p = layout.job_spec();
    std::fs::write(&p, spec.to_json_line()).unwrap();
    let mut perm = std::fs::metadata(&p).unwrap().permissions();
    perm.set_readonly(true);
    std::fs::set_permissions(&p, perm).unwrap();
    // diagnostics: RR_AUDIT_LOGDIR puts the child's stdout / stderr OUTSIDE the run directory (e.g. off a full tmpfs)
    let (out, err) = match std::env::var("RR_AUDIT_LOGDIR") {
        Ok(d) => {
            let tag = format!("{}.a{}", spec.job_id, spec.attempt);
            (
                std::fs::File::create(Path::new(&d).join(format!("{tag}.stdout"))).unwrap(),
                std::fs::File::create(Path::new(&d).join(format!("{tag}.stderr"))).unwrap(),
            )
        }
        Err(_) => (
            std::fs::File::create(layout.stdout_log()).unwrap(),
            std::fs::File::create(layout.stderr_log()).unwrap(),
        ),
    };
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

pub fn wait(c: &mut Child) -> WaitStatus {
    let s = c.wait().unwrap();
    match (s.code(), s.signal()) {
        (Some(c), _) => WaitStatus::Exited(c),
        (None, Some(sig)) => WaitStatus::Signaled(sig),
        _ => WaitStatus::Unknown,
    }
}

pub fn signal(c: &Child, sig: i32) {
    // SAFETY: our own child (not yet reaped), inside the test's private pid namespace (LIGHT_PIDNS=1).
    unsafe {
        libc::kill(c.id() as i32, sig);
    }
}

pub fn classified(spec: &JobSpec, w: WaitStatus, k: KillReason) -> Classification {
    let layout = RunLayout::new(&spec.run_dir);
    let digest = spec_blake3(&std::fs::read(layout.job_spec()).unwrap());
    classify(&assess(&layout, spec, &digest, w, k, false))
}

pub fn outcome(spec: &JobSpec) -> Option<Outcome> {
    Outcome::read(&RunLayout::new(&spec.run_dir).outcome())
        .ok()
        .and_then(|r| r.ok())
}

pub fn events_of(spec: &JobSpec) -> Vec<Value> {
    std::fs::read_to_string(RunLayout::new(&spec.run_dir).events())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect()
}

pub fn units(ev: &[Value], numeric: bool) -> usize {
    ev.iter()
        .filter(|v| v["kind"] == "case_started" && (!numeric || v["numerical"] == 1))
        .count()
}

pub fn canonical_of(spec: &JobSpec, name: &str) -> Option<String> {
    let p = RunLayout::new(&spec.run_dir).out_dir().join(name);
    p.exists().then(|| {
        rustred_campaign_contract::canon::canonical_digest_file(&p)
            .map(|x| x.0)
            .unwrap_or_else(|e| format!("UNCANONICAL {e}"))
    })
}

pub fn canonical_lines(bytes: &[u8]) -> Vec<Vec<u8>> {
    bytes
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .map(|l| rustred_campaign_contract::canon::canonical_line(l).unwrap_or_else(|_| l.to_vec()))
        .collect()
}
