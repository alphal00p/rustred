//! `OUTCOME.json` (child-job.md 3.7): the legacy top-level fields kept verbatim plus ONE new key `job`.
//! Readers tolerate unknown fields everywhere (no `deny_unknown_fields`).

use crate::exit::ExitCode;
use crate::spec::{BudgetHit, Counters, JobKind, JobSpec, WallBudgets, WorkBudgets};
use crate::stop::{CaseResult, JobStatus, StopCode, StopSource};
use crate::{OUTCOME_SCHEMA, is_blake3_hex};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// The whole file. `legacy` holds every top-level key except `job` (the legacy `NodeOutcome` fields for
/// generate/witness, `result` for the other kinds).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    #[serde(flatten)]
    pub legacy: serde_json::Map<String, Value>,
    pub job: JobOutcome,
}

/// `job.stop`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StopInfo {
    pub code: Option<StopCode>,
    #[serde(default)]
    pub detail: Value,
    #[serde(default)]
    pub source: Option<StopSource>,
    /// False only for the hard-safe stop (3.4.3).
    #[serde(default = "t")]
    pub graceful: bool,
}

fn t() -> bool {
    true
}

impl StopInfo {
    pub fn none() -> StopInfo {
        StopInfo {
            code: None,
            detail: Value::Null,
            source: None,
            graceful: true,
        }
    }
}

/// `job.program.exe`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExeInfo {
    pub path: String,
    pub dev: u64,
    pub ino: u64,
    pub size: u64,
    pub mtime: i64,
}

/// `job.program`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramInfo {
    pub name: String,
    pub version: String,
    pub git_commit: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub exe: Option<ExeInfo>,
}

/// `job.effective` (3.2.4). `env`: the behaviour-changing variables that are set, values verbatim except the
/// licence variable, whose value is replaced by [`LICENCE_PRESENT`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Effective {
    pub subcommand: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub node_options: Value,
    #[serde(default)]
    pub order: Value,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// BLAKE3 of the compact JSON of `{subcommand,args,node_options,order,env}`.
    pub blake3: String,
}

/// Placeholder recorded for the licence variable (presence only, never the value).
pub const LICENCE_PRESENT: &str = "<present>";

/// `job.seed.point_source`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointSource {
    #[default]
    Seed,
    Env,
    Region,
}

/// `job.seed`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedInfo {
    pub sample_seed: u64,
    #[serde(default)]
    pub point_source: PointSource,
    /// BLAKE3 of `RUSTRED_PROBE_POINT` when `point_source = env`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point_blake3: Option<String>,
}

/// One entry of `job.inputs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputCheck {
    pub role: String,
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
    pub verified: bool,
}

/// `job.budgets`: the spec's budgets echoed plus the budget that fired.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BudgetsEcho {
    #[serde(default)]
    pub work: WorkBudgets,
    #[serde(default)]
    pub wall: WallBudgets,
    #[serde(default)]
    pub mem_soft_bytes: Option<u64>,
    #[serde(default)]
    pub hit: Option<BudgetHit>,
}

/// `what` of a time decision (3.3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeDecisionWhat {
    GuardRoute,
    GuardTimeout,
    Admission,
    DeferredCheck,
    ScreenSampled,
    ForkUnavailable,
    CaseWall,
    JobWall,
}

/// One decision taken by a clock (`job.time_decisions[]`, event `time_decision`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeDecision {
    pub what: TimeDecisionWhat,
    #[serde(default)]
    pub case_ordinal: Option<u64>,
    #[serde(default)]
    pub rule: Option<u64>,
    pub limit_s: f64,
    pub measured_s: f64,
    pub taken: String,
}

/// One entry of `job.per_case`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerCase {
    pub ordinal: u64,
    #[serde(default)]
    pub case: Value,
    pub result: CaseResult,
    #[serde(default)]
    pub rule: Option<u64>,
    #[serde(default)]
    pub children_open: u64,
}

/// End record kind of a table (`end` = complete, `partial` = stopped).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndKind {
    End,
    Partial,
}

impl EndKind {
    pub fn from_type(t: &str) -> Option<EndKind> {
        match t {
            "end" => Some(EndKind::End),
            "partial" => Some(EndKind::Partial),
            _ => None,
        }
    }
    pub const fn as_str(self) -> &'static str {
        match self {
            EndKind::End => "end",
            EndKind::Partial => "partial",
        }
    }
}

/// `job.outputs.table`. On a hard-safe stop `bytes`, `blake3`, `end`, `canonical_blake3` are null and `sealed` is
/// false; `committed_bytes` / `prefix_blake3` describe the last commit point.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableOutput {
    pub path: String,
    #[serde(default)]
    pub bytes: Option<u64>,
    #[serde(default)]
    pub blake3: Option<String>,
    #[serde(default)]
    pub records: Option<u64>,
    #[serde(default)]
    pub rules: Option<u64>,
    #[serde(default)]
    pub rules_committed: u64,
    #[serde(default)]
    pub committed_bytes: u64,
    #[serde(default)]
    pub prefix_blake3: Option<String>,
    #[serde(default)]
    pub sealed: bool,
    #[serde(default)]
    pub end: Option<EndKind>,
    #[serde(default)]
    pub canonical_blake3: Option<String>,
    /// Number of witness loci written as "elided" (R-f). Must be 0 when the spec forbids elision.
    #[serde(default)]
    pub witness_loci_elided: u64,
}

/// `job.outputs.cert`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamOutput {
    pub path: String,
    #[serde(default)]
    pub bytes: Option<u64>,
    #[serde(default)]
    pub blake3: Option<String>,
    #[serde(default)]
    pub sealed: bool,
}

/// `job.outputs.queue`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueOutput {
    pub bytes: u64,
}

/// `job.outputs.events`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventsOutput {
    pub last_seq: u64,
}

/// `job.outputs.resume`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumeOutput {
    pub path: String,
    pub valid: bool,
}

/// One of `job.outputs.extra`: a sealed JSONL result file of the kind's sealing rule (audit A M4; role and layout
/// path from [`crate::spec::JobKind::sealed_outputs`]). Every figure is recomputed by the verifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtraOutput {
    pub role: String,
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
    #[serde(default)]
    pub records: Option<u64>,
    #[serde(default)]
    pub canonical_blake3: Option<String>,
    #[serde(default)]
    pub end: Option<EndKind>,
    #[serde(default)]
    pub sealed: bool,
}

/// `job.outputs`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outputs {
    #[serde(default)]
    pub table: Option<TableOutput>,
    #[serde(default)]
    pub cert: Option<StreamOutput>,
    #[serde(default)]
    pub queue: Option<QueueOutput>,
    #[serde(default)]
    pub events: Option<EventsOutput>,
    #[serde(default)]
    pub resume: Option<ResumeOutput>,
    #[serde(default)]
    pub extra: Vec<ExtraOutput>,
}

/// `job.timing` (volatile).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Timing {
    #[serde(default)]
    pub started_utc: Option<String>,
    #[serde(default)]
    pub finished_utc: Option<String>,
    #[serde(default)]
    pub wall_s: Option<f64>,
    #[serde(default)]
    pub cpu_s: Option<f64>,
}

/// `job.memory` (volatile).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    #[serde(default)]
    pub vmhwm_kb: Option<u64>,
    #[serde(default)]
    pub rss_kb_at_end: Option<u64>,
    #[serde(default)]
    pub helpers_peak_rss_kb: Option<u64>,
}

/// `job.error.class`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    Usage,
    Input,
    Refused,
    Io,
    Nospace,
    Internal,
    Panic,
}

/// `job.error`. `message` is at most [`ERROR_MESSAGE_MAX`] characters ([`ErrorInfo::new`] truncates).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub class: ErrorClass,
    pub message: String,
}

pub const ERROR_MESSAGE_MAX: usize = 4000;

impl ErrorInfo {
    pub fn new(class: ErrorClass, message: impl Into<String>) -> ErrorInfo {
        let m: String = message.into();
        let message = if m.chars().count() > ERROR_MESSAGE_MAX {
            m.chars().take(ERROR_MESSAGE_MAX).collect()
        } else {
            m
        };
        ErrorInfo { class, message }
    }
}

/// The `job` object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobOutcome {
    pub schema: String,
    pub job_id: String,
    pub attempt: u32,
    pub kind: JobKind,
    pub spec_blake3: String,
    pub status: JobStatus,
    pub exit_code: ExitCode,
    pub stop: StopInfo,
    pub program: ProgramInfo,
    #[serde(default)]
    pub effective: Option<Effective>,
    #[serde(default)]
    pub seed: SeedInfo,
    #[serde(default)]
    pub inputs: Vec<InputCheck>,
    #[serde(default)]
    pub counters: Counters,
    #[serde(default)]
    pub budgets: BudgetsEcho,
    #[serde(default)]
    pub time_decisions: Vec<TimeDecision>,
    #[serde(default)]
    pub time_decided: bool,
    #[serde(default)]
    pub per_case: Vec<PerCase>,
    #[serde(default)]
    pub outputs: Outputs,
    #[serde(default)]
    pub timing: Timing,
    #[serde(default)]
    pub memory: Memory,
    #[serde(default)]
    pub error: Option<ErrorInfo>,
    /// True when this attempt started from a RESUME of an earlier attempt (3.8.2).
    #[serde(default)]
    pub resumed: bool,
}

impl JobOutcome {
    /// A skeleton for `spec`: status, exit code and stop from `code` / `stop`, everything else empty.
    pub fn for_spec(
        spec: &JobSpec,
        spec_blake3: &str,
        program: ProgramInfo,
        code: ExitCode,
        stop: StopInfo,
    ) -> Self {
        JobOutcome {
            schema: OUTCOME_SCHEMA.to_string(),
            job_id: spec.job_id.clone(),
            attempt: spec.attempt,
            kind: spec.kind,
            spec_blake3: spec_blake3.to_string(),
            status: JobStatus::for_exit(code),
            exit_code: code,
            stop,
            program,
            effective: None,
            seed: SeedInfo {
                sample_seed: spec.seed.sample_seed,
                point_source: PointSource::Seed,
                point_blake3: None,
            },
            inputs: Vec::new(),
            counters: Counters::default(),
            budgets: BudgetsEcho {
                work: spec.budgets.work,
                wall: spec.budgets.wall,
                mem_soft_bytes: spec.budgets.mem_soft_bytes,
                hit: None,
            },
            time_decisions: Vec::new(),
            time_decided: false,
            per_case: Vec::new(),
            outputs: Outputs::default(),
            timing: Timing::default(),
            memory: Memory::default(),
            error: None,
            resumed: false,
        }
    }

    /// Internal consistency (fails closed; the supervisor treats a failure as a contract violation):
    /// schema; status = status of the exit code; the stop code is consistent with the exit code; `time_decided`
    /// iff `time_decisions` is non-empty, or the stop is a wall budget; a sealed table names bytes, digest and end
    /// kind, and its end is `end` iff the stop is null; digests are hex; error message length. Revised by audit A
    /// (C2, M3, m2): layout paths only; a sealed table names every figure the verifier re-checks; `error` is null
    /// for exit 0 / 10 / 11 / 12 / 20; `stop.graceful = false` only with 21 / 22 / 30 / 70.
    pub fn check(&self) -> Result<(), String> {
        if self.schema != OUTCOME_SCHEMA {
            return Err(format!(
                "job.schema {:?} is not {OUTCOME_SCHEMA:?}",
                self.schema
            ));
        }
        if self.status != JobStatus::for_exit(self.exit_code) {
            return Err(format!(
                "status {:?} does not go with exit code {}",
                self.status, self.exit_code
            ));
        }
        if let Some(c) = self.stop.code {
            let checker_verdict = self.kind.is_checker()
                && matches!(
                    self.exit_code,
                    ExitCode::DecidedNegative | ExitCode::Undecided
                );
            if !c.consistent_with(self.exit_code) && !checker_verdict {
                return Err(format!(
                    "stop {c:?} does not go with exit code {}",
                    self.exit_code
                ));
            }
        } else if !matches!(
            self.exit_code,
            ExitCode::Complete | ExitCode::DecidedNegative | ExitCode::Undecided | ExitCode::Usage
        ) {
            return Err(format!("exit code {} needs a stop code", self.exit_code));
        }
        let wall = self.stop.code == Some(StopCode::BudgetWall);
        if self.time_decided != (!self.time_decisions.is_empty() || wall) {
            return Err("time_decided must be true iff time_decisions is non-empty or the stop is budget_wall".into());
        }
        if let Some(t) = &self.outputs.table {
            let rel = crate::layout::RunLayout::out_rel(crate::layout::TABLE);
            if t.path != rel {
                return Err(format!(
                    "outputs.table.path {:?} is not the layout path {rel:?}",
                    t.path
                ));
            }
            if t.sealed {
                let (Some(_), Some(d), Some(e), Some(_), Some(_), Some(c)) = (
                    t.bytes,
                    &t.blake3,
                    t.end,
                    t.records,
                    t.rules,
                    &t.canonical_blake3,
                ) else {
                    return Err(
                        "a sealed table needs bytes, blake3, end, records, rules and canonical_blake3".into(),
                    );
                };
                if !is_blake3_hex(d) || !is_blake3_hex(c) {
                    return Err("outputs.table digests must be BLAKE3 hex".into());
                }
                if (e == EndKind::End) != self.stop.code.is_none() {
                    return Err("table end kind must be `end` iff there is no stop".into());
                }
            }
            if let Some(d) = &t.prefix_blake3
                && !is_blake3_hex(d)
            {
                return Err("outputs.table.prefix_blake3 must be BLAKE3 hex".into());
            }
        }
        if let Some(c) = &self.outputs.cert {
            let rel = crate::layout::RunLayout::out_rel(crate::layout::CERT);
            if c.path != rel {
                return Err(format!(
                    "outputs.cert.path {:?} is not the layout path {rel:?}",
                    c.path
                ));
            }
        }
        if let Some(r) = &self.outputs.resume
            && r.path != crate::layout::RESUME
        {
            return Err(format!(
                "outputs.resume.path {:?} is not {:?}",
                r.path,
                crate::layout::RESUME
            ));
        }
        for x in &self.outputs.extra {
            let ok = x
                .path
                .strip_prefix("out/")
                .is_some_and(|n| !n.is_empty() && !n.contains('/') && n != "." && n != "..");
            if !ok {
                return Err(format!(
                    "outputs.extra path {:?} is not a file of out/",
                    x.path
                ));
            }
        }
        let success = matches!(
            self.exit_code,
            ExitCode::Complete
                | ExitCode::Incomplete
                | ExitCode::DecidedNegative
                | ExitCode::Undecided
                | ExitCode::BudgetWork
        );
        if success && self.error.is_some() {
            return Err(format!(
                "exit code {} with a non-null error",
                self.exit_code
            ));
        }
        if !self.stop.graceful
            && !matches!(
                self.exit_code,
                ExitCode::BudgetWall
                    | ExitCode::BudgetMemory
                    | ExitCode::Interrupted
                    | ExitCode::Internal
            )
        {
            return Err(format!(
                "stop.graceful false with exit code {}",
                self.exit_code
            ));
        }
        if !is_blake3_hex(&self.spec_blake3) {
            return Err("spec_blake3 is not a BLAKE3 hex digest".into());
        }
        if let Some(e) = &self.error
            && e.message.chars().count() > ERROR_MESSAGE_MAX
        {
            return Err("error.message longer than 4000 characters".into());
        }
        Ok(())
    }

    /// Whether this outcome belongs to THIS attempt of THIS spec (audit A M1, B F5): `job_id`, `attempt`, `kind` and
    /// `spec_blake3` all equal the supervisor's own values (`spec_blake3` = the digest of the `job.json` it wrote,
    /// which also binds `run_dir`).
    pub fn matches(&self, spec: &JobSpec, spec_blake3: &str) -> bool {
        self.job_id == spec.job_id
            && self.attempt == spec.attempt
            && self.kind == spec.kind
            && self.spec_blake3 == spec_blake3
    }
}

impl Outcome {
    pub fn new(job: JobOutcome) -> Outcome {
        Outcome {
            legacy: serde_json::Map::new(),
            job,
        }
    }

    /// Pretty JSON + "\n".
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = serde_json::to_vec_pretty(self).expect("Outcome serialises");
        v.push(b'\n');
        v
    }

    pub fn from_slice(b: &[u8]) -> Result<Outcome, String> {
        serde_json::from_slice(b).map_err(|e| e.to_string())
    }

    pub fn read(path: &Path) -> std::io::Result<Result<Outcome, String>> {
        Ok(Outcome::from_slice(&std::fs::read(path)?))
    }

    /// Atomic write that REPLACES an existing file (temp + fsync + rename + directory fsync): tests and tools only.
    pub fn write_atomic(&self, path: &Path) -> std::io::Result<()> {
        crate::atomic::write_atomic(path, &self.to_bytes())
    }

    /// The child's OUTCOME write (audit A C4): atomic and NO-CLOBBER ([`crate::atomic::write_atomic_new`]); fails
    /// with `AlreadyExists` when an OUTCOME is already there.
    pub fn write_new(&self, path: &Path) -> std::io::Result<()> {
        crate::atomic::write_atomic_new(path, &self.to_bytes())
    }
}
