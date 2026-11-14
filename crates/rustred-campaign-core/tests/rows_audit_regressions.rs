//! Regression tests for the findings of audit A (WS/notes/implementer.audit-rows-A.md) on `ibp_rows` / `replay_base`,
//! one test per finding id. The boundary-face construction (C1) follows the auditor's experiment `e3_boundary_face`
//! (LA3_handwritten.sym, 3 loops, 9 slots): a TRUE rule (one IBP row solved for its top term) and a FALSE copy with an
//! extra term `1*I(Z)` whose column is scaleless in the generic convention but not on the face n_j = 0.

mod rows_common;

use rows_common::*;
use rustred_campaign_core::ibp_rows::{
    LocusStatus, PointSpec, ReplayLevel, ReplayPolicy, RowsOptions, RowsRule, TableBinding,
    Unsupported, Verdict, identity_test, replay_evidence,
};
use rustred_campaign_core::replay_base::arith::{self, Q};
use rustred_campaign_core::replay_base::coeff::{Ctx, cross_check_engine, eval_ratio};
use rustred_campaign_core::replay_base::expr::Expr;
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::primes::verify_primes;
use serde_json::{Value, json};

fn gcd(a: i128, b: i128) -> i128 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

fn form_text(form: &[Q], m: i128, n: usize) -> String {
    let mut terms = Vec::new();
    for (j, q) in form.iter().enumerate() {
        if q.is_zero() {
            continue;
        }
        let c = q.n * (m / q.d);
        let var = match j {
            0 => String::new(),
            j if j == n + 1 => "d".into(),
            j => format!("n{}", j - 1),
        };
        terms.push(if var.is_empty() {
            format!("{c}")
        } else {
            format!("{c}*{var}")
        });
    }
    let s = terms.join("+").replace("+-", "-");
    if s.is_empty() { "0".into() } else { s }
}

fn pat(shift: &[i64]) -> String {
    let parts: Vec<String> = shift
        .iter()
        .enumerate()
        .map(|(i, &o)| match o {
            0 => format!("n{i}"),
            o if o > 0 => format!("n{i}+{o}"),
            o => format!("n{i}{o}"),
        })
        .collect();
    format!("I({})", parts.join(","))
}

struct Face {
    fam: Family,
    good: Value,
    bad: Value,
    j: usize,
    tshift: Vec<i64>,
}

fn face() -> Face {
    let fam = Family::parse(&data("LA3_handwritten.sym")).unwrap();
    let n = fam.n();
    let mut found = None;
    'o: for m in 1u32..(1 << 7) {
        let s: Vec<bool> = (0..n).map(|i| i < 7 && m >> i & 1 == 1).collect();
        if !fam.zero_mask(&s) {
            continue;
        }
        for j in [7usize, 8] {
            let mut t = s.clone();
            t[j] = true;
            if !fam.zero_mask(&t) {
                found = Some((s, j));
                break 'o;
            }
        }
    }
    let (s, j) = found.unwrap();
    let row = fam.exact_row(0).unwrap();
    let (tshift, tform) = row
        .iter()
        .filter(|(_, f)| f[1..=n].iter().any(|q| !q.is_zero()))
        .max_by_key(|(s, _)| s.iter().sum::<i64>())
        .map(|(s, f)| (s.clone(), f.clone()))
        .unwrap();
    let mut mm: i128 = 1;
    for f in row.values() {
        for q in f {
            mm = mm / gcd(mm, q.d) * q.d;
        }
    }
    let den = form_text(&tform, mm, n);
    let mut rhs: Vec<Value> = row
        .iter()
        .filter(|(sh, _)| **sh != tshift)
        .map(|(sh, f)| {
            let neg: Vec<Q> = f.iter().map(|q| q.neg()).collect();
            json!([pat(sh), format!("({})/({den})", form_text(&neg, mm, n))])
        })
        .collect();
    let chart: String = (0..n).map(|i| if i < 7 { '1' } else { '0' }).collect();
    let good = json!({
        "type": "rule", "index": 0, "chart": chart, "fixed": vec![Value::Null; n],
        "equations": [], "exceptions": [], "target": pat(&tshift), "rhs": rhs.clone(),
        "seeds": [pat(&vec![0; n])], "recenter": vec![0; n], "witness_loci": [], "guard_version": 2,
    });
    let zparts: Vec<String> = (0..n)
        .map(|i| {
            if i < 7 && !s[i] {
                "0".into()
            } else {
                let o = tshift[i] + i64::from(i == j);
                match o {
                    0 => format!("n{i}"),
                    o if o > 0 => format!("n{i}+{o}"),
                    o => format!("n{i}{o}"),
                }
            }
        })
        .collect();
    rhs.push(json!([format!("I({})", zparts.join(",")), "1"]));
    let mut bad = good.clone();
    bad["rhs"] = json!(rhs);
    Face {
        fam,
        good,
        bad,
        j,
        tshift,
    }
}

fn served(f: &Face) -> Vec<Vec<i64>> {
    let n = f.fam.n();
    let mut out: Vec<Vec<i64>> = Vec::new();
    for t in 0..400i64 {
        let mut pt: Vec<i64> = (0..n)
            .map(|i| {
                let h = (t * 2654435761 + (i as i64) * 40503).rem_euclid(1000003);
                if i < 7 { 1 + h % 4 } else { -1 - (h / 7) % 4 }
            })
            .collect();
        if pt[f.j] == 0 {
            pt[f.j] = -1;
        }
        out.push((0..n).map(|i| pt[i] + f.tshift[i]).collect());
    }
    for a in 0..3i64 {
        let mut pt: Vec<i64> = (0..n)
            .map(|i| {
                if i < 7 {
                    2 + (a + i as i64) % 3
                } else {
                    -1 - (i as i64) % 3
                }
            })
            .collect();
        pt[f.j] = -f.tshift[f.j];
        out.push((0..n).map(|i| pt[i] + f.tshift[i]).collect());
    }
    out.sort();
    out.dedup();
    out
}

/// C1: the false rule is never `replay` Established (20 seeds); the true rule is; a sample is only `ReplaySampled`.
#[test]
fn c1_false_rule_on_boundary_face_never_established() {
    let f = face();
    let n = f.fam.n();
    let keys = served(&f);
    let good = RowsRule::from_value(f.good.clone(), n).unwrap();
    let bad = RowsRule::from_value(f.bad.clone(), n).unwrap();
    // generic points cannot see the face (documented limit; the replay level must not rest on them)
    let g = identity_test(
        &f.fam,
        &bad,
        &[PointSpec::Generic, PointSpec::Generic],
        &RowsOptions::default(),
    );
    assert_eq!(g.verdict, Verdict::Pass);
    let t0 = std::time::Instant::now();
    let rg = replay_evidence(
        &f.fam,
        &good,
        &keys,
        &ReplayPolicy::default(),
        &bound(&f.fam),
    );
    println!(
        "C1 good: {:?}, {} served, {} replayed, {:.2}s",
        rg.level,
        keys.len(),
        rg.served_keys_replayed,
        t0.elapsed().as_secs_f64()
    );
    assert_eq!(rg.level, ReplayLevel::Established);
    assert_eq!(rg.served_keys_replayed, keys.len());
    let mut est = 0;
    for seed in 0..20u64 {
        let o = RowsOptions {
            seed,
            ..bound(&f.fam)
        };
        let r = replay_evidence(&f.fam, &bad, &keys, &ReplayPolicy::default(), &o);
        if r.level == ReplayLevel::Established {
            est += 1;
        }
        assert!(
            matches!(r.identity.verdict, Verdict::Fail { .. }),
            "{:?}",
            r.identity.verdict
        );
    }
    println!("C1 bad: Established in {est}/20 seeds (audit: 9/20 before the fix)");
    assert_eq!(est, 0);
    // a sampled policy never yields Established
    let sp = ReplayPolicy {
        max_identity_keys: 8,
        ..ReplayPolicy::default()
    };
    let rs = replay_evidence(&f.fam, &good, &keys, &sp, &bound(&f.fam));
    assert!(
        matches!(rs.level, ReplayLevel::ReplaySampled { replayed: 8, .. }),
        "{:?}",
        rs.level
    );
}

/// C2: families the replay does not model are refused; the table header binds the family; digests are recorded.
#[test]
fn c2_family_validation_and_binding() {
    let src = data("LA3_handwritten.sym");
    let f = face();
    let n = f.fam.n();
    let good = RowsRule::from_value(f.good.clone(), n).unwrap();
    for alt in ["gram(q,q,-1)", "gram(q,q,0)", "gram(q,q,s)"] {
        let fam = Family::parse(&src.replace("gram(q,q,1)", alt)).unwrap();
        let v = identity_test(&fam, &good, &[PointSpec::Generic], &RowsOptions::default());
        println!("C2 {alt}: {:?}", v.verdict);
        assert!(
            matches!(v.verdict, Verdict::Unsupported(Unsupported::Family(_))),
            "{alt}"
        );
    }
    let fam = Family::parse(&src.replace("dimension(d)", "dimension(4-2*eps)")).unwrap();
    let v = identity_test(&fam, &good, &[PointSpec::Generic], &RowsOptions::default());
    assert!(matches!(
        v.verdict,
        Verdict::Unsupported(Unsupported::Family(_))
    ));
    // header binding: matching source passes, another source / name / loop count is refused
    let ok = TableBinding::from_header(
        &json!({"type": "header", "family_source": src, "loops": f.fam.loops(),
        "denominators": n, "parameters": ["d"], "source_conditions": []}),
    )
    .unwrap();
    let v = identity_test(
        &f.fam,
        &good,
        &[PointSpec::Generic],
        &RowsOptions {
            table: Some(ok),
            ..RowsOptions::default()
        },
    );
    assert_eq!(v.verdict, Verdict::Pass);
    assert!(v.family_bound_to_table);
    assert_eq!(v.family_digest, f.fam.source_digest().to_string());
    for h in [
        json!({"type": "header", "family_source": src.replace("prop(D1", "prop(D1 ")}), // whitespace only: accepted
        json!({"type": "header", "family_source": t162_src()}),
        json!({"type": "cert_header", "family": "T162"}),
        json!({"type": "header", "loops": 5}),
        json!({"type": "header", "parameters": ["d", "s"]}),
        json!({"type": "header", "source_conditions": ["x"]}),
    ]
    .iter()
    .enumerate()
    {
        let o = RowsOptions {
            table: TableBinding::from_header(h.1),
            ..RowsOptions::default()
        };
        let v = identity_test(&f.fam, &good, &[PointSpec::Generic], &o);
        if h.0 == 0 {
            assert_eq!(v.verdict, Verdict::Pass);
        } else {
            assert!(
                matches!(v.verdict, Verdict::Unsupported(Unsupported::Family(_))),
                "{h:?} {:?}",
                v.verdict
            );
        }
    }
    let rec = replay_evidence(
        &f.fam,
        &good,
        &served(&f)[..3],
        &ReplayPolicy::default(),
        &RowsOptions::default(),
    );
    assert_eq!(rec.family_digest, f.fam.source_digest().to_string());
    assert!(!rec.family_bound_to_table);
    // re-audit A NEW-M1: no binding -> the level is never granted
    assert!(
        matches!(rec.level, ReplayLevel::Unbound { .. }),
        "{:?}",
        rec.level
    );
}

fn t162_src() -> String {
    data("T162-presented-P3.sym")
}

/// M1: `a/b*c` (left-associative in the engine) is never evaluated with a second meaning: refused by the streaming
/// reader, and every accepted form agrees with the copied engine reader (differential fuzz).
#[test]
fn m1_precedence_identical_or_refused() {
    let p = verify_primes(1, &[])[0];
    let pt: Vec<i64> = vec![3, 5, 7];
    let c = Ctx {
        p,
        vals: pt
            .iter()
            .chain([&11i64])
            .map(|&x| arith::from_i128(p, x as i128))
            .collect(),
    };
    for t in ["1/2*n1", "(n0)/2*n1", "3/4*n1*d", "-1/2*n2", "(n0+1)/2*d"] {
        let (mut nn, mut dd) = ([0u64], [0u64]);
        let r = eval_ratio(t, 3, std::slice::from_ref(&c), &mut nn, &mut dd);
        println!(
            "M1 {t}: {:?}",
            r.as_ref().map(|_| ()).map_err(|e| e.to_string())
        );
        if r.is_ok() {
            let eng = Expr::parse(t, 3)
                .unwrap()
                .at_point(p, &pt, Q::int(11))
                .unwrap();
            assert_eq!(
                arith::mul(p, nn[0], arith::inv(p, dd[0]).unwrap()),
                eng,
                "{t}"
            );
        }
    }
    // fuzz: random texts in the shared grammar, both readers
    let mut s = rustred_campaign_core::replay_base::primes::SplitMix64::new(42);
    let (mut accepted, mut refused) = (0, 0);
    let factor = |s: &mut rustred_campaign_core::replay_base::primes::SplitMix64| -> String {
        match s.range(0, 4) {
            0 => s.range(1, 30).to_string(),
            1 => format!("n{}", s.range(0, 3)),
            2 => "d".into(),
            _ => format!("n{}^{}", s.range(0, 3), s.range(1, 4)),
        }
    };
    for _ in 0..3000 {
        let mono = |s: &mut rustred_campaign_core::replay_base::primes::SplitMix64| -> String {
            (0..s.range(1, 4))
                .map(|_| factor(s))
                .collect::<Vec<_>>()
                .join("*")
        };
        let poly = |s: &mut rustred_campaign_core::replay_base::primes::SplitMix64| -> String {
            let mut t = mono(s);
            for _ in 0..s.range(0, 3) {
                t.push(if s.range(0, 2) == 0 { '+' } else { '-' });
                t.push_str(&mono(s));
            }
            t
        };
        let text = match s.range(0, 5) {
            0 => format!("({})/({})", poly(&mut s), poly(&mut s)),
            1 => format!("{}/{}", mono(&mut s), mono(&mut s)),
            2 => format!("-({})/{}", poly(&mut s), mono(&mut s)),
            3 => poly(&mut s),
            _ => format!("({})/{}*{}", poly(&mut s), factor(&mut s), factor(&mut s)),
        };
        let (mut nn, mut dd) = ([0u64], [0u64]);
        match eval_ratio(&text, 3, std::slice::from_ref(&c), &mut nn, &mut dd) {
            Ok(_) => {
                accepted += 1;
                if dd[0] != 0 {
                    assert_eq!(
                        cross_check_engine(&text, 3, &c, nn[0], dd[0]),
                        Ok(true),
                        "{text}"
                    );
                }
            }
            Err(_) => refused += 1,
        }
    }
    println!("M1 fuzz: {accepted} accepted (all equal to the engine reader), {refused} refused");
    assert!(accepted > 1000);
}

/// M2: no applicable served key (all outside the chart) is never Established.
#[test]
fn m2_nothing_served_is_not_established() {
    let f = face();
    let good = RowsRule::from_value(f.good.clone(), f.fam.n()).unwrap();
    let mis: Vec<Vec<i64>> = served(&f)
        .iter()
        .map(|k| k.iter().map(|x| -x).collect())
        .collect();
    let r = replay_evidence(
        &f.fam,
        &good,
        &mis,
        &ReplayPolicy::default(),
        &bound(&f.fam),
    );
    println!("M2: {:?}", r.level);
    match &r.level {
        ReplayLevel::NotEstablished { reasons } => {
            assert!(reasons.iter().any(|x| x.starts_with("NothingServed")))
        }
        l => panic!("{l:?}"),
    }
    let r = replay_evidence(&f.fam, &good, &[], &ReplayPolicy::default(), &bound(&f.fam));
    assert!(matches!(r.level, ReplayLevel::NotEstablished { .. }));
}

/// M3: an empty witness list without witness guards, and an untested locus, block the level.
#[test]
fn m3_witness_guards_and_untested_loci() {
    let f = face();
    let keys = served(&f);
    let mut nog = f.good.clone();
    nog.as_object_mut().unwrap().remove("guard_version");
    let r = replay_evidence(
        &f.fam,
        &RowsRule::from_value(nog.clone(), f.fam.n()).unwrap(),
        &keys,
        &ReplayPolicy::default(),
        &bound(&f.fam),
    );
    assert!(!r.guards.witness_list_complete);
    assert!(matches!(r.level, ReplayLevel::NotEstablished { .. }));
    // the table header may state the guard version
    let t = TableBinding::from_header(&json!({"type": "header", "guard_version": 2,
        "family_source": data("LA3_handwritten.sym")}))
    .unwrap();
    let r = replay_evidence(
        &f.fam,
        &RowsRule::from_value(nog, f.fam.n()).unwrap(),
        &keys,
        &ReplayPolicy::default(),
        &RowsOptions {
            table: Some(t),
            ..bound(&f.fam)
        },
    );
    assert_eq!(r.level, ReplayLevel::Established);
    // a locus without integer points in the window that is not provably empty: UNTESTED -> blocks
    let mut lo = f.good.clone();
    lo["witness_loci"] = json!(["1+n0^2"]);
    let r = replay_evidence(
        &f.fam,
        &RowsRule::from_value(lo, f.fam.n()).unwrap(),
        &keys,
        &ReplayPolicy::default(),
        &bound(&f.fam),
    );
    assert!(
        matches!(r.loci[0].status, LocusStatus::NoPoint { .. }),
        "{:?}",
        r.loci[0].status
    );
    assert!(matches!(r.level, ReplayLevel::NotEstablished { .. }));
    // an affine locus with no integer point is proved empty; a served key on a locus is recorded as tested there
    let mut lo = f.good.clone();
    lo["witness_loci"] = json!(["2*n0-1", format!("n{}-({})", 0, keys[0][0] - f.tshift[0])]);
    let r = replay_evidence(
        &f.fam,
        &RowsRule::from_value(lo, f.fam.n()).unwrap(),
        &keys,
        &ReplayPolicy::default(),
        &bound(&f.fam),
    );
    println!(
        "M3 loci: {:?}",
        r.loci.iter().map(|l| &l.status).collect::<Vec<_>>()
    );
    assert!(matches!(r.loci[0].status, LocusStatus::NoIntegerPoint));
    assert!(matches!(
        r.loci[1].status,
        LocusStatus::TestedAtServed { .. }
    ));
    assert_eq!(r.level, ReplayLevel::Established);
}

/// M4: the record names family, rule and served-key digests, the key source and the strata mode.
#[test]
fn m4_record_is_reproducible() {
    let f = face();
    let good = RowsRule::from_value(f.good.clone(), f.fam.n()).unwrap();
    let keys = served(&f);
    let pol = ReplayPolicy {
        served_keys_source: Some("test:served(face)".into()),
        ..ReplayPolicy::default()
    };
    let a = replay_evidence(&f.fam, &good, &keys, &pol, &bound(&f.fam));
    let mut rev = keys.clone();
    rev.reverse();
    let b = replay_evidence(&f.fam, &good, &rev, &pol, &bound(&f.fam));
    assert_eq!(a.served_keys_digest, b.served_keys_digest);
    assert_eq!(a.served_keys_digest.len(), 64);
    assert_eq!(a.served_keys_source.as_deref(), Some("test:served(face)"));
    assert!(a.strata.starts_with("explicit"));
    let js = serde_json::to_value(&a).unwrap();
    for k in [
        "family_digest",
        "rule_digest",
        "served_keys_digest",
        "primes",
        "tool_version",
        "identity",
        "pole_scan",
    ] {
        assert!(js.get(k).is_some(), "{k}");
    }
    // the rule digest binds index and raw pivot
    let mut g2 = f.good.clone();
    g2["index"] = json!(7);
    assert_ne!(
        rustred_campaign_core::ibp_rows::rule_digest(&good),
        rustred_campaign_core::ibp_rows::rule_digest(&RowsRule::from_value(g2, f.fam.n()).unwrap())
    );
}

/// minor m3 / m4: empty exception branch and a target in a zero sector are refused (as the engine).
#[test]
fn m3m4_minor_refusals() {
    let f = face();
    let n = f.fam.n();
    let mut e = f.good.clone();
    e["exceptions"] = json!([[]]);
    let v = identity_test(
        &f.fam,
        &RowsRule::from_value(e, n).unwrap(),
        &[PointSpec::Generic],
        &RowsOptions::default(),
    );
    assert!(matches!(
        v.verdict,
        Verdict::Unsupported(Unsupported::EmptyExceptionBranch { branch: 0 })
    ));
    // target in a zero sector: lines outside a scaleless set at 0
    let mut z = f.good.clone();
    let mut zero_lines = None;
    for m in 1u32..(1 << 7) {
        let s: Vec<bool> = (0..n).map(|i| i < 7 && m >> i & 1 == 1).collect();
        if f.fam.zero_mask(&s) {
            zero_lines = Some(s);
            break;
        }
    }
    let s = zero_lines.unwrap();
    let key: Vec<String> = (0..n)
        .map(|i| {
            if i < 7 && !s[i] {
                "0".into()
            } else if i < 7 {
                "1".into()
            } else {
                "0".into()
            }
        })
        .collect();
    z["target"] = json!(format!("I({})", key.join(",")));
    z["fixed"] = json!(
        key.iter()
            .map(|x| x.parse::<i64>().unwrap())
            .collect::<Vec<_>>()
    );
    z["chart"] = json!(
        (0..n)
            .map(|i| if s[i] { '1' } else { '0' })
            .collect::<String>()
    );
    z["rhs"] = json!([[format!("I({})", key.join(",")).replacen("1", "2", 1), "1"]]);
    z["seeds"] = json!([format!("I({})", key.join(","))]);
    let r = RowsRule::from_value(z, n).unwrap();
    let k: Vec<i64> = key.iter().map(|x| x.parse().unwrap()).collect();
    let v = identity_test(&f.fam, &r, &[PointSpec::Key(k)], &RowsOptions::default());
    println!("m4: {:?}", v.verdict);
    assert_eq!(
        v.verdict,
        Verdict::Unsupported(Unsupported::TargetInZeroSector)
    );
}

/// audit B (first finding, same mechanism as C1): a generic-only pass with boundary-sensitive columns does not meet
/// the A2 policy; the true rule's generic pass has no such column issue only if none is dropped (reported either way).
#[test]
fn b1_generic_only_pass_with_boundary_columns_is_not_a2() {
    let f = face();
    let bad = RowsRule::from_value(f.bad.clone(), f.fam.n()).unwrap();
    let v = identity_test(
        &f.fam,
        &bad,
        &[PointSpec::Generic, PointSpec::Generic],
        &RowsOptions::default(),
    );
    let bc: Vec<usize> = v
        .points
        .iter()
        .flat_map(|p| p.cells.iter().map(|c| c.boundary_columns))
        .collect();
    println!(
        "B1 bad: {:?} meets_a2 {} boundary columns {bc:?}",
        v.verdict, v.meets_a2_policy
    );
    assert_eq!(v.verdict, Verdict::Pass);
    assert!(bc.iter().all(|&b| b > 0));
    assert!(!v.meets_a2_policy);
}

/// audit B M1: the random stream (primes, d) derives from the rule digest: changing a coefficient changes the primes.
#[test]
fn bm1_stream_seed_bound_to_rule_digest() {
    let f = face();
    let a = RowsRule::from_value(f.good.clone(), f.fam.n()).unwrap();
    let mut g2 = f.good.clone();
    let c = g2["rhs"][0][1].as_str().unwrap().to_string();
    g2["rhs"][0][1] = json!(format!("{c}+0"));
    let b = RowsRule::from_value(g2, f.fam.n()).unwrap();
    let va = identity_test(&f.fam, &a, &[PointSpec::Generic], &RowsOptions::default());
    let vb = identity_test(&f.fam, &b, &[PointSpec::Generic], &RowsOptions::default());
    assert_ne!(va.primes, vb.primes);
    assert_eq!(va.master_seed, vb.master_seed);
    assert_ne!(va.seed, vb.seed);
    let r = replay_evidence(
        &f.fam,
        &a,
        &served(&f)[..5],
        &ReplayPolicy::default(),
        &bound(&f.fam),
    );
    assert_eq!(r.primes, r.identity.primes);
    // a run-time nonce: two runs differ in nonce and primes; re-running with the recorded nonce reproduces the record
    let r2 = replay_evidence(
        &f.fam,
        &a,
        &served(&f)[..5],
        &ReplayPolicy::default(),
        &bound(&f.fam),
    );
    assert_ne!(r.nonce, r2.nonce);
    let o = RowsOptions {
        nonce: Some(r.nonce),
        ..RowsOptions::default()
    };
    let r3 = replay_evidence(&f.fam, &a, &served(&f)[..5], &ReplayPolicy::default(), &o);
    assert_eq!((r3.seed, r3.primes), (r.seed, r.primes));
}

/// audit B M3: a seed set whose rows would not fit is refused BEFORE the rows are built.
#[test]
fn bm3_row_memory_guard_before_building() {
    let f = face();
    let mut g = f.good.clone();
    let seeds: Vec<String> = (0..2000)
        .map(|k| {
            pat(&(0..f.fam.n())
                .map(|i| if i == 0 { k } else { 0 })
                .collect::<Vec<i64>>())
        })
        .collect();
    g["seeds"] = json!(seeds);
    let r = RowsRule::from_value(g, f.fam.n()).unwrap();
    let o = RowsOptions {
        max_row_bytes: 1_000_000,
        ..RowsOptions::default()
    };
    let v = identity_test(&f.fam, &r, &[PointSpec::Generic], &o);
    assert!(
        matches!(v.verdict, Verdict::MemoryGuard { .. }),
        "{:?}",
        v.verdict
    );
}

// ------------------------------------------------------------------ re-audit A (rows3) ------------------------------

/// re-audit A NEW-M1: family binding is MANDATORY for `replay`. No header / a header without `family_source` / an empty
/// header -> `Unbound` with `family_bound_to_table: false`; `family_source` or the spec digest compared and equal ->
/// `Established` with `family_bound_to_table: true`; a different spec digest is refused.
#[test]
fn ra_m1_family_binding_is_mandatory_for_replay() {
    let src = data("LA3_handwritten.sym");
    let f = face();
    let good = RowsRule::from_value(f.good.clone(), f.fam.n()).unwrap();
    let keys = &served(&f)[..4];
    let run = |table: Option<TableBinding>| {
        replay_evidence(
            &f.fam,
            &good,
            keys,
            &ReplayPolicy::default(),
            &RowsOptions {
                table,
                ..RowsOptions::default()
            },
        )
    };
    for (what, table) in [
        ("no header", None),
        (
            "empty header",
            TableBinding::from_header(&json!({"type": "header"})),
        ),
        (
            "cert_header without family_source",
            TableBinding::from_header(&json!({"type": "cert_header", "family": "LA3",
                "loops": 3, "denominators": 9, "parameters": ["d"], "guard_version": 2})),
        ),
    ] {
        let r = run(table);
        println!(
            "NEW-M1 {what}: {:?} binding {:?}",
            r.level, r.family_binding
        );
        assert!(
            matches!(r.level, ReplayLevel::Unbound { .. }),
            "{what}: {:?}",
            r.level
        );
        assert!(!r.family_bound_to_table, "{what}");
        assert!(!r.identity.family_bound_to_table, "{what}");
    }
    let r = run(TableBinding::from_header(
        &json!({"type": "header", "family_source": src}),
    ));
    assert_eq!(r.level, ReplayLevel::Established);
    assert!(r.family_bound_to_table);
    assert_eq!(r.family_binding, vec!["family_source".to_string()]);
    let r = run(Some(TableBinding::from_family_digest(
        f.fam.squashed_digest(),
    )));
    assert_eq!(r.level, ReplayLevel::Established);
    assert_eq!(r.family_binding, vec!["spec_family_digest".to_string()]);
    let other = Family::parse(&t162_src()).unwrap();
    let r = run(Some(TableBinding::from_family_digest(
        other.squashed_digest(),
    )));
    assert!(
        matches!(
            r.identity.verdict,
            Verdict::Unsupported(Unsupported::Family(_))
        ),
        "{:?}",
        r.identity.verdict
    );
    assert!(matches!(r.level, ReplayLevel::NotEstablished { .. }));
}

/// re-audit A NEW-M2: the family reader never silently models another family: undeclared momentum names, labels out
/// of order, out-of-range / repeated power shifts and malformed propagators are parse ERRORS; constructs the module
/// does not model (incl. a `gram` hidden inside `name(..)`) are refused by name by the family check.
#[test]
fn ra_m2_family_reader_refuses_what_it_does_not_model() {
    let src = data("LA3_handwritten.sym");
    assert!(Family::parse(&src).is_ok());
    for (what, from, to) in [
        (
            "undeclared k4",
            "prop(D9,(k1-k3)^2,0)",
            "prop(D9,(k1-k3+k4)^2,0)",
        ),
        (
            "undeclared q2",
            "prop(D9,(k1-k3)^2,0)",
            "prop(D9,(k1-k3+q2)^2,0)",
        ),
        (
            "undeclared 2*k7",
            "prop(D9,(k1-k3)^2,0)",
            "prop(D9,(k1-k3+2*k7)^2,0)",
        ),
        (
            "labels out of order",
            "prop(D8,(k2-q)^2,0),\n  prop(D9,(k1-k3)^2,0)",
            "prop(D9,(k1-k3)^2,0),\n  prop(D8,(k2-q)^2,0)",
        ),
        ("label gap", "prop(D9,(k1-k3)^2,0)", "prop(D10,(k1-k3)^2,0)"),
        (
            "power shift out of range",
            "gram(q,q,1)",
            "power_shift(D10,(4-d)/2),\n  gram(q,q,1)",
        ),
        (
            "power shift repeated",
            "gram(q,q,1)",
            "power_shift(D1,(4-d)/2),\n  power_shift(D1,(4-d)/2),\n  gram(q,q,1)",
        ),
        ("malformed prop", "prop(D9,(k1-k3)^2,0)", "prop(D9,k1-k3,0)"),
        ("loop named q", "loops(k1,k2,k3)", "loops(k1,k2,q)"),
    ] {
        let t = src.replacen(from, to, 1);
        assert_ne!(t, src, "{what}: replacement did not apply");
        let r = Family::parse(&t);
        println!("NEW-M2 {what}: {:?}", r.as_ref().err());
        assert!(r.is_err(), "{what}: parsed");
    }
    let hidden_gram =
        src.replacen("name(LA3)", "name(gram(q,q,1))", 1)
            .replacen(",\n  gram(q,q,1)", "", 1);
    assert!(
        !hidden_gram.contains("),\n  gram("),
        "the top-level gram must be gone"
    );
    let check = Family::parse(&hidden_gram).map(|f| f.check().clone());
    println!("NEW-M2 gram only inside name: {check:?}");
    assert!(
        matches!(check, Ok(Err(ref e)) if e.contains("gram")),
        "{check:?}"
    );
    for (what, from, to, needle) in [
        (
            "mass construct",
            "gram(q,q,1)",
            "gram(q,q,1),\n  mass(m1)",
            "mass",
        ),
        (
            "second externals",
            "externals(q)",
            "externals(q),externals(p)",
            "externals",
        ),
        (
            "prop flag 2",
            "prop(D9,(k1-k3)^2,0)",
            "prop(D9,(k1-k3)^2,2)",
            "flag",
        ),
    ] {
        let t = src.replacen(from, to, 1);
        let check = Family::parse(&t).map(|f| f.check().clone());
        println!("NEW-M2 {what}: {check:?}");
        match check {
            Ok(Err(e)) | Err(e) => assert!(e.contains(needle), "{what}: {e}"),
            Ok(Ok(())) => panic!("{what}: accepted"),
        }
    }
    // the gram-in-name text must not be accepted by the identity test either
    let f = face();
    let good = RowsRule::from_value(f.good.clone(), f.fam.n()).unwrap();
    let fam = Family::parse(&hidden_gram).unwrap();
    let v = identity_test(&fam, &good, &[PointSpec::Generic], &RowsOptions::default());
    assert!(matches!(
        v.verdict,
        Verdict::Unsupported(Unsupported::Family(_))
    ));
}

/// re-audit A NEW-M3: `VerifiedGroup` and `Family` have no public field (no struct literal, no mutation after the
/// fact) and `VerifiedGroup` derives no `Deserialize`: the only constructors are verify_group / verify_derived /
/// verify_cross_checked. Checked on the source text (a struct literal outside the module would not compile).
#[test]
fn ra_m3_verified_group_and_family_are_unforgeable() {
    let root = env!("CARGO_MANIFEST_DIR");
    for (file, item) in [
        ("src/replay_base/symrows.rs", "pub struct VerifiedGroup {"),
        ("src/replay_base/family.rs", "pub struct Family {"),
    ] {
        let s = std::fs::read_to_string(format!("{root}/{file}")).unwrap();
        let at = s
            .find(item)
            .unwrap_or_else(|| panic!("{item} not in {file}"));
        let body = &s[at..at + s[at..].find("\n}").unwrap()];
        for line in body.lines().skip(1) {
            assert!(
                !line.trim_start().starts_with("pub "),
                "{file}: public field {line}"
            );
        }
        let before = &s[s[..at].rfind("\n\n").unwrap_or(0)..at];
        assert!(
            !before.contains("Deserialize"),
            "{file}: {item} derives Deserialize"
        );
    }
}

/// re-audit A NEW-m1 / NEW-m3 and re-audit B (finding candidate 1b): caller-chosen nonce / primes, controls off and
/// zero generic points are MARKED in the record and block the level; the nonce source is recorded. The B attack: an
/// extra RHS term whose coefficient is p1*p2 vanishes modulo caller-chosen primes [p1, p2] (it passed before).
#[test]
fn ra_minor_overrides_are_marked_and_block_the_level() {
    let f = face();
    let n = f.fam.n();
    let good = RowsRule::from_value(f.good.clone(), n).unwrap();
    let keys = &served(&f)[..4];
    let base = bound(&f.fam);
    let r = replay_evidence(&f.fam, &good, keys, &ReplayPolicy::default(), &base);
    assert_eq!(r.level, ReplayLevel::Established);
    assert_eq!(
        r.nonce_source,
        rustred_campaign_core::ibp_rows::NonceSource::Os
    );
    assert!(r.overrides.is_empty());
    let pp = verify_primes(7, &[]);
    for (what, o, pol) in [
        (
            "caller nonce",
            RowsOptions {
                nonce: Some(7),
                ..base.clone()
            },
            ReplayPolicy::default(),
        ),
        (
            "caller primes",
            RowsOptions {
                primes: Some(pp),
                ..base.clone()
            },
            ReplayPolicy::default(),
        ),
        (
            "controls off",
            RowsOptions {
                controls: false,
                ..base.clone()
            },
            ReplayPolicy::default(),
        ),
        (
            "no generic points",
            base.clone(),
            ReplayPolicy {
                generic_points: 0,
                ..ReplayPolicy::default()
            },
        ),
    ] {
        let r = replay_evidence(&f.fam, &good, keys, &pol, &o);
        println!("override {what}: {:?} {:?}", r.overrides, r.level);
        assert_eq!(r.overrides.len(), 1, "{what}");
        assert!(
            matches!(r.level, ReplayLevel::NotEstablished { .. }),
            "{what}"
        );
    }
    // the B attack: + p1*p2 * I(Z) is invisible modulo [p1, p2]
    let mut atk = f.bad.clone();
    let rhs = atk["rhs"].as_array_mut().unwrap();
    let last = rhs.len() - 1;
    rhs[last][1] = json!(format!("{}", u128::from(pp[0]) * u128::from(pp[1])));
    let atk = RowsRule::from_value(atk, n).unwrap();
    let o = RowsOptions {
        primes: Some(pp),
        ..base.clone()
    };
    let r = replay_evidence(&f.fam, &atk, keys, &ReplayPolicy::default(), &o);
    println!(
        "B attack with caller primes: identity {:?} level {:?}",
        r.identity.verdict, r.level
    );
    assert_eq!(
        r.identity.verdict,
        Verdict::Pass,
        "the attack is invisible at these primes"
    );
    assert!(matches!(r.level, ReplayLevel::NotEstablished { .. }));
    // with run-time primes the extra term is the planted boundary-face term (nonzero): caught on the face keys
    let r = replay_evidence(&f.fam, &atk, &served(&f), &ReplayPolicy::default(), &base);
    assert!(
        matches!(r.identity.verdict, Verdict::Fail { .. }),
        "{:?}",
        r.identity.verdict
    );
}
