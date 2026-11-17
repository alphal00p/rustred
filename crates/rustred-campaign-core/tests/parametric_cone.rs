//! B2 (interfaces/parametric.md section 2): coverage of a cone at first match. Constructed tables with known gaps,
//! first-match shadowing by a blocking record, a multi-slot bound against a simplex cone; every certificate re-checked
//! by the independent checker (incl. an exhaustive box); a seeded differential against brute-force enumeration.

use rustred_campaign_core::cover::{Dispatch, Dispatcher, Table};
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::parametric::check::check_cone_certificate;
use rustred_campaign_core::parametric::cone::{Cone, ConeOptions, certificate, cone_coverage};
use rustred_campaign_core::poly::Poly;

fn poly(s: &str, n: usize) -> Poly {
    Poly::parse_d_free_exact(s, n).unwrap()
}

struct Rec {
    fixed: Vec<Option<i32>>,
    exceptions: Vec<Vec<&'static str>>,
    bounds: Vec<&'static str>,
    elided: bool,
}

fn rec(n: usize) -> Rec {
    Rec {
        fixed: vec![None; n],
        exceptions: vec![],
        bounds: vec![],
        elided: false,
    }
}

fn disp(frame: &NodeFrame, recs: &[Rec]) -> Dispatcher {
    let n = frame.arity();
    let mut t = Table::new("t", n);
    for (i, r) in recs.iter().enumerate() {
        let mut d = CaseDomain::free(n, Some(frame.mask().to_vec()));
        d.fixed = r.fixed.clone();
        d.exceptions = r
            .exceptions
            .iter()
            .map(|b| b.iter().map(|m| poly(m, n)).collect())
            .collect();
        d.bounds = r.bounds.iter().map(|b| poly(b, n)).collect();
        d.witness_elided = r.elided;
        t.push(i as i64, d).unwrap();
    }
    Dispatcher::new(vec![t], vec![0])
        .unwrap()
        .with_chart(frame.mask().to_vec())
        .unwrap()
}

fn run(f: &NodeFrame, d: &Dispatcher, cone: &Cone, ex: (i32, i32)) -> String {
    let cov = cone_coverage(d, f, cone, &ConeOptions::default()).unwrap();
    let cert = certificate(&cov, d, f);
    let chk = check_cone_certificate(&cert, d, f, Some(ex));
    assert!(chk.ok(), "checker: {:?}\ncertificate: {cert}", chk.problems);
    cov.verdict.to_string()
}

#[test]
fn a_free_record_covers_the_orthant() {
    let f = NodeFrame::without_header_lines_first(3, 2);
    let d = disp(&f, &[rec(3)]);
    assert_eq!(run(&f, &d, &Cone::orthant(), (5, -4)), "covered");
    assert_eq!(
        run(&f, &d, &Cone::excess_simplex(&f, 6), (8, -6)),
        "covered"
    );
}

#[test]
fn a_plane_gap_is_uncovered_on_the_orthant_and_outside_a_cone_that_misses_it() {
    let f = NodeFrame::without_header_lines_first(3, 2);
    let mut r = rec(3);
    r.exceptions = vec![vec!["n0-1"]];
    let d = disp(&f, &[r]);
    let cov = cone_coverage(&d, &f, &Cone::orthant(), &ConeOptions::default()).unwrap();
    assert_eq!(cov.verdict, "uncovered");
    let w: Vec<i32> = serde_json::from_value(cov.listed[0]["v"]["witness"].clone()).unwrap();
    assert_eq!(w[0], 1);
    assert!(!matches!(d.first_match(&w), Ok(Dispatch::Covered(_))));
    // the cone n0 >= 2 misses the gap
    let cone = Cone {
        label: "n0 >= 2".into(),
        constraints: vec![(vec![-1, 0, 0], -2)],
    };
    assert_eq!(run(&f, &d, &cone, (5, -4)), "covered");
    assert_eq!(run(&f, &d, &Cone::orthant(), (5, -4)), "uncovered");
}

#[test]
fn first_match_shadowing_by_a_blocking_record() {
    // A (n1 = 1, elided witness loci) is selected first and blocks; B (free) would serve: union-covered, dispatch-open
    let f = NodeFrame::without_header_lines_first(3, 2);
    let mut a = rec(3);
    a.fixed[1] = Some(1);
    a.elided = true;
    let d = disp(&f, &[a, rec(3)]);
    let cov = cone_coverage(&d, &f, &Cone::orthant(), &ConeOptions::default()).unwrap();
    assert_eq!(cov.verdict, "uncovered");
    let w: Vec<i32> = serde_json::from_value(cov.listed[0]["v"]["witness"].clone()).unwrap();
    assert_eq!(w[1], 1);
    assert_eq!(cov.listed[0]["blocked_by"], 0);
    let cone = Cone {
        label: "n1 >= 2".into(),
        constraints: vec![(vec![0, -1, 0], -2)],
    };
    assert_eq!(run(&f, &d, &cone, (5, -4)), "covered");
    // order matters: with B first nothing is open
    let mut a2 = rec(3);
    a2.fixed[1] = Some(1);
    a2.elided = true;
    let d2 = disp(&f, &[rec(3), a2]);
    assert_eq!(run(&f, &d2, &Cone::orthant(), (5, -4)), "covered");
}

#[test]
fn a_multi_slot_bound_against_simplex_cones() {
    // the record serves D <= 3 only (n0 + n1 <= 5): the cone D + r <= 3 is covered, D + r <= 4 is not
    let f = NodeFrame::without_header_lines_first(3, 2);
    let mut r = rec(3);
    r.bounds = vec!["5-n0-n1"];
    let d = disp(&f, &[r]);
    assert_eq!(
        run(&f, &d, &Cone::excess_simplex(&f, 3), (6, -5)),
        "covered"
    );
    assert_eq!(
        run(&f, &d, &Cone::excess_simplex(&f, 4), (6, -5)),
        "uncovered"
    );
    assert_eq!(run(&f, &d, &Cone::orthant(), (6, -5)), "uncovered");
}

// ---------------------------------------------------------------- seeded differential

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

const EXC: [&str; 10] = [
    "n0-1", "n0-2", "n1-1", "n1-3", "n2", "n2+1", "n0-n1", "n0-n1-1", "n1+n2-1", "n0+n2-2",
];
const BND: [&str; 4] = ["6-n0-n1", "n0-2", "3+n2", "4-n0+n2"];

#[test]
fn differential_against_brute_force() {
    let f = NodeFrame::without_header_lines_first(3, 2);
    let mut rng = Rng(20_261_115);
    let (mut cov_n, mut unc_n, mut und_n) = (0, 0, 0);
    for case in 0..120 {
        let nrec = 1 + rng.below(3) as usize;
        let mut recs = Vec::new();
        for _ in 0..nrec {
            let mut r = rec(3);
            if rng.below(3) == 0 {
                let s = rng.below(3) as usize;
                r.fixed[s] = Some(if s < 2 {
                    1 + rng.below(2) as i32
                } else {
                    -(rng.below(2) as i32)
                });
            }
            for _ in 0..rng.below(3) {
                r.exceptions
                    .push(vec![EXC[rng.below(EXC.len() as u64) as usize]]);
            }
            if rng.below(3) == 0 {
                r.bounds.push(BND[rng.below(BND.len() as u64) as usize]);
            }
            r.elided = rng.below(6) == 0;
            recs.push(r);
        }
        let d = disp(&f, &recs);
        let s = 2 + rng.below(3) as i64;
        let cone = Cone::excess_simplex(&f, s);
        let cov = cone_coverage(&d, &f, &cone, &ConeOptions::default()).unwrap();
        let cert = certificate(&cov, &d, &f);
        let ex = (s as i32 + 1, -(s as i32));
        let chk = check_cone_certificate(&cert, &d, &f, Some(ex));
        assert!(chk.ok(), "case {case}: checker {:?}", chk.problems);
        // brute force: the cone is inside the box
        let mut open = None;
        for a in 1..=ex.0 {
            for b in 1..=ex.0 {
                for c in ex.1..=0 {
                    let k = vec![a, b, c];
                    if cone.contains(&k) && !matches!(d.first_match(&k), Ok(Dispatch::Covered(_))) {
                        open.get_or_insert(k);
                    }
                }
            }
        }
        match cov.verdict {
            "covered" => {
                assert!(open.is_none(), "case {case}: covered but {open:?} is open");
                cov_n += 1;
            }
            "uncovered" => {
                assert!(
                    open.is_some(),
                    "case {case}: uncovered but brute force finds no open key"
                );
                unc_n += 1;
            }
            _ => und_n += 1,
        }
    }
    assert!(
        cov_n > 10 && unc_n > 10,
        "generator too narrow: {cov_n} covered, {unc_n} uncovered, {und_n} undecided"
    );
    assert!(und_n * 5 < 120, "too many undecided: {und_n}");
}

#[test]
fn capped_dispatch_gives_the_same_cone_verdicts() {
    use rustred_campaign_core::parametric::cone::{capped_dispatcher, cone_inside_caps};
    let f = NodeFrame::without_header_lines_first(3, 2);
    let mut rng = Rng(0x00ca_99ed);
    let mut agree = 0;
    for case in 0..80 {
        let mut recs = Vec::new();
        for _ in 0..1 + rng.below(3) as usize {
            let mut r = rec(3);
            for _ in 0..rng.below(3) {
                r.exceptions
                    .push(vec![EXC[rng.below(EXC.len() as u64) as usize]]);
            }
            if rng.below(3) == 0 {
                r.bounds.push(BND[rng.below(BND.len() as u64) as usize]);
            }
            r.elided = rng.below(6) == 0;
            recs.push(r);
        }
        let d = disp(&f, &recs);
        let s = 2 + rng.below(3) as i64;
        let cone = Cone::excess_simplex(&f, s);
        let caps = Cone::box_caps(&f, s + 1, -s);
        assert!(cone_inside_caps(&cone, &caps, &f));
        // a cone sticking out of the caps is refused
        assert!(!cone_inside_caps(
            &Cone::excess_simplex(&f, s + 1),
            &caps,
            &f
        ));
        let cd = capped_dispatcher(&d, &f, &caps).unwrap();
        let a = cone_coverage(&d, &f, &cone, &ConeOptions::default()).unwrap();
        let b = cone_coverage(&cd, &f, &cone, &ConeOptions::default()).unwrap();
        let cert = certificate(&b, &cd, &f);
        let chk = check_cone_certificate(&cert, &cd, &f, Some((s as i32 + 1, -(s as i32))));
        assert!(chk.ok(), "case {case}: {:?}", chk.problems);
        if a.verdict != "undecided" && b.verdict != "undecided" {
            assert_eq!(a.verdict, b.verdict, "case {case}");
            agree += 1;
        }
    }
    assert!(agree > 60, "{agree}");
}

#[test]
fn fall_through_poles_lets_a_witness_locus_reach_the_next_record() {
    use rustred_campaign_core::domain::{PoleKind, PoleLocus};
    use rustred_campaign_core::parametric::cone::fall_through_poles;
    let f = NodeFrame::without_header_lines_first(3, 2);
    let mut a = CaseDomain::free(3, Some(f.mask().to_vec()));
    // witness locus (n0 - 2) * d: vanishes identically in d at n0 = 2
    a.poles.push(PoleLocus::new(
        PoleKind::Witness,
        &Poly::parse_exact("(n0-2)*d", 3, true).unwrap(),
    ));
    let mut t = Table::new("t", 3);
    t.push(0, a).unwrap();
    t.push(1, CaseDomain::free(3, Some(f.mask().to_vec())))
        .unwrap();
    let d = Dispatcher::new(vec![t], vec![0])
        .unwrap()
        .with_chart(f.mask().to_vec())
        .unwrap();
    // the core stops at the blocked record: n0 = 2 is open
    assert!(!matches!(
        d.first_match(&[2, 1, 0]),
        Ok(Dispatch::Covered(_))
    ));
    assert_eq!(run(&f, &d, &Cone::orthant(), (5, -4)), "uncovered");
    // under BLOCK = fall through, record 1 serves it
    let ft = fall_through_poles(&d).unwrap();
    match ft.first_match(&[2, 1, 0]) {
        Ok(Dispatch::Covered(r)) => assert_eq!(r.record, 1),
        o => panic!("{o:?}"),
    }
    match ft.first_match(&[3, 1, 0]) {
        Ok(Dispatch::Covered(r)) => assert_eq!(r.record, 0),
        o => panic!("{o:?}"),
    }
    assert_eq!(run(&f, &ft, &Cone::orthant(), (5, -4)), "covered");
}

#[test]
fn a_nonlinear_locus_inside_a_bounded_cone_is_decided_by_enumeration() {
    // record A excludes the integer points of n0 * n1 = 6 (a nonlinear cut the partition leaves unresolved); in the cone
    // D + r <= 3 only (2, 3) and (3, 2) lie on it
    let f = NodeFrame::without_header_lines_first(3, 2);
    let mut a = rec(3);
    a.exceptions = vec![vec!["n0*n1-6"]];
    let d = disp(&f, &[a]);
    let cone = Cone::excess_simplex(&f, 3);
    let cov = cone_coverage(&d, &f, &cone, &ConeOptions::default()).unwrap();
    assert_eq!(cov.verdict, "uncovered", "{:?}", cov.listed);
    let cert = certificate(&cov, &d, &f);
    assert!(check_cone_certificate(&cert, &d, &f, Some((4, -3))).ok());
    // point records serve those two keys: the locus is covered in the cone
    let mut a2 = rec(3);
    a2.exceptions = vec![vec!["n0*n1-6"]];
    let mut p1 = rec(3);
    p1.fixed = vec![Some(2), Some(3), Some(0)];
    let mut p2 = rec(3);
    p2.fixed = vec![Some(3), Some(2), Some(0)];
    let d2 = disp(&f, &[a2, p1, p2]);
    let cov2 = cone_coverage(&d2, &f, &cone, &ConeOptions::default()).unwrap();
    assert_eq!(
        cov2.verdict, "covered",
        "{:?} {:?}",
        cov2.listed, cov2.partition_flags
    );
    let cert2 = certificate(&cov2, &d2, &f);
    let chk = check_cone_certificate(&cert2, &d2, &f, Some((4, -3)));
    assert!(chk.ok(), "{:?}", chk.problems);
    // on the unbounded orthant the far points (1, 6), (6, 1) stay open
    assert_eq!(run(&f, &d2, &Cone::orthant(), (7, -6)), "uncovered");
}

#[test]
fn face_pieces_split_the_orthant_and_the_cone_verdict_exactly() {
    use rustred_campaign_core::parametric::cone::{capped_dispatcher, face_piece, face_pieces};
    let f = NodeFrame::without_header_lines_first(3, 2);
    // disjoint and covering: every chart key of a box lies in exactly one piece
    let pieces = face_pieces(&f, &[2, 0, 1]).unwrap();
    assert_eq!(pieces.len(), 8);
    for a in 1..6 {
        for b in 1..6 {
            for c in -5..1 {
                let k = [a, b, c];
                let hits = pieces.iter().filter(|p| p.contains(&k)).count();
                assert_eq!(hits, 1, "{k:?}");
            }
        }
    }
    assert!(face_pieces(&f, &[0, 0]).is_none() && face_piece(&f, &[(3, true)]).is_none());
    assert_eq!(face_pieces(&f, &[]).unwrap().len(), 1);
    let mut rng = Rng(0x000f_ace5);
    let mut agree = 0;
    for case in 0..60 {
        let mut recs = Vec::new();
        for _ in 0..1 + rng.below(3) as usize {
            let mut r = rec(3);
            for _ in 0..rng.below(3) {
                r.exceptions
                    .push(vec![EXC[rng.below(EXC.len() as u64) as usize]]);
            }
            if rng.below(3) == 0 {
                r.bounds.push(BND[rng.below(BND.len() as u64) as usize]);
            }
            r.elided = rng.below(6) == 0;
            recs.push(r);
        }
        let d = disp(&f, &recs);
        let s = 2 + rng.below(3) as i64;
        let cone = Cone::excess_simplex(&f, s);
        let whole = cone_coverage(&d, &f, &cone, &ConeOptions::default()).unwrap();
        let mut verdicts = Vec::new();
        for p in &pieces {
            let cd = capped_dispatcher(&d, &f, p).unwrap();
            let pc = cone.clone().and(p);
            let b = cone_coverage(&cd, &f, &pc, &ConeOptions::default()).unwrap();
            let cert = certificate(&b, &cd, &f);
            let chk = check_cone_certificate(&cert, &cd, &f, Some((s as i32 + 1, -(s as i32))));
            assert!(chk.ok(), "case {case} {}: {:?}", p.label, chk.problems);
            verdicts.push(b.verdict);
        }
        let joined = if verdicts.contains(&"undecided") {
            "undecided"
        } else if verdicts.contains(&"uncovered") {
            "uncovered"
        } else {
            "covered"
        };
        if whole.verdict != "undecided" && joined != "undecided" {
            assert_eq!(whole.verdict, joined, "case {case}: {verdicts:?}");
            agree += 1;
        }
    }
    assert!(agree > 40, "{agree}");
}

#[test]
fn reduced_fall_through_drops_only_loci_the_exceptions_already_exclude() {
    use rustred_campaign_core::domain::{PoleKind, PoleLocus};
    use rustred_campaign_core::loci::contain::ProofBudget;
    use rustred_campaign_core::parametric::cone::{fall_through_poles, fall_through_poles_reduced};
    let f = NodeFrame::without_header_lines_first(3, 2);
    // record 0: locus n0 = 2 (from (n0-2)*d + (n0-2)) and the exception [n0-2] the engine writes: redundant;
    // locus n1 = 3 (from (n1-3)*d) with no exception: kept (moved)
    let mut a = CaseDomain::free(3, Some(f.mask().to_vec()));
    a.exceptions.push(vec![poly("n0-2", 3)]);
    a.poles.push(PoleLocus::new(
        PoleKind::Witness,
        &Poly::parse_exact("(n0-2)*d+(n0-2)", 3, true).unwrap(),
    ));
    a.poles.push(PoleLocus::new(
        PoleKind::Witness,
        &Poly::parse_exact("(n1-3)*d", 3, true).unwrap(),
    ));
    // a locus outside the record's selected set (n2 <= 0 in the chart; locus n2 = 4): redundant
    a.poles.push(PoleLocus::new(
        PoleKind::Witness,
        &Poly::parse_exact("(n2-4)*d", 3, true).unwrap(),
    ));
    let mut t = Table::new("t", 3);
    t.push(0, a).unwrap();
    t.push(1, CaseDomain::free(3, Some(f.mask().to_vec())))
        .unwrap();
    let d = Dispatcher::new(vec![t], vec![0])
        .unwrap()
        .with_chart(f.mask().to_vec())
        .unwrap();
    let (red, st) = fall_through_poles_reduced(&d, &f, &ProofBudget::default()).unwrap();
    assert_eq!(
        (st.poles, st.redundant, st.moved_nonempty, st.moved_unknown),
        (3, 2, 1, 0),
        "{st:?}"
    );
    assert_eq!(st.redundant_by_branch, 1, "{st:?}");
    let full = fall_through_poles(&d).unwrap();
    // the same dispatch on a box, key by key
    for a0 in 1..6 {
        for a1 in 1..6 {
            for a2 in -4..1 {
                let k = [a0, a1, a2];
                let x = format!("{:?}", red.first_match(&k));
                let y = format!("{:?}", full.first_match(&k));
                assert_eq!(x, y, "{k:?}");
            }
        }
    }
    match red.first_match(&[4, 3, 0]) {
        Ok(Dispatch::Covered(r)) => assert_eq!(r.record, 1),
        o => panic!("{o:?}"),
    }
    match red.first_match(&[2, 1, 0]) {
        Ok(Dispatch::Covered(r)) => assert_eq!(r.record, 1),
        o => panic!("{o:?}"),
    }
}

#[test]
fn region_keys_each_enumerates_a_lattice_region_inside_a_cone_exactly() {
    use rustred_campaign_core::loci::region::Region;
    use rustred_campaign_core::parametric::cone::region_keys_each;
    use rustred_campaign_core::partition::exact::Affine;
    let f = NodeFrame::without_header_lines_first(3, 2);
    // x0 = 1 + t0, x1 = 1 + t0 + t1, x2 = -t1, t0 >= 0, t1 >= 0 (a correlated lattice region, not a cell)
    let aff = |c: i128, a: i128, b: i128| Affine {
        coef: vec![a, b],
        c,
    };
    let reg = Region {
        k: 2,
        x: vec![aff(1, 1, 0), aff(1, 1, 1), aff(0, 0, -1)],
        cons: vec![aff(0, 1, 0), aff(0, 0, 1)],
        names: vec!["t0".into(), "t1".into()],
    };
    for s in 0..7i64 {
        let cone = Cone::excess_simplex(&f, s);
        let mut got = Vec::new();
        region_keys_each(&reg, &cone, 1_000_000, &mut |k| got.push(k.to_vec())).unwrap();
        got.sort();
        let mut want = Vec::new();
        for k0 in 1..20 {
            for k2 in -20..1 {
                let k1 = k0 - k2;
                let k = vec![k0, k1, k2];
                if cone.contains(&k) {
                    want.push(k);
                }
            }
        }
        want.sort();
        assert_eq!(got, want, "S = {s}");
    }
    // unbounded without a cone
    assert!(region_keys_each(&reg, &Cone::orthant(), 1_000_000, &mut |_| {}).is_none());
}

/// TRICKS 51: a declared terminal key may never be served: `records_containing` finds every record whose domain applies at
/// the key (a refusal for a delivered table), and none for a key outside every domain.
#[test]
fn a_record_containing_a_terminal_is_found() {
    use rustred_campaign_core::cover::{Dispatcher, Table};
    use rustred_campaign_core::domain::CaseDomain;
    use rustred_campaign_core::parametric::cone::records_containing;
    let n = 3;
    let chart = vec![true, true, false];
    let mut t = Table::new("t", n);
    // record 0: the face n0 >= 2 (free n0, n1 = 1, n2 = 0)
    let mut d0 = CaseDomain::free(n, Some(chart.clone()));
    d0.fixed = vec![None, Some(1), Some(0)];
    d0.bounds.push(
        rustred_campaign_core::poly::Poly::var(0)
            .sub(&rustred_campaign_core::poly::Poly::constant(2))
            .unwrap(),
    );
    t.push(0, d0).unwrap();
    // record 1: the corner point (1, 1, 0)
    let mut d1 = CaseDomain::free(n, Some(chart.clone()));
    d1.fixed = vec![Some(1), Some(1), Some(0)];
    t.push(1, d1).unwrap();
    let disp = Dispatcher::new(vec![t], vec![0])
        .unwrap()
        .with_chart(chart)
        .unwrap();
    // the corner is contained by record 1 (a terminal there would be refused)
    assert_eq!(records_containing(&disp, &[1, 1, 0]), vec![(0, 1)]);
    // a face key is contained by record 0 only
    assert_eq!(records_containing(&disp, &[3, 1, 0]), vec![(0, 0)]);
    // a key outside every domain: nothing (a terminal there is fine)
    assert!(records_containing(&disp, &[1, 2, 0]).is_empty());
}

/// A LINEAR (several-slot) cap is served exactly on its violation, and inside every cap the capped dispatcher selects
/// what the original selects: an affine piece (an equation as two opposite caps) restricts the partition to a
/// lattice line / plane of the face (lane parametric 2026-11-17, `PARAMETRIC_PIECE_EQS`).
#[test]
fn a_linear_cap_is_served_exactly_on_its_violation() {
    use rustred_campaign_core::parametric::cone::capped_dispatcher;
    let f = NodeFrame::without_header_lines_first(3, 2);
    let mut rng = Rng(0x00af_f1e5);
    let mut inside_n = 0;
    for case in 0..40 {
        let mut recs = Vec::new();
        for _ in 0..1 + rng.below(3) as usize {
            let mut r = rec(3);
            for _ in 0..rng.below(3) {
                r.exceptions
                    .push(vec![EXC[rng.below(EXC.len() as u64) as usize]]);
            }
            if rng.below(3) == 0 {
                r.bounds.push(BND[rng.below(BND.len() as u64) as usize]);
            }
            recs.push(r);
        }
        let d = disp(&f, &recs);
        let c = rng.below(3) as i64 - 1;
        let caps = Cone {
            label: "affine".into(),
            constraints: vec![
                (vec![1, -2, 0], c),
                (vec![-1, 2, 0], -c),
                (vec![0, 1, -1], 6),
            ],
        };
        let cd = capped_dispatcher(&d, &f, &caps).unwrap();
        for k0 in 1..=9i32 {
            for k1 in 1..=5i32 {
                for k2 in -3..=0i32 {
                    {
                        {
                            let k = [k0, k1, k2];
                            let inside = caps.constraints.iter().all(|(a, b)| {
                                a.iter().zip(&k).map(|(x, y)| x * i64::from(*y)).sum::<i64>() <= *b
                            });
                            let got = cd.first_match(&k).unwrap();
                            if !inside {
                                assert!(matches!(&got, Dispatch::Covered(r) if r.table == 0), "case {case} {k:?}: {got:?}");
                                continue;
                            }
                            inside_n += 1;
                            let want = d.first_match(&k).unwrap();
                            match (&got, &want) {
                                (Dispatch::Covered(a), Dispatch::Covered(b)) => {
                                    assert!(a.table >= 1 && a.record == b.record, "case {case} {k:?}")
                                }
                                (Dispatch::Uncovered, Dispatch::Uncovered) => {}
                                (Dispatch::Blocked { at: a, .. }, Dispatch::Blocked { at: b, .. }) => {
                                    assert!(a.table >= 1 && a.record == b.record, "case {case} {k:?}")
                                }
                                _ => panic!("case {case} {k:?}: capped {got:?} vs {want:?}"),
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(inside_n > 100, "{inside_n}");
}

/// Region::reduce_equalities: two opposite constraints are an equation; the reduced region has one parameter fewer per
/// equation and exactly the same integer points (checked by enumeration).
#[test]
fn implicit_equalities_are_made_explicit_with_the_same_points() {
    use rustred_campaign_core::loci::region::Region;
    use rustred_campaign_core::partition::exact::Affine;
    let mut rng = Rng(0x0e9a_11e5);
    for case in 0..60 {
        let k = 3usize;
        let x: Vec<Affine> = (0..k)
            .map(|i| Affine { coef: (0..k).map(|j| i128::from(i == j)).collect(), c: 0 })
            .collect();
        let a: Vec<i128> = (0..k).map(|_| rng.below(5) as i128 - 2).collect();
        if a.iter().all(|&v| v == 0) {
            continue;
        }
        let b = rng.below(7) as i128 - 3;
        let mut cons = vec![
            Affine { coef: a.clone(), c: b },
            Affine { coef: a.iter().map(|v| -v).collect(), c: -b },
        ];
        // box -4..=4 per parameter
        for j in 0..k {
            let mut e = vec![0i128; k];
            e[j] = 1;
            cons.push(Affine { coef: e.clone(), c: 4 });
            e[j] = -1;
            cons.push(Affine { coef: e, c: 4 });
        }
        let r = Region { k, x, cons, names: (0..k).map(|i| format!("t{i}")).collect() };
        let pts = |r: &Region, w: i128| -> std::collections::BTreeSet<Vec<i128>> {
            let mut out = std::collections::BTreeSet::new();
            let rng_k: Vec<i128> = (-w..=w).collect();
            let mut t = vec![-w; r.k];
            loop {
                if r.cons.iter().all(|c| c.coef.iter().zip(&t).map(|(a, b)| a * b).sum::<i128>() + c.c >= 0) {
                    out.insert(r.x.iter().map(|xa| xa.coef.iter().zip(&t).map(|(a, b)| a * b).sum::<i128>() + xa.c).collect());
                }
                let mut s = 0;
                loop {
                    if s == r.k {
                        return out;
                    }
                    if t[s] < *rng_k.last().unwrap() {
                        t[s] += 1;
                        break;
                    }
                    t[s] = rng_k[0];
                    s += 1;
                }
                if r.k == 0 {
                    return out;
                }
            }
        };
        let before = pts(&r, 6);
        match r.reduce_equalities().unwrap() {
            None => assert!(before.is_empty(), "case {case}: reduced to empty but {} points", before.len()),
            Some(red) => {
                assert_eq!(red.k, k - 1, "case {case}");
                // the lattice re-parametrisation may stretch the parameters: a wide window
                assert_eq!(pts(&red, 150), before, "case {case}");
            }
        }
    }
}
