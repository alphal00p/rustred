//! Lane sizegen ITEM 2: pole score, pole objective of the generation choice and of improve (interfaces/genchoice.md 6).

use rustred_campaign_core::economy::genchoice::{FileEntry, Policy};
use rustred_campaign_core::economy::improve::*;
use rustred_campaign_core::economy::polescore::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn summary(max: u64, by: Value, undecided: u64) -> Value {
    json!({"schema": "rustred.campaign.route-pole-census.v1", "box": {"keys": 1000, "keys_with_pole": 7,
        "keys_by_order": by, "max_order": max, "rules_with_pole": 3, "undecided": undecided}})
}

fn score(max: Option<u32>, kwp: u64) -> PoleScore {
    PoleScore {
        keys: 100,
        keys_with_pole: kwp,
        undecided: u64::from(max.is_none()),
        max_order: max,
        keys_by_order: BTreeMap::new(),
        rules_with_pole: None,
        composed_max_order: None,
    }
}

fn cand(i: usize, size: u64, time: u64, s: Option<PoleScore>) -> PoleCand {
    PoleCand {
        cand: i,
        admissible: true,
        size,
        time_ms: time,
        score: s,
    }
}

fn pol(ratio: &str) -> PolePolicy {
    PolePolicy {
        max_size_ratio: Value::String(ratio.into()),
        ..PolePolicy::default()
    }
}

#[test]
fn census_scores_and_undecided_is_never_zero() {
    let s =
        PoleScore::from_census_summary(&summary(2, json!({"0": 993, "1": 6, "2": 1}), 0)).unwrap();
    assert_eq!(s.max_order, Some(2));
    assert_eq!(s.keys_by_order, BTreeMap::from([(1, 6), (2, 1)]));
    assert_eq!(s.rules_with_pole, Some(3));
    assert_eq!(
        s.composed_max_order, None,
        "composed per terminal: not computed = undecided"
    );
    let u = PoleScore::from_census_summary(&summary(0, json!({"0": 990}), 10)).unwrap();
    assert_eq!(
        u.max_order, None,
        "an undecided key makes the score undecided, not regular"
    );
    assert!(!u.decided());
    let r = PoleScore::from_census_rule(
        &json!({"keys": 50, "keys_with_pole": 0, "max_order": 0, "undecided": 0, "label": "4"}),
    )
    .unwrap();
    assert_eq!(r.max_order, Some(0));
    assert!(PoleScore::from_census_summary(&json!({"box": {}})).is_err());
    // score-file lines
    let l = json!({"schema": SCORE_SCHEMA, "node": "T1", "scope": "rule", "cand": "original", "rule_index": 4,
        "score": r});
    assert_eq!(parse_scores(&format!("{l}\n")).unwrap().len(), 1);
    let bad =
        json!({"schema": SCORE_SCHEMA, "node": "T1", "scope": "rule", "cand": "x", "score": r});
    assert!(
        parse_scores(&bad.to_string()).is_err(),
        "rule scope needs rule_index"
    );
}

#[test]
fn objective_regular_then_order_then_size_then_time() {
    let p = pol("unbounded");
    // regular wins over a smaller singular one (unbounded tolerance)
    let c = [
        cand(0, 100, 5, Some(score(Some(1), 3))),
        cand(1, 900, 5, Some(score(Some(0), 0))),
    ];
    let d = choose_poles(&c, Some(0), &p).unwrap().unwrap();
    assert_eq!(
        (d.winner, d.changed, d.reason.as_str()),
        (1, true, "regular")
    );
    // tolerance 2: the regular class (9x the smallest) is rejected for size, recorded
    let d = choose_poles(&c, Some(0), &pol("2")).unwrap().unwrap();
    assert_eq!((d.winner, d.reason.as_str()), (0, "size_guard"));
    assert_eq!(d.rejected_for_size, vec![1]);
    // lower max order wins among singular ones
    let c = [
        cand(0, 100, 5, Some(score(Some(2), 1))),
        cand(1, 150, 5, Some(score(Some(1), 9))),
    ];
    assert_eq!(choose_poles(&c, Some(0), &p).unwrap().unwrap().winner, 1);
    // equal poles: size, then time, then index
    let c = [
        cand(0, 100, 9, Some(score(Some(1), 1))),
        cand(1, 100, 3, Some(score(Some(1), 1))),
    ];
    let d = choose_poles(&c, Some(0), &p).unwrap().unwrap();
    assert_eq!(
        (d.winner, d.reason.as_str()),
        (1, "poles_tied_size_decided"),
        "faster"
    );
    let c = [
        cand(4, 100, 3, Some(score(Some(1), 1))),
        cand(2, 100, 3, Some(score(Some(1), 1))),
    ];
    assert_eq!(
        choose_poles(&c, None, &p).unwrap().unwrap().winner,
        2,
        "then index"
    );
    // undecided ranks after every decided candidate, never as regular
    let c = [
        cand(0, 10, 1, Some(score(None, 0))),
        cand(1, 500, 1, Some(score(Some(2), 50))),
    ];
    let d = choose_poles(&c, Some(0), &p).unwrap().unwrap();
    assert_eq!((d.winner, d.undecided.clone()), (1, vec![0]));
    // no decided score at all: the size choice stands
    let c = [cand(0, 10, 1, None), cand(1, 5, 1, Some(score(None, 0)))];
    let d = choose_poles(&c, Some(0), &p).unwrap().unwrap();
    assert_eq!(
        (d.winner, d.reason.as_str(), d.changed),
        (0, "no_decided_pole_score", false)
    );
    // non-admissible candidates never win (a pole never changes a verdict)
    let mut c = [
        cand(0, 100, 1, Some(score(Some(2), 1))),
        cand(1, 10, 1, Some(score(Some(0), 0))),
    ];
    c[1].admissible = false;
    assert_eq!(choose_poles(&c, Some(0), &p).unwrap().unwrap().winner, 0);
    // bad measure words are refused
    let bad = PolePolicy {
        measures: vec!["poles".into()],
        ..PolePolicy::default()
    };
    assert!(bad.check().is_err());
}

#[test]
fn ignore_switch_is_bit_identical() {
    let off = PolePolicy {
        use_poles: false,
        ..PolePolicy::default()
    };
    let c = [
        cand(0, 100, 5, Some(score(Some(1), 3))),
        cand(1, 900, 5, Some(score(Some(0), 0))),
    ];
    assert_eq!(
        choose_poles(&c, Some(0), &off).unwrap(),
        None,
        "ignored: the caller keeps its size choice"
    );
    // the effective policy of an ignoring run serialises exactly as before the pole objective existed
    let p = Policy {
        poles: off,
        ..Default::default()
    };
    let v = serde_json::to_value(&p).unwrap();
    assert!(v.get("poles").is_none());
    let mut legacy = v.clone();
    legacy.as_object_mut().unwrap().remove("poles");
    assert_eq!(
        serde_json::to_string(&v).unwrap(),
        serde_json::to_string(&legacy).unwrap()
    );
    // a config without `poles` gets the default objective (on); `{"use": false}` turns it off
    let d: Policy = serde_json::from_str("{}").unwrap();
    assert!(!d.poles.is_ignored());
    let o: Policy = serde_json::from_str(r#"{"poles":{"use":false}}"#).unwrap();
    assert!(o.poles.is_ignored());
    // rechoice: ignored = None
    let ch = json!({"node": "T1", "candidates": []});
    assert_eq!(rechoice(&ch, &[], &o.poles).unwrap(), None);
}

fn line(index: i64, exc: &str, monomials: u64) -> String {
    format!(
        r#"{{"type":"rule","chart":"110","index":{index},"fixed":[null,1,0],"equations":[],"target":"I(n0,1,0)","rhs":[["I(a)","1"]],"exceptions":{exc},"monomials":{monomials}}}"#
    )
}

fn rule(l: &str) -> TableRule {
    TableRule::from_line(l).unwrap().unwrap()
}

#[test]
fn rule_verdict_under_the_pole_objective() {
    assert_eq!(
        PolePolicy::default().max_size_ratio,
        json!(DEFAULT_MAX_SIZE_RATIO)
    );
    let p = pol("2");
    let old = rule(&line(3, r#"[["-1+n0"]]"#, 1000));
    let reg = rule(&line(9, "[]", 1500));
    // structurally invalid: refused whatever the poles
    let wider = rule(&line(9, r#"[["n2"]]"#, 10));
    assert!(
        rule_pole_verdict(
            &old,
            Some(&score(Some(1), 4)),
            &wider,
            Some(&score(Some(0), 0)),
            1.0,
            &p
        )
        .unwrap_err()
        .starts_with("exceptions_not_subset")
    );
    // a regular candidate replaces a singular rule even when larger (within the tolerance 2)
    assert_eq!(
        rule_pole_verdict(
            &old,
            Some(&score(Some(1), 4)),
            &reg,
            Some(&score(Some(0), 0)),
            1.0,
            &p
        )
        .unwrap(),
        "replaced_poles"
    );
    // beyond the tolerance: size guard
    let huge = rule(&line(9, "[]", 5000));
    assert!(
        rule_pole_verdict(
            &old,
            Some(&score(Some(1), 4)),
            &huge,
            Some(&score(Some(0), 0)),
            1.0,
            &p
        )
        .unwrap_err()
        .starts_with("size_guard")
    );
    // undecided on either side: never replaces
    assert!(
        rule_pole_verdict(
            &old,
            Some(&score(Some(1), 4)),
            &reg,
            Some(&score(None, 0)),
            1.0,
            &p
        )
        .unwrap_err()
        .starts_with("pole_undecided")
    );
    assert!(
        rule_pole_verdict(&old, None, &reg, Some(&score(Some(0), 0)), 1.0, &p)
            .unwrap_err()
            .starts_with("pole_undecided")
    );
    // equal poles: the size rule decides (smaller replaces, larger does not)
    let small = rule(&line(9, "[]", 200));
    assert_eq!(
        rule_pole_verdict(
            &old,
            Some(&score(Some(1), 4)),
            &small,
            Some(&score(Some(1), 4)),
            1.0,
            &p
        )
        .unwrap(),
        "replaced_size"
    );
    assert!(
        rule_pole_verdict(
            &old,
            Some(&score(Some(1), 4)),
            &reg,
            Some(&score(Some(1), 4)),
            1.0,
            &p
        )
        .unwrap_err()
        .starts_with("poles_not_better")
    );
    // a worse pole order never replaces under an unbounded tolerance, even much smaller ...
    let unb = pol("unbounded");
    assert!(
        rule_pole_verdict(
            &old,
            Some(&score(Some(1), 4)),
            &small,
            Some(&score(Some(2), 1)),
            1.0,
            &unb
        )
        .is_err()
    );
    // ... but with tolerance 2 the old rule (5x the candidate) is rejected for size: the tolerance works both ways
    assert_eq!(
        rule_pole_verdict(
            &old,
            Some(&score(Some(1), 4)),
            &small,
            Some(&score(Some(2), 1)),
            1.0,
            &p
        )
        .unwrap(),
        "replaced_size_guard"
    );
}

#[test]
fn pole_targets_decide_from_scores_and_never_regenerate() {
    let rules: Vec<TableRule> = [line(0, "[]", 10), line(1, "[]", 50), line(2, "[]", 30)]
        .iter()
        .map(|l| {
            let mut r = rule(l);
            r.target = format!("I(t{})", r.index);
            r
        })
        .collect();
    let mut rec = ImproveRecord::empty("T1", &"0".repeat(64), &rules);
    let p = ImprovePolicy {
        poles: Some(PolePolicy::default()),
        ..ImprovePolicy::default()
    };
    assert!(p.poles_on() && p.single());
    let own = |i: i64, m: Option<u32>| ScoreLine {
        schema: SCORE_SCHEMA.into(),
        node: "T1".into(),
        scope: "rule".into(),
        cand: "original".into(),
        rule_index: Some(i),
        table: None,
        score: score(m, 2),
        source: None,
    };
    // rule 0 order 2, rule 2 order 1, rule 1 regular (not targeted); rule 3 unknown
    let mut scores = vec![own(0, Some(2)), own(1, Some(0)), own(2, Some(1))];
    let (t, unscored) = pole_targets(&rules, &rec, &p, &scores);
    assert_eq!(
        t.iter().map(|x| x.0).collect::<Vec<_>>(),
        vec![0, 2],
        "worst pole order first"
    );
    assert!(unscored.is_empty());
    assert!(t[0].1.iter().all(|(_, s)| *s == PoleStep::Generate));
    // rule 0: `rung` was generated in an earlier run (no score yet): awaits; `tight` has a score: decide
    let k0 = rules[0].key();
    rec.rules
        .iter_mut()
        .find(|e| e.key == k0)
        .unwrap()
        .tried
        .push(Tried {
            cand: "rung".into(),
            scope: "rule".into(),
            run: 1,
            args: vec![],
            status: "complete".into(),
            monomials: Some(5),
            bytes: Some(5),
            cpu_ms: Some(1),
            verdict: "pole_score_pending".into(),
            table: Some("/x/rung.jsonl".into()),
            pole: None,
        });
    let mut sc = own(0, Some(0));
    sc.cand = "tight".into();
    sc.table = Some(FileEntry {
        path: "/x/tight.jsonl".into(),
        sha256: "0".repeat(64),
    });
    scores.push(sc);
    let (t, _) = pole_targets(&rules, &rec, &p, &scores);
    let steps: BTreeMap<String, PoleStep> = t[0].1.iter().cloned().collect();
    assert_eq!(steps["rung"], PoleStep::AwaitScore);
    assert!(matches!(steps["tight"], PoleStep::Decide(_)));
    assert_eq!(steps["rung_tight"], PoleStep::Generate);
    // once decided (scope rule_poles) a candidate is done: nothing repeats
    for c in ["rung", "tight", "rung_tight"] {
        rec.rules
            .iter_mut()
            .find(|e| e.key == k0)
            .unwrap()
            .tried
            .push(Tried {
                cand: c.into(),
                scope: "rule_poles".into(),
                run: 2,
                args: vec![],
                status: "scored".into(),
                monomials: None,
                bytes: None,
                cpu_ms: None,
                verdict: "poles_not_better".into(),
                table: None,
                pole: None,
            });
    }
    let (t, _) = pole_targets(&rules, &rec, &p, &scores);
    assert_eq!(t.iter().map(|x| x.0).collect::<Vec<_>>(), vec![2]);
    // a rule without its own score is not targeted and is reported
    let (_, unscored) = pole_targets(&rules, &rec, &p, &scores[1..]);
    assert_eq!(unscored, vec![0]);
}

#[test]
fn rechoice_of_a_finished_choice() {
    let full = |acc: bool, cost: f64, bytes: u64, cpu: u64| {
        json!([{"acceptable": acc, "cpu_ms": cpu, "measure": {"use_cost_ns": cost, "table_bytes": bytes,
            "rules": 10, "max_monomials": 5}}])
    };
    let ch = json!({"node": "T9", "select": "use_cost", "winner": "b",
        "candidates": [{"index": 0, "id": "a", "full": full(true, 300.0, 3000, 50)},
                       {"index": 1, "id": "b", "full": full(true, 100.0, 1000, 70)},
                       {"index": 2, "id": "c", "full": full(false, 10.0, 10, 1)}]});
    let tl = |id: &str, m: u32| ScoreLine {
        schema: SCORE_SCHEMA.into(),
        node: "T9".into(),
        scope: "table".into(),
        cand: id.into(),
        rule_index: None,
        table: None,
        score: score(Some(m), 5),
        source: None,
    };
    // a has max order 1, b max order 2; a is 3x b: tolerance 2 keeps b, tolerance 4 takes a
    let s = [tl("a", 1), tl("b", 2)];
    let v = rechoice(&ch, &s, &pol("2")).unwrap().unwrap();
    assert_eq!(v["decision"]["winner"], 1);
    assert_eq!(v["decision"]["reason"], "size_guard");
    let v = rechoice(&ch, &s, &pol("4")).unwrap().unwrap();
    assert_eq!(
        (
            v["decision"]["winner"].clone(),
            v["decision"]["changed"].clone()
        ),
        (json!(0), json!(true))
    );
    assert_eq!(
        v["candidates"].as_array().unwrap().len(),
        2,
        "only acceptable full tables"
    );
    // b unscored: undecided, never zero: a ranks first; within tolerance 4 a wins, with 2 a is rejected for size
    let v = rechoice(&ch, &s[..1], &pol("4")).unwrap().unwrap();
    assert_eq!(v["decision"]["winner"], 0);
    assert_eq!(v["decision"]["undecided"], json!([1]));
    assert_eq!(v["candidates"][1]["score"], "undecided");
    let v = rechoice(&ch, &s[..1], &pol("2")).unwrap().unwrap();
    assert_eq!(
        (
            v["decision"]["winner"].clone(),
            v["decision"]["reason"].clone()
        ),
        (json!(1), json!("size_guard"))
    );
}
