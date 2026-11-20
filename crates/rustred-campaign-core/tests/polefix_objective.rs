//! Lane polefix (audit r2-poles F3): `poles::pole_objective` with COMPOSED profiles. Alternatives of one key reduce onto
//! the same terminal basis, so their composed profiles tie by construction (POLES_OBJECTIVE.md 2 A); the LOCAL pole
//! measures (regular at d0 first = max local order 0, then max / sum / count) must decide BEFORE the size, so the user's
//! default "regular at d = 4 always wins" holds with composed profiles too; the recorded reason is true
//! (`decided_by` names the entry that separated the chosen candidate). Pure, no Symbolica; toy data.

use rustred_campaign_core::poles::{
    Candidate, ChoiceReason, MaxSizeRatio, PoleObjective, PoleProfile, pole_objective,
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

fn prof(local: &[u32], composed: &[(i32, u32)]) -> PoleProfile {
    PoleProfile {
        local: local.to_vec(),
        composed: Some(composed.iter().map(|&(t, o)| (vec![t], Some(o))).collect()),
    }
}

// The three tests below: the composed-profile machinery of the POLE-FIRST objective, the default until 2026-11-19 and an
// EXPLICIT option since the user ruling (rule 21: the default puts size first).
#[test]
fn a_composed_tie_is_decided_by_the_local_measures_before_the_size() {
    let obj = PoleObjective::withdrawn_pole_first();
    // the same composed vector (regular on the terminals) for both: the regular relation wins whatever its size
    let t = [(1, 0), (2, 0)];
    let cs = [
        cand("regular-big", 100, Some(0)),
        cand("singular-small", 10, Some(1)),
    ];
    let ps = [prof(&[0, 0], &t), prof(&[1, 0], &t)];
    let c = pole_objective(&cs, &ps, &obj, None).unwrap().unwrap();
    assert_eq!(c.id, "regular-big");
    assert_eq!(c.reason, ChoiceReason::Regular);
    assert_eq!(c.profile_used, "composed");
    assert_eq!(c.decided_by, "local:max_pole_order");
    assert_eq!(
        c.measures,
        vec![
            "max_pole_order",
            "sum_pole_orders",
            "singular_terms",
            "local:max_pole_order",
            "local:sum_pole_orders",
            "local:singular_terms",
            "size"
        ]
    );
    assert_eq!(c.chosen_measures, vec![0, 0, 0, 0, 0, 0, 100]);
    assert!(c.report_line().contains("decided by local:max_pole_order"));
    // input order does not matter
    let cs2 = [cs[1].clone(), cs[0].clone()];
    let ps2 = [ps[1].clone(), ps[0].clone()];
    assert_eq!(
        pole_objective(&cs2, &ps2, &obj, None).unwrap().unwrap().id,
        "regular-big"
    );
    // both singular locally (composed tie with a genuine pole on terminal 1): the lower LOCAL max order wins before
    // the size, then the local sum
    let t = [(1, 1), (2, 0)];
    let cs = [
        cand("p2-small", 5, Some(2)),
        cand("p1-big", 500, Some(1)),
        cand("p1-sum3", 50, Some(1)),
    ];
    let ps = [
        prof(&[2, 0, 0], &t),
        prof(&[1, 0, 0], &t),
        prof(&[1, 1, 1], &t),
    ];
    let c = pole_objective(&cs, &ps, &obj, None).unwrap().unwrap();
    assert_eq!(c.id, "p1-big");
    assert_eq!(c.reason, ChoiceReason::LowestPole, "it is singular itself");
    assert_eq!(c.decided_by, "local:sum_pole_orders");
    // the size decides only when the local measures tie too
    let ps = [prof(&[1, 0], &t), prof(&[1, 0], &t), prof(&[0, 1], &t)];
    let c = pole_objective(&cs, &ps, &obj, None).unwrap().unwrap();
    assert_eq!((c.id.as_str(), c.decided_by.as_str()), ("p2-small", "size"));
}

#[test]
fn the_composed_profile_still_decides_first_and_the_reason_says_which() {
    let obj = PoleObjective::withdrawn_pole_first();
    // locally regular, but its composition has a pole; the other has a local pole that cancels: composed decides
    let cs = [
        cand("spurious-local", 100, Some(3)),
        cand("regular-local", 10, Some(0)),
    ];
    let ps = [prof(&[3], &[(1, 0)]), prof(&[0], &[(1, 1)])];
    let c = pole_objective(&cs, &ps, &obj, None).unwrap().unwrap();
    assert_eq!(c.id, "spurious-local");
    assert_eq!(c.reason, ChoiceReason::ComposedRegular);
    assert_eq!(c.decided_by, "max_pole_order");
    assert_eq!(c.to_json()["reason"], "composed_regular");
    assert_eq!(c.to_json()["decided_by"], "max_pole_order");
}

#[test]
fn a_finite_size_ratio_bounds_the_local_regular_class_of_a_composed_tie() {
    // addendum 7: with a finite ratio the regular class is rejected when every member is beyond ratio x smallest
    let obj8 = PoleObjective {
        max_size_ratio: MaxSizeRatio::parse(&json!("8")).unwrap(),
        ..PoleObjective::withdrawn_pole_first()
    };
    let t = [(1, 0)];
    let cs = [
        cand("regular-big", 100, Some(0)),
        cand("singular-small", 10, Some(1)),
    ];
    let ps = [prof(&[0], &t), prof(&[1], &t)];
    let c = pole_objective(&cs, &ps, &obj8, None).unwrap().unwrap();
    assert_eq!(c.id, "singular-small");
    assert_eq!(c.reason, ChoiceReason::SizeGuard);
    assert_eq!(c.decided_by, "size_guard");
    assert_eq!(
        c.rejected_for_size,
        vec![("regular-big".to_string(), "10".to_string())]
    );
    // within the ratio the regular one wins
    let cs = [
        cand("regular-mid", 70, Some(0)),
        cand("singular-small", 10, Some(1)),
    ];
    let c = pole_objective(&cs, &ps, &obj8, None).unwrap().unwrap();
    assert_eq!(
        (c.id.as_str(), c.reason),
        ("regular-mid", ChoiceReason::Regular)
    );
    // the economy objective keeps size first, the local block after the composed measures
    let eco = PoleObjective::from_criteria(
        MaxSizeRatio::Unbounded,
        rustred_campaign_core::poles::params::CriteriaOrder::EconomyFirst,
    );
    let c = pole_objective(&cs, &ps, &eco, None).unwrap().unwrap();
    assert_eq!(c.id, "singular-small");
    assert_eq!(
        c.measures,
        vec![
            "size",
            "regular",
            "max_pole_order",
            "local:regular",
            "local:max_pole_order"
        ]
    );
}
