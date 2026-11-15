//! Lane epsdepth: the lexicographic pole objective (`poles::pole_objective`, lane poles' `select` generalised), the
//! user's eps box and the spec section (interfaces/epsdepth.md 6-8). Pure: no Symbolica.

use rustred_campaign_core::epsdepth::{
    EpsBox, EpsDepthSpec, KeyMode, TerminalSetStatus, dots_rank,
};
use rustred_campaign_core::poles::params::CriteriaOrder;
use rustred_campaign_core::poles::{
    Candidate, ChoiceReason, MaxSizeRatio, PoleObjective, PoleProfile, pole_objective, select,
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

fn local(orders: &[u32]) -> PoleProfile {
    PoleProfile {
        local: orders.to_vec(),
        composed: None,
    }
}

fn composed(orders: &[(i32, u32)]) -> PoleProfile {
    PoleProfile {
        local: vec![],
        composed: Some(orders.iter().map(|&(t, o)| (vec![t], Some(o))).collect()),
    }
}

#[test]
fn lower_max_order_beats_smaller_size_by_default_and_ratio_8_reverses_it_at_100x() {
    let obj = PoleObjective::default();
    assert_eq!(obj.max_size_ratio, MaxSizeRatio::Unbounded);
    // singular order 2 small vs singular order 1 a hundred times larger (no regular one: the generalisation)
    let cs = [cand("small-p2", 10, Some(2)), cand("big-p1", 1000, Some(1))];
    let ps = [local(&[2, 0, 1]), local(&[1, 1, 1])];
    let c = pole_objective(&cs, &ps, &obj, None).unwrap().unwrap();
    assert_eq!(c.id, "big-p1");
    assert_eq!(c.profile_used, "local");
    assert_eq!(c.chosen_measures, vec![1, 3, 3, 1000]);
    assert_eq!(
        c.measures,
        vec![
            "max_pole_order",
            "sum_pole_orders",
            "singular_terms",
            "size"
        ],
        "no eps box: a0 is no measure"
    );
    let obj8 = PoleObjective {
        max_size_ratio: MaxSizeRatio::parse(&json!("8")).unwrap(),
        ..PoleObjective::default()
    };
    let c = pole_objective(&cs, &ps, &obj8, None).unwrap().unwrap();
    assert_eq!(c.id, "small-p2");
    assert_eq!(c.reason, ChoiceReason::SizeGuard);
    assert_eq!(
        c.rejected_for_size,
        vec![("big-p1".to_string(), "100".to_string())]
    );
    assert!(
        c.report_line().contains("max_size_ratio 8"),
        "{}",
        c.report_line()
    );
}

#[test]
fn measures_apply_in_order_composed_decides_when_every_candidate_has_it() {
    let obj = PoleObjective::default();
    // equal max order 1: sum over terminals decides (b), then singular count (c), then size (d)
    let cs = [
        cand("a", 50, Some(3)),
        cand("b", 40, Some(3)),
        cand("c", 30, Some(3)),
    ];
    let ps = [
        composed(&[(1, 1), (2, 1)]),
        composed(&[(1, 1), (2, 0)]),
        composed(&[(1, 1), (2, 1)]),
    ];
    let c = pole_objective(&cs, &ps, &obj, None).unwrap().unwrap();
    assert_eq!((c.id.as_str(), c.profile_used), ("b", "composed"));
    // a local pole of order 3 that CANCELS (composed regular) beats a locally regular relation whose composition has a
    // pole (TRICKS 12/13: the composed profile decides when present)
    let cs = [
        cand("spurious-local", 100, Some(3)),
        cand("regular-local", 10, Some(0)),
    ];
    let ps = [composed(&[(1, 0)]), composed(&[(1, 1)])];
    let c = pole_objective(&cs, &ps, &obj, None).unwrap().unwrap();
    assert_eq!(c.id, "spurious-local");
    assert_eq!(c.reason, ChoiceReason::Regular);
    // one candidate without a composed profile: LOCAL for all (recorded)
    let ps = [composed(&[(1, 0)]), local(&[0])];
    let c = pole_objective(&cs, &ps, &obj, None).unwrap().unwrap();
    assert_eq!((c.id.as_str(), c.profile_used), ("regular-local", "local"));
    assert_eq!(c.lacking_composed, vec!["regular-local".to_string()]);
    // an undecided composed entry also falls back to local
    let ps = [
        PoleProfile {
            local: vec![3],
            composed: Some(vec![(vec![1], None)]),
        },
        composed(&[(1, 1)]),
    ];
    assert_eq!(
        pole_objective(&cs, &ps, &obj, None)
            .unwrap()
            .unwrap()
            .profile_used,
        "local"
    );
}

#[test]
fn eps_box_excess_is_the_first_measure_with_a_box() {
    let bx = EpsBox::from_value(
        &json!({"available": {"default": 1, "keys": [{"key": [2], "order": 3}]}}),
    )
    .unwrap();
    // a: max order 2 but on terminal [2] (available 3): within; b: max order 1... no: b has order 2 on [1] (exceeds)
    let cs = [cand("a", 10, Some(2)), cand("b", 10, Some(2))];
    let ps = [composed(&[(1, 1), (2, 3)]), composed(&[(1, 2), (2, 0)])];
    let c = pole_objective(&cs, &ps, &PoleObjective::default(), Some(&bx))
        .unwrap()
        .unwrap();
    assert_eq!(
        c.id, "a",
        "b needs [1] to eps^2 > 1; a stays within the box although its max order is higher"
    );
    assert_eq!(c.chosen_measures[0], 0);
    assert_eq!(c.measures[0], "eps_box_excess");
    // without the box: max order decides
    let c = pole_objective(&cs, &ps, &PoleObjective::default(), None)
        .unwrap()
        .unwrap();
    assert_eq!(c.id, "b");
    // a box missing a terminal: typed error
    let bx2 =
        EpsBox::from_value(&json!({"available": {"keys": [{"key": [2], "order": 3}]}})).unwrap();
    assert!(pole_objective(&cs, &ps, &PoleObjective::default(), Some(&bx2)).is_err());
}

#[test]
fn select_is_the_special_case_and_the_objective_is_order_independent() {
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
        (MaxSizeRatio::Unbounded, CriteriaOrder::RegularFirst),
        (
            MaxSizeRatio::parse(&json!(2)).unwrap(),
            CriteriaOrder::RegularFirst,
        ),
        (MaxSizeRatio::Unbounded, CriteriaOrder::EconomyFirst),
    ] {
        let want = select(&cs, ratio, crit).unwrap();
        let via = pole_objective(&cs, &[], &PoleObjective::from_criteria(ratio, crit), None)
            .unwrap()
            .unwrap();
        assert_eq!(want.id, via.id);
        for r in 0..cs.len() {
            let mut v = cs.clone();
            v.rotate_left(r);
            let ps: Vec<PoleProfile> = v
                .iter()
                .map(|c| local(&[c.pole_order.unwrap_or(0) as u32]))
                .collect();
            let a = pole_objective(&v, &ps, &PoleObjective::default(), None)
                .unwrap()
                .unwrap()
                .id;
            v.reverse();
            let ps: Vec<PoleProfile> = v
                .iter()
                .map(|c| local(&[c.pole_order.unwrap_or(0) as u32]))
                .collect();
            assert_eq!(
                pole_objective(&v, &ps, &PoleObjective::default(), None)
                    .unwrap()
                    .unwrap()
                    .id,
                a
            );
        }
    }
    // the objective is spec data
    let o = PoleObjective::from_value(
        &json!({"measures": ["max_pole_order", "size"], "max_size_ratio": "5/2"}),
        MaxSizeRatio::Unbounded,
    )
    .unwrap();
    assert_eq!(
        o.to_json(),
        json!({"measures": ["max_pole_order", "size"], "max_size_ratio": "5/2"})
    );
    assert!(
        PoleObjective::from_value(
            &json!({"measures": ["size", "size"]}),
            MaxSizeRatio::Unbounded
        )
        .is_err()
    );
    assert!(
        PoleObjective::from_value(&json!({"measures": ["colour"]}), MaxSizeRatio::Unbounded)
            .is_err()
    );
    assert!(PoleObjective::from_value(&json!({"weights": []}), MaxSizeRatio::Unbounded).is_err());
}

#[test]
fn eps_box_and_spec_parsing_validation_defaults() {
    let b = EpsBox::from_value(&json!({"available": {"default": 2, "keys": [{"key": [1, 0], "order": 4}]}, "needed_order": 1}))
        .unwrap();
    assert_eq!(b.available(&[1, 0]).unwrap(), 4);
    assert_eq!(b.available(&[2, 0]).unwrap(), 2);
    assert_eq!(b.required(2), 3);
    assert_eq!(b.status(&[2, 0], 1).unwrap().word(), "within");
    assert_eq!(b.status(&[2, 0], 3).unwrap().word(), "exceeds:2");
    assert!(
        EpsBox::from_value(
            &json!({"available": {"keys": [{"key": [1], "order": 1}, {"key": [1], "order": 2}]}})
        )
        .is_err()
    );
    assert!(
        EpsBox::from_value(
            &json!({"available": {"keys": [{"key": [1], "order": 1}, {"key": [1, 2], "order": 2}]}})
        )
        .is_err()
    );
    assert!(EpsBox::from_value(&json!({"availabel": {}})).is_err());
    let d = EpsDepthSpec::default();
    assert_eq!(
        (d.d0.as_str(), d.target_exponent, d.budgets.max_depth),
        ("4", 0, 32)
    );
    assert_eq!(d.terminal_set, TerminalSetStatus::NotCertified);
    assert!(d.eps_box.is_none());
    let s = EpsDepthSpec::from_value(
        &json!({"d0": "17/7", "keys": {"mode": "sample", "sample": {"n": 3, "seed": 7}},
        "terminal_set": "certified_regular", "budgets": {"max_depth": 8, "max_keys": 1000}}),
    )
    .unwrap();
    assert_eq!(s.keys, KeyMode::Sample { n: 3, seed: 7 });
    assert!(EpsDepthSpec::from_value(&json!({"d0": "4.0"})).is_err());
    assert!(EpsDepthSpec::from_value(&json!({"budget": {}})).is_err());
    assert!(EpsDepthSpec::from_value(&json!({"keys": {"mode": "layer"}})).is_err());
    // sampling: deterministic, sorted, a subset
    let keys: Vec<Vec<i32>> = (0..50).map(|i| vec![i, 0]).collect();
    let a = s.select_keys(&keys, |_| 0);
    assert_eq!(a.len(), 3);
    assert_eq!(
        a,
        s.select_keys(&keys.iter().rev().cloned().collect::<Vec<_>>(), |_| 0)
    );
    let l =
        EpsDepthSpec::from_value(&json!({"keys": {"mode": "layer", "max_dots_rank": 1}})).unwrap();
    let lines = [true, false];
    let sel = l.select_keys(
        &[vec![1, 0], vec![2, 0], vec![3, 0], vec![1, -1], vec![2, -1]],
        |k| dots_rank(k, &lines),
    );
    assert_eq!(sel, vec![vec![1, -1], vec![1, 0], vec![2, 0]]);
    assert_eq!(s.to_json()["d0"], json!("17/7"));
}
