//! int7 (integration round 2; coordinator's ruling 2026-11-15): a table WITHOUT an `end` record is loaded ONLY through
//! a NAMED list (schema `rustred.campaign.tables-no-end.v1`): the per-source override `marker_rules.accept_missing_end`
//! carries the list's path and sha256 (`"<list path> sha256:<hex>"`) and the table's file must be listed in it.
//! Never a silent refusal (the refusal names the override and the list, and why a given override did not apply),
//! never a silent load (the load records "listed in <list> (sha256:..): <origin>").
//! Constructed tables on the toy sunset (tests/import5_common).

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
fn a_table_without_end_loads_only_through_the_named_list_and_every_refusal_says_why() {
    let d = scratch("int7noend");
    let t = write_table_with(&d, "t.jsonl", &header_value(), &rules(), Some(&[]));
    let other = write_table_with(&d, "other.jsonl", &header_value(), &rules(), Some(&[]));
    let with = |ov: serde_json::Value| spec(&d, &t, &[&TOP], ov, &[]);

    // (1) no override: refused, the message names the override and the list
    let e = try_load(&with(json!({})), &d).unwrap_err();
    let msg = incomplete(&e).unwrap_or_else(|| panic!("typed table_incomplete, got {e:?}"));
    assert!(
        msg.contains("no end record")
            && msg.contains("marker_rules.accept_missing_end")
            && msg.contains("plan/auto-campaign/reports/TABLES-NO-END.json"),
        "{msg}"
    );
    assert!(e.to_string().contains("table_incomplete"), "{e}");

    // (2) the named list that lists the table: loaded, and the load says so (never silent)
    let list = no_end_list(&d, "no-end.json", &[&t]);
    let (set, _ev, _r) = run(&with(json!({"marker_rules": {"accept_missing_end": list}})), &d);
    let ov = &set.report.sources[0].marker_overrides;
    assert!(
        ov.iter().any(|o| o.contains("no end record accepted")
            && o.contains("listed in")
            && o.contains("no-end.json")
            && o.contains("sha256:")
            && o.contains("test writer omits end")),
        "{ov:?}"
    );

    // (3) a valid list that does NOT list this table: refused, the refusal says the list does not list it
    let list_other = no_end_list(&d, "no-end-other.json", &[&other]);
    let e = try_load(
        &with(json!({"marker_rules": {"accept_missing_end": list_other}})),
        &d,
    )
    .unwrap_err();
    let msg = incomplete(&e).unwrap_or_else(|| panic!("{e:?}"));
    assert!(
        msg.contains("does not list") && msg.contains("no-end-other.json"),
        "{msg}"
    );

    // (4) a free-text reason is no longer an override: a typed spec refusal naming the expected form
    let e = try_load(
        &with(json!({"marker_rules": {"accept_missing_end": "the writer omits it"}})),
        &d,
    )
    .unwrap_err();
    assert!(
        matches!(&e, ImportError::Spec(m) if m.contains("is not a named list") && m.contains("sha256:")),
        "{e:?}"
    );

    // (5) the list changed after it was named (sha256 differs): refused
    let stale = list_from(&d, "no-end.json");
    let path = d.join("no-end.json");
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    v["tables"]
        .as_array_mut()
        .unwrap()
        .push(json!({"path": other.display().to_string()}));
    std::fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
    let e = try_load(
        &with(json!({"marker_rules": {"accept_missing_end": stale}})),
        &d,
    )
    .unwrap_err();
    assert!(
        matches!(&e, ImportError::Spec(m) if m.contains("has sha256")),
        "{e:?}"
    );

    // (6) a file of another schema named as the list: refused
    let wrong = d.join("wrong.json");
    std::fs::write(&wrong, br#"{"schema": "something.else", "tables": []}"#).unwrap();
    let sha = import::lines::file_sha256(&wrong).unwrap();
    let e = try_load(
        &with(json!({"marker_rules": {"accept_missing_end": format!("{} sha256:{sha}", wrong.display())}})),
        &d,
    )
    .unwrap_err();
    assert!(
        matches!(&e, ImportError::Spec(m) if m.contains("expected rustred.campaign.tables-no-end.v1")),
        "{e:?}"
    );

    // (7) a table WITH its end record needs no override and records none
    let t2 = write_table_with(&d, "t2.jsonl", &header_value(), &rules(), None);
    let (set, _ev, _r) = run(&spec(&d, &t2, &[&TOP], json!({}), &[]), &d);
    assert!(set.report.sources[0].marker_overrides.is_empty());
}

/// The override text of an existing list file (its current sha256).
fn list_from(d: &std::path::Path, name: &str) -> String {
    let p = d.join(name);
    format!(
        "{} sha256:{}",
        p.display(),
        import::lines::file_sha256(&p).unwrap()
    )
}
