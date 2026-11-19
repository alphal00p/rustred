//! Lane poles, B1: D1 per relation (src/valuation/relation.rs): the stable call `is_regular_at` / `pole_order_of` over
//! the record forms other lanes hold (engine solve-recipe relation, table rule, unnormalised relation), the same verdict
//! from the normalised and the unnormalised form (TRICKS 12(a)), index-dependent rule poles judged per key (TRICKS 9),
//! and the `rustred.campaign.pole-relation.v1` record. Needs the Symbolica licence; SKIPS without it.

mod inputs;
use rustred_campaign_algebra::licensed;
use rustred_campaign_algebra::valuation::D0;
use rustred_campaign_algebra::valuation::relation::{
    analyse_rule, is_regular_at, pole_order_of, relation_record, rule_record,
};
use rustred_campaign_core::poles::record::{RegularAt, check_record_digest};
use serde_json::{Value, json};

fn go() -> bool {
    if !licensed() {
        inputs::missing(format_args!(
            "SKIP: Symbolica licence absent (source WS/ops/env.sh)"
        ));
        return false;
    }
    true
}

fn d4() -> D0 {
    D0::parse("4").unwrap()
}

/// An engine solve-recipe relation record (the shape of results.jsonl; invented keys of arity 3).
fn engine(rhs: Value) -> Value {
    json!({"type": "relation", "key": [2, 1, -1], "verdict": "SOLVED", "target": "I(2,1,-1)", "rhs": rhs,
        "relation_digest": "00ff", "relation_rule_taken": "m1", "d_poles": [], "seconds": 1.5, "ordinal": 3})
}

#[test]
fn engine_relation_records() {
    if !go() {
        return;
    }
    let k = [2, 1, -1];
    // regular (old two-member rhs entries and new three-member ones)
    let r = engine(json!([
        ["I(1,1,-1)", "(-10+3*d)/2"],
        ["I(2,1,0)", "3-d", "top"],
        ["I(2,0,-1)", "1/(d-3)", "sub"]
    ]));
    assert_eq!(is_regular_at(&r, &k, &d4()), RegularAt::Regular);
    assert_eq!(pole_order_of(&r, &k, &d4()), Ok(0));
    // a double pole on a subsector term, a simple one on a top term
    let r = engine(json!([
        ["I(1,1,-1)", "(d-3)/(d-4)", "top"],
        ["I(2,0,-1)", "1/(2*d-8)^2", "sub"]
    ]));
    assert_eq!(
        is_regular_at(&r, &k, &d4()),
        RegularAt::Singular { order: 2 }
    );
    assert_eq!(pole_order_of(&r, &k, &d4()), Ok(2));
    // at another d0 the same relation is regular
    assert_eq!(pole_order_of(&r, &k, &D0::parse("6").unwrap()), Ok(0));
    // a zero coefficient is no term
    let r = engine(json!([["I(1,1,-1)", "(d-4)/(d-4)^2-1/(d-4)", "top"]]));
    assert_eq!(is_regular_at(&r, &k, &d4()), RegularAt::Regular);
    // undecided: another key, an unsolved record, an unparsable coefficient, not a relation
    assert!(matches!(
        is_regular_at(&r, &[1, 1, 1], &d4()),
        RegularAt::Undecided { .. }
    ));
    let mut u = engine(json!([]));
    u["verdict"] = json!("NO_RELATION_IN_SEEDS");
    assert!(matches!(
        is_regular_at(&u, &k, &d4()),
        RegularAt::Undecided { .. }
    ));
    assert!(pole_order_of(&u, &k, &d4()).is_err());
    let bad = engine(json!([["I(1,1,-1)", "log(d)", "top"]]));
    assert!(matches!(
        is_regular_at(&bad, &k, &d4()),
        RegularAt::Undecided { .. }
    ));
    assert!(matches!(
        is_regular_at(&json!({"type": "end"}), &k, &d4()),
        RegularAt::Undecided { .. }
    ));
}

/// TRICKS 12(a): `c_0 I(k) = sum c_j I(k_j)` with `c_0 = (d - 4) x regular`: the unnormalised form shows a regular rhs and
/// a vanishing target coefficient, the normalised form (what the engine writes) shows rhs poles; the verdict is the same.
#[test]
fn normalised_and_unnormalised_give_the_same_verdict() {
    if !go() {
        return;
    }
    let k = [2, 1, -1];
    let un = json!({"key": k, "target_coefficient": "(d-4)*(2*d-7)",
        "rhs": [["I(1,1,-1)", "(d-3)*(2*d-7)", "top"], ["I(2,1,0)", "(d-4)*(d-5)", "top"], ["I(2,0,-1)", "d", "sub"]]});
    let norm = engine(json!([
        ["I(1,1,-1)", "(d-3)/(d-4)", "top"],
        ["I(2,1,0)", "(d-5)/(2*d-7)", "top"],
        ["I(2,0,-1)", "d/((d-4)*(2*d-7))", "sub"]
    ]));
    let a = is_regular_at(&un, &k, &d4());
    let b = is_regular_at(&norm, &k, &d4());
    assert_eq!(a, RegularAt::Singular { order: 1 });
    assert_eq!(a, b);
    // the record of the unnormalised form names the target's own order (+1: it vanishes at d0)
    let rec = relation_record(&un, &k, &d4(), "N", json!({"kind": "point"}), 20).to_json();
    check_record_digest(&rec).unwrap();
    assert_eq!(rec["representation"], "unnormalised");
    assert_eq!(rec["target_order"], 1);
    assert_eq!(rec["pole_order"], 1);
    assert_eq!(rec["terms_with_pole"], 2);
    let rec = relation_record(&norm, &k, &d4(), "N", json!({"kind": "recipe"}), 20).to_json();
    assert_eq!(
        (rec["representation"].clone(), rec["target_order"].clone()),
        (json!("normalised"), Value::Null)
    );
    assert_eq!(rec["pole_order"], 1);
    assert_eq!(rec["verdict"]["relation"], "pole");
    // a common factor of every coefficient cancels: regular
    let common = json!({"key": k, "target_coefficient": "(d-4)^2", "rhs": [["I(1,1,-1)", "(d-4)^2*(d-1)"], ["I(2,1,0)", "(d-4)^3"]]});
    assert_eq!(is_regular_at(&common, &k, &d4()), RegularAt::Regular);
    // a target coefficient that vanishes identically: no relation for the target
    let zero = json!({"key": k, "target_coefficient": "d-d", "rhs": [["I(1,1,-1)", "1"]]});
    assert!(matches!(
        is_regular_at(&zero, &k, &d4()),
        RegularAt::Undecided { .. }
    ));
}

#[test]
fn record_is_deterministic_and_bound_to_the_relation() {
    if !go() {
        return;
    }
    let k = [2, 1, -1];
    let mut r = engine(json!([
        ["I(1,1,-1)", "(d-3)/(d-4)", "top"],
        ["I(2,0,-1)", "1/(2*d-8)^2", "sub"]
    ]));
    let a = relation_record(&r, &k, &d4(), "N", json!({"kind": "recipe", "id": "x"}), 20).to_json();
    let b = relation_record(&r, &k, &d4(), "N", json!({"kind": "recipe", "id": "x"}), 20).to_json();
    assert_eq!(a, b);
    check_record_digest(&a).unwrap();
    assert_eq!(a["relation"]["relation_digest"], "00ff");
    assert_eq!(a["pole_order_top"], 1);
    assert_eq!(a["worst"], json!([{"key": "I(2,0,-1)", "order": 2}]));
    // volatile members do not move the entry digest; a coefficient does
    r["seconds"] = json!(99.0);
    r["ordinal"] = json!(7);
    let c = relation_record(&r, &k, &d4(), "N", json!({"kind": "recipe", "id": "x"}), 20).to_json();
    assert_eq!(a["relation"]["entry_digest"], c["relation"]["entry_digest"]);
    r["rhs"][0][1] = json!("(d-3)/(d-5)");
    let e = relation_record(&r, &k, &d4(), "N", json!({"kind": "recipe", "id": "x"}), 20).to_json();
    assert_ne!(a["relation"]["entry_digest"], e["relation"]["entry_digest"]);
    assert_ne!(a["record_digest"], e["record_digest"]);
    // hidden lift poles are recorded separately, never as a pole of the relation
    r["hidden_d_poles"] = json!(["-4+d", "-3+d"]);
    let h = relation_record(&r, &k, &d4(), "N", json!({}), 20).to_json();
    assert_eq!(h["certificate_pole_at_d0"], true);
    // (the first coefficient was changed above: the subsector term's double pole remains)
    assert_eq!(h["pole_order"], 2);
}

/// TRICKS 9: one rule, regular at one key and singular at another; the record carries per-key verdicts and the loci.
#[test]
fn table_rule_per_key_verdicts() {
    if !go() {
        return;
    }
    // arity 3: n0, n1 lines, n2 numerator; the factor (n1 - 1)(d - 2 n1) is (d - 4) at n1 = 2
    let rule = json!({"type": "rule", "chart": "110", "index": 5, "target": "I(n0,n1,n2)",
        "rhs": [["I(n0-1,n1,n2)", "(d-3)/((n1-1)*(d-2*n1))"], ["I(n0,n1-1,n2)", "n0/(d-5)"], ["I(n0,n1,n2+1)", "(d-2*n0)"]],
        "exceptions": []});
    let keys = vec![vec![2, 2, 0], vec![3, 3, -1], vec![2, 1, 0]];
    assert_eq!(
        is_regular_at(&rule, &keys[0], &d4()),
        RegularAt::Singular { order: 1 }
    );
    assert_eq!(is_regular_at(&rule, &keys[1], &d4()), RegularAt::Regular);
    assert!(matches!(
        is_regular_at(&rule, &keys[2], &d4()),
        RegularAt::Undecided { .. }
    ));
    let a = analyse_rule(&rule, &keys, 3, &d4()).unwrap();
    assert_eq!(a.factor_order, 0);
    // loci: the first coefficient (den (n1-1)(4-2 n1)), the second (num n0: vanishes at n0 = 0), the third (num n0 = 2)
    assert_eq!(a.loci.len(), 3, "{:?}", a.loci);
    let rec = rule_record(
        &rule,
        &keys,
        3,
        &d4(),
        "N",
        json!({"table": "t", "index": 5, "record_sha256": "aa"}),
        20,
    )
    .to_json();
    check_record_digest(&rec).unwrap();
    assert_eq!(rec["scope"], "rule");
    assert_eq!(rec["per_key"].as_array().unwrap().len(), 3);
    assert_eq!(rec["per_key"][0]["relation"], "pole");
    assert_eq!(rec["per_key"][1]["relation"], "regular");
    assert_eq!(rec["per_key"][2]["relation"], "undecided");
    assert_eq!(rec["factor_order"], 0);
    let t0 = &rec["locus"]["terms"][0];
    assert_eq!(t0["pole_rises_on"].as_array().unwrap().len(), 2, "{t0}");
}

/// The census path (`PreparedRule`, exact fast path off the index loci) gives the same verdict as the full substitution
/// (`analyse_rule` / `is_regular_at`) at every key of a grid crossing the loci (on and off, numerator and denominator).
#[test]
fn prepared_rule_equals_full_substitution_on_a_grid() {
    if !go() {
        return;
    }
    use rustred_campaign_algebra::valuation::relation::PreparedRule;
    let rule = json!({"type": "rule", "target": "I(n0,n1,n2)", "rhs": [
        ["I(n0-1,n1,n2)", "(d-3)/((n1-1)*(d-2*n1))"],
        ["I(n0,n1-1,n2)", "(d-n0-2)^2/((d-4)*(3*d-n0-n1-6))"],
        ["I(n0,n1,n2+1)", "(d-2*n0)*(d+n2-4)/(d-5)"],
        ["I(n0-1,n1,n2+1)", "(n0-n1)*(d-4)^2"]]});
    let mut keys = vec![];
    for n0 in 1..=5 {
        for n1 in 2..=5 {
            for n2 in -3..=0 {
                keys.push(vec![n0, n1, n2]);
            }
        }
    }
    let pr = PreparedRule::of_record(&rule, 3, &d4()).unwrap();
    let full = analyse_rule(&rule, &keys, 3, &d4()).unwrap();
    let mut singular = 0;
    for (k, v) in &full.per_key {
        let (fast, _) = pr.at_key(k);
        assert_eq!(&fast, v, "key {k:?}");
        assert_eq!(&is_regular_at(&rule, k, &d4()), v, "key {k:?}");
        singular += usize::from(matches!(v, RegularAt::Singular { .. }));
    }
    // the grid really crosses the loci: some keys singular, some regular, more than one order
    assert!(
        singular > 0 && singular < keys.len(),
        "{singular} of {}",
        keys.len()
    );
    let orders: std::collections::BTreeSet<u64> = full
        .per_key
        .iter()
        .filter_map(|(_, v)| v.pole_order())
        .collect();
    assert!(orders.len() >= 3, "{orders:?}");
}

/// Audit r2-poles F1: a table rule's coefficients are evaluated at the RULE POINT of its target pattern (core
/// `closure::bodies::rule_point`, the engine's `find_rule`), never at the key: target `I(n0+1,n1-2,n2)` maps the key
/// (k0, k1, k2) to the point (k0 - 1, k1 + 2, k2). The term key patterns are evaluated at the point too (sector test).
/// A rule without `target`, an unreadable target or a pattern that does not match the key is UNDECIDED.
#[test]
fn table_rule_coefficients_at_the_rule_point() {
    if !go() {
        return;
    }
    use rustred_campaign_algebra::valuation::relation::PreparedRule;
    // 1/(d - 2 n0): pole at the point n0 = 2, i.e. at the KEY k0 = 3; (d - n1)^-2: double pole at n1 = 4, i.e. k1 = 2
    let rule = json!({"type": "rule", "chart": "110", "index": 2, "target": "I(n0+1,n1-2,n2)",
        "fixed": [null, null, null],
        "rhs": [["I(n0,n1-2,n2)", "1/(d-2*n0)"], ["I(n0+1,n1-3,n2)", "(d-3)/(d-n1)^2"], ["I(n0+1,n1-2,n2+1)", "n2+1"]]});
    // key (3, 1, 0): point (2, 3, 0): first coefficient 1/(d-4) (order 1), second (d-3)/(d-3)^2: regular at 4
    assert_eq!(
        is_regular_at(&rule, &[3, 1, 0], &d4()),
        RegularAt::Singular { order: 1 }
    );
    // the same key read as the point (the old, wrong substitution) would give n0 = 3: 1/(d-6), regular
    // key (2, 2, -1): point (1, 4, -1): 1/(d-2) regular, (d-3)/(d-4)^2 double pole, third coefficient 0 (Zero)
    assert_eq!(
        is_regular_at(&rule, &[2, 2, -1], &d4()),
        RegularAt::Singular { order: 2 }
    );
    // key (2, 1, 0): point (1, 3, 0): regular everywhere
    assert_eq!(is_regular_at(&rule, &[2, 1, 0], &d4()), RegularAt::Regular);
    // the census path agrees and evaluates term patterns at the point: at key (3, 1, 0) the pole term is
    // I(n0, n1-2, n2) at the point = I(2, 1, 0): in the key's top sector (lines 0, 1 stay >= 1)
    let pr = PreparedRule::of_record(&rule, 3, &d4()).unwrap();
    assert!(!pr.point_map().is_identity());
    for k in [[3, 1, 0], [2, 2, -1], [2, 1, 0], [4, 3, -2], [3, 2, 0]] {
        let (v, ..) = pr.at_key_parts(&k);
        assert_eq!(v, is_regular_at(&rule, &k, &d4()), "key {k:?}");
    }
    let (v, with, top) = pr.at_key_parts(&[3, 1, 0]);
    assert_eq!(
        (v, with, top),
        (RegularAt::Singular { order: 1 }, 1, Some(1))
    );
    // at key (2, 2, -1) the double pole sits on I(n0+1, n1-3, n2) = I(2, 1, -1): top sector of (2, 2, -1)
    assert_eq!(pr.at_key_parts(&[2, 2, -1]).2, Some(2));
    // a constant slot of the pattern on a free case slot: target I(n0, 2, n2), no fixed value: the parameter n1 is not
    // set by the key and takes 0 at the rule point (the engine's find_rule / core rule_point); the key has 2 there
    let fixed = json!({"type": "rule", "target": "I(n0,2,n2)", "fixed": [null, null, null],
        "rhs": [["I(n0-1,2,n2)", "1/(d-4+n1)"]]});
    assert_eq!(
        is_regular_at(&fixed, &[1, 2, 0], &d4()),
        RegularAt::Singular { order: 1 },
        "n1 = 0 at the rule point: 1/(d-4) (the key's 2 would give 1/(d-2))"
    );
    // ... and a key whose constant slot differs does not match the pattern: undecided, never a number
    assert!(matches!(
        is_regular_at(&fixed, &[1, 3, 0], &d4()),
        RegularAt::Undecided { .. }
    ));
    assert!(matches!(
        PreparedRule::of_record(&fixed, 3, &d4())
            .unwrap()
            .at_key_parts(&[1, 3, 0])
            .0,
        RegularAt::Undecided { .. }
    ));
    // no target: the rule point is unknown -> undecided (is_regular_at, analyse_rule per key, PreparedRule refuses)
    let bare = json!({"type": "rule", "rhs": [["I(n0-1,n1,n2)", "1/(d-2*n0)"]]});
    assert!(matches!(
        is_regular_at(&bare, &[2, 1, 0], &d4()),
        RegularAt::Undecided { .. }
    ));
    assert!(pole_order_of(&bare, &[2, 1, 0], &d4()).is_err());
    assert!(PreparedRule::of_record(&bare, 3, &d4()).is_err());
    assert!(analyse_rule(&bare, &[vec![2, 1, 0]], 3, &d4()).is_err());
    // an unreadable target: undecided
    let bad = json!({"type": "rule", "target": "I(n0+x,n1,n2)", "rhs": [["I(n0,n1,n2)", "1"]]});
    assert!(matches!(
        is_regular_at(&bad, &[2, 1, 0], &d4()),
        RegularAt::Undecided { .. }
    ));
    // analyse_rule / rule_record give the same per-key verdicts at the rule points
    let keys = vec![vec![3, 1, 0], vec![2, 2, -1], vec![2, 1, 0]];
    let a = analyse_rule(&rule, &keys, 3, &d4()).unwrap();
    let got: Vec<RegularAt> = a.per_key.iter().map(|(_, v)| v.clone()).collect();
    assert_eq!(
        got,
        vec![
            RegularAt::Singular { order: 1 },
            RegularAt::Singular { order: 2 },
            RegularAt::Regular
        ]
    );
    // a canonical target with fixed slots stays the identity map
    let canon = json!({"type": "rule", "target": "I(n0,1,n2)", "fixed": [null, 1, null],
        "rhs": [["I(n0-1,1,n2)", "1/(d-2*n0)"]]});
    let pc = PreparedRule::of_record(&canon, 3, &d4()).unwrap();
    assert!(pc.point_map().is_identity());
    assert_eq!(
        is_regular_at(&canon, &[2, 1, 0], &d4()),
        RegularAt::Singular { order: 1 }
    );
}
