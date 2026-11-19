//! PART 3 (rows3): every routine of `rustred_campaign_core::independent_algebra` against its Symbolica-backed
//! counterpart (`SymbolicaAlgebra`) through the SAME trait `CheckAlgebra`, on random inputs and on the real data:
//! field arithmetic (edge values, verify-range and other primes), primality, rationals, the exact matrix routines,
//! Lee's test / the U, F determinants on real families, coefficient texts (grammar fuzz + the real rule texts),
//! the expression language and case literals, and sparse elimination with certificates.
//! Every test SKIPs without the Symbolica licence (source WS/ops/env.sh).

mod common;
mod inputs;

use common::*;
use rustred_campaign_algebra::SymbolicaAlgebra;
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::replay_base::algebra::{CheckAlgebra, Ctx, Eliminator, Q};
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::primes::{SplitMix64, verify_primes};

fn sym(test: &str) -> Option<SymbolicaAlgebra> {
    licensed_or_skip(test).then(|| SymbolicaAlgebra::new().expect("licensed"))
}

fn test_primes() -> Vec<u64> {
    let mut ps: Vec<u64> = (0..6u64).flat_map(|s| verify_primes(s, &[])).collect();
    ps.extend([
        3,
        5,
        7,
        65_537,
        2_147_483_647,
        2_305_843_009_213_693_951,
        9_223_372_036_854_775_783, // 2^63 - 25
    ]);
    ps
}

#[test]
fn diff_field_arithmetic() {
    let Some(s) = sym("diff_field_arithmetic") else {
        return;
    };
    let i = Independent;
    let mut rng = SplitMix64::new(1);
    let mut n = 0usize;
    for p in test_primes() {
        let mut vals: Vec<u64> = vec![0, 1, 2, p - 1, p - 2, (p - 1) / 2, p.div_ceil(2)];
        vals.extend((0..60).map(|_| rng.range(0, p)));
        for &a in &vals {
            assert_eq!(i.neg(p, a), s.neg(p, a), "neg {a} mod {p}");
            assert_eq!(i.inv(p, a), s.inv(p, a), "inv {a} mod {p}");
            for &b in &vals {
                assert_eq!(i.add(p, a, b), s.add(p, a, b), "add {a} {b} mod {p}");
                assert_eq!(i.sub(p, a, b), s.sub(p, a, b), "sub {a} {b} mod {p}");
                assert_eq!(i.mul(p, a, b), s.mul(p, a, b), "mul {a} {b} mod {p}");
                n += 3;
            }
        }
        for x in [
            i128::MIN,
            i128::MAX,
            -1,
            0,
            1,
            -(p as i128),
            p as i128 * 7 + 3,
        ] {
            assert_eq!(
                i.from_i128(p, x),
                s.from_i128(p, x),
                "from_i128 {x} mod {p}"
            );
        }
        for _ in 0..200 {
            let x = (rng.next_u64() as i128) << 40 ^ rng.next_u64() as i128;
            let x = if rng.next_u64() & 1 == 1 { -x } else { x };
            assert_eq!(i.from_i128(p, x), s.from_i128(p, x));
        }
    }
    println!(
        "field: {n} binary operations equal on {} primes",
        test_primes().len()
    );
}

#[test]
fn diff_primality() {
    let Some(s) = sym("diff_primality") else {
        return;
    };
    let i = Independent;
    let mut rng = SplitMix64::new(2);
    let mut cases: Vec<u64> = vec![
        0,
        1,
        2,
        3,
        4,
        561,
        1105,
        1729,
        2_047,
        3_215_031_751,
        2_152_302_898_747,
        3_474_749_660_383,
        341_550_071_728_321,
        3_825_123_056_546_413_051,
        u64::MAX,
        18_446_744_073_709_551_557,
    ];
    cases.extend((0..20_000).map(|_| rng.next_u64() | 1));
    let lo = 7u64 << 60;
    cases.extend((0..5_000).map(|k| lo + 2 * k + 1));
    let mut primes = 0;
    for &n in &cases {
        let a = i.is_prime(n);
        assert_eq!(a, s.is_prime(n), "is_prime({n})");
        primes += usize::from(a);
    }
    println!("primality: {} numbers equal ({primes} primes)", cases.len());
}

fn rq(rng: &mut SplitMix64, bits: u32) -> Q {
    let m = 1u64 << bits;
    let n = rng.range(0, m) as i128 - (m / 2) as i128;
    let d = rng.range(1, m) as i128;
    Independent.q_new(n, d).unwrap()
}

#[test]
fn diff_rationals() {
    let Some(s) = sym("diff_rationals") else {
        return;
    };
    let i = Independent;
    let mut rng = SplitMix64::new(3);
    let ps = test_primes();
    for _ in 0..20_000 {
        let (a, b) = (rq(&mut rng, 40), rq(&mut rng, 40));
        assert_eq!(i.q_add(a, b), s.q_add(a, b), "{a:?}+{b:?}");
        assert_eq!(i.q_sub(a, b), s.q_sub(a, b));
        assert_eq!(i.q_mul(a, b), s.q_mul(a, b));
        assert_eq!(i.q_div(a, b).ok(), s.q_div(a, b).ok());
        assert_eq!(i.q_neg(a), s.q_neg(a));
        assert_eq!(i.q_new(a.n, -a.d), s.q_new(a.n, -a.d));
        let p = ps[rng.range(0, ps.len() as u64) as usize];
        assert_eq!(i.q_modp(a, p), s.q_modp(a, p));
        let t = format!("{}/{}", a.n, a.d);
        assert_eq!(i.q_parse(&t), s.q_parse(&t));
    }
    for t in [" 17 ", "-3/6", "4/-8", "0/5", "x", "1/0"] {
        assert_eq!(i.q_parse(t).ok(), s.q_parse(t).ok(), "{t}");
    }
    // overflow of the i128 normal form is an error on both sides
    let big = Q::int(i128::MAX);
    assert!(i.q_add(big, Q::ONE).is_err() && s.q_add(big, Q::ONE).is_err());
    println!("rationals: 20000 x 8 operations equal");
}

#[test]
fn diff_linear_algebra() {
    let Some(s) = sym("diff_linear_algebra") else {
        return;
    };
    let i = Independent;
    let mut rng = SplitMix64::new(4);
    let (mut sing, mut tot) = (0, 0);
    for _ in 0..600 {
        let n = rng.range(1, 7) as usize;
        let m: Vec<Vec<i128>> = (0..n)
            .map(|_| (0..n).map(|_| rng.range(0, 7) as i128 - 3).collect())
            .collect();
        assert_eq!(i.det_i128(&m), s.det_i128(&m), "det {m:?}");
        let q: Vec<Vec<Q>> = m
            .iter()
            .map(|r| {
                r.iter()
                    .map(|&x| Independent.q_new(x, rng.range(1, 4) as i128).unwrap())
                    .collect()
            })
            .collect();
        let (a, b) = (i.q_inverse(&q), s.q_inverse(&q));
        assert_eq!(a.is_ok(), b.is_ok(), "inverse solvability {q:?}");
        if let (Ok(a), Ok(b)) = (&a, &b) {
            assert_eq!(a, b);
        } else {
            sing += 1;
        }
        tot += 1;
        // rectangular: rank, rref, unit in row space
        let rows = rng.range(1, 7) as usize;
        let cols = rng.range(1, 8) as usize;
        let r: Vec<Vec<Q>> = (0..rows)
            .map(|_| {
                (0..cols)
                    .map(|_| Q::int(rng.range(0, 5) as i128 - 2))
                    .collect()
            })
            .collect();
        assert_eq!(i.q_rank(&r), s.q_rank(&r), "rank {r:?}");
        let nc = rng.range(1, cols as u64 + 1) as usize;
        assert_eq!(i.q_rref(&r, nc), s.q_rref(&r, nc), "rref {r:?} on {nc}");
        assert_eq!(
            i.q_unit_in_row_space(&r, cols - 1),
            s.q_unit_in_row_space(&r, cols - 1),
            "unit in row space {r:?}"
        );
        let (x, y) = (rng.next_u64() as i64 as i128, rng.range(0, 1 << 20) as i128);
        assert_eq!(i.gcd_i128(x, y), s.gcd_i128(x, y));
    }
    println!("linear algebra: {tot} systems equal ({sing} singular)");
}

/// Lee's test (q_unit_in_row_space) and the U / F determinants (zpoly_det) on real families: every sector mask of
/// the small families, a deterministic sample of the arity-20 one.
#[test]
fn diff_family_models_and_zero_sectors() {
    let Some(s) = sym("diff_family_models_and_zero_sectors") else {
        return;
    };
    let fx = campaign_fixtures();
    let texts: Vec<(String, String)> = vec![
        (
            "TOY2kite".into(),
            std::fs::read_to_string(format!("{fx}/small/m2-slice/families/TOY2kite.sym")).unwrap(),
        ),
        ("LA3".into(), core_data("LA3_handwritten.sym")),
        (
            "L3a".into(),
            std::fs::read_to_string(format!("{fx}/small/m2-slice/families/L3a.sym")).unwrap(),
        ),
        ("T001c".into(), core_data("T001c.sym")),
        ("bubu".into(), core_data("bubu.sym")),
        ("T070c".into(), core_data("T070c.sym")),
        ("T162".into(), core_data("T162-presented-P3.sym")),
    ];
    let mut rng = SplitMix64::new(5);
    for (name, text) in texts {
        let fi = Family::parse_with(&Independent, &text).unwrap();
        let fs = Family::parse_with(&s, &text).unwrap();
        let n = fi.n();
        assert_eq!(n, fs.n());
        for op in 0..fi.n_ops() {
            assert_eq!(
                fi.exact_row(op).unwrap(),
                fs.exact_row_with(&s, op).unwrap(),
                "{name} op {op}"
            );
        }
        let masks: Vec<u64> = if n <= 14 {
            (0..1u64 << n).collect()
        } else {
            (0..1500).map(|_| rng.next_u64() & ((1 << n) - 1)).collect()
        };
        let mut zeros = 0;
        let t0 = std::time::Instant::now();
        for &m in &masks {
            let mask: Vec<bool> = (0..n).map(|k| m >> k & 1 == 1).collect();
            let a = fi.zero_mask_with(&Independent, &mask);
            assert_eq!(a, fs.zero_mask_with(&s, &mask), "{name} mask {m:b}");
            zeros += usize::from(a);
        }
        println!(
            "{name}: {} operator rows and {} sector masks equal ({zeros} scaleless) in {:.2} s",
            fi.n_ops(),
            masks.len(),
            t0.elapsed().as_secs_f64()
        );
    }
}

/// A random expanded polynomial of the engine grammar in n0..n{a-1}, d.
fn poly_text(rng: &mut SplitMix64, arity: usize, terms: usize) -> String {
    poly_text_c(rng, arity, terms, true)
}

/// `big`: coefficients of up to ~38 digits (coefficient texts); else small ones (case literals are small integers:
/// the independent literal type is checked i128 with a big-integer fallback whose normal form overflows sooner).
fn poly_text_c(rng: &mut SplitMix64, arity: usize, terms: usize, big: bool) -> String {
    let mut out = String::new();
    for t in 0..terms {
        let sign = if rng.next_u64() & 1 == 1 { "-" } else { "+" };
        if t > 0 || sign == "-" {
            out.push_str(sign);
        }
        let c = match rng.range(0, 6) {
            0 if big => format!("{}{}", rng.range(1, 1 << 62), rng.range(1, 1 << 62)),
            1 => "1".to_string(),
            _ => rng.range(1, 400).to_string(),
        };
        let mut f = vec![c];
        for _ in 0..rng.range(0, 4) {
            let v = if rng.range(0, 4) == 0 {
                "d".to_string()
            } else {
                format!("n{}", rng.range(0, arity as u64))
            };
            let e = rng.range(1, 4);
            f.push(if e == 1 { v } else { format!("{v}^{e}") });
        }
        out.push_str(&f.join("*"));
    }
    out
}

fn ratio<A: CheckAlgebra>(
    a: &A,
    text: &str,
    arity: usize,
    ctxs: &[Ctx],
) -> Result<Vec<Option<u64>>, String> {
    let (mut n, mut d) = (vec![0; ctxs.len()], vec![0; ctxs.len()]);
    a.eval_ratio(text, arity, ctxs, &mut n, &mut d)
        .map_err(|e| e.to_string())?;
    Ok(ctxs
        .iter()
        .enumerate()
        .map(|(k, c)| a.inv(c.p, d[k]).map(|iv| a.mul(c.p, n[k], iv)))
        .collect())
}

fn contexts(rng: &mut SplitMix64, arity: usize, k: usize) -> Vec<Ctx> {
    let ps = verify_primes(rng.next_u64(), &[]);
    (0..k)
        .map(|j| {
            let p = ps[j % 2];
            Ctx {
                p,
                vals: (0..=arity).map(|_| rng.range(2, p)).collect(),
            }
        })
        .collect()
}

#[test]
fn diff_coefficient_grammar_fuzz() {
    let Some(s) = sym("diff_coefficient_grammar_fuzz") else {
        return;
    };
    let mut rng = SplitMix64::new(6);
    let arity = 5;
    let (mut equal, mut poles) = (0, 0);
    for k in 0..3000 {
        let (tp, tq) = (rng.range(1, 6) as usize, rng.range(1, 4) as usize);
        let p = poly_text(&mut rng, arity, tp);
        let q = poly_text(&mut rng, arity, tq);
        let m = poly_text(&mut rng, arity, 1)
            .trim_start_matches('-')
            .to_string();
        // a bare denominator must be ONE factor in the independent grammar (audit M1): `(P)/M` uses one variable
        let m1 = format!("n{}", rng.range(0, arity as u64));
        let text = match k % 6 {
            0 => format!("({p})/({q})"),
            1 => format!("-({p})/({q})"),
            2 => format!("({p})"),
            3 => p.clone(),
            4 => format!("{m}/({q})"),
            _ => format!("({p})/{m1}"),
        };
        let text = if k % 7 == 0 {
            text.replace('*', " * ").replace('+', " + ")
        } else {
            text
        };
        let ctxs = contexts(&mut rng, arity, 3);
        let a = ratio(&Independent, &text, arity, &ctxs);
        let b = ratio(&s, &text, arity, &ctxs);
        match (&a, &b) {
            (Ok(x), Ok(y)) => {
                for (u, v) in x.iter().zip(y) {
                    if u.is_none() || v.is_none() {
                        poles += 1;
                    } else {
                        assert_eq!(u, v, "{text}");
                    }
                }
                equal += 1;
            }
            _ => panic!("{text}: independent {a:?} symbolica {b:?}"),
        }
    }
    println!("coefficient fuzz: {equal} texts equal at 3 contexts ({poles} pole cells skipped)");
}

/// Every coefficient of real rule texts: the M2 slice tables (arities 5 / 9 / 14 / 20), the F6 span cases, the
/// deep and large scalar test texts.
#[test]
fn diff_coefficient_real_texts() {
    let Some(s) = sym("diff_coefficient_real_texts") else {
        return;
    };
    let fx = campaign_fixtures();
    let mut rng = SplitMix64::new(7);
    let mut texts: Vec<(usize, String)> = Vec::new();
    for (node, arity, max) in [
        ("TOY2kite", 5, 200),
        ("L3a", 9, 200),
        ("L4a", 14, 120),
        ("T281-b6", 20, 60),
    ] {
        let (_, rules) = table_rules(
            &format!("{fx}/small/m2-slice/{node}/table.jsonl"),
            arity,
            max,
        );
        for r in rules {
            texts.extend(r.rhs.iter().map(|(_, c)| (arity, c.clone())));
        }
    }
    if let Some(o) = oracle() {
        for case in o.jsonl("span/F6_span_cases.jsonl") {
            if let Some(line) = f6_line(&o, &case) {
                for r in
                    rustred_campaign_core::ibp_rows::RowsRule::from_json_line(line.trim_end(), 20)
                        .unwrap()
                {
                    texts.extend(r.rhs.iter().map(|(_, c)| (20, c.clone())));
                }
            }
        }
    }
    let t0 = std::time::Instant::now();
    let (mut ti, mut ts) = (0.0, 0.0);
    let mut cells = 0;
    let mut differ = Vec::new();
    for (arity, text) in &texts {
        let ctxs = contexts(&mut rng, *arity, 2);
        let t1 = std::time::Instant::now();
        let a = ratio(&Independent, text, *arity, &ctxs);
        ti += t1.elapsed().as_secs_f64();
        let t2 = std::time::Instant::now();
        let b = ratio(&s, text, *arity, &ctxs);
        ts += t2.elapsed().as_secs_f64();
        match (a, b) {
            (Ok(x), Ok(y)) => {
                for (u, v) in x.iter().zip(&y) {
                    if u.is_some() && v.is_some() {
                        cells += 1;
                        if u != v {
                            differ.push(text.chars().take(80).collect::<String>());
                        }
                    }
                }
            }
            (a, b) => differ.push(format!("{:.80}: {a:?} / {b:?}", text)),
        }
    }
    println!(
        "real coefficient texts: {} texts, {cells} cells equal, {} differ; independent {ti:.2} s, symbolica {ts:.2} s ({:.1} s total)",
        texts.len(),
        differ.len(),
        t0.elapsed().as_secs_f64()
    );
    assert!(differ.is_empty(), "{differ:?}");
}

#[test]
fn diff_expressions_and_literals() {
    let Some(s) = sym("diff_expressions_and_literals") else {
        return;
    };
    let i = Independent;
    let mut rng = SplitMix64::new(8);
    let n = 6;
    for k in 0..2000 {
        // affine forms with rational coefficients, and non-affine ones
        let mut t = format!("{}", rng.range(0, 50) as i64 - 25);
        for v in 0..n + 1 {
            if rng.range(0, 3) == 0 {
                let var = if v == n {
                    "d".to_string()
                } else {
                    format!("n{v}")
                };
                // nonzero coefficients: `0*d` is "contains d" for the independent reader (syntactic), not for
                // Symbolica (normalised): a documented difference of expr_has_d
                let c = [-4i64, -3, -2, -1, 1, 2, 3, 4][rng.range(0, 8) as usize];
                t.push_str(&format!("+{c}*{var}/{}", rng.range(1, 4)));
            }
        }
        if k % 5 == 0 {
            t = format!("({t})/2");
        }
        if k % 11 == 0 {
            t.push_str("+n0*n1");
        }
        let (a, b) = (i.expr_linear(&t, n), s.expr_linear(&t, n));
        assert_eq!(a.is_ok(), b.is_ok(), "{t}: {a:?} {b:?}");
        if let (Ok(x), Ok(y)) = (&a, &b) {
            assert_eq!(x, y, "{t}");
        }
        assert_eq!(
            i.expr_has_d(&t, n).unwrap(),
            s.expr_has_d(&t, n).unwrap(),
            "{t}"
        );
        // case literals: integer polynomials in n, exact zero test, equality up to a constant
        let tl = rng.range(1, 4) as usize;
        let lit = poly_text_c(&mut rng, n, tl, false).replace('d', "n5");
        let (la, lb) = (
            i.literal_parse(&lit, n).unwrap(),
            s.literal_parse(&lit, n).unwrap(),
        );
        for _ in 0..5 {
            let pt: Vec<i32> = (0..n).map(|_| rng.range(0, 7) as i32 - 3).collect();
            assert_eq!(
                i.literal_is_zero_at(&la, &pt).ok(),
                s.literal_is_zero_at(&lb, &pt).ok(),
                "{lit} at {pt:?}"
            );
        }
        // the independent literal type takes expanded text only: compare on an expanded multiple
        let mult = lit.replace("+", "+3*").replace("-", "-3*");
        let mult = if mult.starts_with(char::is_numeric) || mult.starts_with('n') {
            format!("3*{mult}")
        } else {
            mult
        };
        assert_eq!(
            i.literal_same_up_to_constant(&lit, &mult, n),
            s.literal_same_up_to_constant(&lit, &mult, n),
            "{lit} ~ {mult}"
        );
    }
    // zero tests on the real equations and exception literals of the M2 tables
    let fx = campaign_fixtures();
    let (_, rules) = table_rules(&format!("{fx}/small/m2-slice/L4a/table.jsonl"), 14, 400);
    let mut lits = 0;
    for r in &rules {
        for t in r.equations.iter().chain(r.exceptions.iter().flatten()) {
            let (la, lb) = (i.literal_parse(t, 14), s.literal_parse(t, 14));
            assert_eq!(la.is_ok(), lb.is_ok(), "{t}");
            if let (Ok(la), Ok(lb)) = (la, lb) {
                for _ in 0..4 {
                    let pt: Vec<i32> = (0..14).map(|_| rng.range(0, 5) as i32 - 2).collect();
                    assert_eq!(
                        i.literal_is_zero_at(&la, &pt).ok(),
                        s.literal_is_zero_at(&lb, &pt).ok(),
                        "{t}"
                    );
                }
                lits += 1;
            }
        }
    }
    println!("expressions: 2000 random forms equal; {lits} real literals equal at 4 points each");
}

fn sparse_system(
    rng: &mut SplitMix64,
    p: u64,
    rows: usize,
    cols: usize,
    rank: usize,
) -> Vec<Vec<(usize, u64)>> {
    // rank generators, the other rows random combinations of them (dependencies)
    let gens: Vec<Vec<u64>> = (0..rank)
        .map(|_| {
            let mut g = vec![0u64; cols];
            for x in g.iter_mut() {
                if rng.range(0, 5) == 0 {
                    *x = rng.range(1, p);
                }
            }
            g
        })
        .collect();
    (0..rows)
        .map(|k| {
            let dense: Vec<u64> = if k < rank {
                gens[k].clone()
            } else {
                let mut v = vec![0u64; cols];
                for g in &gens {
                    if rng.range(0, 4) == 0 {
                        let c = rng.range(1, p);
                        for (x, y) in v.iter_mut().zip(g) {
                            *x = Independent.add(p, *x, Independent.mul(p, c, *y));
                        }
                    }
                }
                v
            };
            dense
                .into_iter()
                .enumerate()
                .filter(|(_, x)| *x != 0)
                .collect()
        })
        .collect()
}

#[test]
fn diff_sparse_elimination() {
    let Some(s) = sym("diff_sparse_elimination") else {
        return;
    };
    let mut rng = SplitMix64::new(9);
    let (mut solves, mut in_span) = (0, 0);
    for case in 0..60 {
        let p = verify_primes(case, &[])[0];
        let cols = rng.range(10, 300) as usize;
        let rows = rng.range(5, 250) as usize;
        let rank = rng.range(1, rows.min(cols) as u64 + 1) as usize;
        let sys = sparse_system(&mut rng, p, rows, cols, rank);
        let ei = Independent.eliminate(&sys, cols, p, usize::MAX).unwrap();
        let es = s.eliminate(&sys, cols, p, usize::MAX).unwrap();
        assert_eq!(ei.rank(), es.rank(), "case {case} rank");
        let (mut pi, mut ps) = (ei.pivot_columns(), es.pivot_columns());
        pi.sort_unstable();
        ps.sort_unstable();
        assert_eq!(pi, ps, "case {case} pivot column set");
        for t in 0..6 {
            // in the span: a combination of rows; outside (usually): a random vector
            let desired: Vec<(usize, u64)> = if t % 2 == 0 {
                let mut v = vec![0u64; cols];
                for r in &sys {
                    if rng.range(0, 3) == 0 {
                        let c = rng.range(1, p);
                        for &(j, x) in r {
                            v[j] = Independent.add(p, v[j], Independent.mul(p, c, x));
                        }
                    }
                }
                v.into_iter().enumerate().filter(|(_, x)| *x != 0).collect()
            } else {
                let mut v = Vec::new();
                for j in 0..cols {
                    if rng.range(0, 6) == 0 {
                        v.push((j, rng.range(1, p)));
                    }
                }
                v
            };
            let (a, b) = (ei.solve(&desired).unwrap(), es.solve(&desired).unwrap());
            assert_eq!(a.in_span, b.in_span, "case {case} vector {t}");
            assert_eq!(
                a.remainder.first().map(|x| x.0),
                b.remainder.first().map(|x| x.0),
                "case {case} vector {t}: witness column"
            );
            solves += 1;
            in_span += usize::from(a.in_span);
        }
    }
    println!(
        "sparse elimination: 60 systems (rank, pivot set equal), {solves} span tests equal ({in_span} in the span), certificates re-verified on both sides"
    );
}
