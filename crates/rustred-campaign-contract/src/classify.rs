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

// lane adapters: kind solve-recipe's records and per-key outcomes (crate root belongs to the integration lane)
#[path = "recipe.rs"]
pub mod recipe;

use crate::exit::ExitCode;
use crate::layout::RunLayout;
use crate::outcome::{JobOutcome, Outcome};
use crate::resume::{ResumeRef, ResumeState};
use crate::seal::{
    CERT_END_TYPES, SealVerdict, TABLE_END_TYPES, UnsealReason, VerifiedStream, compare_figures,
    open_in_run, verify_prefix, verify_stream, verify_table,
};
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

/// What the verifier established about the outputs of an attempt (S), from the files at their LAYOUT paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedOutputs {
    /// Rule records of the verified table (0 for kinds without a table).
    pub rules: u64,
    /// Elided witness loci found by the verifier in every verified stream (R-f, M2).
    pub elided_loci: u64,
    /// Every verified stream (table first), with (dev, inode), size and digests: the importer re-checks them.
    pub streams: Vec<VerifiedStream>,
    /// Kind solve-recipe (lane adapters): the tally of the verified results file against the job's keys input
    /// ([`recipe::key_outcomes`]); `None` for every other kind. For solve-recipe, S needs it.
    pub relations: Option<recipe::RelationTally>,
}

/// Everything the table looks at.
#[derive(Debug, Clone, PartialEq)]
pub struct Evidence {
    pub kind: JobKind,
    pub wait: WaitStatus,
    pub outcome: OutcomeState,
    /// S: `Some` iff every output of the kind's sealing rule is sealed, verified at its layout path and agrees with
    /// every figure of OUTCOME ([`assess`]); `None` otherwise (never "vacuously true").
    pub verified: Option<VerifiedOutputs>,
    /// Why S is false (diagnostics only).
    pub unsealed_why: Option<String>,
    /// R.
    pub resume_valid: bool,
    /// The resume source a next attempt must use when R is valid: the LAST RESUME record (path + digest), never
    /// OUTCOME's `committed_bytes` (3.8.2; audit A C1, B F1).
    pub resume_ref: Option<ResumeRef>,
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
    /// Row 25: sealed, but the verifier found elided witness loci (elision allowed by the spec); never used (R-f).
    SealedWithElided,
    /// Row 26: a "success" of a kind without a sealing rule; nothing can be verified, nothing is accepted.
    UnverifiableKind,
}

/// What may be accepted from the attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accept {
    /// Every record of the sealed table (canonical form).
    Canonical,
    /// The committed rules of the sealed partial table.
    CommittedRules,
    /// Kind solve-recipe (row 28, lane adapters): each SOLVED + proved record of the sealed results file, on its own
    /// ([`recipe::KeyOutcome::Accepted`]); the other records are typed per-key outcomes.
    SolvedRecords,
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
    /// Row 3 (lane contractfix2, audit ac2-m9; was `item_done_no_rule`): the child CLAIMS (OUTCOME `per_case`) that
    /// no case needs a rule; the claims are bound to no sealed record, so they never close the item by themselves:
    /// the consumer closes it only when its OWN coverage check (forward closure / partition of the item's keys)
    /// confirms every key, else it treats the attempt as row 24 (next rung).
    ConfirmCoverageElseNextRung,
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
    /// Row 25 (`sealed_with_elided`): sealed, but the verifier found elided witness loci; never used (R-f): run the
    /// item again with elision forbidden.
    RerunWithoutElision,
    /// Row 26 (`unverifiable_kind`): a "success" of a kind without a sealing rule; nothing accepted, attention.
    UnverifiableKind,
    /// Rows 28 / 29 (kind solve-recipe, lane adapters): every unsolved key is a typed per-key outcome routed by its
    /// failure class ([`recipe::FailureClass::route`]).
    QueueFailures,
}

/// One row of the decision table (6.5): the single source of the classifier AND of the document's table
/// ([`decision_table_markdown`], asserted equal to child-job.md by a test).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowSpec {
    pub row: u8,
    /// Variant letter of a row with two outcomes ("" or "b" / "c").
    pub sub: &'static str,
    pub condition: &'static str,
    pub kind: OutcomeKind,
    pub accept: Accept,
    pub action: Action,
}

const fn rs(
    row: u8,
    sub: &'static str,
    condition: &'static str,
    kind: OutcomeKind,
    accept: Accept,
    action: Action,
) -> RowSpec {
    RowSpec {
        row,
        sub,
        condition,
        kind,
        accept,
        action,
    }
}

/// The decision table. Evaluation order (first match wins): O normalisation (row 22 reasons), W unknown (21b, 21c,
/// 22), then the c rows in this order: 26, the kind's exit-code set (22), S (22), 25, then for kind solve-recipe 1 /
/// 28 / 29 / 22 and for every other kind 1-8 with 24 and 27 after their siblings, 9-17 (11b / 11c before 11: the supervisor's kill reason K decides over an `interrupted` OUTCOME); then
/// the signal rows 18-21; then 22; then 23.
pub const ROWS: [RowSpec; 34] = {
    use Accept::*;
    use Action::*;
    use OutcomeKind::*;
    [
        rs(1, "", "c=0, O, S", Complete, Canonical, ItemDone),
        rs(
            2,
            "",
            "c=10, O, S, stop max_cases, verified rules >= 1",
            RuleFound,
            Canonical,
            QueueChildren,
        ),
        rs(
            3,
            "",
            "c=10, O, S, stop max_cases, no verified rule, every case result CLAIMED covered / empty / finite / declared_terminal (OUTCOME per_case, unverified: never closes the item by itself)",
            NoRule,
            Nothing,
            ConfirmCoverageElseNextRung,
        ),
        rs(
            24,
            "",
            "c=10, O, S, stop max_cases, no verified rule, another case result or no per-case list",
            NoRule,
            Nothing,
            NextRung,
        ),
        rs(
            4,
            "",
            "c=10, O, S, stop lift_refused / guard_timeout / numeric_search_refused / before_numeric",
            NoRule,
            CommittedRules,
            NextRung,
        ),
        rs(
            5,
            "",
            "c=10, O, S, stop open_regions / open_obligations, verified rules >= 1",
            RuleFound,
            Canonical,
            QueueOpenList,
        ),
        rs(
            27,
            "",
            "c=10, O, S, stop open_regions / open_obligations, no verified rule",
            NoRule,
            Nothing,
            QueueOpenList,
        ),
        rs(
            6,
            "",
            "c=10, O, S, stop not_lower",
            Crashed,
            Nothing,
            Attention,
        ),
        rs(
            28,
            "",
            "c=10, O, S, kind solve-recipe, stop keys_unsolved, accepted relation records >= 1",
            RuleFound,
            SolvedRecords,
            QueueFailures,
        ),
        rs(
            29,
            "",
            "c=10, O, S, kind solve-recipe, stop keys_unsolved, no accepted relation record",
            NoRule,
            Nothing,
            QueueFailures,
        ),
        rs(
            7,
            "",
            "c=11, O, checker kind",
            DecidedNegative,
            Nothing,
            RecordNegativeVerdict,
        ),
        rs(
            7,
            "b",
            "c=12, O, checker kind",
            Undecided,
            Nothing,
            OtherToolOrAttention,
        ),
        rs(
            8,
            "",
            "c=20, O, S (producing kind)",
            Spill,
            CommittedRules,
            NextRungAfterSpill,
        ),
        rs(
            25,
            "",
            "c in {0, 10, 20}, O, S, the spec allowed elision and the verifier found elided witness loci",
            SealedWithElided,
            Nothing,
            RerunWithoutElision,
        ),
        rs(
            26,
            "",
            "c in {0, 10, 20}, O, the kind has no sealing rule (generate-auto)",
            OutcomeKind::UnverifiableKind,
            Nothing,
            Action::UnverifiableKind,
        ),
        rs(9, "", "c=21, O", KilledTime, Nothing, LadderDecides),
        rs(
            10,
            "",
            "c=22, O",
            KilledMemory,
            Nothing,
            NextMemoryClassOrRung,
        ),
        rs(
            11,
            "b",
            "c=30, O, K=time or unresponsive (the supervisor stopped the child for time or silence)",
            KilledTime,
            Nothing,
            LadderDecides,
        ),
        rs(
            11,
            "c",
            "c=30, O, K=memory (the supervisor stopped the child for memory)",
            KilledMemory,
            Nothing,
            NextMemoryClassOrRung,
        ),
        rs(
            11,
            "",
            "c=30, O, K=none or stop",
            Interrupted,
            Nothing,
            ResumeOnCampaignResume,
        ),
        rs(12, "", "c=40", UsageError, Nothing, StopLaunchingKind),
        rs(13, "", "c=41", Refused, Nothing, ItemAttentionNoRetry),
        rs(14, "", "c=42", Refused, Nothing, Attention),
        rs(15, "", "c=50", Lost, Nothing, PauseAllLaunches),
        rs(16, "", "c=51", Crashed, Nothing, RetryOnceThenAttention),
        rs(
            17,
            "",
            "c=70, O",
            Crashed,
            Nothing,
            RetrySameSpecOnceThenAttention,
        ),
        rs(
            18,
            "",
            "s=9, K=memory (or cgroup oom_kill count increased)",
            KilledMemory,
            Nothing,
            NextMemoryClassOrRung,
        ),
        rs(
            19,
            "",
            "s in {9, 15, 2, 1}, K=time or unresponsive",
            KilledTime,
            Nothing,
            LadderDecides,
        ),
        rs(
            20,
            "",
            "s in {9, 15, 2, 1}, K=stop",
            Interrupted,
            Nothing,
            ResumeOnCampaignResume,
        ),
        rs(
            21,
            "",
            "s=9, K=none, no OUTCOME",
            Lost,
            Nothing,
            ResumeOnceThenAttention,
        ),
        rs(
            21,
            "b",
            "W unknown (supervisor restart), no OUTCOME, R valid",
            Lost,
            Nothing,
            ResumeOnceThenAttention,
        ),
        rs(
            21,
            "c",
            "W unknown, no OUTCOME, R invalid",
            Lost,
            Nothing,
            RestartFromScratch,
        ),
        rs(
            22,
            "",
            "contract violation: OUTCOME unreadable, inconsistent (`check`), of another attempt or spec (job_id, attempt, kind, spec_blake3), exit_code != c, a path not the layout path, family_blake3 not the spec's, elided loci under \"forbidden\"; or c in {0, 10, 20} with O and an output not sealed / not verified; or c outside the kind's set (checker 10 / 20, producing kind 11 / 12, solve-recipe 20 or an exit-10 stop other than keys_unsolved, keys_unsolved of another kind, proof-instantiate 10 / 20); or solve-recipe exit 0 with an unsolved record, keys_unsolved without one; or W unknown with an invalid OUTCOME",
            Crashed,
            Nothing,
            Attention,
        ),
        rs(
            23,
            "",
            "any other c or s (an exit status 128+N is read as signal N)",
            Crashed,
            Nothing,
            ResumeOnceIfValidElseAttention,
        ),
    ]
};

/// The classification of one attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Classification {
    /// Row of the decision table ([`ROWS`]).
    pub row: u8,
    /// Variant of the row ("" or "b" / "c").
    pub sub: String,
    pub kind: OutcomeKind,
    pub accept: Accept,
    pub action: Action,
    /// A next attempt may start from RESUME (R valid and the row allows it).
    pub resume_from_r: bool,
    /// The resume reference to put into the next attempt's spec (set iff `resume_from_r`): the last RESUME record.
    pub resume_from: Option<ResumeRef>,
    pub time_decided: bool,
    pub attention: bool,
    pub why: String,
}

fn row(n: u8, sub: &str, why: impl Into<String>) -> Classification {
    let r = ROWS
        .iter()
        .find(|r| r.row == n && r.sub == sub)
        .expect("row in the decision table");
    let attention = matches!(
        r.action,
        Action::Attention
            | Action::StopLaunchingKind
            | Action::ItemAttentionNoRetry
            | Action::UnverifiableKind
    );
    Classification {
        row: n,
        sub: sub.to_string(),
        kind: r.kind,
        accept: r.accept,
        action: r.action,
        resume_from_r: false,
        resume_from: None,
        time_decided: false,
        attention,
        why: why.into(),
    }
}

/// The decision table as the markdown block of child-job.md 6.5 (generated from [`ROWS`]).
pub fn decision_table_markdown() -> String {
    let mut s =
        String::from("| # | condition | kind | accept | ledger action |\n|---|---|---|---|---|\n");
    for r in ROWS.iter() {
        let name = |v: String| v.trim_matches('"').to_string();
        s.push_str(&format!(
            "| {}{} | {} | {} | {} | {} |\n",
            r.row,
            r.sub,
            r.condition,
            name(serde_json::to_string(&r.kind).unwrap()),
            name(serde_json::to_string(&r.accept).unwrap()),
            name(serde_json::to_string(&r.action).unwrap()),
        ));
    }
    s
}

fn is_stop_signal(s: i32) -> bool {
    matches!(s, SIGKILL | SIGTERM | SIGINT | SIGHUP)
}

/// An exit status 128+N (a shell or `timeout` wrapper reporting death by signal N) is read as signal N (audit A m3).
pub fn normalise_wait(w: WaitStatus) -> WaitStatus {
    match w {
        WaitStatus::Exited(c) if (129..=192).contains(&c) => WaitStatus::Signaled(c - 128),
        w => w,
    }
}

/// Apply the decision table.
pub fn classify(e: &Evidence) -> Classification {
    use Action::*;
    let wait = normalise_wait(e.wait);
    let mut o_state = e.outcome.clone();
    if let OutcomeState::Valid(o) = &o_state
        && let WaitStatus::Exited(c) = wait
        && c != o.exit_code.code()
    {
        o_state = OutcomeState::Invalid(format!(
            "OUTCOME exit_code {} but the process exited {c}",
            o.exit_code.code()
        ));
    }
    let elided = e.verified.as_ref().map_or(0, |v| v.elided_loci);
    if let OutcomeState::Valid(_) = &o_state
        && e.elision_forbidden
        && elided > 0
    {
        o_state = OutcomeState::Invalid(format!(
            "{elided} elided witness loci found by the verifier although the spec forbids elision"
        ));
    }
    let r = e.resume_valid;
    let wait = match (wait, &o_state) {
        (WaitStatus::Unknown, OutcomeState::Valid(o)) => WaitStatus::Exited(o.exit_code.code()),
        (WaitStatus::Unknown, OutcomeState::Absent) => {
            let mut c = if r {
                row(21, "b", "supervisor restart: no OUTCOME, RESUME valid")
            } else {
                row(21, "c", "supervisor restart: no OUTCOME, no valid RESUME")
            };
            c.resume_from_r = r;
            c.resume_from = if r { e.resume_ref.clone() } else { None };
            return c;
        }
        (WaitStatus::Unknown, OutcomeState::Invalid(w) | OutcomeState::Unreadable(w)) => {
            return row(
                22,
                "",
                format!("contract violation (supervisor restart): OUTCOME {w}"),
            );
        }
        (w, _) => w,
    };
    let o = match &o_state {
        OutcomeState::Valid(o) => Some(o.as_ref()),
        _ => None,
    };
    let o_time = o.is_some_and(|o| o.time_decided);
    let mut out = decide(e, wait, o, &o_state);
    if matches!(
        out.action,
        LadderDecides
            | NextMemoryClassOrRung
            | ResumeOnCampaignResume
            | PauseAllLaunches
            | RetryOnceThenAttention
            | ResumeOnceThenAttention
            | ResumeOnceIfValidElseAttention
    ) {
        out.resume_from_r = r;
        out.resume_from = if r { e.resume_ref.clone() } else { None };
    }
    if out.action == ResumeOnceIfValidElseAttention && !r {
        out.attention = true;
    }
    out.time_decided = out.time_decided || o_time;
    out
}

fn decide(
    e: &Evidence,
    wait: WaitStatus,
    o: Option<&JobOutcome>,
    o_state: &OutcomeState,
) -> Classification {
    let (k, oom) = (e.kill, e.oom_kill_observed);
    if let WaitStatus::Exited(c) = wait {
        if let Some(o) = o {
            let stop = o.stop.code;
            if matches!(c, 0 | 10 | 20) {
                if e.kind.sealed_outputs().is_none() {
                    return row(26, "", "success of a kind without a sealing rule");
                }
                if e.kind.is_checker() && c != 0 {
                    return row(
                        22,
                        "",
                        format!(
                            "contract violation: exit {c} is not an exit code of a checker kind"
                        ),
                    );
                }
                let Some(v) = &e.verified else {
                    return row(
                        22,
                        "",
                        format!(
                            "contract violation: OUTCOME but the outputs are not sealed: {}",
                            e.unsealed_why.as_deref().unwrap_or("not verified")
                        ),
                    );
                };
                if v.elided_loci > 0 {
                    return row(
                        25,
                        "",
                        format!("{} elided witness loci (elision allowed)", v.elided_loci),
                    );
                }
                // lane adapters: the exit-code sets of the kinds without a rule table
                match e.kind {
                    JobKind::SolveRecipe => return solve_recipe_row(c, stop, v),
                    JobKind::ProofInstantiate if c != 0 => {
                        return row(
                            22,
                            "",
                            format!(
                                "contract violation: exit {c} is not an exit code of kind proof-instantiate"
                            ),
                        );
                    }
                    _ if stop == Some(StopCode::KeysUnsolved) => {
                        return row(
                            22,
                            "",
                            format!("contract violation: stop keys_unsolved of kind {:?}", e.kind),
                        );
                    }
                    _ => {}
                }
                let rules = v.rules;
                match (c, stop) {
                    (0, _) => return row(1, "", "exit 0, OUTCOME, sealed"),
                    (10, Some(StopCode::MaxCases)) => {
                        if rules >= 1 {
                            return row(2, "", "max_cases with verified rules");
                        }
                        if !o.per_case.is_empty()
                            && o.per_case.iter().all(|p| p.result.needs_no_rule())
                        {
                            return row(
                                3,
                                "",
                                "max_cases, no rule; the child claims no case needs one (unverified: confirm by coverage)",
                            );
                        }
                        return row(24, "", "max_cases, no rule, a case result needs a rule");
                    }
                    (
                        10,
                        Some(
                            StopCode::LiftRefused
                            | StopCode::GuardTimeout
                            | StopCode::NumericSearchRefused
                            | StopCode::BeforeNumeric,
                        ),
                    ) => return row(4, "", "refused stop"),
                    (10, Some(StopCode::OpenRegions | StopCode::OpenObligations)) => {
                        return if rules >= 1 {
                            row(5, "", "open regions / obligations")
                        } else {
                            row(27, "", "open regions / obligations, no rule")
                        };
                    }
                    (10, Some(StopCode::NotLower)) => {
                        return row(6, "", "engine invariant: not_lower");
                    }
                    (20, _) => return row(8, "", "work budget (deterministic)"),
                    _ => {
                        return row(
                            22,
                            "",
                            format!("contract violation: exit {c} with stop {stop:?}"),
                        );
                    }
                }
            }
            match c {
                11 | 12 if !e.kind.is_checker() => {
                    return row(
                        22,
                        "",
                        format!(
                            "contract violation: exit {c} is a checker verdict of a producing kind"
                        ),
                    );
                }
                11 => return row(7, "", "checker decided FAIL"),
                12 => return row(7, "b", "checker undecided"),
                21 => {
                    let mut x = row(9, "", "wall budget");
                    x.time_decided = true;
                    return x;
                }
                22 => return row(10, "", "soft memory limit"),
                // The supervisor's own kill reason decides over the child's `interrupted` OUTCOME (rows 11b / 11c):
                // a child stopped for time, silence or memory may still write a valid OUTCOME on its way out.
                30 => match k {
                    KillReason::Time | KillReason::Unresponsive => {
                        let mut x = row(
                            11,
                            "b",
                            format!("interrupted OUTCOME, but the supervisor killed it ({k:?})"),
                        );
                        x.time_decided = true;
                        return x;
                    }
                    KillReason::Memory => {
                        return row(
                            11,
                            "c",
                            "interrupted OUTCOME, but the supervisor killed it for memory",
                        );
                    }
                    KillReason::None | KillReason::Stop => return row(11, "", "interrupted"),
                },
                _ => {}
            }
        }
        match c {
            40 => return row(12, "", "usage error (supervisor bug)"),
            41 => return row(13, "", "input refused"),
            42 => return row(14, "", "contract refusal"),
            50 => return row(15, "", "no space"),
            51 => return row(16, "", "output I/O error"),
            70 if o.is_some() => return row(17, "", "internal error"),
            _ => {}
        }
    }
    if let WaitStatus::Signaled(sig) = wait {
        if sig == SIGKILL && (k == KillReason::Memory || oom) {
            return row(18, "", "SIGKILL for memory");
        }
        if is_stop_signal(sig) && matches!(k, KillReason::Time | KillReason::Unresponsive) {
            let mut x = row(19, "", "killed for time / unresponsive");
            x.time_decided = true;
            return x;
        }
        if is_stop_signal(sig) && k == KillReason::Stop {
            return row(20, "", "killed on a stop request");
        }
        if sig == SIGKILL && k == KillReason::None && matches!(o_state, OutcomeState::Absent) {
            return row(21, "", "SIGKILL from outside, no OUTCOME");
        }
    }
    match o_state {
        OutcomeState::Invalid(why) => row(22, "", format!("contract violation: {why}")),
        OutcomeState::Unreadable(why) => row(
            22,
            "",
            format!("contract violation: OUTCOME unreadable: {why}"),
        ),
        _ => row(23, "", "crashed"),
    }
}

/// Rows 1 / 28 / 29 / 22 of kind solve-recipe (lane adapters): S needs the tally of the results file.
fn solve_recipe_row(c: i32, stop: Option<StopCode>, v: &VerifiedOutputs) -> Classification {
    let Some(t) = &v.relations else {
        return row(
            22,
            "",
            "contract violation: solve-recipe results not tallied against the keys input",
        );
    };
    let unsolved = t.unsolved_total();
    match (c, stop) {
        (0, _) if unsolved == 0 => row(1, "", "exit 0, OUTCOME, sealed, every key solved"),
        (0, _) => row(
            22,
            "",
            format!("contract violation: exit 0 with {unsolved} unsolved records"),
        ),
        (10, Some(StopCode::KeysUnsolved)) if unsolved == 0 => row(
            22,
            "",
            "contract violation: keys_unsolved without an unsolved record",
        ),
        (10, Some(StopCode::KeysUnsolved)) if t.accepted >= 1 => row(
            28,
            "",
            format!(
                "keys_unsolved: {} accepted relations, {unsolved} unsolved keys",
                t.accepted
            ),
        ),
        (10, Some(StopCode::KeysUnsolved)) => row(
            29,
            "",
            format!("keys_unsolved: no accepted relation, {unsolved} unsolved keys"),
        ),
        _ => row(
            22,
            "",
            format!("contract violation: exit {c} with stop {stop:?} for kind solve-recipe"),
        ),
    }
}

/// S of kind solve-recipe (lane adapters): the verified results stream (read through its [`crate::seal::VerifiedHandle`],
/// never a re-opened path), its records against the spec's `keys` input (raw digest = the spec's), and against OUTCOME
/// (`counters.rules_committed` = accepted records; a per-key wall-budget record needs `time_decided`).
pub fn solve_recipe_tally(
    layout: &RunLayout,
    spec: &JobSpec,
    o: &JobOutcome,
    v: &VerifiedOutputs,
) -> Result<(Vec<recipe::KeyOutcome>, recipe::RelationTally), String> {
    let rel = RunLayout::out_rel(crate::layout::RESULTS);
    let s = v
        .streams
        .iter()
        .find(|s| s.rel == rel)
        .ok_or("no verified results stream")?;
    let lines = crate::seal::VerifiedHandle::reopen(&layout.root, s)
        .and_then(|h| h.canonical_lines())
        .map_err(|e| format!("results: {e}"))?;
    let k = spec
        .inputs
        .iter()
        .find(|i| i.role == "keys")
        .ok_or("no keys input")?;
    let kb = std::fs::read(spec.input_path(k)).map_err(|e| format!("keys input: {e}"))?;
    if crate::blake3_hex(&kb) != k.blake3 {
        return Err("keys input digest differs from the spec's".into());
    }
    let keys = recipe::parse_keys_input(&kb)?;
    let (out, t) = recipe::key_outcomes(&lines, &keys)?;
    if t.time_decided_keys() > 0 && !o.time_decided {
        return Err(format!(
            "{} per-key wall-budget records but OUTCOME time_decided is false",
            t.time_decided_keys()
        ));
    }
    if o.counters.rules_committed != t.accepted {
        return Err(format!(
            "OUTCOME counters.rules_committed {} but {} accepted relation records",
            o.counters.rules_committed, t.accepted
        ));
    }
    Ok((out, t))
}

fn read_layout_file(layout: &RunLayout, rel: &str) -> Result<Option<Vec<u8>>, String> {
    use std::io::Read;
    if std::fs::symlink_metadata(layout.root.join(rel)).is_err() {
        return Ok(None);
    }
    let (mut f, _, _) = open_in_run(&layout.root, rel).map_err(|e| e.to_string())?;
    let mut b = Vec::new();
    f.read_to_end(&mut b).map_err(|e| e.to_string())?;
    Ok(Some(b))
}

/// Validity of THIS attempt's `RESUME.json` (3.4.4 item 3, revised by audit A C1 / M7): it parses, passes
/// `check()`, its job_id / attempt / spec_blake3 are this attempt's, and every committed prefix it names is exactly
/// there in this run's files. Returns the record and the reference a next attempt must use.
pub fn resume_state(
    layout: &RunLayout,
    spec: &JobSpec,
    spec_blake3: &str,
) -> Result<(ResumeState, ResumeRef), String> {
    let b = read_layout_file(layout, crate::layout::RESUME)?.ok_or("no RESUME.json")?;
    crate::canon::check_document(&b).map_err(|e| format!("RESUME.json: {e}"))?;
    let r: ResumeState = serde_json::from_slice(&b).map_err(|e| e.to_string())?;
    r.check()?;
    if r.job_id != spec.job_id || r.attempt != spec.attempt || r.spec_blake3 != spec_blake3 {
        return Err("RESUME job_id / attempt / spec_blake3 are not this attempt's".into());
    }
    verify_prefix(&layout.root, &r.table, &TABLE_END_TYPES).map_err(|e| format!("table: {e}"))?;
    if let Some(q) = &r.queue {
        verify_prefix(&layout.root, q, &TABLE_END_TYPES).map_err(|e| format!("queue: {e}"))?;
    }
    let rref = resume_ref_for(layout, &b);
    Ok((r, rref))
}

/// The `spec.resume` of a next attempt: the run's RESUME.json by absolute path and the digest of its bytes `b`.
/// This is the ONLY resume source; OUTCOME's `committed_bytes` / `prefix_blake3` are informational (they may be
/// ahead of RESUME after a hard-safe stop) and never used.
pub fn resume_ref_for(layout: &RunLayout, b: &[u8]) -> ResumeRef {
    let root = std::fs::canonicalize(&layout.root).unwrap_or_else(|_| layout.root.clone());
    ResumeRef {
        from: root.join(crate::layout::RESUME).display().to_string(),
        blake3: crate::blake3_hex(b),
    }
}

/// The OUTCOME state of a run for THIS attempt: OUTCOME parses (read without following symlinks), passes `check()`,
/// matches job_id / attempt / kind / spec_blake3 (M1), and the run's `job.json` is still the spec the supervisor
/// wrote (its digest is `spec_blake3`).
pub fn outcome_state(layout: &RunLayout, spec: &JobSpec, spec_blake3: &str) -> OutcomeState {
    let b = match read_layout_file(layout, crate::layout::OUTCOME) {
        Ok(Some(b)) => b,
        Ok(None) => return OutcomeState::Absent,
        Err(e) => return OutcomeState::Unreadable(e),
    };
    let o = match Outcome::from_slice(&b) {
        Ok(o) => o,
        Err(m) => return OutcomeState::Unreadable(m),
    };
    if let Err(m) = o.job.check() {
        return OutcomeState::Invalid(m);
    }
    if !o.job.matches(spec, spec_blake3) {
        return OutcomeState::Invalid(
            "job_id / attempt / kind / spec_blake3 are not this attempt's".into(),
        );
    }
    match read_layout_file(layout, crate::layout::JOB_SPEC) {
        Ok(Some(j)) if crate::spec::spec_blake3(&j) == spec_blake3 => {}
        _ => {
            return OutcomeState::Invalid(
                "RUN/job.json is not the spec the supervisor wrote".into(),
            );
        }
    }
    OutcomeState::Valid(Box::new(o.job))
}

/// Verify every output of the kind's sealing rule at its layout path against OUTCOME (S). `Err(reason)`: not sealed;
/// a [`UnsealReason::PathNotLayout`] is a contract violation (the caller marks OUTCOME invalid).
pub fn verify_outputs(
    layout: &RunLayout,
    kind: JobKind,
    o: &JobOutcome,
) -> Result<VerifiedOutputs, UnsealReason> {
    let Some(rules) = kind.sealed_outputs() else {
        return Err(UnsealReason::MissingOutput(
            "no sealing rule for this kind".into(),
        ));
    };
    for (k, x) in o.outputs.extra.iter().enumerate() {
        // one entry per role (ac2-m5): the role's entry below is the only one verified, so a second one is refused
        if o.outputs.extra[..k].iter().any(|y| y.role == x.role) {
            return Err(UnsealReason::MissingOutput(format!(
                "outputs.extra role {:?} listed twice",
                x.role
            )));
        }
        if !rules.iter().any(|r| r.role == x.role && r.role != "table") {
            return Err(UnsealReason::MissingOutput(format!(
                "outputs.extra role {:?} is not in the kind's sealing rule",
                x.role
            )));
        }
    }
    let mut v = VerifiedOutputs {
        rules: 0,
        elided_loci: 0,
        streams: Vec::new(),
        relations: None,
    };
    for r in rules {
        let rel = RunLayout::out_rel(r.file);
        let s = if r.role == "table" {
            let Some(t) = &o.outputs.table else {
                return Err(UnsealReason::MissingOutput("table".into()));
            };
            match verify_table(layout, t) {
                SealVerdict::Sealed(s) => {
                    if o.counters.rules_committed != s.rules {
                        return Err(UnsealReason::RuleCountMismatch {
                            expected: o.counters.rules_committed,
                            actual: s.rules,
                        });
                    }
                    v.rules = s.rules;
                    *s
                }
                SealVerdict::Unsealed(u) => return Err(u),
            }
        } else {
            let Some(x) = o.outputs.extra.iter().find(|x| x.role == r.role) else {
                if r.optional {
                    continue;
                }
                return Err(UnsealReason::MissingOutput(r.role.into()));
            };
            if x.path != rel {
                return Err(UnsealReason::PathNotLayout(x.path.clone()));
            }
            if !x.sealed {
                return Err(UnsealReason::OutcomeNotSealed);
            }
            let s = verify_stream(&layout.root, &rel, r.end_types)?;
            compare_figures(
                &s,
                Some(x.bytes),
                Some(&x.blake3),
                x.records,
                x.canonical_blake3.as_deref(),
            )?;
            match x.end {
                Some(e) if e.as_str() == s.end => {}
                _ => {
                    return Err(UnsealReason::EndMismatch {
                        outcome: format!("{:?}", x.end),
                        file: s.end.clone(),
                    });
                }
            }
            s
        };
        v.elided_loci += s.elided_loci;
        v.streams.push(s);
    }
    if let Some(c) = &o.outputs.cert {
        let rel = RunLayout::out_rel(crate::layout::CERT);
        if !c.sealed {
            return Err(UnsealReason::OutcomeNotSealed);
        }
        let s = verify_stream(&layout.root, &rel, &CERT_END_TYPES)?;
        if c.bytes != Some(s.bytes) || c.blake3.as_deref() != Some(s.blake3.as_str()) {
            return Err(UnsealReason::DigestMismatch);
        }
        v.streams.push(s);
    }
    Ok(v)
}

/// Gather the evidence of a finished attempt from its run directory. `spec_blake3` is the supervisor's own digest of
/// the `job.json` it wrote (never re-read from the run directory). Paths come from the layout only.
pub fn assess(
    layout: &RunLayout,
    spec: &JobSpec,
    spec_blake3: &str,
    wait: WaitStatus,
    kill: KillReason,
    oom_kill_observed: bool,
) -> Evidence {
    let mut outcome = outcome_state(layout, spec, spec_blake3);
    let mut verified = None;
    let mut unsealed_why = None;
    if let OutcomeState::Valid(o) = &outcome {
        match verify_outputs(layout, spec.kind, o) {
            Ok(mut v) => {
                // kind solve-recipe (lane adapters): S needs the results' tally against the keys input
                if spec.kind == JobKind::SolveRecipe {
                    match solve_recipe_tally(layout, spec, o, &v) {
                        Ok((_, t)) => v.relations = Some(t),
                        Err(why) => unsealed_why = Some(format!("solve-recipe results: {why}")),
                    }
                }
                // every verified stream's header (at most one per stream, on line 1: the verifier refuses any other,
                // ac2-m6) is bound to the spec's family input
                let want = spec.inputs.iter().find(|i| i.role == "family");
                for f in v.streams.iter().filter_map(|s| s.family_blake3.as_deref()) {
                    if want.is_none_or(|i| i.blake3 != f) {
                        outcome = OutcomeState::Invalid(
                            "table header family_blake3 is not the spec's family input digest"
                                .into(),
                        );
                    }
                }
                if unsealed_why.is_none() {
                    verified = Some(v);
                }
            }
            Err(UnsealReason::PathNotLayout(p)) => {
                outcome = OutcomeState::Invalid(format!(
                    "OUTCOME names {p:?} instead of the layout path"
                ));
            }
            Err(u) => unsealed_why = Some(u.to_string()),
        }
    }
    if !matches!(outcome, OutcomeState::Valid(_)) {
        verified = None;
    }
    let resume = resume_state(layout, spec, spec_blake3).ok();
    Evidence {
        kind: spec.kind,
        wait,
        outcome,
        verified,
        unsealed_why,
        resume_valid: resume.is_some(),
        resume_ref: resume.map(|(_, r)| r),
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
