//! Route-witness rewrite (fable_5_1 next push, input change I2).
//!
//! Every saved route maps a source sector onto an owner through the composed
//! loop map `T = source_to_representative * inverse(owner_to_representative)`
//! (routed_campaign/prepare.rs): source line j becomes the owner-coordinate
//! momentum `v_j T`. Composing T with any loop automorphism M of the owner
//! sector (a unimodular map that permutes the owner's active lines up to sign)
//! gives another valid witness `T M`. The route over-cover can pinch exactly
//! the owner active lines that occur in the expansion of some source
//! irreducible numerator; this tool picks, per route, the automorphism that
//! minimises that cancellation support (then the number of support terms),
//! preferring the saved witness on ties, and rewrites
//! `source_to_representative := (T M) owner_to_representative`.
//!
//! Exact linear algebra (inverses, products, determinants, the basis change
//! to the 15 line squares) uses Symbolica's `Matrix` over Q; the combinatorial
//! search then evaluates integer images with the Symbolica-derived integer
//! coefficient vectors. Nothing here is a proof: every rewritten route is
//! re-verified at load by `symmetry::verify` and `integral_transport::compile`.
//!
//! Usage: route_witness_rewrite SELECTION.json MOMENTA_MANIFEST.json
//!          TRAFFIC.tsv OUT_SELECTION.json OUT_REPORT.tsv OUT_SUMMARY.json
//!   TRAFFIC.tsv: `source_mask<TAB>weight` (header line skipped); routes
//!   without a row get weight 0.
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use symbolica::domains::rational::RationalField;
use symbolica::prelude::{Matrix, Q, Rational};

type M = Matrix<RationalField>;

fn rational_parts(r: &Rational) -> (i128, i128) {
    let text = r.to_string();
    match text.split_once('/') {
        Some((n, d)) => (n.parse().unwrap(), d.parse().unwrap()),
        None => (text.parse().unwrap(), 1),
    }
}

fn from_ints(rows: &[Vec<i64>]) -> M {
    Matrix::from_nested_vec(
        rows.iter()
            .map(|r| r.iter().map(|&x| Rational::from(x)).collect())
            .collect(),
        Q,
    )
    .expect("matrix shape")
}

fn to_ints(m: &M) -> Option<Vec<Vec<i64>>> {
    let (rows, cols) = (m.nrows(), m.ncols());
    let mut out = vec![vec![0i64; cols]; rows];
    for i in 0..rows {
        for j in 0..cols {
            let (n, d) = rational_parts(&m[(i as u32, j as u32)]);
            if d != 1 {
                return None;
            }
            out[i][j] = i64::try_from(n).ok()?;
        }
    }
    Some(out)
}

fn parse_momentum(text: &str, loops: usize) -> Vec<i64> {
    let mut v = vec![0i64; loops];
    let s: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let sign = match bytes[i] {
            b'+' => {
                i += 1;
                1
            }
            b'-' => {
                i += 1;
                -1
            }
            _ => 1,
        };
        assert_eq!(bytes[i], b'k', "momentum {text}");
        i += 1;
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let k: usize = s[start..i].parse().unwrap();
        v[k - 1] += sign;
    }
    v
}

fn row_times(v: &[i64], m: &[Vec<i64>]) -> Vec<i64> {
    let n = m[0].len();
    (0..n)
        .map(|j| v.iter().zip(m).map(|(a, row)| a * row[j]).sum())
        .collect()
}

fn mat_mul(a: &[Vec<i64>], b: &[Vec<i64>]) -> Vec<Vec<i64>> {
    a.iter().map(|row| row_times(row, b)).collect()
}

/// Monomial coordinates of (q . l)^2 over the loop momenta: index of (a, b), a <= b.
fn quad(q: &[i64]) -> Vec<i64> {
    let n = q.len();
    let mut out = Vec::with_capacity(n * (n + 1) / 2);
    for a in 0..n {
        for b in a..n {
            out.push(if a == b { q[a] * q[a] } else { 2 * q[a] * q[b] });
        }
    }
    out
}

struct Owner {
    /// Loop automorphisms as integer matrices (identity first).
    auts: Vec<Vec<Vec<i64>>>,
    complete: bool,
}

/// All loop maps M (integer, det +-1) with v_i M = +-v_pi(i) on the active lines.
fn automorphisms(slots: &[Vec<i64>], active: &[usize], loops: usize) -> (Vec<Vec<Vec<i64>>>, bool) {
    let identity: Vec<Vec<i64>> = (0..loops)
        .map(|i| (0..loops).map(|j| (i == j) as i64).collect())
        .collect();
    // greedy basis of active lines (Symbolica rank)
    let mut basis: Vec<usize> = Vec::new();
    for &i in active {
        let mut rows: Vec<Vec<i64>> = basis.iter().map(|&b| slots[b].clone()).collect();
        rows.push(slots[i].clone());
        if from_ints(&rows).rank() == rows.len() {
            basis.push(i);
        }
        if basis.len() == loops {
            break;
        }
    }
    if basis.len() < loops {
        return (vec![identity], false);
    }
    let b_rows: Vec<Vec<i64>> = basis.iter().map(|&b| slots[b].clone()).collect();
    let binv = from_ints(&b_rows).inv().expect("basis invertible");
    // c_i = v_i Binv as integer numerators over a common denominator per line
    let mut coef: Vec<(Vec<i128>, i128, usize)> = Vec::new(); // (numerators, denominator, last nonzero basis index)
    for &i in active {
        let ci = &from_ints(&[slots[i].clone()]) * &binv;
        let parts: Vec<(i128, i128)> = (0..loops as u32)
            .map(|j| rational_parts(&ci[(0, j)]))
            .collect();
        let den = parts.iter().fold(1i128, |acc, &(_, d)| lcm(acc, d));
        let nums: Vec<i128> = parts.iter().map(|&(n, d)| n * (den / d)).collect();
        let last = nums.iter().rposition(|&x| x != 0).unwrap_or(0);
        coef.push((nums, den, last));
    }
    let mut lookup: HashMap<Vec<i64>, usize> = HashMap::new();
    for &i in active {
        lookup.insert(slots[i].clone(), i);
        lookup.insert(slots[i].iter().map(|x| -x).collect(), i);
    }
    let mut found: Vec<Vec<Vec<i64>>> = vec![identity.clone()];
    let mut assign: Vec<(usize, i64)> = Vec::new();
    fn image(
        nums: &[i128],
        den: i128,
        assign: &[(usize, i64)],
        slots: &[Vec<i64>],
        loops: usize,
    ) -> Option<Vec<i64>> {
        let mut acc = vec![0i128; loops];
        for (m, &(j, s)) in assign.iter().enumerate() {
            if nums[m] == 0 {
                continue;
            }
            for c in 0..loops {
                acc[c] += nums[m] * s as i128 * slots[j][c] as i128;
            }
        }
        acc.iter()
            .map(|&x| {
                if x % den == 0 {
                    i64::try_from(x / den).ok()
                } else {
                    None
                }
            })
            .collect()
    }
    #[allow(clippy::too_many_arguments)]
    fn search(
        k: usize,
        loops: usize,
        active: &[usize],
        slots: &[Vec<i64>],
        coef: &[(Vec<i128>, i128, usize)],
        lookup: &HashMap<Vec<i64>, usize>,
        binv: &M,
        assign: &mut Vec<(usize, i64)>,
        found: &mut Vec<Vec<Vec<i64>>>,
        identity: &[Vec<i64>],
    ) {
        if k == loops {
            let target: Vec<Vec<i64>> = assign
                .iter()
                .map(|&(j, s)| slots[j].iter().map(|x| s * x).collect())
                .collect();
            let m = binv * &from_ints(&target);
            let Some(mi) = to_ints(&m) else { return };
            let det = m.det().expect("square");
            let (dn, dd) = rational_parts(&det);
            if dd != 1 || dn.abs() != 1 {
                return;
            }
            let mut seen = vec![false; slots.len()];
            for &i in active {
                let img = row_times(&slots[i], &mi);
                match lookup.get(&img) {
                    Some(&l) if !seen[l] => seen[l] = true,
                    _ => return,
                }
            }
            if mi != identity {
                found.push(mi);
            }
            return;
        }
        for &j in active {
            if assign.iter().any(|a| a.0 == j) {
                continue;
            }
            for s in [1i64, -1] {
                assign.push((j, s));
                let ok = coef.iter().all(|(nums, den, last)| {
                    if *last != k {
                        return true;
                    }
                    image(nums, *den, assign, slots, loops)
                        .is_some_and(|img| lookup.contains_key(&img))
                });
                if ok {
                    search(
                        k + 1,
                        loops,
                        active,
                        slots,
                        coef,
                        lookup,
                        binv,
                        assign,
                        found,
                        identity,
                    );
                }
                assign.pop();
            }
        }
    }
    search(
        0,
        loops,
        active,
        slots,
        &coef,
        &lookup,
        &binv,
        &mut assign,
        &mut found,
        &identity,
    );
    (found, true)
}

fn gcd(a: i128, b: i128) -> i128 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}
fn lcm(a: i128, b: i128) -> i128 {
    a / gcd(a, b) * b
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let selection_text = fs::read_to_string(&args[1]).unwrap();
    let mut selection: Value = serde_json::from_str(&selection_text).unwrap();
    let manifest: Value = serde_json::from_str(&fs::read_to_string(&args[2]).unwrap()).unwrap();
    let traffic: HashMap<String, f64> = fs::read_to_string(&args[3])
        .unwrap()
        .lines()
        .skip(1)
        .filter_map(|l| {
            l.split_once('\t')
                .map(|(m, w)| (m.to_owned(), w.trim().parse().unwrap()))
        })
        .collect();
    let loops = manifest["loop_count"].as_u64().unwrap() as usize;
    let mut momenta: Vec<(u64, Vec<i64>)> = manifest["momenta"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["index_one_based"].as_u64().unwrap(),
                parse_momentum(m["momentum"].as_str().unwrap(), loops),
            )
        })
        .collect();
    momenta.sort();
    let slots: Vec<Vec<i64>> = momenta.into_iter().map(|(_, v)| v).collect();
    let n = slots.len();
    // basis change: x Q = quad(w) with Q rows = quad(slot_i); integer Qs = D Q^{-1}
    let q_rows: Vec<Vec<i64>> = slots.iter().map(|s| quad(s)).collect();
    assert_eq!(
        q_rows[0].len(),
        n,
        "line squares must form a basis of the scalar products"
    );
    let qinv = from_ints(&q_rows).inv().expect("line squares are a basis");
    let mut den = 1i128;
    for i in 0..n as u32 {
        for j in 0..n as u32 {
            den = lcm(den, rational_parts(&qinv[(i, j)]).1);
        }
    }
    let qs: Vec<Vec<i128>> = (0..n as u32)
        .map(|i| {
            (0..n as u32)
                .map(|j| {
                    let (a, b) = rational_parts(&qinv[(i, j)]);
                    a * (den / b)
                })
                .collect()
        })
        .collect();
    let expand = |w: &[i64]| -> Vec<i128> {
        let y = quad(w);
        (0..n)
            .map(|j| y.iter().zip(&qs).map(|(&a, row)| a as i128 * row[j]).sum())
            .collect()
    };
    // owners
    let mut owners: BTreeMap<String, Owner> = BTreeMap::new();
    for o in selection["owners"].as_array().unwrap() {
        let mask = o["mask"].as_str().unwrap().to_owned();
        let active: Vec<usize> = mask
            .chars()
            .enumerate()
            .filter(|(_, c)| *c == '1')
            .map(|(i, _)| i)
            .collect();
        let (auts, complete) = automorphisms(&slots, &active, loops);
        eprintln!(
            "owner {mask} t={} automorphisms {} {}",
            active.len(),
            auts.len(),
            if complete {
                ""
            } else {
                "(active span < loops: identity only)"
            }
        );
        owners.insert(mask, Owner { auts, complete });
    }
    let parse_matrix = |v: &Value| -> Vec<Vec<i64>> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|r| {
                r.as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_str().unwrap().parse().unwrap())
                    .collect()
            })
            .collect()
    };
    let mut report = String::from(
        "source_mask\towner_mask\ttraffic\tautomorphisms\tsaved_support\tsaved_terms\tbest_support\tbest_terms\tbest_index\tchanged\n",
    );
    let (mut w_total, mut w_saved, mut w_best) = (0f64, 0f64, 0f64);
    let (mut changed, mut routes_done, mut unweighted_saved, mut unweighted_best) =
        (0usize, 0usize, 0usize, 0usize);
    let routes = selection["initial_frontier_routes"].as_array_mut().unwrap();
    for route in routes.iter_mut() {
        if !route["requires_transport"].as_bool().unwrap() {
            continue;
        }
        let source = route["source_mask"].as_str().unwrap().to_owned();
        let owner_mask = route["owner_mask"].as_str().unwrap().to_owned();
        let owner = &owners[&owner_mask];
        let a_s = parse_matrix(&route["source_to_representative"]);
        let a_o = parse_matrix(&route["owner_to_representative"]);
        let t_route =
            to_ints(&(&from_ints(&a_s) * &from_ints(&a_o).inv().expect("owner map invertible")))
                .expect("composed witness must be integral");
        let src_active: Vec<bool> = source.chars().map(|c| c == '1').collect();
        let own_active: Vec<bool> = owner_mask.chars().map(|c| c == '1').collect();
        let evaluate = |t: &[Vec<i64>]| -> Option<(u32, u32)> {
            let mut support = vec![false; n];
            let mut terms = 0u32;
            for j in 0..n {
                let w = row_times(&slots[j], t);
                if src_active[j] {
                    // must be +- an owner active line
                    let ok = (0..n).any(|i| {
                        own_active[i]
                            && (slots[i] == w || slots[i].iter().zip(&w).all(|(a, b)| *a == -b))
                    });
                    if !ok {
                        return None;
                    }
                } else {
                    let x = expand(&w);
                    for i in 0..n {
                        if own_active[i] && x[i] != 0 {
                            support[i] = true;
                            terms += 1;
                        }
                    }
                }
            }
            Some((support.iter().filter(|&&b| b).count() as u32, terms))
        };
        let saved =
            evaluate(&t_route).expect("saved witness maps source actives onto owner actives");
        let mut best = (saved.0, saved.1, 0usize);
        let mut best_t = t_route.clone();
        for (index, m) in owner.auts.iter().enumerate().skip(1) {
            let t = mat_mul(&t_route, m);
            if let Some((s, terms)) = evaluate(&t) {
                if (s, terms) < (best.0, best.1) {
                    best = (s, terms, index);
                    best_t = t;
                }
            }
        }
        let weight = traffic.get(&source).copied().unwrap_or(0.0);
        w_total += weight;
        w_saved += weight * saved.0 as f64;
        w_best += weight * best.0 as f64;
        unweighted_saved += saved.0 as usize;
        unweighted_best += best.0 as usize;
        routes_done += 1;
        if best.2 != 0 {
            changed += 1;
            let new_as = mat_mul(&best_t, &a_o);
            // exact check with Symbolica: new_as * inv(a_o) == best_t
            let back = to_ints(&(&from_ints(&new_as) * &from_ints(&a_o).inv().unwrap())).unwrap();
            assert_eq!(back, best_t, "rewrite must reproduce the chosen witness");
            route["source_to_representative"] = json!(
                new_as
                    .iter()
                    .map(|r| r.iter().map(|x| x.to_string()).collect::<Vec<_>>())
                    .collect::<Vec<_>>()
            );
        }
        report.push_str(&format!(
            "{source}\t{owner_mask}\t{weight}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            owner.auts.len(),
            saved.0,
            saved.1,
            best.0,
            best.1,
            best.2,
            (best.2 != 0) as u8
        ));
    }
    let incomplete: Vec<&String> = owners
        .iter()
        .filter(|(_, o)| !o.complete)
        .map(|(m, _)| m)
        .collect();
    fs::write(
        &args[4],
        serde_json::to_string_pretty(&selection).unwrap() + "\n",
    )
    .unwrap();
    fs::write(&args[5], report).unwrap();
    let summary = json!({
        "schema": "rustred.route-witness-rewrite.v1",
        "routes_evaluated": routes_done, "routes_changed": changed,
        "traffic_total": w_total,
        "traffic_weighted_support_saved": if w_total > 0.0 { w_saved / w_total } else { 0.0 },
        "traffic_weighted_support_best": if w_total > 0.0 { w_best / w_total } else { 0.0 },
        "mean_support_saved": unweighted_saved as f64 / routes_done.max(1) as f64,
        "mean_support_best": unweighted_best as f64 / routes_done.max(1) as f64,
        "owners_without_full_active_span": incomplete,
        "authority": "candidate witnesses only; native symmetry::verify and integral_transport::compile at load remain the proof",
    });
    fs::write(
        &args[6],
        serde_json::to_string_pretty(&summary).unwrap() + "\n",
    )
    .unwrap();
    println!("{summary}");
}
