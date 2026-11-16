//! Lane shipecon: the pure ship-economy library (`economy::ship`): policy, census, candidacy, guard, decision, the
//! iterable record (reuse planning), the shipped spec and the closing-check comparison. Test data only (no node names).

use rustred_campaign_core::economy::ship::*;
use rustred_campaign_core::import::TableSetSpec;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn rule(label: i64, sha: &str) -> RuleRef {
    RuleRef {
        source: "t".into(),
        table: "t".into(),
        label,
        ordinal: label as usize,
        record_sha256: sha.into(),
        table_sha256: Some("tab".into()),
    }
}

/// (label, record sha256, bytes, served keys with their in-start flag)
type CensusRow<'a> = (i64, &'a str, u64, Vec<(Vec<i32>, bool)>);

fn census_of(rows: &[CensusRow], cap: u64) -> Census {
    let mut b = CensusBuilder::new(
        rows.iter()
            .map(|(l, s, by, _)| (rule(*l, s), *by))
            .collect(),
        cap,
    )
    .unwrap();
    for (_, s, _, keys) in rows {
        for (k, st) in keys {
            b.add(k, *st, Some(s));
        }
    }
    b.add(&[9, 9], true, None);
    b.finish()
}

fn trial(key: &[i32], step: &str, ok: bool, secs: f64, bytes: u64, p: &ShipPolicy) -> KeyTrial {
    KeyTrial {
        key: key.to_vec(),
        step: step.into(),
        recipe_id: format!("rec-{step}"),
        recipe_sha256: "rs".into(),
        recipe_digest: Some("rd".into()),
        verdict: if ok {
            "SOLVED".into()
        } else {
            "NO_RELATION_IN_SEEDS".into()
        },
        certified: ok,
        relation_digest: ok.then(|| format!("rel-{key:?}")),
        seconds: Some(secs),
        rhs_terms: Some(10),
        entry_bytes: Some(bytes),
        pole_order_d4: Some(0),
        rhs_outside_certified: Some(0),
        why: (!ok).then(|| "no relation".into()),
        bound: p.bound_value(),
        solved_line: None,
        results: None,
    }
}

fn policy() -> ShipPolicy {
    let mut p = ShipPolicy::default();
    p.recipe.steps = vec!["d2_clast_fon_mM1C".into(), "d2_cunion_fon_mM1C".into()];
    p.bars.min_bytes_saved = 1000;
    p.bars.min_bytes_saved_per_second = 100.0;
    p.candidates.min_rule_bytes = 1000;
    p
}

#[test]
fn policy_defaults_parse_and_refuse_unknown_fields() {
    let p = ShipPolicy::parse(r#"{"schema":"rustred.campaign.shipecon.v1"}"#).unwrap();
    assert_eq!(p, ShipPolicy::default());
    assert_eq!(p.bars.max_recipe_seconds_per_key, 10.0);
    assert_eq!(p.bars.max_recipe_to_table_ratio, None);
    assert_eq!(p.mode, ShipMode::Report);
    assert!(
        ShipPolicy::parse(r#"{"schema":"rustred.campaign.shipecon.v1","bars":{"max_secs":1}}"#)
            .is_err()
    );
    assert!(ShipPolicy::parse(r#"{"schema":"other"}"#).is_err());
    assert!(
        ShipPolicy::parse(
            r#"{"schema":"rustred.campaign.shipecon.v1","recipe":{"steps":["bogus"]}}"#
        )
        .is_err()
    );
    assert!(
        ShipPolicy::parse(
            r#"{"schema":"rustred.campaign.shipecon.v1","bars":{"max_recipe_to_table_ratio":0}}"#
        )
        .is_err()
    );
    // the decision digest moves with a bar, not with the mode
    let mut q = p.clone();
    q.mode = ShipMode::Apply;
    assert_eq!(p.decision_digest(), q.decision_digest());
    assert_ne!(p.digest(), q.digest());
    q.bars.max_recipe_seconds_per_key = 20.0;
    assert_ne!(p.decision_digest(), q.decision_digest());
}

#[test]
fn census_counts_and_key_cap() {
    let c = census_of(
        &[
            (0, "a", 5000, vec![(vec![1, 0], true), (vec![2, 0], false)]),
            (
                1,
                "b",
                7000,
                vec![(vec![1, 1], true), (vec![2, 1], true), (vec![3, 1], true)],
            ),
            (2, "c", 100, vec![]),
        ],
        2,
    );
    assert_eq!(c.rules[0].served_in_start, 1);
    assert_eq!(c.rules[0].served_outside_start, 1);
    assert!(c.rules[0].keys_complete);
    assert_eq!(c.rules[1].served(), 3);
    assert!(!c.rules[1].keys_complete, "over the cap: keys dropped");
    assert!(c.rules[1].keys.is_empty());
    assert_eq!(c.rows_not_rule, 1);
    assert_eq!(c.total_bytes, 12100);
    let t = c.totals();
    assert_eq!(t["serving_no_certified_key"]["rules"], 1);
    assert_eq!(t["serving_start_keys"]["bytes"], 12000);
    assert!(CensusBuilder::new(vec![(rule(0, "x"), 1), (rule(1, "x"), 1)], 1).is_err());
}

#[test]
fn candidacy_classes_and_budget_largest_first() {
    let mut p = policy();
    p.candidates.max_keys_per_rule = 2;
    p.candidates.max_guard_keys = 3;
    let c = census_of(
        &[
            (0, "a", 5000, vec![(vec![1, 0], true), (vec![2, 0], false)]),
            (1, "b", 9000, vec![(vec![1, 1], true), (vec![2, 1], true)]),
            (2, "c", 100, vec![]),
            (3, "d", 500, vec![(vec![1, 3], true)]),
            (
                4,
                "e",
                8000,
                vec![(vec![1, 4], true), (vec![2, 4], true), (vec![3, 4], true)],
            ),
        ],
        2,
    );
    let k = candidacy(&p, &c);
    assert_eq!(k[2], Candidacy::KeptAside);
    assert!(
        matches!(&k[3], Candidacy::NotExamined { reason } if reason.contains("min_rule_bytes"))
    );
    assert!(
        matches!(&k[4], Candidacy::NotExamined { reason } if reason.contains("max_keys_per_rule"))
    );
    // budget 3 keys: the largest (b, 2 keys) first, then a (2 keys) does not fit
    assert_eq!(k[1], Candidacy::Examine);
    assert!(
        matches!(&k[0], Candidacy::NotExamined { reason } if reason.contains("max_guard_keys"))
    );
    p.kept_aside.enabled = false;
    assert!(matches!(
        &candidacy(&p, &c)[2],
        Candidacy::NotExamined { .. }
    ));
}

fn one_rule(bytes: u64, keys: &[&[i32]]) -> RuleService {
    let c = census_of(
        &[(
            0,
            "a",
            bytes,
            keys.iter().map(|k| (k.to_vec(), true)).collect(),
        )],
        64,
    );
    c.rules[0].clone()
}

#[test]
fn guard_takes_the_first_certifying_step_and_lists_failures() {
    let p = policy();
    let svc = one_rule(10_000, &[&[1, 0], &[2, 0]]);
    let t = vec![
        trial(&[1, 0], "d2_clast_fon_mM1C", false, 1.0, 0, &p),
        trial(&[1, 0], "d2_cunion_fon_mM1C", true, 2.0, 100, &p),
        trial(&[2, 0], "d2_clast_fon_mM1C", true, 1.0, 100, &p),
        trial(&[2, 0], "d2_cunion_fon_mM1C", true, 9.0, 100, &p),
    ];
    let g = guard(&p, &svc, &t);
    assert!(g.holds);
    assert_eq!(g.keys[0].step, "d2_cunion_fon_mM1C");
    assert_eq!(g.keys[1].step, "d2_clast_fon_mM1C");
    assert!(!g.digest.is_empty());
    let g2 = guard(&p, &svc, &t[..3]);
    assert!(g2.holds);
    let g3 = guard(&p, &svc, &t[..1]);
    assert!(!g3.holds);
    assert_eq!(g3.failed.len(), 2);
    assert!(g3.failed[0].1.contains("no relation"));
    assert!(g3.failed[1].1.contains("not_tried"));
}

fn tt(load: f64, apply: f64) -> TableTime {
    let mut a = BTreeMap::new();
    a.insert(String::new(), apply);
    TableTime {
        measured: true,
        load_s: load,
        apply_s: a,
        instantiated_bytes: BTreeMap::new(),
        tool: None,
    }
}

#[test]
fn decide_recipe_stored_mixed_and_table() {
    let p = policy();
    let svc = one_rule(1_000_000, &[&[1, 0], &[2, 0]]);
    let rb = BTreeMap::new();
    // both fast: recipe
    let t = vec![
        trial(&[1, 0], "d2_clast_fon_mM1C", true, 2.0, 50_000, &p),
        trial(&[2, 0], "d2_clast_fon_mM1C", true, 3.0, 50_000, &p),
    ];
    let d = decide(
        &p,
        &svc,
        &guard(&p, &svc, &t),
        &tt(1.0, 0.01),
        &rb,
        &BTreeSet::new(),
    );
    assert_eq!(d.outcome, Outcome::Recipe, "{}", d.why);
    assert_eq!(d.seconds_added_total, 5.0);
    assert!(d.bytes_saved > 990_000);
    assert!(d.keys[0].ratio_recipe_to_table.unwrap() > 1.0);
    // one over the bar: mixed (stored at that key)
    let t2 = vec![
        t[0].clone(),
        trial(&[2, 0], "d2_clast_fon_mM1C", true, 30.0, 50_000, &p),
    ];
    let d = decide(
        &p,
        &svc,
        &guard(&p, &svc, &t2),
        &tt(1.0, 0.01),
        &rb,
        &BTreeSet::new(),
    );
    assert_eq!(d.outcome, Outcome::Mixed, "{}", d.why);
    assert_eq!(d.keys[1].form, "stored");
    // all over the bar: stored relations (no seconds added)
    let t3 = vec![
        trial(&[1, 0], "d2_clast_fon_mM1C", true, 40.0, 50_000, &p),
        trial(&[2, 0], "d2_clast_fon_mM1C", true, 30.0, 50_000, &p),
    ];
    let d = decide(
        &p,
        &svc,
        &guard(&p, &svc, &t3),
        &tt(1.0, 0.01),
        &rb,
        &BTreeSet::new(),
    );
    assert_eq!(d.outcome, Outcome::StoredRelations, "{}", d.why);
    assert_eq!(d.seconds_added_total, 0.0);
    // stored relations too large for the ratio: table
    let t4 = vec![
        trial(&[1, 0], "d2_clast_fon_mM1C", true, 40.0, 400_000, &p),
        trial(&[2, 0], "d2_clast_fon_mM1C", true, 30.0, 400_000, &p),
    ];
    let d = decide(
        &p,
        &svc,
        &guard(&p, &svc, &t4),
        &tt(1.0, 0.01),
        &rb,
        &BTreeSet::new(),
    );
    assert_eq!(d.outcome, Outcome::Table);
    assert!(d.why.contains("max_bytes_ratio"), "{}", d.why);
    // no stored relations allowed and over the bar: table
    let mut q = p.clone();
    q.stored.enabled = false;
    let d = decide(
        &q,
        &svc,
        &guard(&q, &svc, &t3),
        &tt(1.0, 0.01),
        &rb,
        &BTreeSet::new(),
    );
    assert_eq!(d.outcome, Outcome::Table);
    // the relative bar
    let mut r = p.clone();
    r.bars.max_recipe_to_table_ratio = Some(10.0);
    let d = decide(
        &r,
        &svc,
        &guard(&r, &svc, &t),
        &tt(0.0, 0.1),
        &rb,
        &BTreeSet::new(),
    );
    assert_eq!(
        d.outcome,
        Outcome::StoredRelations,
        "ratio 20-30 > 10: {}",
        d.why
    );
    // guard failure: table
    let d = decide(
        &p,
        &svc,
        &guard(&p, &svc, &t[..1]),
        &tt(1.0, 0.01),
        &rb,
        &BTreeSet::new(),
    );
    assert_eq!(d.outcome, Outcome::Table);
    assert!(d.why.starts_with("guard"));
    // economy: the recipe does not pay its seconds against the stored relation -> stored (a first-class kind)
    let mut e = p.clone();
    e.bars.min_bytes_saved_per_second = 1e9;
    let d = decide(
        &e,
        &svc,
        &guard(&e, &svc, &t),
        &tt(1.0, 0.01),
        &rb,
        &BTreeSet::new(),
    );
    assert_eq!(d.outcome, Outcome::StoredRelations, "{}", d.why);
    // ... and without stored relations the recipe migration fails the per-second bar -> table
    e.stored.enabled = false;
    let d = decide(
        &e,
        &svc,
        &guard(&e, &svc, &t),
        &tt(1.0, 0.01),
        &rb,
        &BTreeSet::new(),
    );
    assert_eq!(d.outcome, Outcome::Table);
    assert!(d.why.contains("per second"), "{}", d.why);
}

#[test]
fn coverage_rules_later_table_rule_rhs_outside_and_full_target() {
    let p = policy();
    let svc = one_rule(1_000_000, &[&[1, 0], &[2, 0]]);
    let rb = BTreeMap::new();
    let t = vec![
        trial(&[1, 0], "d2_clast_fon_mM1C", true, 40.0, 50_000, &p),
        trial(&[2, 0], "d2_clast_fon_mM1C", true, 40.0, 50_000, &p),
    ];
    // a later table rule takes [2,0] (after_table): only [1,0] is stored
    let taken: BTreeSet<Vec<i32>> = [vec![2, 0]].into_iter().collect();
    let d = decide(&p, &svc, &guard(&p, &svc, &t), &tt(1.0, 0.01), &rb, &taken);
    assert_eq!(d.outcome, Outcome::StoredRelations, "{}", d.why);
    assert_eq!(d.keys[1].form, "later_table_rule");
    assert_eq!(d.bytes_after, 50_000);
    // every key taken by later rules: the rule leaves with nothing shipped in its place
    let all: BTreeSet<Vec<i32>> = [vec![1, 0], vec![2, 0]].into_iter().collect();
    let d = decide(&p, &svc, &guard(&p, &svc, &t), &tt(1.0, 0.01), &rb, &all);
    assert_eq!(d.outcome, Outcome::KeptAside, "{}", d.why);
    // over_table: entries take their keys before any table rule
    let mut o = p.clone();
    o.stored.precedence = Precedence::OverTable;
    let d = decide(&o, &svc, &guard(&o, &svc, &t), &tt(1.0, 0.01), &rb, &all);
    assert_eq!(d.outcome, Outcome::StoredRelations, "{}", d.why);
    // a relation leaving the certified set keeps the rule
    let mut t2 = t.clone();
    t2[0].rhs_outside_certified = Some(3);
    let d = decide(
        &p,
        &svc,
        &guard(&p, &svc, &t2),
        &tt(1.0, 0.01),
        &rb,
        &BTreeSet::new(),
    );
    assert_eq!(d.outcome, Outcome::Table);
    assert!(d.why.contains("certified set"), "{}", d.why);
    let mut q = p.clone();
    q.coverage.require_rhs_in_certified_set = false;
    let d = decide(
        &q,
        &svc,
        &guard(&q, &svc, &t2),
        &tt(1.0, 0.01),
        &rb,
        &BTreeSet::new(),
    );
    assert_eq!(d.outcome, Outcome::StoredRelations);
    // a full-coverage node: nothing leaves the shipped table
    let mut f = p.clone();
    f.coverage.target = Target::Full;
    let d = decide(
        &f,
        &svc,
        &guard(&f, &svc, &t),
        &tt(1.0, 0.01),
        &rb,
        &BTreeSet::new(),
    );
    assert_eq!(d.outcome, Outcome::Table);
    let c = census_of(
        &[
            (0, "a", 5000, vec![]),
            (1, "b", 9000, vec![(vec![1, 1], true)]),
        ],
        4,
    );
    assert!(
        candidacy(&f, &c)
            .iter()
            .all(|x| matches!(x, Candidacy::NotExamined { .. }))
    );
    // over_table puts the stored relation sets BEFORE the tables in the shipped spec
    let r = ShippedRecipe {
        id: "shipecon:stored:r".into(),
        path: "r.json".into(),
        sha256: "00".into(),
        form: "stored".into(),
        assignment: None,
    };
    let s = shipped_spec(
        &spec(),
        &[],
        std::slice::from_ref(&r),
        &[],
        Precedence::OverTable,
    )
    .unwrap();
    assert_eq!(s.sources[0].id, "shipecon:stored:r");
    let s = shipped_spec(
        &spec(),
        &[],
        std::slice::from_ref(&r),
        &[],
        Precedence::AfterTable,
    )
    .unwrap();
    assert_eq!(s.sources.last().unwrap().id, "shipecon:stored:r");
}

#[test]
fn unexamined_and_revert() {
    let svc = one_rule(5000, &[]);
    let d = decide_unexamined(&svc, &Candidacy::KeptAside);
    assert_eq!(d.outcome, Outcome::KeptAside);
    assert_eq!(d.bytes_saved, 5000);
    let mut d2 = d.clone();
    revert(&mut d2, "x");
    assert_eq!(d2.outcome, Outcome::Table);
    assert_eq!(d2.bytes_saved, 0);
    assert!(d2.why.contains("reverted"));
}

fn record(p: &ShipPolicy, svc: &RuleService, trials: Vec<KeyTrial>) -> EconomyRecord {
    let g = guard(p, svc, &trials);
    let d = decide(
        p,
        svc,
        &g,
        &tt(1.0, 0.01),
        &BTreeMap::new(),
        &BTreeSet::new(),
    );
    EconomyRecord {
        schema: ECONOMY_RECORD_SCHEMA.into(),
        kind: RECORD_KIND.into(),
        node: "n".into(),
        subject: svc.rule.clone(),
        inputs_digest: inputs_digest(svc),
        decision_digest: p.decision_digest(),
        policy_digest: p.digest(),
        trials,
        table_time: None,
        not_tried: not_tried(p, &d, Some(&g)),
        decision: d,
        guard: Some(g),
        revert: Value::Null,
    }
}

#[test]
fn iteration_unchanged_continue_fresh() {
    let p = policy();
    let svc = one_rule(1_000_000, &[&[1, 0], &[2, 0]]);
    assert_eq!(plan(&p, &svc, None), Reuse::Fresh);
    let mut b = trial(&[2, 0], "d2_clast_fon_mM1C", false, 120.0, 0, &p);
    b.verdict = "BUDGET_WALL".into();
    let rec = record(
        &p,
        &svc,
        vec![trial(&[1, 0], "d2_clast_fon_mM1C", true, 2.0, 100, &p), b],
    );
    // same settings, same inputs: nothing to do
    assert_eq!(plan(&p, &svc, Some(&rec)), Reuse::Unchanged);
    // a larger wall bound: the budget-stopped trial is not reused, the certified one is
    let mut q = p.clone();
    q.recipe.key_budget_s = Some(600);
    match plan(&q, &svc, Some(&rec)) {
        Reuse::Continue { trials, to_try } => {
            assert_eq!(trials.len(), 1);
            assert_eq!(to_try, vec![("d2_clast_fon_mM1C".to_string(), vec![2, 0])]);
        }
        other => panic!("{other:?}"),
    }
    // a higher bar only: everything reused, nothing to try for certified keys; the budget trial (other bound? same)
    let mut r = p.clone();
    r.bars.max_recipe_seconds_per_key = 60.0;
    match plan(&r, &svc, Some(&rec)) {
        Reuse::Continue { trials, to_try } => {
            assert_eq!(trials.len(), 2);
            assert_eq!(to_try, vec![("d2_cunion_fon_mM1C".to_string(), vec![2, 0])]);
        }
        other => panic!("{other:?}"),
    }
    // changed inputs (a key more): fresh
    let svc2 = one_rule(1_000_000, &[&[1, 0], &[2, 0], &[3, 0]]);
    assert_eq!(plan(&p, &svc2, Some(&rec)), Reuse::Fresh);
    // the record round-trips
    let txt = serde_json::to_string(&rec).unwrap();
    let back: EconomyRecord = serde_json::from_str(&txt).unwrap();
    assert_eq!(back, rec);
    assert!(rec.not_tried.iter().any(|n| n.what.contains("deeper")));
}

fn spec() -> TableSetSpec {
    TableSetSpec::from_json(
        &json!({"schema": "rustred.campaign.import-spec.v1", "frame_from": "t",
            "start": {"path": "s.jsonl", "format": "keys_jsonl"},
            "sources": [{"id": "t", "kind": "engine_table", "path": "t.jsonl",
                         "removed": [{"label": 7, "reason": "earlier"}]},
                        {"id": "u", "kind": "engine_table", "path": "u.jsonl"}]})
        .to_string(),
    )
    .unwrap()
}

#[test]
fn shipped_spec_removes_rules_and_appends_relation_sets_last() {
    let p = policy();
    let svc = one_rule(1_000_000, &[&[1, 0]]);
    let t = vec![trial(&[1, 0], "d2_clast_fon_mM1C", true, 2.0, 100, &p)];
    let d = decide(
        &p,
        &svc,
        &guard(&p, &svc, &t),
        &tt(1.0, 0.01),
        &BTreeMap::new(),
        &BTreeSet::new(),
    );
    assert!(d.outcome.migrates());
    let keep = decide_unexamined(
        &one_rule(10, &[&[5, 5]]),
        &Candidacy::NotExamined { reason: "x".into() },
    );
    let r = ShippedRecipe {
        id: "shipecon:r1".into(),
        path: "r1.json".into(),
        sha256: "00".into(),
        form: "recipe".into(),
        assignment: Some(rustred_campaign_core::import::spec::FileRef {
            path: "a.jsonl".into(),
            sha256: None,
            manifest: None,
            family: None,
        }),
    };
    let s = shipped_spec(
        &spec(),
        &[d.clone(), keep],
        std::slice::from_ref(&r),
        &[],
        Precedence::AfterTable,
    )
    .unwrap();
    assert_eq!(s.sources[0].removed.len(), 2);
    assert_eq!(s.sources[0].removed[0].label, 0);
    assert!(s.sources[0].removed[0].reason.starts_with("shipecon"));
    assert!(s.sources[1].removed.is_empty());
    let last = s.sources.last().unwrap();
    assert_eq!(last.id, "shipecon:r1");
    assert!(last.assignment.is_some());
    // the spec re-parses (strict field lists)
    TableSetSpec::from_json(&spec_json(&s).to_string()).unwrap();
    // a duplicate source id / an unknown rule source: refused
    let mut r2 = r.clone();
    r2.id = "u".into();
    assert!(
        shipped_spec(
            &spec(),
            std::slice::from_ref(&d),
            &[r2],
            &[],
            Precedence::AfterTable
        )
        .is_err()
    );
    let mut d2 = d.clone();
    d2.rule.source = "zz".into();
    assert!(shipped_spec(&spec(), &[d2], &[], &[], Precedence::AfterTable).is_err());
}

#[test]
fn closing_check_compare() {
    let before = ClosureView {
        keys: vec![vec![1], vec![2], vec![3]],
        start: vec![vec![1]],
        terminals: vec![vec![3]],
        open: 0,
        open_keys: vec![],
        blocking: vec![],
    };
    let mut after = before.clone();
    after.keys.push(vec![4]);
    let c = ClosingCheck::compare(&before, &after);
    assert!(c.holds && c.closed);
    assert_eq!(c.new_keys, 1);
    after.terminals.push(vec![4]);
    assert!(!ClosingCheck::compare(&before, &after).holds);
    let mut a2 = before.clone();
    a2.open = 1;
    a2.blocking = vec!["import_open_keys:1".into()];
    let c = ClosingCheck::compare(&before, &a2);
    assert!(!c.holds);
    assert_eq!(c.new_blocking, vec!["import_open_keys:1".to_string()]);
    // a blocking word the original had already is not new
    let mut b2 = before.clone();
    b2.blocking = vec!["below_required_evidence:5".into()];
    let mut a3 = b2.clone();
    a3.blocking = vec!["below_required_evidence:7".into()];
    assert!(ClosingCheck::compare(&b2, &a3).holds);
}

#[test]
fn totals_report_bytes_seconds_and_guard_failures() {
    let p = policy();
    let svc = one_rule(1_000_000, &[&[1, 0], &[2, 0]]);
    let t = vec![
        trial(&[1, 0], "d2_clast_fon_mM1C", true, 2.0, 200_000, &p),
        trial(&[2, 0], "d2_clast_fon_mM1C", true, 4.0, 200_000, &p),
    ];
    let d = decide(
        &p,
        &svc,
        &guard(&p, &svc, &t),
        &tt(1.0, 0.01),
        &BTreeMap::new(),
        &BTreeSet::new(),
    );
    let c = census_of(
        &[(
            0,
            "a",
            1_000_000,
            vec![(vec![1, 0], true), (vec![2, 0], true)],
        )],
        64,
    );
    let v = totals(&c, std::slice::from_ref(&d));
    assert_eq!(v["bytes_before"], 1_000_000);
    assert_eq!(v["bytes_saved"], d.bytes_saved);
    assert_eq!(v["recipe_seconds_per_key"]["keys"], 2);
    assert_eq!(v["recipe_seconds_per_key"]["max"], 4.0);
    let g = guard(&p, &svc, &t[..1]);
    let d2 = decide(
        &p,
        &svc,
        &g,
        &tt(1.0, 0.01),
        &BTreeMap::new(),
        &BTreeSet::new(),
    );
    assert_eq!(totals(&c, &[d2])["guard_failed_rules"], 1);
}

#[test]
fn served_key_sets_refuse_a_lost_key_and_a_lower_evidence_level() {
    use rustred_campaign_core::import::Level;
    let before: BTreeMap<Vec<i32>, ServedAt> = [
        (vec![1, 1, 0], ServedAt::Served(Level::PerKey)),
        (vec![2, 1, 0], ServedAt::Served(Level::Imported)),
        (vec![1, 0, 0], ServedAt::Terminal),
    ]
    .into_iter()
    .collect();
    // the same service (and a higher level) at every key, and keys the original did not have: holds
    let mut after = before.clone();
    after.insert(vec![2, 1, 0], ServedAt::Served(Level::PerKey));
    after.insert(vec![5, 5, 0], ServedAt::Served(Level::Imported));
    let c = compare_served(&before, &after);
    assert!(c.holds, "{c:?}");
    assert_eq!((c.original, c.kept), (3, 3));
    // a key no longer served: refused, named
    let mut a2 = before.clone();
    a2.remove(&vec![1, 1, 0]);
    let c = compare_served(&before, &a2);
    assert!(!c.holds);
    assert_eq!(c.lost, vec![vec![1, 1, 0]]);
    // a lower evidence level: refused
    let mut a3 = before.clone();
    a3.insert(vec![1, 1, 0], ServedAt::Served(Level::Imported));
    let c = compare_served(&before, &a3);
    assert!(!c.holds);
    assert_eq!(c.lower.len(), 1);
    // a terminal turned into a served key is a different service: refused
    let mut a4 = before.clone();
    a4.insert(vec![1, 0, 0], ServedAt::Served(Level::PerKey));
    assert_eq!(compare_served(&before, &a4).lost, vec![vec![1, 0, 0]]);
}

#[test]
fn dead_entries_are_never_shipped() {
    let p = policy();
    let svc = one_rule(1_000_000, &[&[1, 0], &[2, 0]]);
    let t = vec![
        trial(&[1, 0], "d2_clast_fon_mM1C", true, 40.0, 50_000, &p),
        trial(&[2, 0], "d2_clast_fon_mM1C", true, 40.0, 50_000, &p),
    ];
    let mut d = decide(
        &p,
        &svc,
        &guard(&p, &svc, &t),
        &tt(1.0, 0.01),
        &BTreeMap::new(),
        &BTreeSet::new(),
    );
    assert_eq!(d.outcome, Outcome::StoredRelations);
    assert_eq!(d.bytes_after, 100_000);
    // the shipped dispatch gives [2,0] to a table rule: its entry is dead -> not shipped
    let one: BTreeSet<Vec<i32>> = [vec![2, 0]].into_iter().collect();
    assert_eq!(drop_dead_entries(&mut d, &one), 1);
    assert_eq!(d.keys[1].form, "later_table_rule");
    assert_eq!(d.bytes_after, 50_000);
    assert_eq!(d.outcome, Outcome::StoredRelations);
    // every key taken: nothing shipped in the rule's place
    let all: BTreeSet<Vec<i32>> = [vec![1, 0], vec![2, 0]].into_iter().collect();
    drop_dead_entries(&mut d, &all);
    assert_eq!(d.outcome, Outcome::KeptAside);
    assert_eq!(d.bytes_after, 0);
    // a table decision is never touched
    let mut tbl = decide_unexamined(&svc, &Candidacy::NotExamined { reason: "x".into() });
    assert_eq!(drop_dead_entries(&mut tbl, &all), 0);
}
