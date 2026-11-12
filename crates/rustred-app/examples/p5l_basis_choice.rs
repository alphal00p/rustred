//! p5l-maps-rs: per-node ISP-basis choice for the custom 5L-campaign nodes by
//! the exact static term count of their inbound DAG edges (notes/m5-maps.md
//! section 5, step 1).
//!
//! usage: p5l_basis_choice <dag.json> <out.json> [--threads N] [--budget-s S]
//!
//! `dag.json` is written by `tools/p5l-maps/prep_dag.py`: every node's
//! presentation (lines and ISPs over `(k_1..k_L, q)`, the campaign's family
//! files) and, for every custom node C, its inbound edges with one exact
//! substitution `X` per isomorphism of the operated parent onto C.
//!
//! Score of a basis of C (its lines, its ISPs, `q^2`): over the inbound edges,
//! the best alternative (automorphism) of
//! `Sum_slots [terms(F) + terms(F^2)]`, then Forcer's `Sum_slots terms(F)^2`,
//! `F` a parent numerator slot (its ISPs; on a contraction edge also the
//! removed line) carried into C. Contraction: `F(X)` exactly, `terms(F^2) =
//! n (n + 1) / 2`. Insertion/carpet: the integrated symbol `w` (bubble loop
//! `k_0`, or `q` of the carpet) is replaced by `u` (the merged line's momentum,
//! or the child's `q`): `F = a w.w + w.X + Y -> {a u.u, u.X, Y}` and `F^2` adds
//! `u.u X.X` (tensor reduction), terms = distinct monomials of the union, the
//! coefficients being generic in `d`. Nodes are processed top-down (by loops
//! and lines, descending), so a custom parent contributes its chosen ISPs.
//!
//! Search: first-improvement hill climbing over single ISP swaps from the
//! current ISPs, pool = Forcer edge-pair products `p_a.p_b` (lines and `q`),
//! the current and the family-file square ISPs, and the unit products of the
//! node's routing. Coordinates are found modulo the prime 2^61 - 1 during the
//! search and recomputed exactly (`FormBasis`, i128) for the current and the
//! chosen bases; a disagreement is reported, never hidden.
use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

use rayon::prelude::*;
use rustred::propagator::route_choice::{FormBasis, bilinear_form, pair_count, pair_index};
use serde_json::{Value, json};

const P: u64 = (1 << 61) - 1;

fn mulmod(a: u64, b: u64) -> u64 {
    ((u128::from(a) * u128::from(b)) % u128::from(P)) as u64
}

fn to_mod(x: i128) -> u64 {
    x.rem_euclid(i128::from(P)) as u64
}

fn powmod(mut a: u64, mut e: u64) -> u64 {
    let mut r = 1;
    while e > 0 {
        if e & 1 == 1 {
            r = mulmod(r, a);
        }
        a = mulmod(a, a);
        e >>= 1;
    }
    r
}

/// Inverse modulo P of a square matrix; None if singular mod P.
fn inverse_mod(m: &[Vec<u64>]) -> Option<Vec<u64>> {
    let n = m.len();
    let mut a: Vec<Vec<u64>> = m
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut r = r.clone();
            r.extend((0..n).map(|j| u64::from(i == j)));
            r
        })
        .collect();
    for c in 0..n {
        let p = (c..n).find(|&r| a[r][c] != 0)?;
        a.swap(c, p);
        let inv = powmod(a[c][c], P - 2);
        for x in &mut a[c] {
            *x = mulmod(*x, inv);
        }
        let pivot = a[c].clone();
        for (r, row) in a.iter_mut().enumerate() {
            if r == c || row[c] == 0 {
                continue;
            }
            let f = row[c];
            for (x, y) in row.iter_mut().zip(&pivot) {
                *x = (*x + P - mulmod(f, *y)) % P;
            }
        }
    }
    Some(a.into_iter().flat_map(|r| r[n..].to_vec()).collect())
}

type Rows = (Vec<i64>, Vec<i64>);

fn ints(v: &Value) -> Vec<i64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap())
        .collect()
}

#[derive(Clone, Debug)]
struct Node {
    label: String,
    action: String,
    loops: usize,
    lines: Vec<Vec<i64>>,
    isps: Vec<Rows>,
}

#[derive(Clone, Debug)]
struct Alt {
    x: Vec<Vec<i64>>,
    u: Option<Vec<i64>>,
}

#[derive(Clone, Debug)]
struct Edge {
    parent: String,
    kind: String,
    removed: Value,
    alts: Vec<Alt>,
}

fn row_mul(r: &[i64], x: &[Vec<i64>]) -> Vec<i64> {
    (0..x[0].len())
        .map(|j| r.iter().zip(x).map(|(a, row)| a * row[j]).sum())
        .collect()
}

/// The images of one parent slot through one alternative: pair-vector parts.
#[derive(Clone, Debug)]
enum SlotImage {
    /// Contraction: F exactly.
    Exact(Vec<i128>),
    /// Integration: parts A (a u.u), B (u.X), Y, U (u.u), XX (X.X).
    Integrated {
        a: Option<Vec<i128>>,
        b: Vec<i128>,
        y: Vec<i128>,
        uu: Vec<i128>,
        xx: Vec<i128>,
    },
}

fn slot_image(slot: &Rows, alt: &Alt) -> SlotImage {
    let l = row_mul(&slot.0, &alt.x);
    let r = row_mul(&slot.1, &alt.x);
    match &alt.u {
        None => SlotImage::Exact(bilinear_form(&l, &r)),
        Some(u) => {
            let (lw, rw) = (*l.last().unwrap(), *r.last().unwrap());
            let (lr, rr) = (&l[..l.len() - 1], &r[..r.len() - 1]);
            let x: Vec<i64> = lr.iter().zip(rr).map(|(a, b)| lw * b + rw * a).collect();
            let alpha = lw * rw;
            let uu = bilinear_form(u, u);
            SlotImage::Integrated {
                a: (alpha != 0).then(|| uu.clone()),
                b: bilinear_form(u, &x),
                y: bilinear_form(lr, rr),
                uu,
                xx: bilinear_form(&x, &x),
            }
        }
    }
}

/// Support of a form's coordinates: coordinate indices, the q^2 one mapped to
/// `CONST`.
const CONST: u16 = u16::MAX;

trait Coords {
    fn support(&self, v: &[i128]) -> Vec<u16>;
}

struct ModBasis {
    size: usize,
    inverse: Vec<u64>,
}

impl Coords for ModBasis {
    fn support(&self, v: &[i128]) -> Vec<u16> {
        let n = self.size;
        let vm: Vec<(usize, u64)> = v
            .iter()
            .enumerate()
            .filter(|(_, x)| **x != 0)
            .map(|(i, x)| (i, to_mod(*x)))
            .collect();
        let mut out = Vec::new();
        for j in 0..n {
            let mut acc = 0_u64;
            for &(i, x) in &vm {
                acc = (acc + mulmod(x, self.inverse[i * n + j])) % P;
            }
            if acc != 0 {
                out.push(if j == n - 1 { CONST } else { j as u16 });
            }
        }
        out
    }
}

struct ExactBasis(FormBasis, usize);

impl Coords for ExactBasis {
    fn support(&self, v: &[i128]) -> Vec<u16> {
        // FormBasis counts; for supports recompute through unit probes
        let n = self.1;
        let _ = &self.0;
        let mut out = Vec::new();
        for j in 0..n {
            if self.0.coordinate(v, j).expect("exact coordinate") != 0 {
                out.push(if j == n - 1 { CONST } else { j as u16 });
            }
        }
        out
    }
}

fn product_count(pairs: &[(&[u16], &[u16])]) -> u64 {
    let mut seen = std::collections::HashSet::new();
    for (s1, s2) in pairs {
        for &a in *s1 {
            for &b in *s2 {
                let key = if a <= b { (a, b) } else { (b, a) };
                seen.insert(key);
            }
        }
    }
    seen.len() as u64
}

fn union_count(sets: &[&[u16]]) -> u64 {
    let mut all: Vec<u16> = sets.iter().flat_map(|s| s.iter().copied()).collect();
    all.sort_unstable();
    all.dedup();
    all.len() as u64
}

/// (score, proxy) of one slot image under a basis.
fn slot_score(
    image: &SlotImage,
    basis: &dyn Coords,
    cache: &mut HashMap<Vec<i128>, Vec<u16>>,
) -> (u64, u64) {
    let mut sup = |v: &Vec<i128>| -> Vec<u16> {
        if let Some(s) = cache.get(v) {
            return s.clone();
        }
        let s = basis.support(v);
        cache.insert(v.clone(), s.clone());
        s
    };
    match image {
        SlotImage::Exact(f) => {
            let n = sup(f).len() as u64;
            (n + n * (n + 1) / 2, n * n)
        }
        SlotImage::Integrated { a, b, y, uu, xx } => {
            let sa = a.as_ref().map(&mut sup).unwrap_or_default();
            let sb = sup(b);
            let sy = sup(y);
            let su = sup(uu);
            let sxx = sup(xx);
            let n1 = union_count(&[&sa, &sb, &sy]);
            let n2 = product_count(&[
                (&sa, &sa),
                (&sa, &sb),
                (&sa, &sy),
                (&sb, &sb),
                (&sb, &sy),
                (&sy, &sy),
                (&su, &sxx),
            ]);
            (n1 + n2, n1 * n1)
        }
    }
}

/// Inbound images of a node: per edge, per alternative, the slot images.
type Images = Vec<Vec<Vec<SlotImage>>>;

fn node_score(images: &Images, basis: &dyn Coords) -> (u64, u64, Vec<(u64, u64)>) {
    let mut cache = HashMap::new();
    let mut total = (0_u64, 0_u64);
    let mut per_edge = Vec::with_capacity(images.len());
    for edge in images {
        let best = edge
            .iter()
            .map(|alt| {
                alt.iter().fold((0, 0), |acc, image| {
                    let s = slot_score(image, basis, &mut cache);
                    (acc.0 + s.0, acc.1 + s.1)
                })
            })
            .min()
            .unwrap_or((0, 0));
        total = (total.0 + best.0, total.1 + best.1);
        per_edge.push(best);
    }
    (total.0, total.1, per_edge)
}

fn forms_of(node: &Node, isps: &[Rows]) -> Vec<Vec<i128>> {
    node.lines
        .iter()
        .map(|m| bilinear_form(m, m))
        .chain(isps.iter().map(|(l, r)| bilinear_form(l, r)))
        .collect()
}

fn mod_basis(forms: &[Vec<i128>], width: usize) -> Option<ModBasis> {
    let size = pair_count(width);
    let mut rows: Vec<Vec<u64>> = forms
        .iter()
        .map(|f| f.iter().map(|&x| to_mod(x)).collect())
        .collect();
    let mut q2 = vec![0; size];
    q2[pair_index(width, width - 1, width - 1)] = 1;
    rows.push(q2);
    if rows.len() != size {
        return None;
    }
    inverse_mod(&rows).map(|inverse| ModBasis { size, inverse })
}

/// The candidate pool of a node, with labels (D indices are 1-based slots of
/// the family file).
fn pool(node: &Node) -> Vec<(Rows, String)> {
    let l = node.loops;
    let unit = |i: usize| -> Vec<i64> { (0..=l).map(|k| i64::from(k == i)).collect() };
    let q = unit(l);
    let mut out: Vec<(Rows, String)> = Vec::new();
    for a in 0..node.lines.len() {
        for b in a + 1..node.lines.len() {
            out.push((
                (node.lines[a].clone(), node.lines[b].clone()),
                format!("D{}.D{}", a + 1, b + 1),
            ));
        }
        out.push(((node.lines[a].clone(), q.clone()), format!("D{}.q", a + 1)));
    }
    for (i, (lr, rr)) in node.isps.iter().enumerate() {
        out.push(((lr.clone(), rr.clone()), format!("cur{}", i + 1)));
    }
    let name = |v: &[i64]| -> String {
        let names: Vec<String> = (1..=l)
            .map(|i| format!("k{i}"))
            .chain(["q".into()])
            .collect();
        let mut s = String::new();
        for (c, n) in v.iter().zip(&names) {
            match *c {
                0 => {}
                1 => s.push_str(&format!("+{n}")),
                -1 => s.push_str(&format!("-{n}")),
                c => s.push_str(&format!("{c:+}*{n}")),
            }
        }
        s.trim_start_matches('+').to_owned()
    };
    let mut squares: Vec<Vec<i64>> = Vec::new();
    for i in 0..l {
        let mut v = unit(i);
        squares.push(v.clone());
        v[l] = 1;
        squares.push(v);
    }
    for i in 0..l {
        for j in i + 1..l {
            for (sj, sq) in [(-1, 0), (1, 0), (-1, 1)] {
                let mut v = vec![0; l + 1];
                v[i] = 1;
                v[j] = sj;
                v[l] = sq;
                squares.push(v);
            }
        }
    }
    for v in squares {
        out.push(((v.clone(), v.clone()), format!("({})^2", name(&v))));
    }
    for i in 0..l {
        for j in i..=l {
            out.push((
                (unit(i), unit(j)),
                format!("{}.{}", name(&unit(i)), name(&unit(j))),
            ));
        }
    }
    // dedupe by form up to sign
    let mut seen = std::collections::HashSet::new();
    out.retain(|((lr, rr), _)| {
        let f = bilinear_form(lr, rr);
        let neg: Vec<i128> = f.iter().map(|x| -x).collect();
        if f.iter().all(|x| *x == 0) || seen.contains(&neg) {
            return false;
        }
        seen.insert(f)
    });
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let usage = "usage: p5l_basis_choice <dag.json> <out.json> [--threads N] [--budget-s S]";
    assert!(args.len() >= 3, "{usage}");
    let (mut threads, mut budget) = (1_usize, 300.0_f64);
    let mut rest = args[3..].iter();
    while let Some(a) = rest.next() {
        match a.as_str() {
            "--threads" => threads = rest.next().expect(usage).parse().unwrap(),
            "--budget-s" => budget = rest.next().expect(usage).parse().unwrap(),
            other => panic!("unknown argument {other}; {usage}"),
        }
    }
    let pool_threads = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();
    let t0 = Instant::now();
    let dag: Value = serde_json::from_str(&std::fs::read_to_string(&args[1]).unwrap()).unwrap();
    let mut nodes: BTreeMap<String, Node> = BTreeMap::new();
    for (key, n) in dag["nodes"].as_object().unwrap() {
        nodes.insert(
            key.clone(),
            Node {
                label: n["label"].as_str().unwrap().to_owned(),
                action: n["action"].as_str().unwrap().to_owned(),
                loops: n["L"].as_u64().unwrap() as usize,
                lines: n["lines"].as_array().unwrap().iter().map(ints).collect(),
                isps: n["isps"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| (ints(&p[0]), ints(&p[1])))
                    .collect(),
            },
        );
    }
    let mut inbound: BTreeMap<String, Vec<Edge>> = BTreeMap::new();
    for (key, es) in dag["inbound"].as_object().unwrap() {
        let edges = es
            .as_array()
            .unwrap()
            .iter()
            .map(|e| Edge {
                parent: e["parent"].as_str().unwrap().to_owned(),
                kind: e["kind"].as_str().unwrap().to_owned(),
                removed: e["removed"].clone(),
                alts: e["alts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|a| Alt {
                        x: a["X"].as_array().unwrap().iter().map(ints).collect(),
                        u: a.get("u").map(ints),
                    })
                    .collect(),
            })
            .collect();
        inbound.insert(key.clone(), edges);
    }
    // top-down levels: (loops, lines) descending
    let mut levels: BTreeMap<(std::cmp::Reverse<usize>, std::cmp::Reverse<usize>), Vec<String>> =
        BTreeMap::new();
    for key in inbound.keys() {
        let n = &nodes[key];
        levels
            .entry((std::cmp::Reverse(n.loops), std::cmp::Reverse(n.lines.len())))
            .or_default()
            .push(key.clone());
    }
    let mut chosen: BTreeMap<String, Vec<Rows>> = BTreeMap::new();
    let mut results: Vec<Value> = Vec::new();
    for (_, keys) in levels {
        let level: Vec<(String, Vec<Rows>, Value)> = pool_threads.install(|| {
            keys.par_iter()
                .map(|key| {
                    let t = Instant::now();
                    let node = &nodes[key];
                    let width = node.loops + 1;
                    // parent slots: chosen ISPs of custom parents (decided at a higher level), else current
                    let mut images: Images = Vec::new();
                    let mut edge_info = Vec::new();
                    for e in &inbound[key] {
                        let parent = &nodes[&e.parent];
                        let mut slots: Vec<Rows> =
                            chosen.get(&e.parent).cloned().unwrap_or_else(|| parent.isps.clone());
                        if e.kind == "contract" {
                            let i = e.removed.as_u64().unwrap() as usize;
                            slots.push((parent.lines[i].clone(), parent.lines[i].clone()));
                        }
                        images.push(
                            e.alts
                                .iter()
                                .map(|alt| slots.iter().map(|s| slot_image(s, alt)).collect())
                                .collect(),
                        );
                        edge_info.push(json!({
                            "parent": parent.label.clone(), "parent_action": parent.action.clone(),
                            "kind": e.kind.clone(), "alternatives": e.alts.len(),
                            "parent_custom_chosen": chosen.contains_key(&e.parent),
                        }));
                    }
                    let cand = pool(node);
                    let current: Vec<Rows> = node.isps.clone();
                    let score_of = |isps: &[Rows]| -> Option<(u64, u64)> {
                        let b = mod_basis(&forms_of(node, isps), width)?;
                        let (s, p, _) = node_score(&images, &b);
                        Some((s, p))
                    };
                    let start = score_of(&current).expect("the current presentation is a basis");
                    let (mut best, mut cur, mut evaluations, mut sweeps) = (start, current.clone(), 0_u64, 0_u64);
                    let mut labels: Vec<String> = (1..=cur.len()).map(|i| format!("cur{i}")).collect();
                    let mut timed_out = false;
                    if !images.is_empty() {
                        let mut improved = true;
                        while improved && !timed_out {
                            improved = false;
                            sweeps += 1;
                            for pos in 0..cur.len() {
                                for (rows, name) in &cand {
                                    let f = bilinear_form(&rows.0, &rows.1);
                                    if cur.iter().any(|(l, r)| {
                                        let g = bilinear_form(l, r);
                                        g == f || g.iter().zip(&f).all(|(a, b)| *a == -*b)
                                    }) {
                                        continue;
                                    }
                                    let mut trial = cur.clone();
                                    trial[pos] = rows.clone();
                                    evaluations += 1;
                                    if let Some(s) = score_of(&trial)
                                        && s < best
                                    {
                                        best = s;
                                        cur = trial;
                                        labels[pos] = name.clone();
                                        improved = true;
                                    }
                                    if t.elapsed().as_secs_f64() > budget {
                                        timed_out = true;
                                        break;
                                    }
                                }
                                if timed_out {
                                    break;
                                }
                            }
                        }
                    }
                    // exact recount of the current and the chosen bases
                    let exact = |isps: &[Rows]| -> (u64, u64, Vec<(u64, u64)>) {
                        let fb = FormBasis::new(&forms_of(node, isps), width)
                            .expect("exact basis")
                            .expect("a basis");
                        node_score(&images, &ExactBasis(fb, pair_count(width)))
                    };
                    let (e_cur, e_cur_p, cur_edges) = exact(&current);
                    let (e_new, e_new_p, new_edges) = exact(&cur);
                    let agree = (e_cur, e_cur_p) == start && (e_new, e_new_p) == best;
                    let record = json!({
                        "key": key, "label": node.label, "loops": node.loops, "lines": node.lines.len(),
                        "isps": cur.len(), "inbound_edges": inbound[key].len(), "edges": edge_info,
                        "score_current": [e_cur, e_cur_p], "score_chosen": [e_new, e_new_p],
                        "per_edge_current": cur_edges, "per_edge_chosen": new_edges,
                        "chosen_isps": labels,
                        "chosen_rows": cur.iter().map(|(l, r)| json!([l, r])).collect::<Vec<_>>(),
                        "current_rows": current.iter().map(|(l, r)| json!([l, r])).collect::<Vec<_>>(),
                        "pool": cand.len(), "evaluations": evaluations, "sweeps": sweeps,
                        "timed_out": timed_out, "modular_agrees_with_exact": agree,
                        "seconds": t.elapsed().as_secs_f64(),
                    });
                    (key.clone(), cur, record)
                })
                .collect()
        });
        for (key, isps, record) in level {
            chosen.insert(key, isps);
            results.push(record);
        }
    }
    let out = json!({"nodes": results, "seconds": t0.elapsed().as_secs_f64(), "threads": threads, "budget_s": budget});
    std::fs::write(&args[2], serde_json::to_string_pretty(&out).unwrap()).unwrap();
    println!(
        "{{\"nodes\": {}, \"seconds\": {:.1}}}",
        out["nodes"].as_array().unwrap().len(),
        t0.elapsed().as_secs_f64()
    );
}
