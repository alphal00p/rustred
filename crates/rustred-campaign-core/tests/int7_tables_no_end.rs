//! int7 (integration round 2; coordinator's rulings 2026-11-15): a table WITHOUT an `end` record is loaded ONLY through
//! the named RESOLUTION (schema `rustred.campaign.tables-no-end.resolution.v1`): the per-source override
//! `marker_rules.accept_missing_end` carries its path and sha256 (`"<path> sha256:<hex>"`); the table's file must be an
//! entry with the loadable status and its sha256 equal to the entry's `sha256_scanned`. Never a silent refusal (the
//! refusal names the override and the resolution, and why a given override did not apply), never a silent load (the
//! load records "listed in <resolution> (sha256:..) as <status> ..: <why>"). Last test: one of the campaign's real
//! loadable tables through the campaign's real resolution file (skipped where they are not on disk).
//! Constructed tables on the toy sunset (tests/import5_common).

mod import5_common;
mod inputs;
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
            && msg.contains("plan/auto-campaign/reports/TABLES-NO-END.resolution.json"),
        "{msg}"
    );
    assert!(e.to_string().contains("table_incomplete"), "{e}");

    // (2) the named list that lists the table: loaded, and the load says so (never silent)
    let list = no_end_list(&d, "no-end.json", &[&t]);
    let (set, _ev, _r) = run(
        &with(json!({"marker_rules": {"accept_missing_end": list}})),
        &d,
    );
    let ov = &set.report.sources[0].marker_overrides;
    assert!(
        ov.iter().any(|o| o.contains("no end record accepted")
            && o.contains("listed in")
            && o.contains("no-end.json")
            && o.contains("sha256:")
            && o.contains("flagged_astra_workspace")
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
        matches!(&e, ImportError::Spec(m) if m.contains("does not name a resolution") && m.contains("sha256:")),
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

    // (6) a file of another schema named (e.g. the census list rustred.campaign.tables-no-end.v1): refused
    let wrong = d.join("wrong.json");
    std::fs::write(
        &wrong,
        br#"{"schema": "rustred.campaign.tables-no-end.v1", "tables": []}"#,
    )
    .unwrap();
    let sha = import::lines::file_sha256(&wrong).unwrap();
    let e = try_load(
        &with(json!({"marker_rules": {"accept_missing_end": format!("{} sha256:{sha}", wrong.display())}})),
        &d,
    )
    .unwrap_err();
    assert!(
        matches!(&e, ImportError::Spec(m) if m.contains("expected rustred.campaign.tables-no-end.resolution.v1")),
        "{e:?}"
    );

    // (6b) the table listed with another status (an end added since / superseded / unfinished run): refused, naming it
    let st = no_end_list_with(&d, "no-end-status.json", &[&t], "end_added");
    let e = try_load(
        &with(json!({"marker_rules": {"accept_missing_end": st}})),
        &d,
    )
    .unwrap_err();
    let msg = incomplete(&e).unwrap_or_else(|| panic!("{e:?}"));
    assert!(msg.contains("status \"end_added\""), "{msg}");
    // (6c) the table listed, loadable, but its bytes changed since they were verified: refused
    let ok_then = no_end_list(&d, "no-end-bytes.json", &[&t]);
    let mut text = std::fs::read_to_string(&t).unwrap();
    text.push('\n');
    std::fs::write(&t, &text).unwrap();
    let e = try_load(
        &with(json!({"marker_rules": {"accept_missing_end": ok_then}})),
        &d,
    )
    .unwrap_err();
    let msg = incomplete(&e).unwrap_or_else(|| panic!("{e:?}"));
    assert!(msg.contains("verified other bytes"), "{msg}");

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

/// One of the campaign's real loadable tables (T228, Astra's workspace, read only) through the campaign's real
/// resolution file: refused without the override, loaded (classify-only) with it, the load names the resolution.
/// The resolution path from RUSTRED_TABLES_NO_END_RESOLUTION or the campaign's default.
/// int8 (int/10, 2026-11-18): IGNORED. Since 2026-11-15T11:27Z the campaign's resolution file has no entry with status
/// `flagged_astra_workspace` (every no-end table got an end record), so this test passed VACUOUSLY (its skip message was
/// captured) in every suite since; the override path itself is covered by
/// `a_table_without_end_loads_only_through_the_named_list_and_every_refusal_says_why`. Run with --ignored against a
/// resolution that lists a loadable table on disk; a missing input FAILS.
#[test]
#[ignore = "real data: the campaign resolution has had no flagged_astra_workspace entry since 2026-11-15 (see doc)"]
fn a_real_loadable_table_through_the_real_resolution() {
    let res = std::env::var("RUSTRED_TABLES_NO_END_RESOLUTION").unwrap_or_else(|_| {
        "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/plan/auto-campaign/reports/TABLES-NO-END.resolution.json"
            .into()
    });
    let res = std::path::PathBuf::from(res);
    let Ok(text) = std::fs::read_to_string(&res) else {
        inputs::missing(format_args!(
            "SKIP: no resolution file at {}",
            res.display()
        ));
        return;
    };
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    // the smallest loadable entry that is on disk (test data choice; T228 at the time of writing)
    let mut cands: Vec<(u64, std::path::PathBuf)> = v["tables"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["status"] == import::lines::NO_END_LOADABLE_STATUS)
        .filter_map(|e| {
            let p = std::path::PathBuf::from(e["path"].as_str()?);
            let n = std::fs::metadata(&p).ok()?.len();
            Some((n, p))
        })
        .collect();
    cands.sort();
    let Some((_, table)) = cands.first().cloned() else {
        inputs::missing(format_args!(
            "SKIP: no loadable table of the resolution on disk"
        ));
        return;
    };
    let d = scratch("int7noendreal");
    let c = import::sweep::classify_table(
        &table,
        &import::sweep::ClassifyOptions {
            scratch: Some(d.clone()),
            ..Default::default()
        },
    );
    assert_eq!(
        (c.outcome.as_str(), c.reason.as_deref()),
        ("refused", Some("table_incomplete")),
        "{}: {:?}",
        table.display(),
        c.detail
    );
    let ov = format!(
        "{} sha256:{}",
        res.display(),
        import::lines::file_sha256(&res).unwrap()
    );
    let c = import::sweep::classify_table(
        &table,
        &import::sweep::ClassifyOptions {
            scratch: Some(d.clone()),
            accept_missing_end: Some(ov),
            ..Default::default()
        },
    );
    assert_eq!(c.outcome, "loaded", "{}: {:?}", table.display(), c.detail);
}
