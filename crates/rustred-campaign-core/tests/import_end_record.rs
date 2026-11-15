//! Lane partfix4 (implementer, all-nodes sweep finding): the import's strict load REFUSES a table without an `end`
//! record (`table_incomplete`): a missing end marker cannot be told from a file cut at a line boundary. The explicit
//! per-source escape `marker_rules.accept_missing_end` carries a recorded reason (default strict; an empty reason is
//! refused). A rule record after the `end` record and a LAST LINE cut in the middle (no newline, unparsable tail) are
//! typed refusals, never a dropped line. Constructed tables on the toy sunset (tests/import5_common).

mod import5_common;
use import5_common::*;

use import::ImportError;
use import::error::TableRefusal;
use rustred_campaign_core::import;
use serde_json::json;

const TOP: [i32; 5] = [2, 1, 1, 0, 0];
const T1: [i32; 5] = [1, 1, 1, 0, 0];

fn rules() -> Vec<String> {
    vec![
        point_rule(0, &TOP, &[&T1]),
        point_rule(1, &[1, 2, 1, 0, 0], &[&T1]),
    ]
}

fn incomplete(e: &ImportError) -> Option<String> {
    match e {
        ImportError::Refused { reason, .. } => match reason.as_ref() {
            TableRefusal::Incomplete { msg } => Some(msg.clone()),
            _ => None,
        },
        _ => None,
    }
}

#[test]
fn pf4_table_without_end_record_is_refused_unless_overridden_with_a_reason() {
    let d = scratch("noend");
    let t = write_table_with(&d, "t.jsonl", &header_value(), &rules(), Some(&[]));
    let sp = spec(&d, &t, &[&TOP], json!({}), &[]);
    let e = try_load(&sp, &d).expect_err("no end record: refused");
    let msg = incomplete(&e).unwrap_or_else(|| panic!("typed table_incomplete, got {e:?}"));
    assert!(
        msg.contains("no end record") && msg.contains("2 rule"),
        "{msg}"
    );
    assert!(e.to_string().contains("table_incomplete"), "{e}");
    // int7 (coordinator's ruling): the refusal names the override and the list
    assert!(
        msg.contains("marker_rules.accept_missing_end") && msg.contains("TABLES-NO-END.json"),
        "{msg}"
    );
    // the explicit override naming the list of such tables (path + sha256) that lists this table: loaded, recorded
    let why = no_end_list(&d, "no-end.json", &[&t]);
    let sp = spec(
        &d,
        &t,
        &[&TOP],
        json!({"marker_rules": {"accept_missing_end": why}}),
        &[],
    );
    let (set, _ev, _r) = run(&sp, &d);
    let ov = &set.report.sources[0].marker_overrides;
    assert!(
        ov.iter()
            .any(|o| o.contains("no end record accepted") && o.contains("listed in") && o.contains("no-end.json")),
        "{ov:?}"
    );
    // an empty reason: refused
    let sp = spec(
        &d,
        &t,
        &[&TOP],
        json!({"marker_rules": {"accept_missing_end": " "}}),
        &[],
    );
    assert!(matches!(
        try_load(&sp, &d).unwrap_err(),
        ImportError::Spec(_)
    ));
    // the same table WITH its end record: loaded without any override
    let t2 = write_table_with(&d, "t2.jsonl", &header_value(), &rules(), None);
    let (set, _ev, _r) = run(&spec(&d, &t2, &[&TOP], json!({}), &[]), &d);
    assert!(set.report.sources[0].marker_overrides.is_empty());
    // the classify-only load (sweep tooling) reports the stable word, and loads with the override
    let c = import::sweep::classify_table(
        &t,
        &import::sweep::ClassifyOptions {
            scratch: Some(d.clone()),
            ..Default::default()
        },
    );
    assert_eq!(
        (c.outcome.as_str(), c.reason.as_deref()),
        ("refused", Some("table_incomplete"))
    );
    let c = import::sweep::classify_table(
        &t,
        &import::sweep::ClassifyOptions {
            scratch: Some(d.clone()),
            accept_missing_end: Some(why.clone()),
            ..Default::default()
        },
    );
    assert_eq!(c.outcome, "loaded", "{:?}", c.detail);
}

/// A table cut at a line boundary (the last rule records and the end record gone) is refused: it reads exactly like a
/// table that never had an end record.
#[test]
fn pf4_table_cut_at_a_line_boundary_is_refused() {
    let d = scratch("cutline");
    let full = write_table_with(&d, "full.jsonl", &header_value(), &rules(), None);
    let text = std::fs::read_to_string(&full).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    // header, chart, rule 0 (rule 1 and the end record cut off)
    let cut = d.join("cut.jsonl");
    std::fs::write(&cut, lines[..3].join("\n") + "\n").unwrap();
    let e = try_load(&spec(&d, &cut, &[&TOP], json!({}), &[]), &d).unwrap_err();
    assert!(incomplete(&e).is_some(), "{e:?}");
}

/// The LAST line cut in the middle (no trailing newline, an unparsable tail): a typed refusal at that line, with or
/// without an end record before it, and also with the missing-end override: never a dropped line.
#[test]
fn pf4_last_line_cut_in_the_middle_is_a_typed_refusal() {
    let d = scratch("cutmid");
    let full = write_table_with(&d, "full.jsonl", &header_value(), &rules(), None);
    let text = std::fs::read_to_string(&full).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 5);
    let half = &lines[3][..lines[3].len() / 2];
    // (a) rule 1 cut in the middle, no end record
    let a = d.join("a.jsonl");
    std::fs::write(&a, format!("{}\n{half}", lines[..3].join("\n"))).unwrap();
    let listed = no_end_list(&d, "no-end.json", &[&a]);
    for extra in [
        json!({}),
        json!({"marker_rules": {"accept_missing_end": listed}}),
    ] {
        let e = try_load(&spec(&d, &a, &[&TOP], extra.clone(), &[]), &d).unwrap_err();
        assert!(
            matches!(&e, ImportError::Line { line: 4, .. }),
            "{extra}: a typed refusal at line 4, got {e:?}"
        );
    }
    // (b) a complete table followed by a cut line
    let b = d.join("b.jsonl");
    std::fs::write(&b, format!("{}\n{half}", lines.join("\n"))).unwrap();
    let e = try_load(&spec(&d, &b, &[&TOP], json!({}), &[]), &d).unwrap_err();
    assert!(
        matches!(&e, ImportError::Line { line: 6, .. }) || incomplete(&e).is_some(),
        "{e:?}"
    );
    // (c) the last line complete but without its newline: read (the record is whole)
    let c = d.join("c.jsonl");
    std::fs::write(&c, lines.join("\n")).unwrap();
    assert!(try_load(&spec(&d, &c, &[&TOP], json!({}), &[]), &d).is_ok());
}

/// A rule record after the end record: refused (the end record closes the file).
#[test]
fn pf4_rule_after_the_end_record_is_refused() {
    let d = scratch("afterend");
    let tail = vec![
        r#"{"type":"end","charts":1}"#.to_string(),
        point_rule(2, &[1, 1, 2, 0, 0], &[&T1]),
    ];
    let t = write_table_with(&d, "t.jsonl", &header_value(), &rules(), Some(&tail));
    let e = try_load(&spec(&d, &t, &[&TOP], json!({}), &[]), &d).unwrap_err();
    let msg = incomplete(&e).unwrap_or_else(|| panic!("{e:?}"));
    assert!(msg.contains("after the end record"), "{msg}");
}
