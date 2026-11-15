//! Lane regbasis, constructed tests (exact stand-in of the composition; Symbolica licence needed, else SKIP).
//!
//! Toy node: 2 lines + 2 numerators, terminals T1 = corner, T2 = a dotted corner, composed coefficients given exactly.

use std::collections::HashMap;

use rustred_campaign_algebra::licensed;
use rustred_campaign_algebra::regbasis::build::{BuildInput, BuildResult, Published, construct};
use rustred_campaign_algebra::regbasis::exact::row_times;
use rustred_campaign_algebra::regbasis::lattice::HermiteLattice;
use rustred_campaign_algebra::regbasis::series::{QP, parse_rf, parse_rational, rf_valuation, t_vars};
use rustred_campaign_algebra::regbasis::standin::{Formula, StandIn};
use rustred_campaign_algebra::regbasis::tower::{TowerInput, TowerSeed, tower};
use rustred_campaign_core::boxdef::BoxDef;
use rustred_campaign_core::key::Key;
use rustred_campaign_core::regbasis::record::{Binding, verify};
use rustred_campaign_core::regbasis::{MapAvailability, RegBasisSpec, band_keys, rank_of};
use serde_json::json;
use symbolica::domains::rational::{Q, Rational};

const CHART: [bool; 4] = [true, true, false, false];

fn cmp(a: &[i32], b: &[i32]) -> std::cmp::Ordering {
    a.cmp(b)
}

fn t1() -> Key {
    vec![1, 1, 0, 0]
}
fn t2() -> Key {
    vec![2, 1, 0, 0]
}

/// The band: top-sector keys with dots <= 2, rank <= r_max, line powers <= 3 (a BoxDef: data).
fn band(r_max: u32) -> (BoxDef, Vec<Key>) {
    let v = json!({"schema": "rustred.campaign.box.v1", "id": "toy", "name": "toy band", "measure": "dots_rank",
        "chart": "1100", "layers": [[0, r_max], [1, r_max], [2, r_max]], "sectors": [], "cap": 3, "slot_caps": [],
        "keys": [], "origin": null, "derived_from": null});
    let b = BoxDef::from_value(&v).expect("box");
    let k = band_keys(&b);
    (b, k)
}

fn binding(b: &BoxDef) -> Binding {
    Binding {
        node_id: "toy".into(),
        node_identity_digest: "toy-identity".into(),
        box_id: b.id.clone(),
        box_digest: b.digest(),
        closure_statement_digest: "toy-statement".into(),
        d0: "4".into(),
    }
}

/// Toy 1: hidden regular basis {T1, X}, T2 = T1 + (d-4) X. c_R(K) = (f1, f2), c_T = (f1 - f2/(d-4), f2/(d-4)).
fn toy1(k: &[i32]) -> Option<Vec<String>> {
    if k == t1().as_slice() {
        return Some(vec!["1".into(), "0".into()]);
    }
    if k == t2().as_slice() {
        return Some(vec!["0".into(), "1".into()]);
    }
    let (a, b, c, e) = (k[0], k[1], k[2], k[3]);
    let f1 = format!("({a} + {b}*d - ({c}))");
    let f2 = format!("({b} + 2*({e}) + {a}*(d-4) + 1)");
    Some(vec![format!("{f1} - {f2}/(d-4)"), format!("{f2}/(d-4)")])
}

fn run(f: Formula, band_keys: &[Key], r_max: Option<u32>, spec: &RegBasisSpec, terms: &[Key], undecided: &[Key], b: Option<Binding>) -> BuildResult {
    let mut sc = StandIn::new(terms.len(), parse_rational(&spec.d0).unwrap(), f);
    for k in undecided {
        sc.undecided.insert(k.clone());
    }
    let inp = BuildInput {
        chart: &CHART,
        terminals: terms,
        band: band_keys,
        band_r_max: r_max,
        cmp: &cmp,
        spec,
        published: None,
        availability: None,
        binding: b,
    };
    construct(&mut sc, &inp).expect("construct")
}

#[test]
fn toy_dependent_terminals_regular_set_found() {
    if !licensed() {
        eprintln!("SKIP: no Symbolica licence");
        return;
    }
    let (bx, keys) = band(2);
    let spec = RegBasisSpec::default();
    let terms = vec![t1(), t2()];
    let r = run(Box::new(toy1), &keys, None, &spec, &terms, &[], Some(binding(&bx)));
    assert!(r.regular, "{}", r.verdict);
    assert_eq!(r.q, 1);
    assert_eq!(r.lattice.length(), 1);
    assert_eq!(r.ranks.last().unwrap().elementary_t, vec![1]);
    assert_eq!(r.regular_set, vec![t1(), vec![1, 1, -1, 0]], "T1 kept, then the first weight-1 key with a polar part");
    assert_eq!(r.sources, vec!["terminal".to_string(), "candidate".to_string()]);
    assert_eq!(r.kept_terminals, vec![0]);
    let x = r.exact.as_ref().expect("exact");
    assert_eq!(x.det_m_order, Some(1), "det M has a simple zero");
    assert!(x.m_orders.iter().flatten().all(|o| o.is_none_or(|v| v >= 0)), "M regular");
    assert!(x.a_orders.iter().flatten().any(|o| o.is_some_and(|v| v < 0)), "M^-1 has a pole (T degenerate)");
    assert_eq!(r.cert_lattice, Some(true));
    assert_eq!(r.cert_det, Some(true));
    assert_eq!(r.cert_poles_check, Some(true), "lane poles' master-map check agrees");
    assert_eq!(r.cert_keys_checked, Some(r.keys_decided));
    assert_eq!(r.cert_keys_with_pole, Some(0), "every band key finite on R");
    assert!(r.keys_with_pole_on_t > 0);
    assert_eq!(r.r_complete, Some(2));
    assert!(r.verdict.contains("full band"));
    let need_t: Vec<i64> = r.needs[0].rows.iter().map(|x| x.need).collect();
    let need_r: Vec<i64> = r.needs[1].rows.iter().map(|x| x.need).collect();
    assert_eq!(need_t, vec![1, 1]);
    assert_eq!(need_r, vec![0, 0]);
    assert_eq!(r.terminal_in_band_lattice, vec![true, true], "terminals are band keys");
    verify(&r.record).expect("sealed record verifies");
    assert_eq!(r.record["binding"]["box"]["digest"], json!(bx.digest()));
}

/// Toy 2: like toy 1 on ranks <= 1; rank-2 keys get an extra double pole along e1 for some keys.
fn toy2(k: &[i32]) -> Option<Vec<String>> {
    let v = toy1(k)?;
    let r = -(k[2] + k[3]);
    if r == 2 && k != t1().as_slice() && k != t2().as_slice() {
        let h = k[0] + k[2]; // non-zero for some rank-2 keys
        return Some(vec![format!("{} + ({h})/(d-4)^2", v[0]), v[1].clone()]);
    }
    Some(v)
}

#[test]
fn rank_two_failure_of_a_rank_one_basis() {
    if !licensed() {
        eprintln!("SKIP: no Symbolica licence");
        return;
    }
    let (_, keys) = band(2);
    let spec = RegBasisSpec::default();
    let terms = vec![t1(), t2()];
    let full = run(Box::new(toy2), &keys, None, &spec, &terms, &[], None);
    assert!(full.regular);
    assert!(full.ranks[2].grows, "the lattice grows at rank 2");
    assert!(full.ranks[2].length_t > full.ranks[1].length_t);
    let w = full.ranks[2].new_direction_key.clone().expect("a witness of rank 2");
    assert_eq!(rank_of(&CHART, &w), Some(2));
    assert_eq!(full.cert_keys_with_pole, Some(0));
    // the band known only through rank 1: rank-2 keys undecided
    let rank2: Vec<Key> = keys.iter().filter(|k| rank_of(&CHART, k) == Some(2)).cloned().collect();
    let part = run(Box::new(toy2), &keys, Some(2), &spec, &terms, &rank2, None);
    assert_eq!(part.r_complete, Some(1));
    assert!(part.verdict.contains("through rank 1 (lower bound"), "{}", part.verdict);
    assert_ne!(part.regular_set, full.regular_set, "the rank-1 basis is not the full-band basis");
    // the rank-1 basis FAILS at rank 2: some rank-2 key has a pole on it
    let m = &part.exact.as_ref().unwrap().m;
    let d0 = Rational::from(4);
    let mut failed = false;
    for k in &rank2 {
        let c: Vec<_> = toy2(k).unwrap().iter().map(|t| parse_rf(t).unwrap()).collect();
        let on_r = row_times(&c, m);
        if on_r.iter().any(|x| rf_valuation(x, &d0).is_some_and(|v| v < 0)) {
            failed = true;
        }
    }
    assert!(failed, "a set regular through rank 1 fails at rank 2");
}

#[test]
fn permutation_invariance() {
    if !licensed() {
        eprintln!("SKIP: no Symbolica licence");
        return;
    }
    let (_, keys) = band(2);
    let mut spec = RegBasisSpec::default();
    spec.terminals_first = false;
    let terms = vec![t1(), t2()];
    let a = run(Box::new(toy1), &keys, None, &spec, &terms, &[], None);
    // (a) streaming order of the band
    let mut rev = keys.clone();
    rev.reverse();
    let b = run(Box::new(toy1), &rev, None, &spec, &terms, &[], None);
    assert_eq!(a.regular_set, b.regular_set);
    assert!(a.lattice.equals(&b.lattice));
    assert_eq!(a.exact.as_ref().unwrap().m_text, b.exact.as_ref().unwrap().m_text);
    // (b) order of the terminals (coordinates permuted)
    let swapped: Formula = Box::new(|k: &[i32]| toy1(k).map(|v| vec![v[1].clone(), v[0].clone()]));
    let terms2 = vec![t2(), t1()];
    let c = run(swapped, &keys, None, &spec, &terms2, &[], None);
    assert_eq!(a.regular_set, c.regular_set, "the regular set does not depend on the terminal order");
    assert_eq!(a.lattice.length(), c.lattice.length());
    let (ma, mc) = (&a.exact.as_ref().unwrap().m_text, &c.exact.as_ref().unwrap().m_text);
    assert_eq!(ma[0], mc[1]);
    assert_eq!(ma[1], mc[0]);
    // with the terminal preference on, the invariants still agree
    let spec2 = RegBasisSpec::default();
    let d = run(Box::new(toy1), &keys, None, &spec2, &terms, &[], None);
    assert!(d.regular && a.regular);
    assert_eq!(d.lattice.length(), a.lattice.length());
}

/// Index-dependent poles: (d - 2 n1) vanishes at d = 4 only for n1 = 2.
fn toy_index(k: &[i32]) -> Option<Vec<String>> {
    if k == t1().as_slice() {
        return Some(vec!["1".into(), "0".into()]);
    }
    if k == t2().as_slice() {
        return Some(vec!["0".into(), "1".into()]);
    }
    let (a, b, c, e) = (k[0], k[1], k[2], k[3]);
    Some(vec![format!("({a} - ({c}))/(d - 2*{b})"), format!("(1 + {e}*{e})/(d - 2*{b}) + 1")])
}

#[test]
fn index_dependent_poles() {
    if !licensed() {
        eprintln!("SKIP: no Symbolica licence");
        return;
    }
    let (_, keys) = band(2);
    let spec = RegBasisSpec::default();
    let terms = vec![t1(), t2()];
    let r = run(Box::new(toy_index), &keys, None, &spec, &terms, &[], None);
    let polar = keys.iter().filter(|k| k[1] == 2).count();
    assert_eq!(r.keys_with_pole_on_t, polar, "exactly the keys with n1 = 2 are polar");
    assert!(r.regular);
    assert_eq!(r.lattice.length(), 2, "two independent residue directions");
    assert_eq!(r.ranks.last().unwrap().elementary_t, vec![1, 1]);
    assert_eq!(r.cert_keys_with_pole, Some(0));
    assert!(r.regular_set.iter().all(|k| k[1] == 2), "R spans the two polar directions: {:?}", r.regular_set);
}

#[test]
fn rational_d0() {
    if !licensed() {
        eprintln!("SKIP: no Symbolica licence");
        return;
    }
    let (_, keys) = band(1);
    let mut spec = RegBasisSpec::default();
    spec.d0 = "17/7".into();
    let terms = vec![t1(), t2()];
    let f: Formula = Box::new(|k: &[i32]| toy1(k).map(|v| v.iter().map(|s| s.replace("(d-4)", "(7*d-17)")).collect()));
    let r = run(f, &keys, None, &spec, &terms, &[], None);
    assert!(r.regular);
    assert_eq!(r.lattice.length(), 1);
    assert_eq!(r.exact.as_ref().unwrap().det_m_order, Some(1));
    // the same data at d0 = 4 has no pole at all
    let mut spec4 = spec.clone();
    spec4.d0 = "4".into();
    let f4: Formula = Box::new(|k: &[i32]| toy1(k).map(|v| v.iter().map(|s| s.replace("(d-4)", "(7*d-17)")).collect()));
    let r4 = run(f4, &keys, None, &spec4, &terms, &[], None);
    assert_eq!(r4.lattice.length(), 0);
    assert_eq!(r4.regular_set, terms, "regular T is kept");
}

#[test]
fn hermite_lattice_basics() {
    if !licensed() {
        eprintln!("SKIP: no Symbolica licence");
        return;
    }
    let tv = t_vars();
    let p = |c: &[(u16, i64)]| {
        let mut x = QP::new(&Q, None, tv.clone());
        for &(e, v) in c {
            x.append_monomial(Rational::from(v), &[e]);
        }
        x
    };
    // Λ = t^3 O^2 + O (t, 1 + t) + O (0, t^2)
    let mut l = HermiteLattice::new(2, 3, tv.clone());
    assert_eq!(l.insert(&[p(&[(1, 1)]), p(&[(0, 1), (1, 1)])]).unwrap(), 3, "pivots t and t^2: growth 6 - 3");
    assert!(l.contains(&[p(&[(2, 1)]), p(&[(1, 1), (2, 1)])]));
    assert!(!l.contains(&[p(&[(0, 1)]), p(&[])]));
    let mut l2 = HermiteLattice::new(2, 3, tv.clone());
    l2.insert(&[p(&[(2, 3)]), p(&[(1, 3), (2, 3)])]).unwrap(); // t * (generator), times the unit 3
    l2.insert(&[p(&[(1, 1)]), p(&[(0, 1), (1, 1)])]).unwrap();
    assert!(l.equals(&l2), "order of insertion and unit multiples do not matter");
    let s = l.smith_exponents();
    assert_eq!(s.iter().sum::<u32>() as u64, l.det_valuation());
    let mut l3 = l.clone();
    l3.rescale(2);
    assert_eq!(l3.length(), l.length(), "rescaling keeps the length over the socle");
}

/// Tower toy: dot-free key of rank r has c_T = (x + 1, (1 + y)/(d-4)^r) (x, y = |numerator powers|).
fn toy_tower(k: &[i32]) -> Option<Vec<String>> {
    if k == t1().as_slice() {
        return Some(vec!["1".into(), "0".into()]);
    }
    if k == t2().as_slice() {
        return Some(vec!["0".into(), "1".into()]);
    }
    let (x, y) = (-k[2], -k[3]);
    let r = x + y;
    if k[0] == 1 && k[1] == 1 {
        Some(vec![format!("{}", x + 1), format!("(1 + {y})/(d-4)^{r}")])
    } else {
        Some(vec!["d".into(), "1".into()])
    }
}

#[test]
fn tower_test_z_per_rank() {
    if !licensed() {
        eprintln!("SKIP: no Symbolica licence");
        return;
    }
    let spec = RegBasisSpec::default();
    let terms = vec![t1(), t2()];
    let mut sc = StandIn::new(2, Rational::from(4), Box::new(toy_tower));
    let inp = TowerInput { chart: &CHART, terminals: &terms, cmp: &cmp, r_max: 3, spec: &spec, published: None, seed: None, binding: None };
    let t = tower(&mut sc, &inp).unwrap();
    let z: Vec<u64> = t.ranks.iter().map(|x| x.length).collect();
    assert_eq!(z, vec![0, 1, 2, 3], "z grows by one per rank");
    assert_eq!(t.ranks[1].first_new_key, Some(vec![1, 1, -1, 0]), "first dot-free key in the node order");
    assert_eq!(t.z_through, Some((3, 3)));
    assert!(t.verdict.contains("through rank 3"));
    // on published masters T = Q P with Q = diag(1, d-4): one order less
    let p = Published { labels: vec!["P1".into(), "P2".into()], keys: vec![None, None], q: vec![vec!["1".into(), "0".into()], vec!["0".into(), "d-4".into()]] };
    let mut sc = StandIn::new(2, Rational::from(4), Box::new(toy_tower));
    let inp = TowerInput { published: Some(&p), ..inp };
    let tp = tower(&mut sc, &inp).unwrap();
    let zp: Vec<u64> = tp.ranks.iter().map(|x| x.length).collect();
    assert_eq!(zp, vec![0, 0, 1, 2]);
    assert_eq!(tp.reference, "P");
    // seeded with a band construction through rank 1 (exact there)
    let (_, keys) = band(1);
    let b = run(Box::new(toy_tower), &keys, None, &spec, &terms, &[], None);
    let seed = TowerSeed::from_build(&b, false).expect("seed");
    let mut sc = StandIn::new(2, Rational::from(4), Box::new(toy_tower));
    let inp = TowerInput { chart: &CHART, terminals: &terms, cmp: &cmp, r_max: 3, spec: &spec, published: None, seed: Some(seed), binding: None };
    let ts = tower(&mut sc, &inp).unwrap();
    assert!(ts.ranks[0].source.starts_with("band"));
    assert_eq!(ts.ranks.last().unwrap().length, 3);
}

#[test]
fn needs_on_published_masters_and_eps_box() {
    if !licensed() {
        eprintln!("SKIP: no Symbolica licence");
        return;
    }
    let (_, keys) = band(1);
    let spec = RegBasisSpec::default();
    let terms = vec![t1(), t2()];
    // published P with T = Q P, Q = [[1, 0], [1, d-4]]: T2 = P1 + (d-4) P2 (a singular published basis)
    let map = rustred_campaign_algebra::valuation::basis::read_master_map(&json!({
        "rows": ["I(1,1,0,0)", "I(2,1,0,0)"], "cols": ["P1", "P2"], "entries": [["1", "0"], ["1", "d-4"]]})).unwrap();
    let p = Published::from_master_map(&map, &terms).unwrap();
    let mut av = MapAvailability { default: Some(0), keys: HashMap::new(), labels: HashMap::new() };
    av.labels.insert("P1".into(), 2);
    let mut sc = StandIn::new(2, Rational::from(4), Box::new(toy1));
    let inp = BuildInput { chart: &CHART, terminals: &terms, band: &keys, band_r_max: None, cmp: &cmp, spec: &spec,
        published: Some(&p), availability: Some(&av), binding: None };
    let r = construct(&mut sc, &inp).unwrap();
    assert!(r.regular);
    let np = &r.needs[2];
    assert_eq!(np.basis, "P");
    // c_P = c_T Q: (f1 - f2/t + f2/t, f2) = (f1, f2): no pole on P (P = the hidden regular basis)
    assert_eq!(np.rows.iter().map(|x| x.max_pole).collect::<Vec<_>>(), vec![Some(0), Some(0)]);
    assert_eq!(np.missing_total, Some(0));
    let nt = &r.needs[0];
    assert_eq!(nt.missing_total, Some(2), "T needs one order more than the default availability 0, on both");
}

#[test]
fn adapter_on_epsdepth_text_reduction() {
    use rustred_campaign_algebra::SymbolicaAlgebra;
    use rustred_campaign_algebra::epsdepth::{SymbolicaLaurent, TextReduction};
    use rustred_campaign_algebra::regbasis::adapter::EpsdepthCompose;
    use rustred_campaign_core::epsdepth::ComposeParams;
    use rustred_campaign_core::regbasis::Compose;
    if !licensed() {
        eprintln!("SKIP: no Symbolica licence");
        return;
    }
    let (x, y, z) = (vec![1, 2, 0, 0], vec![1, 1, -1, 0], vec![1, 1, 0, -1]);
    let lau = SymbolicaLaurent::new("4").unwrap();
    let mut src = TextReduction::new(SymbolicaAlgebra::new().unwrap(), lau.clone(), 4);
    src.terminal(&t1());
    src.terminal(&t2());
    // a route pole (1/(d-4)) that cancels on Y and survives on X and Z
    src.relation(&x, "rx", &[(&t2(), "1/(d-4)"), (&t1(), "-1/(d-4)")]);
    src.relation(&y, "ry", &[(&x, "d-4"), (&t1(), "3")]);
    src.relation(&z, "rz", &[(&y, "2"), (&x, "-1")]);
    let mut ad = EpsdepthCompose::new(&mut src, lau, 2, ComposeParams::default(), 1000);
    let expected: HashMap<Key, Vec<String>> = [
        (t1(), vec!["1".to_string(), "0".to_string()]),
        (t2(), vec!["0".to_string(), "1".to_string()]),
        (x.clone(), vec!["-1/(d-4)".to_string(), "1/(d-4)".to_string()]),
        (y.clone(), vec!["2".to_string(), "1".to_string()]),
        (z.clone(), vec!["4 + 1/(d-4)".to_string(), "2 - 1/(d-4)".to_string()]),
    ]
    .into_iter()
    .collect();
    // exact composition along the cone
    for (k, want) in &expected {
        let got = ad.exact(k).unwrap();
        for (g, w) in got.iter().zip(want) {
            let diff = &parse_rf(g).unwrap() - &parse_rf(w).unwrap();
            assert!(diff.numerator.is_zero(), "exact {k:?}: {g} vs {w}");
        }
    }
    // streamed series agree with the stand-in's exact expansions
    let keys: Vec<Key> = vec![t1(), t2(), x.clone(), y.clone(), z.clone()];
    let mut sc = StandIn::from_table(2, Rational::from(4), expected.clone());
    let (mut a, mut b) = (vec![None; 5], vec![None; 5]);
    ad.stream(&keys, 1, &mut |i, r| a[i] = Some(r.unwrap())).unwrap();
    sc.stream(&keys, 1, &mut |i, r| b[i] = Some(r.unwrap())).unwrap();
    assert_eq!(a, b);
    // the construction through the adapter
    let spec = RegBasisSpec::default();
    let terms = vec![t1(), t2()];
    let inp = BuildInput { chart: &CHART, terminals: &terms, band: &keys, band_r_max: None, cmp: &cmp, spec: &spec,
        published: None, availability: None, binding: None };
    let r = construct(&mut ad, &inp).unwrap();
    assert!(r.regular, "{}", r.verdict);
    assert_eq!(r.regular_set, vec![t1(), z.clone()]);
    assert_eq!(r.cert_poles_check, Some(true));
    assert_eq!(r.cert_keys_with_pole, Some(0));
}
