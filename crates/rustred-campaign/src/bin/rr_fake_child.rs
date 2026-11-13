//! `rr_fake_child`: a child program that honours the child-job contract `rustred.child-job.v1` exactly as a real
//! child would (child-job.md section 4), following the SCRIPT of its job spec (`spec.fake`, type
//! `rustred_campaign_contract::fake::FakeScript`).
//!
//! Invocation: `rr_fake_child job RUN/job.json` and `rr_fake_child version --json`.
//!
//! Threads: the main thread runs the script and polls the stop flag at its safe points (every 50 ms inside a case);
//! one watchdog thread emits heartbeats, watches signals, the STOP file, wall budgets, the soft memory limit and the
//! stop grace, and performs the hard-safe stop (3.4.3). Canonical outputs depend only on the spec and the resume
//! state; volatile keys may jitter (`volatile_jitter`).

// Nested `if`s are kept on purpose where the inner condition has a side effect (`FINAL.swap`) or takes a lock.
#![allow(clippy::collapsible_if)]

use rustred_campaign_contract::atomic::{
    ENOSPC, create_reserve, exit_for_io, is_nospace, release_reserve,
};
use rustred_campaign_contract::events::{Event, EventSink, Phase};
use rustred_campaign_contract::fake::{
    FakeOp, FakeScript, FaultKind, HelperPlan, OnSigterm, fake_rule_record,
};
use rustred_campaign_contract::layout::{RESERVE_BYTES, RunLayout};
use rustred_campaign_contract::outcome::*;
use rustred_campaign_contract::preflight::{ProgramSelf, Refusal, preflight};
use rustred_campaign_contract::resume::{CaseSpec, Frame, ResumeState};
use rustred_campaign_contract::seal::{CommitPoint, RecordWriter, WriteError};
use rustred_campaign_contract::spec::{
    BudgetHit, Counters, JobKind, JobSpec, OnCaseBudget, Scope, WorkCounter,
};
use rustred_campaign_contract::stop::{CaseResult, StopCode, StopSource};
use rustred_campaign_contract::{CONTRACT, ExitCode, RESUME_SCHEMA, VersionInfo};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering::*};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

const PROGRAM: &str = "rr_fake_child";
const FEATURES: [&str; 1] = ["fake"];

static SIGNALS: AtomicU32 = AtomicU32::new(0);
static LAST_SIG: AtomicI32 = AtomicI32::new(0);
/// Set by whoever writes the final OUTCOME (main thread at the end, watchdog on a hard-safe stop, panic hook).
static FINAL: AtomicBool = AtomicBool::new(false);
static CTX: OnceLock<Arc<Ctx>> = OnceLock::new();
/// Memory held on purpose (alloc op, rss growth).
static HELD: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());

extern "C" fn on_signal(sig: libc::c_int) {
    // async-signal-safe: two atomic stores only
    LAST_SIG.store(sig, Relaxed);
    SIGNALS.fetch_add(1, Relaxed);
}

fn install_signals(on: OnSigterm) {
    let sigs = [libc::SIGTERM, libc::SIGINT, libc::SIGHUP];
    match on {
        OnSigterm::Die => {}
        OnSigterm::Ignore => {
            for s in sigs {
                // SAFETY: setting a signal disposition to SIG_IGN is always sound.
                unsafe { libc::signal(s, libc::SIG_IGN) };
            }
        }
        OnSigterm::Graceful | OnSigterm::Slow => {
            for s in sigs {
                // SAFETY: the handler only performs atomic operations (async-signal-safe); the sigaction struct is
                // fully initialised (zeroed, then the handler and an empty mask).
                unsafe {
                    let mut sa: libc::sigaction = std::mem::zeroed();
                    sa.sa_sigaction = on_signal as *const () as usize;
                    sa.sa_flags = libc::SA_RESTART;
                    libc::sigemptyset(&mut sa.sa_mask);
                    libc::sigaction(s, &sa, std::ptr::null_mut());
                }
            }
        }
    }
}

fn source_of(sig: i32) -> StopSource {
    match sig {
        libc::SIGINT => StopSource::Sigint,
        libc::SIGHUP => StopSource::Sighup,
        _ => StopSource::Sigterm,
    }
}

fn page_kb() -> u64 {
    // SAFETY: sysconf is always safe to call.
    let p = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if p > 0 { p as u64 / 1024 } else { 4 }
}

fn rss_kb_of(pid: i32) -> u64 {
    let p = if pid == 0 {
        "/proc/self/statm".to_string()
    } else {
        format!("/proc/{pid}/statm")
    };
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| {
            s.split_whitespace()
                .nth(1)
                .and_then(|x| x.parse::<u64>().ok())
        })
        .map_or(0, |pages| pages * page_kb())
}

fn vmhwm_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|x| x.parse().ok()))
        })
        .unwrap_or(0)
}

fn program_info() -> ProgramInfo {
    ProgramInfo {
        name: PROGRAM.into(),
        version: env!("CARGO_PKG_VERSION").into(),
        git_commit: rustred_campaign_contract::git_commit_from_build_env(option_env!(
            "RUSTRED_BUILD_GIT_COMMIT"
        )),
        features: FEATURES.iter().map(|s| s.to_string()).collect(),
        exe: None,
    }
}

/// Mutable job state shared with the watchdog.
struct State {
    counters: Counters,
    commit: CommitPoint,
    phase: Phase,
    case_ordinal: Option<u64>,
    case_started: Option<Instant>,
    current: Option<CaseSpec>,
    pending: Vec<CaseSpec>,
    per_case: Vec<PerCase>,
    time_decisions: Vec<TimeDecision>,
    elided: u64,
    hit: Option<BudgetHit>,
    resume_written_at: Option<Instant>,
    resume_valid: bool,
    queue_bytes: u64,
}

struct Ctx {
    spec: JobSpec,
    spec_blake3: String,
    script: FakeScript,
    layout: RunLayout,
    inputs: Vec<InputCheck>,
    sink: EventSink,
    st: Mutex<State>,
    started: Instant,
    started_utc: String,
    stop_requested: AtomicBool,
    stop_source: Mutex<Option<StopSource>>,
    stop_at: Mutex<Option<Instant>>,
    heartbeat_on: AtomicBool,
    helpers: Mutex<Vec<i32>>,
    resumed: bool,
    jitter_seed: AtomicU64,
}

impl Ctx {
    fn st(&self) -> std::sync::MutexGuard<'_, State> {
        self.st.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn jitter(&self) -> Option<(f64, f64)> {
        if !self.script.volatile_jitter {
            return None;
        }
        let x = self.jitter_seed.fetch_add(0x9e37_79b9_7f4a_7c15, Relaxed);
        Some((
            (x % 100_000) as f64 / 1000.0 + 0.001,
            ((x >> 20) % 100_000) as f64 / 1000.0 + 0.002,
        ))
    }

    fn request_stop(&self, src: StopSource) {
        if !self.stop_requested.swap(true, SeqCst) {
            *self.stop_source.lock().unwrap() = Some(src);
            *self.stop_at.lock().unwrap() = Some(Instant::now());
            let _ = self.sink.emit(Event::StopRequested {
                source: src,
                count: SIGNALS.load(Relaxed).max(1),
            });
        }
    }

    fn ms(&self, ms: u64) -> Duration {
        Duration::from_secs_f64(ms as f64 * self.script.time_scale / 1000.0)
    }

    fn kill_helpers(&self) -> u64 {
        let mut peak = 0;
        let pids: Vec<i32> =
            std::mem::take(&mut *self.helpers.lock().unwrap_or_else(|p| p.into_inner()));
        for pid in pids {
            // SAFETY: plain syscalls on our own registered children.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
                let mut status = 0;
                let mut ru: libc::rusage = std::mem::zeroed();
                libc::wait4(pid, &mut status, 0, &mut ru);
                peak = peak.max(ru.ru_maxrss.max(0) as u64);
            }
        }
        peak
    }

    /// The OUTCOME for the current state.
    fn outcome(
        &self,
        code: ExitCode,
        stop: StopInfo,
        table: TableOutput,
        error: Option<ErrorInfo>,
        helpers_peak: u64,
    ) -> Outcome {
        let st = self.st();
        let mut j = JobOutcome::for_spec(&self.spec, &self.spec_blake3, program_info(), code, stop);
        j.inputs = self.inputs.clone();
        j.counters = st.counters;
        j.counters.rules_committed = st.counters.rules_committed;
        j.budgets.hit = st.hit.clone();
        j.time_decisions = st.time_decisions.clone();
        j.time_decided = !j.time_decisions.is_empty() || j.stop.code == Some(StopCode::BudgetWall);
        j.per_case = st.per_case.clone();
        let mut t = table;
        t.witness_loci_elided = st.elided;
        j.outputs.table = Some(t);
        j.outputs.queue = Some(QueueOutput {
            bytes: st.queue_bytes,
        });
        j.outputs.events = Some(EventsOutput {
            last_seq: self.sink.next_seq(),
        });
        j.outputs.resume = Some(ResumeOutput {
            path: rustred_campaign_contract::layout::RESUME.into(),
            valid: st.resume_valid,
        });
        let wall = self.started.elapsed().as_secs_f64();
        j.timing = Timing {
            started_utc: Some(self.started_utc.clone()),
            finished_utc: Some(rustred_campaign_contract::time::utc_now()),
            wall_s: Some((wall * 1000.0).round() / 1000.0),
            cpu_s: None,
        };
        j.memory = Memory {
            vmhwm_kb: Some(vmhwm_kb()),
            rss_kb_at_end: Some(rss_kb_of(0)),
            helpers_peak_rss_kb: Some(helpers_peak),
        };
        j.error = error;
        j.resumed = self.resumed;
        let mut o = Outcome::new(j);
        o.legacy.insert(
            "complete".into(),
            serde_json::Value::from(code == ExitCode::Complete),
        );
        o.legacy.insert(
            "rules".into(),
            serde_json::Value::from(st.counters.rules_committed),
        );
        o
    }

    fn unsealed_table(&self) -> TableOutput {
        let st = self.st();
        TableOutput {
            path: RunLayout::out_rel(rustred_campaign_contract::layout::TABLE),
            rules_committed: st.counters.rules_committed,
            committed_bytes: st.commit.bytes,
            prefix_blake3: Some(st.commit.prefix_blake3.clone()),
            sealed: false,
            ..Default::default()
        }
    }

    /// The hard-safe stop (3.4.3): kill helpers, leave table/queue alone, OUTCOME from the job state, try the
    /// `job_finished` event for 1 s, `_exit`.
    fn hard_stop(&self, code: ExitCode, stop: StopCode, td: Option<TimeDecision>) -> ! {
        if FINAL.swap(true, SeqCst) {
            park_forever();
        }
        let peak = self.kill_helpers();
        if let Some(td) = td {
            self.st().time_decisions.push(td);
        }
        let src = *self.stop_source.lock().unwrap_or_else(|p| p.into_inner());
        let o = self.outcome(
            code,
            StopInfo {
                code: Some(stop),
                detail: serde_json::Value::Null,
                source: src,
                graceful: false,
            },
            self.unsealed_table(),
            None,
            peak,
        );
        let _ = o.write_atomic(&self.layout.outcome());
        self.sink.try_emit_for(
            Event::JobFinished {
                exit_code: code,
                status: o.job.status,
                stop_code: Some(stop),
            },
            Duration::from_secs(1),
        );
        // SAFETY: _exit ends the process without running destructors (the main thread may hold locks).
        unsafe { libc::_exit(code.code()) }
    }

    fn write_resume(&self, force: bool) {
        let mut st = self.st();
        let every = self.spec.resume_every_s as u64;
        let due = force
            || every == 0
            || st
                .resume_written_at
                .is_none_or(|t| t.elapsed() >= Duration::from_secs(every));
        if !due {
            return;
        }
        let r = ResumeState {
            schema: RESUME_SCHEMA.into(),
            job_id: self.spec.job_id.clone(),
            attempt: self.spec.attempt,
            chart: serde_json::Value::from("C"),
            frame: Frame::Presented,
            rules_committed: st.counters.rules_committed,
            table_committed_bytes: st.commit.bytes,
            table_prefix_blake3: st.commit.prefix_blake3.clone(),
            current: st.current.clone(),
            pending: st.pending.clone(),
            numerical: vec![],
            deferred: vec![],
            refused_open: vec![],
            counters_at_commit: st.counters,
            symbolic_cases: st.counters.cases,
            discarded: 0,
        };
        if rustred_campaign_contract::atomic::write_json_atomic(&self.layout.resume(), &r).is_ok() {
            st.resume_written_at = Some(Instant::now());
            st.resume_valid = true;
            let ev = Event::ResumeWritten {
                rules_committed: r.rules_committed,
                pending: r.pending.len() as u64,
                table_bytes: r.table_committed_bytes,
            };
            drop(st);
            let _ = self.sink.emit(ev);
        }
    }
}

fn park_forever() -> ! {
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}

fn case_spec(ordinal: u64) -> CaseSpec {
    CaseSpec {
        frame: Frame::Presented,
        fixed: vec![Some(ordinal as i64)],
        equations: vec![],
        seed_centre_offset: None,
        extra: Default::default(),
    }
}

/// Write an OUTCOME for a refused start (when allowed) and exit with its code.
fn refuse_exit(r: Refusal) -> ! {
    eprintln!("rr_fake_child: {}", r.message);
    if r.write_outcome {
        if let Some((spec, digest)) = &r.spec {
            let mut j = JobOutcome::for_spec(
                spec,
                digest,
                program_info(),
                r.code,
                StopInfo {
                    code: Some(r.stop),
                    detail: serde_json::Value::Null,
                    source: None,
                    graceful: true,
                },
            );
            j.error = Some(ErrorInfo::new(r.class, r.message.clone()));
            let _ = Outcome::new(j).write_atomic(&RunLayout::new(&spec.run_dir).outcome());
        }
    }
    std::process::exit(r.code.code())
}

/// What ends the main thread's script.
enum Finish {
    End {
        stop: Option<StopCode>,
        verdict: Option<ExitCode>,
    },
    /// Graceful stop at a safe point (stop requested).
    Stopped,
    /// A per-case work budget with `on_case_budget: stop`.
    Budget,
    Fail {
        code: ExitCode,
        message: String,
    },
    Io(io::Error),
}

struct Main {
    ctx: Arc<Ctx>,
    table: Option<RecordWriter>,
    queue: RecordWriter,
}

impl Main {
    /// A safe point: Some(Finish::Stopped) when a stop is to be honoured now.
    fn safe_point(&self) -> Option<Finish> {
        if !self.ctx.stop_requested.load(SeqCst) {
            return None;
        }
        if self.ctx.script.on_sigterm == OnSigterm::Slow {
            let at = (*self.ctx.stop_at.lock().unwrap()).unwrap_or_else(Instant::now);
            if at.elapsed() < self.ctx.ms(self.ctx.script.slow_ms) {
                return None;
            }
        }
        Some(Finish::Stopped)
    }

    /// Sleep `d` in 50 ms slices with safe points.
    fn wait(&self, d: Duration) -> Option<Finish> {
        let end = Instant::now() + d;
        loop {
            if let Some(f) = self.safe_point() {
                return Some(f);
            }
            let now = Instant::now();
            if now >= end {
                return None;
            }
            std::thread::sleep((end - now).min(Duration::from_millis(50)));
        }
    }

    fn phase(&self, p: Phase) {
        self.ctx.st().phase = p;
        let _ = self.ctx.sink.emit(Event::Phase { phase: p });
    }

    fn commit(&mut self) -> io::Result<()> {
        let cp = self.table.as_mut().expect("table").commit()?;
        let q = self.queue.commit()?;
        {
            let mut st = self.ctx.st();
            st.commit = cp;
            st.queue_bytes = q.bytes;
        }
        self.ctx.write_resume(false);
        Ok(())
    }

    fn run_case(
        &mut self,
        ordinal: u64,
        ms: u64,
        work: &std::collections::BTreeMap<WorkCounter, u64>,
        result: CaseResult,
        rule: &Option<rustred_campaign_contract::fake::FakeRule>,
        children: &[CaseSpec],
    ) -> Result<(), Finish> {
        let ctx = self.ctx.clone();
        let cs = case_spec(ordinal);
        {
            let mut st = ctx.st();
            st.case_ordinal = Some(ordinal);
            st.case_started = Some(Instant::now());
            st.current = Some(cs.clone());
        }
        self.phase(Phase::CaseSearch);
        let pending = ctx.st().pending.len() as u64;
        let _ = ctx.sink.emit(Event::CaseStarted {
            ordinal,
            case: serde_json::to_value(&cs).unwrap(),
            pending,
            numerical: 0,
        });
        let base = ctx.st().counters;
        let mut case_c = Counters::default();
        let dur = ctx.ms(ms);
        let t0 = Instant::now();
        let mut budget_open = false;
        loop {
            let frac = if dur.is_zero() {
                1.0
            } else {
                (t0.elapsed().as_secs_f64() / dur.as_secs_f64()).min(1.0)
            };
            // advance every counter to its linear target, clamped at a per-case limit
            let mut hit = None;
            for (&c, &total) in work {
                let target = (total as f64 * frac).floor() as u64;
                let target = if frac >= 1.0 { total } else { target };
                let lim = ctx
                    .spec
                    .budgets
                    .work
                    .limits()
                    .into_iter()
                    .find(|&(lc, s, _)| lc == c && s == Scope::Case);
                let (v, h) = match lim {
                    Some((_, _, l)) if target >= l => (l, Some(l)),
                    _ => (target, None),
                };
                let now = case_c.get(c);
                if v > now || c == WorkCounter::TraceRows {
                    if c == WorkCounter::TraceRows {
                        case_c.trace_rows_max = case_c.trace_rows_max.max(v);
                    } else {
                        case_c.add(c, v - now);
                    }
                }
                if let (Some(l), None) = (h, &hit) {
                    hit = Some(BudgetHit {
                        counter: c,
                        scope: Scope::Case,
                        limit: l,
                        value: l,
                        case: Some(serde_json::to_value(&cs).unwrap()),
                    });
                }
            }
            {
                let mut st = ctx.st();
                let mut c = base;
                for wc in WorkCounter::ALL {
                    if wc == WorkCounter::TraceRows {
                        c.trace_rows_max = c.trace_rows_max.max(case_c.trace_rows_max);
                    } else if wc != WorkCounter::Cases {
                        c.add(wc, case_c.get(wc));
                    }
                }
                st.counters = c;
            }
            // job-scope work limits (other than cases)
            if hit.is_none() {
                let job = ctx.st().counters;
                hit = ctx
                    .spec
                    .budgets
                    .work
                    .limits()
                    .into_iter()
                    .find(|&(c, s, l)| {
                        s == Scope::Job && c != WorkCounter::Cases && job.get(c) >= l
                    })
                    .map(|(c, s, l)| BudgetHit {
                        counter: c,
                        scope: s,
                        limit: l,
                        value: job.get(c),
                        case: None,
                    });
            }
            if let Some(h) = hit {
                let _ = ctx.sink.emit(Event::BudgetHit {
                    counter: h.counter,
                    scope: h.scope,
                    limit: h.limit,
                    value: h.value,
                });
                let open = ctx.spec.on_case_budget == OnCaseBudget::Open && h.scope == Scope::Case;
                ctx.st().hit = Some(h);
                if !open {
                    return Err(Finish::Budget);
                }
                budget_open = true;
                break;
            }
            if frac >= 1.0 {
                break;
            }
            if let Some(f) = self.safe_point() {
                return Err(f);
            }
            std::thread::sleep(
                Duration::from_millis(50).min(
                    dur.saturating_sub(t0.elapsed())
                        .max(Duration::from_millis(1)),
                ),
            );
        }
        let result = if budget_open {
            CaseResult::Budget
        } else {
            result
        };
        let mut rule_index = None;
        if let (CaseResult::Rule, Some(r)) = (result, rule) {
            let index = ctx.st().counters.rules_committed;
            let (line, elided) = fake_rule_record(r, index, &ctx.spec.output, ctx.jitter());
            let pos = self
                .table
                .as_mut()
                .expect("table")
                .append(&line)
                .map_err(wfin)?;
            {
                let mut st = ctx.st();
                st.elided += elided;
                st.counters.rules_committed += 1;
                st.counters.rhs_terms += r
                    .record
                    .get("rhs")
                    .and_then(|x| x.as_array())
                    .map_or(0, |a| a.len() as u64);
            }
            rule_index = Some(index);
            let _ = ctx.sink.emit(Event::RuleCommitted {
                index,
                line_offset: pos.offset,
                line_bytes: pos.bytes,
                canon_blake3: pos.canon_blake3,
                rhs: r
                    .record
                    .get("rhs")
                    .and_then(|x| x.as_array())
                    .map_or(0, |a| a.len() as u64),
                trace_rows: case_c.trace_rows_max,
                monomials: case_c.lift_monomials,
            });
        }
        let qrec = serde_json::json!({"type":"case","ordinal":ordinal,"result":result,"rule":rule_index,"children":children});
        self.queue
            .append(&serde_json::to_vec(&qrec).unwrap())
            .map_err(wfin)?;
        {
            let mut st = ctx.st();
            st.counters.cases += 1;
            st.pending.extend(children.iter().cloned());
            st.per_case.push(PerCase {
                ordinal,
                case: serde_json::to_value(&cs).unwrap(),
                result,
                rule: rule_index,
                children_open: children.len() as u64,
            });
            st.current = None;
            st.case_ordinal = None;
            st.case_started = None;
        }
        self.commit().map_err(Finish::Io)?;
        let rc = ctx.st().counters.rules_committed;
        let _ = ctx.sink.emit(Event::CaseFinished {
            ordinal,
            result,
            rule: rule_index,
            children: children.len() as u64,
            rules_committed: rc,
        });
        Ok(())
    }

    fn fault(
        &mut self,
        kind: FaultKind,
        f: &rustred_campaign_contract::fake::Fault,
    ) -> Option<Finish> {
        let ctx = self.ctx.clone();
        match kind {
            FaultKind::SigkillSelf => sigkill_self(),
            FaultKind::Abort => std::process::abort(),
            FaultKind::Panic => panic!("rr_fake_child: scripted panic"),
            FaultKind::TornWrite => {
                let index = ctx.st().counters.rules_committed;
                let line = format!(
                    "{{\"type\":\"rule\",\"index\":{index},\"target\":\"I(torn)\",\"rhs\":[]}}"
                );
                let n = (f.bytes as usize).min(line.len().saturating_sub(1)).max(1);
                let file = self.table.as_mut().expect("table").raw_file();
                let _ = file.write_all(&line.as_bytes()[..n]);
                let _ = file.sync_data();
                sigkill_self()
            }
            FaultKind::Enospc => {
                let e = match &f.path {
                    Some(p) => std::fs::OpenOptions::new()
                        .write(true)
                        .open(p)
                        .and_then(|mut h| h.write_all(&[0u8; 65536]).and_then(|_| h.flush()))
                        .err()
                        .unwrap_or_else(|| io::Error::other("write to the fault path succeeded")),
                    None => io::Error::from_raw_os_error(ENOSPC),
                };
                Some(Finish::Io(e))
            }
            FaultKind::IoError => Some(Finish::Io(io::Error::from_raw_os_error(libc::EIO))),
            FaultKind::Hang => park_forever(),
            FaultKind::StopHeartbeat => {
                ctx.heartbeat_on.store(false, SeqCst);
                park_forever()
            }
            FaultKind::Exit => std::process::exit(f.code),
            FaultKind::KillBeforeOutcome | FaultKind::OutcomeUnsealed => None, // handled at the end
        }
    }

    fn run(&mut self, skip_cases: u64) -> Finish {
        let ctx = self.ctx.clone();
        let script = ctx.script.clone();
        let mut cases_seen = 0u64;
        for (k, op) in script.script.iter().enumerate() {
            for f in script.faults.iter().filter(|f| f.at_op == k) {
                if let Some(fin) = self.fault(f.kind, f) {
                    return fin;
                }
            }
            if let Some(f) = self.safe_point() {
                return f;
            }
            match op {
                FakeOp::Prepare { ms } => {
                    self.phase(Phase::Prepare);
                    if let Some(f) = self.wait(ctx.ms(*ms)) {
                        return f;
                    }
                }
                FakeOp::Case {
                    ordinal,
                    ms,
                    work,
                    result,
                    rule,
                    children,
                } => {
                    cases_seen += 1;
                    if cases_seen <= skip_cases {
                        continue; // committed by the resumed attempt
                    }
                    if let Some(l) = ctx.spec.budgets.work.cases {
                        if ctx.st().counters.cases >= l {
                            return Finish::End {
                                stop: Some(StopCode::MaxCases),
                                verdict: None,
                            };
                        }
                    }
                    if let Err(f) = self.run_case(*ordinal, *ms, work, *result, rule, children) {
                        return f;
                    }
                }
                FakeOp::Heartbeats { n } => {
                    let d = Duration::from_secs(ctx.spec.heartbeat_s as u64 * *n as u64);
                    if let Some(f) = self.wait(d) {
                        return f;
                    }
                }
                FakeOp::TimeDecision {
                    what,
                    limit_s,
                    measured_s,
                    taken,
                } => {
                    let case_ordinal = ctx.st().case_ordinal;
                    let td = TimeDecision {
                        what: *what,
                        case_ordinal,
                        rule: None,
                        limit_s: *limit_s,
                        measured_s: *measured_s,
                        taken: taken.clone(),
                    };
                    ctx.st().time_decisions.push(td.clone());
                    let _ = ctx.sink.emit(Event::TimeDecision(td));
                }
                FakeOp::Alloc { mb } => alloc_touch(*mb << 20),
                FakeOp::Helper { index } => fork_helper(&ctx, script.helpers[*index]),
                FakeOp::End { stop, verdict } => {
                    return Finish::End {
                        stop: *stop,
                        verdict: *verdict,
                    };
                }
                FakeOp::Fail { code, message } => {
                    return Finish::Fail {
                        code: *code,
                        message: message.clone(),
                    };
                }
            }
        }
        for f in script
            .faults
            .iter()
            .filter(|f| f.at_op == script.script.len())
        {
            if let Some(fin) = self.fault(f.kind, f) {
                return fin;
            }
        }
        Finish::End {
            stop: None,
            verdict: None,
        }
    }

    /// Normal end, graceful stop, budget, failure or I/O error: end records, final RESUME, OUTCOME, exit.
    fn finish(mut self, fin: Finish) -> ! {
        let ctx = self.ctx.clone();
        self.phase(Phase::Finishing);
        let src = *ctx.stop_source.lock().unwrap();
        let (code, stop, graceful_src, error) = match fin {
            Finish::End { stop, verdict } => {
                (FakeScript::end_exit(stop, verdict), stop, None, None)
            }
            Finish::Stopped => {
                if src == Some(StopSource::Memory) {
                    (
                        ExitCode::BudgetMemory,
                        Some(StopCode::BudgetMemory),
                        src,
                        None,
                    )
                } else {
                    (
                        ExitCode::Interrupted,
                        Some(StopCode::Interrupted),
                        src,
                        None,
                    )
                }
            }
            Finish::Budget => (ExitCode::BudgetWork, Some(StopCode::BudgetWork), None, None),
            Finish::Fail { code, message } => {
                let (stop, class) = match code {
                    ExitCode::Usage => (StopCode::Refused, ErrorClass::Usage),
                    ExitCode::Input => (StopCode::InputError, ErrorClass::Input),
                    ExitCode::Refused => (StopCode::Refused, ErrorClass::Refused),
                    ExitCode::Io => (StopCode::IoError, ErrorClass::Io),
                    _ => (StopCode::InternalError, ErrorClass::Internal),
                };
                (code, Some(stop), None, Some(ErrorInfo::new(class, message)))
            }
            Finish::Io(e) => io_exit(&ctx, e),
        };
        let faults = &ctx.script.faults;
        let unsealed_claim = faults.iter().any(|f| f.kind == FaultKind::OutcomeUnsealed);
        let kill_before = faults
            .iter()
            .any(|f| f.kind == FaultKind::KillBeforeOutcome);
        let end_kind = if stop.is_none() { "end" } else { "partial" };
        let (a, b) = ctx.jitter().unwrap_or((0.0, 0.0));
        let rules = ctx.st().counters.rules_committed;
        let end = serde_json::json!({"type": end_kind, "charts": 1, "rules": rules, "seconds": a, "vmhwm_kb": (b * 1000.0) as u64});
        let end_line = serde_json::to_vec(&end).unwrap();
        let mut table = TableOutput {
            path: RunLayout::out_rel(rustred_campaign_contract::layout::TABLE),
            rules_committed: rules,
            ..Default::default()
        };
        let mut writer = self.table.take().expect("table");
        if unsealed_claim {
            // contract violation on purpose: no end record, but OUTCOME claims a sealed table
            let st = writer.commit().map_err(Finish::Io);
            let Ok(st) = st else {
                io_exit_now(&ctx, io::Error::other("commit"))
            };
            table.bytes = Some(st.bytes);
            table.blake3 = Some(st.prefix_blake3.clone());
            table.records = Some(st.records);
            table.rules = Some(st.rules);
            table.end = Some(if stop.is_none() {
                EndKind::End
            } else {
                EndKind::Partial
            });
            table.sealed = true;
            table.committed_bytes = st.bytes;
            table.prefix_blake3 = Some(st.prefix_blake3);
        } else {
            match writer.seal(&end_line) {
                Ok(seal) => {
                    seal.apply(&mut table);
                    table.committed_bytes = seal.bytes;
                    table.prefix_blake3 = Some(seal.blake3.clone());
                    // the job state keeps the last COMMIT point (before the end record): the final RESUME's
                    // committed prefix never contains an end record, so a resume continues the records
                }
                Err(WriteError::Io(e)) => io_exit_now(&ctx, e),
                Err(e) => io_exit_now(&ctx, io::Error::other(e.to_string())),
            }
        }
        let qend = serde_json::json!({"type": end_kind, "cases": ctx.st().counters.cases});
        let qbytes = self
            .queue
            .append(&serde_json::to_vec(&qend).unwrap())
            .and_then(|_| self.queue.commit().map_err(WriteError::Io));
        match qbytes {
            Ok(q) => ctx.st().queue_bytes = q.bytes,
            Err(WriteError::Io(e)) => io_exit_now(&ctx, e),
            Err(e) => io_exit_now(&ctx, io::Error::other(e.to_string())),
        }
        ctx.write_resume(true);
        if kill_before {
            sigkill_self();
        }
        if FINAL.swap(true, SeqCst) {
            park_forever();
        }
        let peak = ctx.kill_helpers();
        let detail = ctx.st().hit.clone().map_or(serde_json::Value::Null, |h| {
            serde_json::to_value(h).unwrap()
        });
        let o = ctx.outcome(
            code,
            StopInfo {
                code: stop,
                detail,
                source: graceful_src,
                graceful: true,
            },
            table,
            error,
            peak,
        );
        if let Err(e) = o.write_atomic(&ctx.layout.outcome()) {
            let _ = release_reserve(&ctx.layout.reserve());
            eprintln!("rr_fake_child: OUTCOME: {e}");
            std::process::exit(exit_for_io(&e).code());
        }
        let _ = ctx.sink.emit(Event::JobFinished {
            exit_code: code,
            status: o.job.status,
            stop_code: stop,
        });
        let _ = release_reserve(&ctx.layout.reserve());
        std::process::exit(code.code())
    }
}

fn wfin(e: WriteError) -> Finish {
    match e {
        WriteError::Io(e) => Finish::Io(e),
        other => Finish::Fail {
            code: ExitCode::Internal,
            message: other.to_string(),
        },
    }
}

/// The 3.10 path for an I/O error: free the reserve, OUTCOME (best effort), exit 50 / 51.
fn io_exit(ctx: &Ctx, e: io::Error) -> ! {
    io_exit_now(ctx, e)
}

fn io_exit_now(ctx: &Ctx, e: io::Error) -> ! {
    let code = exit_for_io(&e);
    let (stop, class) = if is_nospace(&e) {
        (StopCode::Nospace, ErrorClass::Nospace)
    } else {
        (StopCode::IoError, ErrorClass::Io)
    };
    let _ = release_reserve(&ctx.layout.reserve());
    if !FINAL.swap(true, SeqCst) {
        let peak = ctx.kill_helpers();
        let o = ctx.outcome(
            code,
            StopInfo {
                code: Some(stop),
                detail: serde_json::Value::Null,
                source: None,
                graceful: true,
            },
            ctx.unsealed_table(),
            Some(ErrorInfo::new(class, e.to_string())),
            peak,
        );
        let _ = o.write_atomic(&ctx.layout.outcome());
        let _ = ctx.sink.emit(Event::JobFinished {
            exit_code: code,
            status: o.job.status,
            stop_code: Some(stop),
        });
    }
    std::process::exit(code.code())
}

fn sigkill_self() -> ! {
    // SAFETY: signalling ourselves.
    unsafe {
        libc::kill(libc::getpid(), libc::SIGKILL);
    }
    park_forever()
}

fn alloc_touch(bytes: u64) {
    let mut v = vec![0u8; bytes as usize];
    for i in (0..v.len()).step_by(4096) {
        v[i] = 1;
    }
    HELD.lock().unwrap_or_else(|p| p.into_inner()).push(v);
}

fn fork_helper(ctx: &Ctx, plan: HelperPlan) {
    // SAFETY: getpid/fork; in the child only async-signal-safe calls (setsid, prctl, getppid, signal, nanosleep,
    // _exit) are made before _exit, so forking from a multi-threaded process is sound.
    let parent = unsafe { libc::getpid() };
    let pid = unsafe { libc::fork() };
    if pid == 0 {
        unsafe {
            if plan.escape_group {
                libc::setsid();
            }
            if plan.pdeathsig {
                libc::prctl(
                    libc::PR_SET_PDEATHSIG,
                    libc::SIGKILL as libc::c_ulong,
                    0,
                    0,
                    0,
                );
                if libc::getppid() != parent {
                    libc::_exit(0);
                }
            }
            let disp = if plan.ignore_sigterm {
                libc::SIG_IGN
            } else {
                libc::SIG_DFL
            };
            for s in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
                libc::signal(s, disp);
            }
            let ts = libc::timespec {
                tv_sec: (plan.sleep_ms / 1000) as libc::time_t,
                tv_nsec: ((plan.sleep_ms % 1000) * 1_000_000) as libc::c_long,
            };
            let mut rem = libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            let mut req = ts;
            while libc::nanosleep(&req, &mut rem) != 0 {
                req = rem;
            }
            libc::_exit(0);
        }
    }
    if pid > 0 {
        let compliant = plan.pdeathsig && !plan.escape_group;
        if compliant {
            ctx.helpers.lock().unwrap().push(pid);
        }
        let _ = ctx.sink.emit(Event::Engine {
            event: serde_json::json!({"helper": {"pid": pid, "registered": compliant, "escape_group": plan.escape_group, "pdeathsig": plan.pdeathsig}}),
        });
    }
}

/// The watchdog thread: heartbeat, signals, STOP file, wall budgets, soft memory limit, stop grace.
fn watchdog(ctx: Arc<Ctx>) {
    let mut seen = 0u32;
    let mut last_hb = Instant::now();
    let mut last_stopfile = Instant::now() - Duration::from_secs(2);
    let hb = Duration::from_secs(ctx.spec.heartbeat_s as u64);
    let grace = Duration::from_secs(ctx.spec.stop_grace_s as u64);
    let mut grown = 0u64;
    loop {
        std::thread::sleep(Duration::from_millis(20));
        let n = SIGNALS.load(Relaxed);
        if n > seen {
            seen = n;
            let src = source_of(LAST_SIG.load(Relaxed));
            if n >= 2 && ctx.stop_requested.load(SeqCst) {
                // second signal: hard-safe path at once
                let code = if *ctx.stop_source.lock().unwrap() == Some(StopSource::Memory) {
                    ExitCode::BudgetMemory
                } else {
                    ExitCode::Interrupted
                };
                let stop = if code == ExitCode::BudgetMemory {
                    StopCode::BudgetMemory
                } else {
                    StopCode::Interrupted
                };
                ctx.hard_stop(code, stop, None);
            }
            ctx.request_stop(src);
        }
        if last_stopfile.elapsed() >= Duration::from_secs(1) {
            last_stopfile = Instant::now();
            if ctx.layout.stop_file().exists() {
                ctx.request_stop(StopSource::StopFile);
            }
        }
        // wall budgets: hard-safe stop, exit 21 (time-decided)
        let w = ctx.spec.budgets.wall;
        if let Some(j) = w.job_s {
            let el = ctx.started.elapsed();
            if el >= Duration::from_secs(j) {
                let td = TimeDecision {
                    what: TimeDecisionWhat::JobWall,
                    case_ordinal: ctx.st().case_ordinal,
                    rule: None,
                    limit_s: j as f64,
                    measured_s: el.as_secs_f64(),
                    taken: "hard_stop".into(),
                };
                ctx.hard_stop(ExitCode::BudgetWall, StopCode::BudgetWall, Some(td));
            }
        }
        if let Some(c) = w.case_s {
            let started = ctx.st().case_started;
            if let Some(t) = started {
                if t.elapsed() >= Duration::from_secs(c) {
                    let td = TimeDecision {
                        what: TimeDecisionWhat::CaseWall,
                        case_ordinal: ctx.st().case_ordinal,
                        rule: None,
                        limit_s: c as f64,
                        measured_s: t.elapsed().as_secs_f64(),
                        taken: "hard_stop".into(),
                    };
                    ctx.hard_stop(ExitCode::BudgetWall, StopCode::BudgetWall, Some(td));
                }
            }
        }
        // rss growth plan
        let rp = ctx.script.rss;
        if rp.grow_bytes_per_s > 0 {
            let target = ((ctx.started.elapsed().as_secs_f64() * rp.grow_bytes_per_s as f64)
                as u64)
                .min(rp.max_bytes);
            if target > grown + (1 << 20) {
                alloc_touch(target - grown);
                grown = target;
            }
        }
        // soft memory limit: self + live helpers
        if let Some(lim) = ctx.spec.budgets.mem_soft_bytes {
            let helpers: Vec<i32> = ctx.helpers.lock().unwrap().clone();
            let kb = rss_kb_of(0) + helpers.iter().map(|&p| rss_kb_of(p)).sum::<u64>();
            if kb * 1024 > lim {
                ctx.request_stop(StopSource::Memory);
            }
        }
        // stop grace exceeded: hard-safe stop
        if ctx.stop_requested.load(SeqCst) {
            let at = *ctx.stop_at.lock().unwrap();
            if at.is_some_and(|t| t.elapsed() >= grace) {
                let mem = *ctx.stop_source.lock().unwrap() == Some(StopSource::Memory);
                let (code, stop) = if mem {
                    (ExitCode::BudgetMemory, StopCode::BudgetMemory)
                } else {
                    (ExitCode::Interrupted, StopCode::Interrupted)
                };
                ctx.hard_stop(code, stop, None);
            }
        }
        if ctx.heartbeat_on.load(SeqCst) && last_hb.elapsed() >= hb {
            last_hb = Instant::now();
            let ev = {
                let st = ctx.st();
                Event::Heartbeat {
                    phase: st.phase,
                    case_ordinal: st.case_ordinal,
                    pending: st.pending.len() as u64,
                    rules_committed: st.counters.rules_committed,
                    counters: st.counters,
                    rss_kb: rss_kb_of(0),
                    vmhwm_kb: vmhwm_kb(),
                    helpers: ctx.helpers.lock().unwrap().len() as u64,
                    stop_requested: ctx.stop_requested.load(SeqCst),
                }
            };
            let _ = ctx.sink.emit(ev);
        }
    }
}

fn run_job(spec_path: &Path) -> ! {
    // signal dispositions first (before any thread or fork), from the script if the spec parses
    let on = std::fs::read(spec_path)
        .ok()
        .and_then(|b| serde_json::from_slice::<JobSpec>(&b).ok())
        .and_then(|s| s.fake.map(|f| f.on_sigterm))
        .unwrap_or_default();
    install_signals(on);
    let me = ProgramSelf {
        name: PROGRAM,
        features: &FEATURES,
        kinds: &[JobKind::Fake],
        blake3: None,
    };
    let prep = match preflight(spec_path, &me, &|k| std::env::var(k).ok()) {
        Ok(p) => p,
        Err(r) => refuse_exit(r),
    };
    let script = prep
        .spec
        .fake
        .clone()
        .expect("validated: kind fake has a script");
    let layout = prep.layout.clone();
    // slow start: nothing written yet (a stop during it is honoured at the first safe point)
    if script.start_delay_ms > 0 {
        std::thread::sleep(Duration::from_secs_f64(
            script.start_delay_ms as f64 * script.time_scale / 1000.0,
        ));
    }
    for d in [layout.out_dir(), layout.tmp_dir()] {
        if let Err(e) = std::fs::create_dir_all(&d) {
            eprintln!("rr_fake_child: {}: {e}", d.display());
            std::process::exit(exit_for_io(&e).code());
        }
    }
    if let Err(e) = create_reserve(&layout.reserve(), RESERVE_BYTES) {
        eprintln!("rr_fake_child: reserve: {e}");
        let _ = release_reserve(&layout.reserve());
        std::process::exit(ExitCode::NoSpace.code());
    }
    let sink = match EventSink::create(&layout.events()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("rr_fake_child: EVENTS: {e}");
            std::process::exit(exit_for_io(&e).code());
        }
    };
    let now_ns = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let resume = match &prep.spec.resume {
        None => None,
        Some(r) => match std::fs::read(&r.from)
            .ok()
            .and_then(|b| serde_json::from_slice::<ResumeState>(&b).ok())
        {
            Some(s) => Some((r.clone(), s)),
            None => refuse_exit(Refusal {
                code: ExitCode::Input,
                stop: StopCode::InputError,
                class: ErrorClass::Input,
                message: "RESUME.json does not parse".into(),
                spec: Some((Box::new(prep.spec.clone()), prep.spec_blake3.clone())),
                write_outcome: true,
            }),
        },
    };
    let ctx = Arc::new(Ctx {
        spec: prep.spec.clone(),
        spec_blake3: prep.spec_blake3.clone(),
        script: script.clone(),
        layout: layout.clone(),
        inputs: prep.inputs.clone(),
        sink,
        st: Mutex::new(State {
            counters: resume
                .as_ref()
                .map(|(_, s)| s.counters_at_commit)
                .unwrap_or_default(),
            commit: CommitPoint {
                bytes: 0,
                prefix_blake3: rustred_campaign_contract::blake3_hex(b""),
                records: 0,
                rules: 0,
            },
            phase: Phase::Prepare,
            case_ordinal: None,
            case_started: None,
            current: None,
            pending: resume
                .as_ref()
                .map(|(_, s)| s.pending.clone())
                .unwrap_or_default(),
            per_case: vec![],
            time_decisions: vec![],
            elided: 0,
            hit: None,
            resume_written_at: None,
            resume_valid: false,
            queue_bytes: 0,
        }),
        started: Instant::now(),
        started_utc: rustred_campaign_contract::time::utc_now(),
        stop_requested: AtomicBool::new(false),
        stop_source: Mutex::new(None),
        stop_at: Mutex::new(None),
        heartbeat_on: AtomicBool::new(true),
        helpers: Mutex::new(Vec::new()),
        resumed: resume.is_some(),
        jitter_seed: AtomicU64::new(now_ns ^ ((std::process::id() as u64) << 32)),
    });
    let _ = CTX.set(ctx.clone());
    std::panic::set_hook(Box::new(|info| {
        eprintln!("rr_fake_child: panic: {info}");
        if let Some(ctx) = CTX.get() {
            if !FINAL.swap(true, SeqCst) {
                let peak = ctx.kill_helpers();
                let o = ctx.outcome(
                    ExitCode::Internal,
                    StopInfo {
                        code: Some(StopCode::Panic),
                        detail: serde_json::Value::Null,
                        source: None,
                        graceful: false,
                    },
                    ctx.unsealed_table(),
                    Some(ErrorInfo::new(ErrorClass::Panic, info.to_string())),
                    peak,
                );
                let _ = o.write_atomic(&ctx.layout.outcome());
                ctx.sink.try_emit_for(
                    Event::JobFinished {
                        exit_code: ExitCode::Internal,
                        status: o.job.status,
                        stop_code: Some(StopCode::Panic),
                    },
                    Duration::from_secs(1),
                );
            }
        }
        // SAFETY: immediate exit from the panicking thread.
        unsafe { libc::_exit(ExitCode::Internal.code()) }
    }));
    let _ = ctx.sink.emit(Event::JobStarted {
        job_id: ctx.spec.job_id.clone(),
        attempt: ctx.spec.attempt,
        spec_blake3: ctx.spec_blake3.clone(),
        pid: std::process::id(),
        program: PROGRAM.into(),
        version: env!("CARGO_PKG_VERSION").into(),
        git_commit: program_info().git_commit,
        resume_from: resume.as_ref().map(|(_, s)| s.attempt),
    });
    let wd = ctx.clone();
    std::thread::spawn(move || watchdog(wd));
    // table: fresh with a header, or the verified committed prefix of the previous attempt
    let table = match &resume {
        Some((r, _)) => RecordWriter::resume_from_prefix(Path::new(&r.table), r.committed_bytes, &r.prefix_blake3, &layout.table()),
        None => RecordWriter::create(&layout.table()).map_err(WriteError::Io).and_then(|mut w| {
            let (z, _) = ctx.jitter().unwrap_or((0.0, 0.0));
            let h = serde_json::json!({"type":"header","schema":"rustred.fake.table.v1","source":PROGRAM,"contract":CONTRACT,"sample_seed":ctx.spec.seed.sample_seed,"zero_seconds":z});
            w.append(&serde_json::to_vec(&h).unwrap()).map(|_| w)
        }),
    };
    let mut table = match table {
        Ok(t) => t,
        Err(WriteError::Io(e)) => io_exit_now(&ctx, e),
        Err(e) => refuse_exit(Refusal {
            code: ExitCode::Input,
            stop: StopCode::InputError,
            class: ErrorClass::Input,
            message: format!("resume prefix: {e}"),
            spec: Some((Box::new(ctx.spec.clone()), ctx.spec_blake3.clone())),
            write_outcome: true,
        }),
    };
    let queue = match RecordWriter::create(&layout.queue()) {
        Ok(q) => q,
        Err(e) => io_exit_now(&ctx, e),
    };
    match table.commit() {
        Ok(cp) => ctx.st().commit = cp,
        Err(e) => io_exit_now(&ctx, e),
    }
    let skip = resume
        .as_ref()
        .map_or(0, |(_, s)| s.counters_at_commit.cases);
    let mut m = Main {
        ctx: ctx.clone(),
        table: Some(table),
        queue,
    };
    let fin = m.run(skip);
    m.finish(fin)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["version", "--json"] => {
            let v = VersionInfo {
                program: PROGRAM.into(),
                package_version: env!("CARGO_PKG_VERSION").into(),
                git_commit: program_info().git_commit,
                contract: vec![CONTRACT.into()],
                features: FEATURES.iter().map(|s| s.to_string()).collect(),
                job_kinds: vec![JobKind::Fake],
                arities: vec![],
            };
            println!("{}", serde_json::to_string(&v).unwrap());
        }
        ["job", path] => run_job(Path::new(path)),
        _ => {
            eprintln!("usage: rr_fake_child job RUN/job.json | rr_fake_child version --json");
            std::process::exit(2);
        }
    }
}
