//! Lane sizegen: the improvement record and improve mode's pure decisions (interfaces/genchoice.md 5).

use rustred_campaign_core::economy::improve::*;
use std::collections::BTreeMap;

fn line(index: i64, fixed: &str, target: &str, exc: &str, monomials: u64) -> String {
    format!(
        r#"{{"type":"rule","chart":"110","index":{index},"fixed":{fixed},"equations":[],"target":"{target}","rhs":[["I(a)","1"]],"exceptions":{exc},"monomials":{monomials}}}"#
    )
}

fn rule(l: &str) -> TableRule {
    TableRule::from_line(l).unwrap().unwrap()
}

#[test]
fn rule_records_and_keys() {
    let r = rule(&line(
        3,
        "[null,1,0]",
        "I(n0,1,0)",
        r#"[["n0","-1+n1"],["-1+n0"]]"#,
        77,
    ));
    assert_eq!(r.index, 3);
    assert_eq!(r.case, "_,1,0|");
    assert_eq!(
        r.exceptions,
        vec!["-1+n0".to_string(), "-1+n1&n0".to_string()],
        "canonical, sorted"
    );
    assert_eq!(r.key(), "I(n0,1,0)|_,1,0|");
    assert!(
        TableRule::from_line(r#"{"type":"header"}"#)
            .unwrap()
            .is_none()
    );
    let h: serde_json::Value = serde_json::from_str(
        r#"{"type":"header","node_order":{"name":"x","coordinate_priority":[1,0,2]}}"#,
    )
    .unwrap();
    assert!(header_order(&h).unwrap().contains("coordinate_priority"));
    assert!(header_order(&serde_json::json!({"type":"header"})).is_none());
}

#[test]
fn replacement_needs_same_case_subset_exceptions_and_smaller() {
    let old = rule(&line(
        0,
        "[null,1,0]",
        "I(n0,1,0)",
        r#"[["-1+n0"],["n2"]]"#,
        5000,
    ));
    let ok = rule(&line(9, "[null,1,0]", "I(n0,1,0)", r#"[["-1+n0"]]"#, 100));
    assert_eq!(replacement_verdict(&old, &ok, 1.0), Ok(()));
    let more_exc = rule(&line(9, "[null,1,0]", "I(n0,1,0)", r#"[["n1"]]"#, 50));
    assert!(
        replacement_verdict(&old, &more_exc, 1.0)
            .unwrap_err()
            .starts_with("exceptions_not_subset")
    );
    let bigger = rule(&line(9, "[null,1,0]", "I(n0,1,0)", r#"[]"#, 6000));
    assert!(
        replacement_verdict(&old, &bigger, 1.0)
            .unwrap_err()
            .starts_with("not_smaller")
    );
    let other = rule(&line(9, "[1,1,0]", "I(1,1,0)", r#"[]"#, 6));
    assert!(replacement_verdict(&old, &other, 1.0).is_err());
    // min_gain 0.01: 100 > 5000 x 0.01 = 50 -> refused
    assert!(replacement_verdict(&old, &ok, 0.01).is_err());
}

#[test]
fn targets_skip_recorded_candidates_and_apply_keeps_the_dispatch_position() {
    let lines = vec![
        r#"{"type":"header","node_order":{"name":"x"}}"#.to_string(),
        line(0, "[null,null,null]", "I(n0,n1,n2)", "[]", 5000),
        line(1, "[1,null,null]", "I(1,n1,n2)", "[]", 3000),
        line(2, "[1,1,null]", "I(1,1,n2)", "[]", 10),
        r#"{"type":"end","rules":3}"#.to_string(),
    ];
    let (_, rules) = table_rules(lines.iter().map(String::as_str)).unwrap();
    let mut rec = ImproveRecord::empty("N", "sha", &rules);
    assert_eq!(
        rec.ranking_by_bytes[0].2, 5000,
        "ranking by bytes, largest first"
    );
    let mut p = ImprovePolicy {
        filter: RuleFilter {
            largest: Some(2),
            min_monomials: None,
            by: RankBy::Monomials,
            exclude: vec![],
        },
        ..ImprovePolicy::default()
    };
    let t = targets(&rules, &rec, &p);
    assert_eq!(t.iter().map(|x| x.0).collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(t[0].1.len(), p.candidates.len());
    // record every candidate of rule 0 as tried: rule 0 drops out; a run with unchanged settings then does nothing more
    for c in &p.candidates {
        rec.rules[0].tried.push(Tried {
            cand: c.name.clone(),
            scope: "rule".into(),
            run: 1,
            args: vec![],
            status: "complete".into(),
            monomials: Some(1),
            bytes: Some(1),
            cpu_ms: Some(1),
            verdict: "not_smaller".into(),
            table: None,
            pole: None,
            derivation: None,
        });
    }
    assert_eq!(
        targets(&rules, &rec, &p)
            .iter()
            .map(|x| x.0)
            .collect::<Vec<_>>(),
        vec![1]
    );
    p.filter = RuleFilter {
        largest: None,
        min_monomials: Some(3000),
        by: RankBy::Monomials,
        exclude: vec![],
    };
    assert_eq!(
        targets(&rules, &rec, &p)
            .iter()
            .map(|x| x.0)
            .collect::<Vec<_>>(),
        vec![1]
    );
    // apply: the new record takes the OLD index (same position), everything else verbatim
    let mut repl = BTreeMap::new();
    repl.insert(1, line(77, "[1,null,null]", "I(1,n1,n2)", "[]", 30));
    let (out, old) = apply(&lines, &repl).unwrap();
    assert_eq!(out.len(), lines.len());
    assert_eq!(old, vec![lines[2].clone()]);
    let r = rule(&out[2]);
    assert_eq!((r.index, r.monomials), (1, 30));
    assert_eq!(out[1], lines[1]);
    assert_eq!(out[4], lines[4]);
    // record round trip and version refusal
    let txt = serde_json::to_string(&rec).unwrap();
    assert_eq!(ImproveRecord::parse(&txt).unwrap(), rec);
    assert!(ImproveRecord::parse(&txt.replace("\"version\":1", "\"version\":99")).is_err());
}

#[test]
fn generation_record_lists_the_same_case_in_the_losers() {
    let w = vec![rule(&line(0, "[null,1,0]", "I(n0,1,0)", "[]", 10))];
    let o = vec![(
        "cand2:full".to_string(),
        vec![rule(&line(5, "[null,1,0]", "I(n0,1,0)", "[]", 999))],
    )];
    let run = RunEntry {
        run: 1,
        mode: "generate".into(),
        settings: serde_json::Value::Null,
        table_in_sha256: None,
        table_out_sha256: "s".into(),
        budget_cpu_ms: None,
        spent_cpu_ms: 5,
        totals: serde_json::Value::Null,
    };
    let rec = generation_record(
        "N",
        "s",
        "cand1",
        &w,
        &o,
        &["cand3: probe gave no table".into()],
        run,
    );
    assert_eq!(rec.rules[0].chosen, "cand1");
    assert_eq!(rec.rules[0].tried.len(), 2);
    assert_eq!(rec.rules[0].tried[1].monomials, Some(999));
    assert_eq!(rec.rules[0].untried.len(), 1);
    assert_eq!(rec.runs.len(), 1);
}

#[test]
fn legacy_header_is_the_identity_order_and_subtrees_replace_whole_domains() {
    let h: serde_json::Value =
        serde_json::from_str(r#"{"type":"header","order_name":"legacy","order_is_program":false}"#)
            .unwrap();
    let o: serde_json::Value = serde_json::from_str(&header_order(&h).unwrap()).unwrap();
    assert_eq!(o["coordinate_priority"], serde_json::Value::Null);
    assert_eq!(o["name"], "id");
    let lines = vec![
        r#"{"type":"header","order_name":"legacy"}"#.to_string(),
        r#"{"type":"chart","chart":"110"}"#.to_string(),
        line(0, "[null,null,null]", "I(n0,n1,n2)", "[]", 5000),
        // the heavy old rule's record is long (as real heavy rules are: bytes ~ monomials)
        line(1, "[1,null,null]", "I(1,n1,n2)", r#"[["-1+n1"]]"#, 3000).replace(
            r#""monomials""#,
            &format!(r#""pad":"{}","monomials""#, "x".repeat(600)),
        ),
        line(2, "[1,1,null]", "I(1,1,n2)", "[]", 10),
        r#"{"type":"chart_end","chart":"110","rules":3}"#.to_string(),
        r#"{"type":"end","charts":1,"rules":3}"#.to_string(),
    ];
    let (_, rules) = table_rules(lines.iter().map(String::as_str)).unwrap();
    // rule 1 regenerated as a sub-table: the case rule with a NEW branch + the branch's child
    let sub = vec![
        line(
            0,
            "[1,null,null]",
            "I(1,n1,n2)",
            r#"[["-1+n1"],["n2"]]"#,
            100,
        ),
        line(1, "[1,null,0]", "I(1,n1,0)", "[]", 50),
    ];
    let subr: Vec<TableRule> = sub.iter().map(|l| rule(l)).collect();
    assert!(
        replacement_verdict(&rules[1], &subr[0], 1.0)
            .unwrap_err()
            .starts_with("exceptions_not_subset")
    );
    assert_eq!(subtree_verdict(&rules[1], &subr, 1.0), Ok(()));
    let big = [
        line(0, "[1,null,null]", "I(1,n1,n2)", "[]", 2900),
        line(1, "[1,null,0]", "I(1,n1,0)", "[]", 200),
    ];
    let bigr: Vec<TableRule> = big.iter().map(|l| rule(l)).collect();
    assert!(
        subtree_verdict(&rules[1], &bigr, 1.0)
            .unwrap_err()
            .starts_with("not_smaller")
    );
    let mut repl = BTreeMap::new();
    repl.insert(1, sub.clone());
    let (out, old) = apply_subtrees(&lines, &repl).unwrap();
    assert_eq!(old, vec![lines[3].clone()]);
    let (_, nr) = table_rules(out.iter().map(String::as_str)).unwrap();
    assert_eq!(
        nr.iter().map(|r| r.index).collect::<Vec<_>>(),
        vec![0, 1, 3, 2],
        "the sub-table's first rule keeps the old index, the child takes a fresh one; file order is the dispatch order"
    );
    assert_eq!(
        nr.iter().map(|r| r.monomials).collect::<Vec<_>>(),
        vec![5000, 100, 50, 10]
    );
    let end: serde_json::Value = serde_json::from_str(out.last().unwrap()).unwrap();
    assert_eq!(end["rules"], 4, "completion markers restated");
    let ce: serde_json::Value = serde_json::from_str(&out[out.len() - 2]).unwrap();
    assert_eq!(ce["rules"], 4);
}

#[test]
fn filter_ranks_by_bytes_when_asked() {
    // rule 0: many monomials, short record; rule 1: few monomials, a long record
    let mut a = rule(&line(0, "[null,1,0]", "I(n0,1,0)", "[]", 5000));
    let mut b = rule(&line(1, "[1,1,0]", "I(1,1,0)", "[]", 10));
    a.bytes = 100;
    b.bytes = 9000;
    let rules = vec![a, b];
    let rec = ImproveRecord::empty("T", &"0".repeat(64), &rules);
    let mut p = ImprovePolicy::default();
    p.filter.largest = Some(1);
    assert_eq!(targets(&rules, &rec, &p)[0].0, 0, "default: by monomials");
    p.filter.by = RankBy::Bytes;
    assert_eq!(targets(&rules, &rec, &p)[0].0, 1, "by bytes");
}

#[test]
fn replacement_follows_the_size_measure() {
    use rustred_campaign_core::economy::genchoice::CostModel;
    // the legacy record: few monomials, huge bytes; the regenerated sub-table: many monomials, far fewer bytes
    let mut old = rule(&line(7, "[null,1,0]", "I(n0,1,0)", "[]", 3195));
    old.bytes = 210_000_000;
    let mut a = rule(&line(7, "[null,1,0]", "I(n0,1,0)", "[]", 900_000));
    a.bytes = 16_000_000;
    let mut b = rule(&line(8, "[1,1,0]", "I(1,1,0)", "[]", 100_000));
    b.bytes = 1_800_000;
    let sub = vec![a.clone(), b];
    let c = CostModel::default();
    assert!(subtree_verdict(&old, &sub, 1.0).unwrap_err().starts_with("not_smaller"), "by monomials: refused");
    assert_eq!(subtree_verdict_by(&old, &sub, 1.0, RankBy::Bytes, &c), Ok(()));
    assert_eq!(subtree_verdict_by(&old, &sub, 1.0, RankBy::UseCost, &c), Ok(()));
    assert_eq!(replacement_verdict_by(&old, &a, 1.0, RankBy::Bytes, &c), Ok(()));
    assert!(replacement_verdict_by(&a, &old, 1.0, RankBy::Bytes, &c).is_err());
    // size_of under use_cost: 30 ns/B + 20 us + 100 ns/monomial
    assert_eq!(size_of(&[&old], RankBy::UseCost, &c), 6_300_000_000 + 20_000 + 319_500);
}

#[test]
fn tranche_yield_stop_and_record_skips() {
    let mut rec = ImproveRecord::empty("T", &"0".repeat(64), &[]);
    rec.runs.push(RunEntry {
        run: 1,
        mode: "improve".into(),
        settings: serde_json::json!({}),
        table_in_sha256: None,
        table_out_sha256: "x".into(),
        budget_cpu_ms: None,
        spent_cpu_ms: 3_600_000 * 10,
        totals: serde_json::json!({"table_bytes_before": 3_000_000_000u64, "table_bytes_after": 1_000_000_000u64}),
    });
    let r = reference_yield(&rec).unwrap();
    assert_eq!(r, 200_000_000.0, "2 GB in 10 CPU-h");
    let y = YieldStop::default();
    assert!((y.min_fraction - 1.0 / 3.0).abs() < 1e-12);
    // 50 MB per CPU-h after 2 CPU-h: below 1/3 x 200 MB: stop; before min_cpu_s: no stop
    assert_eq!(yield_check(100_000_000, 7_200_000, Some(r), &y), (Some(50_000_000.0), true));
    assert!(!yield_check(1, 60_000, Some(r), &y).1);
    assert!(!yield_check(100_000_000, 7_200_000, None, &y).1, "no reference: no stop");
    assert!(!yield_check(140_000_000, 2 * 3_600_000, Some(r), &y).1, "70 MB/CPU-h >= 66.7: go on");
}

#[test]
fn excluded_rules_are_never_targeted() {
    let mut a = rule(&line(0, "[null,1,0]", "I(n0,1,0)", "[]", 5000));
    let mut b = rule(&line(1, "[1,1,0]", "I(1,1,0)", "[]", 10));
    a.bytes = 9000;
    b.bytes = 100;
    let rules = vec![a, b];
    let rec = ImproveRecord::empty("T", &"0".repeat(64), &rules);
    let mut p = ImprovePolicy::default();
    p.filter.largest = Some(1);
    p.filter.by = RankBy::Bytes;
    p.filter.exclude = vec![0];
    assert_eq!(targets(&rules, &rec, &p)[0].0, 1, "the excluded (dead) rule is skipped, the next one taken");
}
