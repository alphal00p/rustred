//! T4 (parser, evaluation, key classes, arities 5/9/14/20) and T3 (negative controls, elided witness loci, overflow).
//! No external data.

use rustred_campaign_core::cover::{Dispatch, Dispatcher, Table};
use rustred_campaign_core::domain::{CaseDomain, DomainOptions, NotInCase, PoleKind, Verdict};
use rustred_campaign_core::key::{KeyClass, NodeFrame, compositions, key_text, parse_key};
use rustred_campaign_core::poly::{EvalError, Poly, PolyError, Value};
use rustred_campaign_core::record::{ReadOptions, RecordReader, RhsMode, TableLine};

fn p(s: &str, n: usize) -> Poly {
    Poly::parse_d_free(s, n).unwrap()
}

#[test]
fn parser_forms_found_in_records() {
    let n = 20;
    // "n0+n11+1", "-2+n0", atoms "n7-(2)", "(0)-n9", cached "0+1*n15", "1-1*n1", equation "a=b".
    let k: Vec<i32> = (0..20)
        .map(|i| if i < 9 { 1 + (i % 3) } else { -(i % 4) })
        .collect();
    assert_eq!(
        p("n0+n11+1", n).eval(&k).unwrap(),
        Value::Small((k[0] + k[11] + 1) as i128)
    );
    assert_eq!(
        p("-2+n0", n).eval(&k).unwrap(),
        Value::Small((k[0] - 2) as i128)
    );
    assert_eq!(
        p("n7-(2)", n).eval(&k).unwrap(),
        Value::Small((k[7] - 2) as i128)
    );
    assert_eq!(
        p("(0)-n9", n).eval(&k).unwrap(),
        Value::Small(-k[9] as i128)
    );
    assert_eq!(p("0+1*n15", n), p("n15", n));
    assert_eq!(p("1-1*n1", n), p("-n1+1", n));
    assert_eq!(p("n3+n8=n12+3", n), p("n3+n8-n12-3", n));
    assert_eq!(p("n0^2-2*n0*n1+n1^2", n), p("(n0-n1)^2", n));
    assert_eq!(p("-n0^2", n).eval(&[3; 20]).unwrap(), Value::Small(-9));
    assert_eq!(p("2*(n0+1)*(n0-1)", n), p("2*n0^2-2", n));
    assert!(p("n0-n0", n).is_zero());
    // display round trip
    let q = p("-30-16*n9-2*n9^2+17*n6+6*n6*n9+n6*n9^2-n6^2", n);
    assert_eq!(Poly::parse_d_free(&q.to_string(), n).unwrap(), q);
    // errors
    assert_eq!(Poly::parse_d_free("n0+d", n), Err(PolyError::ContainsD));
    assert!(matches!(
        Poly::parse_d_free("n20", n),
        Err(PolyError::VariableOutOfRange { var: 20, arity: 20 })
    ));
    assert_eq!(Poly::parse_d_free("n0/(n1-1)", n), Err(PolyError::Division));
    assert!(matches!(
        Poly::parse_d_free("n0+", n),
        Err(PolyError::Syntax { .. })
    ));
    assert!(matches!(
        Poly::parse_d_free("n0 n1", n),
        Err(PolyError::Syntax { .. })
    ));
    assert_eq!(
        Poly::parse_d_free("999999999999999999999999999999999999999999", n),
        Err(PolyError::CoefficientOverflow)
    );
    // d where allowed
    let pd = Poly::parse("-2*n3-n0+d", n, true).unwrap();
    assert!(pd.has_d());
    assert_eq!(pd.eval(&[0; 20]), Err(EvalError::ContainsD));
    let dc = pd.d_coefficients();
    assert_eq!(dc.len(), 2);
    assert_eq!(dc[1], (1, Poly::constant(1)));
}

#[test]
fn overflow_path_is_exact_and_explicit() {
    let q = p("n0^20-n1^20", 2);
    let k = [1000, 1000];
    assert_eq!(q.eval_checked_i128(&k), Err(EvalError::Overflow));
    assert!(q.eval(&k).unwrap().is_zero()); // exact fallback, not a wrap
    let k2 = [1000, 999];
    let v = q.eval(&k2).unwrap();
    assert!(v.is_big());
    assert_eq!(v.signum(), 1);
    // the same through a domain: an equation that only the big path decides
    let mut d = CaseDomain::free(2, None);
    d.equations.push(q.clone());
    assert_eq!(d.applies(&k), Verdict::Applies);
    assert_eq!(d.applies(&k2), Verdict::NotInCase(NotInCase::Equation(0)));
}

#[test]
fn key_classes_and_sectors() {
    let f = NodeFrame::lines_first(5, 3);
    assert_eq!(f.classify(&[1, 2, 1, 0, -1]).unwrap(), KeyClass::Chart);
    assert_eq!(
        f.classify(&[1, 2, 1, 1, 0]).unwrap(),
        KeyClass::TopPositiveNumerator { slots: vec![3] }
    );
    assert_eq!(
        f.classify(&[1, 0, 1, 2, 0]).unwrap(),
        KeyClass::Subsector {
            removed_lines: vec![1],
            positive_numerators: vec![3]
        }
    );
    assert!(f.is_top_sector(&[1, 1, 1, 3, 0]));
    assert!(!f.in_chart_orthant(&[1, 1, 1, 3, 0]));
    assert_eq!(f.chart_violation(&[1, 1, 1, 3, 0]), Some(3));
    assert_eq!(f.sector(&[1, -1, 2, 0, 0]), 0b101);
    assert_eq!(f.sector_slots(&[1, -1, 2, 0, 0]), vec![0, 2]);
    assert_eq!(f.dots(&[1, 3, 2, 0, -2]), Some(3));
    assert_eq!(f.rank(&[1, 3, 2, 0, -2]), Some(2));
    assert_eq!(f.dots(&[1, 3, 2, 1, -2]), None);
    assert!(f.classify(&[1, 1]).is_err());
    let m = NodeFrame::from_mask("11111111110000000000").unwrap();
    assert_eq!((m.arity(), m.n_lines()), (20, 10));
    assert!(NodeFrame::from_mask("11x").is_err());
    let s = m.clone().with_power_shifts(
        vec!["0".into(); 19]
            .into_iter()
            .chain(["1/2".to_string()])
            .collect(),
    );
    assert!(s.require_integer_only().is_err());
    assert!(m.require_integer_only().is_ok());
    assert_eq!(parse_key("I(1,2,-3)").unwrap(), vec![1, 2, -3]);
    assert_eq!(parse_key("[1, 2, -3]").unwrap(), vec![1, 2, -3]);
    assert_eq!(key_text(&[1, 0, -2]), "I(1,0,-2)");
}

#[test]
fn layer_counts_match_combinatorics() {
    // box-layer sizes from the checker's README (T162 box B: 9 lines, 11 numerators).
    let f = NodeFrame::lines_first(20, 9);
    assert_eq!(f.chart_layer(0, 1, Some(5)).len(), 11);
    assert_eq!(f.chart_layer(1, 2, Some(5)).len(), 594);
    assert_eq!(f.chart_layer(4, 2, Some(5)).len(), 32670);
    assert_eq!(f.chart_layer(4, 0, Some(5)).len(), 495);
    assert_eq!(compositions(2, 3, 2).len(), 6);
}

/// A rule domain at each arity; a chart key at that arity; the domain applies, then each mutation changes it.
#[test]
fn arities_5_9_14_20() {
    for (n, lines) in [(5usize, 3usize), (9, 6), (14, 9), (20, 10)] {
        let f = NodeFrame::lines_first(n, lines);
        let mut d = CaseDomain::free(n, Some((0..n).map(|i| f.is_line(i)).collect()));
        d.fixed[0] = Some(2);
        d.equations.push(p(&format!("n1-n{}", lines - 1), n)); // n1 = n_{lines-1}
        d.exceptions.push(vec![p(&format!("n{}", n - 1), n)]); // last numerator = 0 excluded
        let mut k = vec![1i32; n];
        for x in k.iter_mut().skip(lines) {
            *x = 0;
        }
        k[0] = 2;
        k[n - 1] = -1;
        assert_eq!(d.applies(&k), Verdict::Applies, "arity {n}");
        let mut k2 = k.clone();
        k2[n - 1] = 0;
        assert_eq!(d.applies(&k2), Verdict::OnException { branch: 0 });
        let mut k3 = k.clone();
        k3[1] = 3;
        assert_eq!(d.applies(&k3), Verdict::NotInCase(NotInCase::Equation(0)));
        let mut k4 = k.clone();
        k4[lines] = 1; // positive numerator
        assert_eq!(d.applies(&k4), Verdict::ChartMismatch { slot: lines });
        assert_eq!(d.applies_in_chart(&k4), Verdict::Applies); // case test alone is separate
        let mut k5 = k.clone();
        k5[2 % lines] = 0; // removed line (slot 2 for lines >= 3)
        assert!(matches!(d.applies(&k5), Verdict::ChartMismatch { .. }));
        assert_eq!(
            d.applies(&k[..n - 1]),
            Verdict::NotInCase(NotInCase::Arity {
                expected: n,
                got: n - 1
            })
        );
    }
}

const RULE_LINE: &str = r#"{"type":"rule","chart":"11111111110000000000","index":4,"case":"Coordinate(...)","fixed":[null,null,null,1,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,0],"equations":["n0=n1+1"],"target":"I(n0,n1,n2,1,n4,n5,n6,n7,n8,n9,n10,n11,n12,n13,n14,n15,n16,n17,n18,0)","rhs":[["I(n0,n1,n2,1,n4+1,n5,n6,n7-1,n8,n9,n10,n11,n12,n13,n14-1,n15,n16,n17,n18,0)","-n4/(-1+n7)"]],"exceptions":[["-1+n7"],["n18","n17"]],"dispatch":"Partition","witness_loci":["1-n7"],"recenter":[0,0,0,0,0,0,0,-1,0,0,0,0,0,0,0,0,0,0,1,0],"guard_version":2}"#;

fn rule_from(line: &str) -> rustred_campaign_core::record::RuleRecord {
    let mut rd = RecordReader::new(
        line.as_bytes(),
        ReadOptions {
            rhs: RhsMode::KeysOnly,
        },
    );
    match rd.next().unwrap().unwrap() {
        TableLine::Rule(r) => *r,
        o => panic!("{o:?}"),
    }
}

/// T3: planted exception, dropped equation, shifted fixed slot each change the verdict at a known key.
#[test]
fn negative_controls_change_verdicts() {
    let frame = NodeFrame::from_mask("11111111110000000000").unwrap();
    let r = rule_from(RULE_LINE);
    assert_eq!(r.rhs_keys.as_ref().map(Vec::len), Some(1));
    let d = CaseDomain::from_rule_record(&r, &frame, DomainOptions::default()).unwrap();
    let mut k = vec![1i32; 20];
    for x in k.iter_mut().skip(10) {
        *x = 0;
    }
    k[0] = 2; // n0 = n1 + 1
    k[7] = 2; // off exception n7 = 1
    k[18] = -1; // off the second branch
    assert_eq!(d.applies(&k), Verdict::Applies);
    // planted exception: n0 = 2 AND n18 = -1
    let mut planted = d.clone();
    planted.exceptions.push(vec![p("n0-2", 20), p("n18+1", 20)]);
    assert_eq!(planted.applies(&k), Verdict::OnException { branch: 2 });
    // dropped equation: a key that violates n0 = n1 + 1 is admitted only after the drop
    let mut kk = k.clone();
    kk[0] = 3;
    assert_eq!(d.applies(&kk), Verdict::NotInCase(NotInCase::Equation(0)));
    let mut dropped = d.clone();
    dropped.equations.clear();
    assert_eq!(dropped.applies(&kk), Verdict::Applies);
    // shifted fixed slot: n3 fixed to 2 instead of 1
    let mut shifted = d.clone();
    shifted.fixed[3] = Some(2);
    assert_eq!(
        shifted.applies(&k),
        Verdict::NotInCase(NotInCase::FixedSlot {
            slot: 3,
            fixed: 2,
            got: 1
        })
    );
    // the record's own exceptions and witness locus
    let mut ke = k.clone();
    ke[7] = 1;
    assert_eq!(d.applies(&ke), Verdict::OnException { branch: 0 });
    let mut no_exc = d.clone();
    no_exc.exceptions.clear();
    assert_eq!(
        no_exc.applies(&ke),
        Verdict::OnPole {
            kind: PoleKind::Witness,
            index: 0
        }
    );
    // recenter bounds (opt-in): n7 >= 2, n18 <= -1
    let o = DomainOptions {
        recenter_bounds: true,
        ..Default::default()
    };
    let dr = CaseDomain::from_rule_record(&r, &frame, o).unwrap();
    assert_eq!(dr.applies(&k), Verdict::Applies);
    let mut kr = k.clone();
    kr[18] = 0;
    kr[17] = -1;
    assert_eq!(d.applies(&kr), Verdict::Applies);
    assert!(matches!(
        dr.applies(&kr),
        Verdict::NotInCase(NotInCase::Bound(_))
    ));
}

#[test]
fn elided_witness_loci_never_pass() {
    let frame = NodeFrame::from_mask("11111111110000000000").unwrap();
    let line = RULE_LINE.replace(r#""witness_loci":["1-n7"]"#, r#""witness_loci":["elided"]"#);
    let r = rule_from(&line);
    let d = CaseDomain::from_rule_record(&r, &frame, DomainOptions::default()).unwrap();
    let mut k = vec![1i32; 20];
    for x in k.iter_mut().skip(10) {
        *x = 0;
    }
    k[0] = 2;
    k[7] = 2;
    k[18] = -1;
    assert_eq!(d.applies(&k), Verdict::Elided);
    // first-match dispatch fails closed at the elided rule, even when a later rule applies
    let mut t = Table::new("elided-first");
    t.push(4, d.clone());
    t.push(5, CaseDomain::free(20, None));
    let disp = Dispatcher::new(vec![t], vec![0]).unwrap();
    assert!(matches!(
        disp.first_match(&k),
        Dispatch::Undecided {
            verdict: Verdict::Elided,
            ..
        }
    ));
    // keys outside the case are still decided
    let mut k2 = k.clone();
    k2[3] = 2;
    assert_eq!(disp.first_match(&k2).winner().map(|w| w.label), Some(5));
    // witness check disabled: plain pass (documented option)
    let o = DomainOptions {
        witness: false,
        ..Default::default()
    };
    assert_eq!(
        CaseDomain::from_rule_record(&r, &frame, o)
            .unwrap()
            .applies(&k),
        Verdict::Applies
    );
}

#[test]
fn eps_shifted_frame_is_refused() {
    let frame = NodeFrame::from_mask("11111111110000000000")
        .unwrap()
        .with_power_shifts(
            (0..20)
                .map(|i| if i == 3 { "1".into() } else { "0".into() })
                .collect(),
        );
    let r = rule_from(RULE_LINE);
    assert!(CaseDomain::from_rule_record(&r, &frame, DomainOptions::default()).is_err());
}

#[test]
fn pole_semantics_identically_zero_in_d() {
    // P = (n6-1)*(2*n6 - d): vanishes identically in d iff n6 = 1 (2*n6 - d is never identically zero).
    let mut d = CaseDomain::free(20, None);
    d.poles.push(rustred_campaign_core::domain::PoleLocus::new(
        PoleKind::Denominator,
        &Poly::parse("2*n6-2*n6^2-d+d*n6", 20, true).unwrap(),
    ));
    let mut k = vec![0i32; 20];
    k[6] = 1;
    assert_eq!(
        d.applies(&k),
        Verdict::OnPole {
            kind: PoleKind::Denominator,
            index: 0
        }
    );
    k[6] = 2;
    assert_eq!(d.applies(&k), Verdict::Applies);
}

#[test]
fn first_match_priority_is_data_and_union_is_separate() {
    let mut a = Table::new("A");
    let mut da = CaseDomain::free(5, None);
    da.fixed[0] = Some(1);
    a.push(10, da);
    let mut b = Table::new("B");
    b.push(20, CaseDomain::free(5, None));
    let mut db = CaseDomain::free(5, None);
    db.fixed[0] = Some(1);
    b.push(21, db);
    let k = [1, 1, 1, 0, 0];
    let ab = Dispatcher::new(vec![a.clone(), b.clone()], vec![0, 1]).unwrap();
    let ba = ab.with_priority(vec![1, 0]).unwrap();
    assert_eq!(
        ab.first_match(&k).winner().map(|w| (w.table, w.label)),
        Some((0, 10))
    );
    assert_eq!(
        ba.first_match(&k)
            .winner()
            .map(|w| (w.table, w.record, w.label)),
        Some((1, 0, 20))
    );
    let (u, und) = ab.union_cover(&k);
    assert_eq!(
        u.iter().map(|w| w.label).collect::<Vec<_>>(),
        vec![10, 20, 21]
    );
    assert!(und.is_empty());
    assert_eq!(
        ab.first_match(&[2, 1, 1, 0, 0]).winner().map(|w| w.label),
        Some(20)
    );
    assert!(Dispatcher::new(vec![a.clone(), b.clone()], vec![0, 0]).is_err());
    let rep = ab.batch(&[k.to_vec(), vec![2, 1, 1, 0, 0]], 2);
    assert_eq!(
        (rep.covered, rep.uncovered, rep.wins.clone()),
        (2, 0, vec![1, 1])
    );
    // a table listed by nobody is not used
    let only_a = Dispatcher::new(vec![a, b], vec![0]).unwrap();
    assert_eq!(only_a.first_match(&[2, 1, 1, 0, 0]), Dispatch::Uncovered);
}
