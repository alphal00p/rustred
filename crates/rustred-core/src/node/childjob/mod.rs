//! Engine side of the child-job contract `rustred.child-job.v1` (plan/auto-campaign/interfaces/child-job.md of the
//! p-integral workspace; engine changes C1-C8, C16, C17 of its section 5).
//!
//! `rr_node job RUN/job.json` runs ONE job described by a job spec. This module holds the job context (a global
//! [`OnceLock`]: absent in every legacy invocation, where each hook below is a no-op and the legacy output is
//! byte-identical), the hooks the node driver calls at its safe points, the watchdog thread (signals, STOP file,
//! wall budgets, soft memory limit, stop grace, heartbeat) and the writers of OUTCOME.json, RESUME.json and
//! EVENTS.jsonl. Types, file formats, the canonical record form, sealing, the start-up checks and the exit codes
//! come from the shared contract crate (`rustred-campaign-contract`); the fake child of that crate is the executable
//! specification this implementation follows.
//!
//! SAFE POINTS (3.4.2) of `generate` in job mode: (a) each popped symbolic case, right after it left the queue and
//! before any work on it (= the COMMIT POINT of the previous case: its rule is written, its children enqueued, the
//! table and queue log flushed); (b) each numeric endpoint, before its search; (c) the end of the symbolic queue.
//! A stop request (signal, STOP file, memory) is honoured at the next safe point (graceful: sealed partial table,
//! RESUME, OUTCOME `interrupted`, exit 30); when none is reached within `stop_grace_s`, or on a second signal, the
//! watchdog performs the HARD-SAFE stop (helpers killed and reaped, OUTCOME from the last commit point, tables not
//! touched, `_exit`). Not safe points: inside a case search, the exact lift, guard extraction, the fork waits (see
//! the DEVIATION note in the lane notes), canonicalisation.

pub mod proc;

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU8, AtomicU64, Ordering::*};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use cc::events::{Event, EventSink, Phase};
use cc::layout::{CERT, QUEUE, RESUME, RunLayout, TABLE};
use cc::outcome::{
    Effective, ErrorClass, ErrorInfo, EventsOutput, ExeInfo, InputCheck, JobOutcome, Memory,
    Outcome, Outputs, PerCase, ProgramInfo, QueueOutput, ResumeOutput, StopInfo, StreamOutput,
    TableOutput, TimeDecision, TimeDecisionWhat, Timing,
};
use cc::resume::{CaseSpec as JobCase, CommittedFile, Frame, ResumeState};
use cc::seal::{CERT_END_TYPES, StreamDigest, TABLE_END_TYPES};
use cc::spec::{Counters, JobSpec, WorkCounter};
use cc::stop::{CaseResult, JobStatus, StopCode, StopSource};
use cc::{ExitCode, RESUME_SCHEMA};
use rustred_campaign_contract as cc;
use serde_json::Value;

use super::driver::{CaseSpec, StopReason};
use crate::solver::{Case, CoordinateCase};

/// Program name in job mode.
pub const PROGRAM: &str = "rr_node";
/// Features this build reports in `version --json` (3.2.1) and accepts in `program.features_required`.
pub const FEATURES: [&str; 18] = [
    "job",
    "initial_presented",
    "descend_open",
    "no_elision",
    "family_blake3",
    "signals",
    "heartbeat",
    "resume",
    // engine line (coord-ec1-seed-centre.md v2): seed options and symmetry rows usable through `args` / the cases input
    "max_depth",
    "seed_centre",
    "case_seed_centres",
    "seed_face_floor",
    "seed_layer_cap",
    "symmetry_rows",
    "symmetry_symbolic",
    "case_seed_rung",
    "slim_rules",
    "open_on_refusal",
];

static JOB: OnceLock<Arc<Job>> = OnceLock::new();
/// Set by whoever writes the final OUTCOME (main thread at the end, watchdog on a hard-safe stop).
static FINAL: AtomicBool = AtomicBool::new(false);

/// Whether this process runs a job (`rr_node job`). False in every legacy invocation.
pub fn active() -> bool {
    JOB.get().is_some()
}

fn job() -> Option<&'static Arc<Job>> {
    JOB.get()
}

// -------------------------------------------------------------------------------------------------------------
// hooks for the node driver (each a no-op in legacy mode)

/// Which output stream of `generate` a file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Table,
    Queue,
    Cert,
}

/// A stream file of `generate`. In legacy mode a plain `File::create` (truncating, as before). In job mode created
/// with O_EXCL + O_NOFOLLOW (the directory fsynced), and every byte that reaches the file also feeds a
/// [`StreamDigest`] shared with the job context (running raw and canonical digest, verifier rules), so a commit
/// point needs no re-read (C7). An I/O error is remembered for the exit classification (50 / 51).
pub struct ChildFile {
    file: File,
    digest: Option<Arc<Mutex<StreamDigest>>>,
    suppressible: bool,
}

/// A resumed attempt: the table and queue log already hold the committed prefix (copied and verified before the
/// driver starts), so what the driver writes before the resume point (header, zero sectors, chart, the initial
/// queue) is swallowed until [`resume_arm`].
static SUPPRESS: AtomicBool = AtomicBool::new(false);

impl Write for ChildFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.suppressible && SUPPRESS.load(Relaxed) {
            return Ok(buf.len());
        }
        match self.file.write(buf) {
            Ok(n) => {
                if let Some(d) = &self.digest {
                    lock(d.as_ref()).feed(&buf[..n]);
                }
                Ok(n)
            }
            Err(e) => {
                note_io_error(&e);
                Err(e)
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl ChildFile {
    /// `fdatasync` (job mode: before a RESUME naming this prefix is written).
    pub fn sync_data(&self) -> io::Result<()> {
        self.file.sync_data()
    }
}

/// Create a stream file of `generate` (see [`ChildFile`]).
pub fn create_stream(path: &Path, kind: Stream) -> io::Result<ChildFile> {
    let Some(j) = job() else {
        return Ok(ChildFile {
            file: File::create(path)?,
            digest: None,
            suppressible: false,
        });
    };
    use std::os::unix::fs::OpenOptionsExt;
    let mut digest = match kind {
        Stream::Table => StreamDigest::new(&TABLE_END_TYPES).recording_types(),
        Stream::Queue => StreamDigest::new(&TABLE_END_TYPES).recording_types(),
        Stream::Cert => StreamDigest::new(&CERT_END_TYPES),
    };
    // a resumed attempt continues the copied committed prefix of the table and the queue log
    let resumed = RESUMED.get().is_some_and(|r| r.prefix) && kind != Stream::Cert;
    let file = if resumed {
        digest.feed_file(path)?;
        let _ = digest.take_records();
        std::fs::OpenOptions::new()
            .append(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .inspect_err(note_io_error)?
    } else {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .inspect_err(note_io_error)?
    };
    if let Some(d) = path.parent() {
        cc::atomic::fsync_dir(d)?;
    }
    let digest = Arc::new(Mutex::new(digest));
    let slot = match kind {
        Stream::Table => &j.table,
        Stream::Queue => &j.queue,
        Stream::Cert => &j.cert,
    };
    *lock(slot) = Some(digest.clone());
    Ok(ChildFile {
        file,
        digest: Some(digest),
        suppressible: resumed,
    })
}

/// What a resumed attempt continues from (RESUME.json of the previous attempt, its committed table prefix).
pub struct Resumed {
    /// The open queue, `current` first (PRESENTED slots).
    pub cases: Vec<CaseSpec>,
    /// Open numeric endpoints (all slots fixed).
    pub numerical: Vec<Vec<i64>>,
    /// The committed rule records (they become cover entries; indices continue after them).
    pub rules: Vec<Value>,
    pub symbolic_cases: usize,
    /// The driver's running counters at the commit point: [rhs terms, branches, discarded, deferred regions,
    /// deferred branches].
    pub counters: [usize; 5],
    /// Deferred regions and failed branch admissions (charts, PRESENTED slots) with the driver's detail object
    /// (kind, rule, branch, reason, ...; `null` when an older RESUME has none).
    pub deferred: Vec<(CaseSpec, Value)>,
    /// Cases left open by `--open-on-refusal` (PRESENTED slots) with the driver's detail object.
    pub refused_open: Vec<(CaseSpec, Value)>,
    /// A resume of an earlier attempt (the table and queue log continue a copied prefix); false for PRESENTED cases.
    pub prefix: bool,
}

static RESUMED: OnceLock<Resumed> = OnceLock::new();

/// The resume state of a resumed attempt (job mode only).
pub fn resumed() -> Option<&'static Resumed> {
    RESUMED.get()
}

/// The resume point is reached: from here the driver's writes go to the table and queue log again.
pub fn resume_arm(out: &mut impl Write, qlog: &mut impl Write) -> Result<(), String> {
    out.flush().map_err(|e| format!("table: {e}"))?;
    qlog.flush().map_err(|e| format!("queue log: {e}"))?;
    SUPPRESS.store(false, SeqCst);
    Ok(())
}

/// The driver's running counters at a safe point (see [`Resumed::counters`]).
pub type EngineCounters = [usize; 5];
const ENGINE_KEYS: [&str; 5] = [
    "rhs_terms",
    "branches",
    "discarded",
    "deferred_regions",
    "deferred_branches",
];

/// Whether witness loci above the record size limit are written as the literal "elided" (C17, ruling R-f): true in
/// legacy mode (today's behaviour), else `output.elide_witness_loci` of the job spec (default false = forbidden:
/// every locus in full whatever its size).
pub fn elide_witness_loci() -> bool {
    job().is_none_or(|j| j.spec.output.elide_witness_loci) && !LEGACY_NO_ELISION.load(Relaxed)
}

static LEGACY_NO_ELISION: AtomicBool = AtomicBool::new(false);

/// `rr_node generate --no-elide-witness-loci` (legacy command line, opt-in): the C17 behaviour without job mode.
pub fn set_legacy_no_elision() {
    LEGACY_NO_ELISION.store(true, Relaxed);
}

/// The table header's `family_blake3` member (C16) as a JSON fragment `,"family_blake3":"<hex>"` placed after
/// `family_file`; empty in legacy mode (the header stays byte-identical).
pub fn header_family_digest() -> String {
    match job().and_then(|j| j.family_blake3.as_deref()) {
        Some(d) => format!(",\"family_blake3\":\"{d}\""),
        None => String::new(),
    }
}

/// Whether the chart / end / cert end records are the partial kinds. Job mode: whenever the chart did not close
/// (the contract's `end` iff the stop is null); legacy mode: never forced (the caller's own rule applies).
pub fn partial_end(not_complete: bool) -> bool {
    active() && not_complete
}

/// Mark the start of an exact lift (phase `exact_lift`; `trace_rows` feeds the `trace_rows_max` counter).
pub fn exact_started(trace_rows: usize) {
    if let Some(j) = job() {
        j.phase.store(Phase::ExactLift as u8, Relaxed);
        j.counter(WorkCounter::TraceRows)
            .fetch_max(trace_rows as u64, Relaxed);
    }
}

/// Count work (one relaxed add; no-op in legacy mode).
pub fn tick(c: WorkCounter, by: u64) {
    if let Some(j) = job() {
        j.counter(c).fetch_add(by, Relaxed);
    }
}

/// One seed visited by a case search (`seeds`, child-job.md 3.3.1).
#[inline]
pub fn tick_seed() {
    tick(WorkCounter::Seeds, 1);
}

/// One basis row instantiated (`rows`).
#[inline]
pub fn tick_row() {
    tick(WorkCounter::Rows, 1);
}

/// One row evaluated mod p (`probe_evals`).
#[inline]
pub fn tick_probe_eval() {
    tick(WorkCounter::ProbeEvals, 1);
}

/// One independent row added to the modular reducer (`reducer_rows`).
#[inline]
pub fn tick_reducer_row() {
    tick(WorkCounter::ReducerRows, 1);
}

/// The open queue at a safe point in the job's RESUME form: pending cases (PRESENTED slots) and numeric points.
pub(crate) fn queue_lists<const N: usize>(
    pending: &[Case<N>],
    numerical: &[CoordinateCase<N>],
    deferred: &[(Case<N>, Value)],
    refused: &[(Case<N>, Value)],
    n: usize,
) -> OpenQueue {
    OpenQueue {
        pending: pending
            .iter()
            .map(|c| job_case(&super::driver::case_spec(c, n)))
            .collect(),
        numerical: numerical
            .iter()
            .map(|p| {
                p.fixed()[..n]
                    .iter()
                    .map(|v| v.map_or(0, i64::from))
                    .collect()
            })
            .collect(),
        deferred: deferred
            .iter()
            .map(|(c, d)| {
                serde_json::json!({"chart": job_case(&super::driver::case_spec(c, n)), "detail": d})
            })
            .collect(),
        refused: refused
            .iter()
            .map(|(c, d)| {
                serde_json::json!({"case": job_case(&super::driver::case_spec(c, n)), "detail": d})
            })
            .collect(),
    }
}

/// The open queue at a safe point (RESUME `pending`, `numerical`, `deferred`, `refused_open`).
pub struct OpenQueue {
    pub pending: Vec<JobCase>,
    pub numerical: Vec<Vec<i64>>,
    pub deferred: Vec<Value>,
    /// `--open-on-refusal`: `{"case": .., "detail": ..}` per refused case (RESUME `refused_open`).
    pub refused: Vec<Value>,
}

fn job_case(c: &CaseSpec) -> JobCase {
    JobCase {
        frame: Frame::Presented,
        fixed: c.fixed.clone(),
        equations: c
            .equations
            .iter()
            .map(|e| Value::from(e.as_str()))
            .collect(),
        seed_centre_offset: None,
        extra: Default::default(),
    }
}

/// Safe point (a) / (b): called with the case just taken from the queue (`current`, PRESENTED slots), before any
/// work on it. Closes the previous case (per-case result, `case_finished` / `rule_committed` events), makes the
/// COMMIT POINT (table and queue log flushed, their committed prefix recorded; RESUME rewritten when due), then
/// honours a stop request (returns the stop for the driver's normal stop path) or starts `current`.
#[allow(clippy::too_many_arguments)]
pub fn case_boundary(
    out: &mut impl Write,
    qlog: &mut impl Write,
    symbolic_cases: usize,
    counters: EngineCounters,
    current: CaseSpec,
    numerical: bool,
    open: &mut dyn FnMut() -> OpenQueue,
) -> Result<Boundary, String> {
    let Some(j) = job() else {
        return Ok(Boundary::Go);
    };
    j.commit_point(out, qlog, symbolic_cases)?;
    lock(&j.st).engine = counters;
    let current = job_case(&current);
    if j.stop.load(SeqCst) {
        let q = open();
        let pending = q.pending.len();
        j.remember_open(Some(&current), &q);
        j.write_resume(Some(current), q, true);
        let source = lock(&j.stop_source).unwrap_or(StopSource::Sigterm);
        return Ok(Boundary::Stop(StopReason::Interrupted {
            source: serde_json::to_value(source)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default(),
            pending: pending + 1,
        }));
    }
    // `--descend open`: only the job's own cases are solved; every other case is returned (F.subcases.jsonl)
    if DESCEND_OPEN.load(Relaxed) {
        let key = serde_json::to_string(&current).unwrap_or_default();
        let mut own = lock(&OWN_CASES);
        if own.is_none() {
            let q = open();
            let mut set: std::collections::BTreeSet<String> = q
                .pending
                .iter()
                .map(|c| serde_json::to_string(c).unwrap_or_default())
                .collect();
            set.insert(key.clone());
            *own = Some(set);
        }
        if numerical || !own.as_ref().is_some_and(|s| s.contains(&key)) {
            drop(own);
            lock(&j.st)
                .returned
                .push((if numerical { "numeric" } else { "case" }, current));
            return Ok(Boundary::Skip);
        }
    }
    if j.resume_due() {
        let q = open();
        j.write_resume(Some(current.clone()), q, false);
    }
    j.start_case(current, numerical);
    Ok(Boundary::Go)
}

/// What the driver does after a safe point.
pub enum Boundary {
    /// Work on the case.
    Go,
    /// Do not work on it (returned to the supervisor, `--descend open`): `continue`.
    Skip,
    /// Stop (the driver's normal stop path).
    Stop(StopReason),
}

static DESCEND_OPEN: AtomicBool = AtomicBool::new(false);
/// The job's own cases (`--descend open`): the queue at the first safe point.
static OWN_CASES: Mutex<Option<std::collections::BTreeSet<String>>> = Mutex::new(None);

/// `--descend open` (job mode): solve the job's own cases only and return every sub-case, numeric endpoint and
/// deferred region (sealed `out/F.subcases.jsonl`, stop `max_cases`, exit 10) instead of descending into them.
pub fn set_descend_open() {
    DESCEND_OPEN.store(true, Relaxed);
}

/// Safe point (c): the symbolic queue ended (empty, `max_cases`, or a stop). Closes the last case and makes the
/// commit point; RESUME names the open queue (`current` null).
pub fn queue_done(
    out: &mut impl Write,
    qlog: &mut impl Write,
    symbolic_cases: usize,
    counters: EngineCounters,
    open: &mut dyn FnMut() -> OpenQueue,
) -> Result<Option<StopReason>, String> {
    let Some(j) = job() else { return Ok(None) };
    j.commit_point(out, qlog, symbolic_cases)?;
    lock(&j.st).engine = counters;
    let interrupted = j.stop.load(SeqCst) && lock(&j.st).resume_current.is_some();
    let mut stop = None;
    if !interrupted {
        let q = open();
        j.remember_open(None, &q);
        if DESCEND_OPEN.load(Relaxed) {
            let mut st = j.st();
            let n = st.returned.len()
                + q.pending.len()
                + q.numerical.len()
                + q.deferred.len()
                + q.refused.len();
            if n > 0 {
                stop = Some(StopReason::Descended { returned: n });
            }
            st.open_on_end = true;
        }
        j.write_resume(None, q, true);
    }
    j.phase.store(Phase::Numeric as u8, Relaxed);
    Ok(stop)
}

// -------------------------------------------------------------------------------------------------------------
// job context

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

static IO_ERRNO: AtomicI32 = AtomicI32::new(0);

fn note_io_error(e: &io::Error) {
    if let Some(c) = e.raw_os_error() {
        let _ = IO_ERRNO.compare_exchange(0, c, Relaxed, Relaxed);
    } else {
        let _ = IO_ERRNO.compare_exchange(0, -1, Relaxed, Relaxed);
    }
}

struct InFlight {
    ordinal: u64,
    case: Value,
    rules_before: u64,
}

struct State {
    /// Last commit point of the table / queue log.
    table_commit: CommittedFile,
    queue_commit: CommittedFile,
    rules_committed: u64,
    symbolic_cases: u64,
    per_case: Vec<PerCase>,
    in_flight: Option<InFlight>,
    ordinal: u64,
    pending: u64,
    time_decisions: Vec<TimeDecision>,
    resume_written_at: Option<Instant>,
    resume_valid: bool,
    /// `current` of the last RESUME written (an interrupted case).
    resume_current: Option<JobCase>,
    /// The driver's running counters at the last commit point.
    engine: EngineCounters,
    /// Cases returned by `--descend open` (kind, case).
    returned: Vec<(&'static str, JobCase)>,
    /// The open list at the last stop / queue end: (kind, case).
    open_list: Vec<(&'static str, JobCase)>,
    /// `--descend open` decided at the queue end: the open list goes into F.subcases.jsonl even without a stop.
    open_on_end: bool,
}

/// Inputs to start a job (from the bin's `rr_node job`).
pub struct JobStart {
    pub spec: JobSpec,
    pub spec_blake3: String,
    pub layout: RunLayout,
    pub inputs: Vec<InputCheck>,
    pub program: ProgramInfo,
    /// The verified resume source (preflight step 9), when the spec resumes an earlier attempt.
    pub resume: Option<cc::preflight::VerifiedResume>,
}

/// The job context (one per process).
pub struct Job {
    spec: JobSpec,
    spec_blake3: String,
    layout: RunLayout,
    inputs: Vec<InputCheck>,
    family_blake3: Option<String>,
    program: ProgramInfo,
    sink: EventSink,
    started: Instant,
    started_utc: String,
    table: Mutex<Option<Arc<Mutex<StreamDigest>>>>,
    queue: Mutex<Option<Arc<Mutex<StreamDigest>>>>,
    cert: Mutex<Option<Arc<Mutex<StreamDigest>>>>,
    effective: Mutex<Option<Effective>>,
    st: Mutex<State>,
    stop: AtomicBool,
    stop_source: Mutex<Option<StopSource>>,
    stop_at: Mutex<Option<Instant>>,
    counters: [AtomicU64; 9],
    phase: AtomicU8,
    /// Milliseconds since `started` at which the current case started; -1 = none.
    case_started_ms: AtomicI64,
    case_ordinal: AtomicI64,
}

const PHASES: [Phase; 11] = [
    Phase::Prepare,
    Phase::ZeroCensus,
    Phase::CoverLoad,
    Phase::CaseSearch,
    Phase::ExactLift,
    Phase::Guards,
    Phase::Admission,
    Phase::Numeric,
    Phase::SharedNumeric,
    Phase::DeferredCheck,
    Phase::Finishing,
];

fn empty_commit(rel: &str) -> CommittedFile {
    CommittedFile::empty(rel)
}

impl Job {
    fn counter(&self, c: WorkCounter) -> &AtomicU64 {
        let i = WorkCounter::ALL.iter().position(|&x| x == c).unwrap_or(0);
        &self.counters[i]
    }

    fn phase(&self) -> Phase {
        PHASES
            .get(self.phase.load(Relaxed) as usize)
            .copied()
            .unwrap_or(Phase::Prepare)
    }

    fn set_phase(&self, p: Phase) {
        if self.phase.swap(p as u8, Relaxed) != p as u8 {
            let _ = self.sink.emit(Event::Phase { phase: p });
        }
    }

    /// Counter values (cases and rules from the commit state).
    fn counters(&self, st: Option<&State>) -> Counters {
        let mut c = Counters::default();
        for (i, w) in WorkCounter::ALL.iter().enumerate() {
            let v = self.counters[i].load(Relaxed);
            match w {
                WorkCounter::Cases => {}
                WorkCounter::TraceRows => c.trace_rows_max = v,
                _ => c.add(*w, v),
            }
        }
        if let Some(s) = st {
            c.cases = s.symbolic_cases;
            c.rules_committed = s.rules_committed;
        }
        c
    }

    fn st(&self) -> MutexGuard<'_, State> {
        lock(&self.st)
    }

    /// The state for the hard-safe stop: the lock is never held across I/O, so a bounded wait suffices.
    fn st_try(&self) -> Option<MutexGuard<'_, State>> {
        let end = Instant::now() + Duration::from_secs(1);
        loop {
            match self.st.try_lock() {
                Ok(g) => return Some(g),
                Err(std::sync::TryLockError::Poisoned(p)) => return Some(p.into_inner()),
                Err(_) if Instant::now() > end => return None,
                Err(_) => std::thread::sleep(Duration::from_millis(5)),
            }
        }
    }

    fn request_stop(&self, src: StopSource) {
        if !self.stop.swap(true, SeqCst) {
            *lock(&self.stop_source) = Some(src);
            *lock(&self.stop_at) = Some(Instant::now());
            let _ = self.sink.emit(Event::StopRequested {
                source: src,
                count: proc::signals().0.max(1),
            });
        }
    }

    fn digest_commit(
        &self,
        slot: &Mutex<Option<Arc<Mutex<StreamDigest>>>>,
        rel: &str,
    ) -> Result<Option<CommittedFile>, String> {
        let Some(d) = lock(slot).clone() else {
            return Ok(None);
        };
        let d = lock(d.as_ref());
        if let Some(f) = d.failed() {
            return Err(format!("{rel}: record outside the contract: {f}"));
        }
        Ok(d.committed(rel))
    }

    /// Close the case in flight and record the commit point.
    fn commit_point(
        &self,
        out: &mut impl Write,
        qlog: &mut impl Write,
        symbolic_cases: usize,
    ) -> Result<(), String> {
        out.flush().map_err(|e| format!("table: {e}"))?;
        qlog.flush().map_err(|e| format!("queue log: {e}"))?;
        let rel_t = RunLayout::out_rel(TABLE);
        let rel_q = RunLayout::out_rel(QUEUE);
        let t = self.digest_commit(&self.table, &rel_t)?;
        let q = self.digest_commit(&self.queue, &rel_q)?;
        let table_records = lock(&self.table)
            .clone()
            .map(|d| lock(d.as_ref()).take_records())
            .unwrap_or_default();
        let queue_records = lock(&self.queue)
            .clone()
            .map(|d| lock(d.as_ref()).take_records())
            .unwrap_or_default();
        let mut events = Vec::new();
        {
            let mut st = self.st();
            st.symbolic_cases = symbolic_cases as u64;
            if let Some(f) = st.in_flight.take() {
                let rules: Vec<_> = table_records
                    .iter()
                    .filter(|r| r.record_type.as_deref() == Some("rule"))
                    .collect();
                let types: Vec<&str> = queue_records
                    .iter()
                    .filter_map(|r| r.record_type.as_deref())
                    .collect();
                let result = if !rules.is_empty() {
                    CaseResult::Rule
                } else {
                    case_result(&types)
                };
                let children = types
                    .iter()
                    .filter(|t| matches!(**t, "enqueue" | "enqueue_numerical" | "child"))
                    .count() as u64;
                let rule = (!rules.is_empty()).then_some(f.rules_before);
                for (k, r) in rules.iter().enumerate() {
                    events.push(Event::RuleCommitted {
                        index: f.rules_before + k as u64,
                        line_offset: r.offset,
                        line_bytes: r.bytes,
                        canon_blake3: r.canon_blake3.clone(),
                        rhs: 0,
                        trace_rows: 0,
                        monomials: 0,
                    });
                }
                st.per_case.push(PerCase {
                    ordinal: f.ordinal,
                    case: f.case,
                    result,
                    rule,
                    children_open: children,
                });
                let rc = t.as_ref().map_or(st.rules_committed, |c| c.rules);
                events.push(Event::CaseFinished {
                    ordinal: f.ordinal,
                    result,
                    rule,
                    children,
                    rules_committed: rc,
                });
            }
            if let Some(t) = t {
                st.rules_committed = t.rules;
                st.table_commit = t;
            }
            if let Some(q) = q {
                st.queue_commit = q;
            }
        }
        self.case_ordinal.store(-1, Relaxed);
        self.case_started_ms.store(-1, Relaxed);
        for e in events {
            let _ = self.sink.emit(e);
        }
        Ok(())
    }

    fn start_case(&self, case: JobCase, numerical: bool) {
        let case_v = serde_json::to_value(&case).unwrap_or(Value::Null);
        let (ordinal, rules_before, pending) = {
            let mut st = self.st();
            let o = st.ordinal;
            st.ordinal += 1;
            st.in_flight = Some(InFlight {
                ordinal: o,
                case: case_v.clone(),
                rules_before: st.rules_committed,
            });
            (o, st.rules_committed, st.pending)
        };
        let _ = rules_before;
        self.case_ordinal.store(ordinal as i64, Relaxed);
        self.case_started_ms
            .store(self.started.elapsed().as_millis() as i64, Relaxed);
        if numerical {
            self.counter(WorkCounter::NumericPoints)
                .fetch_add(1, Relaxed);
        }
        self.set_phase(if numerical {
            Phase::Numeric
        } else {
            Phase::CaseSearch
        });
        let _ = self.sink.emit(Event::CaseStarted {
            ordinal,
            case: case_v,
            pending,
            numerical: u64::from(numerical),
        });
    }

    /// Remember the open list (F.subcases.jsonl of a stopped job).
    fn remember_open(&self, current: Option<&JobCase>, q: &OpenQueue) {
        let mut l: Vec<(&'static str, JobCase)> = Vec::new();
        if let Some(c) = current {
            l.push(("case", c.clone()));
        }
        l.extend(q.pending.iter().map(|c| ("case", c.clone())));
        l.extend(q.numerical.iter().map(|f| {
            (
                "numeric",
                JobCase {
                    frame: Frame::Presented,
                    fixed: f.iter().map(|&v| Some(v)).collect(),
                    equations: vec![],
                    seed_centre_offset: None,
                    extra: Default::default(),
                },
            )
        }));
        l.extend(
            q.deferred
                .iter()
                .filter_map(|d| serde_json::from_value::<JobCase>(d["chart"].clone()).ok())
                .map(|c| ("region", c)),
        );
        l.extend(
            q.refused
                .iter()
                .filter_map(|d| serde_json::from_value::<JobCase>(d["case"].clone()).ok())
                .map(|c| ("refused", c)),
        );
        self.st().open_list = l;
    }

    fn resume_due(&self) -> bool {
        let st = self.st();
        let every = self.spec.resume_every_s as u64;
        every == 0
            || st
                .resume_written_at
                .is_none_or(|t| t.elapsed() >= Duration::from_secs(every))
    }

    /// RESUME.json at the last commit point (3.8.1). The table and queue prefixes it names are fsynced first.
    fn write_resume(&self, current: Option<JobCase>, q: OpenQueue, force: bool) {
        let OpenQueue {
            pending,
            numerical,
            deferred,
            refused,
        } = q;
        if !force && !self.resume_due() {
            return;
        }
        // the prefixes must be durable before a RESUME names them
        for p in [self.layout.table(), self.layout.queue()] {
            if let Ok(f) = File::open(&p) {
                let _ = f.sync_data();
            }
        }
        let r = {
            let mut st = self.st();
            st.pending = pending.len() as u64;
            st.resume_current = current.clone();
            ResumeState {
                schema: RESUME_SCHEMA.into(),
                job_id: self.spec.job_id.clone(),
                attempt: self.spec.attempt,
                spec_blake3: self.spec_blake3.clone(),
                chart: Value::Null,
                frame: Frame::Presented,
                rules_committed: st.rules_committed,
                table: st.table_commit.clone(),
                queue: Some(st.queue_commit.clone()),
                current,
                pending,
                numerical,
                deferred,
                refused_open: refused,
                counters_at_commit: self.counters(Some(&st)),
                symbolic_cases: st.symbolic_cases,
                discarded: st.engine[2] as u64,
                engine: ENGINE_KEYS
                    .iter()
                    .zip(st.engine)
                    .map(|(k, v)| (k.to_string(), Value::from(v)))
                    .collect(),
            }
        };
        if r.check().is_err() {
            return;
        }
        if cc::atomic::write_json_atomic(&self.layout.resume(), &r).is_ok() {
            {
                let mut st = self.st();
                st.resume_written_at = Some(Instant::now());
                st.resume_valid = true;
            }
            let _ = self.sink.emit(Event::ResumeWritten {
                rules_committed: r.rules_committed,
                pending: r.pending.len() as u64,
                table_bytes: r.table.bytes,
            });
        }
    }

    /// The OUTCOME for the current state. `sealed`: (table output, cert output) of a normal end.
    #[allow(clippy::too_many_arguments)]
    fn outcome(
        &self,
        st: Option<&State>,
        code: ExitCode,
        stop: StopInfo,
        sealed: Option<(TableOutput, Option<StreamOutput>)>,
        error: Option<ErrorInfo>,
        legacy: serde_json::Map<String, Value>,
        helpers_peak: u64,
    ) -> Outcome {
        let mut j = JobOutcome::for_spec(
            &self.spec,
            &self.spec_blake3,
            self.program.clone(),
            code,
            stop,
        );
        j.effective = lock(&self.effective).clone();
        j.inputs = self.inputs.clone();
        j.counters = self.counters(st);
        if let Some(s) = st {
            j.per_case = s.per_case.clone();
            j.time_decisions = s.time_decisions.clone();
        }
        j.time_decided = !j.time_decisions.is_empty() || j.stop.code == Some(StopCode::BudgetWall);
        let (rules, commit) = st.map_or((0, empty_commit(&RunLayout::out_rel(TABLE))), |s| {
            (s.rules_committed, s.table_commit.clone())
        });
        let elided = lock(&self.table)
            .clone()
            .map_or(0, |d| lock(d.as_ref()).elided_loci());
        let (table, cert) = match sealed {
            Some((t, c)) => (t, c),
            None => (
                TableOutput {
                    path: RunLayout::out_rel(TABLE),
                    bytes: None,
                    blake3: None,
                    records: None,
                    rules: None,
                    rules_committed: rules,
                    committed_bytes: commit.bytes,
                    prefix_blake3: Some(commit.blake3.clone()),
                    sealed: false,
                    end: None,
                    canonical_blake3: None,
                    witness_loci_elided: elided,
                },
                None,
            ),
        };
        let queue_bytes = st.map_or(0, |s| s.queue_commit.bytes);
        j.outputs = Outputs {
            table: Some(table),
            cert,
            queue: Some(QueueOutput { bytes: queue_bytes }),
            events: Some(EventsOutput {
                last_seq: self.sink.next_seq(),
            }),
            resume: Some(ResumeOutput {
                path: RESUME.to_string(),
                valid: st.is_some_and(|s| s.resume_valid),
            }),
            extra: vec![],
        };
        let wall = self.started.elapsed().as_secs_f64();
        j.timing = Timing {
            started_utc: Some(self.started_utc.clone()),
            finished_utc: Some(cc::time::utc_now()),
            wall_s: Some((wall * 1000.0).round() / 1000.0),
            cpu_s: Some((proc::cpu_seconds() * 1000.0).round() / 1000.0),
        };
        j.memory = Memory {
            vmhwm_kb: Some(super::driver::vmhwm_kb()),
            rss_kb_at_end: Some(proc::rss_kb(0)),
            helpers_peak_rss_kb: Some(helpers_peak),
        };
        j.error = error;
        j.resumed = self.spec.resume.is_some();
        let mut o = Outcome::new(j);
        o.legacy = legacy;
        o
    }

    fn write_outcome(&self, o: &Outcome) -> io::Result<()> {
        if let Err(e) = o.job.check() {
            eprintln!("rr_node job: OUTCOME self-check: {e}");
        }
        o.write_new(&self.layout.outcome())
    }

    /// The hard-safe stop (3.4.3): kill and reap helpers, leave the streams alone, OUTCOME from the last commit
    /// point, try `job_finished` for 1 s, `_exit`.
    fn hard_stop(&self, code: ExitCode, stop: StopCode, td: Option<TimeDecision>) -> ! {
        if FINAL.swap(true, SeqCst) {
            park_forever();
        }
        let peak = proc::kill_helpers();
        let src = *lock(&self.stop_source);
        let mut g = self.st_try();
        if let (Some(td), Some(g)) = (td.clone(), g.as_mut()) {
            g.time_decisions.push(td);
        }
        let info = StopInfo {
            code: Some(stop),
            detail: Value::Null,
            source: src,
            graceful: false,
        };
        let mut o = self.outcome(
            g.as_deref(),
            code,
            info,
            None,
            None,
            serde_json::Map::new(),
            peak,
        );
        drop(g);
        if g_none_time(&o, &td) {
            o.job.time_decided = true;
        }
        let _ = self.write_outcome(&o);
        self.sink.try_emit_for(
            Event::JobFinished {
                exit_code: code,
                status: JobStatus::for_exit(code),
                stop_code: Some(stop),
            },
            Duration::from_secs(1),
        );
        proc::exit_now(code.code())
    }
}

/// A wall stop whose state lock could not be taken still reports `time_decided`.
fn g_none_time(o: &Outcome, td: &Option<TimeDecision>) -> bool {
    td.is_some() && o.job.time_decisions.is_empty()
}

/// Result of a case without a rule, from the queue-log records written while it ran.
fn case_result(types: &[&str]) -> CaseResult {
    let has = |names: &[&str]| types.iter().any(|t| names.contains(t));
    if has(&["empty_domain", "initial_empty"]) {
        CaseResult::Empty
    } else if has(&["finite_domain"]) {
        CaseResult::Finite
    } else if has(&["declared_terminal"]) {
        CaseResult::DeclaredTerminal
    } else if has(&[
        "covered",
        "union_covered",
        "numerical_covered",
        "subsumed_by_pending",
    ]) {
        CaseResult::Covered
    } else if has(&[
        "lift_refused",
        "screen_refused",
        "guard_timeout",
        "refused",
        "numerical_exhausted",
        "rule_oversized",
    ]) {
        CaseResult::Refused
    } else {
        CaseResult::Error
    }
}

fn park_forever() -> ! {
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}

/// The watchdog thread (3.4.3, 3.4.5, 3.5, 3.11): signals, STOP file, wall budgets, soft memory limit, stop grace,
/// heartbeat. It reads only atomics and short-held state; it never touches solver state.
fn watchdog(j: Arc<Job>) {
    let mut seen = 0u32;
    let mut last_hb = Instant::now();
    let mut last_stopfile = Instant::now() - Duration::from_secs(2);
    let hb = Duration::from_secs(j.spec.heartbeat_s as u64);
    let grace = Duration::from_secs(j.spec.stop_grace_s as u64);
    loop {
        std::thread::sleep(Duration::from_millis(20));
        let (n, last) = proc::signals();
        if n > seen {
            seen = n;
            let src = match last {
                libc::SIGINT => StopSource::Sigint,
                libc::SIGHUP => StopSource::Sighup,
                _ => StopSource::Sigterm,
            };
            if n >= 2 && j.stop.load(SeqCst) {
                let (code, stop) = stop_codes(&j);
                j.hard_stop(code, stop, None);
            }
            j.request_stop(src);
        }
        if last_stopfile.elapsed() >= Duration::from_secs(1) {
            last_stopfile = Instant::now();
            if j.layout.stop_file().exists() {
                j.request_stop(StopSource::StopFile);
            }
        }
        let w = j.spec.budgets.wall;
        if let Some(limit) = w.job_s {
            let el = j.started.elapsed();
            if el >= Duration::from_secs(limit) {
                let td = TimeDecision {
                    what: TimeDecisionWhat::JobWall,
                    case_ordinal: u64::try_from(j.case_ordinal.load(Relaxed)).ok(),
                    rule: None,
                    limit_s: limit as f64,
                    measured_s: el.as_secs_f64(),
                    taken: "hard_stop".into(),
                };
                j.hard_stop(ExitCode::BudgetWall, StopCode::BudgetWall, Some(td));
            }
        }
        if let Some(limit) = w.case_s {
            let t0 = j.case_started_ms.load(Relaxed);
            if t0 >= 0 {
                let el = j.started.elapsed().as_millis() as i64 - t0;
                if el >= (limit as i64) * 1000 {
                    let td = TimeDecision {
                        what: TimeDecisionWhat::CaseWall,
                        case_ordinal: u64::try_from(j.case_ordinal.load(Relaxed)).ok(),
                        rule: None,
                        limit_s: limit as f64,
                        measured_s: el as f64 / 1000.0,
                        taken: "hard_stop".into(),
                    };
                    j.hard_stop(ExitCode::BudgetWall, StopCode::BudgetWall, Some(td));
                }
            }
        }
        if let Some(lim) = j.spec.budgets.mem_soft_bytes {
            let kb = proc::rss_kb(0)
                + proc::live_helpers()
                    .iter()
                    .map(|&p| proc::rss_kb(p))
                    .sum::<u64>();
            if kb * 1024 > lim {
                j.request_stop(StopSource::Memory);
            }
        }
        if j.stop.load(SeqCst) {
            let at = *lock(&j.stop_at);
            if at.is_some_and(|t| t.elapsed() >= grace) {
                let (code, stop) = stop_codes(&j);
                j.hard_stop(code, stop, None);
            }
        }
        if last_hb.elapsed() >= hb {
            last_hb = Instant::now();
            let (pending, rules, counters) = {
                let st = j.st();
                (st.pending, st.rules_committed, j.counters(Some(&st)))
            };
            let _ = j.sink.emit(Event::Heartbeat {
                phase: j.phase(),
                case_ordinal: u64::try_from(j.case_ordinal.load(Relaxed)).ok(),
                pending,
                rules_committed: rules,
                counters,
                rss_kb: proc::rss_kb(0),
                vmhwm_kb: super::driver::vmhwm_kb(),
                helpers: proc::live_helpers().len() as u64,
                stop_requested: j.stop.load(SeqCst),
            });
        }
    }
}

fn stop_codes(j: &Job) -> (ExitCode, StopCode) {
    if *lock(&j.stop_source) == Some(StopSource::Memory) {
        (ExitCode::BudgetMemory, StopCode::BudgetMemory)
    } else {
        (ExitCode::Interrupted, StopCode::Interrupted)
    }
}

// -------------------------------------------------------------------------------------------------------------
// the `generate` job

/// Error OUTCOME of a refused start (after the run directory was verified), no-clobber; then exit.
pub fn refuse_exit(r: cc::preflight::Refusal, program: &ProgramInfo) -> ! {
    eprintln!("rr_node job: {}", r.message);
    if r.write_outcome {
        if let Some((spec, digest)) = &r.spec {
            let stop = StopInfo {
                code: Some(r.stop),
                detail: Value::Null,
                source: None,
                graceful: true,
            };
            let mut j = JobOutcome::for_spec(spec, digest, program.clone(), r.code, stop);
            j.error = Some(ErrorInfo::new(r.class, r.message.clone()));
            let _ = Outcome::new(j).write_new(&RunLayout::new(&spec.run_dir).outcome());
        }
    }
    std::process::exit(r.code.code())
}

/// A usage / setup failure once the job context exists (e.g. the legacy flag parser's `die`): error OUTCOME with
/// the code and exit. No-op (returns) in legacy mode.
pub fn fail_now(code: ExitCode, message: &str) {
    let Some(j) = job() else { return };
    let (stop, class) = match code {
        ExitCode::Usage => (StopCode::Refused, ErrorClass::Usage),
        ExitCode::Input => (StopCode::InputError, ErrorClass::Input),
        ExitCode::Refused => (StopCode::Refused, ErrorClass::Refused),
        ExitCode::NoSpace => (StopCode::Nospace, ErrorClass::Nospace),
        ExitCode::Io => (StopCode::IoError, ErrorClass::Io),
        _ => (StopCode::InternalError, ErrorClass::Internal),
    };
    eprintln!("rr_node job: {message}");
    if !FINAL.swap(true, SeqCst) {
        let peak = proc::kill_helpers();
        let info = StopInfo {
            code: Some(stop),
            detail: Value::Null,
            source: None,
            graceful: true,
        };
        let o = {
            let st = j.st();
            j.outcome(
                Some(&st),
                code,
                info,
                None,
                Some(ErrorInfo::new(class, message)),
                serde_json::Map::new(),
                peak,
            )
        };
        let _ = j.write_outcome(&o);
        let _ = j.sink.emit(Event::JobFinished {
            exit_code: code,
            status: JobStatus::for_exit(code),
            stop_code: Some(stop),
        });
        let _ = cc::atomic::release_reserve(&j.layout.reserve());
    }
    std::process::exit(code.code())
}

/// [`fail_now`] in job mode, never returning.
pub fn fail(code: ExitCode, message: &str) -> ! {
    fail_now(code, message);
    std::process::exit(code.code())
}

/// What the bin hands over to build the node options from the spec's `args` with the legacy parser.
pub struct Resolved {
    pub options: super::driver::NodeOptions,
    pub family_text: String,
    pub family_path: String,
}

/// The behaviour-changing environment variables of child-job.md 2.5 that are set (licence: presence only).
fn effective_env() -> std::collections::BTreeMap<String, String> {
    let mut m = std::collections::BTreeMap::new();
    for k in [
        "RUSTRED_PROBE_POINT",
        "RUSTRED_TRACE_DUMP",
        "RUSTRED_TRACE_DUMP_MIN_ROWS",
        "RUSTRED_TRACE_DUMP_EXIT",
        "RR_REPLAY_RECENTER_SEARCH",
        "RR_REPLAY_RECENTER_LIMIT",
        "RUSTRED_NODE_PROGRESS",
        "RAYON_NUM_THREADS",
        "OMP_NUM_THREADS",
    ] {
        if let Ok(v) = std::env::var(k) {
            m.insert(k.to_string(), v);
        }
    }
    if std::env::var_os("SYMBOLICA_LICENSE").is_some() {
        m.insert(
            "SYMBOLICA_LICENSE".into(),
            cc::outcome::LICENCE_PRESENT.into(),
        );
    }
    m
}

/// `program.exe` of OUTCOME.
pub fn exe_info() -> Option<ExeInfo> {
    use std::os::unix::fs::MetadataExt;
    let p = std::fs::read_link("/proc/self/exe").ok()?;
    let m = std::fs::metadata(&p).ok()?;
    Some(ExeInfo {
        path: p.display().to_string(),
        dev: m.dev(),
        ino: m.ino(),
        size: m.size(),
        mtime: m.mtime(),
    })
}

/// Run one `generate` job whose start-up checks passed (`start`). `resolve` builds the node options from the
/// spec's `args` with the SAME parser as the legacy command line (it may end the process through [`fail_now`]).
/// Never returns.
pub fn run_generate(start: JobStart, resolve: &dyn Fn(&JobSpec) -> Resolved) -> ! {
    let JobStart {
        spec,
        spec_blake3,
        layout,
        inputs,
        program,
        resume,
    } = start;
    let family_blake3 = spec
        .inputs
        .iter()
        .find(|i| i.role == "family")
        .map(|i| i.blake3.clone());
    // out/ and tmp/ (preflight verified out/ absent or empty)
    for d in [layout.out_dir(), layout.tmp_dir()] {
        if let Err(e) = std::fs::create_dir_all(&d) {
            eprintln!("rr_node job: {}: {e}", d.display());
            std::process::exit(cc::atomic::exit_for_io(&e).code());
        }
    }
    // resume: copy the committed prefixes (verified again WHILE copying, into a temporary name renamed only when
    // complete) before anything else is written; a refusal here leaves out/ without tables (exit 41)
    let resumed_from = resume.as_ref().map(|v| v.state.attempt);
    if let Some(v) = &resume {
        if let Err(m) = copy_resume(v, &layout) {
            refuse_exit(
                cc::preflight::Refusal {
                    code: ExitCode::Input,
                    stop: StopCode::InputError,
                    class: ErrorClass::Input,
                    message: format!("resume: {m}"),
                    spec: Some((Box::new(spec.clone()), spec_blake3.clone())),
                    write_outcome: true,
                },
                &program,
            );
        }
    }
    if let Err(e) = cc::atomic::create_reserve(&layout.reserve(), cc::layout::RESERVE_BYTES) {
        eprintln!("rr_node job: reserve: {e}");
        let _ = cc::atomic::release_reserve(&layout.reserve());
        std::process::exit(ExitCode::NoSpace.code());
    }
    let sink = match EventSink::create(&layout.events()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("rr_node job: EVENTS: {e}");
            std::process::exit(cc::atomic::exit_for_io(&e).code());
        }
    };
    let rel_t = RunLayout::out_rel(TABLE);
    let rel_q = RunLayout::out_rel(QUEUE);
    let j = Arc::new(Job {
        family_blake3,
        program: program.clone(),
        sink,
        started: Instant::now(),
        started_utc: cc::time::utc_now(),
        table: Mutex::new(None),
        queue: Mutex::new(None),
        cert: Mutex::new(None),
        effective: Mutex::new(None),
        st: Mutex::new(State {
            table_commit: empty_commit(&rel_t),
            queue_commit: empty_commit(&rel_q),
            rules_committed: 0,
            symbolic_cases: 0,
            per_case: vec![],
            in_flight: None,
            ordinal: 0,
            pending: 0,
            time_decisions: vec![],
            resume_written_at: None,
            resume_valid: false,
            resume_current: None,
            engine: [0; 5],
            returned: vec![],
            open_list: vec![],
            open_on_end: false,
        }),
        stop: AtomicBool::new(false),
        stop_source: Mutex::new(None),
        stop_at: Mutex::new(None),
        counters: Default::default(),
        phase: AtomicU8::new(Phase::Prepare as u8),
        case_started_ms: AtomicI64::new(-1),
        case_ordinal: AtomicI64::new(-1),
        spec,
        spec_blake3,
        layout,
        inputs,
    });
    if JOB.set(j.clone()).is_err() {
        eprintln!("rr_node job: one job per process");
        std::process::exit(ExitCode::Internal.code());
    }
    if let Some(v) = &resume {
        restore(&j, v);
    }
    // panic messages are recorded (the engine itself relies on catch_unwind, so the hook never exits)
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Ok(mut m) = PANIC_MESSAGE.lock() {
            if m.is_none() {
                *m = Some(info.to_string());
            }
        }
        default_hook(info);
    }));
    let _ = j.sink.emit(Event::JobStarted {
        job_id: j.spec.job_id.clone(),
        attempt: j.spec.attempt,
        spec_blake3: j.spec_blake3.clone(),
        pid: std::process::id(),
        program: PROGRAM.into(),
        version: program.version.clone(),
        git_commit: program.git_commit.clone(),
        resume_from: resumed_from,
    });
    let wd = j.clone();
    std::thread::Builder::new()
        .name("rr-job-watchdog".into())
        .spawn(move || watchdog(wd))
        .unwrap_or_else(|e| fail(ExitCode::Internal, &format!("watchdog thread: {e}")));
    // refusals of what this build does not implement (fail closed: a limit that is not enforced is refused)
    unsupported_or_exit(&j.spec);
    let r = resolve(&j.spec);
    let opts = r.options;
    *lock(&j.effective) = Some({
        let node_options = serde_json::to_value(&opts).unwrap_or(Value::Null);
        let order = serde_json::to_value(&opts.order).unwrap_or(Value::Null);
        let env = effective_env();
        let mut e = Effective {
            subcommand: "generate".into(),
            args: j.spec.args.clone(),
            node_options,
            order,
            env,
            blake3: String::new(),
        };
        let bytes = serde_json::to_vec(&e).unwrap_or_default();
        e.blake3 = cc::canon::canonical_line(&bytes)
            .map(|c| cc::blake3_hex(&c))
            .unwrap_or_else(|_| cc::blake3_hex(&bytes));
        e
    });
    j.set_phase(Phase::ZeroCensus);
    let out_dir = j.layout.out_dir();
    let jj = j.clone();
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        super::driver::generate(&r.family_text, &r.family_path, &out_dir, &opts, &mut |e| {
            if let Ok(v) = serde_json::to_value(e) {
                let _ = jj.sink.emit(Event::Engine { event: v });
            }
        })
    }));
    finish(&j, res)
}

static PANIC_MESSAGE: Mutex<Option<String>> = Mutex::new(None);

/// Write `out/F.subcases.jsonl` (records `{"type":"subcase","kind":"case|numeric|region|refused","case":{..}}` in
/// PRESENTED slots, then `{"type":"end","subcases":N}`) and return its OUTCOME entry (role `subcases`).
fn write_subcases(
    j: &Job,
    outcome: &super::driver::NodeOutcome,
) -> Option<cc::outcome::ExtraOutput> {
    let mut list: Vec<(&'static str, JobCase)> = Vec::new();
    {
        let st = j.st();
        list.extend(st.returned.iter().cloned());
        if outcome.stop.is_some() || st.open_on_end {
            list.extend(st.open_list.iter().cloned());
        }
    }
    match &outcome.stop {
        Some(StopReason::LiftRefused { case, .. })
        | Some(StopReason::GuardTimeout { case, .. }) => {
            list.push(("refused", job_case(case)));
        }
        Some(StopReason::NumericSearchRefused { cases }) => {
            list.extend(cases.iter().map(|c| ("numeric", job_case(c))));
        }
        Some(StopReason::OpenRegions { charts, .. }) => {
            list.retain(|(k, _)| *k != "region");
            list.extend(charts.iter().map(|c| ("region", job_case(c))));
        }
        _ => {}
    }
    let rel = RunLayout::out_rel(cc::layout::SUBCASES);
    let mut w = cc::seal::RecordWriter::create(&j.layout.subcases()).ok()?;
    for (kind, c) in &list {
        let line = serde_json::json!({"type": "subcase", "kind": kind, "case": c});
        w.append(&serde_json::to_vec(&line).ok()?).ok()?;
    }
    let end = serde_json::json!({"type": "end", "subcases": list.len()});
    let seal = w.seal(&serde_json::to_vec(&end).ok()?).ok()?;
    Some(cc::outcome::ExtraOutput {
        role: "subcases".into(),
        path: rel,
        bytes: seal.bytes,
        blake3: seal.blake3,
        records: Some(seal.records),
        canonical_blake3: Some(seal.canonical_blake3),
        end: Some(seal.end),
        sealed: true,
    })
}

/// A resumed attempt: the commit state, the counters and the queue of the previous attempt's RESUME.
fn restore(j: &Job, v: &cc::preflight::VerifiedResume) {
    let r = &v.state;
    {
        let mut st = j.st();
        st.table_commit = r.table.clone();
        if let Some(q) = &r.queue {
            st.queue_commit = q.clone();
        }
        st.rules_committed = r.rules_committed;
        st.symbolic_cases = r.symbolic_cases;
        st.pending = r.pending.len() as u64;
    }
    for (i, w) in WorkCounter::ALL.iter().enumerate() {
        let c = &r.counters_at_commit;
        let val = match w {
            WorkCounter::Cases => 0,
            WorkCounter::TraceRows => c.trace_rows_max,
            _ => c.get(*w),
        };
        j.counters[i].store(val, Relaxed);
    }
    let rules = match resume_rules(&j.layout.table()) {
        Ok(r) => r,
        Err(m) => fail(ExitCode::Input, &format!("resume: committed table: {m}")),
    };
    let spec_of = |c: &JobCase| CaseSpec {
        fixed: c.fixed.clone(),
        equations: c
            .equations
            .iter()
            .map(|e| {
                e.as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| e.to_string())
            })
            .collect(),
    };
    let engine = |k: &str| r.engine.get(k).and_then(Value::as_u64).unwrap_or(0) as usize;
    let _ = RESUMED.set(Resumed {
        cases: r
            .current
            .iter()
            .chain(r.pending.iter())
            .map(spec_of)
            .collect(),
        numerical: r.numerical.clone(),
        rules,
        symbolic_cases: r.symbolic_cases as usize,
        counters: [
            engine(ENGINE_KEYS[0]),
            engine(ENGINE_KEYS[1]),
            r.discarded as usize,
            engine(ENGINE_KEYS[3]),
            engine(ENGINE_KEYS[4]),
        ],
        deferred: r
            .deferred
            .iter()
            .filter_map(|d| {
                let c = serde_json::from_value::<JobCase>(d["chart"].clone()).ok()?;
                Some((spec_of(&c), d.get("detail").cloned().unwrap_or(Value::Null)))
            })
            .collect(),
        refused_open: r
            .refused_open
            .iter()
            .filter_map(|d| {
                let c = serde_json::from_value::<JobCase>(d["case"].clone()).ok()?;
                Some((spec_of(&c), d.get("detail").cloned().unwrap_or(Value::Null)))
            })
            .collect(),
        prefix: true,
    });
    SUPPRESS.store(true, SeqCst);
}

/// Copy the committed table and queue prefixes of the previous attempt into this run directory
/// (`seal::RecordWriter::resume_from`: every figure verified while copying).
fn copy_resume(v: &cc::preflight::VerifiedResume, layout: &RunLayout) -> Result<(), String> {
    cc::seal::RecordWriter::resume_from(
        &v.prev.root,
        &v.state.table,
        &TABLE_END_TYPES,
        &layout.table(),
    )
    .map_err(|e| format!("table prefix: {e}"))?;
    match &v.state.queue {
        Some(q) => {
            cc::seal::RecordWriter::resume_from(&v.prev.root, q, &TABLE_END_TYPES, &layout.queue())
                .map_err(|e| format!("queue prefix: {e}"))?;
        }
        None => {
            cc::seal::RecordWriter::create(&layout.queue()).map_err(|e| format!("queue: {e}"))?;
        }
    }
    Ok(())
}

/// The rule records of the copied committed table prefix, in file order.
fn resume_rules(table: &Path) -> Result<Vec<Value>, String> {
    let text = std::fs::read_to_string(table).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for l in text.lines() {
        let v: Value = serde_json::from_str(l).map_err(|e| e.to_string())?;
        if v["type"] == "rule" {
            out.push(v);
        }
    }
    Ok(out)
}

fn unsupported_or_exit(spec: &JobSpec) {
    use cc::spec::{Determinism, JobKind};
    if spec.kind != JobKind::Generate {
        fail(ExitCode::Refused, "this build runs job kind generate only");
    }
    if spec.determinism == Determinism::Strict {
        fail(
            ExitCode::Refused,
            "determinism strict is not supported by this build",
        );
    }
    if spec.seed.sample_seed != 0 {
        fail(
            ExitCode::Refused,
            "seed.sample_seed other than 0 is not supported by this build",
        );
    }
    for (c, _, _) in spec.budgets.work.limits() {
        if c != WorkCounter::Cases {
            fail(
                ExitCode::Refused,
                &format!("work budget {c:?} is not enforced by this build (refused, fail closed)"),
            );
        }
    }
    let fam = spec.inputs.iter().filter(|i| i.role == "family").count();
    if fam != 1 {
        fail(
            ExitCode::Usage,
            "a generate job needs exactly one input of role family",
        );
    }
    for i in &spec.inputs {
        if !matches!(i.role.as_str(), "family" | "cases" | "cover") {
            fail(
                ExitCode::Usage,
                &format!("input role {:?} is not known to generate", i.role),
            );
        }
        if i.role == "cover" && i.upto.is_none() {
            fail(ExitCode::Usage, "a cover input needs upto");
        }
    }
}

/// The cases input (3.8.3): ORIGINAL-frame case lines become the node's initial cases (returned); PRESENTED cases
/// and spill lines replace the driver's initial queue at the resume point (feature `initial_presented`). A PRESENTED
/// case's optional `seed_centre_offset` (`[ints]` or `[[ints], ..]`, PRESENTED slots; engine line, feature
/// `case_seed_centres`) becomes a per-case seed centre entry (returned second; the driver applies it to every case
/// contained in that case, its exception children included), exactly as `--initial-presented` does.
pub fn initial_cases(spec: &JobSpec) -> (Vec<CaseSpec>, Vec<super::driver::CaseSeedCentre>) {
    let mut out = Vec::new();
    let mut centres: Vec<super::driver::CaseSeedCentre> = Vec::new();
    let mut presented = PresentedCases::default();
    for i in spec.inputs.iter().filter(|i| i.role == "cases") {
        let p = spec.input_path(i);
        let text = std::fs::read_to_string(&p)
            .unwrap_or_else(|e| fail(ExitCode::Input, &format!("cases {}: {e}", p.display())));
        for (k, line) in text
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
        {
            match cc::resume::parse_cases_line(line) {
                Ok(cc::resume::CasesLine::Case(c)) if c.frame == Frame::Original => {
                    let eqs: Vec<String> = c
                        .equations
                        .iter()
                        .map(|e| {
                            e.as_str()
                                .map(str::to_string)
                                .unwrap_or_else(|| e.to_string())
                        })
                        .collect();
                    if c.seed_centre_offset.is_some() {
                        fail(
                            ExitCode::Refused,
                            "seed_centre_offset is given in PRESENTED slots: write the case in frame PRESENTED",
                        );
                    }
                    out.push(CaseSpec {
                        fixed: c.fixed,
                        equations: eqs,
                    });
                    continue;
                }
                Ok(cc::resume::CasesLine::Case(c)) => presented.cases.push(c),
                Ok(cc::resume::CasesLine::Spill(sp)) => {
                    presented.cases.extend(sp.current);
                    presented.cases.extend(sp.pending);
                    presented.numerical.extend(sp.numerical);
                }
                Err(e) => fail(ExitCode::Input, &format!("cases line {}: {e}", k + 1)),
            }
        }
    }
    if !presented.cases.is_empty() || !presented.numerical.is_empty() {
        if !out.is_empty() {
            fail(
                ExitCode::Usage,
                "ORIGINAL and PRESENTED cases cannot be mixed in one job",
            );
        }
        for c in &presented.cases {
            if let Some(v) = c.seed_centre_offset.as_ref().filter(|v| !v.is_null()) {
                centres.push(case_seed_centre(c, v));
            }
        }
        // PRESENTED cases replace the driver's initial queue at the resume point (a resumed attempt keeps the
        // queue of its RESUME instead)
        if RESUMED.get().is_none() {
            let _ = RESUMED.set(Resumed {
                cases: presented.cases.iter().map(driver_case).collect(),
                numerical: presented.numerical,
                rules: vec![],
                symbolic_cases: 0,
                counters: [0; 5],
                deferred: vec![],
                refused_open: vec![],
                prefix: false,
            });
        }
    }
    (out, centres)
}

/// One per-case seed centre entry from a PRESENTED case line (the legacy `--initial-presented` parsing): one centre
/// `[ints]` or several `[[ints], ..]` (union of their shells), each of the case's arity.
fn case_seed_centre(c: &JobCase, v: &Value) -> super::driver::CaseSeedCentre {
    let mut offs: Vec<Vec<i64>> = match serde_json::from_value::<Vec<i64>>(v.clone()) {
        Ok(one) => vec![one],
        Err(_) => serde_json::from_value(v.clone())
            .unwrap_or_else(|e| fail(ExitCode::Input, &format!("cases: seed_centre_offset: {e}"))),
    };
    if offs.is_empty() || offs.iter().any(|o| o.len() != c.fixed.len()) {
        fail(
            ExitCode::Input,
            &format!(
                "cases: seed_centre_offset must be one or more offsets of the case arity {}",
                c.fixed.len()
            ),
        );
    }
    let first = offs.remove(0);
    super::driver::CaseSeedCentre {
        case: driver_case(c),
        seed_centre_offset: first,
        extra: offs,
    }
}

/// PRESENTED cases and spill lines of the cases input (3.8.3).
#[derive(Default)]
struct PresentedCases {
    cases: Vec<JobCase>,
    numerical: Vec<Vec<i64>>,
}

fn driver_case(c: &JobCase) -> CaseSpec {
    CaseSpec {
        fixed: c.fixed.clone(),
        equations: c
            .equations
            .iter()
            .map(|e| {
                e.as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| e.to_string())
            })
            .collect(),
    }
}

/// The cover inputs as `cover_tables` (path, upto).
pub fn cover_tables(spec: &JobSpec) -> Vec<(String, usize)> {
    spec.inputs
        .iter()
        .filter(|i| i.role == "cover")
        .map(|i| {
            (
                spec.input_path(i).display().to_string(),
                i.upto.unwrap_or(0) as usize,
            )
        })
        .collect()
}

/// The family input: (text, path).
pub fn family_input(spec: &JobSpec) -> (String, String) {
    let i = spec
        .inputs
        .iter()
        .find(|i| i.role == "family")
        .expect("checked: one family input");
    let p: PathBuf = spec.input_path(i);
    match std::fs::read_to_string(&p) {
        Ok(t) => (t, p.display().to_string()),
        Err(e) => fail(ExitCode::Input, &format!("family {}: {e}", p.display())),
    }
}

type GenResult = std::thread::Result<Result<super::driver::NodeOutcome, String>>;

/// Normal end, graceful stop, engine error or caught panic: seals, final RESUME, OUTCOME, exit.
fn finish(j: &Arc<Job>, res: GenResult) -> ! {
    j.set_phase(Phase::Finishing);
    let table_d = lock(&j.table).clone();
    let errno = IO_ERRNO.load(Relaxed);
    let io_class = |m: &str| -> Option<(ExitCode, String)> {
        (errno != 0).then(|| {
            let e = if errno > 0 {
                io::Error::from_raw_os_error(errno)
            } else {
                io::Error::other(m.to_string())
            };
            (cc::atomic::exit_for_io(&e), format!("{m} ({e})"))
        })
    };
    let outcome = match res {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            if let Some((code, m)) = io_class(&e) {
                if code == ExitCode::NoSpace {
                    let _ = cc::atomic::release_reserve(&j.layout.reserve());
                }
                fail(code, &m);
            }
            // an error before the table exists is an input problem (family, chart, order, two-sided family)
            let code = if table_d.is_none() {
                ExitCode::Input
            } else {
                ExitCode::Internal
            };
            fail(code, &e)
        }
        Err(_) => {
            let m = lock(&PANIC_MESSAGE)
                .clone()
                .unwrap_or_else(|| "panic".into());
            if FINAL.swap(true, SeqCst) {
                park_forever();
            }
            let peak = proc::kill_helpers();
            let info = StopInfo {
                code: Some(StopCode::Panic),
                detail: Value::Null,
                source: None,
                graceful: false,
            };
            let o = {
                let st = j.st();
                j.outcome(
                    Some(&st),
                    ExitCode::Internal,
                    info,
                    None,
                    Some(ErrorInfo::new(ErrorClass::Panic, m)),
                    serde_json::Map::new(),
                    peak,
                )
            };
            let _ = j.write_outcome(&o);
            let _ = j.sink.emit(Event::JobFinished {
                exit_code: ExitCode::Internal,
                status: JobStatus::Error,
                stop_code: Some(StopCode::Panic),
            });
            std::process::exit(ExitCode::Internal.code());
        }
    };
    if let Some((code, m)) = io_class("output write failed") {
        if code == ExitCode::NoSpace {
            let _ = cc::atomic::release_reserve(&j.layout.reserve());
        }
        fail(code, &m);
    }
    // exit code and stop from the engine's outcome
    let src = *lock(&j.stop_source);
    let (code, stop, error, source) = match &outcome.stop {
        None if outcome.complete => (ExitCode::Complete, None, None, None),
        None => (ExitCode::Incomplete, Some(StopCode::NotLower), None, None),
        Some(StopReason::MaxCases { .. }) => {
            (ExitCode::Incomplete, Some(StopCode::MaxCases), None, None)
        }
        Some(StopReason::LiftRefused { .. }) => (
            ExitCode::Incomplete,
            Some(StopCode::LiftRefused),
            None,
            None,
        ),
        Some(StopReason::GuardTimeout { .. }) => (
            ExitCode::Incomplete,
            Some(StopCode::GuardTimeout),
            None,
            None,
        ),
        Some(StopReason::OpenRegions { .. }) => (
            ExitCode::Incomplete,
            Some(StopCode::OpenRegions),
            None,
            None,
        ),
        Some(StopReason::BeforeNumeric { .. }) => (
            ExitCode::Incomplete,
            Some(StopCode::BeforeNumeric),
            None,
            None,
        ),
        Some(StopReason::OpenObligations { .. }) => (
            ExitCode::Incomplete,
            Some(StopCode::OpenObligations),
            None,
            None,
        ),
        Some(StopReason::NumericSearchRefused { .. }) => (
            ExitCode::Incomplete,
            Some(StopCode::NumericSearchRefused),
            None,
            None,
        ),
        Some(StopReason::Error(m)) => (
            ExitCode::Internal,
            Some(StopCode::InternalError),
            Some(ErrorInfo::new(ErrorClass::Internal, m.clone())),
            None,
        ),
        Some(StopReason::Descended { .. }) => {
            (ExitCode::Incomplete, Some(StopCode::MaxCases), None, None)
        }
        Some(StopReason::Interrupted { .. }) => {
            let (c, s) = stop_codes(j);
            (c, Some(s), None, src)
        }
    };
    // seals of the table and the cert
    let table = table_d.as_ref().map(|d| {
        let d = lock(d.as_ref());
        let st = j.st();
        let seal = d.seal();
        TableOutput {
            path: RunLayout::out_rel(TABLE),
            bytes: seal.as_ref().map(|s| s.bytes),
            blake3: seal.as_ref().map(|s| s.blake3.clone()),
            records: seal.as_ref().map(|s| s.records),
            rules: seal.as_ref().map(|s| s.rules),
            rules_committed: seal.as_ref().map_or(st.rules_committed, |s| s.rules),
            committed_bytes: st.table_commit.bytes,
            prefix_blake3: Some(st.table_commit.blake3.clone()),
            sealed: seal.is_some(),
            end: seal.as_ref().map(|s| s.end),
            canonical_blake3: seal.as_ref().map(|s| s.canonical_blake3.clone()),
            witness_loci_elided: d.elided_loci(),
        }
    });
    let cert = lock(&j.cert).clone().map(|d| {
        let d = lock(d.as_ref());
        let seal = d.sealed_figures(&RunLayout::out_rel(CERT));
        StreamOutput {
            path: RunLayout::out_rel(CERT),
            bytes: seal.as_ref().map(|s| s.bytes),
            blake3: seal.as_ref().map(|s| s.blake3.clone()),
            sealed: seal.is_some(),
        }
    });
    // the per-case of the last case if the queue ended without a later safe point
    {
        let mut st = j.st();
        if let Some(f) = st.in_flight.take() {
            let rules = st.rules_committed;
            st.per_case.push(PerCase {
                ordinal: f.ordinal,
                case: f.case,
                result: if rules > f.rules_before {
                    CaseResult::Rule
                } else {
                    CaseResult::Error
                },
                rule: (rules > f.rules_before).then_some(f.rules_before),
                children_open: 0,
            });
        }
        if let Some(t) = &table {
            st.rules_committed = st.rules_committed.max(t.rules.unwrap_or(0));
        }
    }
    // final RESUME (every normal end: the open list; an interrupted stop wrote its own at the safe point)
    if stop != Some(StopCode::Interrupted) && stop != Some(StopCode::BudgetMemory) {
        let pending = {
            let st = j.st();
            // the open list as the engine reports it: the pending count only (case lists are in the queue log)
            st.pending
        };
        let _ = pending;
        if !j.st().resume_valid {
            j.write_resume(
                None,
                OpenQueue {
                    pending: vec![],
                    numerical: vec![],
                    deferred: vec![],
                    refused: vec![],
                },
                true,
            );
        }
    }
    // the open list, sealed (F.subcases.jsonl): returned sub-cases, and for a stopped job its open cases
    let subcases = write_subcases(j, &outcome);
    let legacy = match serde_json::to_value(&outcome) {
        Ok(Value::Object(m)) => m,
        _ => serde_json::Map::new(),
    };
    if FINAL.swap(true, SeqCst) {
        park_forever();
    }
    let peak = proc::kill_helpers();
    let info = StopInfo {
        code: stop,
        detail: outcome
            .stop
            .as_ref()
            .and_then(|s| serde_json::to_value(s).ok())
            .unwrap_or(Value::Null),
        source,
        graceful: true,
    };
    let mut o = {
        let st = j.st();
        let sealed = table.map(|t| (t, cert));
        j.outcome(Some(&st), code, info, sealed, error, legacy, peak)
    };
    if let Some(x) = subcases {
        o.job.outputs.extra.push(x);
    }
    if let Err(e) = j.write_outcome(&o) {
        let _ = cc::atomic::release_reserve(&j.layout.reserve());
        eprintln!("rr_node job: OUTCOME: {e}");
        std::process::exit(cc::atomic::exit_for_io(&e).code());
    }
    let _ = j.sink.emit(Event::JobFinished {
        exit_code: code,
        status: JobStatus::for_exit(code),
        stop_code: stop,
    });
    let _ = cc::atomic::release_reserve(&j.layout.reserve());
    std::process::exit(code.code())
}
