//! lane rulecost (interfaces/rule-cost.md): the pure parts of the application-cost tool and the `rule_cost` measure
//! of improve. Test data only: a five-slot toy owner (lines = slots 0..3, one numerator slot 4) and hand-written probe
//! lines in the probe's format.
use rustred_campaign_core::economy::genchoice::CostModel;
use rustred_campaign_core::economy::improve::{
    ImprovePolicy, RankBy, TableRule, replacement_verdict_eq, size_of, subtree_verdict_eq,
};
use rustred_campaign_core::rulecost::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

const SUPPORT: u64 = 0b01111;

fn rules_output() -> String {
    let header = json!({"tool": "p_rule_cost rules", "schema": PROBE_RULES_SCHEMA, "package": "/pkg", "class": "00000000000000ab",
        "presentation": 7, "arity": 5, "support": "f", "charts": ["f"], "rules": 2, "prepare_seconds": 0.5, "rss_kb": 1});
    let r0 = json!({"chart": "f", "batch": 0, "position": 0, "ordinal": 0, "partition": true, "fixed": [],
        "equalities": 0, "exception_branches": 1, "exception_conditions": 1, "rhs_terms": 3,
        "distinct_coefficients": 3, "distinct_original_denominators": 1, "base_variables": 1,
        "numerator": {"terms": 9, "max_degree_base": 2, "max_degree_index": 1, "max_total_degree_indices": 2, "max_bits": 5},
        "denominator": {"terms": 3}, "original_denominator": {"terms": 3}, "shifts": [[-1,0,0,0,0],[0,-1,0,0,0],[0,0,0,0,1]]});
    let r1 = json!({"chart": "f", "batch": 0, "position": 1, "ordinal": 1, "partition": true, "fixed": [[0, 1], [4, -1]],
        "rhs_terms": 2, "numerator": {"terms": 4}, "denominator": {"terms": 2}, "shifts": [[0,-1,0,0,0],[0,0,-1,0,0]]});
    format!("{header}\n{r0}\n{r1}\n")
}

fn key_line(
    key: [i64; 5],
    position: usize,
    secs: f64,
    children: u64,
    profile: Option<f64>,
) -> Value {
    let mut v = json!({"key": key, "via": {"chart": "f", "position": position}, "seconds": [secs, secs * 1.5],
        "seconds_min": secs, "seconds_median": secs * 1.25, "children": children, "children_same_owner": 1,
        "receivers": [{"owner": "00000000000000ab", "lane": "same-owner", "lines": 4, "children": 1},
                      {"owner": "L1T03-00000000000000cd", "lane": "Insertion", "lines": 3, "children": children - 1}],
        "row": {"numerator": {"terms": 6, "max_degree_base": 3, "max_bits": 40}, "denominator": {"terms": 2}}});
    if let Some(wall) = profile {
        v["profile"] = json!({"level": 2, "wall_s": wall, "categories": {
            "arith.coefficient-normalise": {"s": wall * 0.75, "n": 3}, "fetch.custom-applicability-denominators": {"s": wall * 0.25, "n": 1}}});
    }
    v
}

fn keys_output() -> String {
    let header =
        json!({"tool": "p_rule_cost keys", "schema": PROBE_KEYS_SCHEMA, "keys": 5, "repeat": 2});
    let lines = [
        key_line([2, 1, 1, 1, 0], 0, 0.010, 3, Some(0.012)),
        key_line([1, 2, 1, 1, -1], 0, 0.030, 3, Some(0.032)),
        key_line([1, 1, 3, 1, 0], 0, 0.020, 3, None),
        key_line([1, 1, 1, 2, -1], 1, 0.002, 2, None),
        json!({"key": [9, 9, 9, 9, 9], "error": "outside the support"}),
        json!({"key": [1, 1, 1, 1, 0], "via": "terminal", "seconds_min": 0.0}),
    ];
    let mut s = format!("{header}\n");
    for l in lines {
        s.push_str(&format!("{l}\n"));
    }
    s
}

#[test]
fn probe_outputs_parse_and_a_foreign_schema_is_refused() {
    let (h, rules): (RulesHeader, Vec<ProbeRule>) =
        parse_probe(&rules_output(), PROBE_RULES_SCHEMA).unwrap();
    assert_eq!((h.arity, h.support.as_str(), rules.len()), (5, "f", 2));
    assert_eq!(
        rules[0].id().unwrap(),
        RuleId {
            chart: 0xf,
            position: 0
        }
    );
    assert_eq!(rules[0].coefficient_monomials(), 12);
    assert_eq!(rules[1].fixed, vec![(0, 1), (4, -1)]);
    let (k, keys): (KeysHeader, Vec<ProbeKey>) =
        parse_probe(&keys_output(), PROBE_KEYS_SCHEMA).unwrap();
    assert_eq!((k.keys, k.repeat, keys.len()), (5, 2, 6));
    assert_eq!(
        keys[3].rule(),
        Some(RuleId {
            chart: 0xf,
            position: 1
        })
    );
    assert_eq!(keys[4].rule(), None);
    assert_eq!(keys[5].rule(), None);
    assert!(parse_probe::<KeysHeader, ProbeKey>(&rules_output(), PROBE_KEYS_SCHEMA).is_err());
    assert!(parse_probe::<KeysHeader, ProbeKey>("", PROBE_KEYS_SCHEMA).is_err());
}

#[test]
fn visits_count_every_term_today_and_each_distinct_coefficient_with_a_cache() {
    let (_, rules): (RulesHeader, Vec<ProbeRule>) =
        parse_probe(&rules_output(), PROBE_RULES_SCHEMA).unwrap();
    // 2 x (9 + 3) + 3 original-denominator monomials
    assert_eq!(visits_per_term(&rules[0]), 27);
    assert_eq!(visits_distinct(&rules[0]), None);
    let mut r = rules[0].clone();
    r.coefficients = Some(DistinctCoefficients {
        distinct: 2,
        numerator: PolyShape {
            terms: 5,
            ..PolyShape::default()
        },
        denominator: PolyShape {
            terms: 1,
            ..PolyShape::default()
        },
        distinct_denominator_terms: 1,
        ..DistinctCoefficients::default()
    });
    assert_eq!(visits_distinct(&r), Some(13));
    let line = serde_json::to_string(&r).unwrap();
    let back: ProbeRule = serde_json::from_str(&line).unwrap();
    assert_eq!(back, r);
}

#[test]
fn face_keys_honour_face_shell_and_signs_and_are_deterministic() {
    let spec = SamplerSpec {
        shells: vec![[0, 0], [2, 1], [3, 2], [0, 5]],
        per_shell: 3,
        seed: 7,
    };
    let fixed = [(0usize, 1i64), (4usize, -1i64)];
    let a = face_keys(5, SUPPORT, &fixed, &spec, 11);
    assert_eq!(a, face_keys(5, SUPPORT, &fixed, &spec, 11), "deterministic");
    // shell [0, 0] cannot hold the fixed rank 1; [0, 5] needs rank 4 on a face without a free numerator slot
    assert!(a.iter().all(|(_, s)| *s == [2, 1] || *s == [3, 2]));
    for (key, shell) in &a {
        assert_eq!((key[0], key[4]), (1, -1), "fixed slots");
        assert!(key[..4].iter().all(|&n| n >= 1));
        assert_eq!(shell_of(key, SUPPORT), *shell);
    }
    let per = |s: [u32; 2]| a.iter().filter(|(_, x)| *x == s).count();
    assert_eq!(per([2, 1]), 3);
    assert!(
        per([3, 2]) == 0,
        "rank 2 needs a free numerator slot: the face fixes the only one"
    );
    // a free face reaches every shell; a fixed line at power 0 is no key of the chart
    let free = face_keys(5, SUPPORT, &[], &spec, 1);
    assert_eq!(free.iter().filter(|(_, s)| *s == [0, 0]).count(), 1);
    assert!(face_keys(5, SUPPORT, &[(1, 0)], &spec, 1).is_empty());
}

#[test]
fn aggregate_gives_cost_per_application_children_per_receiver_and_the_split() {
    let (_, keys): (KeysHeader, Vec<ProbeKey>) =
        parse_probe(&keys_output(), PROBE_KEYS_SCHEMA).unwrap();
    let costs = aggregate(&keys, SUPPORT);
    assert_eq!(costs.len(), 2);
    let c = &costs[&RuleId {
        chart: 0xf,
        position: 0,
    }];
    assert_eq!(c.applications, 3);
    assert!((c.seconds_median - 0.020).abs() < 1e-12);
    assert!((c.seconds_p90 - 0.030).abs() < 1e-12);
    assert!((c.seconds_mean - 0.020).abs() < 1e-12);
    assert!((c.children_median - 3.0).abs() < 1e-12);
    let ins = c.receivers.iter().find(|r| r.lane == "Insertion").unwrap();
    assert_eq!((ins.lines, ins.per_application), (3, 2.0));
    assert_eq!(c.profile_applications, 2);
    assert!((c.profile_share["arith.coefficient-normalise"] - 0.75).abs() < 1e-9);
    assert!((c.profile_seconds["arith.coefficient-normalise"] - 0.75 * 0.022).abs() < 1e-9);
    assert_eq!(c.row_max_bits, 40);
    assert_eq!(
        c.by_shell.len(),
        3,
        "dots 1 rank 0, dots 1 rank 1, dots 2 rank 0"
    );
    assert_eq!(quantile(&[], 0.9), 0.0);
    assert_eq!(
        quantile(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0], 0.9),
        9.0
    );
}

#[test]
fn profile_rules_read_owner_statistics_by_label_or_id() {
    let doc = json!({"owners": [
        {"class": "route", "rules": []},
        {"class": "00000000000000ab", "steps": 9, "rules": [["000000000000000f", 0, 7, 1.5], ["000000000000000f", 1, 2, 0.25]]},
        {"class": "00000000000000cd", "rules": [["0000000000000007", 0, 100, 3.0]]}]});
    let p = profile_rules(&doc, "L1T04-00000000000000ab").unwrap();
    assert_eq!(p.len(), 2);
    assert_eq!(
        p[&RuleId {
            chart: 0xf,
            position: 0
        }],
        RunRule {
            calls: 7,
            seconds: 1.5
        }
    );
    assert_eq!(profile_rules(&doc, "ab").unwrap(), p);
    assert!(profile_rules(&doc, "ef").unwrap().is_empty());
    assert!(profile_rules(&json!({}), "ab").is_err());
    // the profile's charts are owner-presentation masks; the table writes 0/1 strings in its own slot order
    let calls = calls_from_profile(&p, &[1, 0, 2, 3, 4], 5);
    assert_eq!(calls["11110:0"], 7.0);
    let one = BTreeMap::from([(
        RuleId {
            chart: 0b00001,
            position: 3,
        },
        RunRule {
            calls: 5,
            seconds: 0.0,
        },
    )]);
    assert_eq!(
        calls_from_profile(&one, &[1, 0, 2, 3, 4], 5)["01000:3"],
        5.0
    );
    assert_eq!(calls_from_profile(&one, &[], 5)["10000:3"], 5.0);
}

/// A toy table_dump.v1 rule record on the five-slot owner.
fn record(index: i64, fixed: [Option<i64>; 5], rhs: &[(&str, &str)], monomials: u64) -> String {
    json!({"type": "rule", "chart": "11110", "index": index, "case": "c", "fixed": fixed, "equations": [],
        "target": "I(n0,n1,n2,n3,n4)", "rhs": rhs.iter().map(|(i, c)| json!([i, c])).collect::<Vec<_>>(),
        "exceptions": [], "monomials": monomials})
    .to_string()
}

#[test]
fn rule_features_classify_children_at_a_generic_key_of_the_case() {
    let line = record(
        0,
        [Some(1), None, None, None, None],
        &[
            ("I(n0-1,n1,n2,n3,n4)", "-1"),         // fixed n0 = 1: line 0 dropped
            ("I(n0,n1-1,n2,n3,n4+1)", "n1/(d-4)"), // free n1: kept (generic key)
            ("I(0,n1,n2,0,n4)", "(n2+1)/(d-4)"),   // two lines dropped
            ("I(n0,n1,n2,n3,n4)", "2/(2*d-7)"),
            ("I(n0,n1,n2", "1"), // unreadable
        ],
        12,
    );
    let v: Value = serde_json::from_str(&line).unwrap();
    let f = RuleFeatures::from_record(&v).unwrap();
    assert_eq!(f.rhs_terms, 5);
    assert_eq!(f.children_same_sector, 2);
    assert_eq!(f.children_lower, BTreeMap::from([(1, 1), (2, 1)]));
    assert_eq!(f.unread_terms, 1);
    assert_eq!(f.children(), 4);
    assert_eq!(f.distinct_denominators, 3, "1, d-4, 2*d-7");
    // -1 | n1/(d-4) = 1 + 2 | (n2+1)/(d-4) = 1 + 2 + 2 | 2/(2*d-7) = 1 + 2 | 1
    assert_eq!(f.coefficient_monomials, 1 + 3 + 5 + 3 + 1);
    // denominators per term: 1 | (d-4) = 1 + 2 | (d-4) | (2*d-7) | 1
    assert_eq!(f.denominator_monomials, 1 + 3 + 3 + 3 + 1);
    assert_eq!(f.distinct_coefficients, 5);
    assert_eq!(
        f.distinct_denominator_monomials,
        1 + 3 + 3,
        "1, (d-4), (2*d-7)"
    );
    assert_eq!(f.distinct_coefficient_monomials, 13);
    assert_eq!(
        (f.distinct_coefficient_bytes, f.max_coefficient_bytes),
        (32, 12)
    );
    let r = TableRule::from_line(&line).unwrap().unwrap();
    assert_eq!(r.cost, Some(f));
    assert!(RuleFeatures::from_record(&json!({"type": "rule"})).is_none());
}

#[test]
fn rule_cost_measure_weighs_calls_cost_and_children_and_stays_opt_in() {
    // default behaviour unchanged: improve ranks by monomials, the cost model carries no rule-cost model
    assert_eq!(ImprovePolicy::default().filter.by, RankBy::Monomials);
    let plain: CostModel = serde_json::from_value(json!({"load_ns_per_byte": 1.0})).unwrap();
    assert!(plain.rule_cost.is_none());
    assert!(!serde_json::to_string(&plain).unwrap().contains("rule_cost"));

    let heavy = TableRule::from_line(&record(
        3,
        [None; 5],
        &[
            ("I(n0,n1-1,n2,n3,n4)", "a"),
            ("I(n0,n1,n2-1,n3,n4)", "b"),
            ("I(n0,n1,n2,n3-1,n4)", "c"),
        ],
        3000,
    ))
    .unwrap()
    .unwrap();
    let light = TableRule::from_line(&record(3, [None; 5], &[("I(n0,n1-1,n2,n3,n4)", "a")], 1000))
        .unwrap()
        .unwrap();
    let rarely = TableRule::from_line(&record(4, [None; 5], &[("I(n0,n1-1,n2,n3,n4)", "a")], 5000))
        .unwrap()
        .unwrap();
    let model = RuleCostModel {
        apply_ns_base: 0.0,
        apply_ns_per_term: 10.0,
        apply_ns_per_monomial: 1.0,
        apply_ns_per_denominator_monomial: 0.0,
        child_ns_same_sector: 100.0,
        child_ns_lower: 0.0,
        calls: BTreeMap::from([("11110:3".to_string(), 50.0), ("11110:4".to_string(), 1.0)]),
        ..RuleCostModel::default()
    };
    let cost = CostModel {
        rule_cost: Some(model.clone()),
        ..CostModel::default()
    };
    // 50 calls x (3000 + 3 x 10 + 3 x 100) vs 1 call x (5000 + 10 + 100)
    assert_eq!(size_of(&[&heavy], RankBy::RuleCost, &cost), 50 * 3330);
    assert_eq!(size_of(&[&rarely], RankBy::RuleCost, &cost), 5110);
    assert!(
        size_of(&[&heavy], RankBy::RuleCost, &cost) > size_of(&[&rarely], RankBy::RuleCost, &cost)
    );
    assert!(
        size_of(&[&heavy], RankBy::Monomials, &cost)
            < size_of(&[&rarely], RankBy::Monomials, &cost)
    );
    // a cheaper replacement at the same position is accepted, a dearer one refused (both at old's calls)
    assert!(replacement_verdict_eq(&heavy, &light, 1.0, RankBy::RuleCost, &cost, false).is_ok());
    let err =
        replacement_verdict_eq(&light, &heavy, 1.0, RankBy::RuleCost, &cost, false).unwrap_err();
    assert!(err.contains("rule-cost"), "{err}");
    // a sub-table is judged at its most expensive piece by default, on average when asked
    let sub = vec![light.clone(), heavy.clone()];
    assert!(subtree_verdict_eq(&heavy, &sub, 1.0, RankBy::RuleCost, &cost, false).is_err());
    let uniform = CostModel {
        rule_cost: Some(RuleCostModel {
            subtable: SubtableCalls::Uniform,
            ..model
        }),
        ..CostModel::default()
    };
    assert!(subtree_verdict_eq(&heavy, &sub, 1.0, RankBy::RuleCost, &uniform, false).is_ok());
    // the cached preset moves the per-term weights to the distinct coefficients / denominators
    let c = RuleCostModel::default().cached();
    assert_eq!(
        (c.apply_ns_per_monomial, c.apply_ns_per_distinct_monomial),
        (0.0, 140.0)
    );
    assert_eq!(c.apply_ns_per_distinct_denominator_monomial, 70.0);
    // without a model: the documented (measured) defaults, uniform calls
    let none = CostModel::default();
    let d = RuleCostModel::default();
    assert_eq!(
        size_of(&[&light], RankBy::RuleCost, &none),
        d.per_application_ns(1000, light.cost.as_ref()).round() as u64
    );
    // 29 us + 140 ns x 1000 monomials + 70 ns x 1 denominator monomial + 3.6 us x 1 child
    assert_eq!(
        size_of(&[&light], RankBy::RuleCost, &none),
        29_000 + 140_000 + 70 + 3_600
    );
}
