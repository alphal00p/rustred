//! Job kind `solve-recipe` of the child-job contract (lane engbase2; interfaces/child-job.md "Job kind solve-recipe",
//! interfaces/adapters.md 1.1-1.4): `rr_node job RUN/job.json` runs the engine line's recipe solve
//! ([`crate::node::recipe::run_batch`], coord-recipe-solve.md v1.1) as an L1 job of the contract.
//!
//! The dispatcher, not the verb, owns everything the contract fixes:
//! - the start-up checks are the contract crate's `preflight` (done by the bin before [`run`]): strict spec
//!   validation (kind allow-list of args, input roles, the kind's constraints), run directory, program, features,
//!   input digests; the inputs are read here ONCE and their BLAKE3 compared with the spec's again (no TOCTOU between
//!   the check and the read), the keys parsed with the CONTRACT's keys reader (the one the supervisor tallies with);
//! - signals: the contract's counting handlers (installed by the bin first thing); the verb's own handler is never
//!   installed: its stop request is the job's flag ([`crate::node::recipe::BatchOptions::stop`]); the STOP file and
//!   the second signal are handled by this module's watchdog (second signal = hard-safe stop: OUTCOME 30 with no
//!   sealed output, `_exit`);
//! - `RUN/EVENTS.jsonl` in the contract's envelope (job_started, phase, engine `RecipeKey` per finished key forwarded
//!   from the verb's own event file, heartbeat, stop_requested, job_finished); the verb's OUTCOME.json / EVENTS.jsonl
//!   stay in `out/` as informational files;
//! - `RUN/OUTCOME.json` (`rustred.child-job.outcome.v1`): written in the verb's end hook
//!   ([`crate::node::recipe::BatchOptions::on_end`]) after `out/results.jsonl` was verified with the contract's sealed
//!   stream verifier and tallied with the contract's own per-key reader against the keys input (a results file the
//!   supervisor would refuse is reported as an internal error, never as a success); the spec digest is the
//!   preflight's (domain-separated) digest; stop codes `keys_unsolved` (exit 10) / `interrupted` (exit 30); per-key
//!   wall-budget records become time decisions; the verb's own outcome object is kept as member `result`.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering::*};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use cc::ExitCode;
use cc::classify::recipe::{FailureClass, KeyLine, KeyOutcome, key_outcomes, parse_keys_input};
use cc::events::{Event, EventSink, Phase};
use cc::layout::{RESULTS, RunLayout};
use cc::outcome::{
    Effective, EndKind, ErrorClass, ErrorInfo, EventsOutput, ExtraOutput, InputCheck, JobOutcome,
    Memory, Outcome, Outputs, ProgramInfo, StopInfo, TimeDecision, TimeDecisionWhat, Timing,
};
use cc::spec::{Counters, JobSpec};
use cc::stop::{JobStatus, StopCode, StopSource};
use rustred_campaign_contract as cc;
use serde_json::{Value, json};

use super::{FINAL, JobStart, effective_env, lock, park_forever, proc};
use crate::node::order::NodeOrder;
use crate::node::recipe;

/// What the bin builds from the spec's `args` with the legacy parser (`--order`, `--threads`, `--max-trace-rows`,
/// `--key-budget-s`; the allow-list itself was checked by the contract crate's preflight).
pub struct RecipeArgs {
    pub order: NodeOrder,
    pub threads: usize,
    pub max_trace_rows: Option<usize>,
    /// `--key-budget-s` or `budgets.wall.case_s` (the spec validation refuses both together)
    pub key_budget_s: Option<f64>,
}

struct RJob {
    spec: JobSpec,
    spec_blake3: String,
    layout: RunLayout,
    inputs: Vec<InputCheck>,
    program: ProgramInfo,
    sink: EventSink,
    started: Instant,
    started_utc: String,
    effective: Mutex<Option<Effective>>,
    /// the recipe's `relation_rule` (v1.2, default m1_first) as the verb parses it; recorded in OUTCOME
    relation_rule: Mutex<Value>,
    keys: Mutex<Vec<KeyLine>>,
    key_budget_s: Mutex<Option<f64>>,
    stop: AtomicBool,
    stop_source: Mutex<Option<StopSource>>,
    stop_at: Mutex<Option<Instant>>,
    keys_done: AtomicU64,
    solved_done: AtomicU64,
    /// 0 prepare, 1 case_search (= keys being solved), 2 finishing
    phase: AtomicU8,
    /// forwarding of the verb's per-key events (watchdog; the end hook drains and closes it before `job_finished`)
    forward: Mutex<Forward>,
}

static RJOB: OnceLock<Arc<RJob>> = OnceLock::new();

/// Whether this process runs a `solve-recipe` job.
pub fn active() -> bool {
    RJOB.get().is_some()
}

impl RJob {
    fn phase(&self) -> Phase {
        match self.phase.load(Relaxed) {
            0 => Phase::Prepare,
            1 => Phase::CaseSearch,
            _ => Phase::Finishing,
        }
    }

    fn set_phase(&self, p: u8) {
        if self.phase.swap(p, Relaxed) != p {
            let _ = self.sink.emit(Event::Phase {
                phase: self.phase(),
            });
        }
    }

    /// Forward the verb's new per-key events; `close`: the last call (every later one is a no-op), so that no event
    /// follows `job_finished`.
    fn forward_events(&self, close: bool) {
        let mut f = lock(&self.forward);
        f.pump(self, &self.layout.out_dir().join("EVENTS.jsonl"));
        if close {
            f.closed = true;
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

    fn counters(&self, rules: u64) -> Counters {
        Counters {
            cases: lock(&self.keys).len() as u64,
            rules_committed: rules,
            ..Default::default()
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn outcome(
        &self,
        code: ExitCode,
        stop: StopInfo,
        extra: Vec<ExtraOutput>,
        rules: u64,
        time_decisions: Vec<TimeDecision>,
        error: Option<ErrorInfo>,
        legacy: serde_json::Map<String, Value>,
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
        j.counters = self.counters(rules);
        j.time_decided = !time_decisions.is_empty();
        j.time_decisions = time_decisions;
        j.outputs = Outputs {
            table: None,
            cert: None,
            queue: None,
            events: Some(EventsOutput {
                last_seq: self.sink.next_seq(),
            }),
            resume: None,
            extra,
        };
        let wall = self.started.elapsed().as_secs_f64();
        j.timing = Timing {
            started_utc: Some(self.started_utc.clone()),
            finished_utc: Some(cc::time::utc_now()),
            wall_s: Some((wall * 1000.0).round() / 1000.0),
            cpu_s: Some((proc::cpu_seconds() * 1000.0).round() / 1000.0),
        };
        j.memory = Memory {
            vmhwm_kb: Some(crate::node::driver::vmhwm_kb()),
            rss_kb_at_end: Some(proc::rss_kb(0)),
            helpers_peak_rss_kb: Some(0),
        };
        j.error = error;
        j.resumed = false;
        let mut o = Outcome::new(j);
        o.legacy = legacy;
        o
    }

    /// Write OUTCOME (no-clobber; the reserve released first), emit `job_finished`; the exit code.
    fn publish(&self, o: &Outcome) -> i32 {
        if let Err(e) = o.job.check() {
            eprintln!("rr_node job: OUTCOME self-check: {e}");
        }
        let _ = cc::atomic::release_reserve(&self.layout.reserve());
        let code = o.job.exit_code;
        if let Err(e) = o.write_new(&self.layout.outcome()) {
            eprintln!("rr_node job: OUTCOME: {e}");
            return cc::atomic::exit_for_io(&e).code();
        }
        let _ = self.sink.emit(Event::JobFinished {
            exit_code: code,
            status: JobStatus::for_exit(code),
            stop_code: o.job.stop.code,
        });
        code.code()
    }

    /// The hard-safe stop (second signal, or no end within the grace period while the verb cannot stop: e.g. its
    /// preparation): OUTCOME 30 / 21 without any sealed output (an L1 job keeps nothing of an interrupted attempt),
    /// `job_finished` tried for 1 s, `_exit`.
    fn hard_stop(&self) -> ! {
        if FINAL.swap(true, SeqCst) {
            park_forever();
        }
        let src = *lock(&self.stop_source);
        let info = StopInfo {
            code: Some(StopCode::Interrupted),
            detail: Value::Null,
            source: src,
            graceful: false,
        };
        let o = self.outcome(
            ExitCode::Interrupted,
            info,
            vec![],
            0,
            vec![],
            None,
            serde_json::Map::new(),
        );
        if let Err(e) = o.job.check() {
            eprintln!("rr_node job: OUTCOME self-check: {e}");
        }
        let _ = cc::atomic::release_reserve(&self.layout.reserve());
        let _ = o.write_new(&self.layout.outcome());
        self.sink.try_emit_for(
            Event::JobFinished {
                exit_code: ExitCode::Interrupted,
                status: JobStatus::for_exit(ExitCode::Interrupted),
                stop_code: Some(StopCode::Interrupted),
            },
            Duration::from_secs(1),
        );
        proc::exit_now(ExitCode::Interrupted.code())
    }

    /// An error OUTCOME (usage / input / internal ...) and its code; nothing accepted.
    fn fail_outcome(&self, code: ExitCode, message: &str) -> i32 {
        let (stop, class) = match code {
            ExitCode::Usage => (StopCode::Refused, ErrorClass::Usage),
            ExitCode::Input => (StopCode::InputError, ErrorClass::Input),
            ExitCode::Refused => (StopCode::Refused, ErrorClass::Refused),
            ExitCode::NoSpace => (StopCode::Nospace, ErrorClass::Nospace),
            ExitCode::Io => (StopCode::IoError, ErrorClass::Io),
            _ => (StopCode::InternalError, ErrorClass::Internal),
        };
        eprintln!("rr_node job: {message}");
        let info = StopInfo {
            code: Some(stop),
            detail: Value::Null,
            source: None,
            graceful: true,
        };
        let o = self.outcome(
            code,
            info,
            vec![],
            0,
            vec![],
            Some(ErrorInfo::new(class, message)),
            serde_json::Map::new(),
        );
        self.publish(&o)
    }
}

/// [`super::fail_now`] for a `solve-recipe` job: error OUTCOME and exit (the hard-safe stop, if it already ended the
/// job, keeps its OUTCOME and code). Returns when no `solve-recipe` job runs.
pub(super) fn fail_now(code: ExitCode, message: &str) {
    let Some(j) = RJOB.get() else { return };
    if FINAL.swap(true, SeqCst) {
        park_forever();
    }
    let c = j.fail_outcome(code, message);
    std::process::exit(c)
}

fn fail(code: ExitCode, message: &str) -> ! {
    fail_now(code, message);
    std::process::exit(code.code())
}

/// The watchdog: signals (first = stop request into the verb's flag, second = hard-safe stop), the STOP file, the
/// backstop grace (the verb's own watchdog seals a partial file after `stop_grace_s`; this one acts only when the
/// verb cannot, 5 s later), heartbeats, and the forwarding of the verb's per-key events into the contract envelope.
fn watchdog(j: Arc<RJob>) {
    let mut seen = 0u32;
    let mut last_hb = Instant::now();
    let mut last_stopfile = Instant::now() - Duration::from_secs(2);
    let mut last_fwd = Instant::now();
    let hb = Duration::from_secs(u64::from(j.spec.heartbeat_s.max(1)));
    let backstop = Duration::from_secs(u64::from(j.spec.stop_grace_s) + 5);
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
            j.request_stop(src);
            if n >= 2 {
                j.hard_stop();
            }
        }
        if last_stopfile.elapsed() >= Duration::from_secs(1) {
            last_stopfile = Instant::now();
            if j.layout.stop_file().exists() {
                j.request_stop(StopSource::StopFile);
            }
        }
        if j.stop.load(SeqCst) && lock(&j.stop_at).is_some_and(|t| t.elapsed() >= backstop) {
            j.hard_stop();
        }
        if last_fwd.elapsed() >= Duration::from_millis(200) {
            last_fwd = Instant::now();
            j.forward_events(false);
        }
        if last_hb.elapsed() >= hb {
            last_hb = Instant::now();
            let total = lock(&j.keys).len() as u64;
            let done = j.keys_done.load(Relaxed);
            let solved = j.solved_done.load(Relaxed);
            let _ = j.sink.emit(Event::Heartbeat {
                phase: j.phase(),
                case_ordinal: None,
                pending: total.saturating_sub(done),
                rules_committed: solved,
                counters: j.counters(solved),
                rss_kb: proc::rss_kb(0),
                vmhwm_kb: crate::node::driver::vmhwm_kb(),
                helpers: 0,
                stop_requested: j.stop.load(SeqCst),
            });
        }
    }
}

/// Incremental reader of the verb's `out/EVENTS.jsonl` (complete lines only).
#[derive(Default)]
struct Forward {
    offset: u64,
    pending: Vec<u8>,
    closed: bool,
}

impl Forward {
    fn pump(&mut self, j: &RJob, path: &std::path::Path) {
        if self.closed {
            return;
        }
        let Ok(mut f) = std::fs::File::open(path) else {
            return;
        };
        use std::io::Seek;
        if f.seek(std::io::SeekFrom::Start(self.offset)).is_err() {
            return;
        }
        let mut buf = Vec::new();
        if f.read_to_end(&mut buf).is_err() {
            return;
        }
        self.offset += buf.len() as u64;
        self.pending.extend_from_slice(&buf);
        while let Some(nl) = self.pending.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=nl).collect();
            let Ok(v) = serde_json::from_slice::<Value>(&line) else {
                continue;
            };
            match v["kind"].as_str() {
                // the verb emits job_started once its preparation (family, zero census, solver) is done
                Some("job_started") => j.set_phase(1),
                Some("key_finished") => {
                    j.keys_done.fetch_add(1, Relaxed);
                    if v["verdict"] == "SOLVED" {
                        j.solved_done.fetch_add(1, Relaxed);
                    }
                    let _ = j.sink.emit(Event::Engine {
                        event: json!({"RecipeKey": {"unique": v["unique"], "verdict": v["verdict"],
                            "seconds": v["seconds"]}}),
                    });
                }
                _ => {}
            }
        }
    }
}

/// One input of the spec: (path, bytes), read once; its BLAKE3 must still be the spec's (41 otherwise).
fn read_input(spec: &JobSpec, role: &str) -> Option<(PathBuf, Vec<u8>)> {
    let i = spec.inputs.iter().find(|i| i.role == role)?;
    let p = spec.input_path(i);
    let b = std::fs::read(&p).unwrap_or_else(|e| {
        fail(
            ExitCode::Input,
            &format!("input {role} {}: {e}", p.display()),
        )
    });
    let d = cc::blake3_hex(&b);
    if d != i.blake3 {
        fail(
            ExitCode::Input,
            &format!(
                "input {role} {}: blake3 {d} differs from the spec's {} (changed after the start-up check)",
                p.display(),
                i.blake3
            ),
        );
    }
    Some((p, b))
}

fn text(role: &str, b: Vec<u8>) -> String {
    String::from_utf8(b).unwrap_or_else(|e| fail(ExitCode::Input, &format!("input {role}: {e}")))
}

fn canon_path(p: &std::path::Path) -> String {
    std::fs::canonicalize(p)
        .unwrap_or_else(|_| p.to_path_buf())
        .display()
        .to_string()
}

/// Exit code of an error the batch returns before any key ran (mirrors the CLI: recipe, arity and two-sided family
/// problems are input errors, everything else internal).
fn batch_error_code(e: &str) -> ExitCode {
    if e.starts_with("recipe") || e.contains("arity") || e.contains("two-sided") {
        ExitCode::Input
    } else {
        ExitCode::Internal
    }
}

/// The end hook (called once by the verb after `out/results.jsonl` is sealed): verify, tally, OUTCOME; the exit code.
fn finish(j: &RJob, verb: &Value, exit: i32, graceful: bool) -> i32 {
    if FINAL.swap(true, SeqCst) {
        park_forever();
    }
    // every per-key event of the verb (written before its results) is forwarded now, none after `job_finished`
    j.forward_events(true);
    j.set_phase(2);
    let internal = |m: String| j.fail_outcome(ExitCode::Internal, &m);
    let rel = RunLayout::out_rel(RESULTS);
    let (vs, handle) =
        match cc::seal::verify_stream_handle(&j.layout.root, &rel, &["end", "partial"]) {
            Ok(x) => x,
            Err(u) => return internal(format!("{rel} is not a sealed stream: {u:?}")),
        };
    let lines = match handle.canonical_lines() {
        Ok(l) => l,
        Err(e) => return internal(format!("{rel}: {e}")),
    };
    let keys = lock(&j.keys).clone();
    let (outs, tally) = match key_outcomes(&lines, &keys) {
        Ok(x) => x,
        Err(e) => {
            return internal(format!(
                "{rel} is not a consistent results file of this job (the supervisor would refuse it): {e}"
            ));
        }
    };
    let unsolved = tally.unsolved_total();
    let (code, stop) = match (exit, vs.end.as_str()) {
        (0, "end") if unsolved == 0 => (ExitCode::Complete, None),
        (10, "end") if unsolved > 0 => (ExitCode::Incomplete, Some(StopCode::KeysUnsolved)),
        (30, "partial") => (ExitCode::Interrupted, Some(StopCode::Interrupted)),
        _ => {
            return internal(format!(
                "the batch ended with exit {exit}, a `{}` results file and {unsolved} unsolved keys",
                vs.end
            ));
        }
    };
    let budget = *lock(&j.key_budget_s);
    let time_decisions: Vec<TimeDecision> = outs
        .iter()
        .filter_map(|o| match o {
            KeyOutcome::Unsolved(u) if u.class == FailureClass::BudgetWall => Some(TimeDecision {
                what: TimeDecisionWhat::CaseWall,
                case_ordinal: Some(u.ordinal),
                rule: None,
                limit_s: budget.unwrap_or(0.0),
                measured_s: u.failure["detail"]["value"].as_f64().unwrap_or(0.0),
                taken: "budget_wall".into(),
            }),
            _ => None,
        })
        .collect();
    let source = if code == ExitCode::Interrupted {
        *lock(&j.stop_source)
    } else {
        None
    };
    let detail = if code == ExitCode::Complete {
        Value::Null
    } else {
        json!({"counts": verb["counts"]})
    };
    let info = StopInfo {
        code: stop,
        detail,
        source,
        graceful,
    };
    let extra = vec![ExtraOutput {
        role: "results".into(),
        path: rel,
        bytes: vs.bytes,
        blake3: vs.blake3.clone(),
        records: Some(vs.records),
        canonical_blake3: Some(vs.canonical_blake3.clone()),
        end: EndKind::from_type(&vs.end),
        sealed: true,
    }];
    let mut legacy = serde_json::Map::new();
    legacy.insert("complete".into(), Value::Bool(code == ExitCode::Complete));
    legacy.insert("rules".into(), Value::from(tally.accepted));
    legacy.insert("relation_rule".into(), lock(&j.relation_rule).clone());
    legacy.insert("result".into(), verb.clone());
    let o = j.outcome(
        code,
        info,
        extra,
        tally.accepted,
        time_decisions,
        None,
        legacy,
    );
    j.publish(&o)
}

/// Run one `solve-recipe` job whose start-up checks passed (`start`). `resolve` builds the batch parameters from the
/// spec's `args` and the family text with the bin's legacy parser (it may end the process through
/// [`super::fail_now`]). Never returns.
pub fn run(start: JobStart, resolve: &dyn Fn(&JobSpec, &str) -> RecipeArgs) -> ! {
    let JobStart {
        spec,
        spec_blake3,
        layout,
        inputs,
        program,
        resume,
    } = start;
    if resume.is_some() {
        // the kind constraints refuse `resume` already (L1); fail closed if a spec reaches here with one
        eprintln!("rr_node job: kind solve-recipe has no resume");
        std::process::exit(ExitCode::Usage.code());
    }
    for d in [layout.out_dir(), layout.tmp_dir()] {
        if let Err(e) = std::fs::create_dir_all(&d) {
            eprintln!("rr_node job: {}: {e}", d.display());
            std::process::exit(cc::atomic::exit_for_io(&e).code());
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
    let j = Arc::new(RJob {
        spec,
        spec_blake3,
        layout,
        inputs,
        program: program.clone(),
        sink,
        started: Instant::now(),
        started_utc: cc::time::utc_now(),
        effective: Mutex::new(None),
        relation_rule: Mutex::new(Value::Null),
        keys: Mutex::new(vec![]),
        key_budget_s: Mutex::new(None),
        stop: AtomicBool::new(false),
        stop_source: Mutex::new(None),
        stop_at: Mutex::new(None),
        keys_done: AtomicU64::new(0),
        solved_done: AtomicU64::new(0),
        phase: AtomicU8::new(0),
        forward: Mutex::new(Forward::default()),
    });
    if RJOB.set(j.clone()).is_err() {
        eprintln!("rr_node job: one job per process");
        std::process::exit(ExitCode::Internal.code());
    }
    let _ = j.sink.emit(Event::JobStarted {
        job_id: j.spec.job_id.clone(),
        attempt: j.spec.attempt,
        spec_blake3: j.spec_blake3.clone(),
        pid: std::process::id(),
        program: super::PROGRAM.into(),
        version: program.version.clone(),
        git_commit: program.git_commit.clone(),
        resume_from: None,
    });
    let _ = j.sink.emit(Event::Phase {
        phase: Phase::Prepare,
    });
    let wd = j.clone();
    if let Err(e) = std::thread::Builder::new()
        .name("rr-job-watchdog".into())
        .spawn(move || watchdog(wd))
    {
        fail(ExitCode::Internal, &format!("watchdog thread: {e}"));
    }
    // inputs: read once, digests compared again, keys through the CONTRACT's reader (the supervisor's tally)
    let spec = &j.spec;
    let (family_path, family) = read_input(spec, "family")
        .unwrap_or_else(|| fail(ExitCode::Usage, "input role family missing"));
    let family_text = text("family", family);
    let (recipe_path, recipe_bytes) = read_input(spec, "recipe")
        .unwrap_or_else(|| fail(ExitCode::Usage, "input role recipe missing"));
    let recipe_text = text("recipe", recipe_bytes);
    // the recipe as the verb parses it: an unparsable recipe is an input error (41) before any work; its relation rule
    // (v1.2: m1_first default | first_admissible) is recorded in OUTCOME (effective.node_options and `relation_rule`)
    let parsed = recipe::Recipe::parse(&recipe_text)
        .unwrap_or_else(|e| fail(ExitCode::Input, &format!("recipe: {e}")));
    let relation_rule = serde_json::to_value(parsed.relation_rule).unwrap_or(Value::Null);
    *lock(&j.relation_rule) = relation_rule.clone();
    let (_, keys_bytes) = read_input(spec, "keys")
        .unwrap_or_else(|| fail(ExitCode::Usage, "input role keys missing"));
    let key_lines = parse_keys_input(&keys_bytes).unwrap_or_else(|e| fail(ExitCode::Input, &e));
    if key_lines.is_empty() {
        fail(ExitCode::Input, "the keys input has no key");
    }
    let expect: BTreeMap<Vec<i64>, String> = match read_input(spec, "expect") {
        Some((_, b)) => {
            recipe::parse_expect(&text("expect", b)).unwrap_or_else(|e| fail(ExitCode::Input, &e))
        }
        None => BTreeMap::new(),
    };
    let keys: Vec<recipe::KeyInput> = key_lines
        .iter()
        .map(|k| recipe::KeyInput {
            key: k.key.clone(),
            tag: k.tag.clone(),
        })
        .collect();
    *lock(&j.keys) = key_lines;
    let a = resolve(spec, &family_text);
    *lock(&j.key_budget_s) = a.key_budget_s;
    let order_json: Value = serde_json::from_str(&a.order.to_json()).unwrap_or(Value::Null);
    *lock(&j.effective) = Some({
        let mut e = Effective {
            subcommand: "solve-recipe".into(),
            args: spec.args.clone(),
            node_options: json!({"threads": a.threads.max(1), "max_trace_rows": a.max_trace_rows,
                "key_budget_s": a.key_budget_s, "proof": true, "expect_keys": expect.len(),
                "determinism_id": recipe::DETERMINISM_ID, "relation_rule": relation_rule,
                "recipe_id": parsed.id.clone(), "recipe_digest": parsed.digest()}),
            order: order_json,
            env: effective_env(),
            blake3: String::new(),
        };
        let bytes = serde_json::to_vec(&e).unwrap_or_default();
        e.blake3 = cc::canon::canonical_line(&bytes)
            .map(|c| cc::blake3_hex(&c))
            .unwrap_or_else(|_| cc::blake3_hex(&bytes));
        e
    });
    // the verb's informational `inputs` echo (its own OUTCOME in out/); the contract OUTCOME carries the preflight's
    let inputs_echo: Vec<Value> = spec
        .inputs
        .iter()
        .map(|i| json!({"role": i.role, "path": canon_path(&spec.input_path(i)), "blake3": i.blake3}))
        .collect();
    let stop_flag = j.clone();
    let end = j.clone();
    let opts = recipe::BatchOptions {
        threads: a.threads.max(1),
        max_trace_rows: a.max_trace_rows,
        proof: true,
        family_path: canon_path(&family_path),
        recipe_path: canon_path(&recipe_path),
        order_text: a.order.to_json(),
        binary: recipe::binary_identity(),
        key_budget_s: a.key_budget_s,
        expect,
        inputs: Value::from(inputs_echo),
        job: None,
        outcome_path: None,
        stop: Some(Arc::new(move || stop_flag.stop.load(SeqCst))),
        stop_file: None,
        stop_grace_s: f64::from(j.spec.stop_grace_s),
        path_base: Some(j.layout.root.clone()),
        events_path: None,
        heartbeat_s: f64::from(j.spec.heartbeat_s.max(1)),
        on_end: Some(Arc::new(move |v: &Value, exit: i32, graceful: bool| {
            finish(&end, v, exit, graceful)
        })),
    };
    let out = j.layout.out_dir();
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        recipe::run_batch(&family_text, &a.order, &recipe_text, &keys, &out, &opts)
    }));
    match res {
        Ok(Ok(code)) => std::process::exit(code),
        Ok(Err(e)) => fail(batch_error_code(&e), &e),
        Err(p) => {
            let m = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".into());
            fail(ExitCode::Internal, &format!("panic: {m}"))
        }
    }
}
