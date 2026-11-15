//! Lane polefix (audit r2-poles F6): regbasis' `Laurent` is Symbolica's `Series<Q>` (no own series arithmetic); the
//! absolute-precision rules the regbasis lattice relies on are checked on hand cases (zero series of any precision,
//! cancellation keeps the absolute precision, products with a zero, negative starts, "infinite" accumulators), and the
//! expansion of a rational function agrees with lane epsdepth's independent route (extended Euclid inverse). Needs the
//! Symbolica licence; SKIPS without it.

mod common;

use rustred_campaign_algebra::epsdepth::SymbolicaLaurent;
use rustred_campaign_algebra::regbasis::series::{
    Laurent, QP, expand_rf, parse_rational, parse_rf, parse_series, t_vars,
};
use rustred_campaign_core::regbasis::SeriesText;
use symbolica::domains::rational::{Q, Rational};

fn poly(c: &[(u16, &str)]) -> QP {
    let mut p = QP::new(&Q, None, t_vars());
    for (e, x) in c {
        p.append_monomial(parse_rational(x).unwrap(), &[*e]);
    }
    p
}

fn text(start: i64, c: &[&str], prec: i64) -> SeriesText {
    SeriesText {
        start,
        coeffs: c.iter().map(|x| x.to_string()).collect(),
        prec,
    }
}

#[test]
fn the_absolute_precision_rules_hold_on_symbolica_series() {
    if !common::licensed_or_skip("polefix_series_rules") {
        return;
    }
    let tv = t_vars();
    // a = t^-1 (1 + 2t) known below 2; b = t (3) known below 3
    let a = Laurent::normalised(-1, poly(&[(0, "1"), (1, "2")]), 2);
    let b = Laurent::normalised(1, poly(&[(0, "3")]), 3);
    assert_eq!((a.valuation(), a.prec(), a.pole_order()), (Some(-1), 2, 1));
    // product: v = 0, prec = min(v(a) + prec(b), v(b) + prec(a)) = min(2, 3) = 2; coefficients 3, 6
    let p = a.mul(&b);
    assert_eq!((p.valuation(), p.prec()), (Some(0), 2));
    assert_eq!(p.coeff(0).unwrap(), Rational::from(3));
    assert_eq!(p.coeff(1).unwrap(), Rational::from(6));
    assert!(p.coeff(2).is_err(), "beyond the precision");
    // sum: prec = min(2, 3); exact cancellation of the leading term keeps the absolute precision
    let c = Laurent::normalised(-1, poly(&[(0, "-1"), (2, "5")]), 4);
    let s = a.add(&c);
    assert_eq!((s.valuation(), s.prec()), (Some(0), 2));
    assert_eq!(s.coeff(0).unwrap(), Rational::from(2));
    let z = a.add(&Laurent::normalised(-1, poly(&[(0, "-1"), (1, "-2")]), 7));
    assert_eq!((z.valuation(), z.prec(), z.decides_pole()), (None, 2, true));
    // a zero of precision 3 times b (v = 1): zero known below 3 + 1 = 4; zero times zero: 3 + 5
    let z3 = Laurent::zero(3, &tv);
    assert_eq!(z3.mul(&b).prec(), 4);
    assert_eq!(b.mul(&z3).prec(), 4);
    assert_eq!(z3.mul(&Laurent::zero(5, &tv)).prec(), 8);
    // a zero of NEGATIVE precision does not decide a pole; an "infinite" accumulator is neutral for the sum
    let zn = Laurent::zero(-2, &tv);
    assert!(!zn.decides_pole());
    assert_eq!(zn.add(&a).prec(), -2);
    let acc = Laurent::zero(i64::MAX / 4, &tv).add(&a.mul(&b));
    assert_eq!((acc.valuation(), acc.prec()), (Some(0), 2));
    // text round trip and the scaled lattice entry t^1 * a truncated below 3: 1 + 2t
    assert_eq!(
        parse_series(&a.to_text(), &tv).unwrap().to_text(),
        a.to_text()
    );
    let t = text(-2, &["1", "0", "-1/2"], 3);
    let l = parse_series(&t, &tv).unwrap();
    assert_eq!((l.valuation(), l.prec(), l.pole_order()), (Some(-2), 3, 2));
    assert_eq!(l.to_text(), t);
    let e = a.scaled_entry(1, 2).unwrap();
    assert_eq!(e, poly(&[(0, "1"), (1, "2")]));
    assert!(a.scaled_entry(0, 1).is_err(), "negative exponent");
    assert!(a.scaled_entry(1, 4).is_err(), "beyond the precision");
}

#[test]
fn the_expansion_agrees_with_lane_epsdepths_route() {
    if !common::licensed_or_skip("polefix_series_expansion") {
        return;
    }
    let tv = t_vars();
    let d0 = parse_rational("4").unwrap();
    let lau = SymbolicaLaurent::new("4").unwrap();
    let alg = rustred_campaign_algebra::SymbolicaAlgebra::new().unwrap();
    for f in [
        "(d-3)/(d-4)^2",
        "(16-8*d+d^2)/(-12+7*d-d^2)",
        "1/((d-4)*(2*d-9)*(d-5)^4)",
        "(3*d-14)^3/(7*d-27)",
    ] {
        let rf = parse_rf(f).unwrap();
        let mine = expand_rf(&rf, &d0, 4, &tv);
        let c = lau.coeff_from_text(&alg, f, 0, &[]).unwrap();
        let theirs = lau.expand_coeff(&c, 4).unwrap().unwrap();
        let (tv_, tp) = SymbolicaLaurent::series_order(&theirs);
        assert_eq!(mine.valuation(), tv_, "{f}");
        for e in mine.valuation().unwrap()..4.min(tp) {
            assert_eq!(
                Some(mine.coeff(e).unwrap()),
                SymbolicaLaurent::series_coefficient(&theirs, e),
                "{f} at t^{e}"
            );
        }
    }
}
