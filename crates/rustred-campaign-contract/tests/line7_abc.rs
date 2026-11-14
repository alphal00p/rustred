//! Line 7, second part (lane contract-line7, 2026-11-14; child-job.md "Line 7" A / B / C): seed minimisation with the
//! centre scan (coord-seed-minimise.md 6, replacing section 5's single centre), multi-term symmetry rows
//! (coord-pole-engine.md 9), typed reader errors of the legacy proof tools (decision row 13b), and the `symface` slice
//! rules as ordinary rule records. Writes temporary tables under TMPDIR; spawns nothing.

mod common;
use common::*;
use rustred_campaign_contract::ExitCode;
use rustred_campaign_contract::arith::{CentreEntry, parse_centre_scan};
use rustred_campaign_contract::canon::canonical_line;
use rustred_campaign_contract::classify::recipe::{self, FailureClass, KeyLine, KeyRoute, Verdict};
use rustred_campaign_contract::classify::{self, *};
use rustred_campaign_contract::instantiate;
use rustred_campaign_contract::outcome::{ErrorClass, ErrorInfo};
use rustred_campaign_contract::seal::{RecordWriter, TABLE_END_TYPES, verify_stream};
use rustred_campaign_contract::spec::*;
use rustred_campaign_contract::stop::StopCode;
use serde_json::{Value, json};

fn gen_spec(extra: &[&str]) -> JobSpec {
    let mut s = sample_spec("/r");
    s.args.extend(extra.iter().map(|a| a.to_string()));
    s
}

fn usage(s: &JobSpec) -> String {
    let e = s.validate().unwrap_err();
    assert_eq!(e.exit_code(), ExitCode::Usage, "{e}");
    e.to_string()
}

// ------------------------------------------------------------------------------------------------ A: arguments

#[test]
fn centre_scan_text_form() {
    assert_eq!(
        parse_centre_scan("floor_of_free,each_free_slot").unwrap(),
        vec![CentreEntry::FloorOfFree, CentreEntry::EachFreeSlot]
    );
    // section 7: the default scan starts with the recipe's own centres
    assert_eq!(
        parse_centre_scan("recipe,floor_of_free,each_free_slot").unwrap(),
        vec![
            CentreEntry::Recipe,
            CentreEntry::FloorOfFree,
            CentreEntry::EachFreeSlot
        ]
    );
    assert_eq!(
        parse_centre_scan("(0;-1;0;12),floor_of_free,(1;0;0;0)").unwrap(),
        vec![
            CentreEntry::Offset(vec![0, -1, 0, 12]),
            CentreEntry::FloorOfFree,
            CentreEntry::Offset(vec![1, 0, 0, 0])
        ]
    );
    for bad in [
        "",
        "floor_of_free,",
        ",each_free_slot",
        "floor_of_free,floor_of_free",
        "(0;-1),(0;-1)",
        "(0;-1),(0)",
        "()",
        "(0;+1)",
        "(0;01)",
        "(0;-0)",
        "(0, -1)",
        "(0,-1)",
        "(0;-1",
        "0;-1",
        "[0,-1]",
        "floor",
        "Floor_of_free",
        " floor_of_free",
        "each_free_slot ",
        "recipe,recipe",
        "Recipe",
    ] {
        assert!(parse_centre_scan(bad).is_err(), "{bad:?} accepted");
    }
}

#[test]
fn generate_seed_minimisation_arguments() {
    gen_spec(&["--centre-scan", "floor_of_free,each_free_slot"])
        .validate()
        .unwrap();
    gen_spec(&["--centre-scan", "(0;0;-1;0)"])
        .validate()
        .unwrap();
    gen_spec(&["--minimise-rules", "support"])
        .validate()
        .unwrap();
    gen_spec(&["--minimise-rules", "greedy", "--minimise-max-solves", "40"])
        .validate()
        .unwrap();
    gen_spec(&[
        "--centre-scan",
        "each_free_slot",
        "--minimise-rules",
        "support",
        "--minimise-max-solves",
        "3",
    ])
    .validate()
    .unwrap();
    // section 7: the minimisation target (a case domain as compact JSON)
    let t = r#"{"fixed":[null,-1,null],"equations":["n0-n2"],"bounds":{"min":[2,null,null],"max":[12,null,null]}}"#;
    gen_spec(&["--minimise-rules", "support", "--minimise-target", t])
        .validate()
        .unwrap();
    gen_spec(&[
        "--minimise-rules",
        "greedy",
        "--minimise-target",
        r#"{"equations":["n1+1"]}"#,
    ])
    .validate()
    .unwrap();
    assert!(usage(&gen_spec(&["--minimise-target", t])).contains("needs --minimise-rules"));
    for bad in [
        "{}",
        "[]",
        "fixed",
        r#"{"fixed":[]}"#,
        r#"{"fixed":[1.5]}"#,
        r#"{"equations":[""]}"#,
        r#"{"equations":"n1"}"#,
        r#"{"bounds":{"min":[1]},"fixed":[null,null]}"#,
        r#"{"bounds":{"low":[1]}}"#,
        r#"{"fixed":[null],"slice":1}"#,
    ] {
        usage(&gen_spec(&[
            "--minimise-rules",
            "support",
            "--minimise-target",
            bad,
        ]));
    }
    // section 6 replaced section 5's single centre: `--seed-centre` does not exist
    usage(&gen_spec(&["--seed-centre", "floor_of_free"]));
    usage(&gen_spec(&["--centre-scan", "floor"]));
    usage(&gen_spec(&["--centre-scan", "(0;-1),(0)"]));
    usage(&gen_spec(&["--minimise-rules", "fast"]));
    assert!(usage(&gen_spec(&["--minimise-max-solves", "4"])).contains("needs --minimise-rules"));
    usage(&gen_spec(&[
        "--minimise-rules",
        "support",
        "--minimise-max-solves",
        "0",
    ]));
    usage(&gen_spec(&[
        "--minimise-rules",
        "support",
        "--minimise-max-solves",
        "x",
    ]));
    // generate only
    let mut s = sample_spec("/r");
    s.kind = JobKind::SolveRecipe;
    s.args = vec![
        "--order".into(),
        "id".into(),
        "--minimise-rules".into(),
        "support".into(),
    ];
    assert!(s.validate().is_err());
}

// ------------------------------------------------------------------------------------------------ A / B: records

const DET: &str = "det-1";

fn solved(i: u64, key: &[i64]) -> Value {
    let mut lower = key.to_vec();
    lower[0] -= 1;
    let t = format!(
        "I({})",
        lower
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    json!({"type":"relation","ordinal":i,"key":key,"tag":null,"verdict":"SOLVED","determinism_id":DET,
        "recipe_digest":D0,"rhs":[[t,"1","top"]],"relation_digest":D1,"all_lower":true,
        "proof":{"kind":"exact-weights","ok":true,"weights":3},"failure":null})
}

fn unsolved(i: u64, key: &[i64], verdict: &str) -> Value {
    json!({"type":"relation","ordinal":i,"key":key,"tag":null,"verdict":verdict,"determinism_id":DET,
        "recipe_digest":D0,"rhs":null,"relation_digest":null,
        "failure":{"schema":"rustred.recipe-failure.v1","class":verdict.to_lowercase(),"message":"m","detail":{}}})
}

fn file(recs: &[Value]) -> Vec<Vec<u8>> {
    let mut counts = serde_json::Map::new();
    for r in recs {
        let v = r["verdict"].as_str().unwrap().to_string();
        let n = counts.get(&v).and_then(Value::as_u64).unwrap_or(0) + 1;
        counts.insert(v, json!(n));
    }
    let end = json!({"type":"end","records":recs.len(),"counts":counts,"recipe_digest":D0,"determinism_id":DET});
    recs.iter()
        .chain(std::iter::once(&end))
        .map(|v| canonical_line(v.to_string().as_bytes()).unwrap())
        .collect()
}

fn keys(n: usize) -> Vec<KeyLine> {
    (0..n)
        .map(|i| KeyLine {
            key: vec![i as i64 + 2, 1, 0],
            tag: None,
        })
        .collect()
}

/// A named change to a record.
type Mutation = fn(&mut Value);

fn minimise() -> Value {
    json!({"mode":"support","measure":"comparator","order":"outermost_first","solves":7,"stopped":"done","seeds_before":21,"seeds_after":9,
        "rhs_before":1015,"rhs_after":269,"original_relation_digest":D0})
}

fn centre_scan() -> Value {
    json!({"entries":[
            {"centre":"floor_of_free","verdict":"NO_RELATION_IN_SEEDS","rhs_before":null,"rhs_after":null,"solves":1,"regular_at_d0":null},
            {"centre":[0,0,-1],"verdict":"SOLVED","rhs_before":1015,"rhs_after":269,"solves":8,"regular_at_d0":true}],
        "chosen":1,"reason":"regular","size_ratio":1.0})
}

fn accepts(r: Value) -> bool {
    recipe::key_outcomes(&file(&[r]), &keys(1)).is_ok()
}

#[test]
fn minimise_and_centre_scan_members() {
    let mut a = solved(0, &[2, 1, 0]);
    a["minimise"] = minimise();
    a["centre_scan"] = centre_scan();
    assert!(accepts(a.clone()));
    // non-volatile: part of the canonical record
    let c = String::from_utf8(canonical_line(a.to_string().as_bytes()).unwrap()).unwrap();
    assert!(
        c.contains("\"minimise\"") && c.contains("\"centre_scan\"") && c.contains("size_ratio")
    );
    let bad: Vec<(&str, Mutation)> = vec![
        ("mode", |r| r["minimise"]["mode"] = json!("fast")),
        ("order", |r| {
            r["minimise"]["order"] = json!("innermost_first")
        }),
        ("stopped", |r| r["minimise"]["stopped"] = json!("wall")),
        ("measure", |r| r["minimise"]["measure"] = json!("degree")),
        ("measure missing", |r| {
            r["minimise"].as_object_mut().unwrap().remove("measure");
        }),
        ("target_domain member", |r| {
            r["minimise"]["target_domain"] = json!({"slice": 1})
        }),
        ("target digest without a target", |r| {
            r["minimise"]["target_domain_blake3"] = json!(D1)
        }),
        ("target digest not hex", |r| {
            r["minimise"]["target_domain"] = json!({"fixed": [null, -1, null]});
            r["minimise"]["target_domain_blake3"] = json!("x")
        }),
        ("seeds grew", |r| r["minimise"]["seeds_after"] = json!(22)),
        ("max_solves after 0 solves", |r| {
            r["minimise"]["stopped"] = json!("max_solves");
            r["minimise"]["solves"] = json!(0)
        }),
        ("digest", |r| {
            r["minimise"]["original_relation_digest"] = json!("x")
        }),
        ("rhs_after a float", |r| {
            r["minimise"]["rhs_after"] = json!(2.5)
        }),
        ("no entries", |r| r["centre_scan"]["entries"] = json!([])),
        ("chosen out of range", |r| {
            r["centre_scan"]["chosen"] = json!(2)
        }),
        ("chosen did not solve", |r| {
            r["centre_scan"]["chosen"] = json!(0)
        }),
        ("reason", |r| r["centre_scan"]["reason"] = json!("first")),
        ("size_ratio negative", |r| {
            r["centre_scan"]["size_ratio"] = json!(-1.0)
        }),
        ("centre token", |r| {
            r["centre_scan"]["entries"][0]["centre"] = json!("floor")
        }),
        ("centre vector", |r| {
            r["centre_scan"]["entries"][0]["centre"] = json!([0.5])
        }),
        ("entry verdict", |r| {
            r["centre_scan"]["entries"][0]["verdict"] = json!("NOPE")
        }),
        ("regular_at_d0", |r| {
            r["centre_scan"]["entries"][1]["regular_at_d0"] = json!("yes")
        }),
    ];
    for (what, f) in bad {
        let mut r = a.clone();
        f(&mut r);
        assert!(!accepts(r), "{what} accepted");
    }
    // measure m1, a target domain with its digest, the recipe centre token
    let mut r = a.clone();
    r["minimise"]["measure"] = json!("m1");
    r["minimise"]["target_domain"] = json!({"fixed": [null, -1, null], "equations": ["n0-n2"],
        "bounds": {"min": [2, null, null], "max": [12, null, null]}});
    r["minimise"]["target_domain_blake3"] = json!(D1);
    r["centre_scan"]["entries"][0]["centre"] = json!("recipe");
    assert!(accepts(r));
    // size_ratio null and a size_guard choice are fine
    let mut r = a.clone();
    r["centre_scan"]["reason"] = json!("size_guard");
    r["centre_scan"]["size_ratio"] = json!(null);
    assert!(accepts(r));
}

#[test]
fn rule_records_with_minimise_in_a_table() {
    let d = TempDir::new("minrule");
    let path = d.path().join("F.table.jsonl");
    let write = |rule: Value| {
        let _ = std::fs::remove_file(&path);
        let mut w = RecordWriter::create(&path).unwrap();
        w.append(rule.to_string().as_bytes()).unwrap();
        w.seal(br#"{"type":"end","rules":1}"#).unwrap();
        verify_stream(d.path(), "F.table.jsonl", &TABLE_END_TYPES)
    };
    let mut rule = json!({"type":"rule","chart":"111","index":0,"case":"c","target":"I(1,1,1)","rhs":[["I(0,1,1)","1"]],
        "seconds_search":1.0});
    rule["minimise"] = minimise();
    rule["centre_scan"] = centre_scan();
    assert!(write(rule.clone()).is_ok());
    let mut bad = rule.clone();
    bad["minimise"]["mode"] = json!("fast");
    assert!(write(bad).is_err());
    let mut bad = rule.clone();
    bad["centre_scan"]["chosen"] = json!(9);
    assert!(write(bad).is_err());
}

#[test]
fn multi_term_symmetry_rows() {
    assert_eq!(
        Verdict::SymmetryFormCheckFailed.name(),
        "SYMMETRY_FORM_CHECK_FAILED"
    );
    assert_eq!(
        FailureClass::SymmetryFormCheckFailed.name(),
        "symmetry_form_check_failed"
    );
    assert_eq!(
        FailureClass::SymmetryFormCheckFailed.route(),
        KeyRoute::NextRecipeRung
    );
    assert!(!FailureClass::SymmetryFormCheckFailed.time_decided());
    let mut a = solved(0, &[2, 1, 0]);
    a["symmetry_rows_used"] = json!({"elements":3,"rows":3,"terms":41,"position":"key_first"});
    a["proof"]["symmetry"] = json!({"rows":3,"form_check":"exact","group_order":8});
    assert!(accepts(a.clone()));
    let c = String::from_utf8(canonical_line(a.to_string().as_bytes()).unwrap()).unwrap();
    assert!(c.contains("symmetry_rows_used") && c.contains("group_order"));
    let mut r = a.clone();
    r["proof"].as_object_mut().unwrap().remove("symmetry");
    assert!(!accepts(r), "SOLVED with symmetry rows, no proof block");
    let mut r = a.clone();
    r["proof"]["symmetry"]["form_check"] = json!("sampled");
    assert!(!accepts(r));
    let mut r = a.clone();
    r["symmetry_rows_used"]["position"] = json!("first");
    assert!(!accepts(r));
    // after_seed with zero rows used needs no proof block
    let mut r = solved(0, &[2, 1, 0]);
    r["symmetry_rows_used"] = json!({"elements":2,"rows":0,"terms":0,"position":"after_seed"});
    assert!(accepts(r));
    // 9d: relations from symmetry rows only (`seeds.ibp_rows: false` in the recipe) are ordinary relation records
    let mut r = a.clone();
    r["symmetry_rows_used"]["position"] = json!("after_seed");
    r["ibp_rows"] = json!(0);
    let (out, t) = recipe::key_outcomes(&file(&[r.clone()]), &keys(1)).unwrap();
    assert_eq!(t.accepted, 1);
    assert!(matches!(out[0], recipe::KeyOutcome::Accepted(_)));
    let mut x = r.clone();
    x["symmetry_rows_used"]["rows"] = json!(0);
    assert!(!accepts(x), "SOLVED from no rows at all");
    let mut x = r.clone();
    x["ibp_rows"] = json!(-1);
    assert!(!accepts(x));
    let mut x = solved(0, &[2, 1, 0]);
    x["ibp_rows"] = json!(120);
    assert!(accepts(x));
    // the typed failure: next escalation step
    let f = unsolved(0, &[2, 1, 0], "SYMMETRY_FORM_CHECK_FAILED");
    let (out, t) = recipe::key_outcomes(&file(&[f]), &keys(1)).unwrap();
    assert_eq!(
        t.unsolved.get(&FailureClass::SymmetryFormCheckFailed),
        Some(&1)
    );
    let recipe::KeyOutcome::Unsolved(u) = &out[0] else {
        panic!()
    };
    assert_eq!(u.route, KeyRoute::NextRecipeRung);
}

// ------------------------------------------------------------------------------------------------ C: reader errors

#[test]
fn reader_error_names() {
    use instantiate::reader_error_in;
    assert_eq!(
        reader_error_in("UNREADABLE_RECORD: line 947"),
        Some("UNREADABLE_RECORD")
    );
    assert_eq!(
        reader_error_in("rr_proof: error UNKNOWN_RECORD_KIND at offset 9"),
        Some("UNKNOWN_RECORD_KIND")
    );
    assert_eq!(reader_error_in("UNREADABLE_RECORDS"), None);
    assert_eq!(reader_error_in("X_UNREADABLE_RECORD"), None);
    assert_eq!(reader_error_in("unreadable record"), None);
    // a per-key answer carrying a reader error is refused (an input error of the job, never a per-key verdict)
    let mut v = json!({"schema":"rustred.proof-kernel.instantiate.v1","rule":0,"key":[1],"regular":"unknown",
        "error":"UNREADABLE_RECORD: record 0"});
    assert!(instantiate::instantiate_answer_use(&v, ElidedLoci::Refuse).is_err());
    v["error"] = json!("EMPTY_EXCEPTION_BRANCH");
    assert!(instantiate::instantiate_answer_use(&v, ElidedLoci::Refuse).is_ok());
}

fn legacy_evidence(kind: JobKind, code: ExitCode, msg: Option<&str>) -> Evidence {
    let mut s = sample_spec("/r");
    s.kind = kind;
    let stop = match code {
        ExitCode::Input => Some(StopCode::InputError),
        ExitCode::Internal => Some(StopCode::InternalError),
        _ => None,
    };
    let mut j = job_outcome(&s, code, stop, false);
    j.kind = kind;
    j.outputs.table = None;
    j.error = msg.map(|m| {
        let class = if code == ExitCode::Input {
            ErrorClass::Input
        } else {
            ErrorClass::Internal
        };
        ErrorInfo::new(class, m)
    });
    Evidence {
        kind,
        wait: WaitStatus::Exited(code.code()),
        outcome: OutcomeState::Valid(Box::new(j)),
        verified: None,
        unsealed_why: None,
        resume_valid: false,
        resume_ref: None,
        kill: KillReason::None,
        oom_kill_observed: false,
        elision_forbidden: true,
    }
}

#[test]
fn decision_row_13b() {
    let m = Some("UNKNOWN_RECORD_KIND: line 947 of the table");
    for kind in [JobKind::ProofInstantiate, JobKind::ProofInstanceSpan] {
        for code in [ExitCode::Input, ExitCode::Internal] {
            let c = classify::classify(&legacy_evidence(kind, code, m));
            assert_eq!(
                (c.row, c.sub.as_str(), c.kind, c.accept, c.action),
                (
                    13,
                    "b",
                    OutcomeKind::Refused,
                    Accept::Nothing,
                    Action::TableUnreadableByTool
                ),
                "{kind:?} {code:?}"
            );
            assert!(c.attention && !c.time_decided);
        }
        assert_eq!(
            classify::classify(&legacy_evidence(kind, ExitCode::Input, Some("bad key"))).row,
            13
        );
        assert_eq!(
            classify::classify(&legacy_evidence(kind, ExitCode::Input, None)).row,
            13
        );
        assert_eq!(
            classify::classify(&legacy_evidence(kind, ExitCode::Internal, None)).row,
            17
        );
    }
    // other kinds keep rows 13 / 17
    for kind in [
        JobKind::CoefficientFacts,
        JobKind::Generate,
        JobKind::SolveRecipe,
    ] {
        assert_eq!(
            classify::classify(&legacy_evidence(kind, ExitCode::Input, m)).row,
            13,
            "{kind:?}"
        );
    }
}

// ------------------------------------------------------------------------------------------------ symface slice rules

#[test]
fn symface_slice_rules_are_ordinary_rule_records() {
    // multi-term symmetry face rules as slice rules (one ordinary rule per value of the substituted power; slice bound
    // as a domain bound; provenance members): the table verifier has no closed member list for rule records
    let d = TempDir::new("symface");
    let path = d.path().join("F.table.jsonl");
    let mut w = RecordWriter::create(&path).unwrap();
    for (i, m) in [1, 2].iter().enumerate() {
        let r = json!({"type":"rule","chart":"111","index":i,"case":format!("symface:F7:m={m}"),
            "target":format!("I(2,1,-{m})"),"rhs":[["I(1,1,0)","3"]],"fixed":{"2":-m},"bounds":{"0":[2,12]},
            "provenance":{"source":"symmetry_row","element":6,"face":"F7"},"slice":{"slot":2,"value":-m,"bound":12}});
        w.append(r.to_string().as_bytes()).unwrap();
    }
    w.seal(br#"{"type":"end","rules":2}"#).unwrap();
    let v = verify_stream(d.path(), "F.table.jsonl", &TABLE_END_TYPES).unwrap();
    assert_eq!(v.rules, 2);
    // the one condition they must meet: (chart, target, case) unique per table (and (chart, index))
    let p2 = d.path().join("G.table.jsonl");
    let mut w = RecordWriter::create(&p2).unwrap();
    for i in 0..2 {
        let r = json!({"type":"rule","chart":"111","index":i,"case":"symface:F7","target":"I(2,1,-1)","rhs":[]});
        w.append(r.to_string().as_bytes()).unwrap();
    }
    w.seal(br#"{"type":"end","rules":2}"#).unwrap();
    assert!(verify_stream(d.path(), "G.table.jsonl", &TABLE_END_TYPES).is_err());
}
