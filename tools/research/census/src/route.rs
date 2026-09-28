//! Route-side coverage census (W0 routecensus lane; handoff §0.1 item 3,
//! critique §2.2). Route domains (phase 1) live in (Route, owner) buckets
//! like Apply domains and are admitted or aliased by the same containment
//! rule, but their natives do no algebra (a conservative box map,
//! `routed/domain_overcover/visit.rs`): their weight is in the successors
//! they emit (admission requests) and in the domains those create.
//!
//! `census route CKPT` measures, read-only:
//! 1. coverage of Route native-pending domains (stratified by admission
//!    generation x points decade) by every Route native at the checkpoint,
//!    by every Route domain with a smaller ID (the well-founded general-union
//!    order) and by every other Route domain of the bucket (upper bound), and
//!    of historical Route natives (stratified by record generation x points
//!    decade) by the natives before dispatch / before commit and by all
//!    earlier IDs; residuals under the `cover` vocabularies; every share by
//!    count, (measured or predicted) seconds, successors, out-edges, created
//!    domains, creator-forest descendants and points;
//! 2. exact edge fan-out of Route (and Apply) natives, the creator forest
//!    (first incoming edge of every domain; subtree sizes), the creator
//!    classes of every domain by state, and on samples of pending Apply and
//!    Route domains the coverage of their Route-native creator (plus the
//!    Apply-side coverage of the pending Apply domain itself).
//!
//! `census route-saturation CKPT` estimates the point-space union of Route
//! natives (by record generation) and of admitted Route domains (by domain
//! segment), as `saturation` does for Apply.
//!
//! Predicted quantities for pending domains are [E]: means over Route natives
//! of the same (owner, rank bound), from record generations >= --model-gen.
use crate::ckpt::{self, L_RESERVED, L_STARTED, L_UNRESERVED};
use crate::compose::cost_law;
use crate::cover::{draw_point, evaluate, pps, uniform, Anchor, Eval, Rng};
use crate::geom::Dom;
use crate::recs;
use crate::util::{self, Ckpt, DomSum, Opts};
use rayon::prelude::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};

const NONE: u32 = u32::MAX;
const CLS: [&str; 3] = ["native", "pending", "delegated"];
const CKIND: [&str; 3] = ["none", "transition", "alias"];

// ----------------------------------------------------------------- edges
/// Exact edge view: creator (first incoming edge) of every domain, transition
/// out-degree and creator out-degree of every inspected source by target
/// phase, and the creator-forest subtree size (including the node).
pub struct Edges {
    pub creator: Vec<u32>,
    /// [target phase][source id]: transition edges (source inspected, s != t).
    pub out: [Vec<u32>; 2],
    /// [target phase][source id]: creator edges.
    pub cre: [Vec<u32>; 2],
    pub subtree: Vec<u32>,
    /// Creator-forest depth (roots 0).
    pub depth: Vec<u16>,
    pub counts: Value,
}

pub fn edges(ck: &Ckpt) -> Edges {
    let nd = ck.doms.len();
    let t0 = std::time::Instant::now();
    let chunk = 1 << 22;
    // Creator = first incoming edge from a smaller ID (a domain is admitted
    // after its creator, so the creator ID is smaller); `first_any` is the
    // first incoming edge of any source, to count targets whose first edge
    // came from a larger ID (initial domains hit later, or order anomalies).
    let first_in: Vec<AtomicU64> = (0..nd).map(|_| AtomicU64::new(u64::MAX)).collect();
    let first_any: Vec<AtomicU64> = (0..nd).map(|_| AtomicU64::new(u64::MAX)).collect();
    ckpt::edges_par(&ck.m, chunk, |base, c| {
        for i in 0..c.len() / 8 {
            let (s, t) = ckpt::edge_at(c, i);
            if s != t {
                first_any[t as usize].fetch_min((base + i) as u64, Relaxed);
            }
            if s < t {
                first_in[t as usize].fetch_min((base + i) as u64, Relaxed);
            }
        }
    });
    eprintln!("route: first-in pass ({:.1} s)", t0.elapsed().as_secs_f64());
    let creator: Vec<AtomicU32> = (0..nd).map(|_| AtomicU32::new(NONE)).collect();
    let mk = || (0..nd).map(|_| AtomicU32::new(0)).collect::<Vec<AtomicU32>>();
    let (oa, or, ca, cr) = (mk(), mk(), mk(), mk());
    // [edges, alias (source not inspected), transition, creator (from inspected),
    //  creator from a non-inspected source, self edges, transition into Route, creator into Route]
    let cnt: Vec<AtomicU64> = (0..9).map(|_| AtomicU64::new(0)).collect();
    ckpt::edges_par(&ck.m, chunk, |base, c| {
        let mut l = [0u64; 9];
        for i in 0..c.len() / 8 {
            let (s, t) = ckpt::edge_at(c, i);
            let (s, t) = (s as usize, t as usize);
            l[0] += 1;
            if s == t {
                l[5] += 1;
                continue;
            }
            let first = s < t && first_in[t].load(Relaxed) == (base + i) as u64;
            l[8] += (s > t && first_any[t].load(Relaxed) == (base + i) as u64) as u64;
            if first {
                creator[t].store(s as u32, Relaxed);
            }
            if ck.nodes[s] & 2 == 0 {
                l[1] += 1;
                l[4] += first as u64;
                continue;
            }
            l[2] += 1;
            let route = ck.doms[t].phase == 1;
            l[6] += route as u64;
            let o = if route { &or[s] } else { &oa[s] };
            o.fetch_add(1, Relaxed);
            if first {
                l[3] += 1;
                l[7] += route as u64;
                let o = if route { &cr[s] } else { &ca[s] };
                o.fetch_add(1, Relaxed);
            }
        }
        for k in 0..9 {
            cnt[k].fetch_add(l[k], Relaxed);
        }
    });
    drop(first_in);
    drop(first_any);
    eprintln!("route: edge pass ({:.1} s)", t0.elapsed().as_secs_f64());
    let un = |v: Vec<AtomicU32>| v.into_iter().map(|a| a.into_inner()).collect::<Vec<u32>>();
    let creator = un(creator);
    let mut subtree = vec![1u32; nd];
    let mut bad = 0usize;
    let mut roots = 0usize;
    for t in (0..nd).rev() {
        let c = creator[t];
        if c == NONE {
            roots += 1;
        } else if (c as usize) < t {
            subtree[c as usize] += subtree[t];
        } else {
            bad += 1;
        }
    }
    let mut depth = vec![0u16; nd];
    for t in 0..nd {
        let c = creator[t];
        if c != NONE && (c as usize) < t {
            depth[t] = depth[c as usize].saturating_add(1);
        }
    }
    let mut dq: Vec<f64> = depth.iter().map(|&d| d as f64).collect();
    let dq = util::quantiles(&mut dq, &[0.5, 0.9, 0.99, 0.999, 1.0]);
    let cnt: Vec<u64> = cnt.iter().map(|a| a.load(Relaxed)).collect();
    let counts = json!({
        "creator_forest_depth_quantiles[0.5,0.9,0.99,0.999,max]": dq,
        "edge_counts[edges, alias(source not inspected), transition, creator, creator_from_non_inspected, self_edges, transition_into_route, creator_into_route, targets_whose_first_edge_is_from_a_larger_id]": cnt,
        "creator_forest_roots(no incoming edge)": roots,
        "creator_not_below_target_id(skipped in subtree sums)": bad,
    });
    Edges { creator, out: [un(oa), un(or)], cre: [un(ca), un(cr)], subtree, depth, counts }
}

// -------------------------------------------------------------- context
struct Ctx<'a> {
    ck: &'a Ckpt,
    n: usize,
    /// Natives per bucket (phase << 16 | owner), sorted by record seq.
    natives: HashMap<u32, Vec<Anchor>>,
    /// All domains per bucket, sorted by id (seq = native record seq or MAX).
    all: HashMap<u32, Vec<Anchor>>,
    series: Option<Vec<(f64, u64)>>,
    wait: f64,
    cap: f64,
    samples: usize,
}

fn pools(ck: &Ckpt, sums: &[DomSum], phases: &[u8]) -> (HashMap<u32, Vec<Anchor>>, HashMap<u32, Vec<Anchor>>) {
    let n = ck.n;
    let mut natives: HashMap<u32, Vec<Anchor>> = HashMap::new();
    for r in &ck.recs {
        if !recs::is_native(r.kind) || !phases.contains(&r.phase) {
            continue;
        }
        let d = &ck.doms[r.id as usize];
        if sums[r.id as usize].empty {
            continue;
        }
        natives.entry(d.bucket()).or_default().push(Anchor::new(r.id, r.seq, d, &sums[r.id as usize], n));
    }
    let mut all: HashMap<u32, Vec<Anchor>> = HashMap::new();
    for (id, d) in ck.doms.iter().enumerate() {
        if !phases.contains(&d.phase) || sums[id].empty {
            continue;
        }
        let ix = ck.native_of[id];
        let seq = if ix == NONE { NONE } else { ck.recs[ix as usize].seq };
        all.entry(d.bucket()).or_default().push(Anchor::new(id as u32, seq, d, &sums[id], n));
    }
    for v in natives.values_mut() {
        v.sort_by_key(|a| a.seq);
    }
    (natives, all)
}

impl Ctx<'_> {
    /// Records committed by (commit time - seconds - wait) on the heartbeat
    /// series (same rule as `cover`); without a series, the commit seq.
    fn dispatch_thr(&self, seq: u32, secs: f64) -> u32 {
        match &self.series {
            None => seq,
            Some(s) => {
                let i = s.partition_point(|&(_, c)| c <= seq as u64);
                let tc = if i < s.len() { s[i].0 } else { s[s.len() - 1].0 };
                let td = tc - secs - self.wait;
                let j = s.partition_point(|&(t, _)| t <= td);
                if j == 0 {
                    0
                } else {
                    (s[j - 1].1 as u32).min(seq)
                }
            }
        }
    }
    /// Coverage of domain `id` (native record seq `seq`, seconds `secs`) by
    /// one anchor set of its own bucket.
    fn eval(&self, id: usize, seq: u32, secs: f64, set: &str, rng: &mut Rng) -> Option<Eval> {
        let q = &self.ck.doms[id];
        let b = q.bucket();
        let empty: Vec<Anchor> = Vec::new();
        let nat = self.natives.get(&b).unwrap_or(&empty);
        let all = self.all.get(&b).unwrap_or(&empty);
        let (n, cap, s) = (self.n, self.cap, self.samples);
        match set {
            "natives_before_dispatch" | "natives_before_commit" => {
                let thr = if set == "natives_before_dispatch" { self.dispatch_thr(seq, secs) } else { seq };
                let k = nat.partition_point(|a| a.seq < thr);
                evaluate(q, n, &nat[..k], &|_| true, cap, s, rng)
            }
            "all_earlier_ids" => {
                let k = all.partition_point(|a| (a.id as usize) < id);
                evaluate(q, n, &all[..k], &|_| true, cap, s, rng)
            }
            // Smaller IDs that are natives or still pending at the checkpoint: a
            // well-founded anchor set without delegated domains (whose
            // representative may be newer than the query).
            "earlier_non_delegated" => {
                let k = all.partition_point(|a| (a.id as usize) < id);
                let l = self.ck.ledger.as_ref().expect("ledger");
                let pend = |x: usize| matches!(l.entries[x].state, L_UNRESERVED | L_RESERVED | L_STARTED);
                evaluate(q, n, &all[..k], &|a| a.seq != NONE || pend(a.id as usize), cap, s, rng)
            }
            "all_natives" => evaluate(q, n, nat, &|a| a.id as usize != id, cap, s, rng),
            "all_other_domains" => evaluate(q, n, all, &|a| a.id as usize != id, cap, s, rng),
            _ => panic!("anchor set {set}"),
        }
    }
}

// ------------------------------------------------------------ sampling
/// Stratified uniform draws (with replacement) of `per` indices per stratum.
/// Returns (population index, stratum, Horvitz-Thompson weight).
fn stratified(keys: &[u16], per: usize, rng: &mut Rng) -> (Vec<(usize, u16, f64)>, BTreeMap<u16, (usize, usize)>) {
    let mut by: BTreeMap<u16, Vec<usize>> = BTreeMap::new();
    for (i, &k) in keys.iter().enumerate() {
        by.entry(k).or_default().push(i);
    }
    let mut out = Vec::new();
    let mut info = BTreeMap::new();
    for (k, v) in by {
        let draws = uniform(v.len(), per, rng);
        let nh: usize = draws.iter().map(|d| d.1).sum();
        for (ix, mult) in &draws {
            out.push((v[*ix], k, *mult as f64 * v.len() as f64 / nh as f64));
        }
        info.insert(k, (v.len(), draws.len()));
    }
    (out, info)
}

fn decade(s: &DomSum) -> u16 {
    if s.infinite {
        return 9;
    }
    match s.points.max(1.0).log10().floor() as i32 {
        x if x <= 3 => x as u16,
        4 | 5 => 4,
        _ => 6,
    }
}
fn stratum_name(k: u16) -> String {
    let dec = match k % 10 {
        0 => "1e0",
        1 => "1e1",
        2 => "1e2",
        3 => "1e3",
        4 => "1e4-1e5",
        6 => ">=1e6",
        _ => "infinite",
    };
    format!("g{}|{}", k / 10, dec)
}

struct Draw {
    id: u32,
    stratum: u16,
    ht: f64,
    w: Vec<f64>,
    evs: Vec<Option<Eval>>,
}

const VOC: [&str; 6] = ["exact_pointwise", "d_only", "hull", "hull_per_d_run", "ar_c2", "hull_c2"];

fn voc_of(ev: &Eval, name: &str) -> (usize, f64) {
    if ev.uncovered == 0.0 {
        return (0, 0.0);
    }
    match name {
        "exact_pointwise" => (1, ev.uncovered),
        "d_only" => (ev.d_only.pieces, ev.d_only.points),
        "hull" => (ev.hull.pieces, ev.hull.points),
        "hull_per_d_run" => (ev.hull_d.pieces, ev.hull_d.points),
        "ar_c2" => (ev.ar.pieces, ev.ar.points),
        _ => (ev.hull_c2.pieces, ev.hull_c2.points),
    }
}

/// Shares per anchor set and weight. Unevaluated queries (infinite or not
/// enumerable) count as uncovered with a full residual and a failed gate.
fn aggregate(
    draws: &[Draw],
    sets: &[&str],
    wnames: &[&str],
    strata: &BTreeMap<u16, (usize, usize)>,
    name: &dyn Fn(u16) -> String,
    group: &dyn Fn(u16) -> String,
) -> Value {
    let mut out = serde_json::Map::new();
    for (si, set) in sets.iter().enumerate() {
        let mut per_w = serde_json::Map::new();
        // Per group of strata (e.g. generation): total weight and covered shares.
        let mut by_group: BTreeMap<String, BTreeMap<&str, [f64; 4]>> = BTreeMap::new();
        for d in draws {
            let g = by_group.entry(group(d.stratum)).or_default();
            for (wi, wn) in wnames.iter().enumerate() {
                let w = d.ht * d.w[wi];
                if !(w > 0.0) {
                    continue;
                }
                let e = g.entry(wn).or_default();
                e[0] += w;
                if let Some(ev) = &d.evs[si] {
                    e[1] += w * (ev.uncovered == 0.0) as u8 as f64;
                    let (p, x) = voc_of(ev, "d_only");
                    e[2] += w * (p <= 8 && x <= 0.10 * ev.points) as u8 as f64;
                    let (p, x) = voc_of(ev, "hull");
                    e[3] += w * (p <= 8 && x <= 0.10 * ev.points) as u8 as f64;
                }
            }
        }
        let by_group: BTreeMap<String, BTreeMap<&str, Value>> = by_group
            .into_iter()
            .map(|(g, m)| (g, m.into_iter().map(|(k, e)| (k, json!({"total_weight": e[0], "fully_covered_share": e[1] / e[0], "gate_d_only": e[2] / e[0], "gate_hull": e[3] / e[0]}))).collect()))
            .collect();
        for (wi, wn) in wnames.iter().enumerate() {
            let (mut tot, mut unev, mut single, mut full, mut uncf) = (0f64, 0f64, 0f64, 0f64, 0f64);
            let mut voc: BTreeMap<&str, [f64; 4]> = BTreeMap::new(); // gate, resid frac, pieces, gate(<=2 pieces, <=10%)
            for d in draws {
                let w = d.ht * d.w[wi];
                if !(w > 0.0) {
                    continue;
                }
                tot += w;
                let Some(ev) = &d.evs[si] else {
                    unev += w;
                    uncf += w;
                    for v in VOC {
                        let e = voc.entry(v).or_default();
                        e[1] += w;
                        e[2] += w;
                    }
                    continue;
                };
                single += w * ev.single_container as u8 as f64;
                full += w * (ev.uncovered == 0.0) as u8 as f64;
                let frac = if ev.points > 0.0 { ev.uncovered / ev.points } else { 0.0 };
                uncf += w * frac;
                for v in VOC {
                    let (pieces, pts) = voc_of(ev, v);
                    let rf = if ev.points > 0.0 { pts / ev.points } else { 0.0 };
                    let e = voc.entry(v).or_default();
                    e[0] += w * (rf <= 0.10 && pieces <= 8) as u8 as f64;
                    e[1] += w * rf;
                    e[2] += w * pieces as f64;
                    e[3] += w * (rf <= 0.10 && pieces <= 2) as u8 as f64;
                }
            }
            if tot <= 0.0 {
                continue;
            }
            let vj: BTreeMap<&str, Value> = voc
                .iter()
                .map(|(k, e)| {
                    (*k, json!({"gate_share(resid<=10%,pieces<=8)": e[0] / tot, "gate_share(resid<=10%,pieces<=2)": e[3] / tot,
                        "mean_residual_fraction": e[1] / tot, "mean_pieces": e[2] / tot}))
                })
                .collect();
            per_w.insert(wn.to_string(), json!({"total_weight": tot, "unevaluated_share": unev / tot,
                "single_container_share": single / tot, "fully_covered_share": full / tot,
                "mean_uncovered_fraction": uncf / tot, "vocabularies": vj}));
        }
        // Count-weighted piece histograms and residual-fraction quantiles of
        // the partially covered draws.
        let mut hist: BTreeMap<&str, BTreeMap<usize, f64>> = BTreeMap::new();
        let mut rfs: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
        let mut partial_w = 0f64;
        let mut partial_n = 0usize;
        for d in draws {
            let Some(ev) = &d.evs[si] else { continue };
            if ev.uncovered == 0.0 {
                continue;
            }
            partial_w += d.ht;
            partial_n += 1;
            for v in ["d_only", "ar_c2", "hull_per_d_run"] {
                *hist.entry(v).or_default().entry(voc_of(ev, v).0).or_default() += d.ht;
            }
            for v in ["exact_pointwise", "d_only", "hull"] {
                rfs.entry(v).or_default().push(voc_of(ev, v).1 / ev.points);
            }
        }
        let q: BTreeMap<&str, Vec<f64>> = rfs.iter_mut().map(|(k, v)| (*k, util::quantiles(v, &[0.1, 0.25, 0.5, 0.75, 0.9]))).collect();
        // Anchors needed by the fully covered draws (greedy first-hit count;
        // count-weighted histogram, distinct-draw quantiles).
        let mut ah: BTreeMap<&str, f64> = BTreeMap::new();
        let mut av: Vec<f64> = Vec::new();
        let (mut aw, mut asum) = (0f64, 0f64);
        for d in draws {
            let Some(ev) = &d.evs[si] else { continue };
            if ev.uncovered != 0.0 {
                continue;
            }
            let k = ev.anchors_used;
            let b = match k {
                0 => "0(single)",
                1 => "1",
                2 => "2",
                3 => "3",
                4 => "4",
                5..=8 => "5-8",
                9..=16 => "9-16",
                17..=64 => "17-64",
                _ => ">64",
            };
            *ah.entry(b).or_default() += d.ht;
            aw += d.ht;
            asum += d.ht * k as f64;
            av.push(k as f64);
        }
        let aq = util::quantiles(&mut av, &[0.5, 0.9, 0.99, 1.0]);
        // Per stratum (count weight within the stratum = uniform draws).
        let mut st: BTreeMap<String, Value> = BTreeMap::new();
        for (k, (nh, dh)) in strata {
            let (mut w, mut f, mut u) = (0f64, 0f64, 0f64);
            for d in draws.iter().filter(|d| d.stratum == *k) {
                w += d.ht;
                match &d.evs[si] {
                    None => u += d.ht,
                    Some(ev) => f += d.ht * (ev.uncovered == 0.0) as u8 as f64,
                }
            }
            if w > 0.0 {
                st.insert(name(*k), json!({"population": nh, "distinct_draws": dh, "fully_covered_share": f / w, "unevaluated_share": u / w}));
            }
        }
        out.insert(set.to_string(), json!({"by_weight": per_w,
            "partial(count-weighted)": {"estimated_domains": partial_w, "distinct_draws": partial_n,
                "pieces_histogram": hist, "residual_fraction_quantiles[0.1,0.25,0.5,0.75,0.9]": q},
            "by_stratum": st, "by_group": by_group,
            "anchors_used_by_fully_covered(greedy first-hit count)": {"count_weighted_mean": if aw > 0.0 { asum / aw } else { f64::NAN },
                "histogram(count-weighted)": ah, "quantiles_distinct_draws[0.5,0.9,0.99,max]": aq}}));
    }
    Value::Object(out)
}

// ------------------------------------------------------------ helpers
fn qstats(v: &mut Vec<f64>) -> Value {
    let n = v.len();
    let s: f64 = v.iter().sum();
    let q = util::quantiles(v, &[0.5, 0.9, 0.99, 0.999, 1.0]);
    json!({"n": n, "sum": s, "mean": if n > 0 { s / n as f64 } else { f64::NAN }, "quantiles[0.5,0.9,0.99,0.999,max]": q})
}
fn small_hist(v: &[u32], cap: u32) -> BTreeMap<String, usize> {
    let mut h: BTreeMap<u32, usize> = BTreeMap::new();
    for &x in v {
        *h.entry(x.min(cap)).or_default() += 1;
    }
    h.into_iter().map(|(k, c)| (if k == cap { format!(">={cap}") } else { format!("{k:03}") }, c)).collect()
}

#[derive(Default, Clone, Copy)]
struct Grp {
    n: f64,
    secs: f64,
    succ: f64,
    edges: f64,
    cre_a: f64,
    cre_r: f64,
    desc: f64,
}
impl Grp {
    fn add(&mut self, o: &Grp) {
        self.n += o.n;
        self.secs += o.secs;
        self.succ += o.succ;
        self.edges += o.edges;
        self.cre_a += o.cre_a;
        self.cre_r += o.cre_r;
        self.desc += o.desc;
    }
    fn sub(&self, o: &Grp) -> Grp {
        Grp {
            n: self.n - o.n,
            secs: self.secs - o.secs,
            succ: self.succ - o.succ,
            edges: self.edges - o.edges,
            cre_a: self.cre_a - o.cre_a,
            cre_r: self.cre_r - o.cre_r,
            desc: self.desc - o.desc,
        }
    }
    fn mean(&self) -> [f64; 6] {
        let m = self.n.max(1.0);
        [self.secs / m, self.succ / m, self.edges / m, (self.cre_a + self.cre_r) / m, self.cre_a / m, self.cre_r / m]
    }
}

/// Pending-weight predictor [E]: means over Route natives of record
/// generation >= `model_gen` per (owner, rank bound) when the group has >= 5
/// natives, else per owner (>= 5), else pooled.
struct Model {
    grp: HashMap<(u16, u8), Grp>,
    by_owner: HashMap<u16, Grp>,
    pooled: Grp,
    model_gen: u8,
}
impl Model {
    fn one(e: &Edges, r: &recs::Rec) -> Grp {
        let id = r.id as usize;
        Grp {
            n: 1.0,
            secs: r.seconds as f64,
            succ: r.successors as f64,
            edges: (e.out[0][id] + e.out[1][id]) as f64,
            cre_a: e.cre[0][id] as f64,
            cre_r: e.cre[1][id] as f64,
            desc: (e.subtree[id] - 1) as f64,
        }
    }
    fn build(ck: &Ckpt, e: &Edges, rn: &[&recs::Rec], model_gen: u8) -> Model {
        let mut grp: HashMap<(u16, u8), Grp> = HashMap::new();
        for r in rn {
            if r.gen < model_gen {
                continue;
            }
            let d = &ck.doms[r.id as usize];
            grp.entry((d.owner, d.rank)).or_default().add(&Model::one(e, r));
        }
        let mut by_owner: HashMap<u16, Grp> = HashMap::new();
        let mut pooled = Grp::default();
        for ((o, _), g) in &grp {
            by_owner.entry(*o).or_default().add(g);
            pooled.add(g);
        }
        Model { grp, by_owner, pooled, model_gen }
    }
    fn predict(&self, d: &Dom) -> ([f64; 6], f64) {
        let g = self
            .grp
            .get(&(d.owner, d.rank))
            .filter(|g| g.n >= 5.0)
            .or_else(|| self.by_owner.get(&d.owner).filter(|g| g.n >= 5.0))
            .unwrap_or(&self.pooled);
        (g.mean(), g.desc / g.n.max(1.0))
    }
    /// Fallback level and group (owner-rank / owner / pooled) of `predict`,
    /// optionally leaving one member `own` out (same thresholds).
    fn level(&self, d: &Dom, own: Option<&Grp>) -> (&'static str, Grp) {
        let z = Grp::default();
        let o = own.unwrap_or(&z);
        if let Some(g) = self.grp.get(&(d.owner, d.rank)).map(|g| g.sub(o)).filter(|g| g.n >= 5.0) {
            return ("owner_rank", g);
        }
        if let Some(g) = self.by_owner.get(&d.owner).map(|g| g.sub(o)).filter(|g| g.n >= 5.0) {
            return ("owner", g);
        }
        ("pooled", self.pooled.sub(o))
    }
    fn summary(&self) -> Value {
        json!({"model_gen(min record generation)": self.model_gen, "groups(owner,rank)": self.grp.len(), "owners": self.by_owner.len(), "natives": self.pooled.n,
            "pooled_means[seconds, successors, out_edges, created, created_apply, created_route]": self.pooled.mean(),
            "pooled_mean_descendants": self.pooled.desc / self.pooled.n.max(1.0)})
    }
}

// ------------------------------------------------------------------ run
pub fn run(dir: &Path, opts: &Opts) {
    let ck = util::load(dir, opts, true);
    let n = ck.n;
    let t0 = std::time::Instant::now();
    let sums = util::dom_sums(&ck.doms, n);
    let seed = opts.num("seed", 20260928u64);
    let per = opts.num("per", 300usize);
    let kc = opts.num("kc", 3000usize);
    let rows_path = opts.get("rows").map(|p| p.to_string());
    let series: Option<Vec<(f64, u64)>> = opts.get("series").map(|p| {
        std::fs::read_to_string(p)
            .unwrap()
            .lines()
            .filter_map(|l| {
                let mut it = l.split_whitespace();
                Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
            })
            .collect()
    });
    let ledger = ck.ledger.as_ref().expect("ledger section");
    let pending = |id: usize| matches!(ledger.entries[id].state, L_UNRESERVED | L_RESERVED | L_STARTED);
    // 0 native, 1 pending, 2 delegated/other.
    let class = |id: usize| -> usize {
        if ck.native_of[id] != NONE {
            0
        } else if pending(id) {
            1
        } else {
            2
        }
    };
    let e = edges(&ck);
    let mut report = serde_json::Map::new();
    report.insert("dir".into(), json!(dir));
    report.insert("generation".into(), json!(ck.m.generation));
    report.insert("params".into(), json!({"per": per, "kc": kc, "seed": seed,
        "cap": opts.num("cap", 2.0e6f64), "samples": opts.num("samples", 20000usize),
        "series": opts.get("series"), "wait": opts.num("wait", 30f64)}));
    report.insert("edges".into(), e.counts.clone());

    // ---- exact fan-out and creation accounting.
    let rn: Vec<&recs::Rec> = ck.recs.iter().filter(|r| recs::is_native(r.kind) && r.phase == 1).collect();
    // Model generations: default the last two record generations of Route natives.
    let model_gen = opts.num("model-gen", rn.iter().map(|r| r.gen).max().unwrap_or(0).saturating_sub(1));
    let an: Vec<&recs::Rec> = ck.recs.iter().filter(|r| recs::is_native(r.kind) && r.phase == 0).collect();
    let fan = |v: &[&recs::Rec]| -> Value {
        let mut by_gen: BTreeMap<u8, Vec<&recs::Rec>> = BTreeMap::new();
        for r in v {
            by_gen.entry(r.gen).or_default().push(r);
        }
        let one = |v: &[&recs::Rec]| -> Value {
            let col = |f: &dyn Fn(&recs::Rec) -> f64| -> Value { qstats(&mut v.iter().map(|r| f(r)).collect()) };
            let id = |r: &recs::Rec| r.id as usize;
            json!({
                "natives": v.len(),
                "seconds": col(&|r| r.seconds as f64),
                "successors(stats)": col(&|r| r.successors as f64),
                "out_edges_to_route": col(&|r| e.out[1][id(r)] as f64),
                "out_edges_to_apply": col(&|r| e.out[0][id(r)] as f64),
                "created_route": col(&|r| e.cre[1][id(r)] as f64),
                "created_apply": col(&|r| e.cre[0][id(r)] as f64),
                "descendants(creator forest)": col(&|r| (e.subtree[id(r)] - 1) as f64),
                "points(finite)": col(&|r| { let s = &sums[id(r)]; if s.infinite { 0.0 } else { s.points } }),
                "hist_out_edges_to_apply": small_hist(&v.iter().map(|r| e.out[0][id(r)]).collect::<Vec<_>>(), 8),
                "hist_created_apply": small_hist(&v.iter().map(|r| e.cre[0][id(r)]).collect::<Vec<_>>(), 8),
                "hist_out_edges_to_route": small_hist(&v.iter().map(|r| e.out[1][id(r)]).collect::<Vec<_>>(), 64),
                "hist_created_route": small_hist(&v.iter().map(|r| e.cre[1][id(r)]).collect::<Vec<_>>(), 64),
            })
        };
        let g: BTreeMap<String, Value> = by_gen.iter().map(|(g, v)| (format!("g{g}"), one(v))).collect();
        json!({"all": one(v), "by_record_generation": g})
    };
    report.insert("fanout_route_natives".into(), fan(&rn));
    report.insert("fanout_apply_natives".into(), fan(&an));
    eprintln!("route: fan-out done ({:.1} s)", t0.elapsed().as_secs_f64());
    // Creator class of every domain: [target phase|target class] -> creator kind counts.
    let ckind = |c: u32| -> &'static str {
        if c == NONE {
            "none(initial)"
        } else {
            let c = c as usize;
            match (ck.native_of[c] != NONE, ck.doms[c].phase) {
                (true, 1) => "route_native",
                (true, _) => "apply_native",
                (false, _) => "non_native",
            }
        }
    };
    let cls_name = ["native", "pending", "delegated"];
    let mut creators: BTreeMap<String, BTreeMap<&str, usize>> = BTreeMap::new();
    let mut by_cgen: BTreeMap<String, usize> = BTreeMap::new();
    for id in 0..ck.doms.len() {
        let d = &ck.doms[id];
        let key = format!("{}|{}", if d.phase == 1 { "Route" } else { "Apply" }, cls_name[class(id)]);
        let c = e.creator[id];
        *creators.entry(key.clone()).or_default().entry(ckind(c)).or_default() += 1;
        if c != NONE && ck.native_of[c as usize] != NONE && ck.doms[c as usize].phase == 1 {
            let g = ck.recs[ck.native_of[c as usize] as usize].gen;
            *by_cgen.entry(format!("{key}|creator_record_gen{g}")).or_default() += 1;
        }
    }
    report.insert("creator_kind_by_target[phase|class]".into(), json!(creators));
    report.insert("route_native_creations_by_creator_record_gen".into(), json!(by_cgen));

    // ---- prediction model per (owner, rank) over Route natives [E].
    let model = Model::build(&ck, &e, &rn, model_gen);
    let predict = |d: &Dom| model.predict(d);
    report.insert("model[E]".into(), model.summary());

    // ---- pools and bucket sizes.
    let (natives, all) = pools(&ck, &sums, &[0, 1]);
    let mut bsz: Vec<f64> = all.iter().filter(|(b, _)| *b >> 16 == 1).map(|(_, v)| v.len() as f64).collect();
    let mut bnat: Vec<f64> = natives.iter().filter(|(b, _)| *b >> 16 == 1).map(|(_, v)| v.len() as f64).collect();
    report.insert("route_buckets".into(), json!({"domains_per_bucket": qstats(&mut bsz), "natives_per_bucket": qstats(&mut bnat)}));
    eprintln!("route: pools built ({:.1} s)", t0.elapsed().as_secs_f64());
    let ctx = Ctx { ck: &ck, n, natives, all, series, wait: opts.num("wait", 30f64), cap: opts.num("cap", 2.0e6f64), samples: opts.num("samples", 20000usize) };
    let mut rng = Rng::new(seed);
    let rows = rows_path.map(|p| std::sync::Mutex::new(std::fs::File::create(p).unwrap()));
    let write_rows = |sample: &str, sets: &[&str], wn: &[&str], draws: &[Draw], sname: &dyn Fn(u16) -> String| {
        let Some(f) = &rows else { return };
        let mut f = f.lock().unwrap();
        for d in draws {
            let evs: BTreeMap<&str, Value> = sets
                .iter()
                .zip(&d.evs)
                .map(|(s, ev)| {
                    (*s, ev.as_ref().map_or(Value::Null, |e| json!({"points": e.points, "exact": e.exact, "candidates": e.candidates,
                        "single": e.single_container, "uncovered": e.uncovered, "anchors_used": e.anchors_used, "d_only": [e.d_only.pieces, e.d_only.points],
                        "hull": e.hull.points, "hull_d": [e.hull_d.pieces, e.hull_d.points], "ar": [e.ar.pieces, e.ar.points]})))
                })
                .collect();
            let w: BTreeMap<&str, f64> = wn.iter().zip(&d.w).map(|(k, v)| (*k, *v)).collect();
            let row = json!({"sample": sample, "id": d.id, "owner": ck.doms[d.id as usize].owner_string(n), "rank": ck.doms[d.id as usize].rank,
                "stratum": sname(d.stratum), "class": CLS[class(d.id as usize)], "ht": d.ht, "w": w, "evals": evs});
            writeln!(f, "{row}").unwrap();
        }
    };

    // ---- (1a) Route native-pending, stratified by admission gen x decade.
    let rpend: Vec<usize> = (0..ck.doms.len()).filter(|&id| ck.doms[id].phase == 1 && pending(id)).collect();
    let keys: Vec<u16> = rpend
        .iter()
        .map(|&id| (util::id_gen(&ck.m, id).min(9) as u16) * 10 + decade(&sums[id]))
        .collect();
    let (plan, strata) = stratified(&keys, per, &mut rng);
    let sets_p = ["all_natives", "earlier_non_delegated", "all_earlier_ids", "all_other_domains"];
    let wn_p = ["count", "pred_seconds[E]", "pred_successors[E]", "pred_out_edges[E]", "pred_created[E]", "pred_created_apply[E]", "pred_created_route[E]", "pred_descendants[E]", "points"];
    let draws: Vec<Draw> = plan
        .par_iter()
        .map(|&(ix, st, ht)| {
            let id = rpend[ix];
            let mut rng = Rng::new(seed ^ (id as u64).wrapping_mul(0x2545F4914F6CDD1D));
            let d = &ck.doms[id];
            let (m, desc) = predict(d);
            let pts = if sums[id].infinite { 0.0 } else { sums[id].points };
            let w = vec![1.0, m[0], m[1], m[2], m[3], m[4], m[5], desc, pts];
            let evs = sets_p.iter().map(|s| ctx.eval(id, NONE, 0.0, s, &mut rng)).collect();
            Draw { id: id as u32, stratum: st, ht, w, evs }
        })
        .collect();
    write_rows("route_pending", &sets_p, &wn_p, &draws, &stratum_name);
    report.insert("route_pending".into(), json!({"population": rpend.len(), "strata": strata.len(),
        "distinct_draws": draws.len(), "coverage": aggregate(&draws, &sets_p, &wn_p, &strata, &stratum_name, &|k| format!("admission_g{}", k / 10))}));
    eprintln!("route: route_pending done ({:.1} s)", t0.elapsed().as_secs_f64());
    drop(draws);

    // ---- (1b) Route natives (historical), stratified by record gen x decade.
    let keys: Vec<u16> = rn.iter().map(|r| (r.gen.min(9) as u16) * 10 + decade(&sums[r.id as usize])).collect();
    let (plan, strata) = stratified(&keys, per, &mut rng);
    let sets_h = ["natives_before_dispatch", "natives_before_commit", "earlier_non_delegated", "all_earlier_ids"];
    let wn_h = ["count", "seconds", "successors", "out_edges", "created", "created_apply", "created_route", "descendants", "points"];
    let draws: Vec<Draw> = plan
        .par_iter()
        .map(|&(ix, st, ht)| {
            let r = rn[ix];
            let id = r.id as usize;
            let mut rng = Rng::new(seed ^ (id as u64).wrapping_mul(0x2545F4914F6CDD1D));
            let pts = if sums[id].infinite { 0.0 } else { sums[id].points };
            let w = vec![1.0, r.seconds as f64, r.successors as f64, (e.out[0][id] + e.out[1][id]) as f64,
                (e.cre[0][id] + e.cre[1][id]) as f64, e.cre[0][id] as f64, e.cre[1][id] as f64, (e.subtree[id] - 1) as f64, pts];
            let evs = sets_h.iter().map(|s| ctx.eval(id, r.seq, r.seconds as f64, s, &mut rng)).collect();
            Draw { id: id as u32, stratum: st, ht, w, evs }
        })
        .collect();
    write_rows("route_natives", &sets_h, &wn_h, &draws, &stratum_name);
    report.insert("route_natives".into(), json!({"population": rn.len(), "strata": strata.len(),
        "distinct_draws": draws.len(), "coverage": aggregate(&draws, &sets_h, &wn_h, &strata, &stratum_name, &|k| format!("record_g{}", k / 10))}));
    eprintln!("route: route_natives done ({:.1} s)", t0.elapsed().as_secs_f64());
    drop(draws);

    // ---- (2) pending domains by the coverage of their Route-native creator.
    let law = cost_law(&ck, &sums, 0);
    let apend: Vec<usize> = (0..ck.doms.len()).filter(|&id| ck.doms[id].phase == 0 && pending(id) && !sums[id].infinite && !sums[id].empty).collect();
    let apred: Vec<f64> = apend.iter().map(|&id| law.predict_binned(ck.doms[id].owner, sums[id].points)).collect();
    let is_rn = |c: u32| c != NONE && ck.native_of[c as usize] != NONE && ck.doms[c as usize].phase == 1;
    let cplans: Vec<(&str, &Vec<usize>, Vec<(usize, usize)>)> = vec![
        ("apply_pending_uniform", &apend, uniform(apend.len(), kc, &mut rng)),
        ("apply_pending_pps_predicted_apply_seconds[E]", &apend, pps(&apred, kc, &mut rng)),
        ("route_pending_uniform", &rpend, uniform(rpend.len(), kc, &mut rng)),
    ];
    let mut cmap = serde_json::Map::new();
    for (name, pop, plan) in cplans {
        // Unique Route-native creators to evaluate.
        let mut cre: Vec<u32> = plan.iter().map(|&(ix, _)| e.creator[pop[ix]]).filter(|&c| is_rn(c)).collect();
        cre.sort_unstable();
        cre.dedup();
        let cev: HashMap<u32, Vec<Option<Eval>>> = cre
            .par_iter()
            .map(|&c| {
                let r = &ck.recs[ck.native_of[c as usize] as usize];
                let mut rng = Rng::new(seed ^ (c as u64).wrapping_mul(0x9E3779B97F4A7C15));
                (c, sets_h.iter().map(|s| ctx.eval(c as usize, r.seq, r.seconds as f64, s, &mut rng)).collect())
            })
            .collect();
        // Coverage of the pending domain itself in its own bucket.
        let sev: HashMap<usize, Vec<Option<Eval>>> = plan
            .par_iter()
            .map(|&(ix, _)| {
                let id = pop[ix];
                let mut rng = Rng::new(seed ^ (id as u64).wrapping_mul(0x2545F4914F6CDD1D));
                (id, ["all_natives", "all_earlier_ids"].iter().map(|s| ctx.eval(id, NONE, 0.0, s, &mut rng)).collect())
            })
            .collect();
        // Tallies: weight kinds count (and predicted Apply seconds for the Apply samples).
        let mut t: BTreeMap<String, f64> = BTreeMap::new();
        let mut add = |k: String, w: f64| *t.entry(k).or_default() += w;
        for &(ix, mult) in &plan {
            let id = pop[ix];
            let w = mult as f64;
            let c = e.creator[id];
            add("draws".into(), w);
            add(format!("creator:{}", ckind(c)), w);
            let self_cov = sev.get(&id).map(|v| v.iter().map(|x| x.as_ref().is_some_and(|ev| ev.uncovered == 0.0)).collect::<Vec<_>>());
            if let Some(sc) = &self_cov {
                add(format!("self_fully_covered_by:all_natives={}", sc[0]), w);
                add(format!("self_fully_covered_by:all_earlier_ids={}", sc[1]), w);
            }
            if is_rn(c) {
                let evs = &cev[&c];
                for (s, ev) in sets_h.iter().zip(evs) {
                    let full = ev.as_ref().is_some_and(|ev| ev.uncovered == 0.0);
                    let unev = ev.is_none();
                    add(format!("route_native_creator_fully_covered_by:{s}"), w * full as u8 as f64);
                    add(format!("route_native_creator_unevaluated:{s}"), w * unev as u8 as f64);
                    if let Some(sc) = &self_cov {
                        add(format!("joint[creator_covered_by:{s}={full}|self_covered_by:all_earlier_ids={}]", sc[1]), w);
                        add(format!("joint[creator_covered_by:{s}={full}|self_covered_by:all_natives={}]", sc[0]), w);
                    }
                }
            }
        }
        let draws = t.get("draws").copied().unwrap_or(0.0);
        let shares: BTreeMap<String, f64> = t.iter().map(|(k, v)| (k.clone(), v / draws.max(1e-300))).collect();
        cmap.insert(name.to_string(), json!({"population": pop.len(), "draws": draws, "distinct": plan.len(),
            "route_native_creators_evaluated": cre.len(), "shares_of_draws": shares}));
        eprintln!("route: {name} done ({:.1} s)", t0.elapsed().as_secs_f64());
    }
    report.insert("pending_by_creator_coverage".into(), Value::Object(cmap));
    report.insert("pending_apply_predicted_seconds[E]".into(), json!(apred.iter().sum::<f64>()));

    // ---- (3) every admitted domain at its admission: union cover by the
    // domains that existed then (every smaller ID, any state: the general
    // union cover at admission, D2) and by the natives committed before its
    // creator's record (merged natives only: the G2' rule at admission). A
    // covered domain would not have been created. `self_or_ancestor` also
    // counts domains with a covered creator-forest ancestor within --max-up
    // levels (their subtree would not have been generated) [E: pointwise
    // idealization, an upper bound on the cascade].
    let per_adm = opts.num("per-adm", 600usize);
    let max_up = opts.num("max-up", 64usize);
    let adm: Vec<usize> = (0..ck.doms.len()).filter(|&id| e.creator[id] != NONE && !sums[id].empty).collect();
    let keys: Vec<u16> = adm.iter().map(|&id| ck.doms[id].phase as u16 * 10 + util::id_gen(&ck.m, id).min(9) as u16).collect();
    let (plan, strata) = stratified(&keys, per_adm, &mut rng);
    let sets_a = ["all_earlier_ids", "earlier_non_delegated", "natives_before_creator_commit"];
    let wn_a = ["count", "count_native", "count_delegated", "count_pending", "descendants", "points"];
    let creator_seq = |id: usize| -> u32 {
        let c = e.creator[id];
        if c == NONE {
            return 0;
        }
        let ix = ck.native_of[c as usize];
        if ix == NONE { 0 } else { ck.recs[ix as usize].seq }
    };
    let eval_adm = |id: usize, si: usize, rng: &mut Rng| -> Option<Eval> {
        if si == 0 {
            ctx.eval(id, NONE, 0.0, "all_earlier_ids", rng)
        } else if si == 1 {
            ctx.eval(id, NONE, 0.0, "earlier_non_delegated", rng)
        } else {
            ctx.eval(id, creator_seq(id), 0.0, "natives_before_commit", rng)
        }
    };
    let draws: Vec<Draw> = plan
        .par_iter()
        .map(|&(ix, st, ht)| {
            let id = adm[ix];
            let mut rng = Rng::new(seed ^ (id as u64).wrapping_mul(0x2545F4914F6CDD1D));
            let cls = class(id);
            let pts = if sums[id].infinite { 0.0 } else { sums[id].points };
            let w = vec![1.0, (cls == 0) as u8 as f64, (cls == 2) as u8 as f64, (cls == 1) as u8 as f64, (e.subtree[id] - 1) as f64, pts];
            let evs = (0..3).map(|si| eval_adm(id, si, &mut rng)).collect();
            Draw { id: id as u32, stratum: st, ht, w, evs }
        })
        .collect();
    let phase_name = |k: u16| if k / 10 == 1 { "Route" } else { "Apply" };
    let name_a = |k: u16| format!("{}|admission_g{}", phase_name(k), k % 10);
    let group_a = |k: u16| phase_name(k).to_string();
    write_rows("all_domains_at_admission", &sets_a, &wn_a, &draws, &name_a);
    let cov = aggregate(&draws, &sets_a, &wn_a, &strata, &name_a, &group_a);
    eprintln!("route: all_domains_at_admission done ({:.1} s)", t0.elapsed().as_secs_f64());
    // Ancestor chains.
    let chains: Vec<Vec<u32>> = draws
        .iter()
        .map(|d| {
            let mut v = Vec::new();
            let mut c = e.creator[d.id as usize];
            while c != NONE && v.len() < max_up {
                v.push(c);
                c = e.creator[c as usize];
            }
            v
        })
        .collect();
    let mut anc: Vec<u32> = chains.iter().flatten().copied().collect();
    anc.sort_unstable();
    anc.dedup();
    let aev: HashMap<u32, [bool; 3]> = anc
        .par_iter()
        .map(|&a| {
            if e.creator[a as usize] == NONE || sums[a as usize].empty {
                return (a, [false, false, false]);
            }
            let mut rng = Rng::new(seed ^ (a as u64).wrapping_mul(0x2545F4914F6CDD1D));
            let r0 = eval_adm(a as usize, 0, &mut rng).is_some_and(|ev| ev.uncovered == 0.0);
            let r1 = eval_adm(a as usize, 1, &mut rng).is_some_and(|ev| ev.uncovered == 0.0);
            let r2 = eval_adm(a as usize, 2, &mut rng).is_some_and(|ev| ev.uncovered == 0.0);
            (a, [r0, r1, r2])
        })
        .collect();
    let mut at: BTreeMap<String, f64> = BTreeMap::new();
    let mut add = |k: String, w: f64| *at.entry(k).or_default() += w;
    let mut depth: Vec<f64> = Vec::new();
    for (d, ch) in draws.iter().zip(&chains) {
        let ph = group_a(d.stratum);
        depth.push(e.depth[d.id as usize] as f64);
        add(format!("{ph}|domains"), d.ht);
        let trunc = ch.last().is_some_and(|&l| ch.len() == max_up && e.creator[l as usize] != NONE);
        add(format!("{ph}|chain_truncated"), d.ht * trunc as u8 as f64);
        for (si, set) in sets_a.iter().enumerate() {
            let selfc = d.evs[si].as_ref().is_some_and(|ev| ev.uncovered == 0.0);
            let ancc = ch.iter().any(|a| aev[a][si]);
            add(format!("{ph}|{set}|self_covered"), d.ht * selfc as u8 as f64);
            add(format!("{ph}|{set}|ancestor_covered_only"), d.ht * (!selfc && ancc) as u8 as f64);
            add(format!("{ph}|{set}|self_or_ancestor_covered"), d.ht * (selfc || ancc) as u8 as f64);
        }
    }
    report.insert("all_domains_at_admission".into(), json!({"population": adm.len(), "strata": strata.len(),
        "distinct_draws": draws.len(), "coverage": cov, "max_up": max_up, "ancestors_evaluated": anc.len(),
        "sampled_depth_quantiles[0.5,0.9,0.99,max]": util::quantiles(&mut depth, &[0.5, 0.9, 0.99, 1.0]),
        "ancestor_cascade(estimated domains, HT)": at}));
    eprintln!("route: ancestor cascade done ({:.1} s)", t0.elapsed().as_secs_f64());
    println!("{}", serde_json::to_string_pretty(&Value::Object(report)).unwrap());
}

// ---------------------------------------------------------- saturation
/// Point-space union of Route natives by record generation (window = record
/// segment) and of admitted Route domains by domain segment, by the
/// first-cover decomposition |U| = sum_i |D_i minus earlier D_j| (as
/// `saturation`), pooled over all Route owners: each draw picks a window
/// member with probability proportional to its points (or uniformly) and one
/// uniform point in it, and tests that point against the earlier members of
/// the same bucket. Infinite domains are counted but never drawn.
pub fn saturation(dir: &Path, opts: &Opts) {
    let ck = util::load(dir, opts, true);
    let n = ck.n;
    let sums = util::dom_sums(&ck.doms, n);
    let draws = opts.num("draws", 4000usize);
    let seed = opts.num("seed", 20260928u64);
    let (natives, all) = pools(&ck, &sums, &[1]);
    let mut rwin: Vec<(u64, u32, u32)> = Vec::new();
    let mut s0 = 0u32;
    for seg in &ck.m.records {
        rwin.push((seg.generation, s0, s0 + seg.count as u32));
        s0 += seg.count as u32;
    }
    let iwin: Vec<(u64, u32, u32)> = ck.m.domains.iter().map(|s| (s.generation, s.first as u32, (s.first + s.count) as u32)).collect();
    let mut out = serde_json::Map::new();
    for (kind, wins) in [("natives_by_record_generation", &rwin), ("admitted_by_domain_segment", &iwin)] {
        let pool = if kind.starts_with("natives") { &natives } else { &all };
        let mut cum = 0f64;
        let mut rows = Vec::new();
        for (wi, &(g, lo, hi)) in wins.iter().enumerate() {
            let key = |a: &Anchor| if kind.starts_with("natives") { a.seq } else { a.id };
            // Buckets in key order, so draws depend on the seed only (not on
            // the HashMap iteration order).
            let mut bkeys: Vec<u32> = pool.keys().copied().collect();
            bkeys.sort_unstable();
            let members: Vec<(u32, usize)> = bkeys
                .iter()
                .flat_map(|b| pool[b].iter().enumerate().filter(|(_, a)| key(a) >= lo && key(a) < hi).map(move |(i, _)| (*b, i)))
                .collect();
            let (mut fin, mut inf, mut spts) = (0usize, 0usize, 0f64);
            let wp: Vec<f64> = members
                .iter()
                .map(|&(b, i)| {
                    let s = &sums[pool[&b][i].id as usize];
                    if s.infinite {
                        inf += 1;
                        0.0
                    } else {
                        fin += 1;
                        spts += s.points;
                        s.points
                    }
                })
                .collect();
            let wu: Vec<f64> = members.iter().map(|&(b, i)| if sums[pool[&b][i].id as usize].infinite { 0.0 } else { 1.0 }).collect();
            let mut rng = Rng::new(seed ^ ((wi as u64) << 8) ^ kind.len() as u64);
            let est = |weights: &[f64], rng: &mut Rng| -> (f64, usize) {
                let picks = pps(weights, draws, rng);
                let res: Vec<(usize, usize)> = picks
                    .par_iter()
                    .map(|&(mi, mult)| {
                        let (b, i) = members[mi];
                        let v = &pool[&b];
                        let a = &v[i];
                        let d = &ck.doms[a.id as usize];
                        let mut rng = Rng::new(seed ^ (a.id as u64).wrapping_mul(0x2545F4914F6CDD1D) ^ wi as u64);
                        let (mut new, mut tot) = (0usize, 0usize);
                        for _ in 0..mult {
                            let Some((x, av, rv)) = draw_point(d, n, &mut rng) else { continue };
                            tot += 1;
                            // Natives are sorted by seq and `all` by id: earlier = prefix.
                            let covered = v[..i].iter().any(|c| c.hit(n, &x, av, rv));
                            new += !covered as usize;
                        }
                        (new, tot)
                    })
                    .collect();
                let (nw, tt) = res.iter().fold((0, 0), |s, x| (s.0 + x.0, s.1 + x.1));
                (if tt > 0 { nw as f64 / tt as f64 } else { f64::NAN }, tt)
            };
            let (fp, tp) = est(&wp, &mut rng);
            let (fu, tu) = est(&wu, &mut rng);
            let newp = if fp.is_finite() { fp * spts } else { 0.0 };
            cum += newp;
            rows.push(json!({"gen": g, "members": members.len(), "finite": fin, "infinite": inf, "sum_points": spts,
                "new_fraction_points": fp, "draws_points": tp, "new_points_est": newp, "union_after": cum,
                "new_points_per_member": if members.is_empty() { 0.0 } else { newp / members.len() as f64 },
                "new_fraction_uniform_domain": fu, "draws_uniform": tu}));
            eprintln!("route-saturation: {kind} window {wi} (gen {g}) done");
        }
        out.insert(kind.to_string(), json!(rows));
    }
    // Sanity: every native sits in the natives pool of its bucket.
    let nn: usize = natives.values().map(|v| v.len()).sum();
    println!("{}", serde_json::to_string_pretty(&json!({"dir": dir, "draws": draws, "route_natives_pooled": nn, "windows": out})).unwrap());
}

// ------------------------------------------------------- apply natives
/// `census route-apply CKPT`: the historical-native block of `route` for
/// Apply natives (stratified by record generation x points decade), with
/// the creation weights (created domains, creator-forest descendants), so the
/// indirect domain-volume effect of G2' (a fully covered job emits no
/// successors) can be read next to the Route one.
pub fn apply_natives(dir: &Path, opts: &Opts) {
    let ck = util::load(dir, opts, true);
    let n = ck.n;
    let t0 = std::time::Instant::now();
    let sums = util::dom_sums(&ck.doms, n);
    let seed = opts.num("seed", 20260928u64);
    let per = opts.num("per", 200usize);
    let series: Option<Vec<(f64, u64)>> = opts.get("series").map(|p| {
        std::fs::read_to_string(p)
            .unwrap()
            .lines()
            .filter_map(|l| {
                let mut it = l.split_whitespace();
                Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
            })
            .collect()
    });
    let e = edges(&ck);
    let an: Vec<&recs::Rec> = ck.recs.iter().filter(|r| recs::is_native(r.kind) && r.phase == 0).collect();
    let (natives, all) = pools(&ck, &sums, &[0]);
    eprintln!("route-apply: pools built ({:.1} s)", t0.elapsed().as_secs_f64());
    let ctx = Ctx { ck: &ck, n, natives, all, series, wait: opts.num("wait", 30f64), cap: opts.num("cap", 2.0e6f64), samples: opts.num("samples", 20000usize) };
    let mut rng = Rng::new(seed);
    let keys: Vec<u16> = an.iter().map(|r| (r.gen.min(9) as u16) * 10 + decade(&sums[r.id as usize])).collect();
    let (plan, strata) = stratified(&keys, per, &mut rng);
    let sets_h = ["natives_before_dispatch", "natives_before_commit", "earlier_non_delegated", "all_earlier_ids"];
    let wn_h = ["count", "seconds", "successors", "out_edges", "created", "created_apply", "created_route", "descendants", "points"];
    let draws: Vec<Draw> = plan
        .par_iter()
        .map(|&(ix, st, ht)| {
            let r = an[ix];
            let id = r.id as usize;
            let mut rng = Rng::new(seed ^ (id as u64).wrapping_mul(0x2545F4914F6CDD1D));
            let pts = if sums[id].infinite { 0.0 } else { sums[id].points };
            let w = vec![1.0, r.seconds as f64, r.successors as f64, (e.out[0][id] + e.out[1][id]) as f64,
                (e.cre[0][id] + e.cre[1][id]) as f64, e.cre[0][id] as f64, e.cre[1][id] as f64, (e.subtree[id] - 1) as f64, pts];
            let evs = sets_h.iter().map(|s| ctx.eval(id, r.seq, r.seconds as f64, s, &mut rng)).collect();
            Draw { id: id as u32, stratum: st, ht, w, evs }
        })
        .collect();
    eprintln!("route-apply: apply_natives done ({:.1} s)", t0.elapsed().as_secs_f64());
    if let Some(p) = opts.get("rows") {
        let mut f = std::io::BufWriter::new(std::fs::File::create(p).unwrap());
        for d in &draws {
            let evs: BTreeMap<&str, Value> = sets_h
                .iter()
                .zip(&d.evs)
                .map(|(s, ev)| {
                    (*s, ev.as_ref().map_or(Value::Null, |e| json!({"points": e.points, "exact": e.exact, "candidates": e.candidates,
                        "single": e.single_container, "uncovered": e.uncovered, "anchors_used": e.anchors_used, "d_only": [e.d_only.pieces, e.d_only.points],
                        "hull": e.hull.points})))
                })
                .collect();
            let w: BTreeMap<&str, f64> = wn_h.iter().zip(&d.w).map(|(k, v)| (*k, *v)).collect();
            let row = json!({"sample": "apply_natives", "id": d.id, "owner": ck.doms[d.id as usize].owner_string(n),
                "rank": ck.doms[d.id as usize].rank, "stratum": stratum_name(d.stratum), "ht": d.ht, "w": w, "evals": evs});
            writeln!(f, "{row}").unwrap();
        }
    }
    let tot_created: f64 = an.iter().map(|r| (e.cre[0][r.id as usize] + e.cre[1][r.id as usize]) as f64).sum();
    let report = json!({"dir": dir, "generation": ck.m.generation,
        "params": {"per": per, "seed": seed, "wait": opts.num("wait", 30f64), "series": opts.get("series")},
        "edges": e.counts,
        "apply_natives": {"population": an.len(), "strata": strata.len(), "distinct_draws": draws.len(),
            "created_exact_total": tot_created,
            "coverage": aggregate(&draws, &sets_h, &wn_h, &strata, &stratum_name, &|k| format!("record_g{}", k / 10))}});
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

// ------------------------------------------------------------ route hits
/// `census route-hits CKPT --rows-in ROWS.jsonl [--rows OUT.jsonl]`: for
/// every row of a `census route` rows file (same checkpoint), the incoming
/// edges of the drawn domain split into its creator edge (first incoming
/// edge from a smaller ID) and its LATER hits (transition in-edges from
/// inspected Route / Apply sources, alias in-edges from non-inspected
/// sources; self edges excluded), plus the pending-weight predictor of
/// `route` at the domain's (owner, rank bound), in-sample and leave-one-out
/// (a model-generation Route native is removed from its own group mean).
///
/// Why: a domain that an admission-time union cover never creates cannot
/// receive its later hits; each later request then needs its own admission
/// (a single-container hit elsewhere, or a union test and a multi-anchor
/// alias). The in-degree is the first-level count of those redirected
/// requests (distinct edges; repeated requests of one source collapse onto
/// one edge). The leave-one-out predictor lets the coverage-creation
/// relation of historical natives calibrate the pending creation weights.
/// Also prints exact later-hit totals per (phase, admission generation,
/// class) over every admitted domain, to check the sampled estimates.
pub fn hits(dir: &Path, opts: &Opts) {
    let ck = util::load(dir, opts, true);
    let t0 = std::time::Instant::now();
    let e = edges(&ck);
    let nd = ck.doms.len();
    // In-degree by source kind: 0 transition from a Route native, 1 transition
    // from an Apply native, 2 alias (source not inspected), 3 self edges.
    let mk = || (0..nd).map(|_| AtomicU32::new(0)).collect::<Vec<AtomicU32>>();
    let ind = [mk(), mk(), mk(), mk()];
    ckpt::edges_par(&ck.m, 1 << 22, |_, c| {
        for i in 0..c.len() / 8 {
            let (s, t) = ckpt::edge_at(c, i);
            let (s, t) = (s as usize, t as usize);
            let k = if s == t {
                3
            } else if ck.nodes[s] & 2 == 0 {
                2
            } else if ck.doms[s].phase == 1 {
                0
            } else {
                1
            };
            ind[k][t].fetch_add(1, Relaxed);
        }
    });
    let ind: Vec<Vec<u32>> = ind.into_iter().map(|v| v.into_iter().map(|a| a.into_inner()).collect()).collect();
    eprintln!("route-hits: in-degree pass ({:.1} s)", t0.elapsed().as_secs_f64());
    // Creator edge kind: 0 none (initial), 1 transition, 2 alias.
    let ckind = |t: usize| -> u8 {
        let c = e.creator[t];
        if c == NONE {
            0
        } else if ck.nodes[c as usize] & 2 != 0 {
            1
        } else {
            2
        }
    };
    let later = |t: usize| -> (u32, u32) {
        let tr = ind[0][t] + ind[1][t];
        let al = ind[2][t];
        match ckind(t) {
            1 => (tr - 1, al),
            2 => (tr, al - 1),
            _ => (tr, al),
        }
    };
    let ledger = ck.ledger.as_ref().expect("ledger section");
    let class = |id: usize| -> usize {
        if ck.native_of[id] != NONE {
            0
        } else if matches!(ledger.entries[id].state, L_UNRESERVED | L_RESERVED | L_STARTED) {
            1
        } else {
            2
        }
    };
    // Exact totals over every admitted (non-initial) domain.
    let mut tot: BTreeMap<String, [f64; 5]> = BTreeMap::new();
    for t in 0..nd {
        if e.creator[t] == NONE {
            continue;
        }
        let (tr, al) = later(t);
        let ph = if ck.doms[t].phase == 1 { "Route" } else { "Apply" };
        for key in [format!("{ph}|admission_g{}", util::id_gen(&ck.m, t).min(9)), format!("{ph}|{}", CLS[class(t)]), format!("{ph}|all")] {
            let v = tot.entry(key).or_default();
            v[0] += 1.0;
            v[1] += tr as f64;
            v[2] += al as f64;
            v[3] += (tr + al == 0) as u8 as f64;
            v[4] += ind[0][t] as f64 - (ckind(t) == 1 && ck.doms[e.creator[t] as usize].phase == 1) as u8 as f64;
        }
    }
    let totals: BTreeMap<String, Value> = tot
        .iter()
        .map(|(k, v)| (k.clone(), json!({"domains": v[0], "later_transition_in_edges": v[1], "later_alias_in_edges": v[2],
            "mean_later_transition": v[1] / v[0], "mean_later_alias": v[2] / v[0], "share_without_later_hits": v[3] / v[0],
            "later_transition_from_route_sources": v[4]})))
        .collect();
    // Model (identical to `route`'s).
    let rn: Vec<&recs::Rec> = ck.recs.iter().filter(|r| recs::is_native(r.kind) && r.phase == 1).collect();
    let model_gen = opts.num("model-gen", rn.iter().map(|r| r.gen).max().unwrap_or(0).saturating_sub(1));
    let model = Model::build(&ck, &e, &rn, model_gen);
    // Rows.
    let rin = opts.get("rows-in").expect("--rows-in ROWS.jsonl (census route --rows)");
    let mut out = opts.get("rows").map(|p| std::io::BufWriter::new(std::fs::File::create(p).unwrap()));
    let mut nrows = 0usize;
    let mut by_sample: BTreeMap<String, usize> = BTreeMap::new();
    for line in std::fs::read_to_string(rin).unwrap().lines() {
        let r: Value = serde_json::from_str(line).unwrap();
        let sample = r["sample"].as_str().unwrap().to_string();
        let id = r["id"].as_u64().unwrap() as usize;
        let d = &ck.doms[id];
        let (tr, al) = later(id);
        let (lvl, g) = model.level(d, None);
        let m = g.mean();
        // Leave-one-out only for Route natives of the model generations.
        let own = {
            let ix = ck.native_of[id];
            (ix != NONE && d.phase == 1 && ck.recs[ix as usize].gen >= model_gen).then(|| Model::one(&e, &ck.recs[ix as usize]))
        };
        let (llvl, lg) = model.level(d, own.as_ref());
        let lm = lg.mean();
        let row = json!({"sample": sample, "id": id, "phase": d.phase, "class": CLS[class(id)], "creator_edge": CKIND[ckind(id) as usize],
            "in_transition_from_route": ind[0][id], "in_transition_from_apply": ind[1][id], "in_alias": ind[2][id], "self_edges": ind[3][id],
            "later_transition": tr, "later_alias": al,
            "pred": {"level": lvl, "n": g.n, "seconds": m[0], "successors": m[1], "created": m[3], "created_apply": m[4], "created_route": m[5]},
            "pred_loo": {"level": llvl, "n": lg.n, "left_out": own.is_some(), "seconds": lm[0], "successors": lm[1], "created": lm[3], "created_apply": lm[4], "created_route": lm[5]}});
        if let Some(f) = out.as_mut() {
            writeln!(f, "{row}").unwrap();
        }
        nrows += 1;
        *by_sample.entry(sample).or_default() += 1;
    }
    // Optional: admitted domains drawn PPS by their later hits (per phase),
    // with their coverage at admission. The share of draws that are covered
    // is the share of later hits that land on a covered domain (Hansen-
    // Hurwitz), i.e. the redirected requests of an admission-time union
    // cover, without the heavy-tail variance of a uniform draw.
    let kp = opts.num("pps-adm", 0usize);
    let mut pps_out = Value::Null;
    if kp > 0 {
        let n = ck.n;
        let sums = util::dom_sums(&ck.doms, n);
        let seed = opts.num("seed", 20260928u64);
        let (natives, all) = pools(&ck, &sums, &[0, 1]);
        let ctx = Ctx { ck: &ck, n, natives, all, series: None, wait: 0.0, cap: opts.num("cap", 2.0e6f64), samples: opts.num("samples", 20000usize) };
        eprintln!("route-hits: pools built ({:.1} s)", t0.elapsed().as_secs_f64());
        let creator_seq = |id: usize| -> u32 {
            let c = e.creator[id];
            if c == NONE {
                return 0;
            }
            let ix = ck.native_of[c as usize];
            if ix == NONE { 0 } else { ck.recs[ix as usize].seq }
        };
        let sets_a = ["all_earlier_ids", "earlier_non_delegated", "natives_before_creator_commit"];
        let mut rng = Rng::new(seed ^ 0x5151);
        let mut f = opts.get("pps-rows").map(|p| std::io::BufWriter::new(std::fs::File::create(p).unwrap()));
        let mut summ = serde_json::Map::new();
        for (ph, phname) in [(1u8, "Route"), (0u8, "Apply")] {
            let adm: Vec<usize> = (0..nd).filter(|&id| e.creator[id] != NONE && !sums[id].empty && ck.doms[id].phase == ph).collect();
            let w: Vec<f64> = adm.iter().map(|&id| { let (tr, al) = later(id); (tr + al) as f64 }).collect();
            let wsum: f64 = w.iter().sum();
            let plan = pps(&w, kp, &mut rng);
            let evs: Vec<(usize, usize, Vec<Option<Eval>>)> = plan
                .par_iter()
                .map(|&(ix, mult)| {
                    let id = adm[ix];
                    let mut rng = Rng::new(seed ^ (id as u64).wrapping_mul(0x2545F4914F6CDD1D));
                    let v = vec![
                        ctx.eval(id, NONE, 0.0, "all_earlier_ids", &mut rng),
                        ctx.eval(id, NONE, 0.0, "earlier_non_delegated", &mut rng),
                        ctx.eval(id, creator_seq(id), 0.0, "natives_before_commit", &mut rng),
                    ];
                    (id, mult, v)
                })
                .collect();
            let mut cov = [0f64; 3];
            let mut unev = [0f64; 3];
            for (id, mult, v) in &evs {
                for si in 0..3 {
                    match &v[si] {
                        None => unev[si] += *mult as f64,
                        Some(ev) => cov[si] += *mult as f64 * (ev.uncovered == 0.0) as u8 as f64,
                    }
                }
                if let Some(f) = f.as_mut() {
                    let (tr, al) = later(*id);
                    let ev: BTreeMap<&str, Value> = sets_a
                        .iter()
                        .zip(v)
                        .map(|(s, ev)| (*s, ev.as_ref().map_or(Value::Null, |e| json!({"uncovered": e.uncovered, "points": e.points, "exact": e.exact,
                            "single": e.single_container, "anchors_used": e.anchors_used, "candidates": e.candidates}))))
                        .collect();
                    writeln!(f, "{}", json!({"phase": phname, "id": id, "mult": mult, "later_transition": tr, "later_alias": al,
                        "admission_gen": util::id_gen(&ck.m, *id), "class": CLS[class(*id)], "evals": ev})).unwrap();
                }
            }
            let k = kp as f64;
            summ.insert(phname.into(), json!({"admitted": adm.len(), "later_hits_total": wsum, "draws": kp, "distinct": evs.len(),
                "hit_weighted_covered_share": sets_a.iter().zip(cov).map(|(s, c)| (s.to_string(), json!(c / k))).collect::<BTreeMap<_, _>>(),
                "hit_weighted_unevaluated_share": sets_a.iter().zip(unev).map(|(s, c)| (s.to_string(), json!(c / k))).collect::<BTreeMap<_, _>>(),
                "redirected_hits_estimate(later_hits_total x covered share)": sets_a.iter().zip(cov).map(|(s, c)| (s.to_string(), json!(wsum * c / k))).collect::<BTreeMap<_, _>>()}));
            eprintln!("route-hits: PPS {phname} done ({:.1} s)", t0.elapsed().as_secs_f64());
        }
        pps_out = json!({"draws_per_phase": kp, "weight": "later hits (transition + alias, creator and self edges excluded)", "by_phase": summ});
    }
    let report = json!({"dir": dir, "generation": ck.m.generation, "rows_in": rin, "rows": nrows, "rows_by_sample": by_sample,
        "edges": e.counts, "model[E]": model.summary(), "pps_by_later_hits_at_admission": pps_out,
        "in_edge_totals[transition from Route, transition from Apply, alias, self]": (0..4).map(|k| ind[k].iter().map(|&x| x as u64).sum::<u64>()).collect::<Vec<_>>(),
        "later_hits_exact(admitted domains; excludes the creator edge and self edges)": totals});
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
