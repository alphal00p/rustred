//! Global-potential check (`docs/research/five_loop_auxiliary_scope_2026-09-25.md`,
//! last section): aggregate the observed dependency edges to (phase, owner)
//! nodes with weight w = max over edges of Pmax(target) - Pmax(source), and
//! look for a potential phi with phi(s) >= phi(t) + w(s, t) on every edge,
//! i.e. P + phi non-increasing along every observed transition. It exists iff
//! the aggregated graph has no positive-weight cycle. Observational only: a
//! positive cycle in this conservative graph is "unresolved", not a proof of
//! nontermination, and the absence of one proves nothing about future work.
//!
//! Edge classes: the source is an inspected native (transition edge) or not
//! (alias/transfer edge, skipped). A transition edge is a "creator" edge when
//! it is the first edge into its target (the target was admitted from this
//! parent's successor, so its geometry is the successor image); other
//! transition edges point at pre-existing containers (hits), whose extent
//! over-states the shift.
use crate::ckpt;
use crate::util::{self, Opts};
use rayon::prelude::*;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

const INF: i16 = i16::MAX;

#[derive(Clone, Copy, Default)]
struct W {
    count: u64,
    max_dp: i16,
    max_da: i16,
    max_dr: i16,
    pos_dp: u64,
}
impl W {
    fn new() -> Self {
        W { count: 0, max_dp: i16::MIN, max_da: i16::MIN, max_dr: i16::MIN, pos_dp: 0 }
    }
    fn add(&mut self, dp: i16, da: i16, dr: i16) {
        self.count += 1;
        self.max_dp = self.max_dp.max(dp);
        self.max_da = self.max_da.max(da);
        self.max_dr = self.max_dr.max(dr);
        self.pos_dp += (dp > 0) as u64;
    }
    fn merge(&mut self, o: &W) {
        self.count += o.count;
        self.max_dp = self.max_dp.max(o.max_dp);
        self.max_da = self.max_da.max(o.max_da);
        self.max_dr = self.max_dr.max(o.max_dr);
        self.pos_dp += o.pos_dp;
    }
}

/// Longest-path potentials by Bellman-Ford; returns (potentials, a positive
/// cycle as node list if one exists).
fn potential(nodes: usize, edges: &[(u32, u32, i32)]) -> (Vec<i64>, Option<Vec<u32>>) {
    let mut phi = vec![0i64; nodes];
    let mut pred = vec![u32::MAX; nodes];
    let mut last = None;
    for _ in 0..=nodes {
        let mut changed = None;
        for &(s, t, w) in edges {
            let v = phi[t as usize] + w as i64;
            if v > phi[s as usize] {
                phi[s as usize] = v;
                pred[s as usize] = t;
                changed = Some(s);
            }
        }
        last = changed;
        if changed.is_none() {
            break;
        }
    }
    let Some(mut v) = last else {
        return (phi, None);
    };
    for _ in 0..nodes {
        v = pred[v as usize];
    }
    let mut cyc = vec![v];
    let mut u = pred[v as usize];
    while u != v && cyc.len() <= nodes {
        cyc.push(u);
        u = pred[u as usize];
    }
    (phi, Some(cyc))
}

pub fn run(dir: &Path, opts: &Opts) {
    let m = ckpt::manifest(dir, opts.get("manifest"));
    let n = m.arity;
    let doms = ckpt::domains(&m);
    let flags = ckpt::nodes(&m);
    let nd = doms.len();
    let sums = util::dom_sums(&doms, n);
    // Dense node index per bucket.
    let mut bucket_ix: HashMap<u32, u32> = HashMap::new();
    let mut bucket_of = Vec::new();
    let node: Vec<u32> = doms
        .iter()
        .map(|d| {
            let k = d.bucket();
            *bucket_ix.entry(k).or_insert_with(|| {
                bucket_of.push(k);
                (bucket_of.len() - 1) as u32
            })
        })
        .collect();
    let ext: Vec<[i16; 3]> = sums
        .iter()
        .map(|s| {
            if s.infinite {
                [INF; 3]
            } else if s.empty {
                [INF - 1; 3]
            } else {
                [s.ext[5] as i16, s.ext[1] as i16, s.ext[3] as i16]
            }
        })
        .collect();
    let tsup: Vec<u8> = doms.iter().map(|d| d.t() as u8).collect();
    // Pass 1: first incoming edge per target.
    let first_in: Vec<AtomicU64> = (0..nd).map(|_| AtomicU64::new(u64::MAX)).collect();
    let chunk = 1 << 22;
    ckpt::edges_par(&m, chunk, |base, c| {
        for i in 0..c.len() / 8 {
            let (_, t) = ckpt::edge_at(c, i);
            first_in[t as usize].fetch_min((base + i) as u64, Ordering::Relaxed);
        }
    });
    eprintln!("potential: first-in pass done");
    // Pass 2: aggregate.
    type Agg = (HashMap<(u32, u32), W>, HashMap<(u32, u32), W>, [u64; 8], HashMap<(u8, i16), [u64; 2]>);
    let parts = std::sync::Mutex::new(Vec::<Agg>::new());
    ckpt::edges_par(&m, chunk, |base, c| {
        let mut creator: HashMap<(u32, u32), W> = HashMap::new();
        let mut all: HashMap<(u32, u32), W> = HashMap::new();
        // [edges, alias(source not inspected), transition, creator, infinite/empty endpoint, self-edge, creator dp>0, all dp>0]
        let mut cnt = [0u64; 8];
        // (support class, dp) histogram for creator edges: class 0 same support, 1 strict sub, 2 other; [creator, hit]
        let mut hist: HashMap<(u8, i16), [u64; 2]> = HashMap::new();
        for i in 0..c.len() / 8 {
            let (s, t) = ckpt::edge_at(c, i);
            let (s, t) = (s as usize, t as usize);
            cnt[0] += 1;
            if flags[s] & 2 == 0 {
                cnt[1] += 1;
                continue;
            }
            cnt[2] += 1;
            let is_creator = first_in[t].load(Ordering::Relaxed) == (base + i) as u64;
            cnt[3] += is_creator as u64;
            if s == t {
                cnt[5] += 1;
                continue;
            }
            let (es, et) = (ext[s], ext[t]);
            if es[0] >= INF - 1 || et[0] >= INF - 1 {
                cnt[4] += 1;
                continue;
            }
            let dp = et[0] - es[0];
            let da = et[1] - es[1];
            let dr = et[2] - es[2];
            let key = (node[s], node[t]);
            all.entry(key).or_insert_with(W::new).add(dp, da, dr);
            cnt[7] += (dp > 0) as u64;
            let cls = if tsup[t] == tsup[s] && doms[t].owner == doms[s].owner {
                0
            } else if doms[t].owner & doms[s].owner == doms[t].owner && tsup[t] < tsup[s] {
                1
            } else {
                2
            };
            hist.entry((cls, dp)).or_insert([0; 2])[if is_creator { 0 } else { 1 }] += 1;
            if is_creator {
                creator.entry(key).or_insert_with(W::new).add(dp, da, dr);
                cnt[6] += (dp > 0) as u64;
            }
        }
        parts.lock().unwrap().push((creator, all, cnt, hist));
    });
    let mut creator: HashMap<(u32, u32), W> = HashMap::new();
    let mut all: HashMap<(u32, u32), W> = HashMap::new();
    let mut cnt = [0u64; 8];
    let mut hist: HashMap<(u8, i16), [u64; 2]> = HashMap::new();
    for (c, a, k, h) in parts.into_inner().unwrap() {
        for (key, w) in c {
            creator.entry(key).or_insert_with(W::new).merge(&w);
        }
        for (key, w) in a {
            all.entry(key).or_insert_with(W::new).merge(&w);
        }
        for i in 0..8 {
            cnt[i] += k[i];
        }
        for (key, v) in h {
            let e = hist.entry(key).or_insert([0; 2]);
            e[0] += v[0];
            e[1] += v[1];
        }
    }
    let nodes = bucket_of.len();
    let name = |b: u32| {
        let k = bucket_of[b as usize];
        let d = crate::geom::Dom { owner: (k & 0xffff) as u16, phase: (k >> 16) as u8, ..Default::default() };
        format!("{}:{}", if d.phase == 0 { "A" } else { "R" }, d.owner_string(n))
    };
    let mut report = serde_json::Map::new();
    for (label, g) in [("creator", &creator), ("all_transitions", &all)] {
        for (wname, pick) in [("P", 0usize), ("A", 1), ("R", 2)] {
            let edges: Vec<(u32, u32, i32)> = g
                .iter()
                .map(|(&(s, t), w)| (s, t, [w.max_dp, w.max_da, w.max_dr][pick] as i32))
                .collect();
            let (phi, cyc) = potential(nodes, &edges);
            let pos_self: usize = edges.iter().filter(|e| e.0 == e.1 && e.2 > 0).count();
            let pos_edges: usize = edges.iter().filter(|e| e.2 > 0).count();
            let cyc_desc = cyc.as_ref().map(|c| {
                let mut v = Vec::new();
                // The predecessor chain runs s -> pred[s] = t, i.e. edge s->t.
                for k in 0..c.len() {
                    let s = c[k];
                    let t = c[(k + 1) % c.len()];
                    let w = g.get(&(s, t)).map(|w| [w.max_dp, w.max_da, w.max_dr][pick]);
                    v.push(json!({"from": name(s), "to": name(t), "w": w, "edges": g.get(&(s, t)).map(|w| w.count)}));
                }
                v
            });
            report.insert(
                format!("{label}|{wname}"),
                json!({"node_pairs": edges.len(), "positive_pairs": pos_edges, "positive_self_loops": pos_self,
                    "positive_cycle": cyc.is_some(), "cycle": cyc_desc,
                    "max_potential": if cyc.is_none() { phi.iter().max().copied() } else { None }}),
            );
        }
    }
    let mut h: Vec<_> = hist.into_iter().collect();
    h.sort();
    let hist_json: Vec<_> = h.iter().map(|((c, dp), v)| { let cls = ["same", "sub", "other"][*c as usize]; json!([cls, dp, v[0], v[1]]) }).collect();
    // Largest positive self-loop weights (tight graph).
    let mut selfpos: Vec<_> = creator.iter().filter(|(k, w)| k.0 == k.1 && w.max_dp > 0).collect();
    selfpos.sort_by_key(|(_, w)| -(w.max_dp as i32));
    let selfpos: Vec<_> = selfpos
        .iter()
        .take(20)
        .map(|(k, w)| json!({"node": name(k.0), "max_dP": w.max_dp, "edges": w.count, "edges_dP>0": w.pos_dp}))
        .collect();
    let out = json!({
        "dir": dir, "generation": m.generation, "nodes": nodes,
        "edge_counts[edges, alias_source_not_inspected, transition, creator, infinite_or_empty_endpoint, self_edge, creator_dP>0, transition_dP>0]": cnt,
        "graphs": report,
        "creator_self_loops_with_dP>0(top)": selfpos,
        "dP_histogram[class, dP, creator_edges, hit_edges]": hist_json,
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
