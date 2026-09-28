//! W0.9(b) offline oracle: adaptive dense/sparse (A, R) level cells on a
//! drained walk closure (read-only).
//!
//! usage: levelcells <result.json|records.jsonl> [thetas=0,0.1,0.2,0.3,0.4,0.5,0.6,0.8,1.01]
//!
//! Scheme (algorithmic lens): per (owner, A, R) level keep the covered point
//! count; once covered/full >= theta the level is promoted to ONE exact level
//! cell inspected in full; below theta points are handled exactly (each
//! distinct point inspected once, as a residual of the admitted domain that
//! first reaches it). Apply phase only; Route unchanged.
//!
//! Static view (final closure): dense = levels with final fill >= theta.
//! Online view (admission ID order of the native Apply records): residual
//! inspections are the records with >= 1 new point in a not-yet-promoted
//! level; a promotion adds one cell inspection of the full level.
//! Both views count only points of the EXACT closure; the unfilled part of a
//! dense level is reported as extra points (new obligations whose successors
//! the exact closure never needed: closure growth is NOT modelled).
//!
//! Cost projection [E]: per-owner law seconds = k * c * points^alpha fitted
//! by OLS on log(seconds) vs log(points) over native Apply records, with k
//! calibrated so the fit reproduces the owner's measured total seconds.
use std::collections::HashMap;
use w0_falsify::*;

struct Level {
    owner: u16,
    a: i64,
    r: i64,
    full: f64,
    distinct: u64,
}

struct Record {
    owner: u16,
    seconds: f64,
    start: usize,
    end: usize,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];
    let thetas: Vec<f64> = args
        .get(2)
        .map(|s| s.split(',').map(|x| x.parse().unwrap()).collect())
        .unwrap_or_else(|| vec![0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.8, 1.01]);

    let mut owners: Vec<(String, Vec<bool>)> = Vec::new();
    let mut owner_idx: HashMap<String, u16> = HashMap::new();
    let mut point_id: HashMap<(u16, u128), u32> = HashMap::new();
    let mut point_level: Vec<u32> = Vec::new();
    let mut level_id: HashMap<(u16, i64, i64), u32> = HashMap::new();
    let mut levels: Vec<Level> = Vec::new();
    let mut pts: Vec<u32> = Vec::new();
    let mut recs: Vec<Record> = Vec::new();
    let (mut route_native, mut route_seconds, mut apply_delegated) = (0usize, 0.0f64, 0usize);
    let mut enum_fail = 0usize;
    let mut last_id: Option<u64> = None;
    let mut nonmono = 0usize;
    let mut string_ids = 0usize;
    let mut max_points_owner: HashMap<u16, u64> = HashMap::new();

    let n = for_each_record(path, |r| {
        let phase = r.phase_str().to_owned();
        if phase == "Route" {
            if r.record_kind == "native_inspection" {
                route_native += 1;
                route_seconds += r.seconds.unwrap_or(0.0);
            }
            return;
        }
        if phase != "Apply" {
            return;
        }
        if r.record_kind != "native_inspection" {
            apply_delegated += 1;
            return;
        }
        match r.id_u64() {
            Some(id) => {
                if last_id.is_some_and(|l| id <= l) {
                    nonmono += 1;
                }
                last_id = Some(id);
            }
            None => string_ids += 1,
        }
        let oi = *owner_idx.entry(r.owner.clone()).or_insert_with(|| {
            owners.push((r.owner.clone(), r.owner_bits()));
            (owners.len() - 1) as u16
        });
        let bits = owners[oi as usize].1.clone();
        let start = pts.len();
        let ok = enumerate(&bits, &r.lower, &r.upper, r.rank, r.pb(), &mut |key| {
            let next = point_id.len() as u32;
            let pid = *point_id.entry((oi, key)).or_insert_with(|| {
                let (a, r) = level_of(&bits, key);
                let nl = levels.len() as u32;
                let lid = *level_id.entry((oi, a, r)).or_insert_with(|| {
                    levels.push(Level { owner: oi, a, r, full: level_size(&bits, a, r), distinct: 0 });
                    nl
                });
                levels[lid as usize].distinct += 1;
                point_level.push(lid);
                next
            });
            pts.push(pid);
        });
        if !ok {
            enum_fail += 1;
        }
        let np = (pts.len() - start) as u64;
        let m = max_points_owner.entry(oi).or_default();
        *m = (*m).max(np);
        recs.push(Record { owner: oi, seconds: r.seconds.unwrap_or(0.0), start, end: pts.len() });
    });

    let distinct = point_id.len() as f64;
    let today_points = pts.len() as f64;
    let today_secs: f64 = recs.iter().map(|r| r.seconds).sum();
    println!("records {n} apply_native {} apply_delegated {apply_delegated} route_native {route_native} route_seconds {route_seconds:.1}", recs.len());
    println!("checks enum_fail={enum_fail} nonmonotone_ids={nonmono} string_ids={string_ids} owners={} levels={}", owners.len(), levels.len());
    println!(
        "TODAY apply_inspections={} inspected_points={today_points:.4e} distinct_points={distinct:.4e} overlap={:.2}x apply_seconds={today_secs:.1}",
        recs.len(),
        today_points / distinct
    );

    // Per-owner cost law fit: log s = log c + alpha log p (records with s > 0, p > 0).
    let mut fit: Vec<(f64, f64, f64)> = vec![(0.0, 1.0, 1.0); owners.len()]; // (log c, alpha, k)
    for (oi, (mask, _)) in owners.iter().enumerate() {
        let (mut sx, mut sy, mut sxx, mut sxy, mut cnt) = (0.0, 0.0, 0.0, 0.0, 0.0);
        let mut measured = 0.0;
        for r in recs.iter().filter(|r| r.owner as usize == oi) {
            measured += r.seconds;
            let p = (r.end - r.start) as f64;
            if r.seconds > 0.0 && p > 0.0 {
                let (x, y) = (p.ln(), r.seconds.ln());
                sx += x;
                sy += y;
                sxx += x * x;
                sxy += x * y;
                cnt += 1.0;
            }
        }
        if cnt < 3.0 {
            continue;
        }
        let alpha = (cnt * sxy - sx * sy) / (cnt * sxx - sx * sx);
        let logc = (sy - alpha * sx) / cnt;
        let predicted: f64 = recs
            .iter()
            .filter(|r| r.owner as usize == oi)
            .map(|r| (logc + alpha * ((r.end - r.start).max(1) as f64).ln()).exp())
            .sum();
        let k = if predicted > 0.0 { measured / predicted } else { 1.0 };
        fit[oi] = (logc, alpha, k);
        let n_owner = recs.iter().filter(|r| r.owner as usize == oi).count();
        let pts_owner: usize = recs.iter().filter(|r| r.owner as usize == oi).map(|r| r.end - r.start).sum();
        let dist_owner: u64 = levels.iter().filter(|l| l.owner as usize == oi).map(|l| l.distinct).sum();
        println!(
            "FIT {mask} inspections={n_owner} points={pts_owner} distinct={dist_owner} overlap={:.2}x seconds={measured:.1} alpha={alpha:.3} k={k:.3} max_domain_points={}",
            pts_owner as f64 / dist_owner.max(1) as f64,
            max_points_owner.get(&(oi as u16)).copied().unwrap_or(0)
        );
    }
    let cost = |oi: u16, p: f64| -> f64 {
        let (logc, alpha, k) = fit[oi as usize];
        if p <= 0.0 {
            0.0
        } else {
            k * (logc + alpha * p.ln()).exp()
        }
    };
    let cost_linear = |oi: u16, p: f64, per_point: &HashMap<u16, f64>| p * per_point.get(&oi).copied().unwrap_or(0.0);
    // Linear per-point cost at the owner's largest decade (optimistic for big cells).
    let mut per_point_big: HashMap<u16, f64> = HashMap::new();
    for oi in 0..owners.len() as u16 {
        let maxp = max_points_owner.get(&oi).copied().unwrap_or(0) as f64;
        let (mut s, mut p) = (0.0, 0.0);
        for r in recs.iter().filter(|r| r.owner == oi) {
            let np = (r.end - r.start) as f64;
            if np * 10.0 >= maxp {
                s += r.seconds;
                p += np;
            }
        }
        per_point_big.insert(oi, if p > 0.0 { s / p } else { 0.0 });
    }
    let today_fit: f64 = recs.iter().map(|r| cost(r.owner, (r.end - r.start) as f64)).sum();
    println!("COSTCHECK today_measured={today_secs:.1} today_fit_calibrated={today_fit:.1}");

    // Static view.
    for &theta in &thetas {
        let (mut cells, mut dpts, mut extra, mut spts, mut slev) = (0usize, 0.0, 0.0, 0.0, 0usize);
        let (mut dsecs, mut dsecs_lin, mut max_cell) = (0.0, 0.0, 0.0f64);
        for l in &levels {
            let fill = l.distinct as f64 / l.full;
            if fill >= theta {
                cells += 1;
                dpts += l.full;
                extra += l.full - l.distinct as f64;
                dsecs += cost(l.owner, l.full);
                dsecs_lin += cost_linear(l.owner, l.full, &per_point_big);
                max_cell = max_cell.max(l.full);
            } else {
                slev += 1;
                spts += l.distinct as f64;
            }
        }
        println!(
            "STATIC theta={theta} dense_cells={cells} dense_points={dpts:.4e} dense_extra_points={extra:.4e} sparse_levels={slev} sparse_points={spts:.4e} inspected_points={:.4e} vs_distinct={:.3}x vs_today_points={:.2}x max_cell_points={max_cell:.4e} dense_cell_seconds_E_powerlaw={dsecs:.1} dense_cell_seconds_E_linear_bigdecade={dsecs_lin:.1}",
            dpts + spts,
            (dpts + spts) / distinct,
            today_points / (dpts + spts)
        );
    }

    // Online view in admission (ID) order.
    for &theta in &thetas {
        let mut seen = vec![false; point_level.len()];
        let mut covered = vec![0u64; levels.len()];
        let mut promoted = vec![false; levels.len()];
        let (mut residual_insp, mut sparse_pts, mut cells, mut dense_pts, mut extra) = (0usize, 0.0, 0usize, 0.0, 0.0);
        let (mut fully_union, mut fully_dense, mut mixed_free) = (0usize, 0usize, 0usize);
        let (mut resid_secs, mut dense_secs, mut dense_secs_lin) = (0.0, 0.0, 0.0);
        // Residual cost bounds from the measured record: full record cost (upper
        // bound: residual as expensive as the whole domain) and the share of
        // new points (proportional).
        let (mut resid_full, mut resid_prop) = (0.0, 0.0);
        let mut free_secs_today = 0.0;
        let mut per_owner: HashMap<u16, (usize, f64, usize, f64)> = HashMap::new();
        let mut touched: Vec<u32> = Vec::new();
        let mut max_cell_owner: HashMap<u16, f64> = HashMap::new();
        for r in &recs {
            touched.clear();
            let (mut new, mut dense_hits, mut union_hits) = (0u64, 0u64, 0u64);
            for &pid in &pts[r.start..r.end] {
                let lvl = point_level[pid as usize];
                if promoted[lvl as usize] {
                    dense_hits += 1;
                    continue;
                }
                if seen[pid as usize] {
                    union_hits += 1;
                    continue;
                }
                seen[pid as usize] = true;
                covered[lvl as usize] += 1;
                new += 1;
                touched.push(lvl);
            }
            let e = per_owner.entry(r.owner).or_default();
            if new > 0 {
                residual_insp += 1;
                sparse_pts += new as f64;
                let c = cost(r.owner, new as f64);
                resid_secs += c;
                resid_full += r.seconds;
                resid_prop += r.seconds * new as f64 / (r.end - r.start) as f64;
                e.0 += 1;
                e.1 += new as f64;
            } else {
                free_secs_today += r.seconds;
                if union_hits == 0 {
                    fully_dense += 1;
                } else if dense_hits == 0 {
                    fully_union += 1;
                } else {
                    mixed_free += 1;
                }
            }
            touched.sort_unstable();
            touched.dedup();
            for &lvl in &touched {
                let l = &levels[lvl as usize];
                if !promoted[lvl as usize] && theta <= 1.0 && covered[lvl as usize] as f64 >= theta * l.full {
                    promoted[lvl as usize] = true;
                    cells += 1;
                    dense_pts += l.full;
                    extra += l.full - l.distinct as f64;
                    dense_secs += cost(l.owner, l.full);
                    dense_secs_lin += cost_linear(l.owner, l.full, &per_point_big);
                    e.2 += 1;
                    e.3 += l.full;
                    let m = max_cell_owner.entry(l.owner).or_default();
                    *m = m.max(l.full);
                }
            }
        }
        let inspected = sparse_pts + dense_pts;
        println!(
            "ONLINE theta={theta} inspections={} (residual={residual_insp} dense_cells={cells}) vs_today_inspections={:.3} inspected_points={inspected:.4e} (sparse_new={sparse_pts:.4e} dense_full={dense_pts:.4e} dense_extra_outside_exact_closure={extra:.4e}) vs_distinct={:.3}x vs_today_points={:.2}x free_records(fully_union={fully_union} fully_dense={fully_dense} mixed={mixed_free}) today_seconds_of_free_records={free_secs_today:.1} E_seconds(residual_powerlaw={resid_secs:.1} dense_powerlaw={dense_secs:.1} dense_linear_bigdecade={dense_secs_lin:.1}) E_total_powerlaw={:.1} vs_today_seconds={:.3} E_residual_full_record_cost={resid_full:.1} E_residual_proportional={resid_prop:.1} E_range_vs_today=[{:.3},{:.3}]",
            residual_insp + cells,
            (residual_insp + cells) as f64 / recs.len() as f64,
            inspected / distinct,
            today_points / inspected,
            resid_secs + dense_secs,
            (resid_secs + dense_secs) / today_secs,
            (resid_secs.min(resid_prop) + dense_secs.min(dense_secs_lin)) / today_secs,
            (resid_full + dense_secs.max(dense_secs_lin)) / today_secs
        );
        if (theta - 0.4).abs() < 1e-9 || theta > 1.0 {
            let mut v: Vec<_> = per_owner.iter().collect();
            v.sort_by(|a, b| owners[*a.0 as usize].0.cmp(&owners[*b.0 as usize].0));
            for (oi, (ri, sp, c, dp)) in v {
                let today_n = recs.iter().filter(|r| r.owner == *oi).count();
                let today_p: usize = recs.iter().filter(|r| r.owner == *oi).map(|r| r.end - r.start).sum();
                println!(
                    "ONLINE_OWNER theta={theta} {} residual_inspections={ri} sparse_points={sp:.4e} dense_cells={c} dense_points={dp:.4e} today_inspections={today_n} today_points={today_p} max_cell_points={:.0} max_today_domain_points={}",
                    owners[*oi as usize].0,
                    max_cell_owner.get(oi).copied().unwrap_or(0.0),
                    max_points_owner.get(oi).copied().unwrap_or(0)
                );
            }
        }
    }
}
