//! Independent re-audit (lane audit-contract2, 2026-11-14): the decision table child-job.md 6.5 / 6.6, enumerated.
//!
//! Every combination of (job kind) x (wait status: every table exit code, other codes, 128+N, signals, unknown) x
//! (OUTCOME: absent, torn/unreadable, invalid, and every internally consistent valid OUTCOME (exit code x stop x
//! rules x per-case list x time decisions)) x (seal state: not verified, verified with 0 / 1 rules, with elided
//! loci) x (RESUME valid / invalid) x (kill reason K) x (cgroup oom) x (elision forbidden / allowed) is classified.
//! For each: no panic; the row exists in `ROWS` and the classification carries that row's kind / accept / action;
//! the row equals the row of an ORACLE written from the document text alone (not from classify.rs); and the safety
//! predicate "accept != nothing ONLY with a valid OUTCOME of this exit code, c in {0, 10, 20}, a kind with a
//! sealing rule, verified outputs without elided loci, a stop the row allows" holds.
//! The full product runs in release (`--release -- --ignored`); the default test samples every 97th combination.

mod common;
use common::*;
use rustred_campaign_contract::ExitCode;
use rustred_campaign_contract::classify::{
    self, Accept, Evidence, KillReason, OutcomeState, ROWS, VerifiedOutputs, WaitStatus,
};
use rustred_campaign_contract::outcome::{JobOutcome, PerCase, StopInfo, TimeDecision, TimeDecisionWhat};
use rustred_campaign_contract::resume::ResumeRef;
use rustred_campaign_contract::spec::JobKind;
use rustred_campaign_contract::stop::{CaseResult, StopCode};
use std::collections::BTreeMap;

// ------------------------------------------------------------------------------------------------ the oracle

/// The row the DOCUMENT assigns (child-job.md 6.5, "Rows are evaluated in this order (first match wins): O
/// normalisation (row 22 reasons), W unknown (21b, 21c, 22), then for c in {0, 10, 20} with O: 26, the kind's
/// exit-code set (22), S (22), 25, then 1-8 (24 after 3, 27 after 5), then 9-17 (11b / 11c before 11), then the
/// signal rows 18-21, then 22, then 23", plus the "Precisions" list and 6.6).
fn oracle(e: &Evidence) -> (u8, &'static str) {
    // 128+N is death by signal N
    let w = match e.wait {
        WaitStatus::Exited(c) if (129..=192).contains(&c) => WaitStatus::Signaled(c - 128),
        w => w,
    };
    #[derive(PartialEq)]
    enum O<'a> {
        Absent,
        Bad,
        Ok(&'a JobOutcome),
    }
    let mut o = match &e.outcome {
        OutcomeState::Absent => O::Absent,
        OutcomeState::Unreadable(_) | OutcomeState::Invalid(_) => O::Bad,
        OutcomeState::Valid(j) => O::Ok(j),
    };
    // O normalisation: exit_code != c; elided loci under "forbidden"
    if let (O::Ok(j), WaitStatus::Exited(c)) = (&o, w)
        && j.exit_code.code() != c
    {
        o = O::Bad;
    }
    let elided = e.verified.as_ref().map_or(0, |v| v.elided_loci);
    if matches!(o, O::Ok(_)) && e.elision_forbidden && elided > 0 {
        o = O::Bad;
    }
    // W unknown (6.6)
    let w = match (w, &o) {
        (WaitStatus::Unknown, O::Ok(j)) => WaitStatus::Exited(j.exit_code.code()),
        (WaitStatus::Unknown, O::Absent) => return if e.resume_valid { (21, "b") } else { (21, "c") },
        (WaitStatus::Unknown, O::Bad) => return (22, ""),
        (w, _) => w,
    };
    if let WaitStatus::Exited(c) = w {
        if let O::Ok(j) = &o {
            let stop = j.stop.code;
            if [0, 10, 20].contains(&c) {
                if e.kind == JobKind::GenerateAuto {
                    return (26, "");
                }
                let checker = matches!(
                    e.kind,
                    JobKind::Descent
                        | JobKind::Replay
                        | JobKind::Laporta
                        | JobKind::ProofSpan
                        | JobKind::ProofExact
                        | JobKind::ProofStream
                );
                if checker && c != 0 {
                    return (22, "");
                }
                let Some(v) = &e.verified else { return (22, "") };
                if v.elided_loci > 0 {
                    return (25, "");
                }
                if c == 0 {
                    return (1, "");
                }
                if c == 20 {
                    return (8, "");
                }
                return match stop {
                    Some(StopCode::MaxCases) => {
                        if v.rules >= 1 {
                            (2, "")
                        } else if !j.per_case.is_empty()
                            && j.per_case.iter().all(|p| {
                                matches!(
                                    p.result,
                                    CaseResult::Covered
                                        | CaseResult::Empty
                                        | CaseResult::Finite
                                        | CaseResult::DeclaredTerminal
                                )
                            })
                        {
                            (3, "")
                        } else {
                            (24, "")
                        }
                    }
                    Some(
                        StopCode::LiftRefused
                        | StopCode::GuardTimeout
                        | StopCode::NumericSearchRefused
                        | StopCode::BeforeNumeric,
                    ) => (4, ""),
                    Some(StopCode::OpenRegions | StopCode::OpenObligations) => {
                        if v.rules >= 1 { (5, "") } else { (27, "") }
                    }
                    Some(StopCode::NotLower) => (6, ""),
                    _ => (22, ""),
                };
            }
            let checker = matches!(
                e.kind,
                JobKind::Descent
                    | JobKind::Replay
                    | JobKind::Laporta
                    | JobKind::ProofSpan
                    | JobKind::ProofExact
                    | JobKind::ProofStream
            );
            match c {
                11 | 12 if !checker => return (22, ""),
                11 => return (7, ""),
                12 => return (7, "b"),
                21 => return (9, ""),
                22 => return (10, ""),
                30 => {
                    return match e.kill {
                        KillReason::Time | KillReason::Unresponsive => (11, "b"),
                        KillReason::Memory => (11, "c"),
                        _ => (11, ""),
                    };
                }
                _ => {}
            }
        }
        match c {
            40 => return (12, ""),
            41 => return (13, ""),
            42 => return (14, ""),
            50 => return (15, ""),
            51 => return (16, ""),
            70 if matches!(o, O::Ok(_)) => return (17, ""),
            _ => {}
        }
    }
    if let WaitStatus::Signaled(s) = w {
        let stop_sig = [9, 15, 2, 1].contains(&s);
        if s == 9 && (e.kill == KillReason::Memory || e.oom_kill_observed) {
            return (18, "");
        }
        if stop_sig && matches!(e.kill, KillReason::Time | KillReason::Unresponsive) {
            return (19, "");
        }
        if stop_sig && e.kill == KillReason::Stop {
            return (20, "");
        }
        if s == 9 && e.kill == KillReason::None && o == O::Absent {
            return (21, "");
        }
    }
    if o == O::Bad {
        return (22, "");
    }
    (23, "")
}

/// The safety predicate, independent of any row: what MUST hold whenever something is accepted.
fn may_accept(e: &Evidence) -> Result<(), String> {
    let OutcomeState::Valid(j) = &e.outcome else {
        return Err("accepted without a valid OUTCOME".into());
    };
    let c = match e.wait {
        WaitStatus::Exited(c) => {
            if c != j.exit_code.code() {
                return Err(format!("accepted with process exit {c} != OUTCOME {}", j.exit_code.code()));
            }
            c
        }
        WaitStatus::Unknown => j.exit_code.code(),
        WaitStatus::Signaled(s) => return Err(format!("accepted after death by signal {s}")),
    };
    if ![0, 10, 20].contains(&c) {
        return Err(format!("accepted with exit {c}"));
    }
    if j.kind.sealed_outputs().is_none() || e.kind.sealed_outputs().is_none() {
        return Err("accepted for a kind without a sealing rule".into());
    }
    let Some(v) = &e.verified else {
        return Err("accepted without verified outputs".into());
    };
    if v.elided_loci > 0 {
        return Err("accepted with elided witness loci".into());
    }
    if e.kind.is_checker() && c != 0 {
        return Err("checker accepted on 10 / 20".into());
    }
    if c == 10 {
        match j.stop.code {
            Some(StopCode::MaxCases | StopCode::OpenRegions | StopCode::OpenObligations) if v.rules >= 1 => {}
            Some(
                StopCode::LiftRefused
                | StopCode::GuardTimeout
                | StopCode::NumericSearchRefused
                | StopCode::BeforeNumeric,
            ) => {}
            s => return Err(format!("accepted on exit 10 with stop {s:?} and {} verified rules", v.rules)),
        }
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------ the space

fn waits() -> Vec<WaitStatus> {
    let mut w: Vec<WaitStatus> = [
        0, 1, 2, 9, 10, 11, 12, 13, 20, 21, 22, 23, 30, 31, 40, 41, 42, 43, 50, 51, 52, 69, 70, 71, 101, 127, 128, 129,
        130, 134, 137, 143, 158, 192, 193, 255, 256, -1, i32::MIN, i32::MAX,
    ]
    .iter()
    .map(|&c| WaitStatus::Exited(c))
    .collect();
    w.extend([0, 1, 2, 3, 6, 9, 11, 13, 15, 31, 64, -9].iter().map(|&s| WaitStatus::Signaled(s)));
    w.push(WaitStatus::Unknown);
    w
}

/// Every OUTCOME the contract can call valid for `kind`: internally consistent (`check()` passes) and matching the
/// spec, over exit code x stop x rules x per-case list x time decisions x graceful.
fn valid_outcomes(kind: JobKind) -> Vec<JobOutcome> {
    let mut spec = sample_spec("/r");
    spec.kind = kind;
    let mut out = Vec::new();
    let stops: Vec<Option<StopCode>> = std::iter::once(None).chain(StopCode::ALL.iter().copied().map(Some)).collect();
    for &code in ExitCode::ALL.iter() {
        for &stop in &stops {
            for rules in [0u64, 1] {
                for pc in 0..3 {
                    for td in [false, true] {
                        for graceful in [true, false] {
                            let mut j = job_outcome(&spec, code, stop, true);
                            j.kind = kind;
                            j.counters.rules_committed = rules;
                            if let Some(t) = j.outputs.table.as_mut() {
                                t.rules = Some(rules);
                                t.rules_committed = rules;
                            }
                            j.stop = StopInfo {
                                code: stop,
                                detail: serde_json::Value::Null,
                                source: None,
                                graceful,
                            };
                            j.per_case = match pc {
                                0 => vec![],
                                1 => vec![per_case(CaseResult::Covered), per_case(CaseResult::Finite)],
                                _ => vec![per_case(CaseResult::Covered), per_case(CaseResult::Refused)],
                            };
                            j.time_decisions = if td {
                                vec![TimeDecision {
                                    what: TimeDecisionWhat::GuardRoute,
                                    case_ordinal: Some(0),
                                    rule: None,
                                    limit_s: 60.0,
                                    measured_s: 61.0,
                                    taken: "content".into(),
                                }]
                            } else {
                                vec![]
                            };
                            j.time_decided = td || stop == Some(StopCode::BudgetWall);
                            if j.check().is_ok() {
                                out.push(j);
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

fn per_case(r: CaseResult) -> PerCase {
    PerCase {
        ordinal: 0,
        case: serde_json::Value::Null,
        result: r,
        rule: None,
        children_open: 0,
    }
}

fn run(stride: u64) -> BTreeMap<String, u64> {
    let kills = [
        KillReason::None,
        KillReason::Time,
        KillReason::Memory,
        KillReason::Stop,
        KillReason::Unresponsive,
    ];
    let verified: Vec<Option<VerifiedOutputs>> = vec![
        None,
        Some(VerifiedOutputs { rules: 0, elided_loci: 0, streams: vec![] }),
        Some(VerifiedOutputs { rules: 1, elided_loci: 0, streams: vec![] }),
        Some(VerifiedOutputs { rules: 1, elided_loci: 2, streams: vec![] }),
    ];
    let rref = ResumeRef {
        from: "/w/jobs/J.a1/RESUME.json".into(),
        blake3: D0.into(),
    };
    let mut stats: BTreeMap<String, u64> = BTreeMap::new();
    let mut n: u64 = 0;
    let mut checked: u64 = 0;
    let mut mismatches: Vec<String> = Vec::new();
    let mut unsafe_accepts: Vec<String> = Vec::new();
    let mut panics: u64 = 0;
    for &kind in JobKind::ALL.iter() {
        let mut outcomes: Vec<OutcomeState> = vec![
            OutcomeState::Absent,
            OutcomeState::Unreadable("torn".into()),
            OutcomeState::Invalid("other attempt".into()),
        ];
        outcomes.extend(valid_outcomes(kind).into_iter().map(|j| OutcomeState::Valid(Box::new(j))));
        for wait in waits() {
            for o in &outcomes {
                for v in &verified {
                    for r in [false, true] {
                        for k in kills {
                            for oom in [false, true] {
                                for elision_forbidden in [true, false] {
                                    n += 1;
                                    if n % stride != 0 {
                                        continue;
                                    }
                                    checked += 1;
                                    let e = Evidence {
                                        kind,
                                        wait,
                                        outcome: o.clone(),
                                        verified: v.clone(),
                                        unsealed_why: None,
                                        resume_valid: r,
                                        resume_ref: r.then(|| rref.clone()),
                                        kill: k,
                                        oom_kill_observed: oom,
                                        elision_forbidden,
                                    };
                                    let got = std::panic::catch_unwind(|| classify::classify(&e));
                                    let Ok(c) = got else {
                                        panics += 1;
                                        continue;
                                    };
                                    let spec_row = ROWS.iter().find(|x| x.row == c.row && x.sub == c.sub);
                                    let Some(spec_row) = spec_row else {
                                        mismatches.push(format!("row {}{} not in ROWS", c.row, c.sub));
                                        continue;
                                    };
                                    assert_eq!(
                                        (spec_row.kind, spec_row.accept, spec_row.action),
                                        (c.kind, c.accept, c.action),
                                        "classification differs from its ROWS entry"
                                    );
                                    let want = oracle(&e);
                                    if (c.row, c.sub.as_str()) != want && mismatches.len() < 20 {
                                        mismatches.push(format!(
                                            "kind {kind:?} wait {wait:?} O {} v {:?} R {r} K {k:?} oom {oom} ef {elision_forbidden}: classify {}{} oracle {}{}",
                                            match &e.outcome {
                                                OutcomeState::Valid(j) => format!("valid(exit {}, stop {:?}, rules {}, pc {})", j.exit_code.code(), j.stop.code, j.counters.rules_committed, j.per_case.len()),
                                                x => format!("{x:?}"),
                                            },
                                            v.as_ref().map(|v| (v.rules, v.elided_loci)),
                                            c.row, c.sub, want.0, want.1
                                        ));
                                    }
                                    if c.accept != Accept::Nothing
                                        && let Err(why) = may_accept(&e)
                                        && unsafe_accepts.len() < 20
                                    {
                                        unsafe_accepts.push(format!("row {}{}: {why}", c.row, c.sub));
                                    }
                                    if c.resume_from.is_some() && !r {
                                        unsafe_accepts.push("resume_from without a valid RESUME".into());
                                    }
                                    *stats.entry(format!("{:>2}{:<1}", c.row, c.sub)).or_default() += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!("decision table: {n} combinations in the space, {checked} classified (stride {stride}), {panics} panics");
    assert_eq!(panics, 0, "classify panicked");
    assert!(mismatches.is_empty(), "rows differ from the document's oracle:\n{}", mismatches.join("\n"));
    assert!(unsafe_accepts.is_empty(), "unsafe acceptance:\n{}", unsafe_accepts.join("\n"));
    stats.insert("_classified".into(), checked);
    stats
}

#[test]
fn ac2_decision_table_sampled() {
    let stats = run(97);
    eprintln!("{stats:?}");
}

#[test]
#[ignore = "full product (~20 M combinations): run with --release -- --ignored"]
fn ac2_decision_table_full_product() {
    let stats = run(1);
    for (k, v) in &stats {
        eprintln!("  row {k}: {v}");
    }
    // every row of the document is reachable
    for r in ROWS.iter() {
        let key = format!("{:>2}{:<1}", r.row, r.sub);
        assert!(stats.contains_key(&key), "row {key} never produced");
    }
}
