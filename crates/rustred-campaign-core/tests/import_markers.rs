//! Lane import5 (interfaces/import.md 18.2 / 18.3): `composite_terminal_ledger` keys are DECLARED TERMINALS (closure,
//! statement, comparison); the completeness markers `composite_assembly_complete` / `assembly_end` and the provenance
//! record `assembly_provenance` are checked against the file (typed refusals). Constructed tables shaped like the real
//! composite assemblies (T270: ledger + marker, no residual records; T211: provenance + residuals + marker; T228:
//! assembly_end only), on the toy sunset (tests/import5_common).

mod import5_common;
use import5_common::*;

use import::{ImportError, RowStatus};
use rustred_campaign_core::import;
use serde_json::{Value, json};

const TOP: [i32; 5] = [2, 2, 1, 0, 0];
const T1: [i32; 5] = [1, 1, 1, 0, 0];
const T2: [i32; 5] = [2, 1, 1, 0, 0];

fn ledger(keys: &[&[i32]], count: Option<usize>) -> String {
    let mut v = json!({"type": "composite_terminal_ledger", "schema": "test.ledger.v1",
        "keys": keys.iter().map(|k| kt(k)).collect::<Vec<_>>(), "label": "terminal ledger (test)",
        "native_residual_bytes": false});
    if let Some(c) = count {
        v["count"] = json!(c);
    }
    // the real Astra tables write WITH SPACES (Python json.dumps): so does this test
    serde_json::to_string(&v)
        .unwrap()
        .replace("\":", "\": ")
        .replace(",\"", ", \"")
}

fn complete(rules: usize, residuals: usize, complete: bool) -> String {
    json!({"type": "composite_assembly_complete", "schema": "test.v1", "complete": complete, "rules": rules,
        "rhs_terms": 2, "residuals": residuals, "expected_LP": residuals})
    .to_string()
}

fn rules_two() -> Vec<String> {
    vec![
        point_rule(0, &TOP, &[&T1, &T2]),
        point_rule(1, &[1, 2, 1, 0, 0], &[&T1]),
    ]
}

/// T270 shape: two rule records, a ledger of 2 keys, a completion marker, NO residual records and NO end record. The
/// ledger keys are declared terminals with their provenance in the closure and the statement.
#[test]
fn ledger_keys_are_declared_terminals_t270_shape() {
    let d = scratch("ledger");
    let tail = vec![ledger(&[&T1, &T2], Some(2)), complete(2, 2, true)];
    let t = write_table_with(&d, "t.jsonl", &header_value(), &rules_two(), Some(&tail));
    // partfix4: a table without an end record loads only with the explicit, recorded override (the real T270
    // composite has none: its writer omits it); int7: the override names the list of such tables (path + sha256)
    let list = no_end_list(&d, "no-end.json", &[&t]);
    let sp = spec(
        &d,
        &t,
        &[&TOP],
        json!({"marker_rules": {"accept_missing_end": list}}),
        &[],
    );
    let (set, ev, r) = run(&sp, &d);
    let rep = &set.report.sources[0];
    assert_eq!(rep.table_terminals, 2);
    assert_eq!(set.report.terminals_from_tables, 2);
    assert_eq!(rep.markers.get("composite_terminal_ledger"), Some(&1));
    assert_eq!(rep.assembly_files.len(), 1);
    assert_eq!(row(&r, &T1).status, RowStatus::Terminal);
    assert_eq!(row(&r, &T2).status, RowStatus::Terminal);
    assert_eq!(row(&r, &TOP).status, RowStatus::Served);
    assert!(
        !r.blocking()
            .iter()
            .any(|b| b.starts_with("descent") || b.contains("open")),
        "{:?}",
        r.blocking()
    );
    let d1 = set.terminal_provenance.get(T1.as_slice()).unwrap();
    assert_eq!(d1.provenance, "composite_terminal_ledger");
    assert_eq!(d1.line, 5);
    // statement: the terminal row carries its provenance; terminals.json lists the table-declared terminals
    let out = d.join("st");
    import::write_statement(&set, &ev, &r, &out, None).unwrap();
    let rows: Vec<Value> = std::fs::read_to_string(out.join("statement.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let t1 = rows.iter().find(|v| v["key"] == json!(T1)).unwrap();
    assert_eq!(t1["status"], json!("terminal"));
    assert_eq!(
        t1["terminal"]["provenance"],
        json!("composite_terminal_ledger")
    );
    let top = rows.iter().find(|v| v["key"] == json!(TOP)).unwrap();
    assert!(top.get("terminal").is_none());
    let tj: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("terminals.json")).unwrap())
            .unwrap();
    assert_eq!(tj["declared_by_tables"].as_array().unwrap().len(), 2);
    // comparison: a hand statement naming the same terminals agrees
    let hand = d.join("hand.jsonl");
    std::fs::write(
        &hand,
        [
            json!([kt(&TOP), true, "rule", "A#0"]),
            json!([kt(&T1), false, "terminal", ""]),
            json!([kt(&T2), false, "terminal", ""]),
        ]
        .iter()
        .map(|v| format!("{v}\n"))
        .collect::<String>(),
    )
    .unwrap();
    let hs =
        import::read_hand_statement(&hand, &import::HandFormat::positional(0, 1, 2, 3), N).unwrap();
    let cmp = import::compare(&set, &r, &hs);
    assert!(
        cmp.terminals_equal,
        "{:?} vs {:?}",
        cmp.terminals_hand, cmp.terminals_ours
    );
}

/// `table_terminals`: `none` declares nothing from the table (the ledger keys are then uncovered); a key declared by
/// a `terminals` source keeps that declaration (no table provenance); `ledger_and_residuals` adds residual keys.
#[test]
fn table_terminals_policy() {
    let d = scratch("ledger-policy");
    let tail = vec![
        ledger(&[&T1], None),
        complete(2, 2, true),
        r#"{"type":"end","charts":1}"#.to_string(),
    ];
    let mut lines = rules_two();
    lines.push(json!({"type":"residual","chart":MASK,"key":kt(&T2)}).to_string());
    let t = write_table_with(&d, "t.jsonl", &header_value(), &lines, Some(&tail));
    // none
    let sp = spec(&d, &t, &[&TOP], json!({"table_terminals":"none"}), &[]);
    let (set, _ev, r) = run(&sp, &d);
    assert_eq!(set.report.sources[0].table_terminals, 0);
    assert_eq!(row(&r, &T1).status, RowStatus::Open);
    // default ledger: T1 terminal, T2 (residual record) open
    let sp = spec(&d, &t, &[&TOP], json!({}), &[]);
    let (_set, _ev, r) = run(&sp, &d);
    assert_eq!(row(&r, &T1).status, RowStatus::Terminal);
    assert_eq!(row(&r, &T2).status, RowStatus::Open);
    // ledger_and_residuals: both
    let sp = spec(
        &d,
        &t,
        &[&TOP],
        json!({"table_terminals":"ledger_and_residuals"}),
        &[],
    );
    let (set, _ev, r) = run(&sp, &d);
    assert_eq!(row(&r, &T2).status, RowStatus::Terminal);
    assert_eq!(set.terminal_provenance[&T2.to_vec()].provenance, "residual");
    // a terminals source declared FIRST keeps the key (no table provenance)
    let st = write_keys(&d, &[&TOP]);
    let sp = json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"A",
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"T","kind":"terminals","keys":[T1]},
                   {"id":"A","kind":"engine_table","path":t.display().to_string()}]});
    let (set, _ev, r) = run(&sp, &d);
    assert_eq!(row(&r, &T1).status, RowStatus::Terminal);
    assert!(!set.terminal_provenance.contains_key(T1.as_slice()));
    assert_eq!(set.report.terminals_other, 1);
}

fn refusal(sp: &Value, d: &std::path::Path) -> (String, String) {
    match try_load(sp, d) {
        Err(ImportError::Refused { reason, .. }) => (reason.word(), reason.to_string()),
        Err(e) => panic!("expected a typed refusal, got {e}"),
        Ok(_) => panic!("expected a typed refusal, the table loaded"),
    }
}

/// Completeness markers: `complete` false, `rules` != rule records, `residuals` / `expected_LP` != declared terminals
/// (residual records + ledger keys) -> the table is REFUSED (typed). A correct marker loads; T228's `assembly_end`
/// (no complete, no residuals) is checked on `rules` only.
#[test]
fn completeness_markers_are_checked_against_the_file() {
    let d = scratch("markers");
    let cases: Vec<(&str, Vec<String>, Option<&str>)> = vec![
        (
            "ok",
            vec![ledger(&[&T1, &T2], Some(2)), complete(2, 2, true)],
            None,
        ),
        (
            "incomplete",
            vec![ledger(&[&T1, &T2], None), complete(2, 2, false)],
            Some("completion:composite_assembly_complete:complete"),
        ),
        (
            "rules",
            vec![ledger(&[&T1, &T2], None), complete(3, 2, true)],
            Some("completion:composite_assembly_complete:rules"),
        ),
        (
            "fewer",
            vec![ledger(&[&T1, &T2], None), complete(1, 2, true)],
            Some("completion:composite_assembly_complete:rules"),
        ),
        (
            "residuals",
            vec![ledger(&[&T1], None), complete(2, 2, true)],
            Some("completion:composite_assembly_complete:residuals"),
        ),
        (
            "ledger-count",
            vec![ledger(&[&T1, &T2], Some(3)), complete(2, 2, true)],
            Some("ledger_malformed"),
        ),
        (
            "ledger-key",
            vec![
                r#"{"type":"composite_terminal_ledger","keys":["I(1,1)"],"count":1}"#.to_string(),
                complete(2, 1, true),
            ],
            Some("ledger_malformed"),
        ),
        (
            "assembly-end-ok",
            vec![json!({"type":"assembly_end","rules":2,"rhs_terms":3}).to_string()],
            None,
        ),
        (
            "assembly-end-bad",
            vec![json!({"type":"assembly_end","rules":5,"rhs_terms":3}).to_string()],
            Some("completion:assembly_end:rules"),
        ),
        (
            "no-complete",
            vec![json!({"type":"composite_assembly_complete","rules":2}).to_string()],
            Some("completion:composite_assembly_complete:complete"),
        ),
    ];
    for (tag, mut tail, want) in cases {
        tail.push(r#"{"type":"end","charts":1}"#.to_string()); // partfix4: complete files (the end record is checked after the markers)
        let t = write_table_with(
            &d,
            &format!("{tag}.jsonl"),
            &header_value(),
            &rules_two(),
            Some(&tail),
        );
        let sp = spec(&d, &t, &[&TOP], json!({}), &[]);
        match want {
            None => {
                try_load(&sp, &d).unwrap_or_else(|e| panic!("{tag}: {e}"));
            }
            Some(w) => {
                let (word, text) = refusal(&sp, &d);
                assert_eq!(word, w, "{tag}: {text}");
            }
        }
    }
}

/// The minimised-table escape (`marker_rules.accept_fewer`, explicit and recorded): a marker stating MORE rules than
/// the file has loads with the reason in the report; fewer stated, `complete` false or a terminal mismatch stay refused.
#[test]
fn accept_fewer_override_is_explicit_and_bounded() {
    let d = scratch("markers-override");
    let why = "minimised table: the minimiser kept the original assembly's marker";
    let t = write_table_with(
        &d,
        "more.jsonl",
        &header_value(),
        &rules_two(),
        Some(&[
            ledger(&[&T1, &T2], None),
            complete(2895, 2, true),
            r#"{"type":"end","charts":1}"#.to_string(),
        ]),
    );
    let sp = spec(
        &d,
        &t,
        &[&TOP],
        json!({"marker_rules":{"accept_fewer": why}}),
        &[],
    );
    let set = try_load(&sp, &d).unwrap();
    let ov = &set.report.sources[0].marker_overrides;
    assert_eq!(ov.len(), 1);
    assert!(ov[0].contains("2895") && ov[0].contains(why), "{ov:?}");
    // strict without the override
    let sp = spec(&d, &t, &[&TOP], json!({}), &[]);
    assert_eq!(
        refusal(&sp, &d).0,
        "completion:composite_assembly_complete:rules"
    );
    // the override does not cover fewer stated rules nor a terminal mismatch
    for (tag, tail) in [
        (
            "less",
            vec![ledger(&[&T1, &T2], None), complete(1, 2, true)],
        ),
        ("terms", vec![ledger(&[&T1], None), complete(2895, 2, true)]),
    ] {
        let mut tail = tail;
        tail.push(r#"{"type":"end","charts":1}"#.to_string());
        let t = write_table_with(
            &d,
            &format!("{tag}.jsonl"),
            &header_value(),
            &rules_two(),
            Some(&tail),
        );
        let sp = spec(
            &d,
            &t,
            &[&TOP],
            json!({"marker_rules":{"accept_fewer": why}}),
            &[],
        );
        assert!(refusal(&sp, &d).0.starts_with("completion:"), "{tag}");
    }
}

/// `assembly_provenance`: pure provenance (any other content ignored), except `one_physical_order`, which must restate
/// the header's order name (T211: "legacy"); a mismatch is refused (typed). T211 shape: provenance second, residual
/// records before the closing marker.
#[test]
fn assembly_provenance_order_must_agree_with_the_header() {
    let d = scratch("provenance");
    let prov = |o: &str| {
        json!({"type":"assembly_provenance","schema":"test.v1","source_manifest":"/x","source_joins":["a"],
            "one_physical_order":o,"completion_semantics":"external"})
        .to_string()
    };
    let res = |k: &[i32]| json!({"type":"residual","chart":MASK,"key":kt(k)}).to_string();
    for (tag, order, ok) in [("same", "legacy", true), ("other", "pred", false)] {
        let p = d.join(format!("{tag}.jsonl"));
        let mut s = format!("{}\n{}\n{}\n", header_value(), prov(order), chart_line());
        for l in rules_two() {
            s.push_str(&l);
            s.push('\n');
        }
        s.push_str(&format!(
            "{}\n{}\n{}\n{}\n",
            res(&T1),
            res(&T2),
            complete(2, 2, true),
            r#"{"type":"end","charts":1}"#
        ));
        std::fs::write(&p, s).unwrap();
        let sp = spec(&d, &p, &[&TOP], json!({}), &[]);
        if ok {
            let set = try_load(&sp, &d).unwrap();
            assert_eq!(
                set.report.sources[0].markers.get("assembly_provenance"),
                Some(&1)
            );
        } else {
            let (w, text) = refusal(&sp, &d);
            assert_eq!(w, "order_mismatch", "{text}");
        }
    }
    // provenance without one_physical_order (libimports5 shape: tool / note / original_records): not interpreted
    let p = d.join("libimports.jsonl");
    let mut s = format!("{}\n{}\n", header_value(), chart_line());
    for l in rules_two() {
        s.push_str(&l);
        s.push('\n');
    }
    s.push_str(&format!(
        "{}\n{}\n",
        json!({"type":"assembly_provenance","tool":"x","note":"copy","original_records":[{"type":"composite_terminal_ledger","keys":["garbage"]}]}),
        r#"{"type":"end","charts":1,"rules":2,"residuals":0}"#
    ));
    std::fs::write(&p, s).unwrap();
    try_load(&spec(&d, &p, &[&TOP], json!({}), &[]), &d).unwrap();
}

/// The classify-only load (interfaces/import.md 18.6) reports the same outcomes as the import.
#[test]
fn classify_only_load_reports_kinds_and_refusals() {
    let d = scratch("classify");
    let ok = write_table_with(
        &d,
        "ok.jsonl",
        &header_value(),
        &rules_two(),
        Some(&[
            ledger(&[&T1, &T2], Some(2)),
            complete(2, 2, true),
            r#"{"type":"end","charts":1}"#.to_string(),
        ]),
    );
    let bad = write_table_with(
        &d,
        "bad.jsonl",
        &header_value(),
        &rules_two(),
        Some(&[
            ledger(&[&T1, &T2], Some(2)),
            complete(9, 2, true),
            r#"{"type":"end","charts":1}"#.to_string(),
        ]),
    );
    let o = import::sweep::ClassifyOptions {
        scratch: Some(d.clone()),
        ..Default::default()
    };
    let c = import::sweep::classify_table(&ok, &o);
    assert_eq!(c.outcome, "loaded", "{c:?}");
    assert_eq!((c.records, c.dispatched, c.table_terminals), (2, 2, 2));
    assert_eq!(c.kinds.get("composite_assembly_complete"), Some(&1));
    assert!(c.assembly);
    let c = import::sweep::classify_table(&bad, &o);
    assert_eq!(c.outcome, "refused");
    assert_eq!(
        c.reason.as_deref(),
        Some("completion:composite_assembly_complete:rules")
    );
    let c = import::sweep::classify_table(
        &bad,
        &import::sweep::ClassifyOptions {
            scratch: Some(d.clone()),
            accept_fewer_rules: Some("test".into()),
            ..Default::default()
        },
    );
    assert_eq!(c.outcome, "loaded");
    assert_eq!(c.marker_overrides.len(), 1);
}
