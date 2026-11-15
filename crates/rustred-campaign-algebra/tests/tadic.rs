//! lane tadic: the system route-pole order p* (TRICKS 15/16/17). Constructed systems with known answers, the two
//! screen counterexamples, row-order invariance, brute-force and Symbolica check-mode cross-checks, big integers,
//! d0 other than 4, a real IBP system, and the engine row-span equality (item 2). Tests SKIP without the licence.

mod common;

use std::collections::HashSet;

use rustred_campaign_algebra::tadic::check::{brute_force_upper, check_result};
use rustred_campaign_algebra::tadic::lattice::{saturate_rows, smith_exponents};
use rustred_campaign_algebra::tadic::{SymbolicaTadic, screen_at_d0};
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::tadic::{
    Arith, ExactSystem, Interest, PStar, PoleOrderOracle, PoleQuery, Rat, TadicBudget, Verdict,
    build_ibp_system, keeps_regularity, parse_shell, seeds_around,
};

/// Columns are one-slot keys: [0] = K, [1..10) lower, [10..) upper.
fn lower(k: &[i64]) -> bool {
    (1..10).contains(&k[0])
}

/// Affine entry with value `c0 + c1 t` at t = d - d0 (d0 integer): a + b d with b = c1, a = c0 - c1 d0.
fn e(key: i64, c0: i128, c1: i128, d0: i128) -> (Vec<i64>, i128, i128) {
    (vec![key], c0 - c1 * d0, c1)
}

fn q<'a>(target: &'a [i64], d0: Rat, interest: Interest) -> PoleQuery<'a> {
    PoleQuery {
        target,
        lower: &lower,
        d0,
        interest,
        arith: Arith::Exact,
        budget: TadicBudget::default(),
        want_relation: true,
    }
}

fn oracle() -> SymbolicaTadic {
    let mut o = SymbolicaTadic::new().expect("licensed");
    o.relation_orders = 3;
    o
}

/// p* + every cross-check that applies (check mode, brute force); returns the exact p* and the screen-at-d0 flag.
fn solve_and_check(sys: &ExactSystem, d0: Rat, bf: bool) -> (PStar, Option<bool>) {
    let target = [0i64];
    let qq = q(&target, d0, Interest::AllLower);
    let r = oracle().best_pole_order(sys, &qq);
    assert_eq!(r.verdict, Verdict::Exact, "{:?}", r.notes);
    let p = r.p_star.expect("p*");
    let rep = check_result(sys, &qq, &r, 2_000_000).expect("check mode");
    assert!(
        rep.consistent,
        "check mode disagrees: {rep:?} for {p:?} relation {:?}",
        r.relation
    );
    if bf {
        let b = brute_force_upper(sys, &qq, -4, 4, 4);
        match p {
            PStar::Order(x) => assert_eq!(b, Some(x), "brute force {b:?} vs p* {x}"),
            PStar::NoRelation => assert_eq!(b, None),
        }
    }
    (p, r.screen_at_d0)
}

#[test]
fn tadic_saturation_regular_relation_invisible_to_the_screen_at_d0() {
    if !common::licensed_or_skip("tadic_saturation") {
        return;
    }
    // r1 = (u:1, K:1, S1:1), r2 = (u:1+t, K:1, S1:1+t, S2:t): mod t the rows coincide (no admissible relation at d0),
    // yet r2 - (1+t) r1 = (K:-t, S2:t): I(K) = I(S2), regular. TRICKS 16.
    let d0 = 4;
    let sys = ExactSystem::from_rows(
        1,
        &[
            vec![e(10, 1, 0, d0), e(0, 1, 0, d0), e(1, 1, 0, d0)],
            vec![
                e(10, 1, 1, d0),
                e(0, 1, 0, d0),
                e(1, 1, 1, d0),
                e(2, 0, 1, d0),
            ],
        ],
        "constructed",
    );
    let (p, screen) = solve_and_check(&sys, Rat::int(4), true);
    assert_eq!(p, PStar::Order(0));
    assert_eq!(
        screen,
        Some(false),
        "the screen at d0 must NOT see this relation"
    );
}

#[test]
fn tadic_screen_relation_at_d0_that_does_not_lift() {
    if !common::licensed_or_skip("tadic_nolift") {
        return;
    }
    let d0 = 4;
    // (a) r1 = (u:t, K:1), r2 = (u:t, K:1+t, S:1): at d0 r1 says I(K) = 0; exactly: r2 - r1 = (K:t, S:1): p* = 1.
    let sys = ExactSystem::from_rows(
        1,
        &[
            vec![e(10, 0, 1, d0), e(0, 1, 0, d0)],
            vec![e(10, 0, 1, d0), e(0, 1, 1, d0), e(1, 1, 0, d0)],
        ],
        "constructed",
    );
    let (p, screen) = solve_and_check(&sys, Rat::int(4), true);
    assert_eq!(p, PStar::Order(1));
    assert_eq!(
        screen,
        Some(true),
        "the screen finds a relation at d0 although the best relation is singular"
    );
    // (b) r1 = (u:t, K:1), r2 = (u:t, K:1, S:t): no admissible relation at all; the screen still says "exists"
    let sys = ExactSystem::from_rows(
        1,
        &[
            vec![e(10, 0, 1, d0), e(0, 1, 0, d0)],
            vec![e(10, 0, 1, d0), e(0, 1, 0, d0), e(1, 0, 1, d0)],
        ],
        "constructed",
    );
    let (p, screen) = solve_and_check(&sys, Rat::int(4), true);
    assert_eq!(p, PStar::NoRelation);
    assert_eq!(screen, Some(true));
}

#[test]
fn tadic_minimal_relations_singular_larger_one_regular_and_inherent_order_two() {
    if !common::licensed_or_skip("tadic_minimal") {
        return;
    }
    let d0 = 4;
    // r1 = (K:t, S1:1) (the only one-term relation: singular), r2 = (S1:1, S2:t, S3:t): r1 - r2 = I(K) = I(S2)+I(S3)
    let sys = ExactSystem::from_rows(
        1,
        &[
            vec![e(0, 0, 1, d0), e(1, 1, 0, d0)],
            vec![e(1, 1, 0, d0), e(2, 0, 1, d0), e(3, 0, 1, d0)],
        ],
        "constructed",
    );
    let target = [0i64];
    let r = oracle().best_pole_order(&sys, &q(&target, Rat::int(4), Interest::AllLower));
    assert_eq!(r.p_star, Some(PStar::Order(0)));
    let rel = r.relation.expect("relation");
    assert_eq!(rel.profile.max_order, 0);
    assert!(rel.terms.len() >= 2, "{rel:?}");
    solve_and_check(&sys, Rat::int(4), true);
    // inherent in this system: r1 = (u:1, K:t), r2 = (u:t, S:1): t r1 - r2 = t^2 I(K) - I(S): p* = 2
    let sys = ExactSystem::from_rows(
        1,
        &[
            vec![e(10, 1, 0, d0), e(0, 0, 1, d0)],
            vec![e(10, 0, 1, d0), e(1, 1, 0, d0)],
        ],
        "constructed",
    );
    let (p, screen) = solve_and_check(&sys, Rat::int(4), true);
    assert_eq!(p, PStar::Order(2));
    assert_eq!(screen, Some(false));
    // the same system with only S as column of interest vs with no column of interest
    let r = oracle().best_pole_order(&sys, &q(&target, Rat::int(4), Interest::Keys(vec![])));
    assert_eq!(
        r.p_star,
        Some(PStar::Order(0)),
        "poles on non-interest columns do not count"
    );
}

#[test]
fn tadic_index_dependent_pole_and_other_d0() {
    if !common::licensed_or_skip("tadic_index") {
        return;
    }
    // r1 = (u:1, K: d - 2n), r2 = (u:1, S:1): I(K) = I(S)/(d - 2n)
    let sys_n = |a: i128, b: i128| {
        ExactSystem::from_rows(
            1,
            &[
                vec![(vec![10], 1, 0), (vec![0], a, b)],
                vec![(vec![10], 1, 0), (vec![1], 1, 0)],
            ],
            "constructed",
        )
    };
    assert_eq!(
        solve_and_check(&sys_n(-4, 1), Rat::int(4), true).0,
        PStar::Order(1)
    ); // n = 2 at d0 = 4
    assert_eq!(
        solve_and_check(&sys_n(-6, 1), Rat::int(4), true).0,
        PStar::Order(0)
    ); // n = 3 at d0 = 4
    assert_eq!(
        solve_and_check(&sys_n(-6, 1), Rat::int(6), true).0,
        PStar::Order(1)
    ); // n = 3 at d0 = 6
    let half = Rat::new(9, 2).unwrap();
    assert_eq!(
        solve_and_check(&sys_n(-9, 2), half, false).0,
        PStar::Order(1)
    ); // 2d - 9 at d0 = 9/2
    assert_eq!(
        solve_and_check(&sys_n(-9, 2), Rat::int(4), false).0,
        PStar::Order(0)
    );
}

#[test]
fn tadic_big_integer_coefficients() {
    if !common::licensed_or_skip("tadic_bigint") {
        return;
    }
    let big: i128 = 10i128.pow(30) + 7;
    let big2: i128 = 3 * 10i128.pow(29) - 11;
    // r1 = (u: big, K: big2 (d - 4)), r2 = (u: big2, S: big): big2 r1 - big r2 = big2^2 (d-4) I(K) - big^2 I(S): p* = 1
    let sys = ExactSystem::from_rows(
        1,
        &[
            vec![(vec![10], big, 0), (vec![0], -4 * big2, big2)],
            vec![(vec![10], big2, 0), (vec![1], big, 0)],
        ],
        "constructed",
    );
    let (p, _) = solve_and_check(&sys, Rat::int(4), false);
    assert_eq!(p, PStar::Order(1));
}

/// SplitMix64 for reproducible random systems.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn small(&mut self, m: i128) -> i128 {
        (self.next() % (2 * m as u64 + 1)) as i128 - m
    }
}

#[test]
fn tadic_random_systems_vs_brute_force_check_mode_and_row_orders() {
    if !common::licensed_or_skip("tadic_random") {
        return;
    }
    let mut rng = Rng(20261115);
    let mut seen = std::collections::BTreeMap::new();
    let (mut bf_equal, mut bf_above) = (0, 0);
    for case in 0..60 {
        let nrows = 2 + (rng.next() % 3) as usize;
        let cols: Vec<i64> = vec![0, 1, 2, 10, 11];
        let mut rows = Vec::new();
        for _ in 0..nrows {
            let mut r = Vec::new();
            for &c in &cols {
                if rng.next().is_multiple_of(3) {
                    continue;
                }
                // value c0 + c1 t with a bias to vanish at t = 0
                let c0 = if rng.next().is_multiple_of(2) {
                    0
                } else {
                    rng.small(2)
                };
                let c1 = rng.small(2);
                if c0 != 0 || c1 != 0 {
                    r.push(e(c, c0, c1, 4));
                }
            }
            rows.push(r);
        }
        let sys = ExactSystem::from_rows(1, &rows, "random");
        if !sys.columns.contains(&vec![0]) {
            continue;
        }
        let (p, _) = solve_and_check(&sys, Rat::int(4), false);
        *seen.entry(format!("{p:?}")).or_insert(0) += 1;
        // the ansatz is an UPPER bound (Laurent coefficients of bounded degree); equality is expected, counted
        let target0 = [0i64];
        let b = brute_force_upper(
            &sys,
            &q(&target0, Rat::int(4), Interest::AllLower),
            -6,
            8,
            4,
        );
        match (p, b) {
            (PStar::Order(x), Some(y)) => {
                assert!(y >= x, "case {case}: ansatz found order {y} below p* {x}");
                if y == x { bf_equal += 1 } else { bf_above += 1 }
            }
            (PStar::Order(_), None) => bf_above += 1,
            (PStar::NoRelation, Some(y)) => {
                panic!("case {case}: ansatz found order {y} where p* says no relation")
            }
            (PStar::NoRelation, None) => bf_equal += 1,
        }
        // row-order invariance: reversed rows
        let rev: Vec<_> = rows.iter().rev().cloned().collect();
        let sys2 = ExactSystem::from_rows(1, &rev, "random");
        let target = [0i64];
        let r2 = oracle().best_pole_order(&sys2, &q(&target, Rat::int(4), Interest::AllLower));
        assert_eq!(
            r2.p_star,
            Some(p),
            "case {case}: p* depends on the row order"
        );
    }
    println!(
        "random systems by p*: {seen:?}; brute force equal {bf_equal}, above (ansatz too small) {bf_above}"
    );
    assert!(
        seen.len() >= 3,
        "the random family must exercise several p* values: {seen:?}"
    );
}

#[test]
fn tadic_keeps_regularity_hook() {
    if !common::licensed_or_skip("tadic_keeps") {
        return;
    }
    let d0 = 4;
    // the saturation example plus a third row making a regular relation reachable in another way; dropping r2 loses
    // the regular relation: p* rises from 0 to "no relation"
    let rows = vec![
        vec![e(10, 1, 0, d0), e(0, 1, 0, d0), e(1, 1, 0, d0)],
        vec![
            e(10, 1, 1, d0),
            e(0, 1, 0, d0),
            e(1, 1, 1, d0),
            e(2, 0, 1, d0),
        ],
    ];
    let mut before = ExactSystem::from_rows(1, &rows, "constructed");
    before.seeds = vec![vec![0], vec![1]];
    let after = before.restrict_to_seeds(&[0]);
    let target = [0i64];
    let qq = q(&target, Rat::int(4), Interest::AllLower);
    let o = oracle();
    let v = keeps_regularity(&o, &before, &after, &qq, None);
    assert_eq!(v.bound, Some(PStar::Order(0)));
    assert_eq!(v.kept, Some(false));
    let full = o.best_pole_order(&before, &qq);
    assert_eq!(
        full.relation_seeds,
        Some(vec![0, 1]),
        "the relation combines both seeds"
    );
    let v = keeps_regularity(
        &o,
        &before,
        &before.restrict_to_seeds(&[0, 1]),
        &qq,
        Some(&full),
    );
    assert_eq!(v.kept, Some(true));
    assert!(v.by_trace, "kept by the seed trace, no recomputation");
    let v = keeps_regularity(&o, &before, &after, &qq, Some(&full));
    assert_eq!((v.kept, v.by_trace), (Some(false), false));
}

#[test]
fn tadic_lattice_api_saturation_and_smith() {
    if !common::licensed_or_skip("tadic_lattice") {
        return;
    }
    // rows (1, 1) and (1, 1 + (d-4)): module index t: Smith exponents [0, 1]; saturated basis has rank 2 and is unimodular
    let rows = vec![
        vec![(0u32, "1".to_string()), (1, "1".to_string())],
        vec![(0u32, "1".to_string()), (1, "d-3".to_string())],
    ];
    assert_eq!(smith_exponents(&rows, Rat::int(4)).unwrap(), vec![0, 1]);
    let b = saturate_rows(&rows, Rat::int(4)).unwrap();
    assert_eq!(b.rank, 2);
    assert_eq!(b.saturation_exponent, 1);
    // a pole: row (1/(d-4), 0) and (0, 1): exponents [-1, 0]
    let rows = vec![
        vec![(0u32, "1/(d-4)".to_string())],
        vec![(1u32, "1".to_string())],
    ];
    assert_eq!(smith_exponents(&rows, Rat::int(4)).unwrap(), vec![-1, 0]);
}

fn ws() -> String {
    "/shared/vhirschi/t3code/vakint_dev/p_integral_dev".into()
}

fn recipe_fixtures() -> String {
    std::env::var("RR_RECIPE_FIXTURES").unwrap_or_else(|_| {
        "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/fixtures/recipe-solve"
            .into()
    })
}

/// A real IBP system (T178, recipe-solve fixtures): p* is invariant under row permutations and the check mode agrees.
#[test]
fn tadic_real_ibp_system_row_order_invariance() {
    if !common::licensed_or_skip("tadic_real_small") {
        return;
    }
    let fam_path = format!("{}/T178.sym", recipe_fixtures());
    let Ok(text) = std::fs::read_to_string(&fam_path) else {
        println!("SKIP: {fam_path} absent");
        return;
    };
    let fam = Family::parse(&text).expect("family");
    let n = fam.n();
    let lines: Vec<bool> = (0..n).map(|i| i < 10).collect();
    let key: Vec<i64> = vec![1, 1, 1, 1, 1, 1, 1, 1, 2, 1, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0];
    let shells: Vec<_> = ["S0", "S1"]
        .iter()
        .map(|s| parse_shell(s).unwrap())
        .collect();
    let seeds = seeds_around(&key, &[], &shells, &lines, true);
    let sys = build_ibp_system(&fam, &seeds, true).expect("system");
    // lower = strictly smaller (dots + rank), ties: lexicographic (a generic test order, not the node's)
    let dr = |k: &[i64]| -> i64 {
        (0..n)
            .map(|i| if lines[i] { (k[i] - 1).max(0) } else { -k[i] })
            .sum::<i64>()
            - 100 * (0..n).filter(|&i| lines[i] && k[i] <= 0).count() as i64
    };
    let kk = key.clone();
    let low = move |k: &[i64]| (dr(k), k.to_vec()) < (dr(&kk), kk.clone());
    let mut pstars = HashSet::new();
    for order in 0..3u32 {
        let perm = rustred_campaign_algebra::tadic::row_order(sys.rows.len(), order);
        let mut s2 = sys.clone();
        s2.rows = perm.iter().map(|&i| sys.rows[i].clone()).collect();
        s2.labels = perm.iter().map(|&i| sys.labels[i]).collect();
        let qq = PoleQuery {
            target: &key,
            lower: &low,
            d0: Rat::int(4),
            interest: Interest::AllLower,
            arith: Arith::Exact,
            budget: TadicBudget::default(),
            want_relation: true,
        };
        let r = SymbolicaTadic::new().unwrap().best_pole_order(&s2, &qq);
        assert_eq!(r.verdict, Verdict::Exact);
        println!(
            "T178 S0+S1 order {order}: rows {} cols {} p* {:?} rank {:?} relation terms {:?} screen {:?} {:.2}s",
            sys.rows.len(),
            sys.columns.len(),
            r.p_star,
            r.rank.rank_rows,
            r.relation.as_ref().map(|x| x.terms.len()),
            r.screen_at_d0,
            r.seconds
        );
        if order == 0 {
            let rep = check_result(&s2, &qq, &r, 3_000_000).expect("check");
            assert!(rep.consistent, "{rep:?}");
        }
        pstars.insert(format!("{:?}", r.p_star));
    }
    assert_eq!(
        pstars.len(),
        1,
        "p* must not depend on the row order: {pstars:?}"
    );
}

/// Item 2: the engine's solve-recipe rows vs ours, on the fixture recipe results_13_x8_C (T178, no symmetry rows):
/// seed count equal; the engine's exact relation lies in the span of OUR rows at its seeds_used (mod 2 primes at a
/// random d, screen arithmetic).
#[test]
fn tadic_engine_row_span_equality_on_a_recipe() {
    if !common::licensed_or_skip("tadic_engine_equality") {
        return;
    }
    let dir = recipe_fixtures();
    let Ok(text) = std::fs::read_to_string(format!("{dir}/T178.sym")) else {
        println!("SKIP: fixtures absent");
        return;
    };
    let fam = Family::parse(&text).expect("family");
    let n = fam.n();
    let recipe: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{dir}/recipes/results_13_x8_C_json.json")).unwrap(),
    )
    .unwrap();
    let rec: serde_json::Value = serde_json::from_str(
        std::fs::read_to_string(format!(
            "{dir}/reference/results_13_x8_C_json.results.jsonl"
        ))
        .unwrap()
        .lines()
        .next()
        .unwrap(),
    )
    .unwrap();
    let key: Vec<i64> = serde_json::from_value(rec["key"].clone()).unwrap();
    let chart = recipe["chart"].as_str().unwrap();
    let lines: Vec<bool> = chart.chars().map(|c| c == '1').collect();
    let centres: Vec<Vec<i64>> =
        serde_json::from_value(recipe["seeds"]["centres"].clone()).unwrap();
    let shells: Vec<_> = recipe["seeds"]["generator"]["shells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| parse_shell(s.as_str().unwrap()).unwrap())
        .collect();
    let mut seeds = seeds_around(&key, &centres, &shells, &lines, true);
    if let Some(mo) = recipe["seeds"]["min_offset"].as_object() {
        for (k, v) in mo {
            let j: usize = k.parse().unwrap();
            let v = v.as_i64().unwrap();
            seeds.retain(|s| s[j] - key[j] >= v);
        }
    }
    let total = rec["seeds_total"].as_u64().unwrap() as usize;
    println!("seeds: ours {} engine seeds_total {total}", seeds.len());
    assert_eq!(
        seeds.len(),
        total,
        "seed generation differs from the engine's build_seeds"
    );
    let used: Vec<Vec<i64>> = match &rec["seeds_used"] {
        serde_json::Value::String(t) => serde_json::from_str(t).unwrap(),
        v => serde_json::from_value(v.clone()).unwrap(),
    };
    let used_abs: Vec<Vec<i64>> = used
        .iter()
        .map(|o| (0..n).map(|j| key[j] + o[j]).collect())
        .collect();
    let all_sys = build_ibp_system(&fam, &seeds, true).unwrap();
    println!(
        "rows: ours at all {} seeds {} (engine examined {} rows until its hit; trace {})",
        seeds.len(),
        all_sys.rows.len(),
        rec["rows"],
        rec["trace_rows"]
    );
    let sys = build_ibp_system(&fam, &used_abs, true).unwrap();
    // the engine relation as a row at a random d (mod p): e_K - sum c_j e_j, appended with a fresh column X
    let rhs: Vec<(String, String)> = serde_json::from_value(rec["rhs"].clone()).unwrap();
    let parse_key = |s: &str| -> Vec<i64> {
        s.trim_start_matches("I(")
            .trim_end_matches(')')
            .split(',')
            .map(|x| x.trim().parse().unwrap())
            .collect()
    };
    for (pi, &prime) in rustred_campaign_algebra::tadic::SCREEN_PRIMES
        .iter()
        .enumerate()
    {
        let dval: i128 = 1_000_003 + 7919 * pi as i128;
        let alg = rustred_campaign_algebra::SymbolicaAlgebra::new().unwrap();
        let mut rows: Vec<Vec<(Vec<i64>, i128, i128)>> = sys
            .rows
            .iter()
            .map(|r| {
                r.iter()
                    .map(|&(c, a, b)| (sys.columns[c as usize].clone(), a, b))
                    .collect()
            })
            .collect();
        let mut w: Vec<(Vec<i64>, i128, i128)> = vec![(key.clone(), 1, 0), (vec![999; n], 1, 0)];
        for (k, coef) in &rhs {
            let kk = parse_key(k);
            if fam.zero_key(&kk) {
                continue;
            }
            let (num, den) = alg.parse_ratio(coef, n).expect("coefficient");
            let ev = |p: &rustred_campaign_algebra::tadic::ZP| -> u128 {
                // the coefficient is a polynomial in d only (key put in): evaluate at dval mod prime
                let mut acc: u128 = 0;
                let pr = prime as u128;
                let dv = (dval as u128) % pr;
                let pz = symbolica::domains::integer::Integer::from(prime);
                for i in 0..p.nterms() {
                    let ex = p.exponents(i)[n] as u32;
                    let mut ci = &p.coefficients[i] % &pz;
                    if ci < 0 {
                        ci += &pz;
                    }
                    let mut x: u128 = u64::try_from(ci).expect("residue") as u128;
                    for _ in 0..ex {
                        x = x * dv % pr;
                    }
                    acc = (acc + x) % pr;
                }
                acc
            };
            let (nv, dv) = (ev(&num), ev(&den));
            // c = nv / dv mod p; the row entry is -c
            let inv = modpow(dv as u64, prime - 2, prime);
            let c = (nv * inv as u128 % prime as u128) as u64;
            w.push((kk, ((prime - c) % prime) as i128, 0));
        }
        rows.push(w);
        let s2 = ExactSystem::from_rows(n, &rows, "membership");
        let x = vec![999i64; n];
        let none = |_: &[i64]| false;
        let qq = PoleQuery {
            target: &x,
            lower: &none,
            d0: Rat::int(dval),
            interest: Interest::AllLower,
            arith: Arith::Screen { order: 1, prime },
            budget: TadicBudget::default(),
            want_relation: false,
        };
        let inside = screen_at_d0(&s2, &qq, prime).unwrap();
        println!(
            "prime {prime}: engine relation in span(our rows at seeds_used) at d = {dval}: {inside}"
        );
        assert!(
            inside,
            "the engine's relation is not in the span of our rows (mod {prime})"
        );
    }
}

fn modpow(mut b: u64, mut e: u64, m: u64) -> u64 {
    let mut r: u128 = 1;
    let mut bb = b as u128 % m as u128;
    while e > 0 {
        if e & 1 == 1 {
            r = r * bb % m as u128;
        }
        bb = bb * bb % m as u128;
        e >>= 1;
    }
    b = r as u64;
    b
}

#[allow(dead_code)]
fn _ws_used() -> String {
    ws()
}
