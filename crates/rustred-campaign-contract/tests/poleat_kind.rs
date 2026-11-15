//! Kind `pole-regular-at` (lane coreseed; interfaces/coreseed.md 10): spec rules (roles, allow-list, legacy
//! constraints), the answer checks against the pinned requests, and the decision rows it reaches: 37 (answers sealed and
//! checked: diagnostic records), 38 (exit 0 / 10 with answers not readable: nothing read, never attention), 39 (exit
//! 41: the requests input refused), signals as for every kind.
//! Test data only (L8): keys of arity 3, made-up digests.

mod common;

use common::*;
use rustred_campaign_contract::classify::poleat::{self, ANSWER_SCHEMA, REQUEST_SCHEMA};
use rustred_campaign_contract::classify::{
    self, Accept, Action, KillReason, OutcomeKind, ROWS, WaitStatus,
};
use rustred_campaign_contract::exit::ExitCode;
use rustred_campaign_contract::layout::{RESULTS, RunLayout};
use rustred_campaign_contract::outcome::{
    EndKind, ErrorClass, ErrorInfo, ExtraOutput, JobOutcome, Outcome, ProgramInfo, StopInfo,
};
use rustred_campaign_contract::seal::RecordWriter;
use rustred_campaign_contract::spec::*;
use rustred_campaign_contract::stop::StopCode;
use serde_json::{Value, json};

fn b3(b: &[u8]) -> String {
    rustred_campaign_contract::blake3_hex(b)
}

const DIG: &str = "1111111111111111111111111111111111111111111111111111111111111111";

fn requests(n: usize) -> Vec<Value> {
    (0..n)
        .map(|i| {
            json!({"schema": REQUEST_SCHEMA, "id": format!("r{i}"), "key": [2, 1, -(i as i64) - 1], "d0": "4",
                "record": {"type": "relation", "key": [2, 1, -(i as i64) - 1], "relation_digest": format!("{DIG}{i}")}})
        })
        .collect()
}

fn answer(q: &Value, verdict: &str, order: Value) -> Value {
    json!({"schema": ANSWER_SCHEMA, "id": q["id"], "key": q["key"], "d0": "4", "verdict": verdict,
        "pole_order": order, "relation": {"relation_digest": q["record"]["relation_digest"]}})
}

fn jsonl(v: &[Value]) -> Vec<u8> {
    v.iter()
        .flat_map(|x| format!("{x}\n").into_bytes())
        .collect()
}

struct Job {
    _t: TempDir,
    spec: JobSpec,
    digest: String,
    layout: RunLayout,
}

fn job(tag: &str, reqs: &[Value]) -> Job {
    let t = TempDir::new(tag);
    let run = t.path().join("run");
    let layout = RunLayout::new(&run);
    for d in [
        layout.in_dir(),
        layout.out_dir(),
        layout.tmp_dir(),
        layout.sup_dir(),
    ] {
        std::fs::create_dir_all(d).unwrap();
    }
    let rb = jsonl(reqs);
    let rp = layout.in_dir().join("requests.jsonl");
    std::fs::write(&rp, &rb).unwrap();
    let mut spec = sample_spec(&run.display().to_string());
    spec.kind = JobKind::PoleRegularAt;
    spec.program = ProgramRef {
        name: "rr_poles".into(),
        blake3: D0.into(),
        features_required: vec![],
    };
    spec.inputs = vec![InputRef {
        role: "requests".into(),
        path: rp.display().to_string(),
        blake3: b3(&rb),
        upto: None,
    }];
    spec.args = vec!["--d0".into(), "4".into()];
    spec.budgets = Budgets::default();
    spec.validate().unwrap();
    let bytes = spec.to_json_line();
    let digest = spec_blake3(&bytes);
    std::fs::write(layout.job_spec(), &bytes).unwrap();
    Job {
        _t: t,
        spec,
        digest,
        layout,
    }
}

fn program() -> ProgramInfo {
    ProgramInfo {
        name: "rr_poles".into(),
        version: "legacy".into(),
        git_commit: rustred_campaign_contract::UNKNOWN_COMMIT.into(),
        features: vec!["legacy-wrapped".into()],
        exe: None,
    }
}

/// What the supervisor-side wrapper writes for an exit 0 whose answers check: sealed results + OUTCOME.
fn seal_and_outcome(j: &Job, answers: &[Value], seal: bool) {
    let mut o = JobOutcome::for_spec(
        &j.spec,
        &j.digest,
        program(),
        ExitCode::Complete,
        StopInfo::none(),
    );
    if seal {
        let mut w = RecordWriter::create(&j.layout.results()).unwrap();
        for a in answers {
            w.append(a.to_string().as_bytes()).unwrap();
        }
        let s = w
            .seal(
                json!({"type": "end", "records": answers.len()})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
        o.outputs.extra.push(ExtraOutput {
            role: "results".into(),
            path: RunLayout::out_rel(RESULTS),
            bytes: s.bytes,
            blake3: s.blake3,
            records: Some(s.records),
            canonical_blake3: Some(s.canonical_blake3),
            end: Some(EndKind::End),
            sealed: true,
        });
    }
    Outcome::new(o).write_new(&j.layout.outcome()).unwrap();
}

fn classify_job(j: &Job, wait: WaitStatus) -> classify::Classification {
    let ev = classify::assess(&j.layout, &j.spec, &j.digest, wait, KillReason::None, false);
    classify::classify(&ev)
}

#[test]
fn the_kind_is_a_legacy_diagnostic_kind_with_pinned_requests() {
    let k = JobKind::PoleRegularAt;
    assert_eq!(k.level(), Level::Legacy);
    assert_eq!(k.program(), "rr_poles");
    assert!(k.legacy_wrapped() && k.fake_emulated() && k.sealed_outputs().is_some());
    let roles = k.input_roles().unwrap();
    assert_eq!(roles.required, &["requests"]);
    let j = job("pole-spec", &requests(1));
    let mut bad = j.spec.clone();
    bad.args = vec!["--elided-loci".into(), "facts".into()];
    assert!(bad.validate().is_err(), "flag outside the allow-list");
    let mut bad = j.spec.clone();
    bad.inputs.clear();
    assert!(bad.validate().is_err(), "the requests input is required");
    let argv = poleat::argv(&j.spec).unwrap();
    assert_eq!(argv[0], "regular-at");
    assert!(argv.contains(&"--d0".to_string()));
    // rows 37 / 38 / 39 exist with their semantics: diagnostic records only, never attention
    let r = |n: u8| ROWS.iter().find(|r| r.row == n).unwrap();
    assert_eq!(
        (r(37).kind, r(37).accept),
        (OutcomeKind::Complete, Accept::DiagnosticRecords)
    );
    assert_eq!(
        (r(38).accept, r(38).action),
        (Accept::Nothing, Action::DiagnosticsUndecided)
    );
    assert_eq!(
        (r(39).accept, r(39).action),
        (Accept::Nothing, Action::DiagnosticsUndecided)
    );
}

#[test]
fn answers_are_checked_against_their_requests() {
    let q = requests(3);
    let good = vec![
        answer(&q[0], "regular", json!(0)),
        answer(&q[1], "pole", json!(2)),
        answer(&q[2], "undecided", Value::Null),
    ];
    let a = poleat::check_answers(&jsonl(&q), &jsonl(&good)).unwrap();
    assert_eq!(
        a.iter().map(|x| x.pole_order).collect::<Vec<_>>(),
        vec![Some(0), Some(2), None]
    );
    // one missing, reordered, wrong digest, inconsistent order, unknown verdict: refused
    assert!(poleat::check_answers(&jsonl(&q), &jsonl(&good[..2])).is_err());
    let mut r = good.clone();
    r.swap(0, 1);
    assert!(poleat::check_answers(&jsonl(&q), &jsonl(&r)).is_err());
    let mut r = good.clone();
    r[1]["relation"]["relation_digest"] = json!("other");
    assert!(poleat::check_answers(&jsonl(&q), &jsonl(&r)).is_err());
    let mut r = good.clone();
    r[1]["pole_order"] = json!(0);
    assert!(poleat::check_answers(&jsonl(&q), &jsonl(&r)).is_err());
    let mut r = good.clone();
    r[2]["verdict"] = json!("singular");
    assert!(poleat::check_answers(&jsonl(&q), &jsonl(&r)).is_err());
}

#[test]
fn sealed_checked_answers_are_row_37_diagnostic_records() {
    let q = requests(2);
    let j = job("pole-37", &q);
    seal_and_outcome(
        &j,
        &[
            answer(&q[0], "regular", json!(0)),
            answer(&q[1], "pole", json!(1)),
        ],
        true,
    );
    let c = classify_job(&j, WaitStatus::Exited(0));
    assert_eq!((c.row, c.accept), (37, Accept::DiagnosticRecords), "{c:?}");
}

#[test]
fn unreadable_answers_are_row_38_never_attention() {
    // answers not sealed (the wrapper refused them)
    let q = requests(2);
    let j = job("pole-38a", &q);
    seal_and_outcome(&j, &[], false);
    let c = classify_job(&j, WaitStatus::Exited(0));
    assert_eq!(
        (c.row, c.action),
        (38, Action::DiagnosticsUndecided),
        "{c:?}"
    );
    // sealed, but not the requests' answers (a missing answer): S fails -> 38
    let j = job("pole-38b", &q);
    seal_and_outcome(&j, &[answer(&q[0], "regular", json!(0))], true);
    let c = classify_job(&j, WaitStatus::Exited(0));
    assert_eq!(c.row, 38, "{c:?}");
    // the pinned requests changed after the spec: S fails -> 38
    let j = job("pole-38c", &q);
    seal_and_outcome(
        &j,
        &[
            answer(&q[0], "regular", json!(0)),
            answer(&q[1], "regular", json!(0)),
        ],
        true,
    );
    std::fs::write(j.layout.in_dir().join("requests.jsonl"), b"{}\n").unwrap();
    let c = classify_job(&j, WaitStatus::Exited(0));
    assert_eq!(c.row, 38, "{c:?}");
}

#[test]
fn a_refused_input_is_row_39_and_a_signal_is_as_for_every_kind() {
    let q = requests(1);
    let j = job("pole-39", &q);
    let mut o = JobOutcome::for_spec(
        &j.spec,
        &j.digest,
        program(),
        ExitCode::Input,
        StopInfo {
            code: Some(StopCode::InputError),
            ..StopInfo::none()
        },
    );
    o.error = Some(ErrorInfo::new(
        ErrorClass::Input,
        format!("{}: requests input digest differs", poleat::INPUT_MISMATCH),
    ));
    Outcome::new(o).write_new(&j.layout.outcome()).unwrap();
    let c = classify_job(&j, WaitStatus::Exited(41));
    assert_eq!(
        (c.row, c.action),
        (39, Action::DiagnosticsUndecided),
        "{c:?}"
    );
    // a signal is classified as for every kind (no OUTCOME, killed by the supervisor's stop)
    let j = job("pole-sig", &q);
    let ev = classify::assess(
        &j.layout,
        &j.spec,
        &j.digest,
        WaitStatus::Signaled(15),
        KillReason::Stop,
        false,
    );
    assert_eq!(classify::classify(&ev).row, 20);
}
