//! Round-trip and golden-file tests for every schema (tests/data/*.json[l]): a schema change is a visible diff.
//! Regenerate with `RR_UPDATE_GOLDEN=1 cargo test -p rustred-campaign-contract --test schemas` and review the diff.

mod common;
use common::*;
use rustred_campaign_contract::events::*;
use rustred_campaign_contract::fake::*;
use rustred_campaign_contract::outcome::*;
use rustred_campaign_contract::resume::*;
use rustred_campaign_contract::spec::*;
use rustred_campaign_contract::stop::*;
use rustred_campaign_contract::{ExitCode, VersionInfo};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::PathBuf;

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

/// Compare the pretty JSON of `v` with the golden file, and parse the golden file back into `v`.
fn golden<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(name: &str, v: &T) {
    let mut s = serde_json::to_string_pretty(v).unwrap();
    s.push('\n');
    let p = data(name);
    if std::env::var_os("RR_UPDATE_GOLDEN").is_some() {
        std::fs::write(&p, &s).unwrap();
    }
    let g = std::fs::read_to_string(&p)
        .unwrap_or_else(|_| panic!("golden {name} missing (RR_UPDATE_GOLDEN=1)"));
    assert_eq!(s, g, "golden {name} differs");
    let back: T = serde_json::from_str(&g).unwrap();
    assert_eq!(&back, v, "golden {name} does not parse back");
}

fn golden_lines(name: &str, lines: &[String]) {
    let s: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let p = data(name);
    if std::env::var_os("RR_UPDATE_GOLDEN").is_some() {
        std::fs::write(&p, &s).unwrap();
    }
    let g = std::fs::read_to_string(&p).unwrap_or_else(|_| panic!("golden {name} missing"));
    assert_eq!(s, g, "golden {name} differs");
}

#[test]
fn golden_job_spec() {
    let s = sample_spec("/abs/CDIR/jobs/a71d0c9e/a1");
    s.validate().unwrap();
    golden("job_spec.generate.json", &s);
    // compact line form round-trips too
    assert_eq!(JobSpec::from_json(&s.to_json_line()).unwrap(), s);
}

fn fake_script() -> FakeScript {
    FakeScript {
        script: vec![
            FakeOp::Prepare { ms: 200 },
            FakeOp::Case {
                ordinal: 0,
                ms: 1500,
                work: [(WorkCounter::Seeds, 40000), (WorkCounter::Rows, 900000)]
                    .into_iter()
                    .collect(),
                result: CaseResult::Rule,
                rule: Some(FakeRule {
                    record: serde_json::json!({"chart":"C","target":"I(1)","rhs":[["I(0)","1"]]}),
                    loci: Some(FakeLoci { n: 2, len: 5000 }),
                }),
                children: vec![CaseSpec {
                    frame: Frame::Presented,
                    fixed: vec![Some(1), None, Some(0)],
                    equations: vec![],
                    seed_centre_offset: None,
                    extra: Default::default(),
                }],
            },
            FakeOp::Case {
                ordinal: 1,
                ms: 800,
                work: Default::default(),
                result: CaseResult::Covered,
                rule: None,
                children: vec![],
            },
            FakeOp::Heartbeats { n: 5 },
            FakeOp::TimeDecision {
                what: TimeDecisionWhat::GuardRoute,
                limit_s: 60.0,
                measured_s: 60.5,
                taken: "content".into(),
            },
            FakeOp::Alloc { mb: 16 },
            FakeOp::Helper { index: 0 },
            FakeOp::End {
                stop: Some(StopCode::MaxCases),
                verdict: None,
            },
        ],
        on_sigterm: OnSigterm::Slow,
        slow_ms: 3000,
        start_delay_ms: 0,
        rss: RssPlan::default(),
        helpers: vec![HelperPlan {
            sleep_ms: 60000,
            pdeathsig: true,
            ignore_sigterm: true,
            escape_group: false,
        }],
        faults: vec![Fault {
            at_op: 2,
            kind: FaultKind::TornWrite,
            bytes: 37,
            code: 0,
            path: None,
        }],
        time_scale: 1.0,
        volatile_jitter: true,
    }
}

#[test]
fn golden_fake_spec() {
    let mut s = sample_spec("/abs/CDIR/jobs/f1/a1");
    s.kind = JobKind::Fake;
    s.program = ProgramRef {
        name: "rr_fake_child".into(),
        blake3: D0.into(),
        features_required: vec!["fake".into()],
    };
    s.args.clear();
    s.inputs.clear();
    s.output.elide_witness_loci = true;
    s.fake = Some(fake_script());
    s.validate().unwrap();
    golden("job_spec.fake.json", &s);
}

#[test]
fn golden_outcome_complete_and_hard_stop() {
    let spec = sample_spec("/abs/CDIR/jobs/a71d0c9e/a1");
    let mut j = job_outcome(&spec, ExitCode::Incomplete, Some(StopCode::MaxCases), true);
    j.program.exe = Some(ExeInfo {
        path: "/abs/bin/rr_node".into(),
        dev: 64769,
        ino: 1234,
        size: 98765432,
        mtime: 1794614400,
    });
    j.effective = Some(Effective {
        subcommand: "generate".into(),
        args: spec.args.clone(),
        node_options: serde_json::json!({"max_rule_monomials":1000000}),
        order: serde_json::json!({"name":"phys_swap_0_3_rot7"}),
        env: [("SYMBOLICA_LICENSE".to_string(), LICENCE_PRESENT.to_string())]
            .into_iter()
            .collect(),
        blake3: D1.into(),
    });
    j.inputs = vec![InputCheck {
        role: "family".into(),
        path: "/abs/T162.sym".into(),
        bytes: 4242,
        blake3: D1.into(),
        verified: true,
    }];
    j.counters = Counters {
        cases: 1,
        seeds: 40000,
        rows: 900000,
        rules_committed: 1,
        ..Default::default()
    };
    j.per_case = vec![PerCase {
        ordinal: 0,
        case: serde_json::json!({"frame":"PRESENTED","fixed":[1,null]}),
        result: CaseResult::Rule,
        rule: Some(0),
        children_open: 3,
    }];
    j.outputs.queue = Some(QueueOutput { bytes: 300 });
    j.outputs.events = Some(EventsOutput { last_seq: 41 });
    j.outputs.resume = Some(ResumeOutput {
        path: "RESUME.json".into(),
        valid: true,
    });
    j.outputs.extra = vec![ExtraOutput {
        role: "witness".into(),
        path: "out/WITNESS.jsonl".into(),
        bytes: 10,
        blake3: D0.into(),
    }];
    j.timing = Timing {
        started_utc: Some("2026-11-13T23:00:00.000Z".into()),
        finished_utc: Some("2026-11-13T23:00:18.100Z".into()),
        wall_s: Some(18.1),
        cpu_s: Some(17.4),
    };
    j.memory = Memory {
        vmhwm_kb: Some(431000),
        rss_kb_at_end: Some(400000),
        helpers_peak_rss_kb: Some(0),
    };
    j.check().unwrap();
    let mut o = Outcome::new(j);
    o.legacy.insert("complete".into(), serde_json::json!(false));
    o.legacy.insert("rules".into(), serde_json::json!(1));
    golden("outcome.incomplete.json", &o);
    // hard-safe stop: unsealed, commit point only
    let mut h = job_outcome(
        &spec,
        ExitCode::BudgetWall,
        Some(StopCode::BudgetWall),
        false,
    );
    h.stop.graceful = false;
    h.time_decisions = vec![TimeDecision {
        what: TimeDecisionWhat::CaseWall,
        case_ordinal: Some(0),
        rule: None,
        limit_s: 300.0,
        measured_s: 300.2,
        taken: "hard_stop".into(),
    }];
    let t = h.outputs.table.as_mut().unwrap();
    t.committed_bytes = 2048;
    t.prefix_blake3 = Some(D1.into());
    h.check().unwrap();
    golden("outcome.hard_stop.json", &Outcome::new(h));
    // an error outcome
    let mut e = job_outcome(&spec, ExitCode::Input, Some(StopCode::InputError), false);
    e.outputs.table = None;
    e.error = Some(ErrorInfo::new(
        ErrorClass::Input,
        "input \"family\" digest mismatch",
    ));
    e.check().unwrap();
    golden("outcome.input_error.json", &Outcome::new(e));
}

#[test]
fn outcome_tolerates_unknown_and_keeps_legacy() {
    let g = std::fs::read_to_string(data("outcome.incomplete.json")).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&g).unwrap();
    v["future_top"] = serde_json::json!(1);
    v["job"]["future_field"] = serde_json::json!({"x":1});
    let o: Outcome = serde_json::from_value(v).unwrap();
    assert_eq!(o.legacy["future_top"], serde_json::json!(1));
    assert_eq!(o.legacy["rules"], serde_json::json!(1));
}

#[test]
fn outcome_check_rejects_inconsistencies() {
    let spec = sample_spec("/r");
    let ok = job_outcome(&spec, ExitCode::Complete, None, true);
    ok.check().unwrap();
    let mut a = ok.clone();
    a.status = JobStatus::Incomplete;
    assert!(a.check().is_err(), "status vs exit code");
    let mut b = ok.clone();
    b.stop.code = Some(StopCode::BudgetWork);
    assert!(b.check().is_err(), "stop vs exit code");
    let mut c = ok.clone();
    c.time_decisions.push(TimeDecision {
        what: TimeDecisionWhat::Admission,
        case_ordinal: None,
        rule: None,
        limit_s: 1.0,
        measured_s: 2.0,
        taken: "deferred".into(),
    });
    assert!(
        c.check().is_err(),
        "time_decided must follow time_decisions"
    );
    let mut d = ok.clone();
    d.outputs.table.as_mut().unwrap().end = Some(EndKind::Partial);
    assert!(d.check().is_err(), "end kind vs stop");
    let mut e = job_outcome(&spec, ExitCode::Incomplete, None, true);
    e.outputs.table.as_mut().unwrap().end = Some(EndKind::Partial);
    assert!(e.check().is_err(), "exit 10 needs a stop");
    let long = ErrorInfo::new(ErrorClass::Internal, "x".repeat(5000));
    assert_eq!(long.message.len(), 4000);
}

#[test]
fn golden_events() {
    let c = Counters {
        cases: 1,
        seeds: 10,
        ..Default::default()
    };
    let evs = vec![
        (
            Some("2026-11-13T23:00:00.000Z"),
            Event::JobStarted {
                job_id: "a71d0c9e".into(),
                attempt: 1,
                spec_blake3: D0.into(),
                pid: 4242,
                program: "rr_node".into(),
                version: "0.1.0".into(),
                git_commit: "unknown".into(),
                resume_from: None,
            },
        ),
        (
            None,
            Event::Phase {
                phase: Phase::CaseSearch,
            },
        ),
        (
            None,
            Event::CaseStarted {
                ordinal: 0,
                case: serde_json::json!({"frame":"PRESENTED","fixed":[1,null]}),
                pending: 3,
                numerical: 0,
            },
        ),
        (
            None,
            Event::RuleCommitted {
                index: 0,
                line_offset: 812,
                line_bytes: 1904,
                canon_blake3: D1.into(),
                rhs: 12,
                trace_rows: 40,
                monomials: 300,
            },
        ),
        (
            None,
            Event::CaseFinished {
                ordinal: 0,
                result: CaseResult::Rule,
                rule: Some(0),
                children: 2,
                rules_committed: 1,
            },
        ),
        (
            None,
            Event::Engine {
                event: serde_json::json!({"ExactStarted":{"trace_rows":40}}),
            },
        ),
        (
            None,
            Event::TimeDecision(TimeDecision {
                what: TimeDecisionWhat::GuardRoute,
                case_ordinal: Some(0),
                rule: Some(0),
                limit_s: 60.0,
                measured_s: 60.0,
                taken: "content".into(),
            }),
        ),
        (
            Some("2026-11-13T23:00:10.000Z"),
            Event::Heartbeat {
                phase: Phase::ExactLift,
                case_ordinal: Some(0),
                pending: 3,
                rules_committed: 1,
                counters: c,
                rss_kb: 1000,
                vmhwm_kb: 2000,
                helpers: 0,
                stop_requested: false,
            },
        ),
        (
            None,
            Event::BudgetHit {
                counter: WorkCounter::Seeds,
                scope: Scope::Case,
                limit: 10,
                value: 10,
            },
        ),
        (
            None,
            Event::StopRequested {
                source: StopSource::Sigterm,
                count: 1,
            },
        ),
        (
            None,
            Event::ResumeWritten {
                rules_committed: 1,
                pending: 2,
                table_bytes: 2716,
            },
        ),
        (
            Some("2026-11-13T23:00:11.000Z"),
            Event::JobFinished {
                exit_code: ExitCode::Interrupted,
                status: JobStatus::Interrupted,
                stop_code: Some(StopCode::Interrupted),
            },
        ),
    ];
    let mut lines = Vec::new();
    for (seq, (utc, e)) in evs.into_iter().enumerate() {
        let l = EventLine {
            v: 1,
            seq: seq as u64,
            t: round_ms(seq as f64 * 1.2345),
            utc: utc.map(str::to_string),
            event: e,
        };
        let bytes = l.to_line();
        assert_eq!(EventLine::parse(&bytes[..bytes.len() - 1]).unwrap(), l);
        lines.push(String::from_utf8(bytes[..bytes.len() - 1].to_vec()).unwrap());
    }
    golden_lines("events.jsonl", &lines);
    // utc on a kind that must not carry it is rejected
    assert!(
        EventLine::parse(br#"{"v":1,"seq":0,"t":0.0,"utc":"x","kind":"phase","phase":"prepare"}"#)
            .is_err()
    );
    assert!(
        EventLine::parse(br#"{"v":2,"seq":0,"t":0.0,"kind":"phase","phase":"prepare"}"#).is_err()
    );
}

#[test]
fn golden_resume_and_cases() {
    let case = CaseSpec {
        frame: Frame::Presented,
        fixed: vec![Some(2), None, Some(-1)],
        equations: vec![serde_json::json!("n1-n2")],
        seed_centre_offset: Some(serde_json::json!([[3, -1]])),
        extra: Default::default(),
    };
    let r = ResumeState {
        schema: rustred_campaign_contract::RESUME_SCHEMA.into(),
        job_id: "a71d0c9e".into(),
        attempt: 1,
        chart: serde_json::json!("11111111100000000000"),
        frame: Frame::Presented,
        rules_committed: 1,
        table_committed_bytes: 2716,
        table_prefix_blake3: D1.into(),
        current: Some(case.clone()),
        pending: vec![case.clone()],
        numerical: vec![vec![1, 1, 2, 0, -1]],
        deferred: vec![
            serde_json::json!({"chart":"C","conjunction":"n1=2","reason":"admit_timeout","rule":0,"branch":1}),
        ],
        refused_open: vec![],
        counters_at_commit: Counters {
            cases: 1,
            rules_committed: 1,
            ..Default::default()
        },
        symbolic_cases: 1,
        discarded: 0,
    };
    r.check().unwrap();
    golden("resume.json", &r);
    let rr = ResumeRef {
        from: "/abs/run/a1/RESUME.json".into(),
        blake3: D0.into(),
        table: "/abs/run/a1/out/F.table.jsonl".into(),
        committed_bytes: 2716,
        prefix_blake3: D1.into(),
    };
    golden("resume_ref.json", &rr);
    // cases input lines
    let lines = [
        r#"{"frame":"PRESENTED","fixed":[1,null,0],"equations":[]}"#,
        r#"{"frame":"ORIGINAL","fixed":[null,null,2],"equations":["n1-n3"],"seed_centre_offset":[[0,-1]],"note":"kept"}"#,
        r#"{"type":"spill","pending":[{"frame":"PRESENTED","fixed":[1,null,0]}],"current":null,"numerical":[[1,1,1]]}"#,
    ];
    match parse_cases_line(lines[0]).unwrap() {
        CasesLine::Case(c) => assert_eq!(c.fixed, vec![Some(1), None, Some(0)]),
        x => panic!("{x:?}"),
    }
    match parse_cases_line(lines[1]).unwrap() {
        CasesLine::Case(c) => {
            assert_eq!(c.frame, Frame::Original);
            assert_eq!(c.extra["note"], serde_json::json!("kept"));
            let back = serde_json::to_value(&c).unwrap();
            assert_eq!(
                back,
                serde_json::from_str::<serde_json::Value>(lines[1]).unwrap()
            );
        }
        x => panic!("{x:?}"),
    }
    match parse_cases_line(lines[2]).unwrap() {
        CasesLine::Spill(s) => assert_eq!((s.pending.len(), s.numerical.len()), (1, 1)),
        x => panic!("{x:?}"),
    }
    assert!(parse_cases_line(r#"{"frame":"SIDEWAYS","fixed":[]}"#).is_err());
}

#[test]
fn golden_version() {
    let v = VersionInfo {
        program: "rr_fake_child".into(),
        package_version: "0.1.0".into(),
        git_commit: rustred_campaign_contract::git_commit_from_build_env(None),
        contract: vec![rustred_campaign_contract::CONTRACT.into()],
        features: vec!["fake".into()],
        job_kinds: vec![JobKind::Fake],
        arities: vec![],
    };
    golden("version.json", &v);
    assert_eq!(
        rustred_campaign_contract::git_commit_from_build_env(Some(" ")),
        "unknown"
    );
    assert_eq!(
        rustred_campaign_contract::git_commit_from_build_env(Some("dff6a5a5")),
        "dff6a5a5"
    );
}

#[test]
fn golden_exit_and_stop_tables() {
    let exits: Vec<serde_json::Value> = ExitCode::ALL
        .iter()
        .map(|e| serde_json::json!({"code": e.code(), "name": e.name(), "status": JobStatus::for_exit(*e), "outcome_required": e.outcome_required(), "resumable": format!("{:?}", e.resumable())}))
        .collect();
    let stops: Vec<serde_json::Value> = StopCode::ALL
        .iter()
        .map(|s| serde_json::json!({"stop": s, "exit": s.exit_code().code()}))
        .collect();
    golden(
        "exit_codes.json",
        &serde_json::json!({"exit_codes": exits, "stops": stops, "volatile": rustred_campaign_contract::canon::VOLATILE_BY_TYPE.iter().map(|(t, k)| serde_json::json!({"type": t, "keys": k})).collect::<Vec<_>>()}),
    );
}
