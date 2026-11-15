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
//! Extended by lane adapters (2026-11-14): kinds solve-recipe / proof-instantiate, stop keys_unsolved, rows 28 / 29
//! (oracle written from interfaces/adapters.md 1.2 and the regenerated child-job.md 6.5), the solve-recipe verified
//! states (results tally: accepted 0 / 1 x unsolved 0 / 1, or no tally) and the `solved_records` accept mode.
//! Extended by lane contract-line7 (2026-11-14; child-job.md "Line 7"): kind node-basis (row 30, exit 0 only), the
//! solve-recipe PROBE tallies (answered 0 / 1 x unanswered 0 / 1, and a lying tally with an accepted relation; rows
//! 31 / 32) and the `diagnostic_records` accept mode, which the safety predicate allows only for those two; OUTCOMEs
//! naming a typed reader error (UNREADABLE_RECORD / UNKNOWN_RECORD_KIND) at exit 41 / 70 (row 13b for proof-instantiate
//! and proof-instance-span).
//! Extended by lane symcert (2026-11-15; interfaces/proof-symrows-job.md 4): kind proof-symrows (row 40 exit 0 with a
//! verified certificate; row 43 for exit 10 / 20 or an unverified certificate, before 22; row 41 at exit 41 naming
//! SYMROW_REFUSED and row 42 at exit 41 / 70 naming a reader error, before 13b / 13) and the `certificates` accept mode
//! (proof-symrows at exit 0 only).

mod common;
use common::*;
use rustred_campaign_contract::ExitCode;
use rustred_campaign_contract::classify::recipe::{FailureClass, ProbeVerdict, RelationTally};
use rustred_campaign_contract::classify::{
    self, Accept, Evidence, KillReason, OutcomeState, ROWS, VerifiedOutputs, WaitStatus,
};
use rustred_campaign_contract::outcome::{
    ErrorClass, ErrorInfo, JobOutcome, PerCase, StopInfo, TimeDecision, TimeDecisionWhat,
};
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
        (WaitStatus::Unknown, O::Absent) => {
            return if e.resume_valid { (21, "b") } else { (21, "c") };
        }
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
                // lane symcert: a proof-symrows certificate is read only at exit 0 when verified (43, never 22)
                if e.kind == JobKind::ProofSymrows && (e.verified.is_none() || c != 0) {
                    return (43, "");
                }
                let Some(v) = &e.verified else {
                    return (22, "");
                };
                if v.elided_loci > 0 {
                    return (25, "");
                }
                if e.kind == JobKind::ProofSymrows {
                    return (40, "");
                }
                // adapters.md 1.2: solve-recipe uses rows 1 / 28 / 29 and needs the results tally (S); proof-instantiate
                // has exit 0 only; keys_unsolved belongs to solve-recipe only
                if e.kind == JobKind::SolveRecipe {
                    let Some(t) = &v.relations else {
                        return (22, "");
                    };
                    // line 7: a probe job (its tally says so): 31 / 32 / 22, never an accepted relation
                    if t.probe {
                        let open: u64 = t
                            .probes
                            .iter()
                            .filter(|(v, _)| !v.is_answer())
                            .map(|(_, n)| n)
                            .sum();
                        return match (c, stop) {
                            (0, _) if open == 0 && t.accepted == 0 => (31, ""),
                            (10, Some(StopCode::KeysUnsolved)) if open >= 1 && t.accepted == 0 => {
                                (32, "")
                            }
                            _ => (22, ""),
                        };
                    }
                    let unsolved: u64 = t.unsolved.values().sum();
                    return match (c, stop) {
                        (0, _) if unsolved == 0 => (1, ""),
                        (10, Some(StopCode::KeysUnsolved)) if unsolved >= 1 && t.accepted >= 1 => {
                            (28, "")
                        }
                        (10, Some(StopCode::KeysUnsolved)) if unsolved >= 1 => (29, ""),
                        _ => (22, ""),
                    };
                }
                // line 7: node-basis has exit 0 only (row 30)
                if e.kind == JobKind::NodeBasis {
                    return if c == 0 { (30, "") } else { (22, "") };
                }
                if e.kind.legacy_wrapped() && c != 0 {
                    return (22, "");
                }
                if stop == Some(StopCode::KeysUnsolved) {
                    return (22, "");
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
                        if v.rules >= 1 {
                            (5, "")
                        } else {
                            (27, "")
                        }
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
        // lane symcert: proof-symrows' typed refusal of a symmetry row (41) and its reader errors (42)
        if let O::Ok(j) = &o
            && [41, 70].contains(&c)
            && e.kind == JobKind::ProofSymrows
        {
            let m = j.error.as_ref().map(|x| x.message.as_str()).unwrap_or("");
            if c == 41 && m.contains("SYMROW_REFUSED") {
                return (41, "");
            }
            if m.contains("UNREADABLE_RECORD") || m.contains("UNKNOWN_RECORD_KIND") {
                return (42, "");
            }
        }
        // line 7: a legacy proof tool's typed reader error (in OUTCOME's error message) is the input refusal 13b
        if let O::Ok(j) = &o
            && [41, 70].contains(&c)
            && matches!(
                e.kind,
                JobKind::ProofInstantiate | JobKind::ProofInstanceSpan
            )
            && j.error.as_ref().is_some_and(|x| {
                x.message.contains("UNREADABLE_RECORD") || x.message.contains("UNKNOWN_RECORD_KIND")
            })
        {
            return (13, "b");
        }
        match c {
            40 => return (12, ""),
            41 => return (13, ""),
            42 => return (14, ""),
            50 => return (15, ""),
            // row 15b (lane int5): a host-resource errno named by the OUTCOME error, at 51 / 70
            51 | 70 if matches!(&o, O::Ok(j) if j.error.as_ref().is_some_and(|x| x.message.contains("Os { code: 11"))) =>
            {
                return (15, "b");
            }
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
fn may_accept(e: &Evidence, accept: Accept) -> Result<(), String> {
    let OutcomeState::Valid(j) = &e.outcome else {
        return Err("accepted without a valid OUTCOME".into());
    };
    let c = match e.wait {
        WaitStatus::Exited(c) => {
            if c != j.exit_code.code() {
                return Err(format!(
                    "accepted with process exit {c} != OUTCOME {}",
                    j.exit_code.code()
                ));
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
    if (e.kind == JobKind::SolveRecipe || e.kind.legacy_wrapped()) && c == 20 {
        return Err("a kind without work budgets accepted on 20".into());
    }
    // line 7: diagnostic records only from node-basis (exit 0) and solve-recipe probe jobs; never rules / relations
    let probe = v.relations.as_ref().is_some_and(|t| t.probe);
    let diagnostic_kind = e.kind == JobKind::NodeBasis || (e.kind == JobKind::SolveRecipe && probe);
    // lane symcert: certificates only from proof-symrows, and only at exit 0
    if (accept == Accept::Certificates) != (e.kind == JobKind::ProofSymrows) {
        return Err(format!("accept {accept:?} for kind {:?}", e.kind));
    }
    if e.kind == JobKind::ProofSymrows {
        return if c == 0 {
            Ok(())
        } else {
            Err(format!("proof-symrows accepted on exit {c}"))
        };
    }
    if (accept == Accept::DiagnosticRecords) != diagnostic_kind {
        return Err(format!(
            "accept {accept:?} for kind {:?} (probe tally {probe})",
            e.kind
        ));
    }
    if e.kind == JobKind::NodeBasis {
        return if c == 0 {
            Ok(())
        } else {
            Err(format!("node-basis accepted on exit {c}"))
        };
    }
    if e.kind == JobKind::SolveRecipe && probe {
        let t = v.relations.as_ref().expect("probe tally");
        let open: u64 = t
            .probes
            .iter()
            .filter(|(v, _)| !v.is_answer())
            .map(|(_, n)| n)
            .sum();
        return match c {
            _ if t.accepted != 0 => Err("a probe job with an accepted relation".into()),
            0 if open == 0 => Ok(()),
            10 if j.stop.code == Some(StopCode::KeysUnsolved) && open >= 1 => Ok(()),
            _ => Err(format!(
                "probe job read on exit {c} with {open} unanswered keys"
            )),
        };
    }
    if e.kind == JobKind::SolveRecipe {
        let Some(t) = &v.relations else {
            return Err("solve-recipe accepted without the results tally".into());
        };
        let unsolved: u64 = t.unsolved.values().sum();
        return match c {
            0 if unsolved == 0 => Ok(()),
            10 if j.stop.code == Some(StopCode::KeysUnsolved)
                && t.accepted >= 1
                && unsolved >= 1 =>
            {
                Ok(())
            }
            _ => Err(format!(
                "solve-recipe accepted on exit {c} with {} accepted / {unsolved} unsolved",
                t.accepted
            )),
        };
    }
    if c == 10 {
        match j.stop.code {
            Some(StopCode::MaxCases | StopCode::OpenRegions | StopCode::OpenObligations)
                if v.rules >= 1 => {}
            Some(
                StopCode::LiftRefused
                | StopCode::GuardTimeout
                | StopCode::NumericSearchRefused
                | StopCode::BeforeNumeric,
            ) => {}
            s => {
                return Err(format!(
                    "accepted on exit 10 with stop {s:?} and {} verified rules",
                    v.rules
                ));
            }
        }
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------ the space

fn waits() -> Vec<WaitStatus> {
    let mut w: Vec<WaitStatus> = [
        0,
        1,
        2,
        9,
        10,
        11,
        12,
        13,
        20,
        21,
        22,
        23,
        30,
        31,
        40,
        41,
        42,
        43,
        50,
        51,
        52,
        69,
        70,
        71,
        101,
        127,
        128,
        129,
        130,
        134,
        137,
        143,
        158,
        192,
        193,
        255,
        256,
        -1,
        i32::MIN,
        i32::MAX,
    ]
    .iter()
    .map(|&c| WaitStatus::Exited(c))
    .collect();
    w.extend(
        [0, 1, 2, 3, 6, 9, 11, 13, 15, 31, 64, -9]
            .iter()
            .map(|&s| WaitStatus::Signaled(s)),
    );
    w.push(WaitStatus::Unknown);
    w
}

/// Every OUTCOME the contract can call valid for `kind`: internally consistent (`check()` passes) and matching the
/// spec, over exit code x stop x rules x per-case list x time decisions x graceful.
fn valid_outcomes(kind: JobKind) -> Vec<JobOutcome> {
    let mut spec = sample_spec("/r");
    spec.kind = kind;
    let mut out = Vec::new();
    let stops: Vec<Option<StopCode>> = std::iter::once(None)
        .chain(StopCode::ALL.iter().copied().map(Some))
        .collect();
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
                                1 => vec![
                                    per_case(CaseResult::Covered),
                                    per_case(CaseResult::Finite),
                                ],
                                _ => vec![
                                    per_case(CaseResult::Covered),
                                    per_case(CaseResult::Refused),
                                ],
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
    let base: Vec<Option<VerifiedOutputs>> = vec![
        None,
        Some(VerifiedOutputs {
            rules: 0,
            elided_loci: 0,
            streams: vec![],
            relations: None,
        }),
        Some(VerifiedOutputs {
            rules: 1,
            elided_loci: 0,
            streams: vec![],
            relations: None,
        }),
        Some(VerifiedOutputs {
            rules: 1,
            elided_loci: 2,
            streams: vec![],
            relations: None,
        }),
    ];
    // solve-recipe: the same states plus the results tally (accepted 0 / 1 x unsolved 0 / 1)
    let tally = |accepted: u64, unsolved: u64| {
        let mut t = RelationTally {
            records: accepted + unsolved,
            accepted,
            ..Default::default()
        };
        if unsolved > 0 {
            t.unsolved.insert(FailureClass::NoRelationInSeeds, unsolved);
        }
        Some(VerifiedOutputs {
            rules: 0,
            elided_loci: 0,
            streams: vec![],
            relations: Some(t),
        })
    };
    let mut with_tally = base.clone();
    with_tally.extend([tally(0, 0), tally(1, 0), tally(0, 1), tally(1, 1)]);
    // line 7: probe tallies (answered x unanswered), and one that lies (an accepted relation in a probe job)
    let probe_tally = |answered: u64, open: u64, accepted: u64| {
        let mut t = RelationTally {
            records: answered + open,
            accepted,
            probe: true,
            ..Default::default()
        };
        if answered > 0 {
            t.probes.insert(ProbeVerdict::NoRelationAtD0, answered);
        }
        if open > 0 {
            t.probes.insert(ProbeVerdict::UnresolvedPrimes, open);
        }
        Some(VerifiedOutputs {
            rules: 0,
            elided_loci: 0,
            streams: vec![],
            relations: Some(t),
        })
    };
    with_tally.extend([
        probe_tally(0, 0, 0),
        probe_tally(1, 0, 0),
        probe_tally(0, 1, 0),
        probe_tally(1, 1, 0),
        probe_tally(1, 0, 1),
    ]);
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
        outcomes.extend(
            valid_outcomes(kind)
                .into_iter()
                .map(|j| OutcomeState::Valid(Box::new(j))),
        );
        // line 7: OUTCOMEs naming a typed reader error (row 13b for the legacy proof kinds; 13 / 17 elsewhere)
        let reader: Vec<OutcomeState> = valid_outcomes(kind)
            .into_iter()
            .filter(|j| [41, 70].contains(&j.exit_code.code()))
            .filter_map(|mut j| {
                let class = if j.exit_code.code() == 41 {
                    ErrorClass::Input
                } else {
                    ErrorClass::Internal
                };
                j.error = Some(ErrorInfo::new(class, "UNKNOWN_RECORD_KIND: line 947"));
                j.check().is_ok().then(|| OutcomeState::Valid(Box::new(j)))
            })
            .collect();
        outcomes.extend(reader);
        // OUTCOMEs naming a host-resource errno at 51 / 70 (row 15b; no full-product case had one before line 7)
        let errno: Vec<OutcomeState> = valid_outcomes(kind)
            .into_iter()
            .filter(|j| [51, 70].contains(&j.exit_code.code()))
            .filter_map(|mut j| {
                let class = if j.exit_code.code() == 51 {
                    ErrorClass::Io
                } else {
                    ErrorClass::Internal
                };
                j.error = Some(ErrorInfo::new(
                    class,
                    "failed to spawn thread: Os { code: 11, kind: WouldBlock }",
                ));
                j.check().is_ok().then(|| OutcomeState::Valid(Box::new(j)))
            })
            .collect();
        outcomes.extend(errno);
        // lane symcert: OUTCOMEs naming the kernel's symmetry-row refusal at 41 / 70 (row 41 for proof-symrows at 41)
        let symrow: Vec<OutcomeState> = valid_outcomes(kind)
            .into_iter()
            .filter(|j| [41, 70].contains(&j.exit_code.code()))
            .filter_map(|mut j| {
                let class = if j.exit_code.code() == 41 {
                    ErrorClass::Input
                } else {
                    ErrorClass::Internal
                };
                j.error = Some(ErrorInfo::new(
                    class,
                    "SYMROW_REFUSED: loop part not unimodular (symrow e3 [1, 2])",
                ));
                j.check().is_ok().then(|| OutcomeState::Valid(Box::new(j)))
            })
            .collect();
        outcomes.extend(symrow);
        let verified = if kind == JobKind::SolveRecipe {
            &with_tally
        } else {
            &base
        };
        for wait in waits() {
            for o in &outcomes {
                for v in verified {
                    for r in [false, true] {
                        for k in kills {
                            for oom in [false, true] {
                                for elision_forbidden in [true, false] {
                                    n += 1;
                                    if !n.is_multiple_of(stride) {
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
                                    let spec_row =
                                        ROWS.iter().find(|x| x.row == c.row && x.sub == c.sub);
                                    let Some(spec_row) = spec_row else {
                                        mismatches
                                            .push(format!("row {}{} not in ROWS", c.row, c.sub));
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
                                        && let Err(why) = may_accept(&e, c.accept)
                                        && unsafe_accepts.len() < 20
                                    {
                                        unsafe_accepts
                                            .push(format!("row {}{}: {why}", c.row, c.sub));
                                    }
                                    if c.resume_from.is_some() && !r {
                                        unsafe_accepts
                                            .push("resume_from without a valid RESUME".into());
                                    }
                                    *stats
                                        .entry(format!("{:>2}{:<1}", c.row, c.sub))
                                        .or_default() += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "decision table: {n} combinations in the space, {checked} classified (stride {stride}), {panics} panics"
    );
    assert_eq!(panics, 0, "classify panicked");
    assert!(
        mismatches.is_empty(),
        "rows differ from the document's oracle:\n{}",
        mismatches.join("\n")
    );
    assert!(
        unsafe_accepts.is_empty(),
        "unsafe acceptance:\n{}",
        unsafe_accepts.join("\n")
    );
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
