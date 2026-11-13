//! Supervisor-side classification of a finished attempt: child-job.md 6.5 (decision table, first matching row wins)
//! and 6.6 (supervisor restart, wait status unknown). Pure function [`classify`] over [`Evidence`]; [`assess`]
//! gathers the file evidence of a run directory (reads only).
//!
//! Inputs: W = wait status (exit code c or signal s or unknown); O = OUTCOME valid, internally consistent and
//! matching the spec; S = table sealed ([`crate::seal::verify_sealed`]; vacuously true for kinds without a table);
//! R = RESUME valid; K = the supervisor's own kill reason.
//!
//! Additions by package contract (2026-11-13), each documented in child-job.md:
//! - O counts as present-but-invalid (row 22) when its `exit_code` differs from c, when `check()` fails, or when the
//!   spec forbids witness-locus elision and the table reports elided loci (ruling R-f);
//! - row 24 (evaluated right after row 3): c=10, O, S, stop max_cases, no rule, some case result not in the
//!   "no rule needed" set => no_rule, next rung (the document left this case to no row);
//! - rows 19 and 20 also match SIGTERM / SIGINT / SIGHUP deaths (an L1 child may die on the supervisor's SIGTERM);
//! - W unknown (6.6): O valid => c := O.exit_code and the table applies; else row 21 (R valid: resume once) or
//!   row 21 with "restart from scratch" (R invalid).

use crate::exit::ExitCode;
use crate::layout::RunLayout;
use crate::outcome::{JobOutcome, Outcome};
use crate::resume::ResumeState;
use crate::seal::{prefix_blake3, verify_sealed, SealVerdict, UnsealReason};
use crate::spec::{JobKind, JobSpec};
use crate::stop::StopCode;
use serde::{Deserialize, Serialize};

/// Linux signal numbers used by the table.
pub const SIGHUP: i32 = 1;
pub const SIGINT: i32 = 2;
pub const SIGKILL: i32 = 9;
pub const SIGTERM: i32 = 15;

/// How the child ended, as `waitpid` reported it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitStatus {
    Exited(i32),
    Signaled(i32),
    /// Supervisor restart: the child was not ours to reap (6.6).
    Unknown,
}

/// The supervisor's own reason for killing the child.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KillReason {
    #[default]
    None,
    Time,
    Memory,
    Stop,
    Unresponsive,
}

/// State of OUTCOME.json.
#[derive(Debug, Clone, PartialEq)]
pub enum OutcomeState {
    Absent,
    /// Present but not parseable.
    Unreadable(String),
    /// Parseable but not ours / inconsistent (reason).
    Invalid(String),
    Valid(Box<JobOutcome>),
}

/// Everything the table looks at.
#[derive(Debug, Clone, PartialEq)]
pub struct Evidence {
    pub kind: JobKind,
    pub wait: WaitStatus,
    pub outcome: OutcomeState,
    /// S. For kinds without a table, pass `true` ([`assess`] does).
    pub sealed: bool,
    /// R.
    pub resume_valid: bool,
    pub kill: KillReason,
    /// The cgroup's `oom_kill` count increased during the attempt.
    pub oom_kill_observed: bool,
    /// The spec forbade witness-locus elision (R-f; the campaign's default).
    pub elision_forbidden: bool,
}

/// Outcome kinds (PLAN.v2 3.3.4 closed list, plus the two checker verdicts of row 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeKind {
    RuleFound,
    Complete,
    NoRule,
    Spill,
    Refused,
    KilledTime,
    KilledMemory,
    Interrupted,
    Crashed,
    UsageError,
    SetupError,
    Lost,
    DecidedNegative,
    Undecided,
}

/// What may be accepted from the attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accept {
    /// Every record of the sealed table (canonical form).
    Canonical,
    /// The committed rules of the sealed partial table.
    CommittedRules,
    Nothing,
}

/// The ledger action of the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Row 1.
    ItemDone,
    /// Row 2: RESUME.pending -> new child items.
    QueueChildren,
    /// Row 3: no rule needed; finite points -> point items.
    ItemDoneNoRule,
    /// Rows 4, 24: next rung for the open cases.
    NextRung,
    /// Row 5: open list -> items.
    QueueOpenList,
    /// Rows 6, 14, 22.
    Attention,
    /// Row 7, c = 11: record the verdict, never credit the checked rule.
    RecordNegativeVerdict,
    /// Row 7, c = 12: another tool or attention.
    OtherToolOrAttention,
    /// Row 8: next rung (never the same rung with a larger budget).
    NextRungAfterSpill,
    /// Rows 9, 19: the ladder decides; resume allowed from R.
    LadderDecides,
    /// Rows 10, 18: next memory class once if the policy allows, else next rung.
    NextMemoryClassOrRung,
    /// Rows 11, 20: on campaign resume, new attempt with `resume` = R (else from scratch).
    ResumeOnCampaignResume,
    /// Row 12: supervisor bug, stop launching this kind.
    StopLaunchingKind,
    /// Row 13: item attention, no retry.
    ItemAttentionNoRetry,
    /// Row 15: pause all launches until free space; item queued again.
    PauseAllLaunches,
    /// Row 16: retry once (resume from R), then attention.
    RetryOnceThenAttention,
    /// Row 17: retry once with the same spec, then attention.
    RetrySameSpecOnceThenAttention,
    /// Row 21: resume from R once, then attention.
    ResumeOnceThenAttention,
    /// Row 21 under 6.6 with R invalid.
    RestartFromScratch,
    /// Row 23: resume from R once if valid, else attention.
    ResumeOnceIfValidElseAttention,
}

/// The classification of one attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Classification {
    /// Row of the decision table (1-23 as in the document; 24 = added by package contract).
    pub row: u8,
    pub kind: OutcomeKind,
    pub accept: Accept,
    pub action: Action,
    /// A next attempt may start from RESUME (R valid and the row allows it).
    pub resume_from_r: bool,
    pub time_decided: bool,
    pub attention: bool,
    pub why: String,
}

fn row(n: u8, kind: OutcomeKind, accept: Accept, action: Action, why: impl Into<String>) -> Classification {
    let attention = matches!(action, Action::Attention | Action::StopLaunchingKind | Action::ItemAttentionNoRetry);
    Classification { row: n, kind, accept, action, resume_from_r: false, time_decided: false, attention, why: why.into() }
}

fn is_stop_signal(s: i32) -> bool {
    matches!(s, SIGKILL | SIGTERM | SIGINT | SIGHUP)
}

/// Apply the decision table.
pub fn classify(e: &Evidence) -> Classification {
    use Accept::*;
    use Action::*;
    use OutcomeKind::*;

    // normalise O
    let mut o_state = e.outcome.clone();
    if let OutcomeState::Valid(o) = &o_state {
        if let WaitStatus::Exited(c) = e.wait {
            if c != o.exit_code.code() {
                o_state = OutcomeState::Invalid(format!("OUTCOME exit_code {} but the process exited {c}", o.exit_code.code()));
            }
        }
    }
    if let OutcomeState::Valid(o) = &o_state {
        let elided = o.outputs.table.as_ref().map_or(0, |t| t.witness_loci_elided);
        if e.elision_forbidden && elided > 0 {
            o_state = OutcomeState::Invalid(format!("{elided} witness loci elided although the spec forbids elision"));
        }
    }
    let sealed = e.sealed || !e.kind.produces_table();
    let r = e.resume_valid;

    // 6.6: wait status unknown
    let wait = match (e.wait, &o_state) {
        (WaitStatus::Unknown, OutcomeState::Valid(o)) => WaitStatus::Exited(o.exit_code.code()),
        (WaitStatus::Unknown, _) => {
            let mut c = if r {
                row(21, Lost, Nothing, ResumeOnceThenAttention, "supervisor restart: no valid OUTCOME, RESUME valid")
            } else {
                row(21, Lost, Nothing, RestartFromScratch, "supervisor restart: no valid OUTCOME, no valid RESUME")
            };
            c.resume_from_r = r;
            return c;
        }
        (w, _) => w,
    };
    let o = match &o_state {
        OutcomeState::Valid(o) => Some(o.as_ref()),
        _ => None,
    };
    let o_time = o.is_some_and(|o| o.time_decided);
    let mut out = decide(wait, o, &o_state, sealed, e.kill, e.oom_kill_observed);
    if matches!(out.action, LadderDecides | NextMemoryClassOrRung | ResumeOnCampaignResume | PauseAllLaunches | RetryOnceThenAttention | ResumeOnceThenAttention | ResumeOnceIfValidElseAttention)
    {
        out.resume_from_r = r;
    }
    if out.action == ResumeOnceIfValidElseAttention && !r {
        out.attention = true;
    }
    out.time_decided = out.time_decided || o_time;
    out
}

fn decide(
    wait: WaitStatus,
    o: Option<&JobOutcome>,
    o_state: &OutcomeState,
    s: bool,
    k: KillReason,
    oom: bool,
) -> Classification {
    use Accept::*;
    use Action::*;
    use OutcomeKind::*;
    if let WaitStatus::Exited(c) = wait {
        if let Some(o) = o {
            let stop = o.stop.code;
            let rules = o.counters.rules_committed.max(o.outputs.table.as_ref().map_or(0, |t| t.rules_committed));
            match c {
                0 if s => return row(1, Complete, Canonical, ItemDone, "exit 0, OUTCOME, sealed"),
                10 if s && stop == Some(StopCode::MaxCases) => {
                    if rules >= 1 {
                        return row(2, RuleFound, Canonical, QueueChildren, "max_cases with committed rules");
                    }
                    if !o.per_case.is_empty() && o.per_case.iter().all(|p| p.result.needs_no_rule()) {
                        return row(3, Complete, Nothing, ItemDoneNoRule, "max_cases, no rule, nothing needed");
                    }
                    return row(24, NoRule, Nothing, NextRung, "max_cases, no rule, a case result needs a rule");
                }
                10 if s
                    && matches!(
                        stop,
                        Some(StopCode::LiftRefused | StopCode::GuardTimeout | StopCode::NumericSearchRefused | StopCode::BeforeNumeric)
                    ) =>
                {
                    return row(4, NoRule, CommittedRules, NextRung, "refused stop");
                }
                10 if s && matches!(stop, Some(StopCode::OpenRegions | StopCode::OpenObligations)) => {
                    return row(5, RuleFound, Canonical, QueueOpenList, "open regions / obligations");
                }
                10 if s && stop == Some(StopCode::NotLower) => {
                    return row(6, Crashed, Nothing, Attention, "engine invariant: not_lower");
                }
                11 => return row(7, DecidedNegative, Nothing, RecordNegativeVerdict, "checker decided FAIL"),
                12 => return row(7, Undecided, Nothing, OtherToolOrAttention, "checker undecided"),
                20 if s => return row(8, Spill, CommittedRules, NextRungAfterSpill, "work budget (deterministic)"),
                21 => {
                    let mut x = row(9, KilledTime, Nothing, LadderDecides, "wall budget");
                    x.time_decided = true;
                    return x;
                }
                22 => return row(10, KilledMemory, Nothing, NextMemoryClassOrRung, "soft memory limit"),
                30 => return row(11, Interrupted, Nothing, ResumeOnCampaignResume, "interrupted"),
                _ => {}
            }
        }
        match c {
            40 => return row(12, UsageError, Nothing, StopLaunchingKind, "usage error (supervisor bug)"),
            41 => return row(13, Refused, Nothing, ItemAttentionNoRetry, "input refused"),
            42 => return row(14, Refused, Nothing, Attention, "contract refusal"),
            50 => return row(15, Lost, Nothing, PauseAllLaunches, "no space"),
            51 => return row(16, Crashed, Nothing, RetryOnceThenAttention, "output I/O error"),
            70 if o.is_some() => return row(17, Crashed, Nothing, RetrySameSpecOnceThenAttention, "internal error"),
            _ => {}
        }
    }
    if let WaitStatus::Signaled(sig) = wait {
        if sig == SIGKILL && (k == KillReason::Memory || oom) {
            return row(18, KilledMemory, Nothing, NextMemoryClassOrRung, "SIGKILL for memory");
        }
        if is_stop_signal(sig) && matches!(k, KillReason::Time | KillReason::Unresponsive) {
            let mut x = row(19, KilledTime, Nothing, LadderDecides, "killed for time / unresponsive");
            x.time_decided = true;
            return x;
        }
        if is_stop_signal(sig) && k == KillReason::Stop {
            return row(20, Interrupted, Nothing, ResumeOnCampaignResume, "killed on a stop request");
        }
        if sig == SIGKILL && k == KillReason::None && matches!(o_state, OutcomeState::Absent) {
            return row(21, Lost, Nothing, ResumeOnceThenAttention, "SIGKILL from outside, no OUTCOME");
        }
    }
    match o_state {
        OutcomeState::Invalid(why) => return row(22, Crashed, Nothing, Attention, format!("contract violation: {why}")),
        OutcomeState::Unreadable(why) => {
            return row(22, Crashed, Nothing, Attention, format!("contract violation: OUTCOME unreadable: {why}"));
        }
        OutcomeState::Valid(_) if matches!(wait, WaitStatus::Exited(0 | 10 | 20)) && !s => {
            return row(22, Crashed, Nothing, Attention, "contract violation: OUTCOME but the table is not sealed");
        }
        _ => {}
    }
    row(23, Crashed, Nothing, ResumeOnceIfValidElseAttention, "crashed")
}

/// Validity of `RESUME.json` for `spec` (3.4.4 item 3): parses, schema, job_id / attempt match, and the BLAKE3 of
/// the first `table_committed_bytes` bytes of `out/F.table.jsonl` is `table_prefix_blake3`.
pub fn resume_state(layout: &RunLayout, spec: &JobSpec) -> Result<ResumeState, String> {
    let b = std::fs::read(layout.resume()).map_err(|e| e.to_string())?;
    let r: ResumeState = serde_json::from_slice(&b).map_err(|e| e.to_string())?;
    r.check()?;
    if r.job_id != spec.job_id || r.attempt != spec.attempt {
        return Err("RESUME job_id / attempt do not match the spec".into());
    }
    match prefix_blake3(&layout.table(), r.table_committed_bytes) {
        Ok(Some(d)) if d == r.table_prefix_blake3 => Ok(r),
        Ok(_) => Err("committed prefix digest mismatch".into()),
        Err(e) => Err(format!("table: {e}")),
    }
}

/// The OUTCOME state of a run for `spec`.
pub fn outcome_state(layout: &RunLayout, spec: &JobSpec) -> OutcomeState {
    let b = match std::fs::read(layout.outcome()) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return OutcomeState::Absent,
        Err(e) => return OutcomeState::Unreadable(e.to_string()),
    };
    let o = match Outcome::from_slice(&b) {
        Ok(o) => o,
        Err(m) => return OutcomeState::Unreadable(m),
    };
    if let Err(m) = o.job.check() {
        return OutcomeState::Invalid(m);
    }
    if !o.job.matches(spec) {
        return OutcomeState::Invalid("job_id / attempt / kind do not match the spec".into());
    }
    OutcomeState::Valid(Box::new(o.job))
}

/// The seal verdict of a run's table (`None` for kinds without a table or without a valid OUTCOME).
pub fn table_verdict(layout: &RunLayout, kind: JobKind, o: &OutcomeState) -> Option<SealVerdict> {
    if !kind.produces_table() {
        return None;
    }
    let OutcomeState::Valid(o) = o else { return None };
    Some(match &o.outputs.table {
        None => SealVerdict::Unsealed(UnsealReason::OutcomeIncomplete),
        Some(t) => verify_sealed(&layout.resolve(&t.path), t),
    })
}

/// Gather the evidence of a finished attempt from its run directory.
pub fn assess(layout: &RunLayout, spec: &JobSpec, wait: WaitStatus, kill: KillReason, oom_kill_observed: bool) -> Evidence {
    let outcome = outcome_state(layout, spec);
    let sealed = match table_verdict(layout, spec.kind, &outcome) {
        None => !spec.kind.produces_table(),
        Some(v) => v.is_sealed(),
    };
    Evidence {
        kind: spec.kind,
        wait,
        outcome,
        sealed,
        resume_valid: resume_state(layout, spec).is_ok(),
        kill,
        oom_kill_observed,
        elision_forbidden: spec.elision_forbidden(),
    }
}

/// Exit code of a wait status if it is a table entry.
pub fn table_exit(w: WaitStatus) -> Option<ExitCode> {
    match w {
        WaitStatus::Exited(c) => ExitCode::from_code(c),
        _ => None,
    }
}
