//! AUDIT r2-poles (independent auditor, question 1): exactness of the valuation at d0 (lane poles' `valuation`), the
//! relation-level verdicts, and the two in-process text judges of the seed minimiser / coreseed (core crate).
//! Expected values are derived by hand in the comments. Tests marked `#[ignore = "AUDIT FINDING ..."]` assert the
//! CORRECT answer and FAIL on the audited code: run them with `--ignored` to reproduce a finding.
//! Tests SKIP without the Symbolica licence (as the crate's other tests).

mod common;

use rustred_campaign_algebra::valuation::relation::{is_regular_at, pole_order_of};
use rustred_campaign_algebra::valuation::{
    Coefficient, D0, Prepared, Valuation, ValuationError, order_at_key, order_of_text,
};
use rustred_campaign_core::coreseed::{D0Verdict, DenominatorJudge, RegularityJudge};
use rustred_campaign_core::poles::record::RegularAt;
use rustred_campaign_core::seedmin::{LocalValuationJudge, PoleOrder, RelationPoles};
use serde_json::json;

fn d4() -> D0 {
    D0::parse("4").unwrap()
}

fn ord(t: &str) -> Valuation {
    order_of_text(t, &d4()).unwrap_or_else(|e| panic!("{t}: {e}"))
}

#[test]
fn audit_valuation_hidden_factors_and_cancellations() {
    if !common::licensed_or_skip("audit_valuation_hidden") {
        return;
    }
    // unreduced common factors
    assert_eq!(ord("(d-4)*(d-3)/((d-4)^2*(d-3))"), Valuation::Order(-1));
    assert_eq!(ord("(d-4)^5/((d-4)^5)"), Valuation::Order(0));
    // (d-4)^3 hidden in an expanded numerator, over (d-4)
    assert_eq!(ord("(d^3-12*d^2+48*d-64)/(d-4)"), Valuation::Order(2));
    // expanded (d-4)^2 in the denominator times a factor vanishing elsewhere
    assert_eq!(ord("(d-5)/(d^3-13*d^2+56*d-80)"), Valuation::Order(-2)); // (d-4)^2 (d-5)
    // sums of fractions: poles that cancel, poles that do not
    assert_eq!(ord("1/(d-4) - 1/(d-4) + 3"), Valuation::Order(0));
    assert_eq!(ord("1/(d-4) + 1/(4-d)"), Valuation::Zero);
    assert_eq!(ord("1/(d-4) - (d-3)/((d-4)*(d-3))"), Valuation::Zero);
    // 1/(d-4)^2 - 1/((d-4)(d-5)) = -1/((d-4)^2 (d-5)): the double pole survives
    assert_eq!(ord("1/(d-4)^2 - 1/((d-4)*(d-5))"), Valuation::Order(-2));
    // (1/(d-4) - 1/(d-3)) (d-4) = 1/(d-3): regular, not zero
    assert_eq!(ord("(1/(d-4) - 1/(d-3))*(d-4)"), Valuation::Order(0));
    // leading terms cancel to a higher zero: (d-4)/(d-3) - (d-4)/(d-2) = (d-4)/((d-3)(d-2)): order +1
    assert_eq!(ord("(d-4)/(d-3) - (d-4)/(d-2)"), Valuation::Order(1));
    // 1/(d-4) - 1/((d-4)(d-3)) = ((d-3) - 1)/((d-4)(d-3)) = 1/(d-3): the simple poles cancel: order 0
    assert_eq!(ord("1/(d-4) - 1/((d-4)*(d-3))"), Valuation::Order(0));
    // 1/(d-4)^2 - 1/((d-4)(d-3)) = ((d-3) - (d-4))/((d-4)^2 (d-3)) = 1/((d-4)^2 (d-3)): order -2
    assert_eq!(ord("1/(d-4)^2 - 1/((d-4)*(d-3))"), Valuation::Order(-2));
}

#[test]
fn audit_valuation_big_integers_zero_and_rational_d0() {
    if !common::licensed_or_skip("audit_valuation_big") {
        return;
    }
    // 10^30 (d - 4) / (d-4)^2: order -1
    assert_eq!(
        ord("(1000000000000000000000000000000*d-4000000000000000000000000000000)/(d-4)^2"),
        Valuation::Order(-1)
    );
    // (10^40 d - 4 10^40 + 1) / 10^40 = d - 4 + 10^-40: NOT zero at 4: order 0
    assert_eq!(ord("(10^40*d-4*10^40+1)/10^40"), Valuation::Order(0));
    // big coprime factors near 4: (d - 4 - 10^-30) has no root at 4
    assert_eq!(ord("1/(10^30*d - 4*10^30 - 1)"), Valuation::Order(0));
    // zero coefficient: Zero (no order), pole order 0, regular
    assert_eq!(ord("0"), Valuation::Zero);
    assert_eq!(ord("d-d"), Valuation::Zero);
    assert_eq!(Valuation::Zero.pole_order(), 0);
    assert!(Valuation::Zero.is_regular());
    // rational d0
    let d92 = D0::parse("9/2").unwrap();
    assert_eq!(
        order_of_text("1/(2*d-9)", &d92).unwrap(),
        Valuation::Order(-1)
    );
    assert_eq!(
        order_of_text("1/(2*d-9)", &d4()).unwrap(),
        Valuation::Order(0)
    );
    assert_eq!(
        order_of_text("(2*d-9)^2/(4*d^2-36*d+81)", &d92).unwrap(),
        Valuation::Order(0)
    );
    assert_eq!(D0::parse("18/4").unwrap().text(), "9/2");
    assert!(D0::parse("4.5").is_err());
    // a zero denominator is an error, never an order
    assert!(order_of_text("1/(d-d)", &d4()).is_err());
}

#[test]
fn audit_valuation_index_dependent_and_prepared_agree() {
    if !common::licensed_or_skip("audit_valuation_index") {
        return;
    }
    // 1/(d - 2 n0): pole exactly at n0 = 2
    assert_eq!(
        order_at_key("1/(d-2*n0)", 1, &[2], &d4()).unwrap(),
        Valuation::Order(-1)
    );
    assert_eq!(
        order_at_key("1/(d-2*n0)", 1, &[3], &d4()).unwrap(),
        Valuation::Order(0)
    );
    // 1/((d-4)(n0-1) + (d-4)^2): generic order -1, at n0 = 1 it is 1/(d-4)^2: -2
    assert_eq!(
        order_at_key("1/((d-4)*(n0-1)+(d-4)^2)", 1, &[1], &d4()).unwrap(),
        Valuation::Order(-2)
    );
    assert_eq!(
        order_at_key("1/((d-4)*(n0-1)+(d-4)^2)", 1, &[3], &d4()).unwrap(),
        Valuation::Order(-1)
    );
    // Prepared (locus shortcut) == full substitution at every key of a box, incl. zero numerators and zero denominators
    let text = "(n0-2)*(d-4)*(3*d-n1-10)/((d-2*n0)*(d-4)^2*(n1+1)*(d-n0-n1))";
    let c = Coefficient::parse(text, 2).unwrap();
    let p = Prepared::new(c.clone(), &d4());
    let mut seen_err = 0;
    for a in -3..=6 {
        for b in -3..=6 {
            let k = [a, b];
            let x = c.at_key(&k, &d4());
            let y = p.at_key(&k);
            match (&x, &y) {
                (Ok(u), Ok(v)) => assert_eq!(u, v, "key {k:?}"),
                (Err(ValuationError::ZeroDenominatorAtKey { .. }), Err(_)) => seen_err += 1,
                _ => panic!("key {k:?}: {x:?} vs {y:?}"),
            }
        }
    }
    assert!(seen_err > 0, "the box contains the n1 = -1 line");
    // hand values: k = (2, 3): numerator vanishes identically (n0 - 2 = 0) -> Zero
    assert_eq!(c.at_key(&[2, 3], &d4()).unwrap(), Valuation::Zero);
    // k = (3, 1): (d-4)(3d-11)/((d-6)(d-4)^2 2 (d-4)) -> (d-4)^-2
    assert_eq!(c.at_key(&[3, 1], &d4()).unwrap(), Valuation::Order(-2));
    // k = (1, 2): 3d - 12 = 3 (d-4) in the numerator, d - 2 (d - 2) ... : (d-4)^2 / ((d-2) (d-4)^2 (d-3)) -> 0
    assert_eq!(c.at_key(&[1, 2], &d4()).unwrap(), Valuation::Order(0));
}

#[test]
fn audit_relation_normalised_and_unnormalised_agree() {
    if !common::licensed_or_skip("audit_relation_forms") {
        return;
    }
    let key = vec![1i64, 2];
    // (d-4) I(K) = I(a) + (d-4) I(b): normalised I(K) = I(a)/(d-4) + I(b): order 1
    let un = json!({"key": key, "target_coefficient": "(d-4)", "rhs": [["I(1,1)", "1"], ["I(0,2)", "d-4"]]});
    assert_eq!(
        is_regular_at(&un, &key, &d4()),
        RegularAt::Singular { order: 1 }
    );
    let norm = json!({"type": "relation", "verdict": "SOLVED", "key": key, "rhs": [["I(1,1)", "1/(d-4)"], ["I(0,2)", "1"]]});
    assert_eq!(pole_order_of(&norm, &key, &d4()).unwrap(), 1);
    // a target coefficient with a pole makes the normalised relation vanish to first order: regular
    let un2 = json!({"key": key, "target_coefficient": "1/(d-4)", "rhs": [["I(1,1)", "1"]]});
    assert_eq!(is_regular_at(&un2, &key, &d4()), RegularAt::Regular);
    // a target coefficient that vanishes identically: no relation for the target (undecided, never regular)
    let un3 = json!({"key": key, "target_coefficient": "d-d", "rhs": [["I(1,1)", "1"]]});
    assert!(matches!(
        is_regular_at(&un3, &key, &d4()),
        RegularAt::Undecided { .. }
    ));
    // the record's key must be the key judged
    assert!(pole_order_of(&norm, &[9, 9], &d4()).is_err());
}

/// FINDING (MAJOR, latent): a table rule's coefficients are evaluated at the KEY, not at the RULE POINT of its target
/// pattern. Target "I(n0+1,n1)" at key (3, 1) has rule point n0 = 2, n1 = 1 (the engine's find_rule and the core's
/// `closure::bodies::rule_point`); there the rhs coefficient 1/(d - 2 n0) = 1/(d-4): pole order 1.
#[test]
#[ignore = "AUDIT FINDING r2poles-F1: valuation::relation substitutes the key, not the rule point"]
fn audit_rule_coefficients_at_the_rule_point_noncanonical_target() {
    if !common::licensed_or_skip("audit_rule_point") {
        return;
    }
    let rule = json!({"type": "rule", "chart": "11", "index": 1, "target": "I(n0+1,n1)",
        "rhs": [["I(n0,n1)", "1/(d-2*n0)"]], "exceptions": [], "dispatch": "Partition"});
    assert_eq!(
        is_regular_at(&rule, &[3, 1], &d4()),
        RegularAt::Singular { order: 1 },
        "the rule point of key (3,1) under target I(n0+1,n1) is n0 = 2"
    );
}

/// FINDING (MINOR): the seed minimiser's TEMPORARY in-process judge splits the coefficient text at its single top-level
/// '/', so `3/4*(-4+d)` (= 3(d-4)/4, a ZERO at 4) is read as 3 / (4 (d-4)): pole order 1.
#[test]
#[ignore = "AUDIT FINDING r2poles-F5: LocalValuationJudge reads A/B*C as A/(B*C)"]
fn audit_local_valuation_judge_precedence() {
    if !common::licensed_or_skip("audit_local_judge") {
        return;
    }
    let rec = json!({"type": "relation", "verdict": "SOLVED", "key": [1, 1], "rhs": [["I(1,0)", "3/4*(-4+d)"]]});
    let exact = pole_order_of(&rec, &[1, 1], &d4()).unwrap();
    assert_eq!(exact, 0);
    assert_eq!(
        LocalValuationJudge.pole_order_of(&rec, &[1, 1], "4"),
        PoleOrder::Order { order: 0 }
    );
}

/// FINDING (MINOR): coreseed's DenominatorJudge claims "a regular verdict is sound" but evaluates everything after the
/// single top-level '/': `1/(d-4)+1` (a POLE at 4) is judged regular (it evaluates "(d-4)+1" at 4).
#[test]
#[ignore = "AUDIT FINDING r2poles-F5: DenominatorJudge unsound on A/B+C"]
fn audit_denominator_judge_soundness() {
    if !common::licensed_or_skip("audit_denominator_judge") {
        return;
    }
    let rec = json!({"type": "relation", "verdict": "SOLVED", "key": [1, 1], "rhs": [["I(1,0)", "1/(d-4)+1"]]});
    assert_eq!(pole_order_of(&rec, &[1, 1], &d4()).unwrap(), 1);
    assert_ne!(
        DenominatorJudge.is_regular_at(&rec, &[1, 1], "4"),
        D0Verdict::Regular,
        "a sound judge may say undecided, never regular"
    );
}

/// The judges agree with the exact valuation on the ENGINE's printed form `(N)/(D)` (the only form they meet today).
#[test]
fn audit_text_judges_on_the_engine_form() {
    if !common::licensed_or_skip("audit_text_judges_engine_form") {
        return;
    }
    let cases = [
        "(-10+3*d)/2",
        "-1/2",
        "(8-3*d)/(-8+2*d)",
        "(16-8*d+d^2)/(-12+7*d-d^2)",
        "(2656-2032*d+518*d^2-44*d^3)/(26460-30408*d+13101*d^2-2508*d^3+180*d^4)",
        "(1)/(64-48*d+12*d^2-d^3)",
        "3-d",
    ];
    for c in cases {
        let rec =
            json!({"type": "relation", "verdict": "SOLVED", "key": [1], "rhs": [["I(0)", c]]});
        let exact = pole_order_of(&rec, &[1], &d4()).unwrap();
        assert_eq!(
            LocalValuationJudge.pole_order_of(&rec, &[1], "4"),
            PoleOrder::Order {
                order: exact as u32
            },
            "{c}"
        );
        let dj = DenominatorJudge.is_regular_at(&rec, &[1], "4");
        if exact > 0 {
            assert_ne!(dj, D0Verdict::Regular, "{c}");
        }
    }
}
