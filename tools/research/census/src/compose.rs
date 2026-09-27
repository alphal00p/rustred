//! stats, compose (native-pending composition and pending vs committed
//! envelope) and cost (per-owner cost exponents).
use crate::ckpt::{self, L_COMPLETED, L_DELEGATE, L_RESERVED, L_STARTED, L_UNRESERVED};
use crate::geom::{self, Dom};
use crate::recs::{K_DELEGATED, K_NATIVE};
use crate::util::{self, Ckpt, DomSum, Opts, ols};
use rayon::prelude::*;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

pub fn stats(dir: &Path, opts: &Opts) {
    let ck = util::load(dir, opts, true);
    let n = ck.n;
    let mut flags = [0usize; 8];
    for &f in &ck.nodes {
        flags[(f & 7) as usize] += 1;
    }
    let mut lstates = [0usize; 7];
    if let Some(l) = &ck.ledger {
        for e in &l.entries {
            lstates[e.state as usize] += 1;
        }
    }
    let mut kinds: BTreeMap<String, (usize, f64)> = BTreeMap::new();
    let mut nonmono = 0usize;
    let mut maxid = 0u32;
    let mut behind = Vec::new();
    for r in &ck.recs {
        let k = format!("{}|{}", r.kind, r.phase);
        let e = kinds.entry(k).or_default();
        e.0 += 1;
        e.1 += r.seconds as f64;
        if crate::recs::is_native(r.kind) {
            if r.id < maxid {
                nonmono += 1;
                behind.push((maxid - r.id) as f64);
            }
            maxid = maxid.max(r.id);
        }
    }
    let q = util::quantiles(&mut behind, &[0.5, 0.9, 0.99, 0.999, 1.0]);
    let sums = util::dom_sums(&ck.doms, n);
    let mut phase = [(0usize, 0usize, 0usize, 0f64); 2];
    for (d, s) in ck.doms.iter().zip(&sums) {
        let p = &mut phase[d.phase as usize];
        p.0 += 1;
        if s.infinite {
            p.1 += 1;
        } else if s.empty {
            p.2 += 1;
        } else {
            p.3 += s.points;
        }
    }
    // Points per Apply domain by decade: natives vs other (pending or delegated).
    let mut dec: BTreeMap<(u8, i32), (usize, f64, f64)> = BTreeMap::new();
    for (id, (d, s)) in ck.doms.iter().zip(&sums).enumerate() {
        if d.phase != 0 || s.infinite || s.empty {
            continue;
        }
        let cls = if ck.native_of[id] != u32::MAX { 0u8 } else { match ck.ledger.as_ref().map(|l| l.entries[id].state) { Some(ckpt::L_UNRESERVED | ckpt::L_RESERVED | ckpt::L_STARTED) => 1, _ => 2 } };
        let b = s.points.log10().floor() as i32;
        let e = dec.entry((cls, b)).or_default();
        e.0 += 1;
        e.1 += s.points;
        if cls == 0 {
            e.2 += ck.recs[ck.native_of[id] as usize].seconds as f64;
        }
    }
    let dec: Vec<Value> = dec.iter().map(|((c, b), e)| { let cls = ["native", "pending", "delegated"][*c as usize]; json!([cls, b, e.0, e.1, e.2]) }).collect();
    let (live, buckets) = ckpt::live(&ck.m, ck.doms.len());
    let nlive = live.iter().filter(|&&x| x).count();
    let out = json!({
        "apply_points_by_class_decade[class, log10, count, points, native_seconds]": dec,
        "dir": dir, "generation": ck.m.generation, "arity": n, "policy": ck.m.policy,
        "domains": ck.doms.len(), "records": ck.recs.len(), "index_buckets": buckets, "live_candidates": nlive,
        "node_flags_count_by_value(bit0 sealed, bit1 inspected, bit2 closed)": flags,
        "ledger_states[unres,res,started,completed,failed,cancelled,delegate]": lstates,
        "ledger": ck.ledger.as_ref().map(|l| json!({"ready": l.ready, "cursor": l.cursor, "lookahead": l.lookahead,
            "reserved_through": l.reserved_through, "transfers": l.transfers,
            "native_publications": l.native_publications, "delegated_publications": l.delegated_publications,
            "protected_initial_prefix": l.protected_initial_prefix})),
        "records_by_kind|phase(count,seconds)": kinds,
        "native_records_out_of_id_order": nonmono,
        "native_id_lag_quantiles[0.5,0.9,0.99,0.999,max]": q,
        "domains_by_phase[count,infinite,empty,finite_points]": {"Apply": phase[0], "Route": phase[1]},
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}

/// Per-owner Apply cost law seconds = exp(a) * points^b fitted on natives.
pub struct CostLaw {
    pub per_owner: HashMap<u16, (f64, f64, f64, usize)>,
    pub pooled: (f64, f64, f64, usize),
    /// Binned law per owner: (ln mean points, ln mean seconds) per half-decade
    /// of points with >= 20 natives, ascending.
    pub binned: HashMap<u16, Vec<(f64, f64)>>,
    pub binned_pooled: Vec<(f64, f64)>,
}

fn bins(v: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut m: BTreeMap<i64, (usize, f64, f64)> = BTreeMap::new();
    for &(lp, ls) in v {
        let k = (lp / std::f64::consts::LN_10 * 2.0).floor() as i64;
        let e = m.entry(k).or_default();
        e.0 += 1;
        e.1 += lp.exp();
        e.2 += ls.exp();
    }
    m.values().filter(|e| e.0 >= 20).map(|e| ((e.1 / e.0 as f64).ln(), (e.2 / e.0 as f64).ln())).collect()
}

fn interp(b: &[(f64, f64)], x: f64) -> f64 {
    if b.len() == 1 {
        return b[0].1;
    }
    if x <= b[0].0 {
        return b[0].1;
    }
    for w in b.windows(2) {
        if x <= w[1].0 {
            let f = (x - w[0].0) / (w[1].0 - w[0].0);
            return w[0].1 + f * (w[1].1 - w[0].1);
        }
    }
    let (p, q) = (b[b.len() - 2], b[b.len() - 1]);
    let slope = ((q.1 - p.1) / (q.0 - p.0)).clamp(0.5, 2.0);
    q.1 + slope * (x - q.0)
}

pub fn cost_law(ck: &Ckpt, sums: &[DomSum], min_gen: u8) -> CostLaw {
    let mut by: HashMap<u16, Vec<(f64, f64)>> = HashMap::new();
    let mut all = Vec::new();
    for r in &ck.recs {
        if !crate::recs::is_native(r.kind) || r.phase != 0 || r.gen < min_gen {
            continue;
        }
        let s = &sums[r.id as usize];
        if s.infinite || s.empty || s.points <= 0.0 || r.seconds <= 0.0 {
            continue;
        }
        let p = (s.points.ln(), (r.seconds as f64).ln());
        by.entry(ck.doms[r.id as usize].owner).or_default().push(p);
        all.push(p);
    }
    CostLaw {
        binned: by.iter().map(|(k, v)| (*k, bins(v))).filter(|(_, b)| b.len() >= 2).collect(),
        binned_pooled: bins(&all),
        per_owner: by.into_iter().map(|(k, v)| (k, ols(&v))).collect(),
        pooled: ols(&all),
    }
}

impl CostLaw {
    /// Predicted seconds from the binned (half-decade mean) law [E].
    pub fn predict_binned(&self, owner: u16, points: f64) -> f64 {
        let b = self.binned.get(&owner).unwrap_or(&self.binned_pooled);
        if b.is_empty() {
            return self.predict(owner, points);
        }
        interp(b, points.max(1.0).ln()).exp()
    }
    /// Predicted seconds for an Apply domain of this owner [E].
    pub fn predict(&self, owner: u16, points: f64) -> f64 {
        let (b, a, _, n) = self.per_owner.get(&owner).copied().unwrap_or(self.pooled);
        let (b, a) = if n >= 30 && b.is_finite() { (b, a) } else { (self.pooled.0, self.pooled.1) };
        (a + b * points.max(1.0).ln()).exp()
    }
}

/// Weighted least-squares slope of ln(mean seconds) on ln(mean points) over
/// decade bins (decade, n, sum points, sum seconds) with n >= 10, mean
/// seconds > 0 and decade < `below`; weight n. NaN with fewer than 2 bins.
fn binmean_wls(bins: impl Iterator<Item = (i32, usize, f64, f64)>, below: i32) -> f64 {
    let v: Vec<(f64, f64, f64)> = bins
        .filter(|&(k, n, p, s)| k < below && n >= 10 && p > 0.0 && s > 0.0)
        .map(|(_, n, p, s)| (n as f64, (p / n as f64).ln(), (s / n as f64).ln()))
        .collect();
    if v.len() < 2 {
        return f64::NAN;
    }
    let w: f64 = v.iter().map(|e| e.0).sum();
    let mx = v.iter().map(|e| e.0 * e.1).sum::<f64>() / w;
    let my = v.iter().map(|e| e.0 * e.2).sum::<f64>() / w;
    let sxx: f64 = v.iter().map(|e| e.0 * (e.1 - mx) * (e.1 - mx)).sum();
    let sxy: f64 = v.iter().map(|e| e.0 * (e.1 - mx) * (e.2 - my)).sum();
    sxy / sxx
}

pub fn cost(dir: &Path, opts: &Opts) {
    let ck = util::load(dir, opts, true);
    let n = ck.n;
    let sums = util::dom_sums(&ck.doms, n);
    // Per owner: all natives, by generation, and by points decade.
    #[derive(Default)]
    struct O {
        count: usize,
        seconds: f64,
        points: f64,
        succ: f64,
        xy: Vec<(f64, f64)>,
        xs: Vec<(f64, f64)>,
        dec: BTreeMap<i32, (usize, f64, f64, f64)>,
        gen: BTreeMap<u8, Vec<(f64, f64)>>,
        /// Decade bins per record generation: (n, sum points, sum seconds).
        gdec: BTreeMap<u8, BTreeMap<i32, (usize, f64, f64)>>,
    }
    let mut by: HashMap<(u8, u16), O> = HashMap::new();
    for r in &ck.recs {
        if !crate::recs::is_native(r.kind) {
            continue;
        }
        let d = &ck.doms[r.id as usize];
        let s = &sums[r.id as usize];
        let o = by.entry((r.phase, d.owner)).or_default();
        o.count += 1;
        o.seconds += r.seconds as f64;
        o.succ += r.successors as f64;
        if s.infinite || s.empty || s.points <= 0.0 {
            continue;
        }
        o.points += s.points;
        if r.phase != 0 {
            continue;
        }
        let dec = s.points.log10().floor() as i32;
        let e = o.dec.entry(dec).or_default();
        e.0 += 1;
        e.1 += s.points;
        e.2 += r.seconds as f64;
        e.3 += r.successors as f64;
        let g = o.gdec.entry(r.gen).or_default().entry(dec).or_default();
        g.0 += 1;
        g.1 += s.points;
        g.2 += r.seconds as f64;
        if r.seconds > 0.0 {
            let p = (s.points.ln(), (r.seconds as f64).ln());
            o.xy.push(p);
            o.gen.entry(r.gen).or_default().push(p);
        }
        if r.successors > 0 {
            o.xs.push((s.points.ln(), (r.successors as f64).ln()));
        }
    }
    let total_apply: f64 = by.iter().filter(|(k, _)| k.0 == 0).map(|(_, o)| o.seconds).sum();
    let mut rows: Vec<_> = by.iter().filter(|(k, _)| k.0 == 0).collect();
    rows.sort_by(|a, b| b.1.seconds.partial_cmp(&a.1.seconds).unwrap());
    let mut out = Vec::new();
    let mut pooled = Vec::new();
    for ((_, owner), o) in &rows {
        pooled.extend_from_slice(&o.xy);
        let (b, a, r2, m) = ols(&o.xy);
        let (bs, _, r2s, _) = ols(&o.xs);
        // Top-decade exponent: slope between mean log-seconds of the two
        // highest populated decades with >= 20 natives.
        let decs: Vec<_> = o.dec.iter().filter(|(_, e)| e.0 >= 20).collect();
        let top = if decs.len() >= 2 {
            let (d1, e1) = decs[decs.len() - 2];
            let (d2, e2) = decs[decs.len() - 1];
            let _ = (d1, d2);
            ((e2.2 / e2.0 as f64).ln() - (e1.2 / e1.0 as f64).ln())
                / ((e2.1 / e2.0 as f64).ln() - (e1.1 / e1.0 as f64).ln())
        } else {
            f64::NAN
        };
        let gens: BTreeMap<u8, (f64, f64, usize, f64)> = o
            .gen
            .iter()
            .map(|(g, v)| {
                let (b, _, r2, m) = ols(v);
                let bw = o.gdec.get(g).map_or(f64::NAN, |d| binmean_wls(d.iter().map(|(k, e)| (*k, e.0, e.1, e.2)), i32::MAX));
                (*g, (b, r2, m, bw))
            })
            .collect();
        // The plan's estimator (lens tool perfskeptic/slopes.py): weighted
        // least squares of ln(mean seconds) on ln(mean points) over decade
        // bins with >= 10 natives, weight = natives per bin.
        let binmean = binmean_wls(o.dec.iter().map(|(k, e)| (*k, e.0, e.1, e.2)), i32::MAX);
        let binmean_lt5 = binmean_wls(o.dec.iter().map(|(k, e)| (*k, e.0, e.1, e.2)), 5);
        let lt5: Vec<(f64, f64)> = o.xy.iter().copied().filter(|p| p.0 < 5.0 * std::f64::consts::LN_10).collect();
        let (b_lt5, _, r2_lt5, m_lt5) = ols(&lt5);
        let dec: BTreeMap<i32, Value> = o
            .dec
            .iter()
            .map(|(k, e)| {
                (*k, json!({"n": e.0, "mean_points": e.1 / e.0 as f64, "mean_seconds": e.2 / e.0 as f64,
                    "mean_successors": e.3 / e.0 as f64, "seconds_share": e.2 / o.seconds}))
            })
            .collect();
        out.push(json!({
            "owner": ck.doms.iter().find(|d| d.owner == *owner).map(|d| d.owner_string(n)),
            "t": owner.count_ones(), "natives": o.count, "seconds": o.seconds,
            "seconds_share": o.seconds / total_apply, "points": o.points, "successors": o.succ,
            "cost_exponent": b, "cost_intercept": a, "cost_r2": r2, "fit_n": m,
            "succ_exponent": bs, "succ_r2": r2s, "top_decade_exponent": top,
            "cost_exponent_by_gen[b,r2,n,b_binmean_wls]": gens, "decades": dec,
            "cost_exponent_binmean_wls": binmean, "cost_exponent_binmean_wls_pts_lt_1e5": binmean_lt5,
            "cost_exponent_ols_pts_lt_1e5[b,r2,n]": [b_lt5, r2_lt5, m_lt5],
        }));
    }
    let (b, a, r2, m) = ols(&pooled);
    let mut pdec: BTreeMap<i32, (usize, f64, f64)> = BTreeMap::new();
    for (_, o) in &rows {
        for (k, e) in &o.dec {
            let p = pdec.entry(*k).or_default();
            p.0 += e.0;
            p.1 += e.1;
            p.2 += e.2;
        }
    }
    let pb = binmean_wls(pdec.iter().map(|(k, e)| (*k, e.0, e.1, e.2)), i32::MAX);
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"apply_seconds": total_apply,
            "pooled": {"cost_exponent": b, "intercept": a, "r2": r2, "n": m, "cost_exponent_binmean_wls": pb}, "owners": out}))
        .unwrap()
    );
}

/// Composition of the native-pending set and the pending vs committed
/// envelope in (A, R, P, D).
pub fn compose(dir: &Path, opts: &Opts) {
    let ck = util::load(dir, opts, true);
    let n = ck.n;
    let ledger = ck.ledger.as_ref().expect("ledger section");
    let sums = util::dom_sums(&ck.doms, n);
    let (live, _) = ckpt::live(&ck.m, ck.doms.len());
    let law = cost_law(&ck, &sums, 0);
    let is_pending = |id: usize| {
        matches!(ledger.entries[id].state, L_UNRESERVED | L_RESERVED | L_STARTED)
    };
    let committed_native = |id: usize| ck.native_of[id] != u32::MAX;
    // Committed envelope per (phase, owner): extrema over natives, and the
    // (A, R) level set touched by native points (Apply only).
    #[derive(Default, Clone)]
    struct Env {
        n: usize,
        ext: [i32; 8],
        levels: std::collections::BTreeSet<(i32, i32)>,
    }
    let mut env: HashMap<u32, Env> = HashMap::new();
    for id in 0..ck.doms.len() {
        if !committed_native(id) {
            continue;
        }
        let d = &ck.doms[id];
        let s = &sums[id];
        if s.empty {
            continue;
        }
        let e = env.entry(d.bucket()).or_insert_with(|| Env {
            n: 0,
            ext: [i32::MAX, i32::MIN, i32::MAX, i32::MIN, i32::MAX, i32::MIN, i32::MAX, i32::MIN],
            levels: Default::default(),
        });
        e.n += 1;
        for k in 0..8 {
            e.ext[k] = if k % 2 == 0 { e.ext[k].min(s.ext[k]) } else { e.ext[k].max(s.ext[k]) };
        }
        if d.phase == 0 {
            if let Some(l) = geom::levels(d, n) {
                for sa in 0..l.da.len() {
                    for sr in 0..l.dr.len() {
                        if l.at(sa, sr) > 0 {
                            e.levels.insert((sa as i32 + l.t, sr as i32));
                        }
                    }
                }
            }
        }
    }
    // Staircase (dominance hull) of committed levels per Apply owner.
    let stair = |e: &Env, a: i32, r: i32| e.levels.iter().any(|&(a2, r2)| a <= a2 && r <= r2);
    #[derive(Default)]
    struct Agg {
        count: usize,
        points: f64,
        infinite: usize,
        pred_seconds: f64,
    }
    impl Agg {
        fn add(&mut self, s: &DomSum, pred: f64) {
            self.count += 1;
            if s.infinite {
                self.infinite += 1;
            } else {
                self.points += s.points;
            }
            self.pred_seconds += pred;
        }
        fn v(&self) -> Value {
            json!([self.count, self.points, self.infinite, self.pred_seconds])
        }
    }
    let mut tabs: BTreeMap<String, BTreeMap<String, Agg>> = BTreeMap::new();
    let mut add = |tab: &str, key: String, s: &DomSum, pred: f64| {
        tabs.entry(tab.to_string()).or_default().entry(key).or_default().add(s, pred);
    };
    // Envelope escape accounting (Apply): pending points at levels outside
    // the committed level set / outside its staircase.
    let mut esc = [0f64; 6]; // [pending pts, outside set pts, outside stair pts, domains, dom any outside set, dom any outside stair]
    let mut esc_owner: HashMap<u16, [f64; 6]> = HashMap::new();
    let mut ext_escape: BTreeMap<String, [usize; 5]> = BTreeMap::new(); // phase -> [n, A>, R>, P>, D<]
    let cursor = ledger.cursor;
    let nd = ck.doms.len();
    for id in 0..nd {
        if !is_pending(id) {
            continue;
        }
        let d = &ck.doms[id];
        let s = &sums[id];
        let ph = if d.phase == 0 { "Apply" } else { "Route" };
        let pred = if d.phase == 0 && !s.infinite && !s.empty { law.predict_binned(d.owner, s.points) } else { 0.0 };
        let st = match ledger.entries[id].state {
            L_UNRESERVED => "unreserved",
            L_RESERVED => "reserved",
            _ => "started",
        };
        add("phase|state|live", format!("{ph}|{st}|{}", live[id]), s, pred);
        add("phase|t", format!("{ph}|{:02}", d.t()), s, pred);
        add("phase|rank", format!("{ph}|{:02}", if d.rank == geom::INF8 { 99 } else { d.rank as i32 }), s, pred);
        if !s.infinite && !s.empty {
            add("phase|Pmax", format!("{ph}|{:02}", s.ext[5]), s, pred);
            add("phase|Amax", format!("{ph}|{:02}", s.ext[1]), s, pred);
            add("phase|Rmax", format!("{ph}|{:02}", s.ext[3]), s, pred);
            add("phase|Dmin", format!("{ph}|{:+03}", s.ext[6]), s, pred);
            let pts = s.points;
            let b = if pts <= 0.0 { -1 } else { pts.log10().floor() as i32 };
            add("phase|log10points", format!("{ph}|{b:02}"), s, pred);
        } else if s.empty {
            add("phase|empty", ph.to_string(), s, pred);
        }
        add("phase|admission_gen", format!("{ph}|{}", util::id_gen(&ck.m, id)), s, pred);
        let rel = if id < cursor { "before_cursor".to_string() } else {
            let dd = id - cursor;
            format!("after_cursor_1e{}", if dd == 0 { 0 } else { (dd as f64).log10().floor() as i32 })
        };
        add("phase|queue_position", format!("{ph}|{rel}"), s, pred);
        if d.phase == 0 {
            add("apply_owner", d.owner_string(n), s, pred);
        }
        // Envelope comparison.
        let e = env.get(&d.bucket());
        let x = ext_escape.entry(ph.to_string()).or_default();
        x[0] += 1;
        if let Some(e) = e {
            if !s.empty {
                x[1] += (s.ext[1] > e.ext[1]) as usize;
                x[2] += (s.ext[3] > e.ext[3]) as usize;
                x[3] += (s.ext[5] > e.ext[5]) as usize;
                x[4] += (s.ext[6] < e.ext[6]) as usize;
            }
        } else {
            x[1] += 1;
            x[2] += 1;
            x[3] += 1;
            x[4] += 1;
        }
        if d.phase == 0 && !s.infinite && !s.empty {
            let l = geom::levels(d, n).unwrap();
            let (mut out_set, mut out_stair) = (0f64, 0f64);
            for sa in 0..l.da.len() {
                for sr in 0..l.dr.len() {
                    let c = l.at(sa, sr) as f64;
                    if c == 0.0 {
                        continue;
                    }
                    let (a, r) = (sa as i32 + l.t, sr as i32);
                    let (in_set, in_stair) = match e {
                        Some(e) => (e.levels.contains(&(a, r)), stair(e, a, r)),
                        None => (false, false),
                    };
                    if !in_set {
                        out_set += c;
                    }
                    if !in_stair {
                        out_stair += c;
                    }
                }
            }
            let v = [s.points, out_set, out_stair, 1.0, (out_set > 0.0) as u8 as f64, (out_stair > 0.0) as u8 as f64];
            let o = esc_owner.entry(d.owner).or_default();
            for k in 0..6 {
                esc[k] += v[k];
                o[k] += v[k];
            }
        }
    }
    let tabs: BTreeMap<String, BTreeMap<String, Value>> = tabs
        .into_iter()
        .map(|(k, v)| (k, v.into_iter().map(|(k2, a)| (k2, a.v())).collect()))
        .collect();
    let mut owners: Vec<_> = esc_owner.into_iter().collect();
    owners.sort_by(|a, b| b.1[0].partial_cmp(&a.1[0]).unwrap());
    let owners: Vec<Value> = owners
        .iter()
        .take(80)
        .map(|(o, v)| {
            let e = env.get(&(*o as u32));
            json!({"owner": Dom { owner: *o, ..Default::default() }.owner_string(n),
                "pending_points": v[0], "outside_level_set_points": v[1], "outside_staircase_points": v[2],
                "pending_domains": v[3], "domains_touching_outside_set": v[4], "domains_touching_outside_staircase": v[5],
                "committed_envelope[Amin,Amax,Rmin,Rmax,Pmin,Pmax,Dmin,Dmax]": e.map(|e| e.ext),
                "committed_levels": e.map(|e| e.levels.len())})
        })
        .collect();
    let out = json!({
        "dir": dir, "generation": ck.m.generation,
        "pending_native_total": (0..nd).filter(|&i| is_pending(i)).count(),
        "ledger_cursor": cursor, "ledger_reserved_through": ledger.reserved_through,
        "tables(count, points, infinite, predicted_apply_seconds[E])": tabs,
        "extrema_escape_by_phase[n, Amax>committed, Rmax>committed, Pmax>committed, Dmin<committed]": ext_escape,
        "apply_level_escape[pending_points, outside_set_points, outside_staircase_points, domains, touching_outside_set, touching_outside_staircase]": esc,
        "apply_level_escape_by_owner": owners,
        "cost_law_pooled[b,a,r2,n]": [law.pooled.0, law.pooled.1, law.pooled.2, law.pooled.3],
        "committed_native_count": ck.native_of.iter().filter(|&&x| x != u32::MAX).count(),
        "delegated_records": ck.recs.iter().filter(|r| r.kind == K_DELEGATED).count(),
        "ledger_delegate_entries": ledger.entries.iter().filter(|e| e.state == L_DELEGATE).count(),
        "ledger_completed_entries": ledger.entries.iter().filter(|e| e.state == L_COMPLETED).count(),
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
    let _ = sums.par_iter().count();
}

#[cfg(test)]
mod tests {
    use super::binmean_wls;
    /// Exact power law on bin means: slope recovered; bins with < 10 natives
    /// and bins at or above the cut are ignored.
    #[test]
    fn binmean_wls_recovers_power_law() {
        let bins: Vec<(i32, usize, f64, f64)> = (0..6)
            .map(|k| {
                let n = if k == 5 { 3 } else { 100 + 10 * k as usize };
                let p = 10f64.powi(k) * 3.0;
                (k, n, p * n as f64, 0.01 * p.powf(0.8) * n as f64)
            })
            .collect();
        let b = binmean_wls(bins.iter().copied(), i32::MAX);
        assert!((b - 0.8).abs() < 1e-9, "{b}");
        let mut skew = bins.clone();
        skew[5].1 = 3; // still excluded
        skew[4].3 *= 10.0;
        let b4 = binmean_wls(skew.iter().copied(), 4);
        assert!((b4 - 0.8).abs() < 1e-9, "{b4}");
        assert!(binmean_wls(bins.iter().copied().take(1), i32::MAX).is_nan());
    }
}
