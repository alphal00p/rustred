//! Lane poles, B3: the selection among admissible candidates (src/poles/select.rs), the spec parameters
//! (src/poles/params.rs), the domain split (src/poles/split.rs) and the D1 record digest (src/poles/record.rs).
//! Pure: no Symbolica, no I/O.

use rustred_campaign_core::poles::params::CriteriaOrder;
use rustred_campaign_core::poles::record::{
    PoleRelationRecord, RegularAt, TermPole, check_record_digest,
};
use rustred_campaign_core::poles::{
    Candidate, ChoiceReason, MaxSizeRatio, PoleParams, RegularAlternative, aggregate_orders,
    select, split_domain,
};
use serde_json::json;

fn cand(id: &str, size: u64, pole: Option<u64>) -> Candidate {
    Candidate {
        id: id.into(),
        admissible: true,
        excluded_by: None,
        evidence_level: 0,
        size,
        pole_order: pole,
    }
}

const RF: CriteriaOrder = CriteriaOrder::RegularFirst;

#[test]
fn default_prefers_a_regular_candidate_100_times_larger() {
    let p = PoleParams::default();
    assert_eq!(p.prefer.max_size_ratio, MaxSizeRatio::Unbounded);
    let cs = [
        cand("small-singular", 10, Some(1)),
        cand("big-regular", 1000, Some(0)),
    ];
    let c = select(&cs, p.prefer.max_size_ratio, p.prefer.criteria_order).unwrap();
    assert_eq!(c.id, "big-regular");
    assert_eq!(c.reason, ChoiceReason::Regular);
    assert_eq!(c.chosen_size, 1000);
    assert_eq!(c.smallest_size, 10);
    assert_eq!(c.regular_alternative, RegularAlternative::Chosen);
    let line = c.report_line();
    assert!(
        line.contains("max_size_ratio unbounded") && line.contains("chosen size 1000"),
        "{line}"
    );
    assert!(line.contains("smallest admissible size 10"), "{line}");
}

#[test]
fn ratio_8_does_not() {
    let r = MaxSizeRatio::parse(&json!("8")).unwrap();
    let cs = [
        cand("small-singular", 10, Some(1)),
        cand("big-regular", 1000, Some(0)),
    ];
    let c = select(&cs, r, RF).unwrap();
    assert_eq!(c.id, "small-singular");
    assert_eq!(c.reason, ChoiceReason::SizeGuard);
    assert_eq!(
        c.regular_alternative,
        RegularAlternative::RejectedForSize {
            id: "big-regular".into(),
            size: 1000,
            ratio: "100".into()
        }
    );
    assert!(
        c.report_line()
            .contains("rejected for size: 1000 terms, ratio 100"),
        "{}",
        c.report_line()
    );
    // within the ratio the regular one wins: 80 <= 8 x 10
    let cs = [
        cand("small-singular", 10, Some(1)),
        cand("regular", 80, Some(0)),
    ];
    assert_eq!(select(&cs, r, RF).unwrap().id, "regular");
    let cs = [
        cand("small-singular", 10, Some(1)),
        cand("regular", 81, Some(0)),
    ];
    assert_eq!(select(&cs, r, RF).unwrap().id, "small-singular");
    // a rational ratio, exact: 5/2 admits 25 against 10, not 26
    let r = MaxSizeRatio::parse(&json!("5/2")).unwrap();
    assert_eq!(
        select(&[cand("s", 10, Some(2)), cand("r", 25, Some(0))], r, RF)
            .unwrap()
            .id,
        "r"
    );
    assert_eq!(
        select(&[cand("s", 10, Some(2)), cand("r", 26, Some(0))], r, RF)
            .unwrap()
            .id,
        "s"
    );
}

#[test]
fn never_returns_a_non_admissible_candidate() {
    let mut reg = cand("regular-over-cap", 50, Some(0));
    reg.admissible = false;
    reg.excluded_by = Some("max_solves".into());
    let mut tiny = cand("tiny-inadmissible", 1, Some(0));
    tiny.admissible = false;
    let cs = [reg, tiny, cand("singular", 100, Some(2))];
    let c = select(&cs, MaxSizeRatio::Unbounded, RF).unwrap();
    assert_eq!(c.id, "singular");
    assert_eq!(c.reason, ChoiceReason::Only);
    assert_eq!(
        c.regular_alternative,
        RegularAlternative::OverCap {
            id: "regular-over-cap".into(),
            cap: "max_solves".into(),
            size: 50
        }
    );
    assert!(
        c.report_line()
            .contains("regular candidate exists, over max_solves"),
        "{}",
        c.report_line()
    );
    // nothing admissible: no choice
    let mut a = cand("a", 1, Some(0));
    a.admissible = false;
    assert!(select(&[a], MaxSizeRatio::Unbounded, RF).is_none());
    assert!(select(&[], MaxSizeRatio::Unbounded, RF).is_none());
}

#[test]
fn identical_input_is_order_independent() {
    let cs = vec![
        cand("e", 40, Some(1)),
        cand("a", 30, Some(2)),
        cand("c", 30, Some(1)),
        cand("b", 30, Some(1)),
        cand("u", 5, None),
        cand("r2", 300, Some(0)),
        cand("r1", 300, Some(0)),
    ];
    for (ratio, crit) in [
        (MaxSizeRatio::Unbounded, RF),
        (MaxSizeRatio::parse(&json!(2)).unwrap(), RF),
        (MaxSizeRatio::Unbounded, CriteriaOrder::EconomyFirst),
    ] {
        let want = select(&cs, ratio, crit).unwrap().id;
        // every rotation and the reversal give the same choice
        for r in 0..cs.len() {
            let mut v = cs.clone();
            v.rotate_left(r);
            assert_eq!(select(&v, ratio, crit).unwrap().id, want);
            v.reverse();
            assert_eq!(select(&v, ratio, crit).unwrap().id, want);
        }
    }
    // unbounded: the smaller id among equal regular candidates
    assert_eq!(select(&cs, MaxSizeRatio::Unbounded, RF).unwrap().id, "r1");
    // ratio 2 (300 > 2 x 5): lowest pole order, then size, then id; unknown ranks last
    assert_eq!(
        select(&cs, MaxSizeRatio::parse(&json!(2)).unwrap(), RF)
            .unwrap()
            .id,
        "b"
    );
    // economy first: the smallest (unknown regularity, 5 terms)
    assert_eq!(
        select(&cs, MaxSizeRatio::Unbounded, CriteriaOrder::EconomyFirst)
            .unwrap()
            .id,
        "u"
    );
}

#[test]
fn singular_ranking_and_evidence_levels() {
    // among singular: lower pole order first, then fewer terms
    let cs = [
        cand("p2-small", 5, Some(2)),
        cand("p1-big", 50, Some(1)),
        cand("p1-mid", 20, Some(1)),
    ];
    let c = select(&cs, MaxSizeRatio::Unbounded, RF).unwrap();
    assert_eq!(
        (c.id.as_str(), c.reason),
        ("p1-mid", ChoiceReason::LowestPole)
    );
    assert_eq!(c.regular_alternative, RegularAlternative::None);
    // a higher evidence level wins before regularity (relation-entry 5)
    let mut strong = cand("strong-singular", 10, Some(1));
    strong.evidence_level = 2;
    let c = select(
        &[strong, cand("weak-regular", 10, Some(0))],
        MaxSizeRatio::Unbounded,
        RF,
    )
    .unwrap();
    assert_eq!(c.id, "strong-singular");
    // TRICKS 9: two alternatives over one domain, one regular at all keys, one singular at all: the regular one
    let a = aggregate_orders(&vec![Some(0); 54]);
    let b = aggregate_orders(&vec![Some(1); 54]);
    assert_eq!((a, b), (Some(0), Some(1)));
    assert_eq!(aggregate_orders(&[Some(0), None]), None);
    assert_eq!(aggregate_orders(&[Some(0), Some(3), Some(1)]), Some(3));
    let c = select(
        &[cand("singular-face", 500, b), cand("regular-face", 700, a)],
        MaxSizeRatio::Unbounded,
        RF,
    );
    assert_eq!(c.unwrap().id, "regular-face");
}

#[test]
fn max_size_ratio_parsing() {
    assert_eq!(
        MaxSizeRatio::parse(&json!("unbounded")).unwrap(),
        MaxSizeRatio::Unbounded
    );
    assert_eq!(
        MaxSizeRatio::parse(&json!("8")).unwrap(),
        MaxSizeRatio::Ratio { num: 8, den: 1 }
    );
    assert_eq!(MaxSizeRatio::parse(&json!(8)).unwrap().text(), "8");
    assert_eq!(MaxSizeRatio::parse(&json!("10/4")).unwrap().text(), "5/2");
    assert_eq!(MaxSizeRatio::parse(&json!("1")).unwrap().text(), "1");
    for bad in [
        json!("1/2"),
        json!(0),
        json!("0"),
        json!("x"),
        json!(2.5),
        json!("2/0"),
        json!(null),
        json!("-3"),
    ] {
        assert!(MaxSizeRatio::parse(&bad).is_err(), "{bad}");
    }
}

#[test]
fn spec_parameters_defaults_and_overrides() {
    let d = PoleParams::from_spec(None, None).unwrap();
    assert_eq!(d, PoleParams::default());
    assert!(d.prefer_on());
    assert_eq!(d.dimensions, vec!["4"]);
    assert_eq!(d.prefer_regular_at, vec!["4"]);
    assert_eq!(d.prefer.max_size_ratio.text(), "unbounded");
    assert_eq!(d.census.truncation, 6);
    assert_eq!(d.inherent.max_cone_keys, 200_000);
    let pd = json!({"dimensions": ["8/2", "6", "17/7"], "primes": 3,
        "census": {"mode": "sampled", "sample": {"n": 10, "seed": 7}},
        "prefer_regular": {"max_size_ratio": "8"}, "inherent": {"budget_s": 5}});
    let rp = json!({"prefer_regular_at": [], "other_member": 1,
        "prefer_regular": {"max_size_ratio": 8, "terminal_set": {"max_rotations": 2, "export": "found"}}});
    let p = PoleParams::from_spec(Some(&pd), Some(&rp)).unwrap();
    assert_eq!(p.dimensions, vec!["4", "6", "17/7"]);
    assert_eq!(p.primes, 3);
    assert_eq!(
        (
            p.census.mode.as_str(),
            p.census.sample_n,
            p.census.sample_seed
        ),
        ("sampled", 10, 7)
    );
    assert!(!p.prefer_on());
    assert_eq!(p.prefer.max_size_ratio.text(), "8");
    assert_eq!(p.prefer.terminal_set.max_rotations, Some(2));
    assert_eq!(p.prefer.terminal_set.export, "found");
    assert_eq!(p.inherent.budget_s, 5);
    // refusals: unknown member, bad choice, one prime, duplicate dimension, conflicting ratios
    for (pd, rp) in [
        (json!({"dimension": ["4"]}), json!({})),
        (json!({"census": {"mode": "fast"}}), json!({})),
        (json!({"primes": 1}), json!({})),
        (json!({"dimensions": ["4", "8/2"]}), json!({})),
        (json!({"dimensions": ["4.0"]}), json!({})),
        (
            json!({"prefer_regular": {"max_size_ratio": "8"}}),
            json!({"prefer_regular": {"max_size_ratio": "unbounded"}}),
        ),
        (
            json!({}),
            json!({"prefer_regular": {"criteria_order": "regular_within_factor"}}),
        ),
    ] {
        assert!(
            PoleParams::from_spec(Some(&pd), Some(&rp)).is_err(),
            "{pd} {rp}"
        );
    }
    // the policy digest changes with a value in force, not with member order
    let a = PoleParams::default().policy_digest();
    let b = PoleParams::from_spec(
        None,
        Some(&json!({"prefer_regular": {"max_size_ratio": "8"}})),
    )
    .unwrap()
    .policy_digest();
    assert_ne!(a, b);
    assert_eq!(
        a,
        PoleParams::from_spec(Some(&json!({})), Some(&json!({})))
            .unwrap()
            .policy_digest()
    );
}

#[test]
fn domain_split_with_condition_and_cure() {
    // a generic 4-slot domain: slots 0, 1 lines, 2, 3 numerators; singular exactly where line 1 sits at its lowest
    // power 2 (the shape of TRICKS 12 / 13, invented numbers)
    let mut per_key = vec![];
    for n1 in 2..=4 {
        for n2 in -2..=0 {
            let k = vec![1, n1, n2, 0];
            let v = if n1 == 2 {
                RegularAt::Singular { order: 1 }
            } else {
                RegularAt::Regular
            };
            per_key.push((k, v));
        }
    }
    per_key.push((
        vec![1, 3, 0, -1],
        RegularAt::Undecided {
            reason: "budget".into(),
        },
    ));
    let s = split_domain(&per_key);
    assert_eq!(
        (s.regular.len(), s.singular.len(), s.undecided.len()),
        (6, 3, 1)
    );
    assert_eq!(s.condition, Some(vec![(1, 2)]));
    assert_eq!(s.cures.len(), 1);
    assert_eq!(s.cures[0]["slot"], 1);
    assert_eq!(s.cures[0]["offset"], 1);
    let j = s.to_json();
    assert_eq!(j["condition"]["text"], "n1 = 2");
    // no simple condition: singular keys scattered
    let per_key = vec![
        (vec![1, 2, 0], RegularAt::Singular { order: 1 }),
        (vec![1, 3, -1], RegularAt::Singular { order: 2 }),
        (vec![1, 2, -1], RegularAt::Regular),
        (vec![1, 3, 0], RegularAt::Regular),
    ];
    let s = split_domain(&per_key);
    assert_eq!(s.condition, None);
    assert!(s.cures.is_empty());
    // all regular
    let s = split_domain(&[(vec![1, 1], RegularAt::Regular)]);
    assert!(s.singular.is_empty() && s.condition.is_none());
}

#[test]
fn record_digest_is_deterministic_and_checked() {
    let rec = PoleRelationRecord {
        node: "N".into(),
        d0: "4".into(),
        scope: "key".into(),
        key: Some(vec![1, 2, 0]),
        rule: None,
        source: json!({"kind": "recipe", "id": "r1"}),
        relation: json!({"relation_digest": "ab", "entry_digest": "cd"}),
        representation: "normalised".into(),
        target_order: None,
        terms: vec![
            TermPole {
                key: "I(1,1,0)".into(),
                part: Some("top".into()),
                order: Some(-1),
            },
            TermPole {
                key: "I(0,2,0)".into(),
                part: Some("sub".into()),
                order: Some(-2),
            },
            TermPole {
                key: "I(1,1,-1)".into(),
                part: Some("top".into()),
                order: None,
            },
        ],
        verdict: RegularAt::Singular { order: 2 },
        factor_order: None,
        locus: None,
        per_key: vec![],
        certificate_pole_at_d0: None,
        alternative: json!({"status": "not_tried"}),
        chosen: None,
        choice_reason: None,
        policy_digest: None,
        classification: None,
        worst_max: 20,
        method: "test".into(),
    };
    let a = rec.to_json();
    assert_eq!(a, rec.to_json());
    check_record_digest(&a).unwrap();
    assert_eq!(
        (a["pole_order"].clone(), a["pole_order_top"].clone()),
        (json!(2), json!(1))
    );
    assert_eq!(
        (a["terms_with_pole"].clone(), a["terms"].clone()),
        (json!(2), json!(2))
    );
    assert_eq!(a["worst"], json!([{"key": "I(0,2,0)", "order": 2}]));
    let mut b = a.clone();
    b["pole_order"] = json!(0);
    assert!(check_record_digest(&b).is_err());
}
