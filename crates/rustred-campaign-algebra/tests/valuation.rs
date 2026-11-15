//! Lane poles, B0: the exact order at d = d0 of rational coefficients (src/valuation.rs, Symbolica throughout).
//! Cancellations between numerator and denominator, high multiplicity, rational d0, literals above 128 bit, the zero
//! coefficient as its own value, typed errors, and INDEX-DEPENDENT poles decided at the key (TRICKS item 9).
//! Needs the Symbolica licence (`source WS/ops/env.sh`); SKIPS without it.

use rustred_campaign_algebra::licensed;
use rustred_campaign_algebra::valuation::{
    Coefficient, D0, Valuation, ValuationError, order_at_key, order_of_ratio, order_of_text,
};

fn go() -> bool {
    if !licensed() {
        eprintln!("SKIP: Symbolica licence absent (source WS/ops/env.sh)");
        return false;
    }
    true
}

fn d4() -> D0 {
    D0::parse("4").unwrap()
}

fn ord(t: &str) -> Valuation {
    order_of_text(t, &d4()).unwrap_or_else(|e| panic!("{t}: {e}"))
}

#[test]
fn d0_parsing() {
    if !go() {
        return;
    }
    assert_eq!(D0::parse("4").unwrap().text(), "4");
    assert_eq!(D0::parse("8/2").unwrap().text(), "4");
    assert_eq!(D0::parse("+34/14").unwrap().text(), "17/7");
    assert_eq!(D0::parse("-6/4").unwrap().text(), "-3/2");
    assert_eq!(D0::parse("6/-4").unwrap().text(), "-3/2");
    assert_eq!(D0::parse("17/7").unwrap().label(), "d17_7");
    assert_eq!(D0::parse("-3/2").unwrap().label(), "dm3_2");
    for bad in ["4.0", "abc", "1/0", "", "4 / 2", "d", "1e3", "/2", "2/"] {
        assert!(
            matches!(D0::parse(bad), Err(ValuationError::D0NotRational { .. })),
            "{bad:?}"
        );
    }
}

#[test]
fn orders_and_cancellations() {
    if !go() {
        return;
    }
    // regular, non-zero
    assert_eq!(ord("1"), Valuation::Order(0));
    assert_eq!(ord("(d-3)/(d-5)"), Valuation::Order(0));
    // simple pole / zero
    assert_eq!(ord("1/(d-4)"), Valuation::Order(-1));
    assert_eq!(ord("d-4"), Valuation::Order(1));
    // cancellation between numerator and denominator: (d-4)^2 / (3d - 12) = (d-4)/3
    assert_eq!(ord("(d-4)^2/(3*d-12)"), Valuation::Order(1));
    // expanded forms: (d^2 - 8d + 16) / (d^3 - 12 d^2 + 48 d - 64) = 1/(d-4)
    assert_eq!(
        ord("(d^2-8*d+16)/(d^3-12*d^2+48*d-64)"),
        Valuation::Order(-1)
    );
    // a product hiding the factor: (2d - 8)(d - 3) / ((d - 4)^3 (d + 1))
    assert_eq!(ord("(2*d-8)*(d-3)/((d-4)^3*(d+1))"), Valuation::Order(-2));
    // high multiplicity
    assert_eq!(ord("(d-4)^40/(2*d-8)^43"), Valuation::Order(-3));
    assert_eq!(ord("(d-4)^7*(d-3)/(d-4)^3"), Valuation::Order(4));
    // sums that vanish at 4 only to higher order: (d-4)^3 + (d-4)^5
    assert_eq!(ord("(d-4)^3+(d-4)^5"), Valuation::Order(3));
    // numerator / denominator given separately
    assert_eq!(
        order_of_ratio("(d-4)^2", "(d-4)*(d+1)", &d4()).unwrap(),
        Valuation::Order(1)
    );
    assert_eq!(
        order_of_ratio("1/(d-4)", "(d-4)^2", &d4()).unwrap(),
        Valuation::Order(-3)
    );
    // pole_order / is_regular
    assert_eq!(Valuation::Order(-3).pole_order(), 3);
    assert_eq!(Valuation::Order(2).pole_order(), 0);
    assert!(Valuation::Zero.is_regular());
    // the one-loop massless bubble (A3, analytic): I(3,1) = (d-3)(d-4)/2 I(1,1) (normalisation 1): the inverse
    // coefficient (I(1,1) in terms of I(3,1)) has a simple pole at 4, the forward one a simple zero
    assert_eq!(ord("(d-3)*(d-4)/2"), Valuation::Order(1));
    assert_eq!(ord("2/((d-3)*(d-4))"), Valuation::Order(-1));
}

#[test]
fn rational_d0_and_big_literals() {
    if !go() {
        return;
    }
    let d = D0::parse("17/7").unwrap();
    assert_eq!(
        order_of_text("(7*d-17)^2/(d-1)", &d).unwrap(),
        Valuation::Order(2)
    );
    assert_eq!(
        order_of_text("1/(7*d-17)", &d).unwrap(),
        Valuation::Order(-1)
    );
    assert_eq!(order_of_text("1/(d-4)", &d).unwrap(), Valuation::Order(0));
    // literals above 128 bit (2^128 + 1, 2^128 + 3, 2^200 + 1)
    let a = "340282366920938463463374607431768211457";
    let b = "340282366920938463463374607431768211459";
    let c = "1606938044258990275541962092341162602522202993782792835301377";
    assert_eq!(
        ord(&format!("({a}*(d-4))/({b}*(d-4)^2)")),
        Valuation::Order(-1)
    );
    assert_eq!(
        ord(&format!("({c}*d - 4*{c})^3/({a}*d+{b})")),
        Valuation::Order(3)
    );
    // a d0 with a big numerator: the factor (3 d - a) vanishes at d0 = a/3 only
    let big = D0::parse(&format!("{a}/3")).unwrap();
    assert_eq!(
        order_of_text(&format!("(d-1)/((3*d-{a})^2)"), &big).unwrap(),
        Valuation::Order(-2)
    );
    assert_eq!(
        order_of_text(&format!("(d-1)/((3*d-{b})^2)"), &big).unwrap(),
        Valuation::Order(0)
    );
    // cancellation with big coefficients: (a d - 4 a)(b d + 1) / (c (d - 4)^2)
    assert_eq!(
        ord(&format!("({a}*d-4*{a})*({b}*d+1)/({c}*(d-4)^2)")),
        Valuation::Order(-1)
    );
}

#[test]
fn zero_is_its_own_value() {
    if !go() {
        return;
    }
    assert_eq!(ord("0"), Valuation::Zero);
    assert_eq!(ord("(d-4)-(d-4)"), Valuation::Zero);
    assert_eq!(ord("0/(d-4)"), Valuation::Zero);
    assert_ne!(ord("0"), Valuation::Order(0));
    // zero AT a key (any d), non-zero generically
    let c = Coefficient::parse("(n0-3)*(d-2)/(d-4)", 1).unwrap();
    assert_eq!(c.at_key(&[3], &d4()).unwrap(), Valuation::Zero);
    assert_eq!(c.at_key(&[2], &d4()).unwrap(), Valuation::Order(-1));
}

#[test]
fn typed_errors() {
    if !go() {
        return;
    }
    // a function of d is not a rational function of d: Symbolica's conversion keeps it as an extra variable
    let e = order_of_text("sin(d)", &d4()).unwrap_err();
    assert_eq!(
        e,
        ValuationError::NotRationalInD {
            symbols: vec!["sin(d)".into()]
        }
    );
    let e = order_of_text("(d-4", &d4()).unwrap_err();
    assert!(matches!(e, ValuationError::NotRational { .. }), "{e}");
    let e = order_of_text("d*x+1", &d4()).unwrap_err();
    assert_eq!(
        e,
        ValuationError::NotRationalInD {
            symbols: vec!["x".into()]
        }
    );
    let e = order_of_text("1.5*d", &d4()).unwrap_err();
    assert!(
        matches!(
            e,
            ValuationError::NotRational { .. } | ValuationError::NotRationalInD { .. }
        ),
        "{e}"
    );
    let e = order_of_text("1/(d-d)", &d4()).unwrap_err();
    assert!(
        matches!(
            e,
            ValuationError::ZeroDenominator | ValuationError::NotRational { .. }
        ),
        "{e}"
    );
    let e = order_of_ratio("d", "0", &d4()).unwrap_err();
    assert_eq!(e, ValuationError::ZeroDenominator);
    // a parametric coefficient has no order without a key
    let e = order_of_text("n0/(d-4)", &d4()).unwrap_err();
    assert!(matches!(e, ValuationError::NotRationalInD { .. }), "{e}");
    let c = Coefficient::parse("n0/(d-2*n0)", 1).unwrap();
    assert_eq!(
        c.order(&d4()).unwrap_err(),
        ValuationError::NotRationalInD {
            symbols: vec!["n0".into()]
        }
    );
    // key arity
    assert_eq!(
        c.at_key(&[1, 2], &d4()).unwrap_err(),
        ValuationError::KeyArity {
            expected: 1,
            got: 2
        }
    );
    // a symbol outside the index range of the given arity
    let e = Coefficient::parse("n3*d", 2).unwrap_err();
    assert_eq!(
        e,
        ValuationError::NotRationalInD {
            symbols: vec!["n3".into()]
        }
    );
}

/// TRICKS item 9 (hand work, bank#2344): a factor (n8 - 1)(d - 2 n8) is (d - 4) at n8 = 2: the rule is regular at one key
/// and singular at another; the verdict is made AT THE KEY. The generic order and the exact index condition come from
/// the shifted polynomial (Symbolica), not from searching (d - 4) in the text (which does not contain it).
#[test]
fn index_dependent_pole_is_decided_at_the_key() {
    if !go() {
        return;
    }
    let text = "(d-3)/((n8-1)*(d-2*n8))";
    assert!(!text.contains("d-4"));
    let c = Coefficient::parse(text, 9).unwrap();
    assert!(c.depends_on_indices());
    let mut k = [1i64, 1, 1, 1, 1, 1, 1, 1, 2];
    assert_eq!(c.at_key(&k, &d4()).unwrap(), Valuation::Order(-1));
    k[8] = 3;
    assert_eq!(c.at_key(&k, &d4()).unwrap(), Valuation::Order(0));
    k[8] = 1;
    assert_eq!(
        c.at_key(&k, &d4()).unwrap_err(),
        ValuationError::ZeroDenominatorAtKey { key: k.to_vec() }
    );
    // the same through the one-shot helper
    assert_eq!(
        order_at_key(text, 9, &[0, 0, 0, 0, 0, 0, 0, 0, 2], &d4()).unwrap(),
        Valuation::Order(-1)
    );
    // generic: regular, with the denominator locus (n8 - 1)(n8 - 2) = 0 as irreducible factors
    let g = c.generic(&d4());
    assert_eq!(g.order, Valuation::Order(0));
    assert_eq!(g.den_multiplicity, 0);
    assert_eq!(g.den_locus.factors.len(), 2, "{:?}", g.den_locus);
    let fs: Vec<&str> = g
        .den_locus
        .factors
        .iter()
        .map(|(f, _)| f.as_str())
        .collect();
    assert!(fs.iter().all(|f| f.contains("n8")), "{fs:?}");
    assert!(g.num_locus.is_empty(), "{:?}", g.num_locus);
    // at d0 = 6 the locus moves to n8 = 3
    let d6 = D0::parse("6").unwrap();
    k[8] = 3;
    assert_eq!(c.at_key(&k, &d6).unwrap(), Valuation::Order(-1));
    k[8] = 2;
    assert_eq!(c.at_key(&k, &d6).unwrap(), Valuation::Order(0));
}

/// A key-dependent locus of several indices (the shape of the helper's T162 open-face denominator, addendum 6:
/// (n0 - 1)(6 d - 2 n0 - 2 n9 - n10 - n12 - 2 n19 - 18)), with numerator loci too.
#[test]
fn multi_index_locus_and_numerator_locus() {
    if !go() {
        return;
    }
    let arity = 20;
    let c = Coefficient::parse("(d-3)/((n0-1)*(6*d-2*n0-2*n9-n10-n12-2*n19-18))", arity).unwrap();
    let key = |n0: i64, n9: i64, n10: i64| {
        let mut k = vec![1i64; arity];
        k[0] = n0;
        for s in 9..arity {
            k[s] = 0;
        }
        k[9] = n9;
        k[10] = n10;
        k
    };
    // at d = 4: 6 - 2 n0 - 2 n9 - n10 = 0 -> pole
    assert_eq!(
        c.at_key(&key(3, 0, 0), &d4()).unwrap(),
        Valuation::Order(-1)
    );
    assert_eq!(c.at_key(&key(2, 0, 0), &d4()).unwrap(), Valuation::Order(0));
    assert_eq!(
        c.at_key(&key(6, -2, -2), &d4()).unwrap(),
        Valuation::Order(-1)
    );
    assert_eq!(
        c.at_key(&key(6, -1, -2), &d4()).unwrap(),
        Valuation::Order(0)
    );
    let g = c.generic(&d4());
    assert_eq!(g.order, Valuation::Order(0));
    assert_eq!(g.den_locus.factors.len(), 2, "{:?}", g.den_locus);
    // numerator locus: (d - 2 n0)/(d - 3) vanishes at d = 4 exactly when n0 = 2
    let c = Coefficient::parse("(d-2*n0)/(d-3)", 1).unwrap();
    let g = c.generic(&d4());
    assert_eq!(g.order, Valuation::Order(0));
    assert_eq!(g.num_locus.factors.len(), 1, "{:?}", g.num_locus);
    assert_eq!(c.at_key(&[2], &d4()).unwrap(), Valuation::Order(1));
    assert_eq!(c.at_key(&[5], &d4()).unwrap(), Valuation::Order(0));
    // an n-independent factor (d - 4)^2 times an index-dependent one: generic order -2, rising to -3 on the locus
    let c = Coefficient::parse("1/((d-4)^2*(d-n3))", 4).unwrap();
    let g = c.generic(&d4());
    assert_eq!(g.order, Valuation::Order(-2));
    assert_eq!(g.den_multiplicity, 2);
    assert_eq!(
        c.at_key(&[0, 0, 0, 4], &d4()).unwrap(),
        Valuation::Order(-3)
    );
    assert_eq!(
        c.at_key(&[0, 0, 0, 5], &d4()).unwrap(),
        Valuation::Order(-2)
    );
}
