//! Lane sizegen: the improvement record and improve mode's pure decisions (interfaces/genchoice.md 5).

use rustred_campaign_core::economy::improve::*;
use serde_json::Value;
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
    assert_eq!(r.legacy_key(), "I(n0,1,0)|_,1,0|");
    assert!(
        r.key().ends_with("#0") && r.key().len() == 66,
        "the record identity (audit 2026-11-19 a3)"
    );
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
            include: vec![],
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
        include: vec![],
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
    assert!(ImproveRecord::parse(&txt.replace("\"version\":2", "\"version\":99")).is_err());
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
    assert!(
        subtree_verdict(&old, &sub, 1.0)
            .unwrap_err()
            .starts_with("not_smaller"),
        "by monomials: refused"
    );
    assert_eq!(
        subtree_verdict_by(&old, &sub, 1.0, RankBy::Bytes, &c),
        Ok(())
    );
    assert_eq!(
        subtree_verdict_by(&old, &sub, 1.0, RankBy::UseCost, &c),
        Ok(())
    );
    assert_eq!(
        replacement_verdict_by(&old, &a, 1.0, RankBy::Bytes, &c),
        Ok(())
    );
    assert!(replacement_verdict_by(&a, &old, 1.0, RankBy::Bytes, &c).is_err());
    // size_of under use_cost: 30 ns/B + 20 us + 100 ns/monomial
    assert_eq!(
        size_of(&[&old], RankBy::UseCost, &c),
        6_300_000_000 + 20_000 + 319_500
    );
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
    assert_eq!(
        yield_check(100_000_000, 7_200_000, Some(r), &y),
        (Some(50_000_000.0), true)
    );
    assert!(!yield_check(1, 60_000, Some(r), &y).1);
    assert!(
        !yield_check(100_000_000, 7_200_000, None, &y).1,
        "no reference: no stop"
    );
    assert!(
        !yield_check(140_000_000, 2 * 3_600_000, Some(r), &y).1,
        "70 MB/CPU-h >= 66.7: go on"
    );
    assert_eq!(
        reference_yield_run(&rec),
        Some((200_000_000.0, 1)),
        "with the run it comes from"
    );
}

/// interfaces/genchoice.md 5b (v1.7): NO yield stop unless the config writes one; `{}` = the documented values; the
/// effective setting is one describable line; the sliding window takes the newest settled items up to its CPU.
#[test]
fn yield_stop_is_explicit_described_and_windowed() {
    assert_eq!(
        ImprovePolicy::default().yield_stop,
        None,
        "an absent key is no stop (was Some(default) until v1.6)"
    );
    let absent: ImprovePolicy = serde_json::from_str("{}").unwrap();
    assert_eq!(absent.yield_stop, None);
    let explicit: ImprovePolicy = serde_json::from_str(r#"{"yield_stop": {}}"#).unwrap();
    assert_eq!(explicit.yield_stop, Some(YieldStop::default()));
    let w: ImprovePolicy =
        serde_json::from_str(r#"{"yield_stop": {"window_cpu_s": 7200, "min_cpu_s": 0}}"#).unwrap();
    assert_eq!(w.yield_stop.as_ref().unwrap().window_cpu_s, Some(7200));
    assert!(
        serde_json::from_str::<ImprovePolicy>(r#"{"yield_stop": {"windw": 1}}"#).is_err(),
        "unknown field refused"
    );
    let y = YieldStop::default();
    let d = y.describe(Some(59_700_000.0), "reference = record run 1");
    assert!(
        d.contains("0.3333 x 59700000")
            && d.contains("record run 1")
            && d.contains("cumulative over the run"),
        "{d}"
    );
    assert!(d.contains("armed after 3600 CPU-s"), "{d}");
    let dw = YieldStop {
        window_cpu_s: Some(7200),
        ..YieldStop::default()
    }
    .describe(None, "no reference");
    assert!(
        dw.contains("never stops") && dw.contains("sliding window of the last 7200 CPU-s"),
        "{dw}"
    );
    // window: (cpu_ms, gain) oldest first; the newest items until their CPU reaches the window
    let settled = [
        (3_600_000u64, 100u64),
        (1_000_000, 0),
        (1_000_000, 0),
        (1_000_000, 5),
    ];
    assert_eq!(
        window_totals(&settled, 2_000_000),
        (5, 2_000_000),
        "the two newest reach 2,000 CPU-s"
    );
    assert_eq!(
        window_totals(&settled, 2_500_000),
        (5, 3_000_000),
        "a third item is needed to reach the window"
    );
    assert_eq!(
        window_totals(&settled, u64::MAX),
        (105, 6_600_000),
        "never reached: everything (= cumulative)"
    );
    assert_eq!(window_totals(&[], 1), (0, 0));
    // the cumulative yield of these items stays above a line the window falls under
    let r = 1_000_000.0;
    let (ws, wc) = window_totals(&settled, 2_000_000);
    assert!(
        yield_check(
            ws,
            wc,
            Some(r),
            &YieldStop {
                min_cpu_s: 0,
                ..y.clone()
            }
        )
        .1,
        "window: 9 B/CPU-h < 1/3 MB"
    );
    assert!(
        !yield_check(
            105_000_000,
            6_600_000,
            Some(r),
            &YieldStop { min_cpu_s: 0, ..y }
        )
        .1,
        "cumulative: ~57 MB/CPU-h"
    );
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
    assert_eq!(
        targets(&rules, &rec, &p)[0].0,
        1,
        "the excluded (dead) rule is skipped, the next one taken"
    );
}

#[test]
fn assembly_markers_with_a_rule_count_are_restated() {
    let per = std::collections::BTreeMap::from([("110".to_string(), 7u64)]);
    let m = restate_marker(
        r#"{"type":"composite_assembly_complete","complete":true,"rules":2895,"residuals":2}"#,
        2956,
        &per,
    );
    assert!(m.contains("\"rules\":2956"), "{m}");
    let c = restate_marker(
        r#"{"type":"chart_end","chart":"110","rules":3}"#,
        2956,
        &per,
    );
    assert!(c.contains("\"rules\":7"), "{c}");
    let r = restate_marker(r#"{"type":"residual","rules":1}"#, 2956, &per);
    assert!(
        r.contains("\"rules\":1"),
        "residual records are not markers"
    );
}

#[test]
fn explicit_lists_and_equal_size_regeneration_for_exactness() {
    use rustred_campaign_core::economy::genchoice::CostModel;
    let mut a = rule(&line(0, "[null,1,0]", "I(n0,1,0)", "[]", 5000));
    let mut b = rule(&line(1, "[1,1,0]", "I(1,1,0)", "[]", 10));
    a.bytes = 9000;
    b.bytes = 100;
    let rules = vec![a.clone(), b.clone()];
    let rec = ImproveRecord::empty("T", &"0".repeat(64), &rules);
    let mut p = ImprovePolicy::default();
    p.filter.largest = Some(1);
    p.filter.include = vec![1, 0];
    let t = targets(&rules, &rec, &p);
    assert_eq!(
        t.iter().map(|x| x.0).collect::<Vec<_>>(),
        vec![1, 0],
        "the explicit list, in its order, largest ignored"
    );
    let c = CostModel::default();
    // equal size: refused normally, accepted for exactness; larger: always refused
    assert!(replacement_verdict_eq(&b, &b.clone(), 0.9, RankBy::Bytes, &c, false).is_err());
    assert_eq!(
        replacement_verdict_eq(&b, &b.clone(), 0.9, RankBy::Bytes, &c, true),
        Ok(())
    );
    let mut bigger = b.clone();
    bigger.bytes = 101;
    assert!(replacement_verdict_eq(&b, &bigger, 0.9, RankBy::Bytes, &c, true).is_err());
}

/// Third defect (lane parametric, 2026-11-17): the improved table must satisfy the native importer's ORDER RULE (index =
/// position among the chart's rules in file order). `set_index` changes only the top-level index number (never a nested
/// "index" member, never a string); `replacement_lines` positions a sub-table and keeps origin.dispatch_position.
#[test]
fn improved_tables_are_positioned_for_the_native_importer() {
    let mut p = Positions::default();
    assert!(p.take("a", 0).is_ok() && p.take("a", 1).is_ok() && p.take("b", 0).is_ok());
    assert!(
        p.take("a", 3).unwrap_err().contains("out of order"),
        "a gap is refused"
    );
    assert_eq!(
        (p.peek("a"), p.peek("b"), p.peek("c")),
        (2, 1, 0),
        "per chart"
    );
    // only the top-level number moves: nested "index" members and string contents stay
    let l = r#"{"type":"rule","chart":"11","index":7,"origin":{"index":7,"note":"\"index\":7"},"rhs":[["I(1,1)","1"]]}"#;
    let n = set_index(l, 3).unwrap();
    assert_eq!(
        n,
        l.replacen(r#""index":7,"origin""#, r#""index":3,"origin""#, 1)
    );
    let v: Value = serde_json::from_str(&n).unwrap();
    assert_eq!(
        (v["index"].as_i64(), v["origin"]["index"].as_i64()),
        (Some(3), Some(7))
    );
    assert_eq!(v["origin"]["note"], "\"index\":7");
    assert_eq!(
        set_index(&n, 3).unwrap(),
        n,
        "already right: unchanged bytes"
    );
    // a key order with the index late in the line, with a space
    let late = r#"{"case":"x","chart":"11","index": 12,"type":"rule"}"#;
    assert_eq!(
        set_index(late, 4).unwrap(),
        r#"{"case":"x","chart":"11","index": 4,"type":"rule"}"#
    );
    // replacement lines: consecutive positions from the old rule's place, provenance kept
    let old = r#"{"type":"rule","chart":"11","index":5,"origin":{"dispatch_position":42,"source":"bank#1"},"rhs":[]}"#;
    let sub = vec![
        r#"{"chart":"11","target":"I(n0,1)","rhs":[]}"#.to_string(),
        r#"{"chart":"11","target":"I(1,1)","rhs":[],"origin":{"engine":"x"}}"#.to_string(),
    ];
    let out = replacement_lines(&sub, old, 5, 5, 2).unwrap();
    let vs: Vec<Value> = out
        .iter()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        (vs[0]["index"].as_i64(), vs[1]["index"].as_i64()),
        (Some(5), Some(6))
    );
    for (k, v) in vs.iter().enumerate() {
        assert_eq!(v["type"], "rule");
        assert_eq!(
            v["origin"]["dispatch_position"], 42,
            "kept from the replaced record"
        );
        assert_eq!(v["origin"]["replaces"]["index"], 5);
        assert_eq!(v["origin"]["replaces"]["subtable_rule"], k);
        assert_eq!(
            v["origin"]["replaces"]["record_sha256"]
                .as_str()
                .unwrap()
                .len(),
            64
        );
        assert_eq!(v["origin"]["improve_run"], 2);
    }
    assert_eq!(
        vs[1]["origin"]["engine"], "x",
        "the line's own origin members stay"
    );
    let no_origin = r#"{"type":"rule","chart":"11","index":5,"rhs":[]}"#;
    let v: Value =
        serde_json::from_str(&replacement_lines(&sub[..1], no_origin, 5, 9, 1).unwrap()[0])
            .unwrap();
    assert!(v["origin"].get("dispatch_position").is_none() && v["index"] == 9);
}

/// Fourth defect (2026-11-17): the certification facts of a replacement come from the tool (every sub-table rule, never only the
/// first); declared terminals are checked exactly (fixed slots, equations, exception branches).
#[test]
fn replacement_facts_list_every_rule_and_check_declared_terminals() {
    let r = |idx: i64, fixed: Value, exc: Value, target: &str| {
        serde_json::json!({"type":"rule","chart":"111","index":idx,"fixed":fixed,"equations":[],"exceptions":exc,
            "target":target,"rhs":[["I(n0-1,n1,n2)","1"]],"monomials":3}).to_string()
    };
    let old = r(
        4,
        serde_json::json!([1, null, null]),
        serde_json::json!([["n2"], ["-1+n1"]]),
        "I(1,n1,n2)",
    );
    let subs = vec![
        r(
            4,
            serde_json::json!([1, null, null]),
            serde_json::json!([["n2"], ["-2+n1"]]),
            "I(1,n1,n2)",
        ),
        r(
            5,
            serde_json::json!([1, 1, null]),
            serde_json::json!([]),
            "I(1,1,n2)",
        ),
        r(
            6,
            serde_json::json!([1, 2, null]),
            serde_json::json!([]),
            "I(1,2,n2)",
        ),
    ];
    let v = |l: &str| serde_json::from_str::<Value>(l).unwrap();
    assert!(
        serves_key(&v(&subs[0]), &[1, 1, 3]).unwrap(),
        "-1+n1 is not a branch of the case rule (it captured it)"
    );
    assert!(
        !serves_key(&v(&subs[0]), &[1, 2, 3]).unwrap(),
        "exception branch -2+n1 holds"
    );
    assert!(
        !serves_key(&v(&subs[0]), &[1, 5, 0]).unwrap(),
        "exception branch n2 holds"
    );
    assert!(serves_key(&v(&subs[0]), &[1, 5, 3]).unwrap());
    assert!(
        !serves_key(&v(&subs[1]), &[2, 1, 0]).unwrap(),
        "fixed slot differs"
    );
    assert!(serves_key(&v(&subs[1]), &[1, 1, 0]).unwrap());
    let f = replacement_facts(&old, &subs, &[vec![1, 1, 0], vec![7, 7, 7]]).unwrap();
    let pos: Vec<i64> = f["replacement"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["position"].as_i64().unwrap())
        .collect();
    assert_eq!(pos, vec![4, 5, 6], "every rule of the sub-table");
    assert_eq!(f["replacement_rules"], 3);
    assert_eq!(f["captured_branches"], serde_json::json!(["-1+n1"]));
    assert_eq!(
        f["new_branches_of_the_case_rule"],
        serde_json::json!(["-2+n1"])
    );
    assert_eq!(
        f["terminal_check"],
        serde_json::json!([{"position": 5, "terminal": [1, 1, 0]}]),
        "{f:#}"
    );
    assert_eq!(f["old"]["target_rhs_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(
        f["replacement"][0]["target_rhs_sha256"], f["old"]["target_rhs_sha256"],
        "same target and rhs here"
    );
    let clean = replacement_facts(&old, &subs, &[vec![9, 9, 9]]).unwrap();
    assert_eq!(clean["terminal_check"], "clean");
    let none = replacement_facts(&old, &subs, &[]).unwrap();
    assert!(
        none["terminal_check"]
            .as_str()
            .unwrap()
            .starts_with("not requested")
    );
}

// --------------------------------------------------------------------------------------------------------------
// audit 2026-11-19 (Astra SOLVE-AUDIT candidate/REPORT.md, lane rulecost): domain bounds (a2) and record identity (a3)
// --------------------------------------------------------------------------------------------------------------

/// Six one-term T162-final records (helper-proved symmetry services / a symmetry alias), verbatim from
/// T162.cand.table.jsonl minus their provenance members: 1911, 1925, 1926, 1927, 1928, 1929.
const T162_ONE_TERM: [&str; 6] = [
    r#"{"type":"rule","chart":"11111111100000000000","index":1911,"fixed":[null,1,1,null,null,1,1,1,1,0,0,null,0,0,0,0,0,0,0,0],"equations":[],"exceptions":[],"target":"I(n0,1,1,n3,n4,1,1,1,1,0,0,n11,0,0,0,0,0,0,0,0)","rhs":[["I(n0,n3,1,1,n4,1,1,1,1,0,0,n11,0,0,0,0,0,0,0,0)","1"]],"domain_bounds":{"max":[null,null,null,null,null,null,null,null,null,null,null,-1,null,null,null,null,null,null,null,null],"min":[2,null,null,2,2,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null]}}"#,
    r#"{"type":"rule","chart":"11111111100000000000","index":1925,"fixed":[1,null,1,1,1,1,null,null,1,0,0,0,0,0,0,0,0,0,0,0],"equations":[],"exceptions":[],"target":"I(1,n1,1,1,1,1,n6,n7,1,0,0,0,0,0,0,0,0,0,0,0)","rhs":[["I(n6,1,1,1,n1,1,1,n7,1,0,0,0,0,0,0,0,0,0,0,0)","1"]],"domain_bounds":{"min":[null,2,null,null,null,null,2,2,null,null,null,null,null,null,null,null,null,null,null,null],"max":[null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null]}}"#,
    r#"{"type":"rule","chart":"11111111100000000000","index":1926,"fixed":[1,1,null,null,null,1,1,1,1,0,0,0,0,0,0,0,0,0,0,0],"equations":[],"exceptions":[],"target":"I(1,1,n2,n3,n4,1,1,1,1,0,0,0,0,0,0,0,0,0,0,0)","rhs":[["I(1,n4,1,n3,1,n2,1,1,1,0,0,0,0,0,0,0,0,0,0,0)","1"]],"domain_bounds":{"min":[null,null,2,2,2,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null],"max":[null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null]}}"#,
    r#"{"type":"rule","chart":"11111111100000000000","index":1927,"fixed":[1,1,null,1,1,null,1,1,1,null,0,0,0,0,0,0,0,0,0,0],"equations":[],"exceptions":[],"target":"I(1,1,n2,1,1,n5,1,1,1,n9,0,0,0,0,0,0,0,0,0,0)","rhs":[["I(n5,1,1,1,1,1,1,1,n2,0,n9,0,0,0,0,0,0,0,0,0)","1"]],"domain_bounds":{"min":[null,null,2,null,null,2,null,null,null,null,null,null,null,null,null,null,null,null,null,null],"max":[null,null,null,null,null,null,null,null,null,-1,null,null,null,null,null,null,null,null,null,null]}}"#,
    r#"{"type":"rule","chart":"11111111100000000000","index":1928,"fixed":[1,1,1,1,1,1,null,1,null,0,0,0,0,0,0,0,0,0,0,0],"equations":[],"exceptions":[],"target":"I(1,1,1,1,1,1,n6,1,n8,0,0,0,0,0,0,0,0,0,0,0)","rhs":[["I(n6,1,1,1,1,1,1,1,n8,0,0,0,0,0,0,0,0,0,0,0)","1"]],"domain_bounds":{"min":[null,null,null,null,null,null,2,null,2,null,null,null,null,null,null,null,null,null,null,null],"max":[null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null]}}"#,
    r#"{"type":"rule","chart":"11111111100000000000","index":1929,"fixed":[1,1,1,1,1,1,1,null,null,0,0,0,0,0,null,0,0,0,0,0],"equations":[],"exceptions":[],"target":"I(1,1,1,1,1,1,1,n7,n8,0,0,0,0,0,n14,0,0,0,0,0)","rhs":[["I(1,1,1,1,1,n7,n8,1,1,0,0,0,0,0,0,0,0,0,n14,0)","1"]],"domain_bounds":{"min":[null,null,null,null,null,null,null,2,2,null,null,null,null,null,null,null,null,null,null,null],"max":[null,null,null,null,null,null,null,null,null,null,null,null,null,null,-1,null,null,null,null,null]}}"#,
];

fn t162_key(lines: [i32; 9]) -> Vec<i32> {
    let mut k = lines.to_vec();
    k.extend([0; 11]);
    k
}

#[test]
fn audit_a2_domain_bounds_exclude_the_declared_terminals() {
    // T162's two declared terminals: the all-ones corner and the dot on slot 0
    let corner = t162_key([1; 9]);
    let dot = t162_key([2, 1, 1, 1, 1, 1, 1, 1, 1]);
    for l in T162_ONE_TERM {
        let v: Value = serde_json::from_str(l).unwrap();
        assert!(
            !serves_key(&v, &corner).unwrap(),
            "{l}: the corner is outside its bounds"
        );
        assert!(
            !serves_key(&v, &dot).unwrap(),
            "{l}: the dot is outside its bounds"
        );
    }
    // without the bounds the old predicate accepted the corner for all six (and the dot for 1911): the audit's finding
    for l in T162_ONE_TERM {
        let mut v: Value = serde_json::from_str(l).unwrap();
        v.as_object_mut().unwrap().remove("domain_bounds");
        assert!(serves_key(&v, &corner).unwrap());
    }
    // the positive case: 1925 with its three line powers (slots 1, 6, 7) at 2; one of them at 1 is outside
    let v: Value = serde_json::from_str(T162_ONE_TERM[1]).unwrap();
    assert!(serves_key(&v, &t162_key([1, 2, 1, 1, 1, 1, 2, 2, 1])).unwrap());
    assert!(!serves_key(&v, &t162_key([1, 2, 1, 1, 1, 1, 2, 1, 1])).unwrap());
    // the bounds as the native importer reads them; a side of the wrong arity is an error, not "unbounded"
    let r = rule(T162_ONE_TERM[1]);
    let b = r.bounds.clone().unwrap();
    assert_eq!(
        (b.0[1], b.0[6], b.0[7], b.0[0]),
        (Some(2), Some(2), Some(2), None)
    );
    let bad = T162_ONE_TERM[1].replace(
        r#""max":[null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null]"#,
        r#""max":[null]"#,
    );
    assert!(serves_key(&serde_json::from_str(&bad).unwrap(), &corner).is_err());
    // a replacement may not narrow the domain by a bound (target, case and exceptions alone would let it pass)
    let with = |l: String, b: &str| format!("{},\"domain_bounds\":{b}}}", &l[..l.len() - 1]);
    let old = rule(&line(4, "[null,1,0]", "I(n0,1,0)", "[]", 100));
    let narrower = rule(&with(
        line(4, "[null,1,0]", "I(n0,1,0)", "[]", 10),
        r#"{"min":[2,null,null],"max":null}"#,
    ));
    let e = structural_verdict(&old, &narrower).unwrap_err();
    assert!(e.starts_with("bounds_narrower"), "{e}");
    let old_b = rule(&with(
        line(4, "[null,1,0]", "I(n0,1,0)", "[]", 100),
        r#"{"min":[2,null,null],"max":null}"#,
    ));
    let wider = rule(&line(4, "[null,1,0]", "I(n0,1,0)", "[]", 10));
    assert!(
        structural_verdict(&old_b, &wider).is_ok(),
        "dropping a bound widens the domain"
    );
    assert!(
        structural_verdict(&old_b, &narrower).is_ok(),
        "an equal bound"
    );
}

#[test]
fn audit_a3_the_history_identity_tells_records_apart() {
    // the shape of T162 #1621 (a 302-term IBP rule, four exception branches) and #1927 (a one-term symmetry, none):
    // same target and fixed slots, i.e. the same version-1 key
    let a = r#"{"type":"rule","chart":"110","index":0,"fixed":[1,null,null],"equations":[],"target":"I(1,n1,n2)","rhs":[["I(1,n1-1,n2)","n1/(d-4)"],["I(1,n1,n2-1)","1"]],"exceptions":[["-1+n1"],["n2"]]}"#;
    let b = r#"{"type":"rule","chart":"110","index":1,"fixed":[1,null,null],"equations":[],"target":"I(1,n1,n2)","rhs":[["I(1,n2,n1)","1"]],"exceptions":[]}"#;
    let c = r#"{"type":"rule","chart":"110","index":2,"fixed":[2,null,null],"equations":[],"target":"I(2,n1,n2)","rhs":[["I(1,n1,n2)","1"]],"exceptions":[]}"#;
    let (_, rules) = table_rules([a, b, c]).unwrap();
    assert_eq!(rules[0].legacy_key(), rules[1].legacy_key());
    assert_ne!(rules[0].key(), rules[1].key());
    // an attempt on the first does not mark the second as tried
    let tried = |cand: &str| Tried {
        cand: cand.into(),
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
    };
    let mut rec = ImproveRecord::empty("n", "sha", &rules);
    rec.rules[0].tried.push(tried("relift"));
    assert!(rec.tried(&rules[0].key(), "relift"));
    assert!(
        !rec.tried(&rules[1].key(), "relift"),
        "1927 stays eligible after an attempt on 1621"
    );
    // save / reload keeps both apart, in either order
    let back = ImproveRecord::parse(&serde_json::to_string(&rec).unwrap()).unwrap();
    assert!(back.tried(&rules[0].key(), "relift") && !back.tried(&rules[1].key(), "relift"));
    let (_, swapped) = table_rules([b, a]).unwrap();
    let mut rec2 = back.clone();
    rec2.align(&swapped);
    assert!(rec2.tried(&swapped[1].key(), "relift") && !rec2.tried(&swapped[0].key(), "relift"));
    // renumbering keeps the identity; byte-identical bodies are told apart by their occurrence
    let b7 = b.replace(r#""index":1"#, r#""index":7"#);
    let (_, re) = table_rules([a, b7.as_str()]).unwrap();
    assert_eq!(re[1].key(), rules[1].key());
    let (_, dup) = table_rules([a, a]).unwrap();
    assert_ne!(dup[0].key(), dup[1].key());
    // a VERSION-1 record: its entry under the shared legacy key is credited to NEITHER rule; a unique one is carried
    let mut v1 = ImproveRecord::empty("n", "sha", &rules);
    v1.version = 1;
    for e in v1.rules.iter_mut() {
        e.key = format!("{}|{}", e.target, e.case);
    }
    v1.rules.truncate(1);
    v1.rules[0].tried.push(tried("relift"));
    let mut uniq = ImproveRecord::empty("n", "sha", &rules[2..])
        .rules
        .remove(0);
    uniq.key = rules[2].legacy_key();
    uniq.tried.push(tried("rung"));
    v1.rules.push(uniq);
    let v1 = ImproveRecord::parse(&serde_json::to_string(&v1).unwrap()).expect("version 1 is read");
    let mut m = v1.clone();
    m.align(&rules);
    assert_eq!(m.version, RECORD_VERSION);
    assert!(!m.tried(&rules[0].key(), "relift") && !m.tried(&rules[1].key(), "relift"));
    assert!(
        m.entry(&rules[0].key())
            .unwrap()
            .why
            .contains("not credited")
    );
    assert!(
        m.tried(&rules[2].key(), "rung"),
        "an unambiguous version-1 entry is carried"
    );
}
