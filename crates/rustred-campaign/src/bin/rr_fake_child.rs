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
//!
//! Independence (audit A M6): the fake child has its OWN minimal implementation of the table / queue writer with its
//! running digests, the seal, the canonical record form (a serde_json `Value` serialiser, see [`own`]) and the
//! OUTCOME writer (JSON built field by field, no-clobber write), and does NOT call the contract crate's
//! `RecordWriter`, `canonical_*`, `TableSeal`, `Outcome::write_*` or `JobOutcome::check`. Every process test that
//! classifies a fake run with the crate's verifier is therefore a DIFFERENTIAL test of the verifier. Still shared:
//! spec parsing and preflight, the event sink, the RESUME type, the exit-code / stop / status serde types, the
//! witness-locus policy and the rule record builder of `fake`.

// Nested `if`s are kept on purpose where the inner condition has a side effect (`FINAL.swap`) or takes a lock.
#![allow(clippy::collapsible_if)]

use rustred_campaign_contract::atomic::{
    ENOSPC, create_reserve, exit_for_io, is_nospace, release_reserve,
};
use rustred_campaign_contract::events::{Event, EventSink, Phase};
use rustred_campaign_contract::fake::{
    FakeOp, FakeScript, FaultKind, HelperPlan, OnSigterm, fake_rule_record,
};
use rustred_campaign_contract::layout::{QUEUE, RESERVE_BYTES, RunLayout, TABLE};
use rustred_campaign_contract::outcome::{ErrorClass, PerCase, TimeDecision, TimeDecisionWhat};
use rustred_campaign_contract::preflight::{ProgramSelf, Refusal, preflight};
use rustred_campaign_contract::resume::{CaseSpec, Frame, ResumeState};
use rustred_campaign_contract::spec::{
    BudgetHit, Counters, JobKind, JobSpec, OnCaseBudget, Scope, WorkCounter,
};
use rustred_campaign_contract::stop::{CaseResult, StopCode, StopSource};
use rustred_campaign_contract::{CONTRACT, ExitCode, RESUME_SCHEMA, VersionInfo};
use serde_json::{Value, json};
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

fn git_commit() -> String {
    rustred_campaign_contract::git_commit_from_build_env(option_env!("RUSTRED_BUILD_GIT_COMMIT"))
}

/// `job.program` of OUTCOME, built by hand.
fn program_json() -> Value {
    json!({"name": PROGRAM, "version": env!("CARGO_PKG_VERSION"), "git_commit": git_commit(),
           "features": FEATURES, "exe": null})
}

/// The fake child's OWN writers (audit A M6): a second, deliberately simple implementation of the record stream with
/// running digests, the seal, the canonical record form and the no-clobber OUTCOME write. It must not call the
/// contract crate's sealing / canonical functions: the crate's verifier is then tested against it.
// lane adapters: kind solve-recipe emulated (src/bin/rr_fake_child/solve_recipe.rs)
#[path = "rr_fake_child/solve_recipe.rs"]
mod solve_recipe;
// lane closeloop2: the legacy rr_proof kinds emulated (src/bin/rr_fake_child/proof.rs)
#[path = "rr_fake_child/proof.rs"]
mod proof;
// lane symjob: kind symmetry-composite emulated (src/bin/rr_fake_child/symcomp.rs)
#[path = "rr_fake_child/symcomp.rs"]
mod symcomp;

mod own {
    use serde_json::Value;
    use std::fs::{File, OpenOptions};
    use std::io::{self, Read, Write};
    use std::os::unix::fs::OpenOptionsExt;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Volatile top-level keys (contract 3.2.3 v2), written again here on purpose.
    fn volatile(ty: Option<&str>, key: &str, family_digest: bool) -> bool {
        match ty {
            Some("header") => key == "zero_seconds" || (key == "family_file" && family_digest),
            Some("rule") => key == "seconds_search" || key == "lift_seconds",
            Some(
                "chart_end" | "chart_partial" | "end" | "partial" | "chart_error" | "cert_end"
                | "cert_partial" | "relation",
            ) => key == "seconds" || key == "vmhwm_kb",
            Some("verdict") | None => key == "seconds" || key == "utc" || key.ends_with("_seconds"),
            Some(_) => false,
        }
    }

    fn write(v: &Value, out: &mut Vec<u8>) {
        match v {
            Value::Object(m) => {
                let mut keys: Vec<&String> = m.keys().collect();
                keys.sort();
                out.push(b'{');
                for (i, k) in keys.iter().enumerate() {
                    if i > 0 {
                        out.push(b',');
                    }
                    out.extend_from_slice(serde_json::to_string(k).unwrap().as_bytes());
                    out.push(b':');
                    write(&m[*k], out);
                }
                out.push(b'}');
            }
            Value::Array(a) => {
                out.push(b'[');
                for (i, x) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(b',');
                    }
                    write(x, out);
                }
                out.push(b']');
            }
            Value::String(s) => out.extend_from_slice(serde_json::to_string(s).unwrap().as_bytes()),
            other => out.extend_from_slice(other.to_string().as_bytes()),
        }
    }

    /// The canonical form of one of the fake's own records (serde_json `Value`, keys sorted, volatile keys removed)
    /// and its `type`.
    pub fn canonical(line: &[u8]) -> Option<(Vec<u8>, Option<String>)> {
        let v: Value = serde_json::from_slice(line).ok()?;
        let mut m = v.as_object()?.clone();
        let ty = m.get("type").and_then(Value::as_str).map(str::to_string);
        let fam = ty.as_deref() == Some("header")
            && m.get("family_blake3")
                .and_then(Value::as_str)
                .is_some_and(|d| {
                    d.len() == 64
                        && d.bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                });
        m.retain(|k, _| !volatile(ty.as_deref(), k, fam));
        let mut out = Vec::new();
        write(&Value::Object(m), &mut out);
        Some((out, ty))
    }

    fn hex(h: &blake3::Hasher) -> String {
        h.finalize().to_hex().to_string()
    }

    /// BLAKE3 key-derivation contexts of the per-record and canonical-stream digests, written down from
    /// child-job.md 3.2.3 "DIGEST KINDS" (lane contractfix2, ac2-m7), not taken from the crate: a drift between this
    /// file and the crate shows up as an unsealed fake run.
    const RECORD_CONTEXT: &str = "rustred.child-job.v1 2026-11-14 record digest";
    const STREAM_CONTEXT: &str = "rustred.child-job.v1 2026-11-14 canonical stream digest";

    /// The canonical-stream digest's hasher (its own domain, child-job.md 3.2.3).
    pub fn canon_hasher() -> blake3::Hasher {
        blake3::Hasher::new_derive_key(STREAM_CONTEXT)
    }

    /// Figures of a stream (at a commit point or sealed).
    #[derive(Debug, Clone)]
    pub struct Figures {
        pub bytes: u64,
        pub blake3: String,
        pub records: u64,
        pub rules: u64,
        pub canonical: String,
    }

    pub struct Writer {
        file: File,
        raw: blake3::Hasher,
        canon: blake3::Hasher,
        bytes: u64,
        records: u64,
        rules: u64,
    }

    fn create_new(path: &Path) -> io::Result<File> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
    }

    fn fsync_parent(path: &Path) -> io::Result<()> {
        File::open(path.parent().unwrap_or(Path::new(".")))?.sync_all()
    }

    impl Writer {
        fn empty(file: File) -> Writer {
            Writer {
                file,
                raw: blake3::Hasher::new(),
                canon: canon_hasher(),
                bytes: 0,
                records: 0,
                rules: 0,
            }
        }

        /// A new stream (the file must not exist; never follows a symlink), its directory fsynced.
        pub fn create(path: &Path) -> io::Result<Writer> {
            let f = create_new(path)?;
            fsync_parent(path)?;
            Ok(Writer::empty(f))
        }

        fn push(&mut self, rec: &[u8]) -> io::Result<(u64, u64, String)> {
            let (c, ty) = canonical(rec).ok_or_else(|| io::Error::other("invalid record"))?;
            let mut line = rec.to_vec();
            line.push(b'\n');
            self.file.write_all(&line)?;
            let off = self.bytes;
            self.raw.update(&line);
            self.canon.update(&c);
            self.canon.update(b"\n");
            self.bytes += line.len() as u64;
            self.records += 1;
            if ty.as_deref() == Some("rule") {
                self.rules += 1;
            }
            let mut h = blake3::Hasher::new_derive_key(RECORD_CONTEXT);
            h.update(&c);
            h.update(b"\n");
            Ok((off, line.len() as u64, hex(&h)))
        }

        /// Append one record; returns (offset, line bytes, canonical digest of the record).
        pub fn append(&mut self, rec: &[u8]) -> io::Result<(u64, u64, String)> {
            self.push(rec)
        }

        pub fn figures(&self) -> Figures {
            Figures {
                bytes: self.bytes,
                blake3: hex(&self.raw),
                records: self.records,
                rules: self.rules,
                canonical: hex(&self.canon),
            }
        }

        pub fn commit(&mut self) -> io::Result<Figures> {
            self.file.sync_data()?;
            Ok(self.figures())
        }

        pub fn seal(mut self, end: &[u8]) -> io::Result<Figures> {
            self.push(end)?;
            self.file.sync_all()?;
            Ok(self.figures())
        }

        pub fn raw_file(&mut self) -> &mut File {
            &mut self.file
        }

        /// Copy the committed prefix `want` of `src` into a new `dst`, checking every figure with this module's own
        /// code (independent of the crate's preflight check that already ran).
        pub fn copy_prefix(src: &Path, want: &Figures, dst: &Path) -> Result<Writer, String> {
            let mut f = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(src)
                .map_err(|e| format!("{}: {e}", src.display()))?;
            let mut b = Vec::new();
            Read::by_ref(&mut f)
                .take(want.bytes)
                .read_to_end(&mut b)
                .map_err(|e| e.to_string())?;
            if b.len() as u64 != want.bytes || (!b.is_empty() && b.last() != Some(&b'\n')) {
                return Err("prefix shorter than committed or not at a line boundary".into());
            }
            let mut w = Writer::create(dst).map_err(|e| e.to_string())?;
            let r = (|| {
                for l in b.split(|&c| c == b'\n').filter(|l| !l.is_empty()) {
                    let (_, ty) = canonical(l).ok_or("invalid record in prefix")?;
                    if matches!(ty.as_deref(), Some("end" | "partial")) {
                        return Err("end record in prefix".to_string());
                    }
                    w.push(l).map_err(|e| e.to_string())?;
                }
                let g = w.figures();
                if g.bytes != want.bytes
                    || g.blake3 != want.blake3
                    || g.records != want.records
                    || g.rules != want.rules
                    || g.canonical != want.canonical
                {
                    return Err(format!("prefix figures differ: {g:?} vs {want:?}"));
                }
                w.file.sync_all().map_err(|e| e.to_string())
            })();
            if let Err(e) = r {
                drop(w);
                let _ = std::fs::remove_file(dst);
                return Err(e);
            }
            Ok(w)
        }
    }

    static SEQ: AtomicU64 = AtomicU64::new(0);

    /// No-clobber atomic write: complete temp file, hard link to `path` (fails if anything is there), unlink temp.
    pub fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
        let tmp = path.with_file_name(format!(
            "{}.fake-tmp.{}.{}",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("x"),
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let mut f = create_new(&tmp)?;
        let r = f.write_all(bytes).and_then(|_| f.sync_all());
        drop(f);
        let r = r.and_then(|_| std::fs::hard_link(&tmp, path));
        let _ = std::fs::remove_file(&tmp);
        r?;
        fsync_parent(path)
    }
}

use own::Figures;

fn committed(path: &str, f: &Figures) -> rustred_campaign_contract::resume::CommittedFile {
    rustred_campaign_contract::resume::CommittedFile {
        path: path.to_string(),
        bytes: f.bytes,
        blake3: f.blake3.clone(),
        records: f.records,
        rules: f.rules,
        canonical_blake3: f.canonical.clone(),
    }
}

fn figures_of(c: &rustred_campaign_contract::resume::CommittedFile) -> Figures {
    Figures {
        bytes: c.bytes,
        blake3: c.blake3.clone(),
        records: c.records,
        rules: c.rules,
        canonical: c.canonical_blake3.clone(),
    }
}

/// The fake's own map exit code -> `job.status` (contract 3.6), independent of `JobStatus::for_exit`.
fn status_of(code: ExitCode) -> &'static str {
    match code.code() {
        0 => "complete",
        10 => "incomplete",
        11 => "decided_negative",
        12 => "undecided",
        20..=22 => "budget",
        30 => "interrupted",
        40 | 42 => "refused",
        _ => "error",
    }
}

fn error_class(c: ErrorClass) -> Value {
    serde_json::to_value(c).unwrap()
}

/// The `job` object of OUTCOME for `spec`, built field by field (the fields every exit path shares).
#[allow(clippy::too_many_arguments)]
fn base_job(
    spec: &JobSpec,
    digest: &str,
    code: ExitCode,
    stop: Option<StopCode>,
    source: Option<StopSource>,
    graceful: bool,
    detail: Value,
    error: Option<(ErrorClass, String)>,
) -> serde_json::Map<String, Value> {
    let err = error.map(|(c, m)| {
        let m: String = m.chars().take(4000).collect();
        json!({"class": error_class(c), "message": m})
    });
    let j = json!({
        "schema": rustred_campaign_contract::OUTCOME_SCHEMA,
        "job_id": spec.job_id, "attempt": spec.attempt, "kind": spec.kind, "spec_blake3": digest,
        "status": status_of(code), "exit_code": code.code(),
        "stop": {"code": stop, "detail": detail, "source": source, "graceful": graceful},
        "program": program_json(), "effective": null,
        "seed": {"sample_seed": spec.seed.sample_seed, "point_source": "seed"},
        "inputs": [], "counters": Counters::default(),
        "budgets": {"work": spec.budgets.work, "wall": spec.budgets.wall,
                    "mem_soft_bytes": spec.budgets.mem_soft_bytes, "hit": null},
        "time_decisions": [], "time_decided": false, "per_case": [],
        "outputs": {"extra": []}, "timing": {}, "memory": {}, "error": err, "resumed": false,
    });
    match j {
        Value::Object(m) => m,
        _ => unreachable!(),
    }
}

/// The OUTCOME file bytes (pretty JSON + "\n") with the legacy keys.
fn outcome_bytes(job: serde_json::Map<String, Value>, code: ExitCode, rules: u64) -> Vec<u8> {
    let o =
        json!({"complete": code == ExitCode::Complete, "rules": rules, "job": Value::Object(job)});
    let mut v = serde_json::to_vec_pretty(&o).unwrap();
    v.push(b'\n');
    v
}

/// `outputs.table`, built by hand: `sealed` = the figures of the sealed file and its end kind.
fn table_json(
    sealed: Option<(&Figures, &str)>,
    commit: &Figures,
    rules_committed: u64,
    elided: u64,
) -> Value {
    json!({
        "path": RunLayout::out_rel(TABLE),
        "bytes": sealed.map(|s| s.0.bytes), "blake3": sealed.map(|s| s.0.blake3.clone()),
        "records": sealed.map(|s| s.0.records), "rules": sealed.map(|s| s.0.rules),
        "rules_committed": rules_committed,
        "committed_bytes": commit.bytes, "prefix_blake3": commit.blake3,
        "sealed": sealed.is_some(), "end": sealed.map(|s| s.1),
        "canonical_blake3": sealed.map(|s| s.0.canonical.clone()),
        "witness_loci_elided": elided,
    })
}

fn empty_figures() -> Figures {
    Figures {
        bytes: 0,
        blake3: blake3::hash(b"").to_hex().to_string(),
        records: 0,
        rules: 0,
        canonical: own::canon_hasher().finalize().to_hex().to_string(),
    }
}

/// Mutable job state shared with the watchdog. Never held across I/O (audit A m11).
struct State {
    counters: Counters,
    commit: Figures,
    queue_commit: Figures,
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
    inputs: Vec<rustred_campaign_contract::outcome::InputCheck>,
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

    /// The state for the hard-safe stop: the lock is only ever held briefly (no I/O under it), so a bounded wait is
    /// enough; after 1 s the stop goes on with an empty snapshot rather than hang (m11).
    fn st_try(&self) -> Option<std::sync::MutexGuard<'_, State>> {
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

    /// The OUTCOME bytes for the current state. `table` = `outputs.table` (None: built from the commit point).
    #[allow(clippy::too_many_arguments)]
    fn outcome(
        &self,
        st: Option<&State>,
        code: ExitCode,
        stop: Option<StopCode>,
        source: Option<StopSource>,
        graceful: bool,
        table: Option<Value>,
        error: Option<(ErrorClass, String)>,
        helpers_peak: u64,
    ) -> Vec<u8> {
        let detail = st
            .and_then(|s| s.hit.clone())
            .map_or(Value::Null, |h| serde_json::to_value(h).unwrap());
        let mut j = base_job(
            &self.spec,
            &self.spec_blake3,
            code,
            stop,
            source,
            graceful,
            detail,
            error,
        );
        let rules = st.map_or(0, |s| s.counters.rules_committed);
        let tds = st.map(|s| s.time_decisions.clone()).unwrap_or_default();
        let time_decided = !tds.is_empty() || stop == Some(StopCode::BudgetWall);
        let ef = empty_figures();
        let commit = st.map_or(&ef, |s| &s.commit);
        let table =
            table.unwrap_or_else(|| table_json(None, commit, rules, st.map_or(0, |s| s.elided)));
        let ins: Vec<Value> = self
            .inputs
            .iter()
            .map(|i| json!({"role": i.role, "path": i.path, "bytes": i.bytes, "blake3": i.blake3, "verified": i.verified}))
            .collect();
        j.insert("inputs".into(), Value::from(ins));
        if let Some(s) = st {
            j.insert("counters".into(), serde_json::to_value(s.counters).unwrap());
            j.insert(
                "per_case".into(),
                serde_json::to_value(&s.per_case).unwrap(),
            );
            if let Some(b) = j.get_mut("budgets").and_then(Value::as_object_mut) {
                b.insert("hit".into(), serde_json::to_value(&s.hit).unwrap());
            }
        }
        j.insert("time_decisions".into(), serde_json::to_value(&tds).unwrap());
        j.insert("time_decided".into(), Value::from(time_decided));
        j.insert(
            "outputs".into(),
            json!({
                "table": table,
                "queue": {"bytes": st.map_or(0, |s| s.queue_bytes)},
                "events": {"last_seq": self.sink.next_seq()},
                "resume": {"path": rustred_campaign_contract::layout::RESUME, "valid": st.is_some_and(|s| s.resume_valid)},
                "extra": [],
            }),
        );
        let wall = self.started.elapsed().as_secs_f64();
        j.insert(
            "timing".into(),
            json!({"started_utc": self.started_utc, "finished_utc": rustred_campaign_contract::time::utc_now(),
                   "wall_s": (wall * 1000.0).round() / 1000.0, "cpu_s": null}),
        );
        j.insert(
            "memory".into(),
            json!({"vmhwm_kb": vmhwm_kb(), "rss_kb_at_end": rss_kb_of(0), "helpers_peak_rss_kb": helpers_peak}),
        );
        j.insert("resumed".into(), Value::from(self.resumed));
        outcome_bytes(j, code, rules)
    }

    /// The hard-safe stop (3.4.3): kill helpers, leave table/queue alone, OUTCOME from the job state, try the
    /// `job_finished` event for 1 s, `_exit`.
    fn hard_stop(&self, code: ExitCode, stop: StopCode, td: Option<TimeDecision>) -> ! {
        if FINAL.swap(true, SeqCst) {
            park_forever();
        }
        let peak = self.kill_helpers();
        let src = *self.stop_source.lock().unwrap_or_else(|p| p.into_inner());
        let mut g = self.st_try();
        if let (Some(td), Some(g)) = (td, g.as_mut()) {
            g.time_decisions.push(td);
        }
        let b = self.outcome(g.as_deref(), code, Some(stop), src, false, None, None, peak);
        drop(g);
        let _ = own::write_new(&self.layout.outcome(), &b);
        self.sink.try_emit_for(
            Event::JobFinished {
                exit_code: code,
                status: rustred_campaign_contract::JobStatus::for_exit(code),
                stop_code: Some(stop),
            },
            Duration::from_secs(1),
        );
        // SAFETY: _exit ends the process without running destructors (the main thread may hold locks).
        unsafe { libc::_exit(code.code()) }
    }

    /// RESUME at a commit point (at most every `resume_every_s`, or `force`): the state is copied under the lock and
    /// written WITHOUT holding it (m11).
    fn write_resume(&self, force: bool) {
        let r = {
            let st = self.st();
            let every = self.spec.resume_every_s as u64;
            let due = force
                || every == 0
                || st
                    .resume_written_at
                    .is_none_or(|t| t.elapsed() >= Duration::from_secs(every));
            if !due {
                return;
            }
            ResumeState {
                schema: RESUME_SCHEMA.into(),
                job_id: self.spec.job_id.clone(),
                attempt: self.spec.attempt,
                spec_blake3: self.spec_blake3.clone(),
                chart: Value::from("C"),
                frame: Frame::Presented,
                rules_committed: st.counters.rules_committed,
                table: committed(&RunLayout::out_rel(TABLE), &st.commit),
                queue: Some(committed(&RunLayout::out_rel(QUEUE), &st.queue_commit)),
                current: st.current.clone(),
                pending: st.pending.clone(),
                numerical: vec![],
                deferred: vec![],
                refused_open: vec![],
                counters_at_commit: st.counters,
                symbolic_cases: st.counters.cases,
                discarded: 0,
                engine: Default::default(),
            }
        };
        if rustred_campaign_contract::atomic::write_json_atomic(&self.layout.resume(), &r).is_ok() {
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
/// Report a refused start on stderr, write an error OUTCOME only when preflight allows it (the run directory is
/// verified to be this attempt's), never over an existing one (no-clobber), and exit with its code.
fn refuse_exit(r: Refusal) -> ! {
    eprintln!("rr_fake_child: {}", r.message);
    if r.write_outcome {
        if let Some((spec, digest)) = &r.spec {
            let j = base_job(
                spec,
                digest,
                r.code,
                Some(r.stop),
                None,
                true,
                Value::Null,
                Some((r.class, r.message.clone())),
            );
            let _ = own::write_new(
                &RunLayout::new(&spec.run_dir).outcome(),
                &outcome_bytes(j, r.code, 0),
            );
        }
    }
    std::process::exit(r.code.code())
}

/// The 3.10 path for an I/O error: free the reserve, OUTCOME (best effort), exit 50 / 51.
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
        let bytes = {
            let st = ctx.st();
            ctx.outcome(
                Some(&st),
                code,
                Some(stop),
                None,
                true,
                None,
                Some((class, e.to_string())),
                peak,
            )
        };
        let _ = own::write_new(&ctx.layout.outcome(), &bytes);
        let _ = ctx.sink.emit(Event::JobFinished {
            exit_code: code,
            status: rustred_campaign_contract::JobStatus::for_exit(code),
            stop_code: Some(stop),
        });
    }
    std::process::exit(code.code())
}

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
    table: Option<own::Writer>,
    queue: own::Writer,
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
            st.queue_commit = q;
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
        // Work advances in a FIXED number of steps (one per 50 ms of the script's unscaled time), not by the wall-clock
        // fraction: the counter values at a budget hit (and every other counter then) are a function of the script
        // alone, never of scheduling delays under load (lane int2: the flaky "deterministic" budget test compared
        // counters that had advanced by wall time).
        let steps = ms.div_ceil(50).max(1);
        let mut step = 0u64;
        loop {
            let frac = if dur.is_zero() {
                1.0
            } else {
                (step as f64 / steps as f64).min(1.0)
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
            step += 1;
            // sleep until this step's share of the (scaled) case time, in slices of at most 50 ms with a safe point
            // after each; a late wake-up only delays the next step, it never skips work
            let due = dur.mul_f64(step as f64 / steps as f64);
            loop {
                let wait = due.saturating_sub(t0.elapsed());
                std::thread::sleep(wait.clamp(Duration::from_millis(1), Duration::from_millis(50)));
                if t0.elapsed() >= due {
                    break;
                }
                if let Some(f) = self.safe_point() {
                    return Err(f);
                }
            }
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
            let (off, len, canon) = self
                .table
                .as_mut()
                .expect("table")
                .append(&line)
                .map_err(Finish::Io)?;
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
                line_offset: off,
                line_bytes: len,
                canon_blake3: canon,
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
            .map_err(Finish::Io)?;
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
                // audit B F6: the fault path is only ever a CHARACTER DEVICE (e.g. /dev/full), never a regular file
                let e = match &f.path {
                    Some(p) => {
                        use std::os::unix::fs::FileTypeExt;
                        let dev = std::fs::symlink_metadata(p)
                            .is_ok_and(|m| m.file_type().is_char_device());
                        if dev {
                            std::fs::OpenOptions::new()
                                .write(true)
                                .open(p)
                                .and_then(|mut h| {
                                    h.write_all(&[0u8; 65536]).and_then(|_| h.flush())
                                })
                                .err()
                                .unwrap_or_else(|| {
                                    io::Error::other("write to the fault path succeeded")
                                })
                        } else {
                            io::Error::other("fault path is not a character device")
                        }
                    }
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
                    let done = ctx.st().counters;
                    if ctx.spec.budgets.work.cases_exhausted(&done) {
                        return Finish::End {
                            stop: Some(StopCode::MaxCases),
                            verdict: None,
                        };
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
                FakeOp::Relation { .. }
                | FakeOp::RecipeQuirk { .. }
                | FakeOp::ProofAnswer { .. } => {
                    return Finish::Fail {
                        code: ExitCode::Usage,
                        message: "op only for an emulated kind (solve-recipe, rr_proof)".into(),
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
                (code, Some(stop), None, Some((class, message)))
            }
            Finish::Io(e) => io_exit_now(&ctx, e),
        };
        let faults = &ctx.script.faults;
        let unsealed_claim = faults.iter().any(|f| f.kind == FaultKind::OutcomeUnsealed);
        let kill_before = faults
            .iter()
            .any(|f| f.kind == FaultKind::KillBeforeOutcome);
        let end_kind = if stop.is_none() { "end" } else { "partial" };
        let (a, b) = ctx.jitter().unwrap_or((0.0, 0.0));
        let (rules, elided, commit) = {
            let st = ctx.st();
            (st.counters.rules_committed, st.elided, st.commit.clone())
        };
        let end = json!({"type": end_kind, "charts": 1, "rules": rules, "seconds": a, "vmhwm_kb": (b * 1000.0) as u64});
        let end_line = serde_json::to_vec(&end).unwrap();
        let mut writer = self.table.take().expect("table");
        let table = if unsealed_claim {
            // contract violation on purpose: no end record, but OUTCOME claims a sealed table
            let f = match writer.commit() {
                Ok(f) => f,
                Err(e) => io_exit_now(&ctx, e),
            };
            table_json(Some((&f, end_kind)), &f, rules, elided)
        } else {
            match writer.seal(&end_line) {
                // the job state keeps the last COMMIT point (before the end record): the final RESUME's committed
                // prefix never contains an end record, so a resume continues the records
                Ok(f) => table_json(Some((&f, end_kind)), &commit, rules, elided),
                Err(e) => io_exit_now(&ctx, e),
            }
        };
        let qend = json!({"type": end_kind, "cases": ctx.st().counters.cases});
        let q = self
            .queue
            .append(&serde_json::to_vec(&qend).unwrap())
            .and_then(|_| self.queue.commit());
        match q {
            Ok(q) => ctx.st().queue_bytes = q.bytes,
            Err(e) => io_exit_now(&ctx, e),
        }
        ctx.write_resume(true);
        if kill_before {
            sigkill_self();
        }
        if FINAL.swap(true, SeqCst) {
            park_forever();
        }
        let peak = ctx.kill_helpers();
        let bytes = {
            let st = ctx.st();
            ctx.outcome(
                Some(&st),
                code,
                stop,
                graceful_src,
                true,
                Some(table),
                error,
                peak,
            )
        };
        if let Err(e) = own::write_new(&ctx.layout.outcome(), &bytes) {
            let _ = release_reserve(&ctx.layout.reserve());
            eprintln!("rr_fake_child: OUTCOME: {e}");
            std::process::exit(exit_for_io(&e).code());
        }
        let _ = ctx.sink.emit(Event::JobFinished {
            exit_code: code,
            status: rustred_campaign_contract::JobStatus::for_exit(code),
            stop_code: stop,
        });
        let _ = release_reserve(&ctx.layout.reserve());
        std::process::exit(code.code())
    }
}

fn sigkill_self() -> ! {
    die_by_signal(libc::SIGKILL)
}

/// End killed by `sig` (default disposition restored first; SIGKILL cannot be caught anyway): the one self-signal form
/// of the fake child (`kill(getpid(), ..)`; lane closeloop3: also the legacy rr_proof emulation's end on a signal).
pub(crate) fn die_by_signal(sig: libc::c_int) -> ! {
    // SAFETY: restoring the default disposition and signalling ourselves.
    unsafe {
        if sig != libc::SIGKILL {
            libc::signal(sig, libc::SIG_DFL);
        }
        libc::kill(libc::getpid(), sig);
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
    // lane adapters: an emulated kind runs its own mode (kind solve-recipe)
    if std::fs::read(spec_path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .is_some_and(|v| v["kind"] == "solve-recipe")
    {
        solve_recipe::run(spec_path)
    }
    // lane symjob: kind symmetry-composite (rr_rf_symflow emulated over Q with the campaign core)
    if std::fs::read(spec_path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .is_some_and(|v| v["kind"] == "symmetry-composite")
    {
        symcomp::run(spec_path)
    }
    if let Some(k) = std::fs::read(spec_path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v["kind"].as_str().map(str::to_string))
        .filter(|k| {
            k == "proof-instance-span" || k == "proof-instantiate" || k == "coefficient-facts"
        })
    {
        proof::run(spec_path, &k)
    }
    let me = ProgramSelf {
        name: PROGRAM,
        features: &FEATURES,
        kinds: &[JobKind::Fake],
        blake3: None,
    };
    // preflight verifies the run directory before anything is written and, with `resume`, the whole resume source
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
    let refuse_input = |msg: String| -> ! {
        refuse_exit(Refusal {
            code: ExitCode::Input,
            stop: StopCode::InputError,
            class: ErrorClass::Input,
            message: msg,
            spec: Some((Box::new(prep.spec.clone()), prep.spec_blake3.clone())),
            write_outcome: true,
        })
    };
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
    // tables first (resume: the committed prefixes, checked again by this program's own code), so that a refused
    // resume leaves out/ empty and writes no reserve or event
    let resume = prep.resume.clone();
    let (table, queue) = match &resume {
        Some(v) => {
            let t = own::Writer::copy_prefix(
                &v.prev.table(),
                &figures_of(&v.state.table),
                &layout.table(),
            )
            .unwrap_or_else(|e| refuse_input(format!("resume table prefix: {e}")));
            let q = match &v.state.queue {
                Some(q) => {
                    own::Writer::copy_prefix(&v.prev.queue(), &figures_of(q), &layout.queue())
                        .unwrap_or_else(|e| refuse_input(format!("resume queue prefix: {e}")))
                }
                None => own::Writer::create(&layout.queue())
                    .unwrap_or_else(|e| refuse_input(format!("queue: {e}"))),
            };
            (t, q)
        }
        None => {
            let t = own::Writer::create(&layout.table());
            let q = own::Writer::create(&layout.queue());
            match (t, q) {
                (Ok(t), Ok(q)) => (t, q),
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("rr_fake_child: out/: {e}");
                    std::process::exit(exit_for_io(&e).code());
                }
            }
        }
    };
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
    let commit0 = table.figures();
    let queue0 = queue.figures();
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
                .map(|v| v.state.counters_at_commit)
                .unwrap_or_default(),
            commit: commit0,
            queue_bytes: queue0.bytes,
            queue_commit: queue0,
            phase: Phase::Prepare,
            case_ordinal: None,
            case_started: None,
            current: None,
            pending: resume
                .as_ref()
                .map(|v| v.state.pending.clone())
                .unwrap_or_default(),
            per_case: vec![],
            time_decisions: vec![],
            elided: 0,
            hit: None,
            resume_written_at: None,
            resume_valid: false,
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
                let g = ctx.st_try();
                let b = ctx.outcome(
                    g.as_deref(),
                    ExitCode::Internal,
                    Some(StopCode::Panic),
                    None,
                    false,
                    None,
                    Some((ErrorClass::Panic, info.to_string())),
                    peak,
                );
                drop(g);
                let _ = own::write_new(&ctx.layout.outcome(), &b);
                ctx.sink.try_emit_for(
                    Event::JobFinished {
                        exit_code: ExitCode::Internal,
                        status: rustred_campaign_contract::JobStatus::Error,
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
        git_commit: git_commit(),
        resume_from: resume.as_ref().map(|v| v.state.attempt),
    });
    let wd = ctx.clone();
    std::thread::spawn(move || watchdog(wd));
    let mut table = table;
    if resume.is_none() {
        // a fresh table starts with its header; the family file (when the spec has one) is named by path AND content
        // digest, which makes the path volatile in the canonical form (coordinator ruling on family_file)
        let (z, _) = ctx.jitter().unwrap_or((0.0, 0.0));
        let mut h = json!({"type":"header","schema":"rustred.fake.table.v1","source":PROGRAM,"contract":CONTRACT,
                           "sample_seed":ctx.spec.seed.sample_seed,"zero_seconds":z});
        if let Some(f) = ctx.spec.inputs.iter().find(|i| i.role == "family") {
            h["family_file"] = Value::from(f.path.clone());
            h["family_blake3"] = Value::from(f.blake3.clone());
        }
        if let Err(e) = table.append(&serde_json::to_vec(&h).unwrap()) {
            io_exit_now(&ctx, e)
        }
    }
    match table.commit() {
        Ok(cp) => ctx.st().commit = cp,
        Err(e) => io_exit_now(&ctx, e),
    }
    let skip = resume
        .as_ref()
        .map_or(0, |v| v.state.counters_at_commit.cases);
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
                git_commit: git_commit(),
                contract: vec![CONTRACT.into()],
                features: FEATURES.iter().map(|s| s.to_string()).collect(),
                job_kinds: vec![JobKind::Fake, JobKind::SolveRecipe],
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
