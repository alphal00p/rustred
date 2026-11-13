#![allow(dead_code)]
//! Shared harness of the `fake_child_*` tests: run `rr_fake_child` as a real process in its own session (as the
//! supervisor does, contract 3.4.6), in a temp dir under the system temp dir, classify its files with the contract's
//! decision table, and make sure no process is left behind.

use rustred_campaign_contract::classify::{
    Classification, KillReason, WaitStatus, assess, classify,
};
use rustred_campaign_contract::events::{Event, EventLine};
use rustred_campaign_contract::fake::*;
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::outcome::Outcome;
use rustred_campaign_contract::resume::{ResumeRef, ResumeState};
use rustred_campaign_contract::seal::file_blake3;
use rustred_campaign_contract::spec::*;
use rustred_campaign_contract::stop::CaseResult;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

pub const BIN: &str = env!("CARGO_BIN_EXE_rr_fake_child");
pub const D0: &str = "0000000000000000000000000000000000000000000000000000000000000000";
static N: AtomicU32 = AtomicU32::new(0);

/// Root temp dir of one test (removed on drop, after every process of the test is gone).
pub struct Root {
    pub path: PathBuf,
    pgids: Vec<i32>,
    extra_pids: Vec<i32>,
}

impl Root {
    pub fn new(tag: &str) -> Root {
        let path = std::env::temp_dir().join(format!(
            "rr-fake-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Root {
            path,
            pgids: vec![],
            extra_pids: vec![],
        }
    }

    /// A run directory `jobs/<job>/a<attempt>`.
    pub fn run_dir(&self, job: &str, attempt: u32) -> PathBuf {
        let d = rustred_campaign_contract::layout::attempt_dir(&self.path, job, attempt);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Assert that no process of any group spawned by this test, and none of the recorded helper pids, is alive.
    pub fn assert_no_process_left(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let live = self.live();
            if live.is_empty() {
                return;
            }
            if Instant::now() > deadline {
                self.kill_all();
                panic!("processes left behind: {live:?}");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn live(&self) -> Vec<i32> {
        let mut v = Vec::new();
        for pid in all_pids() {
            if let Some((state, pgrp)) = stat_of(pid)
                && state != 'Z'
                && (self.pgids.contains(&pgrp) || self.extra_pids.contains(&pid))
            {
                v.push(pid);
            }
        }
        v
    }

    fn kill_all(&self) {
        for &g in &self.pgids {
            // SAFETY: our own process groups (we spawned their leaders with setsid).
            unsafe { libc::kill(-g, libc::SIGKILL) };
        }
        for &p in &self.extra_pids {
            if stat_of(p).is_some() {
                // SAFETY: pids recorded from our own children's events.
                unsafe { libc::kill(p, libc::SIGKILL) };
            }
        }
    }

    /// Register a helper pid (from the child's events) for the leftover check.
    pub fn track_pid(&mut self, pid: i32) {
        self.extra_pids.push(pid);
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        self.kill_all();
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

pub fn all_pids() -> Vec<i32> {
    std::fs::read_dir("/proc")
        .map(|d| {
            d.filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// (state, pgrp) of a pid from /proc/<pid>/stat.
pub fn stat_of(pid: i32) -> Option<(char, i32)> {
    let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after = &s[s.rfind(')')? + 2..];
    let mut it = after.split_whitespace();
    let state = it.next()?.chars().next()?;
    let _ppid = it.next()?;
    let pgrp = it.next()?.parse().ok()?;
    Some((state, pgrp))
}

pub fn alive(pid: i32) -> bool {
    matches!(stat_of(pid), Some((s, _)) if s != 'Z')
}

/// A spec of kind fake for `run_dir`.
pub fn spec(run_dir: &Path, job: &str, attempt: u32, script: FakeScript) -> JobSpec {
    JobSpec {
        schema: rustred_campaign_contract::CONTRACT.into(),
        job_id: job.into(),
        attempt,
        kind: JobKind::Fake,
        run_dir: run_dir.to_str().unwrap().into(),
        program: ProgramRef {
            name: "rr_fake_child".into(),
            blake3: D0.into(),
            features_required: vec!["fake".into()],
        },
        inputs: vec![],
        args: vec![],
        seed: SeedSpec { sample_seed: 7 },
        budgets: Budgets::default(),
        on_case_budget: OnCaseBudget::Stop,
        determinism: Determinism::Recorded,
        output: OutputOptions::default(),
        heartbeat_s: 1,
        stop_grace_s: 2,
        resume_every_s: 0,
        resume: None,
        env_allow: vec![],
        fake: Some(script),
    }
}

pub fn script(ops: Vec<FakeOp>) -> FakeScript {
    FakeScript {
        script: ops,
        on_sigterm: OnSigterm::Graceful,
        slow_ms: 0,
        start_delay_ms: 0,
        rss: RssPlan::default(),
        helpers: vec![],
        faults: vec![],
        time_scale: 1.0,
        volatile_jitter: false,
    }
}

/// A rule case.
pub fn rule_case(ordinal: u64, ms: u64) -> FakeOp {
    FakeOp::Case {
        ordinal,
        ms,
        work: [
            (WorkCounter::Seeds, 100 * (ordinal + 1)),
            (WorkCounter::Rows, 1000),
        ]
        .into_iter()
        .collect(),
        result: CaseResult::Rule,
        rule: Some(FakeRule {
            record: serde_json::json!({"chart":"11111","case":ordinal,"target":format!("I({ordinal},1,1)"),"rhs":[["I(0,1,1)","-1/2"],["I(1,0,1)",format!("{ordinal}")]]}),
            loci: None,
        }),
        children: vec![],
    }
}

pub fn covered_case(ordinal: u64, ms: u64) -> FakeOp {
    FakeOp::Case {
        ordinal,
        ms,
        work: Default::default(),
        result: CaseResult::Covered,
        rule: None,
        children: vec![],
    }
}

pub fn end(stop: Option<rustred_campaign_contract::StopCode>) -> FakeOp {
    FakeOp::End {
        stop,
        verdict: None,
    }
}

/// A spawned child.
pub struct Running {
    pub child: Child,
    pub pid: i32,
    pub layout: RunLayout,
    pub spec: JobSpec,
    pub started: Instant,
}

/// Write `job.json` (read-only) and spawn the child in its own session.
pub fn spawn(root: &mut Root, spec: &JobSpec) -> Running {
    let layout = RunLayout::new(&spec.run_dir);
    let p = layout.job_spec();
    std::fs::write(&p, spec.to_json_line()).unwrap();
    let mut perm = std::fs::metadata(&p).unwrap().permissions();
    perm.set_readonly(true);
    std::fs::set_permissions(&p, perm).unwrap();
    let out = std::fs::File::create(layout.stdout_log()).unwrap();
    let err = std::fs::File::create(layout.stderr_log()).unwrap();
    let mut cmd = Command::new(BIN);
    cmd.arg("job")
        .arg(&p)
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .env_remove("RUSTRED_PROBE_POINT");
    // SAFETY: setsid is async-signal-safe; it makes the child a session and group leader (contract 3.4.6).
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = cmd.spawn().unwrap();
    let pid = child.id() as i32;
    root.pgids.push(pid);
    Running {
        child,
        pid,
        layout,
        spec: spec.clone(),
        started: Instant::now(),
    }
}

impl Running {
    /// Wait for the end; on timeout SIGKILL the group and panic.
    pub fn wait(&mut self, timeout: Duration) -> WaitStatus {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(st) = self.child.try_wait().unwrap() {
                return match (st.code(), st.signal()) {
                    (Some(c), _) => WaitStatus::Exited(c),
                    (_, Some(s)) => WaitStatus::Signaled(s),
                    _ => WaitStatus::Unknown,
                };
            }
            if Instant::now() > deadline {
                self.signal_group(libc::SIGKILL);
                let _ = self.child.wait();
                panic!(
                    "child did not finish within {timeout:?}; stderr: {}",
                    self.stderr()
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn signal_group(&self, sig: i32) {
        // SAFETY: our own child's process group.
        unsafe { libc::kill(-self.pid, sig) };
    }

    pub fn signal_pid(&self, sig: i32) {
        // SAFETY: our own child.
        unsafe { libc::kill(self.pid, sig) };
    }

    pub fn stderr(&self) -> String {
        std::fs::read_to_string(self.layout.stderr_log()).unwrap_or_default()
    }

    pub fn events(&self) -> Vec<EventLine> {
        read_events(&self.layout)
    }

    /// Wait until an event matching `f` appears.
    pub fn wait_event(&self, timeout: Duration, f: impl Fn(&Event) -> bool) -> EventLine {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(e) = self.events().into_iter().find(|e| f(&e.event)) {
                return e;
            }
            if Instant::now() > deadline {
                panic!(
                    "event not seen within {timeout:?}; stderr: {}",
                    self.stderr()
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn classify(&self, wait: WaitStatus, kill: KillReason) -> Classification {
        classify(&assess(&self.layout, &self.spec, wait, kill, false))
    }

    pub fn outcome(&self) -> Outcome {
        Outcome::read(&self.layout.outcome()).unwrap().unwrap()
    }

    pub fn helper_pids(&self) -> Vec<(i32, bool)> {
        self.events()
            .iter()
            .filter_map(|e| match &e.event {
                Event::Engine { event } => {
                    let h = event.get("helper")?;
                    Some((h["pid"].as_i64()? as i32, h["registered"].as_bool()?))
                }
                _ => None,
            })
            .collect()
    }
}

/// Complete event lines (a torn last line is ignored, as readers must).
pub fn read_events(layout: &RunLayout) -> Vec<EventLine> {
    let b = std::fs::read(layout.events()).unwrap_or_default();
    let mut v = Vec::new();
    let mut it = b.split(|&c| c == b'\n').peekable();
    while let Some(l) = it.next() {
        if it.peek().is_none() {
            break; // the part after the last "\n"
        }
        v.push(
            EventLine::parse(l)
                .unwrap_or_else(|e| panic!("bad event line {}: {e}", String::from_utf8_lossy(l))),
        );
    }
    v
}

/// Spec of a resume attempt `attempt` from the run of `prev` (same script).
pub fn resume_spec(root: &Root, prev: &Running, attempt: u32) -> JobSpec {
    let r: ResumeState =
        serde_json::from_slice(&std::fs::read(prev.layout.resume()).unwrap()).unwrap();
    let (_, d) = file_blake3(&prev.layout.resume()).unwrap();
    let dir = root.run_dir(&prev.spec.job_id, attempt);
    let mut s = prev.spec.clone();
    s.attempt = attempt;
    s.run_dir = dir.to_str().unwrap().into();
    s.resume = Some(ResumeRef {
        from: prev.layout.resume().to_str().unwrap().into(),
        blake3: d,
        table: prev.layout.table().to_str().unwrap().into(),
        committed_bytes: r.table_committed_bytes,
        prefix_blake3: r.table_prefix_blake3,
    });
    if let Some(f) = s.fake.as_mut() {
        f.faults.clear();
        f.on_sigterm = OnSigterm::Graceful;
    }
    s
}

/// The canonical digest recorded in a run's OUTCOME (also recomputed from the file).
pub fn canonical_of(r: &Running) -> String {
    let o = r.outcome();
    let t = o.job.outputs.table.unwrap();
    let (d, _) =
        rustred_campaign_contract::canon::canonical_digest_file(&r.layout.table()).unwrap();
    assert_eq!(t.canonical_blake3.as_deref(), Some(d.as_str()));
    d
}

/// Run `spec` to the end (no interference) and return the run.
pub fn run_to_end(root: &mut Root, spec: &JobSpec, timeout: Duration) -> (Running, WaitStatus) {
    let mut r = spawn(root, spec);
    let w = r.wait(timeout);
    (r, w)
}
