//! Job spec validation (typed errors, exit 40 / 42, unknown-field policy), the child's preflight, atomic writes and
//! the events tail reader.

mod common;
use common::*;
use rustred_campaign_contract::ExitCode;
use rustred_campaign_contract::atomic::*;
use rustred_campaign_contract::events::*;
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::preflight::*;
use rustred_campaign_contract::spec::*;
use std::io::Write;

fn err(s: &JobSpec) -> SpecError {
    s.validate().unwrap_err()
}

#[test]
fn spec_validation_typed_errors() {
    let ok = sample_spec("/abs/run");
    ok.validate().unwrap();
    let mut s = ok.clone();
    s.schema = "rustred.child-job.v0".into();
    assert!(matches!(err(&s), SpecError::Schema(_)));
    let mut s = ok.clone();
    s.job_id = "../x".into();
    assert!(matches!(
        err(&s),
        SpecError::BadField {
            field: "job_id",
            ..
        }
    ));
    let mut s = ok.clone();
    s.attempt = 0;
    assert!(matches!(
        err(&s),
        SpecError::BadField {
            field: "attempt",
            ..
        }
    ));
    let mut s = ok.clone();
    s.run_dir = "rel/run".into();
    assert!(matches!(
        err(&s),
        SpecError::BadField {
            field: "run_dir",
            ..
        }
    ));
    let mut s = ok.clone();
    s.program.blake3 = "ABC".into();
    assert!(matches!(
        err(&s),
        SpecError::BadField {
            field: "program.blake3",
            ..
        }
    ));
    let mut s = ok.clone();
    s.args.push("--max-cases=1".into());
    let e = err(&s);
    assert_eq!(e, SpecError::TypedFlagInArgs("--max-cases".into()));
    assert_eq!(e.exit_code(), ExitCode::Usage);
    let mut s = ok.clone();
    s.args.push("--registry".into());
    let e = err(&s);
    assert_eq!(e, SpecError::RefusedFlag("--registry".into()));
    assert_eq!(e.exit_code(), ExitCode::Refused);
    let mut s = ok.clone();
    s.determinism = Determinism::Strict;
    s.args.extend(["--on-heavy".into(), "auto".into()]);
    assert!(matches!(err(&s), SpecError::BadField { field: "args", .. }));
    let mut s = ok.clone();
    s.budgets.work.seeds_per_case = Some(0);
    assert!(matches!(
        err(&s),
        SpecError::BadField {
            field: "budgets.work",
            ..
        }
    ));
    let mut s = ok.clone();
    s.heartbeat_s = 0;
    assert!(matches!(
        err(&s),
        SpecError::BadField {
            field: "heartbeat_s",
            ..
        }
    ));
    let mut s = ok.clone();
    s.env_allow.push("1BAD".into());
    assert!(matches!(
        err(&s),
        SpecError::BadField {
            field: "env_allow",
            ..
        }
    ));
    let mut s = ok.clone();
    s.kind = JobKind::Fake;
    assert!(matches!(err(&s), SpecError::ArgNotAllowed(_)));
    s.args.clear();
    assert!(matches!(err(&s), SpecError::FakeMismatch(_)));
}

#[test]
fn spec_unknown_fields_are_refused_and_defaults_apply() {
    let ok = sample_spec("/abs/run");
    let mut v = serde_json::to_value(&ok).unwrap();
    v["budgets"]["work"]["seeds_per_cse"] = serde_json::json!(5);
    let e = JobSpec::from_json(&serde_json::to_vec(&v).unwrap()).unwrap_err();
    assert!(
        matches!(e, SpecError::Parse(ref m) if m.contains("seeds_per_cse")),
        "{e}"
    );
    assert_eq!(e.exit_code(), ExitCode::Usage);
    let mut v = serde_json::to_value(&ok).unwrap();
    v["surprise"] = serde_json::json!(true);
    assert!(JobSpec::from_json(&serde_json::to_vec(&v).unwrap()).is_err());
    // minimal spec: defaults (elision forbidden, recorded, stop, heartbeat 10, grace 20, resume 30)
    let min = format!(
        r#"{{"schema":"rustred.child-job.v1","job_id":"j1","attempt":1,"kind":"descent","run_dir":"/abs/r","program":{{"name":"rr_node","blake3":"{D0}"}}}}"#
    );
    let s = JobSpec::from_json(min.as_bytes()).unwrap();
    assert!(s.elision_forbidden());
    assert_eq!(
        (s.heartbeat_s, s.stop_grace_s, s.resume_every_s),
        (10, 20, 30)
    );
    assert_eq!(
        (s.determinism, s.on_case_budget),
        (Determinism::Recorded, OnCaseBudget::Stop)
    );
    assert!(JobSpec::from_json(b"not json").is_err());
}

#[test]
fn budgets_first_hit() {
    let w = WorkBudgets {
        seeds_per_case: Some(10),
        cases: Some(2),
        ..Default::default()
    };
    let mut case = Counters::default();
    let mut job = Counters::default();
    assert!(w.first_hit(&case, &job).is_none());
    case.add(WorkCounter::Seeds, 10);
    let h = w.first_hit(&case, &job).unwrap();
    assert_eq!(
        (h.counter, h.scope, h.limit, h.value),
        (WorkCounter::Seeds, Scope::Case, 10, 10)
    );
    case = Counters::default();
    assert!(!w.cases_exhausted(&job));
    job.add(WorkCounter::Cases, 2);
    assert!(
        w.first_hit(&case, &job).is_none(),
        "cases is max_cases, not a work budget"
    );
    assert!(w.cases_exhausted(&job));
    job.add(WorkCounter::TraceRows, 5);
    job.add(WorkCounter::TraceRows, 3);
    assert_eq!(job.trace_rows_max, 5);
}

const ME: ProgramSelf<'static> = ProgramSelf {
    name: "rr_node",
    features: &["initial_presented"],
    kinds: &[JobKind::Generate],
    blake3: None,
};

fn setup(tag: &str) -> (TempDir, JobSpec, std::path::PathBuf) {
    let d = TempDir::new(tag);
    let fam = d.path().join("T.sym");
    std::fs::write(&fam, b"family").unwrap();
    let mut s = sample_spec(d.path().to_str().unwrap());
    s.inputs = vec![InputRef {
        role: "family".into(),
        path: "T.sym".into(),
        blake3: rustred_campaign_contract::blake3_hex(b"family"),
        upto: None,
    }];
    let p = d.path().join("job.json");
    std::fs::write(&p, s.to_json_line()).unwrap();
    (d, s, p)
}

fn no_env(_: &str) -> Option<String> {
    None
}

#[test]
fn preflight_ok_and_each_refusal() {
    let (d, s, p) = setup("pf");
    let ok = preflight(&p, &ME, &no_env).unwrap();
    assert_eq!(ok.inputs.len(), 1);
    assert!(ok.inputs[0].verified);
    assert_eq!(
        ok.spec_blake3,
        rustred_campaign_contract::spec::spec_blake3(&std::fs::read(&p).unwrap())
    );
    let code = |r: Result<Prepared, Refusal>| r.map(|_| ()).unwrap_err();
    // unreadable spec: 40, run dir unknown
    let r = code(preflight(&d.path().join("nope.json"), &ME, &no_env));
    assert_eq!((r.code, r.write_outcome), (ExitCode::Usage, false));
    // features
    let me2 = ProgramSelf {
        features: &[],
        ..ME
    };
    assert_eq!(code(preflight(&p, &me2, &no_env)).code, ExitCode::Refused);
    // wrong kind / program / own digest
    let me3 = ProgramSelf {
        kinds: &[JobKind::Descent],
        ..ME
    };
    assert_eq!(code(preflight(&p, &me3, &no_env)).code, ExitCode::Refused);
    let me4 = ProgramSelf {
        blake3: Some(D1),
        ..ME
    };
    assert_eq!(code(preflight(&p, &me4, &no_env)).code, ExitCode::Refused);
    // gated env
    let env = |k: &str| (k == "RUSTRED_PROBE_POINT").then(|| "1,2".to_string());
    assert_eq!(code(preflight(&p, &ME, &env)).code, ExitCode::Refused);
    // input digest mismatch: 41
    std::fs::write(d.path().join("T.sym"), b"changed").unwrap();
    let r = code(preflight(&p, &ME, &no_env));
    assert_eq!((r.code, r.write_outcome), (ExitCode::Input, true));
    std::fs::write(d.path().join("T.sym"), b"family").unwrap();
    // dirty out/
    std::fs::create_dir_all(d.path().join("out")).unwrap();
    std::fs::write(d.path().join("out/x"), b"").unwrap();
    assert_eq!(code(preflight(&p, &ME, &no_env)).code, ExitCode::Refused);
    std::fs::remove_file(d.path().join("out/x")).unwrap();
    // OUTCOME exists: 42 and OUTCOME must not be overwritten
    std::fs::write(d.path().join("OUTCOME.json"), b"{}").unwrap();
    let r = code(preflight(&p, &ME, &no_env));
    assert_eq!((r.code, r.write_outcome), (ExitCode::Refused, false));
    std::fs::remove_file(d.path().join("OUTCOME.json")).unwrap();
    // run_dir elsewhere: 42, nothing written
    let mut s2 = s.clone();
    s2.run_dir = "/somewhere/else".into();
    std::fs::write(&p, s2.to_json_line()).unwrap();
    let r = code(preflight(&p, &ME, &no_env));
    assert_eq!((r.code, r.write_outcome), (ExitCode::Refused, false));
    // invalid spec content: 40 with the spec known
    let mut s3 = s.clone();
    s3.args.push("--out".into());
    std::fs::write(&p, s3.to_json_line()).unwrap();
    let r = code(preflight(&p, &ME, &no_env));
    assert_eq!((r.code, r.write_outcome), (ExitCode::Usage, true));
}

#[test]
fn atomic_write_replaces_whole_file() {
    let d = TempDir::new("atomic");
    let p = d.path().join("OUTCOME.json");
    write_atomic(&p, b"first").unwrap();
    write_atomic(&p, b"second, longer").unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), b"second, longer");
    let names: Vec<_> = std::fs::read_dir(d.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 1, "no temp file left: {names:?}");
    // a failing target directory leaves nothing behind
    assert!(write_atomic(&d.path().join("missing/x.json"), b"x").is_err());
    // reserve
    let r = d.path().join(".reserve");
    create_reserve(&r, rustred_campaign_contract::layout::RESERVE_BYTES).unwrap();
    assert_eq!(std::fs::metadata(&r).unwrap().len(), 1 << 20);
    release_reserve(&r).unwrap();
    release_reserve(&r).unwrap();
    // /dev/full gives ENOSPC
    if let Ok(mut f) = std::fs::OpenOptions::new().write(true).open("/dev/full") {
        let e = f
            .write_all(&[0u8; 4096])
            .and_then(|_| f.flush())
            .unwrap_err();
        assert!(is_nospace(&e));
        assert_eq!(exit_for_io(&e), ExitCode::NoSpace);
    }
}

#[test]
fn events_sink_and_tail() {
    let d = TempDir::new("events");
    let p = d.path().join("EVENTS.jsonl");
    let sink = EventSink::create(&p).unwrap();
    let mut tail = EventsTail::new(4096);
    assert!(tail.poll(&p).unwrap().is_empty());
    sink.emit(Event::Phase {
        phase: Phase::Prepare,
    })
    .unwrap();
    sink.emit(Event::JobFinished {
        exit_code: ExitCode::Complete,
        status: rustred_campaign_contract::JobStatus::Complete,
        stop_code: None,
    })
    .unwrap();
    // a torn line followed by its completion in a later poll
    let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
    f.write_all(br#"{"v":1,"seq":2,"t":0.5,"kind":"pha"#)
        .unwrap();
    let items = tail.poll(&p).unwrap();
    assert_eq!(items.len(), 2);
    match &items[1] {
        TailItem::Event(e) => {
            assert_eq!(e.seq, 1);
            assert!(e.utc.is_some());
        }
        x => panic!("{x:?}"),
    }
    f.write_all(b"se\",\"phase\":\"numeric\"}\n").unwrap();
    // an over-long line is skipped, bounded
    let long = format!("{{\"x\":\"{}\"}}\n", "a".repeat(10_000));
    f.write_all(long.as_bytes()).unwrap();
    f.write_all(b"garbage\n").unwrap();
    let items = tail.poll(&p).unwrap();
    assert_eq!(items.len(), 3, "{items:?}");
    assert!(matches!(&items[0], TailItem::Event(e) if e.seq == 2));
    assert!(matches!(&items[1], TailItem::Bad(_, n) if *n > 10_000));
    assert!(matches!(&items[2], TailItem::Bad(_, 7)));
    assert_eq!(tail.offset(), std::fs::metadata(&p).unwrap().len());
    assert_eq!(sink.next_seq(), 2);
}

#[test]
fn layout_paths() {
    use rustred_campaign_contract::layout::{attempt_dir, attempt_rel, job_id};
    // plan 3.3.0: WDIR/jobs/<job content id>.<rung>.a<attempt>/
    let j = job_id("0123456789abcdef", "L0");
    assert_eq!(attempt_rel(&j, 2), "jobs/0123456789abcdef.L0.a2");
    let l = RunLayout::new("/c/jobs/0123456789abcdef.L0.a2");
    assert_eq!(attempt_dir(std::path::Path::new("/c"), &j, 2), l.root);
    assert_eq!(
        l.table(),
        std::path::PathBuf::from("/c/jobs/0123456789abcdef.L0.a2/out/F.table.jsonl")
    );
    assert_eq!(l.at("out/F.table.jsonl"), l.table());
    assert_eq!(RunLayout::out_rel("F.cert.jsonl"), "out/F.cert.jsonl");
}

/// (lane engbase) every engine-line seed option of coord-ec1-seed-centre.md v2 section 6 passes the generate
/// allow-list in its two-token form; malformed values and the file-reading `--region` do not.
#[test]
fn engine_line_seed_options_are_allowed() {
    let base = sample_spec("/abs/run");
    let worked: [&[&str]; 8] = [
        &["--max-depth", "2"],
        &[
            "--max-depth",
            "2",
            "--seed-centre-offset",
            "0:-1",
            "--seed-face-floor",
            "1",
        ],
        &[
            "--max-depth",
            "3",
            "--seed-centre-offset",
            "6:-1,11:1",
            "--seed-face-floor",
            "1",
        ],
        &[
            "--max-depth",
            "2",
            "--seed-centre-offset",
            "0:-1;2:-1;7:-1",
            "--seed-face-floor",
            "1",
        ],
        &[
            "--seed-layer-cap",
            "0,0,1",
            "--max-depth",
            "3",
            "--numeric-depth",
            "3",
        ],
        &[
            "--seed-layer-cap",
            "0,1",
            "--max-depth",
            "3",
            "--numeric-depth",
            "4",
        ],
        &[
            "--max-depth",
            "2",
            "--symmetry-symbolic",
            "--seed-schedule",
            "symfirst",
        ],
        &[
            "--symmetry-rows",
            "--open-on-refusal",
            "--slim-rules",
            "--no-screen",
        ],
    ];
    for extra in worked {
        let mut s = base.clone();
        s.args = vec!["--order".into(), "phys".into()];
        s.args.extend(extra.iter().map(|x| x.to_string()));
        s.validate()
            .unwrap_or_else(|e| panic!("{extra:?} refused: {e:?}"));
    }
    for (bad, want_40) in [
        (&["--seed-layer-cap", "0/0"][..], true),
        (&["--seed-layer-cap=0,0,1"][..], true),
        (&["--max-depth", "two"][..], true),
        (&["--region", "region.json"][..], true),
    ] {
        let mut s = base.clone();
        s.args = vec!["--order".into(), "phys".into()];
        s.args.extend(bad.iter().map(|x| x.to_string()));
        let e = err(&s);
        assert_eq!(e.exit_code() == ExitCode::Usage, want_40, "{bad:?}: {e:?}");
    }
}
