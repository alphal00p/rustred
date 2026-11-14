//! Kinds solve-recipe and proof-instantiate (lane adapters, interfaces/adapters.md 1): spec rules (input roles, args
//! allow-list, kind constraints, the fake-child rule), the per-key outcomes of a results file (every acceptance and
//! rejection rule of `recipe::key_outcomes`), and the decision rows 1 / 28 / 29 / 22 of the kind. Pure: no process.

mod common;
use common::*;
use rustred_campaign_contract::ExitCode;
use rustred_campaign_contract::classify::recipe::{
    self, FailureClass, KeyLine, KeyOutcome, KeyRoute, RelationTally,
};
use rustred_campaign_contract::classify::*;
use rustred_campaign_contract::spec::*;
use rustred_campaign_contract::stop::StopCode;
use serde_json::{Value, json};

fn sr_spec() -> JobSpec {
    let mut s = sample_spec("/r");
    s.kind = JobKind::SolveRecipe;
    s.program.features_required = vec![];
    s.budgets = Budgets::default();
    s.args = vec!["--order".into(), "id".into(), "--threads".into(), "4".into()];
    s.inputs = ["family", "recipe", "keys"]
        .iter()
        .map(|r| InputRef {
            role: (*r).into(),
            path: format!("/abs/{r}"),
            blake3: D1.into(),
            upto: None,
        })
        .collect();
    s
}

fn field(e: SpecError) -> String {
    match e {
        SpecError::BadField { field, .. } => field.to_string(),
        other => format!("{other:?}"),
    }
}

#[test]
fn solve_recipe_spec_rules() {
    let s = sr_spec();
    s.validate().unwrap();
    assert_eq!(JobKind::SolveRecipe.level(), Level::L1);
    assert_eq!(JobKind::SolveRecipe.program(), "rr_node");
    assert!(!JobKind::SolveRecipe.is_checker() && !JobKind::SolveRecipe.produces_table());
    assert_eq!(
        serde_json::to_string(&JobKind::SolveRecipe).unwrap(),
        "\"solve-recipe\""
    );
    // allow-list: the four flags, everything else 40
    let mut x = s.clone();
    x.args.extend(["--max-trace-rows".into(), "9".into(), "--key-budget-s".into(), "30".into()]);
    x.validate().unwrap();
    for bad in [
        vec!["--expect", "x"],
        vec!["--no-proof"],
        vec!["--keys", "a"],
        vec!["--max-trace-rows=9"],
        vec!["--threads", "four"],
    ] {
        let mut y = s.clone();
        y.args.extend(bad.iter().map(|a| a.to_string()));
        let e = y.validate().unwrap_err();
        assert_eq!(e.exit_code(), ExitCode::Usage, "{bad:?}: {e}");
    }
    // input roles: exactly family / recipe / keys, expect at most once, nothing else
    let mut y = s.clone();
    y.inputs.pop();
    assert_eq!(field(y.validate().unwrap_err()), "inputs");
    let mut y = s.clone();
    y.inputs.push(y.inputs[0].clone());
    assert_eq!(field(y.validate().unwrap_err()), "inputs");
    let mut y = s.clone();
    let mut e = y.inputs[0].clone();
    e.role = "expect".into();
    y.inputs.push(e.clone());
    y.validate().unwrap();
    e.role = "cases".into();
    y.inputs.push(e);
    assert_eq!(field(y.validate().unwrap_err()), "inputs");
    // kind constraints
    let mut y = s.clone();
    y.budgets.work.trace_rows_max = Some(10);
    assert_eq!(field(y.validate().unwrap_err()), "budgets.work");
    let mut y = s.clone();
    y.budgets.wall.job_s = Some(10);
    assert_eq!(field(y.validate().unwrap_err()), "budgets.wall.job_s");
    let mut y = s.clone();
    y.budgets.mem_soft_bytes = Some(10);
    assert_eq!(field(y.validate().unwrap_err()), "budgets.mem_soft_bytes");
    let mut y = s.clone();
    y.budgets.wall.case_s = Some(30);
    y.validate().unwrap();
    y.args.extend(["--key-budget-s".into(), "30".into()]);
    assert_eq!(field(y.validate().unwrap_err()), "budgets.wall.case_s");
    let mut y = s.clone();
    y.seed.sample_seed = 1;
    assert_eq!(field(y.validate().unwrap_err()), "seed.sample_seed");
    let mut y = s.clone();
    y.resume = Some(rustred_campaign_contract::resume::ResumeRef {
        from: "/x/RESUME.json".into(),
        blake3: D0.into(),
    });
    assert_eq!(field(y.validate().unwrap_err()), "resume");
    let mut y = s.clone();
    y.on_case_budget = OnCaseBudget::Open;
    assert_eq!(field(y.validate().unwrap_err()), "on_case_budget");
    let mut y = s.clone();
    y.determinism = Determinism::Strict;
    y.validate().unwrap();
    y.budgets.wall.case_s = Some(3);
    assert_eq!(field(y.validate().unwrap_err()), "determinism");
    let mut y = s.clone();
    y.args = vec!["--threads".into(), "1".into()];
    assert_eq!(field(y.validate().unwrap_err()), "args");
}

#[test]
fn a_fake_script_on_solve_recipe_only_for_the_fake_program() {
    let mut s = sr_spec();
    s.fake = Some(rustred_campaign_contract::fake::FakeScript {
        script: vec![rustred_campaign_contract::fake::FakeOp::End {
            stop: None,
            verdict: None,
        }],
        on_sigterm: Default::default(),
        slow_ms: 0,
        start_delay_ms: 0,
        rss: Default::default(),
        helpers: vec![],
        faults: vec![],
        time_scale: 1.0,
        volatile_jitter: false,
    });
    assert!(matches!(s.validate(), Err(SpecError::FakeMismatch(_))));
    s.program.name = FAKE_PROGRAM.into();
    s.validate().unwrap();
    s.kind = JobKind::Generate;
    s.args = vec!["--order".into(), "id".into()];
    assert!(matches!(s.validate(), Err(SpecError::FakeMismatch(_))));
}

#[test]
fn proof_instantiate_spec_rules() {
    let mut s = sample_spec("/r");
    s.kind = JobKind::ProofInstantiate;
    s.program.name = "rr_proof".into();
    s.program.features_required = vec![];
    s.budgets = Budgets::default();
    s.args = vec![];
    s.inputs = ["requests", "table"]
        .iter()
        .map(|r| InputRef {
            role: (*r).into(),
            path: format!("/abs/{r}"),
            blake3: D1.into(),
            upto: None,
        })
        .collect();
    s.validate().unwrap();
    assert_eq!(JobKind::ProofInstantiate.level(), Level::Legacy);
    let mut y = s.clone();
    y.args = vec!["--rule".into(), "1".into()];
    assert!(y.validate().is_err());
    let mut y = s.clone();
    y.budgets.wall.case_s = Some(1);
    assert!(y.validate().is_err());
}

#[test]
fn generate_allows_screen() {
    let mut s = sample_spec("/r");
    s.args.extend(["--screen".into(), "40,8,24,2000".into()]);
    s.validate().unwrap();
    s.args.extend(["--screen".into(), "1,2,3,4".into()]);
    assert!(s.validate().is_err(), "not repeatable");
}

// ------------------------------------------------------------------------------------------------ key outcomes

const DET: &str = "det-1";

fn solved(i: u64, key: &[i64]) -> Value {
    let mut lower = key.to_vec();
    lower[0] -= 1;
    let t = |k: &[i64]| {
        format!(
            "I({})",
            k.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")
        )
    };
    json!({"type":"relation","ordinal":i,"key":key,"tag":null,"verdict":"SOLVED","determinism_id":DET,
        "recipe_digest":D0,"rhs":[[t(&lower),"1","top"]],"relation_digest":D1,"all_lower":true,
        "proof":{"kind":"exact-weights","ok":true,"weights":3},"failure":null})
}

fn unsolved(i: u64, key: &[i64], verdict: &str) -> Value {
    json!({"type":"relation","ordinal":i,"key":key,"tag":null,"verdict":verdict,"determinism_id":DET,
        "recipe_digest":D0,"rhs":null,"relation_digest":null,
        "failure":{"schema":"rustred.recipe-failure.v1","class":verdict.to_lowercase(),"message":"m","detail":{}}})
}

fn end(ty: &str, recs: &[Value]) -> Value {
    let mut counts = serde_json::Map::new();
    for r in recs {
        let v = r["verdict"].as_str().unwrap().to_string();
        let n = counts.get(&v).and_then(Value::as_u64).unwrap_or(0) + 1;
        counts.insert(v, json!(n));
    }
    json!({"type":ty,"records":recs.len(),"counts":counts,"recipe_digest":D0,"determinism_id":DET})
}

fn lines(recs: &[Value], end_rec: Value) -> Vec<Vec<u8>> {
    recs.iter()
        .chain(std::iter::once(&end_rec))
        .map(|v| rustred_campaign_contract::canon::canonical_line(v.to_string().as_bytes()).unwrap())
        .collect()
}

fn keys(ks: &[&[i64]]) -> Vec<KeyLine> {
    ks.iter()
        .map(|k| KeyLine {
            key: k.to_vec(),
            tag: None,
        })
        .collect()
}

#[test]
fn key_outcomes_accept_proved_records_and_type_the_others() {
    let recs = vec![
        solved(0, &[2, 1, 0]),
        unsolved(1, &[3, 1, 0], "NOT_IN_DOMAIN"),
        unsolved(2, &[4, 1, 0], "BUDGET_WALL"),
    ];
    let (out, t) = recipe::key_outcomes(
        &lines(&recs, end("end", &recs)),
        &keys(&[&[2, 1, 0], &[3, 1, 0], &[4, 1, 0]]),
    )
    .unwrap();
    assert_eq!(t.records, 3);
    assert_eq!(t.accepted, 1);
    assert_eq!(t.unsolved_total(), 2);
    assert_eq!(t.time_decided_keys(), 1);
    match &out[0] {
        KeyOutcome::Accepted(a) => {
            assert_eq!(a.rhs_keys, vec![vec![1, 1, 0]]);
            assert_eq!(a.recipe_digest, D0);
        }
        o => panic!("{o:?}"),
    }
    match &out[1] {
        KeyOutcome::Unsolved(u) => {
            assert_eq!(u.class, FailureClass::NotInDomain);
            assert_eq!(u.route, KeyRoute::NextRecipeRung);
        }
        o => panic!("{o:?}"),
    }
}

#[test]
fn key_outcomes_refuse_every_inconsistency() {
    let k = keys(&[&[2, 1], &[3, 1]]);
    let good = vec![solved(0, &[2, 1]), unsolved(1, &[3, 1], "IRREGULAR")];
    assert!(recipe::key_outcomes(&lines(&good, end("end", &good)), &k).is_ok());
    let mut cases: Vec<(&str, Vec<Vec<u8>>)> = vec![];
    let mutate = |f: &dyn Fn(&mut Vec<Value>)| {
        let mut r = good.clone();
        f(&mut r);
        lines(&r, end("end", &r))
    };
    cases.push(("unproved", mutate(&|r| r[0]["proof"]["ok"] = json!(false))));
    cases.push(("not lower", mutate(&|r| r[0]["all_lower"] = json!(false))));
    cases.push(("bad digest", mutate(&|r| r[0]["relation_digest"] = json!("abc"))));
    cases.push(("failure on solved", mutate(&|r| r[0]["failure"] = json!({}))));
    cases.push(("wrong key", mutate(&|r| r[0]["key"] = json!([9, 1]))));
    cases.push(("wrong ordinal", mutate(&|r| r[1]["ordinal"] = json!(5))));
    cases.push(("unknown verdict", mutate(&|r| r[1]["verdict"] = json!("MAYBE"))));
    cases.push(("class mismatch", mutate(&|r| r[1]["failure"]["class"] = json!("error"))));
    cases.push(("two recipe digests", mutate(&|r| r[1]["recipe_digest"] = json!(D1))));
    cases.push(("two determinism ids", mutate(&|r| r[1]["determinism_id"] = json!("x"))));
    cases.push(("rhs arity", mutate(&|r| r[0]["rhs"] = json!([["I(1)", "1", "top"]]))));
    cases.push((
        "interrupted in end",
        mutate(&|r| r[1] = unsolved(1, &[3, 1], "INTERRUPTED")),
    ));
    cases.push((
        "unproved verdict",
        mutate(&|r| r[0]["verdict"] = json!("SOLVED_UNPROVED")),
    ));
    let mut l = lines(&good, end("end", &good));
    l.remove(1);
    cases.push(("missing record", l));
    let mut e = end("end", &good);
    e["records"] = json!(5);
    cases.push(("end records", lines(&good, e)));
    let mut e = end("end", &good);
    e["counts"] = json!({"SOLVED": 2});
    cases.push(("end counts", lines(&good, e)));
    cases.push(("no end", lines(&good[..1], good[1].clone())));
    for (what, l) in cases {
        assert!(recipe::key_outcomes(&l, &k).is_err(), "{what} accepted");
    }
    // a `partial` file may hold INTERRUPTED records (never accepted: exit 30 rows need no S)
    let p = vec![solved(0, &[2, 1]), unsolved(1, &[3, 1], "INTERRUPTED")];
    assert!(recipe::key_outcomes(&lines(&p, end("partial", &p)), &k).is_ok());
}

// ------------------------------------------------------------------------------------------------ rows

fn ev(code: ExitCode, stop: Option<StopCode>, tally: Option<(u64, u64)>) -> Evidence {
    let s = sr_spec();
    let mut j = job_outcome(&s, code, stop, false);
    j.outputs.table = None;
    let relations = tally.map(|(a, u)| {
        let mut t = RelationTally {
            records: a + u,
            accepted: a,
            ..Default::default()
        };
        if u > 0 {
            t.unsolved.insert(FailureClass::NoRelationInSeeds, u);
        }
        t
    });
    Evidence {
        kind: JobKind::SolveRecipe,
        wait: WaitStatus::Exited(code.code()),
        outcome: OutcomeState::Valid(Box::new(j)),
        verified: Some(VerifiedOutputs {
            rules: 0,
            elided_loci: 0,
            streams: vec![],
            relations,
        }),
        unsealed_why: None,
        resume_valid: false,
        resume_ref: None,
        kill: KillReason::None,
        oom_kill_observed: false,
        elision_forbidden: true,
    }
}

#[test]
fn rows_of_solve_recipe() {
    let r = |e: Evidence| {
        let c = classify(&e);
        (c.row, c.accept, c.action)
    };
    let ks = Some(StopCode::KeysUnsolved);
    assert_eq!(r(ev(ExitCode::Complete, None, Some((3, 0)))).0, 1);
    assert_eq!(r(ev(ExitCode::Complete, None, Some((2, 1)))).0, 22);
    assert_eq!(
        r(ev(ExitCode::Incomplete, ks, Some((2, 1)))),
        (28, Accept::SolvedRecords, Action::QueueFailures)
    );
    assert_eq!(
        r(ev(ExitCode::Incomplete, ks, Some((0, 2)))),
        (29, Accept::Nothing, Action::QueueFailures)
    );
    assert_eq!(r(ev(ExitCode::Incomplete, ks, Some((2, 0)))).0, 22);
    assert_eq!(r(ev(ExitCode::Incomplete, ks, None)).0, 22, "S needs the tally");
    assert_eq!(
        r(ev(ExitCode::Incomplete, Some(StopCode::MaxCases), Some((1, 1)))).0,
        22
    );
    assert_eq!(
        r(ev(ExitCode::BudgetWork, Some(StopCode::BudgetWork), Some((1, 0)))).0,
        22
    );
    // keys_unsolved of another kind
    let mut e = ev(ExitCode::Incomplete, ks, Some((1, 1)));
    e.kind = JobKind::Generate;
    if let OutcomeState::Valid(j) = &mut e.outcome {
        j.kind = JobKind::Generate;
    }
    assert_eq!(classify(&e).row, 22);
    // proof-instantiate: exit 0 only
    let mut e = ev(ExitCode::Incomplete, Some(StopCode::MaxCases), None);
    e.kind = JobKind::ProofInstantiate;
    assert_eq!(classify(&e).row, 22);
    // interrupted: row 11, nothing accepted
    let mut e = ev(ExitCode::Interrupted, Some(StopCode::Interrupted), None);
    e.verified = None;
    assert_eq!(classify(&e).row, 11);
}
