//! One test per row of the decision table child-job.md 6.5 (first matching row wins), plus the additions by package
//! contract (row 24, exit-code / elision consistency, 6.6 restart). Each test cites its row.

mod common;
use common::*;
use rustred_campaign_contract::ExitCode;
use rustred_campaign_contract::classify::*;
use rustred_campaign_contract::outcome::{JobOutcome, PerCase};
use rustred_campaign_contract::spec::JobKind;
use rustred_campaign_contract::stop::{CaseResult, StopCode};

/// Evidence as `assess` would build it; with `sealed` the verifier's figures are taken from the outcome (rules,
/// elided loci), as a verified table that agrees with OUTCOME would give.
fn ev(wait: WaitStatus, o: Option<JobOutcome>, sealed: bool) -> Evidence {
    let verified = match (&o, sealed) {
        (Some(j), true) => Some(VerifiedOutputs {
            rules: j.counters.rules_committed,
            elided_loci: j
                .outputs
                .table
                .as_ref()
                .map_or(0, |t| t.witness_loci_elided),
            streams: vec![],
        }),
        (None, true) => Some(VerifiedOutputs {
            rules: 0,
            elided_loci: 0,
            streams: vec![],
        }),
        _ => None,
    };
    Evidence {
        kind: JobKind::Generate,
        wait,
        outcome: o.map_or(OutcomeState::Absent, |o| OutcomeState::Valid(Box::new(o))),
        verified,
        unsealed_why: None,
        resume_ref: None,
        resume_valid: true,
        kill: KillReason::None,
        oom_kill_observed: false,
        elision_forbidden: true,
    }
}

fn o(code: ExitCode, stop: Option<StopCode>) -> JobOutcome {
    let s = sample_spec("/r");
    job_outcome(&s, code, stop, true)
}

fn with_rules(mut j: JobOutcome, n: u64) -> JobOutcome {
    j.counters.rules_committed = n;
    j
}

fn with_cases(mut j: JobOutcome, rs: &[CaseResult]) -> JobOutcome {
    j.per_case = rs
        .iter()
        .enumerate()
        .map(|(i, r)| PerCase {
            ordinal: i as u64,
            case: serde_json::Value::Null,
            result: *r,
            rule: None,
            children_open: 0,
        })
        .collect();
    j
}

fn x(e: Evidence) -> Classification {
    classify(&e)
}

#[test]
fn row_01_exit0_outcome_sealed_complete() {
    // | 1 | c=0, O, S | complete | accept canonical records; item done |
    let c = x(ev(
        WaitStatus::Exited(0),
        Some(o(ExitCode::Complete, None)),
        true,
    ));
    assert_eq!(
        (c.row, c.kind, c.accept, c.action),
        (
            1,
            OutcomeKind::Complete,
            Accept::Canonical,
            Action::ItemDone
        )
    );
    assert!(!c.attention && !c.time_decided);
}

#[test]
fn row_02_max_cases_with_rule_rule_found() {
    // | 2 | c=10, O, S, stop max_cases, rules_committed >= 1 | rule_found | accept; RESUME.pending -> new child items |
    let c = x(ev(
        WaitStatus::Exited(10),
        Some(with_rules(
            o(ExitCode::Incomplete, Some(StopCode::MaxCases)),
            1,
        )),
        true,
    ));
    assert_eq!(
        (c.row, c.kind, c.accept, c.action),
        (
            2,
            OutcomeKind::RuleFound,
            Accept::Canonical,
            Action::QueueChildren
        )
    );
}

#[test]
fn row_03_max_cases_no_rule_nothing_needed_complete() {
    // | 3 | c=10, O, S, stop max_cases, no rule, case result covered/empty/finite/declared_terminal | complete | item done |
    for r in [
        CaseResult::Covered,
        CaseResult::Empty,
        CaseResult::Finite,
        CaseResult::DeclaredTerminal,
    ] {
        let j = with_cases(o(ExitCode::Incomplete, Some(StopCode::MaxCases)), &[r]);
        let c = x(ev(WaitStatus::Exited(10), Some(j), true));
        assert_eq!(
            (c.row, c.kind, c.accept, c.action),
            (
                3,
                OutcomeKind::Complete,
                Accept::Nothing,
                Action::ItemDoneNoRule
            )
        );
    }
}

#[test]
fn row_24_max_cases_no_rule_other_result_no_rule() {
    // row 24 (added by package contract, evaluated after row 3): max_cases, no rule, a result outside the set
    for r in [
        CaseResult::Refused,
        CaseResult::Budget,
        CaseResult::Error,
        CaseResult::RegistryCovered,
    ] {
        let j = with_cases(
            o(ExitCode::Incomplete, Some(StopCode::MaxCases)),
            &[CaseResult::Covered, r],
        );
        let c = x(ev(WaitStatus::Exited(10), Some(j), true));
        assert_eq!(
            (c.row, c.kind, c.action),
            (24, OutcomeKind::NoRule, Action::NextRung)
        );
    }
    // no per_case at all: also row 24 (fail closed: nothing proves that no rule is needed)
    let c = x(ev(
        WaitStatus::Exited(10),
        Some(o(ExitCode::Incomplete, Some(StopCode::MaxCases))),
        true,
    ));
    assert_eq!(c.row, 24);
}

#[test]
fn row_04_refused_stops_no_rule() {
    // | 4 | c=10, O, S, stop lift_refused / guard_timeout / numeric_search_refused / before_numeric | no_rule (refused) |
    for s in [
        StopCode::LiftRefused,
        StopCode::GuardTimeout,
        StopCode::NumericSearchRefused,
        StopCode::BeforeNumeric,
    ] {
        let c = x(ev(
            WaitStatus::Exited(10),
            Some(o(ExitCode::Incomplete, Some(s))),
            true,
        ));
        assert_eq!(
            (c.row, c.kind, c.accept, c.action),
            (
                4,
                OutcomeKind::NoRule,
                Accept::CommittedRules,
                Action::NextRung
            )
        );
    }
}

#[test]
fn row_05_open_regions_obligations_rule_found() {
    // | 5 | c=10, O, S, stop open_regions / open_obligations | rule_found | accept; open list -> items |
    for s in [StopCode::OpenRegions, StopCode::OpenObligations] {
        let c = x(ev(
            WaitStatus::Exited(10),
            Some(with_rules(o(ExitCode::Incomplete, Some(s)), 1)),
            true,
        ));
        assert_eq!(
            (c.row, c.kind, c.action),
            (5, OutcomeKind::RuleFound, Action::QueueOpenList)
        );
        // audit A m1: no verified rule -> row 27 (no_rule, accept nothing, open list -> items)
        let c = x(ev(
            WaitStatus::Exited(10),
            Some(o(ExitCode::Incomplete, Some(s))),
            true,
        ));
        assert_eq!(
            (c.row, c.kind, c.accept, c.action),
            (
                27,
                OutcomeKind::NoRule,
                Accept::Nothing,
                Action::QueueOpenList
            )
        );
    }
}

#[test]
fn row_06_not_lower_crashed() {
    // | 6 | c=10, O, S, stop not_lower | crashed (engine invariant) | accept nothing; attention |
    let c = x(ev(
        WaitStatus::Exited(10),
        Some(o(ExitCode::Incomplete, Some(StopCode::NotLower))),
        true,
    ));
    assert_eq!(
        (c.row, c.kind, c.accept),
        (6, OutcomeKind::Crashed, Accept::Nothing)
    );
    assert!(c.attention);
}

#[test]
fn row_07_checker_verdicts() {
    // | 7 | c=11 / c=12, O | decided_negative / undecided | record the verdict; never credit on 11 |
    let mut e = ev(
        WaitStatus::Exited(11),
        Some(o(ExitCode::DecidedNegative, None)),
        false,
    );
    e.kind = JobKind::ProofSpan;
    if let OutcomeState::Valid(j) = &mut e.outcome {
        j.kind = JobKind::ProofSpan;
        j.outputs.table = None;
    }
    let c = x(e.clone());
    assert_eq!(
        (c.row, c.kind, c.accept, c.action),
        (
            7,
            OutcomeKind::DecidedNegative,
            Accept::Nothing,
            Action::RecordNegativeVerdict
        )
    );
    e.wait = WaitStatus::Exited(12);
    if let OutcomeState::Valid(j) = &mut e.outcome {
        j.exit_code = ExitCode::Undecided;
        j.status = rustred_campaign_contract::JobStatus::Undecided;
    }
    let c = x(e.clone());
    assert_eq!(
        (c.row, c.sub.as_str(), c.kind, c.action),
        (7, "b", OutcomeKind::Undecided, Action::OtherToolOrAttention)
    );
    // audit A m1: a verdict code from a producing kind is a contract violation
    e.kind = JobKind::Generate;
    if let OutcomeState::Valid(j) = &mut e.outcome {
        j.kind = JobKind::Generate;
    }
    assert_eq!(x(e).row, 22);
}

#[test]
fn row_08_budget_work_spill() {
    // | 8 | c=20, O, S | spill (budget_work, deterministic) | accept committed rules; next rung |
    let c = x(ev(
        WaitStatus::Exited(20),
        Some(o(ExitCode::BudgetWork, Some(StopCode::BudgetWork))),
        true,
    ));
    assert_eq!(
        (c.row, c.kind, c.accept, c.action),
        (
            8,
            OutcomeKind::Spill,
            Accept::CommittedRules,
            Action::NextRungAfterSpill
        )
    );
    assert!(!c.time_decided);
}

#[test]
fn row_09_budget_wall_killed_time() {
    // | 9 | c=21, O | killed_time (time_decided) | accept nothing; ladder decides; resume allowed from R |
    let c = x(ev(
        WaitStatus::Exited(21),
        Some(o(ExitCode::BudgetWall, Some(StopCode::BudgetWall))),
        false,
    ));
    assert_eq!(
        (c.row, c.kind, c.accept, c.action),
        (
            9,
            OutcomeKind::KilledTime,
            Accept::Nothing,
            Action::LadderDecides
        )
    );
    assert!(c.time_decided && c.resume_from_r);
}

#[test]
fn row_10_budget_memory_killed_memory() {
    // | 10 | c=22, O | killed_memory (soft) | next memory class once if the policy allows, else next rung |
    let c = x(ev(
        WaitStatus::Exited(22),
        Some(o(ExitCode::BudgetMemory, Some(StopCode::BudgetMemory))),
        true,
    ));
    assert_eq!(
        (c.row, c.kind, c.action),
        (10, OutcomeKind::KilledMemory, Action::NextMemoryClassOrRung)
    );
}

#[test]
fn row_11_interrupted() {
    // | 11 | c=30, O | interrupted | nothing accepted; on campaign resume: new attempt with resume = R |
    for sealed in [true, false] {
        let c = x(ev(
            WaitStatus::Exited(30),
            Some(o(ExitCode::Interrupted, Some(StopCode::Interrupted))),
            sealed,
        ));
        assert_eq!(
            (c.row, c.kind, c.accept, c.action),
            (
                11,
                OutcomeKind::Interrupted,
                Accept::Nothing,
                Action::ResumeOnCampaignResume
            )
        );
        assert!(c.resume_from_r);
    }
    let mut e = ev(
        WaitStatus::Exited(30),
        Some(o(ExitCode::Interrupted, Some(StopCode::Interrupted))),
        true,
    );
    e.resume_valid = false;
    assert!(!x(e).resume_from_r);
}

#[test]
fn row_11b_11c_supervisor_kill_decides_over_interrupted_outcome() {
    // | 11b | c=30, O, K=time or unresponsive | killed_time | | 11c | c=30, O, K=memory | killed_memory |
    // (supervisor lane's request: a child stopped for time / silence / memory may still write `interrupted`)
    for (k, row_sub, kind, action, timed) in [
        (KillReason::Time, "b", OutcomeKind::KilledTime, Action::LadderDecides, true),
        (KillReason::Unresponsive, "b", OutcomeKind::KilledTime, Action::LadderDecides, true),
        (KillReason::Memory, "c", OutcomeKind::KilledMemory, Action::NextMemoryClassOrRung, false),
        (KillReason::Stop, "", OutcomeKind::Interrupted, Action::ResumeOnCampaignResume, false),
        (KillReason::None, "", OutcomeKind::Interrupted, Action::ResumeOnCampaignResume, false),
    ] {
        let mut e = ev(
            WaitStatus::Exited(30),
            Some(o(ExitCode::Interrupted, Some(StopCode::Interrupted))),
            true,
        );
        e.kill = k;
        let c = x(e);
        assert_eq!(
            (c.row, c.sub.as_str(), c.kind, c.accept, c.action, c.time_decided),
            (11, row_sub, kind, Accept::Nothing, action, timed),
            "{k:?}"
        );
    }
}

#[test]
fn row_12_usage_error() {
    // | 12 | c=40 | usage_error | supervisor bug: stop launching this kind, attention |
    let c = x(ev(WaitStatus::Exited(40), None, false));
    assert_eq!(
        (c.row, c.kind, c.action),
        (12, OutcomeKind::UsageError, Action::StopLaunchingKind)
    );
    assert!(c.attention);
}

#[test]
fn row_13_input_refused() {
    // | 13 | c=41 | refused (input) | item attention; no retry |
    let c = x(ev(
        WaitStatus::Exited(41),
        Some(o(ExitCode::Input, Some(StopCode::InputError))),
        false,
    ));
    assert_eq!(
        (c.row, c.kind, c.action),
        (13, OutcomeKind::Refused, Action::ItemAttentionNoRetry)
    );
}

#[test]
fn row_14_contract_refused() {
    // | 14 | c=42 | refused (contract) | attention |
    let c = x(ev(WaitStatus::Exited(42), None, false));
    assert_eq!(
        (c.row, c.kind, c.action),
        (14, OutcomeKind::Refused, Action::Attention)
    );
}

#[test]
fn row_15_nospace_lost() {
    // | 15 | c=50 | lost (nospace) | pause ALL launches until free space; resume from R |
    let c = x(ev(WaitStatus::Exited(50), None, false));
    assert_eq!(
        (c.row, c.kind, c.action),
        (15, OutcomeKind::Lost, Action::PauseAllLaunches)
    );
    assert!(c.resume_from_r);
}

#[test]
fn row_16_io_crashed() {
    // | 16 | c=51 | crashed (io) | retry once (resume from R), then attention |
    let c = x(ev(WaitStatus::Exited(51), None, false));
    assert_eq!(
        (c.row, c.kind, c.action),
        (16, OutcomeKind::Crashed, Action::RetryOnceThenAttention)
    );
}

#[test]
fn row_17_internal_crashed() {
    // | 17 | c=70, O | crashed (internal) | retry once with the same spec, then attention |
    let c = x(ev(
        WaitStatus::Exited(70),
        Some(o(ExitCode::Internal, Some(StopCode::Panic))),
        false,
    ));
    assert_eq!(
        (c.row, c.kind, c.action),
        (
            17,
            OutcomeKind::Crashed,
            Action::RetrySameSpecOnceThenAttention
        )
    );
    // without OUTCOME: row 23
    assert_eq!(x(ev(WaitStatus::Exited(70), None, false)).row, 23);
}

#[test]
fn row_18_sigkill_memory() {
    // | 18 | s=9, K=memory (or cgroup oom_kill count increased) | killed_memory | as row 10 |
    let mut e = ev(WaitStatus::Signaled(SIGKILL), None, false);
    e.kill = KillReason::Memory;
    let c = x(e.clone());
    assert_eq!(
        (c.row, c.kind, c.action),
        (18, OutcomeKind::KilledMemory, Action::NextMemoryClassOrRung)
    );
    e.kill = KillReason::None;
    e.oom_kill_observed = true;
    assert_eq!(x(e).row, 18);
}

#[test]
fn row_19_killed_time() {
    // | 19 | s=9, K=time or unresponsive | killed_time | as row 9 |  (+ SIGTERM/SIGINT/SIGHUP, package contract)
    for (s, k) in [
        (SIGKILL, KillReason::Time),
        (SIGKILL, KillReason::Unresponsive),
        (SIGTERM, KillReason::Time),
    ] {
        let mut e = ev(WaitStatus::Signaled(s), None, false);
        e.kill = k;
        let c = x(e);
        assert_eq!(
            (c.row, c.kind, c.action),
            (19, OutcomeKind::KilledTime, Action::LadderDecides)
        );
        assert!(c.time_decided);
    }
}

#[test]
fn row_20_killed_on_stop() {
    // | 20 | s=9, K=stop | interrupted | as row 11 |  (+ SIGTERM: an L1 child dies on the supervisor's SIGTERM)
    for s in [SIGKILL, SIGTERM, SIGINT, SIGHUP] {
        let mut e = ev(WaitStatus::Signaled(s), None, false);
        e.kill = KillReason::Stop;
        let c = x(e);
        assert_eq!(
            (c.row, c.kind, c.action),
            (20, OutcomeKind::Interrupted, Action::ResumeOnCampaignResume)
        );
    }
}

#[test]
fn row_21_outside_sigkill_lost() {
    // | 21 | s=9, K=none, no OUTCOME | lost | resume from R once, then attention |
    let c = x(ev(WaitStatus::Signaled(SIGKILL), None, false));
    assert_eq!(
        (c.row, c.kind, c.action),
        (21, OutcomeKind::Lost, Action::ResumeOnceThenAttention)
    );
    assert!(c.resume_from_r);
}

#[test]
fn row_22_contract_violations() {
    // | 22 | O present but job_id/attempt mismatch, or c in {0,10,20} with O but table not sealed | crashed | attention |
    let mut e = ev(WaitStatus::Exited(0), None, true);
    e.outcome = OutcomeState::Invalid("job_id mismatch".into());
    let c = x(e);
    assert_eq!(
        (c.row, c.kind, c.accept),
        (22, OutcomeKind::Crashed, Accept::Nothing)
    );
    assert!(c.attention);
    for (code, stop) in [
        (ExitCode::Complete, None),
        (ExitCode::Incomplete, Some(StopCode::MaxCases)),
        (ExitCode::BudgetWork, Some(StopCode::BudgetWork)),
    ] {
        let c = x(ev(
            WaitStatus::Exited(code.code()),
            Some(o(code, stop)),
            false,
        ));
        assert_eq!(c.row, 22, "{code}");
    }
    // unreadable OUTCOME
    let mut e = ev(WaitStatus::Exited(0), None, true);
    e.outcome = OutcomeState::Unreadable("EOF".into());
    assert_eq!(x(e).row, 22);
}

#[test]
fn row_22_exit_code_differs_from_outcome() {
    // package contract: OUTCOME.exit_code must equal the process exit code
    let c = x(ev(
        WaitStatus::Exited(0),
        Some(o(ExitCode::Incomplete, Some(StopCode::MaxCases))),
        true,
    ));
    assert_eq!(c.row, 22);
}

#[test]
fn row_22_elided_loci_when_forbidden() {
    // ruling R-f: elided witness loci are unusable; with elision forbidden they are a contract violation
    let mut j = o(ExitCode::Complete, None);
    j.outputs.table.as_mut().unwrap().witness_loci_elided = 2;
    let c = x(ev(WaitStatus::Exited(0), Some(j.clone()), true));
    assert_eq!(c.row, 22);
    // audit A M2: with elision allowed the table may seal, but the class is sealed_with_elided (row 25), never used
    let mut e = ev(WaitStatus::Exited(0), Some(j), true);
    e.elision_forbidden = false;
    let c = x(e);
    assert_eq!(
        (c.row, c.kind, c.accept, c.action),
        (
            25,
            OutcomeKind::SealedWithElided,
            Accept::Nothing,
            Action::RerunWithoutElision
        )
    );
}

#[test]
fn row_23_other_crashes() {
    // | 23 | any other c or s, no OUTCOME | crashed | resume from R once if valid, else attention |
    for w in [
        WaitStatus::Exited(101),
        WaitStatus::Exited(0),
        WaitStatus::Exited(1),
        WaitStatus::Signaled(6),
        WaitStatus::Signaled(11),
    ] {
        let c = x(ev(w, None, false));
        assert_eq!(
            (c.row, c.kind, c.action),
            (
                23,
                OutcomeKind::Crashed,
                Action::ResumeOnceIfValidElseAttention
            ),
            "{w:?}"
        );
        assert!(c.resume_from_r && !c.attention);
    }
    let mut e = ev(WaitStatus::Signaled(6), None, false);
    e.resume_valid = false;
    let c = x(e);
    assert!(!c.resume_from_r && c.attention);
    // s=9, K=none, but a valid OUTCOME (child finished its OUTCOME, then was killed from outside): row 23
    let c = x(ev(
        WaitStatus::Signaled(SIGKILL),
        Some(o(ExitCode::Complete, None)),
        true,
    ));
    assert_eq!(c.row, 23);
}

#[test]
fn restart_6_6_unknown_wait() {
    // 6.6: W unknown: O valid => rows 1-17 with c = O.job.exit_code; else R valid => row 21; else from scratch
    let c = x(ev(
        WaitStatus::Unknown,
        Some(o(ExitCode::Complete, None)),
        true,
    ));
    assert_eq!(c.row, 1);
    let c = x(ev(
        WaitStatus::Unknown,
        Some(o(ExitCode::BudgetWall, Some(StopCode::BudgetWall))),
        false,
    ));
    assert_eq!(c.row, 9);
    let c = x(ev(WaitStatus::Unknown, None, false));
    assert_eq!(
        (c.row, c.sub.as_str(), c.action, c.resume_from_r),
        (21, "b", Action::ResumeOnceThenAttention, true)
    );
    let mut e = ev(WaitStatus::Unknown, None, false);
    e.resume_valid = false;
    let c = x(e);
    assert_eq!(
        (c.row, c.sub.as_str(), c.action, c.resume_from_r),
        (21, "c", Action::RestartFromScratch, false)
    );
    // audit A m4: W unknown with an invalid / unreadable OUTCOME is a contract violation, not "lost"
    let mut e = ev(WaitStatus::Unknown, None, false);
    e.outcome = OutcomeState::Invalid("x".into());
    assert_eq!(x(e).row, 22);
}

#[test]
fn time_decided_flag_is_carried() {
    // "Any row with time_decided: true keeps its kind and carries the flag into the job record."
    let mut j = o(ExitCode::Complete, None);
    j.time_decided = true;
    let c = x(ev(WaitStatus::Exited(0), Some(j), true));
    assert_eq!(
        (c.row, c.kind, c.time_decided),
        (1, OutcomeKind::Complete, true)
    );
}

#[test]
fn checker_kinds_need_their_sealed_results() {
    // audit A M4: S is never vacuously true; a checker's results file must be sealed and verified
    let s = sample_spec("/r");
    let mut j = job_outcome(&s, ExitCode::Complete, None, false);
    j.kind = JobKind::Descent;
    j.outputs.table = None;
    let mut e = ev(WaitStatus::Exited(0), Some(j.clone()), false);
    e.kind = JobKind::Descent;
    assert_eq!(x(e).row, 22);
    let mut e = ev(WaitStatus::Exited(0), Some(j), true);
    e.kind = JobKind::Descent;
    assert_eq!(x(e).row, 1);
}
