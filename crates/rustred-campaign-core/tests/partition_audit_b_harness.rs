//! Port of the audit-part-B differential harness (notes/implementer.audit-part-B.md; original harness under
//! implementer-scratch/audit-part-B/audit_b): its random-table generator (literal families: one-slot of degree 1-6
//! with integer / repeated / rational / no roots, 2-3-slot linear with unit / non-unit / mixed coefficients, products,
//! quadrics; a shared literal pool so literals recur as equations and exceptions; duplicates, restrictions, empty rules,
//! fixed values out of the chart), the naive oracle (`CaseDomain::applies` per record, union coverage), the
//! DESCRIPTION invariant (every uncovered window key lies on a listed face AND in a listed generic cell or locus piece /
//! unresolved region; every witness uncovered), memo == reference, and the proof checks (no wrong `Proved`, every
//! `NotContained` / `NonEmpty` witness verified) plus the auditor's 59 hand traps.
//!
//! Default suite: a seeded small run (2,000 tables, small arities). Full run (ignored, release):
//! `light_build.sh <lane> test -p rustred-campaign-core --release --test partition_audit_b_harness -- --ignored`
//! (200,000 tables; `AUDIT_B_TABLES` overrides the count).

use std::collections::BTreeSet;

use rustred_campaign_core::cover::RuleRef;
use rustred_campaign_core::domain::{CaseDomain, Stage, Verdict};
use rustred_campaign_core::key::{Key, NodeFrame};
use rustred_campaign_core::partition::exact::MPoly;
use rustred_campaign_core::partition::{
    ANY_OFF, ClassHit, Containment, DomainList, ElidedPolicy, Emptiness, PartitionOptions,
    PartitionReport, ProofBudget, contained_in_with, face_of, gap_partition, is_empty_with,
    shadowed_with,
};
use rustred_campaign_core::poly::Poly;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
    fn chance(&mut self, permille: u32) -> bool {
        self.below(1000) < permille as u64
    }
    fn pick<T: Clone>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u64) as usize].clone()
    }
}

#[derive(Clone)]
struct Cfg {
    n: usize,
    lines: usize,
    nrules: (usize, usize),
    r: i64,
    /// 1 one-slot, 2 linear, 4 products, 8 quadrics, 16 irrational / rational roots
    fam: u32,
    bounds: bool,
    pfix: u32,
    w: i32,
}

fn c(n: usize, v: i128) -> MPoly {
    MPoly::constant(n, v)
}
fn x(n: usize, s: usize) -> MPoly {
    MPoly::var(n, s)
}
fn add(a: &MPoly, b: &MPoly) -> MPoly {
    a.add(b).unwrap()
}
fn mul(a: &MPoly, b: &MPoly) -> MPoly {
    a.mul(b).unwrap()
}

fn chart_val(g: &mut Rng, line: bool, r: i64) -> i64 {
    let d = g.range(0, r);
    if line { 1 + d } else { -d }
}

fn one_slot(g: &mut Rng, n: usize, s: usize, line: bool, r: i64, fam: u32) -> MPoly {
    let v = x(n, s);
    let deg = g.range(1, 6) as usize;
    let mut p = c(n, 1);
    let mut d = 0;
    while d < deg {
        let k = g.below(10);
        if fam & 16 != 0 && k == 0 && d + 2 <= deg {
            let q = match g.below(3) {
                0 => add(&mul(&v, &v), &c(n, -2)),
                1 => add(&mul(&v, &v), &c(n, 1)),
                _ => add(&add(&mul(&v, &v), &v.scale(-3).unwrap()), &c(n, 1)),
            };
            p = mul(&p, &q);
            d += 2;
        } else if fam & 16 != 0 && k == 1 {
            let w = g.range(-r, r);
            p = mul(&p, &add(&v.scale(2).unwrap(), &c(n, -(2 * w as i128 + 1))));
            d += 1;
        } else {
            let root = if g.chance(700) {
                chart_val(g, line, r)
            } else {
                g.range(-r - 2, r + 2)
            };
            let f = add(&v, &c(n, -root as i128));
            p = mul(&p, &f);
            d += 1;
            if d < deg && g.chance(200) {
                p = mul(&p, &f);
                d += 1;
            }
        }
    }
    if g.chance(100) {
        p = p.scale(g.pick(&[2, -1, 3])).unwrap();
    }
    p
}

fn linear_multi(g: &mut Rng, n: usize, r: i64) -> MPoly {
    let k = if n >= 3 && g.chance(400) { 3 } else { 2 }.min(n);
    let mut slots: Vec<usize> = vec![];
    while slots.len() < k {
        let s = g.below(n as u64) as usize;
        if !slots.contains(&s) {
            slots.push(s);
        }
    }
    let style = g.below(6);
    let coefs: Vec<i128> = match style {
        0 => vec![1, -1, 0],
        1 => vec![-1, -1, 1],
        2 => vec![2, -1, -2],
        3 => vec![3, 5, -7],
        _ => (0..3)
            .map(|_| g.pick(&[-3i128, -2, -1, 1, 2, 3, 0, 1, -1]))
            .collect(),
    };
    let mut p = MPoly::zero(n);
    for (i, &s) in slots.iter().enumerate() {
        if coefs[i] != 0 {
            p = add(&p, &x(n, s).scale(coefs[i]).unwrap());
        }
    }
    if p.is_zero() {
        p = x(n, slots[0]);
    }
    let k0 = match style {
        1 => 1,
        2 => 4,
        3 => -7,
        _ => g.range(-r, r) as i128,
    };
    add(&p, &c(n, k0))
}

fn quadric(g: &mut Rng, n: usize) -> MPoly {
    let a = g.below(n as u64) as usize;
    let mut b = g.below(n as u64) as usize;
    if b == a {
        b = (a + 1) % n;
    }
    let (u, v) = (x(n, a), x(n, b));
    match g.below(4) {
        0 => add(
            &add(&mul(&u, &u), &mul(&v, &v).scale(-2).unwrap()),
            &c(n, -1),
        ),
        1 => add(&mul(&u, &v), &c(n, -6)),
        2 => add(&add(&mul(&u, &u), &mul(&v, &v)), &c(n, -25)),
        _ => add(&mul(&u, &v), &c(n, 6)),
    }
}

fn literal(g: &mut Rng, cfg: &Cfg, mask: &[bool]) -> MPoly {
    let n = cfg.n;
    loop {
        match g.below(5) {
            0 | 1 if cfg.fam & 1 != 0 => {
                let s = g.below(n as u64) as usize;
                return one_slot(g, n, s, mask[s], cfg.r, cfg.fam);
            }
            2 if cfg.fam & 2 != 0 => return linear_multi(g, n, cfg.r),
            3 if cfg.fam & 4 != 0 => {
                let a = linear_multi(g, n, cfg.r);
                let s = g.below(n as u64) as usize;
                let b = if g.chance(500) {
                    linear_multi(g, n, cfg.r)
                } else {
                    add(&x(n, s), &c(n, -chart_val(g, mask[s], cfg.r) as i128))
                };
                return mul(&a, &b);
            }
            4 if cfg.fam & 8 != 0 => return quadric(g, n),
            _ => {}
        }
    }
}

fn gen_table(g: &mut Rng, cfg: &Cfg) -> DomainList {
    let n = cfg.n;
    let mask: Vec<bool> = (0..n).map(|i| i < cfg.lines).collect();
    let nr = g.range(cfg.nrules.0 as i64, cfg.nrules.1 as i64) as usize;
    let pool: Vec<MPoly> = (0..(3 + nr / 2)).map(|_| literal(g, cfg, &mask)).collect();
    let tp = |p: &MPoly| -> Poly { p.to_poly(n).expect("small coefficients") };
    let mut ds: Vec<CaseDomain> = vec![];
    for _ in 0..nr {
        let kind = g.below(10);
        if kind == 0 && !ds.is_empty() {
            let d = g.pick(&ds);
            ds.push(d);
            continue;
        }
        if kind == 1 && !ds.is_empty() {
            let mut d = g.pick(&ds);
            if g.chance(500) {
                let s = g.below(n as u64) as usize;
                d.fixed[s] = Some(chart_val(g, mask[s], cfg.r) as i32);
            } else {
                d.equations.push(tp(&g.pick(&pool)));
            }
            ds.push(d);
            continue;
        }
        let mut d = CaseDomain::free(n, Some(mask.clone()));
        for (s, &line) in mask.iter().enumerate() {
            if g.chance(cfg.pfix) {
                d.fixed[s] = Some(if g.chance(980) {
                    if g.chance(500) {
                        i32::from(line)
                    } else {
                        chart_val(g, line, cfg.r) as i32
                    }
                } else if line {
                    0
                } else {
                    1
                });
            }
        }
        let lit = |g: &mut Rng| -> MPoly {
            if g.chance(600) {
                g.pick(&pool)
            } else {
                literal(g, cfg, &mask)
            }
        };
        if kind == 2 {
            let p = lit(g);
            d.equations.push(tp(&p));
            d.exceptions.push(vec![tp(&p)]);
            ds.push(d);
            continue;
        }
        for _ in 0..g.pick(&[0usize, 0, 0, 1, 1, 2]) {
            let p = lit(g);
            d.equations.push(tp(&p));
        }
        if cfg.bounds {
            for _ in 0..g.pick(&[0usize, 0, 1]) {
                d.bounds.push(tp(&linear_multi(g, n, cfg.r)));
            }
        }
        for _ in 0..g.pick(&[0usize, 0, 1, 1, 2, 3]) {
            let k = g.pick(&[1usize, 1, 1, 2, 3]);
            let br: Vec<Poly> = (0..k).map(|_| tp(&lit(g))).collect();
            d.exceptions.push(br);
        }
        ds.push(d);
    }
    let mut l = DomainList::new(n);
    for (i, d) in ds.into_iter().enumerate() {
        l.push(RuleRef::bare(0, i, i as i64), d);
    }
    l
}

fn window(n: usize, e: usize, w: i32) -> Vec<Key> {
    let mut out = vec![vec![]];
    for s in 0..n {
        let vals: Vec<i32> = if s < e {
            (1..=w).collect()
        } else {
            (-(w - 1)..=0).collect()
        };
        out = out
            .into_iter()
            .flat_map(|k: Key| {
                vals.iter().map(move |&v| {
                    let mut k2 = k.clone();
                    k2.push(v);
                    k2
                })
            })
            .collect();
    }
    out
}

/// Exact integer parameters of a key on a region (fraction-free Gauss-Jordan; the parametrisations are injective).
fn solve_params(r: &rustred_campaign_core::loci::Region, key: &[i32]) -> Option<Vec<i128>> {
    fn g(a: i128, b: i128) -> i128 {
        let (mut a, mut b) = (a.abs(), b.abs());
        while b != 0 {
            let t = a % b;
            a = b;
            b = t;
        }
        a
    }
    let (k, n) = (r.k, r.x.len());
    let mut rows: Vec<Vec<i128>> = (0..n)
        .map(|i| {
            let mut v = r.x[i].coef.clone();
            v.push(key[i] as i128 - r.x[i].c);
            v
        })
        .collect();
    let mut piv = vec![];
    let mut row = 0;
    for col in 0..k {
        let Some(p) = (row..n).find(|&i| rows[i][col] != 0) else {
            continue;
        };
        rows.swap(row, p);
        for i in 0..n {
            if i != row && rows[i][col] != 0 {
                let (a, b) = (rows[row][col], rows[i][col]);
                let pivot_row = rows[row].clone();
                for (dst, src) in rows[i].iter_mut().zip(&pivot_row) {
                    *dst = *dst * a - src * b;
                }
                let gg = rows[i].iter().fold(0, |acc, &v| g(acc, v));
                if gg > 1 {
                    for v in rows[i].iter_mut() {
                        *v /= gg;
                    }
                }
            }
        }
        piv.push(col);
        row += 1;
    }
    if rows[row..].iter().any(|r| r[k] != 0) || piv.len() < k {
        return None;
    }
    let mut t = vec![0i128; k];
    for (i, &cc) in piv.iter().enumerate() {
        if rows[i][k] % rows[i][cc] != 0 {
            return None;
        }
        t[cc] = rows[i][k] / rows[i][cc];
    }
    (r.at(&t).ok()? == key.iter().map(|&v| v as i128).collect::<Vec<_>>()).then_some(t)
}

fn described(rep: &PartitionReport, k: &[i32]) -> Result<(), String> {
    let fk = face_of(&rep.nf().lines, k);
    let Some(face) = rep.faces().iter().find(|f| f.key() == fk) else {
        return Err(format!("face {fk:?} not listed"));
    };
    let slots: Vec<usize> = face.sd.iter().chain(&face.sn).copied().collect();
    let cell: Vec<u16> = slots
        .iter()
        .map(|&s| rep.state_index(s, k[s] as i64).unwrap())
        .collect();
    if face.generic_cells.iter().any(|cc| {
        cc.iter()
            .zip(&cell)
            .all(|(&cs, &st)| cs == st || (cs == ANY_OFF && st != 0))
    }) {
        return Ok(());
    }
    if let Some(res) = &rep.resolution() {
        for l in res.loci.iter().filter(|l| l.face == fk) {
            let regions = l
                .pieces
                .iter()
                .map(|p| &p.region)
                .chain(l.unresolved_parts.iter().map(|u| &u.region));
            for r in regions {
                if let Some(t) = solve_params(r, k)
                    && r.contains_t(&t).unwrap()
                {
                    return Ok(());
                }
            }
        }
    }
    Err(format!(
        "{:?} key on no generic cell / locus piece of face {fk:?}",
        rep.class_of(k).unwrap()
    ))
}

#[derive(Default, Debug)]
struct Stats {
    tables: u64,
    keys: u64,
    uncovered: u64,
    memo_compared: u64,
    proofs: u64,
    proved: u64,
    unknown: u64,
}

fn check_table(
    l: &DomainList,
    cfg: &Cfg,
    seed: u64,
    idx: u64,
    st: &mut Stats,
    memo: bool,
    proofs: bool,
) {
    let n = cfg.n;
    let e = cfg.lines;
    let f = NodeFrame::without_header_lines_first(n, e);
    let opts = PartitionOptions {
        elided: ElidedPolicy::Ordinary,
        ..Default::default()
    };
    let rep = gap_partition(l, &f, &opts, None).unwrap();
    let keys = window(n, e, cfg.w);
    let what = format!("seed {seed} table {idx}");
    for k in &keys {
        let cov = l
            .items
            .iter()
            .any(|(_, d)| d.applies(k) == Verdict::Applies);
        assert_eq!(rep.uncovered(k).unwrap(), !cov, "{what}: oracle at {k:?}");
        if !cov {
            st.uncovered += 1;
            assert_ne!(rep.class_of(k).unwrap(), ClassHit::Covered, "{what}: {k:?}");
            if let Err(why) = described(&rep, k) {
                panic!("{what}: uncovered {k:?} not described: {why}");
            }
        }
    }
    st.keys += keys.len() as u64;
    if let Some(res) = &rep.resolution() {
        for loc in &res.loci {
            for w in &loc.witness {
                assert!(rep.uncovered(w).unwrap(), "{what}: witness {w:?} covered");
            }
        }
    }
    for fc in rep.faces() {
        if let Some(w) = &fc.witness {
            assert!(
                rep.uncovered(w).unwrap(),
                "{what}: face witness {w:?} covered"
            );
        }
    }
    if memo {
        let b = gap_partition(
            l,
            &f,
            &PartitionOptions {
                memo: false,
                ..opts.clone()
            },
            None,
        )
        .unwrap();
        let ca: BTreeSet<_> = rep.classes().iter().cloned().collect();
        let cb: BTreeSet<_> = b.classes().iter().cloned().collect();
        assert_eq!(ca, cb, "{what}: memo != reference classes");
        let fa: BTreeSet<_> = rep.faces().iter().map(|x| (x.key(), x.kind)).collect();
        let fb: BTreeSet<_> = b.faces().iter().map(|x| (x.key(), x.kind)).collect();
        assert_eq!(fa, fb, "{what}: memo != reference faces");
        st.memo_compared += 1;
    }
    if proofs {
        let budget = ProofBudget {
            max_nodes: 20_000,
            max_work: 2_000_000,
            wall: Some(std::time::Duration::from_secs(5)),
            ..Default::default()
        };
        let m = l.items.len();
        for i in 0..m.min(4) {
            let d = &l.items[i].1;
            let applies_at: Vec<&Key> = keys
                .iter()
                .filter(|k| d.applies(k) == Verdict::Applies)
                .collect();
            st.proofs += 1;
            match is_empty_with(d, &f, &budget) {
                Emptiness::ProvedEmpty { certificate } => {
                    st.proved += 1;
                    assert!(
                        applies_at.is_empty(),
                        "{what}: rule {i} proved empty ({certificate}) but applies"
                    );
                }
                Emptiness::NonEmpty { witness } => {
                    assert_eq!(d.applies(&witness), Verdict::Applies, "{what}")
                }
                Emptiness::Unknown { .. } => st.unknown += 1,
            }
            let others: Vec<&CaseDomain> =
                (0..m).filter(|&j| j != i).map(|j| &l.items[j].1).collect();
            st.proofs += 1;
            match contained_in_with(d, &others, &f, &budget) {
                Containment::Proved { certificate } => {
                    st.proved += 1;
                    for k in &applies_at {
                        assert!(
                            others.iter().any(|o| o.applies(k) == Verdict::Applies),
                            "{what}: rule {i} proved contained ({certificate}) but {k:?} only in it"
                        );
                    }
                }
                Containment::NotContained { witness } => {
                    assert_eq!(d.applies(&witness), Verdict::Applies, "{what}");
                    assert!(
                        !others
                            .iter()
                            .any(|o| o.applies(&witness) == Verdict::Applies),
                        "{what}"
                    );
                }
                Containment::Unknown { .. } => st.unknown += 1,
            }
            st.proofs += 1;
            match shadowed_with(l, i, &f, &budget) {
                Containment::Proved { certificate } => {
                    st.proved += 1;
                    for k in &applies_at {
                        assert!(
                            (0..i).any(|j| l.items[j].1.applies(k).stage() != Stage::NotSelected),
                            "{what}: rule {i} proved shadowed ({certificate}) but fires at {k:?}"
                        );
                    }
                }
                Containment::NotContained { witness } => {
                    assert_eq!(d.applies(&witness), Verdict::Applies, "{what}");
                    assert!(
                        !(0..i)
                            .any(|j| l.items[j].1.applies(&witness).stage() != Stage::NotSelected),
                        "{what}"
                    );
                }
                Containment::Unknown { .. } => st.unknown += 1,
            }
        }
    }
    st.tables += 1;
}

/// The auditor's configurations (lines first; its random-mask configuration is not ported).
fn configs() -> Vec<(&'static str, Cfg)> {
    let base = |n, lines, r, fam, w| Cfg {
        n,
        lines,
        nrules: (1, 12),
        r,
        fam,
        bounds: false,
        pfix: 300,
        w,
    };
    vec![
        ("a3_all", base(3, 2, 6, 31, 11)),
        ("a3_lin", base(3, 2, 6, 3, 11)),
        ("a4_all", base(4, 2, 4, 31, 8)),
        (
            "a5_bounds",
            Cfg {
                bounds: true,
                ..base(5, 3, 3, 31, 5)
            },
        ),
        ("a6_all", base(6, 3, 2, 31, 4)),
    ]
}

fn run(n_tables: u64, seed0: u64, memo_every: u64, proofs_every: u64) -> Stats {
    let cfgs = configs();
    let mut st = Stats::default();
    for t in 0..n_tables {
        let (_, cfg) = &cfgs[(t % cfgs.len() as u64) as usize];
        let seed = seed0.wrapping_add(t.wrapping_mul(0x9E37_79B9));
        let mut g = Rng(seed);
        let l = gen_table(&mut g, cfg);
        check_table(
            &l,
            cfg,
            seed,
            t,
            &mut st,
            t % memo_every == 0,
            t % proofs_every == 0,
        );
    }
    st
}

#[test]
fn audit_b_harness_seeded_small() {
    let st = run(2_000, 1001, 5, 10);
    println!("audit-B harness (seeded, 2,000 tables): {st:?}");
    assert_eq!(st.tables, 2_000);
}

#[test]
#[ignore = "full audit-part-B campaign (200,000 tables; hours); release; run with --ignored"]
fn audit_b_harness_full() {
    let n: u64 = std::env::var("AUDIT_B_TABLES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200_000);
    let st = run(n, 424_242, 5, 5);
    println!("audit-B harness (full, {n} tables): {st:?}");
}

/// The auditor's 59 hand traps (mask 110): no wrong `Proved`, no wrong witness, and the LINEAR instances decided
/// (audit-part-B M1: far witnesses are found exactly, bounded instances by exhaustive search).
#[test]
fn audit_b_traps() {
    let n = 3;
    let f = NodeFrame::without_header_lines_first(n, 2);
    let r =
        |eqs: &[&str], exc: &[&[&str]], fixed: &[(usize, i32)], bounds: &[&str]| -> CaseDomain {
            let mut d = CaseDomain::free(n, Some(vec![true, true, false]));
            d.equations = eqs
                .iter()
                .map(|t| Poly::parse(t, n, false).unwrap())
                .collect();
            d.exceptions = exc
                .iter()
                .map(|b| {
                    b.iter()
                        .map(|t| Poly::parse(t, n, false).unwrap())
                        .collect()
                })
                .collect();
            d.bounds = bounds
                .iter()
                .map(|t| Poly::parse(t, n, false).unwrap())
                .collect();
            for &(s, v) in fixed {
                d.fixed[s] = Some(v);
            }
            d
        };
    let lin = |d: &CaseDomain| {
        d.equations
            .iter()
            .chain(&d.bounds)
            .chain(d.exceptions.iter().flatten())
            .all(|p| MPoly::from_poly(p, n, false).unwrap().total_degree() <= 1)
    };
    let empty_cases: Vec<(&str, CaseDomain, Option<bool>)> = vec![
        ("2x=1", r(&["2*n0-1"], &[], &[], &[]), Some(true)),
        ("2x-2y=1", r(&["2*n0-2*n1-1"], &[], &[], &[]), Some(true)),
        (
            "6x+10y+15z=1",
            r(&["6*n0+10*n1+15*n2-1"], &[], &[], &[]),
            Some(false),
        ),
        (
            "6x+10y+15z=1 z=0",
            r(&["6*n0+10*n1+15*n2-1"], &[], &[(2, 0)], &[]),
            Some(true),
        ),
        (
            "6x+10y+15z=1 box",
            r(&["6*n0+10*n1+15*n2-1"], &[], &[], &["3-n0", "3-n1", "n2+1"]),
            Some(false),
        ),
        (
            "6x+10y+15z=2 box",
            r(&["6*n0+10*n1+15*n2-2"], &[], &[], &["3-n0", "3-n1", "n2+1"]),
            None,
        ),
        ("x+y+1=0", r(&["n0+n1+1"], &[], &[], &[]), Some(true)),
        ("x+z=0", r(&["n0+n2"], &[], &[], &[]), Some(false)),
        (
            "x-y=100000",
            r(&["n0-n1-100000"], &[], &[], &[]),
            Some(false),
        ),
        (
            "x-y=100000 y=1 exc",
            r(&["n0-n1-100000"], &[&["n0-100001"]], &[(1, 1)], &[]),
            Some(true),
        ),
        (
            "x-y=100000 exc",
            r(&["n0-n1-100000"], &[&["n0-100001"]], &[], &[]),
            Some(false),
        ),
        (
            "eq e exc e",
            r(&["n0-n1+2*n2"], &[&["n0-n1+2*n2"]], &[], &[]),
            Some(true),
        ),
        ("3x-3y=2", r(&["3*n0-3*n1-2"], &[], &[], &[]), Some(true)),
        ("2x+4z=1", r(&["2*n0+4*n2-1"], &[], &[], &[]), Some(true)),
        ("xy=-6", r(&["n0*n1+6"], &[], &[], &[]), Some(true)),
        ("Pell", r(&["n0^2-2*n1^2-1"], &[], &[], &[]), Some(false)),
        ("x^2=2", r(&["n0^2-2"], &[], &[], &[]), Some(true)),
        ("exc 2x-1", r(&[], &[&["2*n0-1"]], &[], &[]), Some(false)),
        (
            "x=5 exc x-5",
            r(&[], &[&["n0-5"]], &[(0, 5)], &[]),
            Some(true),
        ),
        (
            "x=y=100000 exc x-y",
            r(&["n0-100000", "n1-100000"], &[&["n0-n1"]], &[], &[]),
            Some(true),
        ),
        (
            "box exc",
            r(&[], &[&["n0-1"], &["n0-2"]], &[], &["2-n0", "2-n1", "n2"]),
            Some(true),
        ),
        ("x>=100000", r(&[], &[], &[], &["n0-100000"]), Some(false)),
        (
            "x+y=200001 x-y=1",
            r(&["n0+n1-200001", "n0-n1-1"], &[], &[], &[]),
            Some(false),
        ),
        (
            "x+y=200000 x-y=1",
            r(&["n0+n1-200000", "n0-n1-1"], &[], &[], &[]),
            Some(true),
        ),
        (
            "x-y=1000 exc z",
            r(&["n0-n1-1000"], &[&["n2"]], &[], &[]),
            Some(false),
        ),
        (
            "x>=100000 exc x-y",
            r(&[], &[&["n0-n1"]], &[], &["n0-100000"]),
            Some(false),
        ),
        (
            "x^2-3x+2=0 (roots)",
            r(&["n0^2-3*n0+2"], &[], &[], &[]),
            Some(false),
        ),
        (
            "x^2-3x+2=0, x=1,2 excepted",
            r(&["n0^2-3*n0+2"], &[&["n0-1"], &["n0-2"]], &[], &[]),
            Some(true),
        ),
    ];
    let mut wrong = vec![];
    for (label, d, truth) in &empty_cases {
        let v = is_empty_with(d, &f, &ProofBudget::default());
        match (&v, truth) {
            (Emptiness::ProvedEmpty { .. }, Some(false)) => {
                wrong.push(format!("{label}: wrong Proved"))
            }
            (Emptiness::NonEmpty { witness }, _) if d.applies(witness) != Verdict::Applies => {
                wrong.push(format!("{label}: bad witness"))
            }
            (Emptiness::NonEmpty { .. }, Some(true)) => {
                wrong.push(format!("{label}: witness in an empty domain"))
            }
            (Emptiness::Unknown { reason }, Some(_)) if lin(d) => {
                wrong.push(format!("{label}: Unknown on a linear instance ({reason})"))
            }
            _ => {}
        }
        println!("EMPTY {label:40} {truth:?} -> {v:?}");
    }
    let free = r(&[], &[], &[], &[]);
    let cont_cases: Vec<(&str, CaseDomain, Vec<CaseDomain>, Option<bool>)> = vec![
        (
            "free < exc 2x-1",
            free.clone(),
            vec![r(&[], &[&["2*n0-1"]], &[], &[])],
            Some(true),
        ),
        (
            "free < exc x-y-100000",
            free.clone(),
            vec![r(&[], &[&["n0-n1-100000"]], &[], &[])],
            Some(false),
        ),
        (
            "free < exc 6x+10y+15z-1",
            free.clone(),
            vec![r(&[], &[&["6*n0+10*n1+15*n2-1"]], &[], &[])],
            Some(false),
        ),
        (
            "free < exc 6x+10y-1",
            free.clone(),
            vec![r(&[], &[&["6*n0+10*n1-1"]], &[], &[])],
            Some(true),
        ),
        (
            "free < exc 2x-2y-1",
            free.clone(),
            vec![r(&[], &[&["2*n0-2*n1-1"]], &[], &[])],
            Some(true),
        ),
        (
            "eq x-y < exc x-y + eq x-y",
            r(&["n0-n1"], &[], &[], &[]),
            vec![
                r(&[], &[&["n0-n1"]], &[], &[]),
                r(&["n0-n1"], &[], &[], &[]),
            ],
            Some(true),
        ),
        (
            "free < three pieces",
            free.clone(),
            vec![
                r(&[], &[&["n0-2"]], &[], &[]),
                r(&[], &[&["n1-3"]], &[(0, 2)], &[]),
                r(&[], &[], &[(0, 2), (1, 3)], &[]),
            ],
            Some(true),
        ),
        (
            "free < two pieces",
            free.clone(),
            vec![
                r(&[], &[&["n0-2"]], &[], &[]),
                r(&[], &[&["n1-3"]], &[(0, 2)], &[]),
            ],
            Some(false),
        ),
        (
            "free < exc xy-6",
            free.clone(),
            vec![r(&[], &[&["n0*n1-6"]], &[], &[])],
            Some(false),
        ),
        (
            "free < exc Pell",
            free.clone(),
            vec![r(&[], &[&["n0^2-2*n1^2-1"]], &[], &[])],
            Some(false),
        ),
        (
            "free < exc xy+6",
            free.clone(),
            vec![r(&[], &[&["n0*n1+6"]], &[], &[])],
            Some(true),
        ),
        (
            "free < exc x^2-2",
            free.clone(),
            vec![r(&[], &[&["n0^2-2"]], &[], &[])],
            Some(true),
        ),
        (
            "free < exc [x,y=100000]",
            free.clone(),
            vec![r(&[], &[&["n0-100000", "n1-100000"]], &[], &[])],
            Some(false),
        ),
        (
            "free < exc [x,y=100000] + x=100000",
            free.clone(),
            vec![
                r(&[], &[&["n0-100000", "n1-100000"]], &[], &[]),
                r(&[], &[], &[(0, 100000)], &[]),
            ],
            Some(true),
        ),
        (
            "free < exc x-y-z-100000 + exc x+z-5",
            free.clone(),
            vec![
                r(&[], &[&["n0-n1-n2-100000"]], &[], &[]),
                r(&[], &[&["n0+n2-5"]], &[], &[]),
            ],
            Some(false),
        ),
        (
            "2x-y-2z+4 exc y-2",
            free.clone(),
            vec![
                r(&[], &[&["2*n0-n1-2*n2+4"]], &[], &[]),
                r(&["2*n0-n1-2*n2+4"], &[&["n1-2"]], &[], &[]),
            ],
            Some(true),
        ),
        (
            "2x-y-2z+4 exc y-6",
            free.clone(),
            vec![
                r(&[], &[&["2*n0-n1-2*n2+4"]], &[], &[]),
                r(&["2*n0-n1-2*n2+4"], &[&["n1-6"]], &[], &[]),
            ],
            Some(false),
        ),
        (
            "exc x-y-100000 + x=1",
            free.clone(),
            vec![
                r(&[], &[&["n0-n1-100000"]], &[], &[]),
                r(&[], &[], &[(0, 1)], &[]),
            ],
            Some(false),
        ),
        (
            "free < exc x-y-10",
            free.clone(),
            vec![r(&[], &[&["n0-n1-10"]], &[], &[])],
            Some(false),
        ),
        (
            "free < exc x-y-40",
            free.clone(),
            vec![r(&[], &[&["n0-n1-40"]], &[], &[])],
            Some(false),
        ),
        (
            "free < exc x-y-1000",
            free.clone(),
            vec![r(&[], &[&["n0-n1-1000"]], &[], &[])],
            Some(false),
        ),
        (
            "2x-y-2z+4 exc y-3",
            free.clone(),
            vec![
                r(&[], &[&["2*n0-n1-2*n2+4"]], &[], &[]),
                r(&["2*n0-n1-2*n2+4"], &[&["n1-3"]], &[], &[]),
            ],
            Some(true),
        ),
        (
            "free < exc z + exc z+1",
            free.clone(),
            vec![r(&[], &[&["n2"]], &[], &[]), r(&[], &[&["n2+1"]], &[], &[])],
            Some(true),
        ),
        (
            "point < exc x-100001",
            r(&["n0+n1-200001", "n0-n1-1"], &[], &[], &[]),
            vec![r(&[], &[&["n0-100001"]], &[], &[])],
            Some(false),
        ),
        (
            "point < exc x-100000",
            r(&["n0+n1-200001", "n0-n1-1"], &[], &[], &[]),
            vec![r(&[], &[&["n0-100000"]], &[], &[])],
            Some(true),
        ),
        (
            "x=y < exc x^2-y^2",
            r(&["n0-n1"], &[], &[], &[]),
            vec![r(&[], &[&["n0^2-n1^2"]], &[], &[])],
            Some(false),
        ),
        (
            "x-y=1 < exc x^2-y^2",
            r(&["n0-n1-1"], &[], &[], &[]),
            vec![r(&[], &[&["n0^2-n1^2"]], &[], &[])],
            Some(true),
        ),
        (
            "free < eq y^2-3y+2",
            free.clone(),
            vec![r(&["n1^2-3*n1+2"], &[], &[], &[])],
            Some(false),
        ),
    ];
    for (label, d, others, truth) in &cont_cases {
        let os: Vec<&CaseDomain> = others.iter().collect();
        let v = contained_in_with(d, &os, &f, &ProofBudget::default());
        let linear = lin(d) && others.iter().all(lin);
        match (&v, truth) {
            (Containment::Proved { .. }, Some(false)) => {
                wrong.push(format!("{label}: wrong Proved"))
            }
            (Containment::NotContained { witness }, _)
                if d.applies(witness) != Verdict::Applies
                    || os.iter().any(|o| o.applies(witness) == Verdict::Applies) =>
            {
                wrong.push(format!("{label}: bad witness"))
            }
            (Containment::NotContained { .. }, Some(true)) => {
                wrong.push(format!("{label}: witness but contained"))
            }
            (Containment::Unknown { reason }, Some(_)) if linear => {
                wrong.push(format!("{label}: Unknown on a linear instance ({reason})"))
            }
            _ => {}
        }
        println!("CONT  {label:40} {truth:?} -> {v:?}");
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}
