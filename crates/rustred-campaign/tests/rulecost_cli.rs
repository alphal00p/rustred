//! lane rulecost: `rr_campaign rule-cost` end to end with a FAKE probe child (a shell script that writes recorded
//! probe outputs). Spawns a child: run with `LIGHT_PIDNS=1` (L10). Test data only: a five-slot toy owner.
use rustred_campaign::rulecost::rule_cost_cmd;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn sha256_hex(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rulecost-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The fake probe: `rules <pkg> <owner> <out>` copies rules.out to <out>; `keys <pkg> <owner> <keys> <out> ..` checks
/// that every key line names a key and copies keys.out to <out>; anything else fails.
fn fake_probe(dir: &Path) -> PathBuf {
    let rules_header = json!({"tool": "p_rule_cost rules", "schema": "rustred.rule-cost.rules.v1", "class": "00000000000000ab",
        "presentation": 7, "arity": 5, "support": "f", "charts": ["f"], "rules": 2});
    let r0 = json!({"chart": "f", "position": 0, "ordinal": 0, "fixed": [], "rhs_terms": 3,
        "numerator": {"terms": 9, "max_degree_base": 2}, "denominator": {"terms": 3}, "distinct_original_denominators": 1});
    let r1 = json!({"chart": "f", "position": 1, "ordinal": 1, "fixed": [[0, 1]], "rhs_terms": 2,
        "numerator": {"terms": 4}, "denominator": {"terms": 2}});
    std::fs::write(
        dir.join("rules.out"),
        format!("{rules_header}\n{r0}\n{r1}\n"),
    )
    .unwrap();
    let keys_header = json!({"tool": "p_rule_cost keys", "schema": "rustred.rule-cost.keys.v1", "keys": 3, "repeat": 2});
    let k = |key: [i64; 5], pos: usize, s: f64| {
        json!({"key": key, "via": {"chart": "f", "position": pos}, "seconds": [s, s], "seconds_min": s, "seconds_median": s,
            "children": 2, "children_same_owner": 1,
            "receivers": [{"owner": "00000000000000ab", "lane": "same-owner", "lines": 4, "children": 1},
                          {"owner": "L1T03-00000000000000cd", "lane": "Insertion", "lines": 3, "children": 1}]})
    };
    std::fs::write(
        dir.join("keys.out"),
        format!(
            "{keys_header}\n{}\n{}\n{}\n",
            k([2, 1, 1, 1, 0], 0, 0.5),
            k([1, 2, 1, 1, 0], 0, 0.7),
            k([1, 1, 1, 2, 0], 1, 0.01)
        ),
    )
    .unwrap();
    let script = dir.join("p_rule_cost");
    std::fs::write(
        &script,
        "#!/bin/sh\nset -e\ncase \"$1\" in\n  rules) cp \"$FAKE_DIR/rules.out\" \"$4\" ;;\n  keys) grep -q key \"$4\"; cp \"$FAKE_DIR/keys.out\" \"$5\" ;;\n  *) echo bad >&2; exit 3 ;;\nesac\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    script
}

fn spec(dir: &Path, probe: &Path, extra: Value) -> PathBuf {
    let mut s = json!({"schema": "rustred.campaign.rule-cost.v1",
        "probe": {"path": probe, "sha256": sha256_hex(&std::fs::read(probe).unwrap())},
        "package": dir.join("pkg"), "owner": "L1T04-00000000000000ab", "rules": ["f:0", "f:1"],
        "sampler": {"shells": [[1, 0], [2, 0]], "per_shell": 2, "seed": 3}, "repeat": 2, "profile_level": 0,
        "env": {"FAKE_DIR": dir}});
    for (k, v) in extra.as_object().unwrap() {
        s[k] = v.clone();
    }
    let p = dir.join("spec.json");
    std::fs::write(&p, serde_json::to_vec_pretty(&s).unwrap()).unwrap();
    p
}

fn args(spec: &Path, out: &Path, more: &[&str]) -> Vec<String> {
    let mut a = vec![
        "--spec".to_string(),
        spec.display().to_string(),
        "--dir".to_string(),
        out.display().to_string(),
    ];
    a.extend(more.iter().map(|s| s.to_string()));
    a
}

#[test]
fn rule_cost_samples_measures_aggregates_and_joins_the_profile() {
    let dir = tmp("e2e");
    let probe = fake_probe(&dir);
    let profile = json!({"owners": [{"class": "00000000000000ab", "rules": [["000000000000000f", 0, 10, 6.0], ["000000000000000f", 1, 100, 1.0]]}]});
    std::fs::write(dir.join("owner_stats.json"), profile.to_string()).unwrap();
    // the owner's source table: record 1 is a slot alias (no payload position), so payload position 1 is RECORD 2
    // (audit 2026-11-19 a1: the profile's payload ordinal is never used as a record index)
    let rec = |i: usize, rhs: &str| {
        json!({"type": "rule", "chart": "11110", "index": i, "case": "c", "fixed": [null, null, null, null, 0],
            "equations": [], "target": "I(n0,n1,n2,n3,0)", "rhs": [[rhs, "1"]], "exceptions": []})
        .to_string()
    };
    let table = [
        json!({"type": "header", "schema": "table_dump.v1"}).to_string(),
        rec(0, "I(n0-1,n1,n2,n3,0)"),
        rec(1, "I(n1,n0,n2,n3,0)"),
        rec(2, "I(n0,n1-1,n2,n3,0)"),
    ]
    .join("\n");
    std::fs::write(dir.join("t.jsonl"), table).unwrap();
    let sp = spec(
        &dir,
        &probe,
        json!({"profile": dir.join("owner_stats.json"), "table": dir.join("t.jsonl")}),
    );
    let out = dir.join("out");
    let summary = rule_cost_cmd(&args(&sp, &out, &[])).unwrap();
    assert!(summary.contains("3 keys"), "{summary}");
    // sampled keys: on each rule's face, both shells, round-robin over the rules
    let keys = std::fs::read_to_string(out.join("keys.jsonl")).unwrap();
    let keys: Vec<Value> = keys
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert!(!keys.is_empty() && keys.len() <= 8);
    assert!(keys.iter().any(|k| k["rule"] == "f:1"));
    assert!(
        keys.iter()
            .filter(|k| k["rule"] == "f:1")
            .all(|k| k["key"][0] == 1)
    );
    let report: Value =
        serde_json::from_slice(&std::fs::read(out.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["schema"], "rustred.campaign.rule-cost-report.v1");
    let rows = report["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["rule"], "f:0", "targets in the spec's order");
    assert_eq!(rows[0]["cost"]["applications"], 2);
    assert!((rows[0]["cost"]["seconds_median"].as_f64().unwrap() - 0.6).abs() < 1e-9);
    assert_eq!(rows[0]["run"]["calls"], 10);
    assert!((rows[0]["predicted_run_seconds"].as_f64().unwrap() - 6.0).abs() < 1e-9);
    assert_eq!(rows[0]["coefficient_monomials"], 12);
    let model: Value =
        serde_json::from_slice(&std::fs::read(out.join("rule_cost_model.json")).unwrap()).unwrap();
    assert_eq!(model["calls"]["11110:0"], 10.0);
    assert_eq!(
        model["calls"]["11110:2"], 100.0,
        "payload 1 = record 2 (record 1 is an alias)"
    );
    assert!(model["calls"].get("11110:1").is_none());
    assert_eq!(report["profile_unmapped"].as_array().unwrap().len(), 0);
    let md = std::fs::read_to_string(out.join("report.md")).unwrap();
    assert!(
        md.contains("| f:0 |") && md.contains("Insertion 3L 1.0"),
        "{md}"
    );
    // re-aggregation without a child, other spec settings
    let again = rule_cost_cmd(&args(&sp, &out, &["--aggregate-only"])).unwrap();
    assert_eq!(again, summary);
    // without a source table nothing is bound: no model, every profile position reported unmapped
    let sp2 = spec(
        &dir,
        &probe,
        json!({"profile": dir.join("owner_stats.json")}),
    );
    let out2 = dir.join("out_unbound");
    rule_cost_cmd(&args(&sp2, &out2, &[])).unwrap();
    assert!(!out2.join("rule_cost_model.json").exists());
    let r2: Value =
        serde_json::from_slice(&std::fs::read(out2.join("report.json")).unwrap()).unwrap();
    assert_eq!(r2["profile_unmapped"].as_array().unwrap().len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rule_cost_refuses_a_changed_probe_and_reports_a_failing_child() {
    let dir = tmp("refuse");
    let probe = fake_probe(&dir);
    let sp = spec(
        &dir,
        &probe,
        json!({"probe": {"path": probe, "sha256": "00"}}),
    );
    let e = rule_cost_cmd(&args(&sp, &dir.join("out"), &[])).unwrap_err();
    assert!(e.contains("sha256"), "{e}");
    // a probe that fails: the error names the status and its last stderr line
    let bad = dir.join("bad_probe");
    std::fs::write(&bad, "#!/bin/sh\necho 'no such owner' >&2\nexit 2\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let sp = spec(&dir, &bad, json!({}));
    let e = rule_cost_cmd(&args(&sp, &dir.join("out2"), &[])).unwrap_err();
    assert!(e.contains("no such owner"), "{e}");
    let sp = spec(&dir, &probe, json!({"schema": "other"}));
    assert!(
        rule_cost_cmd(&args(&sp, &dir.join("out3"), &[]))
            .unwrap_err()
            .contains("schema")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rule_score_reads_records_and_estimates_the_cached_engine() {
    use rustred_campaign::rulecost::rule_score_cmd;
    let dir = tmp("score");
    let rec = |index: i64, rhs: &[(&str, &str)]| {
        json!({"type": "rule", "chart": "11110", "index": index, "fixed": [null, null, null, null, null],
            "equations": [], "target": "I(n0,n1,n2,n3,n4)", "exceptions": [],
            "rhs": rhs.iter().map(|(i, c)| json!([i, c])).collect::<Vec<_>>()})
        .to_string()
    };
    // two children with the same coefficient: the cache specialises it once
    let shared = "(n1+n2+d)/(d-4)";
    let table = format!(
        "{}\n{}\n{}\n",
        json!({"type": "header", "schema": "table_dump.v1"}),
        rec(
            0,
            &[
                ("I(n0,n1-1,n2,n3,n4)", shared),
                ("I(n0,n1,n2-1,n3,n4)", shared)
            ]
        ),
        rec(1, &[("I(n0,n1-1,n2,n3,n4)", "1")])
    );
    std::fs::write(dir.join("t.jsonl"), table).unwrap();
    let out = dir.join("score.jsonl");
    let a = |more: &[&str]| -> Vec<String> {
        let mut v = vec![
            "--table".to_string(),
            dir.join("t.jsonl").display().to_string(),
        ];
        v.extend(more.iter().map(|s| s.to_string()));
        v
    };
    let summary =
        rule_score_cmd(&a(&["--include", "0", "--out", &out.display().to_string()])).unwrap();
    assert!(summary.contains("1 rules"), "{summary}");
    let line: Value = serde_json::from_str(std::fs::read_to_string(&out).unwrap().trim()).unwrap();
    assert_eq!(
        (line["index"].as_i64(), line["rhs_terms"].as_u64()),
        (Some(0), Some(2))
    );
    assert_eq!(line["distinct_coefficients"], 1);
    assert_eq!(line["sharing"], 2.0);
    assert!(line["cached_ns_estimate"].as_f64().unwrap() < line["today_ns"].as_f64().unwrap());
    assert!(rule_score_cmd(&["--model".to_string()]).is_err());
    // keys go to the first serving rule (file order); presentation slots mapped to table slots
    std::fs::write(
        dir.join("k.jsonl"),
        "{\"key\": [2, 1, 1, 1, 0]}\n{\"key\": [1, 1, 2, 1, -1]}\n",
    )
    .unwrap();
    let k = dir.join("k.jsonl").display().to_string();
    let summary = rule_score_cmd(&a(&[
        "--keys",
        &k,
        "--table-to-presentation",
        "1,0,2,3,4",
        "--out",
        &out.display().to_string(),
    ]))
    .unwrap();
    assert!(summary.contains("keys 2 served 2 unserved 0"), "{summary}");
    let lines: Vec<Value> = std::fs::read_to_string(&out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        (
            lines[0]["keys_served"].as_u64(),
            lines[1]["keys_served"].as_u64()
        ),
        (Some(2), Some(0))
    );
    // one level: the key [1,2,2,1,0] meets rule 0, whose two children stay in the sector and meet rule 0 again
    std::fs::write(dir.join("k1.jsonl"), "{\"key\": [1, 2, 2, 1, 0]}\n").unwrap();
    let k1 = dir.join("k1.jsonl").display().to_string();
    let o = out.display().to_string();
    let summary = rule_score_cmd(&a(&["--keys", &k1, "--one-level", "--out", &o])).unwrap();
    assert!(
        summary.contains("ONE LEVEL") && summary.contains("2.0 same-sector children per key"),
        "{summary}"
    );
    let first = std::fs::read_to_string(&out).unwrap();
    let l0: Value = serde_json::from_str(first.lines().next().unwrap()).unwrap();
    let own = l0["today_ns"].as_f64().unwrap();
    let one = l0["one_level_today_ns_per_key"].as_f64().unwrap();
    assert!((one - 3.0 * own).abs() < 1e-6 * own, "{one} vs 3 x {own}");
    assert!(
        rule_score_cmd(&a(&["--one-level"])).is_err(),
        "one level needs keys"
    );
    // levels: [1,2,2,1,0] -> children [1,1,2,1,0], [1,2,1,1,0]; each has ONE same-sector child [1,1,1,1,0] (met twice: one step)
    let summary = rule_score_cmd(&a(&["--keys", &k1, "--levels", "3", "--out", &o])).unwrap();
    assert!(
        summary.contains("level 1: 2 distinct keys stepped"),
        "{summary}"
    );
    assert!(
        summary.contains("2 same-sector children produced, 1 new"),
        "{summary}"
    );
    assert!(
        summary.contains("level 2: 1 distinct keys stepped"),
        "{summary}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
