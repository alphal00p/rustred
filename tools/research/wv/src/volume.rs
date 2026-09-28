//! D6 in domain-volume units: what a symmetry-canonicalising admission would
//! have avoided on the saved graph, given the per-ID merge flags of `wv sym`
//! (`--flags-out`, bit0 = the image under a verified automorphism is contained
//! in an admitted domain with a smaller ID).
//!
//! Direct: the flagged domains themselves (they would have been admission
//! hits on the earlier container, so no ID, no ledger obligation, no
//! inspection, no out-edges). Cascade: the least fixpoint of "every in-edge
//! source is avoided" over the saved dependency edges (source depends on
//! target), protected initial prefix excluded. The least fixpoint is a lower
//! bound on the cascade on the explored graph; the unexplored future of
//! pending domains is not modelled. Everything is reported by count per class
//! (natives, committed delegated, native-pending, delegate-pending), by
//! native seconds for natives [M] and by predicted seconds for native-pending
//! [E] (per (phase, owner) half-decade binned mean native seconds against
//! lattice points, the census "binned law").
use crate::ckpt::{self, L_DELEGATE, L_RESERVED, L_STARTED, L_UNRESERVED};
use crate::geom;

use crate::util::{self, Opts};
use rayon::prelude::*;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

const C_NATIVE: u8 = 0;
const C_DELEGATED: u8 = 1;
const C_PENDING_NATIVE: u8 = 2;
const C_PENDING_DELEGATE: u8 = 3;
const C_OTHER: u8 = 4;
const CLASS: [&str; 5] = [
    "native",
    "committed_delegated",
    "pending_native",
    "pending_delegate",
    "other",
];

#[derive(Default, Clone)]
struct Sum {
    domains: [u64; 2],
    class: [[u64; 5]; 2],
    native_seconds: [f64; 2],
    pending_predicted_seconds: [f64; 2],
    out_edges: u64,
}

impl Sum {
    fn add(&mut self, phase: usize, class: u8, sec: f64, pred: f64, out_edges: u64) {
        self.domains[phase] += 1;
        self.class[phase][class as usize] += 1;
        if class == C_NATIVE {
            self.native_seconds[phase] += sec;
        }
        if class == C_PENDING_NATIVE {
            self.pending_predicted_seconds[phase] += pred;
        }
        self.out_edges += out_edges;
    }
    fn merge(&mut self, o: &Sum) {
        for p in 0..2 {
            self.domains[p] += o.domains[p];
            for c in 0..5 {
                self.class[p][c] += o.class[p][c];
            }
            self.native_seconds[p] += o.native_seconds[p];
            self.pending_predicted_seconds[p] += o.pending_predicted_seconds[p];
        }
        self.out_edges += o.out_edges;
    }
    fn json(&self, total: Option<&Sum>) -> Value {
        let share = |a: f64, b: f64| if b > 0.0 { a / b } else { 0.0 };
        let mut m = serde_json::Map::new();
        for (p, name) in ["apply", "route"].iter().enumerate() {
            let mut c = serde_json::Map::new();
            c.insert("domains".into(), json!(self.domains[p]));
            for (k, cn) in CLASS.iter().enumerate() {
                c.insert((*cn).into(), json!(self.class[p][k]));
            }
            c.insert("native_seconds".into(), json!(self.native_seconds[p]));
            c.insert(
                "pending_native_predicted_seconds_E".into(),
                json!(self.pending_predicted_seconds[p]),
            );
            if let Some(t) = total {
                c.insert(
                    "share_domains".into(),
                    json!(share(self.domains[p] as f64, t.domains[p] as f64)),
                );
                c.insert(
                    "share_natives".into(),
                    json!(share(self.class[p][0] as f64, t.class[p][0] as f64)),
                );
                c.insert(
                    "share_native_seconds".into(),
                    json!(share(self.native_seconds[p], t.native_seconds[p])),
                );
                c.insert(
                    "share_pending_native".into(),
                    json!(share(self.class[p][2] as f64, t.class[p][2] as f64)),
                );
                c.insert(
                    "share_pending_native_predicted_seconds_E".into(),
                    json!(share(
                        self.pending_predicted_seconds[p],
                        t.pending_predicted_seconds[p]
                    )),
                );
                let pend = |s: &Sum| (s.class[p][2] + s.class[p][3]) as f64;
                c.insert("share_pending_all".into(), json!(share(pend(self), pend(t))));
            }
            m.insert((*name).into(), Value::Object(c));
        }
        let dom = (self.domains[0] + self.domains[1]) as f64;
        m.insert("domains_both_phases".into(), json!(dom));
        m.insert("out_edges".into(), json!(self.out_edges));
        if let Some(t) = total {
            m.insert(
                "share_domains_both_phases".into(),
                json!(share(dom, (t.domains[0] + t.domains[1]) as f64)),
            );
            m.insert(
                "share_out_edges".into(),
                json!(share(self.out_edges as f64, t.out_edges as f64)),
            );
            let nat = |s: &Sum| (s.class[0][0] + s.class[1][0]) as f64;
            m.insert("share_natives_both_phases".into(), json!(share(nat(self), nat(t))));
            let pn = |s: &Sum| (s.class[0][2] + s.class[1][2] + s.class[0][3] + s.class[1][3]) as f64;
            m.insert("share_pending_both_phases".into(), json!(share(pn(self), pn(t))));
        }
        Value::Object(m)
    }
}

pub fn run(dir: &Path, opts: &Opts) {
    let t0 = std::time::Instant::now();
    let ck = util::load(dir, opts, true);
    let n = ck.n;
    let doms = &ck.doms;
    let total = doms.len();
    // --flags A[,B...]: per-ID flag files of separate `wv sym` passes, OR-ed
    // (e.g. an Apply pass and a `--skip-apply 1 --route-all 1` Route pass).
    let mut flags: Vec<u8> = Vec::new();
    for f in opts.get("flags").expect("--flags").split(',') {
        let b = std::fs::read(f).unwrap();
        if flags.is_empty() {
            flags = b;
        } else {
            assert_eq!(b.len(), flags.len(), "flags length {f}");
            flags.iter_mut().zip(b).for_each(|(x, y)| *x |= y);
        }
    }
    assert_eq!(flags.len(), total, "flags length");
    let out_path = opts.get("out").expect("--out");
    let ledger = ck.ledger.as_ref().expect("ledger");
    let prefix = ledger.protected_initial_prefix.unwrap_or(0);
    // Class per ID.
    let mut class = vec![C_OTHER; total];
    let mut has_record = vec![false; total];
    for r in &ck.recs {
        has_record[r.id as usize] = true;
    }
    for id in 0..total {
        if ck.native_of[id] != u32::MAX {
            class[id] = C_NATIVE;
        } else if has_record[id] {
            class[id] = C_DELEGATED;
        }
    }
    for id in 0..total {
        if has_record[id] {
            continue;
        }
        class[id] = match ledger.entries.get(id).map(|e| e.state) {
            Some(L_UNRESERVED) | Some(L_RESERVED) | Some(L_STARTED) => C_PENDING_NATIVE,
            Some(L_DELEGATE) => C_PENDING_DELEGATE,
            _ => C_OTHER,
        };
    }
    let seconds: Vec<f64> = ck
        .native_of
        .iter()
        .map(|&r| {
            if r == u32::MAX {
                0.0
            } else {
                ck.recs[r as usize].seconds as f64
            }
        })
        .collect();
    // Binned law [E]: mean native seconds per (phase, owner, half-decade of points).
    let points: Vec<f64> = doms
        .par_iter()
        .map(|d| geom::points(d, n).map_or(f64::NAN, |p| p as f64))
        .collect();
    let bin = |p: f64| -> i32 {
        if p.is_nan() {
            i32::MAX
        } else {
            (2.0 * p.max(1.0).log10()).floor() as i32
        }
    };
    let mut law: HashMap<(u8, u16, i32), (f64, u64)> = HashMap::new();
    let mut owner_law: HashMap<(u8, u16), (f64, u64)> = HashMap::new();
    for id in 0..total {
        if class[id] != C_NATIVE {
            continue;
        }
        let d = &doms[id];
        let e = law.entry((d.phase, d.owner, bin(points[id]))).or_default();
        e.0 += seconds[id];
        e.1 += 1;
        let o = owner_law.entry((d.phase, d.owner)).or_default();
        o.0 += seconds[id];
        o.1 += 1;
    }
    let predict = |id: usize| -> f64 {
        let d = &doms[id];
        let b = bin(points[id]);
        if let Some(&(s, c)) = law.get(&(d.phase, d.owner, b)) {
            return s / c as f64;
        }
        // nearest populated bin of the same (phase, owner); infinite -> largest
        let mut best: Option<(i32, f64)> = None;
        for (&(ph, ow, bb), &(s, c)) in &law {
            if ph != d.phase || ow != d.owner {
                continue;
            }
            let dist = if b == i32::MAX { -bb } else { (bb - b).abs() };
            if best.is_none_or(|(bd, _)| dist < bd) {
                best = Some((dist, s / c as f64));
            }
        }
        best.map(|x| x.1).unwrap_or_else(|| {
            owner_law
                .get(&(d.phase, d.owner))
                .map_or(0.0, |&(s, c)| s / c as f64)
        })
    };
    let pred: Vec<f64> = (0..total)
        .into_par_iter()
        .map(|id| if class[id] == C_PENDING_NATIVE { predict(id) } else { 0.0 })
        .collect();
    eprintln!("classes and law ({:.1} s)", t0.elapsed().as_secs_f64());

    // Out-edge CSR by source.
    let outdeg: Vec<AtomicU32> = (0..total).map(|_| AtomicU32::new(0)).collect();
    let indeg: Vec<AtomicU32> = (0..total).map(|_| AtomicU32::new(0)).collect();
    let nedges = AtomicU64::new(0);
    ckpt::edges_par(&ck.m, 1 << 20, |_, c| {
        let k = c.len() / 8;
        for i in 0..k {
            let (s, t) = ckpt::edge_at(c, i);
            outdeg[s as usize].fetch_add(1, Ordering::Relaxed);
            indeg[t as usize].fetch_add(1, Ordering::Relaxed);
        }
        nedges.fetch_add(k as u64, Ordering::Relaxed);
    });
    let mut off = vec![0u64; total + 1];
    for i in 0..total {
        off[i + 1] = off[i] + outdeg[i].load(Ordering::Relaxed) as u64;
    }
    let m_edges = off[total] as usize;
    let fill: Vec<AtomicU64> = off[..total].iter().map(|&o| AtomicU64::new(o)).collect();
    let tgt: Vec<AtomicU32> = (0..m_edges).map(|_| AtomicU32::new(0)).collect();
    ckpt::edges_par(&ck.m, 1 << 20, |_, c| {
        for i in 0..c.len() / 8 {
            let (s, t) = ckpt::edge_at(c, i);
            let p = fill[s as usize].fetch_add(1, Ordering::Relaxed);
            tgt[p as usize].store(t, Ordering::Relaxed);
        }
    });
    drop(fill);
    let tgt: Vec<u32> = tgt.into_iter().map(|a| a.into_inner()).collect();
    eprintln!("edges {} CSR ({:.1} s)", m_edges, t0.elapsed().as_secs_f64());

    let phase_of = |id: usize| (doms[id].phase as usize).min(1);
    let tally = |set: &[bool]| -> Sum {
        (0..total)
            .into_par_iter()
            .fold(Sum::default, |mut acc, id| {
                if set[id] {
                    acc.add(
                        phase_of(id),
                        class[id],
                        seconds[id],
                        pred[id],
                        off[id + 1] - off[id],
                    );
                }
                acc
            })
            .reduce(Sum::default, |mut a, b| {
                a.merge(&b);
                a
            })
    };
    let all = vec![true; total];
    let totals = tally(&all);
    let cascade = |seed: &[bool]| -> Vec<bool> {
        let mut avoided = seed.to_vec();
        for id in 0..prefix.min(total) {
            avoided[id] = false;
        }
        let mut live: Vec<u32> = indeg.iter().map(|a| a.load(Ordering::Relaxed)).collect();
        let mut stack: Vec<u32> = (0..total as u32).filter(|&i| avoided[i as usize]).collect();
        while let Some(v) = stack.pop() {
            let v = v as usize;
            for &t in &tgt[off[v] as usize..off[v + 1] as usize] {
                let t = t as usize;
                live[t] -= 1;
                if live[t] == 0 && !avoided[t] && t >= prefix {
                    avoided[t] = true;
                    stack.push(t as u32);
                }
            }
        }
        avoided
    };
    let mut sets: Vec<(&str, Vec<bool>)> = Vec::new();
    let bit = |id: usize, b: u8| flags[id] & b != 0;
    let apply_direct: Vec<bool> = (0..total).map(|i| doms[i].phase == 0 && bit(i, 1)).collect();
    let route_direct: Vec<bool> = (0..total).map(|i| doms[i].phase == 1 && bit(i, 1)).collect();
    let both_direct: Vec<bool> = (0..total).map(|i| bit(i, 1)).collect();
    let apply_upper: Vec<bool> = (0..total).map(|i| doms[i].phase == 0 && bit(i, 2)).collect();
    let route_upper: Vec<bool> = (0..total).map(|i| doms[i].phase == 1 && bit(i, 2)).collect();
    let apply_cascade = cascade(&apply_direct);
    let both_cascade = cascade(&both_direct);
    sets.push(("apply_direct", apply_direct));
    sets.push(("route_direct", route_direct));
    sets.push(("both_direct", both_direct));
    sets.push(("apply_direct_plus_cascade", apply_cascade));
    sets.push(("both_direct_plus_cascade", both_cascade));
    sets.push(("apply_upper_bound_direct", apply_upper));
    sets.push(("route_upper_bound_direct", route_upper));
    let evaluated = [
        (0..total).filter(|&i| doms[i].phase == 0 && bit(i, 0x80)).count(),
        (0..total).filter(|&i| doms[i].phase == 1 && bit(i, 0x80)).count(),
    ];
    let mut res = serde_json::Map::new();
    for (name, set) in &sets {
        res.insert((*name).into(), tally(set).json(Some(&totals)));
        eprintln!("  {name} ({:.1} s)", t0.elapsed().as_secs_f64());
    }
    let out = json!({
        "schema": "rustred.wv.volume.v1",
        "checkpoint": dir.display().to_string(),
        "generation": ck.m.generation,
        "domains": total,
        "edges": nedges.load(Ordering::Relaxed),
        "protected_initial_prefix": prefix,
        "flags_evaluated": {"apply": evaluated[0], "route": evaluated[1]},
        "totals": totals.json(None),
        "sets": res,
        "labels": {
            "direct": "[M-off] exact containment of the image under a verified automorphism in an earlier-ID admitted domain",
            "cascade": "[M-off lower bound] least fixpoint over saved edges; unexplored futures of pending domains not modelled",
            "pending_native_predicted_seconds_E": "[E] per (phase, owner, half-decade) binned mean native seconds",
        },
        "seconds_elapsed": t0.elapsed().as_secs_f64(),
    });
    std::fs::write(out_path, serde_json::to_string_pretty(&out).unwrap()).unwrap();
    eprintln!("wrote {out_path} ({:.1} s)", t0.elapsed().as_secs_f64());
}
