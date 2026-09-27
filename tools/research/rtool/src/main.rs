// Read-only analysis of walk domain records (result.json pretty array or CP5 records JSONL).
// usage: rtool <sum|union> <file> <owner_class.txt> [max_records]
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};

#[derive(Deserialize, Default, Clone, Copy)]
struct PB {
    max_positive_power: Option<u64>,
    min_power_difference: Option<i64>,
    max_power_difference: Option<i64>,
}

#[derive(Deserialize)]
struct Rec {
    #[allow(dead_code)]
    id: usize,
    owner: String,
    phase: Option<String>,
    lower: Vec<u64>,
    upper: Vec<Option<u64>>,
    rank: Option<u32>,
    power_bounds: Option<PB>,
    record_kind: String,
    seconds: Option<f64>,
    stats: Option<serde_json::Value>,
    #[allow(dead_code)]
    representative_id: Option<usize>,
}

fn dist(bounds: &[(u64, Option<u64>)], cap: Option<i64>) -> Option<Vec<u128>> {
    let cap = match cap {
        Some(c) => c,
        None => {
            if bounds.iter().any(|b| b.1.is_none()) {
                return None;
            }
            bounds.iter().map(|b| b.1.unwrap() as i64).sum()
        }
    };
    if cap < 0 {
        return Some(vec![]);
    }
    let cap = cap as usize;
    let mut counts = vec![0u128; cap + 1];
    counts[0] = 1;
    for &(lo, hi) in bounds {
        let lo = lo as usize;
        let hi2 = hi.map_or(cap, |h| (h as usize).min(cap));
        let mut new = vec![0u128; cap + 1];
        if lo > hi2 {
            return Some(new);
        }
        let mut pre = vec![0u128; cap + 2];
        for s in 0..=cap {
            pre[s + 1] = pre[s] + counts[s];
        }
        for s in 0..=cap {
            if s < lo {
                continue;
            }
            let b = s - lo;
            let a = s.saturating_sub(hi2);
            new[s] = pre[b + 1] - pre[a];
        }
        counts = new;
    }
    Some(counts)
}

fn limits(owner: &[bool], rank: Option<u32>, pb: PB) -> (Option<i64>, Option<i64>) {
    let t = owner.iter().filter(|&&b| b).count() as i64;
    let mut acap = pb.max_positive_power.map(|a| a as i64 - t);
    let mut rcap = rank.map(|r| r as i64);
    if acap.is_none() {
        if let (Some(d), Some(r)) = (pb.max_power_difference, rcap) {
            acap = Some(r + d - t);
        }
    }
    if rcap.is_none() {
        if let (Some(d), Some(a)) = (pb.min_power_difference, acap) {
            rcap = Some(a + t - d);
        }
    }
    (acap, rcap)
}

fn points(owner: &[bool], lower: &[u64], upper: &[Option<u64>], rank: Option<u32>, pb: PB) -> Option<u128> {
    let t = owner.iter().filter(|&&b| b).count() as i64;
    let (acap, rcap) = limits(owner, rank, pb);
    let act: Vec<_> = (0..owner.len()).filter(|&i| owner[i]).map(|i| (lower[i], upper[i])).collect();
    let ina: Vec<_> = (0..owner.len()).filter(|&i| !owner[i]).map(|i| (lower[i], upper[i])).collect();
    let da = dist(&act, acap)?;
    let dr = dist(&ina, rcap)?;
    let mut total = 0u128;
    for (sa, &ca) in da.iter().enumerate() {
        if ca == 0 {
            continue;
        }
        let a = sa as i64 + t;
        for (sr, &cr) in dr.iter().enumerate() {
            if cr == 0 {
                continue;
            }
            let d = a - sr as i64;
            if pb.min_power_difference.is_some_and(|m| d < m) || pb.max_power_difference.is_some_and(|m| d > m) {
                continue;
            }
            total += ca * cr;
        }
    }
    Some(total)
}

/// Enumerate points (packed 6 bits per coordinate) of a finite domain.
fn enumerate(owner: &[bool], lower: &[u64], upper: &[Option<u64>], rank: Option<u32>, pb: PB, out: &mut dyn FnMut(u128)) -> bool {
    let n = owner.len();
    let t = owner.iter().filter(|&&b| b).count() as i64;
    let (acap, rcap) = limits(owner, rank, pb);
    let mut hi = vec![0u64; n];
    for i in 0..n {
        let cap = if owner[i] { acap } else { rcap };
        hi[i] = match (upper[i], cap) {
            (Some(u), Some(c)) => u.min(c.max(0) as u64),
            (Some(u), None) => u,
            (None, Some(c)) => c.max(0) as u64,
            (None, None) => return false,
        };
        if hi[i] >= 63 {
            return false;
        }
    }
    if (0..n).any(|i| lower[i] > hi[i]) {
        return true;
    }
    // minimal remaining sums of lower bounds, for pruning
    let mut rest_a = vec![0i64; n + 1];
    let mut rest_r = vec![0i64; n + 1];
    for i in (0..n).rev() {
        rest_a[i] = rest_a[i + 1] + if owner[i] { lower[i] as i64 } else { 0 };
        rest_r[i] = rest_r[i + 1] + if owner[i] { 0 } else { lower[i] as i64 };
    }
    fn rec(
        i: usize, key: u128, sa: i64, sr: i64, n: usize, owner: &[bool], lower: &[u64], hi: &[u64],
        acap: Option<i64>, rcap: Option<i64>, t: i64, pb: PB, rest_a: &[i64], rest_r: &[i64], out: &mut dyn FnMut(u128),
    ) {
        if i == n {
            let d = sa + t - sr;
            if pb.min_power_difference.is_none_or(|m| d >= m) && pb.max_power_difference.is_none_or(|m| d <= m) {
                out(key);
            }
            return;
        }
        for v in lower[i]..=hi[i] {
            let (na, nr) = if owner[i] { (sa + v as i64, sr) } else { (sa, sr + v as i64) };
            if acap.is_some_and(|c| na + rest_a[i + 1] > c) || rcap.is_some_and(|c| nr + rest_r[i + 1] > c) {
                break;
            }
            rec(i + 1, (key << 6) | v as u128, na, nr, n, owner, lower, hi, acap, rcap, t, pb, rest_a, rest_r, out);
        }
    }
    rec(0, 0, 0, 0, n, owner, lower, &hi, acap, rcap, t, pb, &rest_a, &rest_r, out);
    true
}

fn stat(v: &Option<serde_json::Value>, key: &str) -> f64 {
    v.as_ref().and_then(|s| s.get(key)).and_then(|x| x.as_f64()).unwrap_or(0.0)
}

#[derive(Default, Debug)]
struct Agg {
    count: f64,
    seconds: f64,
    successors: f64,
    points: f64,
    inf: f64,
    pieces: f64,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args[1].clone();
    let path = &args[2];
    let class_path = &args[3];
    let max_records: usize = args.get(4).map_or(usize::MAX, |s| s.parse().unwrap());
    let mut class: HashMap<String, String> = HashMap::new();
    for line in std::fs::read_to_string(class_path).unwrap().lines() {
        let mut it = line.split_whitespace();
        let m = it.next().unwrap().to_string();
        let c = it.next().unwrap().to_string();
        class.insert(m, c);
    }
    let file = std::fs::File::open(path).unwrap();
    let reader = BufReader::with_capacity(1 << 24, file);
    let jsonl = path.ends_with(".jsonl");
    let mut agg: HashMap<String, Agg> = HashMap::new();
    let mut owner_agg: HashMap<String, Agg> = HashMap::new();
    let mut sets: HashMap<String, HashSet<u128>> = HashMap::new();
    let mut enum_fail = 0usize;
    let mut buf = String::new();
    let mut in_rec = false;
    let mut n = 0usize;
    let mut cover: HashMap<String, [f64; 9]> = HashMap::new();
    let mut cover2: std::collections::BTreeMap<String, [f64; 5]> = Default::default();
    let mut scale: std::collections::BTreeMap<(String, i32), [f64; 5]> = Default::default();
    let mut last_id: Option<usize> = None;
    let mut nonmono = 0usize;
    let mut process = |text: &str, n: &mut usize| {
        let r: Rec = match serde_json::from_str(text) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("parse error {e}");
                return;
            }
        };
        *n += 1;
        let owner: Vec<bool> = r.owner.bytes().map(|b| b == b'1').collect();
        let phase = r.phase.clone().unwrap_or_default();
        let cls = if phase == "Apply" {
            class.get(&r.owner).cloned().unwrap_or_else(|| "uninstalled".into())
        } else {
            "route".into()
        };
        let key = format!("{}|{}|{}", r.record_kind, phase, cls);
        let pb = r.power_bounds.unwrap_or_default();
        let pts = points(&owner, &r.lower, &r.upper, r.rank, pb);
        let succ = if phase == "Route" {
            stat(&r.stats, "apply_domains") + stat(&r.stats, "route_domains")
        } else {
            stat(&r.stats, "successors")
        };
        let pieces = r.stats.as_ref().and_then(|s| s.get("matching")).and_then(|m| m.get("pieces")).and_then(|x| x.as_f64()).unwrap_or(0.0);
        for a in [agg.entry(key).or_default(), owner_agg.entry(format!("{}|{}|{}", r.owner, phase, r.record_kind)).or_default()] {
            a.count += 1.0;
            a.seconds += r.seconds.unwrap_or(0.0);
            a.successors += succ;
            a.pieces += pieces;
            match pts {
                Some(p) => a.points += p as f64,
                None => a.inf += 1.0,
            }
        }
        if mode == "scale" && r.record_kind == "native_inspection" && phase == "Apply" {
            if let Some(p) = pts {
                let b = if p == 0 { -1 } else { (p as f64).log10().floor() as i32 };
                let e = scale.entry((cls.clone(), b)).or_default();
                e[0] += 1.0; e[1] += p as f64; e[2] += pieces; e[3] += succ; e[4] += r.seconds.unwrap_or(0.0);
                let e = scale.entry(("ALL".to_string(), b)).or_default();
                e[0] += 1.0; e[1] += p as f64; e[2] += pieces; e[3] += succ; e[4] += r.seconds.unwrap_or(0.0);
            }
        }
        if mode == "cover" {
            // admission-order union coverage: points of this domain already covered by earlier admitted domains
            if last_id.is_some_and(|l| r.id <= l) { nonmono += 1; }
            last_id = Some(r.id);
            let set = sets.entry(format!("{}|{}", r.owner, phase)).or_default();
            let (mut tot, mut new) = (0u64, 0u64);
            enumerate(&owner, &r.lower, &r.upper, r.rank, pb, &mut |k| {
                tot += 1;
                if set.insert(k) { new += 1; }
            });
            let c = cover.entry(format!("{}|{}", phase, r.record_kind)).or_default();
            c[0] += 1.0;
            if tot == 0 { c[1] += 1.0; } else if new == 0 { c[2] += 1.0; } else if new == tot { c[3] += 1.0; } else { c[4] += 1.0; c[7] += (tot - new) as f64 / tot as f64; }
            c[5] += tot as f64;
            c[6] += new as f64;
            if new > 0 && new < tot && (new as f64) <= 0.1 * tot as f64 { c[8] += 1.0; }
            let cls2 = if tot == 0 { "empty" } else if new == 0 { "fully_covered" } else if new == tot { "fully_new" } else if (new as f64) <= 0.1 * tot as f64 { "partial_le10pct_new" } else { "partial_gt10pct_new" };
            let e = cover2.entry(format!("{}|{}|{}", phase, r.record_kind, cls2)).or_default();
            e[0] += 1.0; e[1] += succ; e[2] += r.seconds.unwrap_or(0.0); e[3] += tot as f64; e[4] += new as f64;
        }
        if mode == "union" && r.record_kind == "native_inspection" && phase == "Apply" {
            let set = sets.entry(format!("{}|{}", r.owner, phase)).or_default();
            if !enumerate(&owner, &r.lower, &r.upper, r.rank, pb, &mut |k| {
                set.insert(k);
            }) {
                enum_fail += 1;
            }
        }
    };
    for line in reader.lines() {
        let line = line.unwrap();
        if n >= max_records {
            break;
        }
        if jsonl {
            process(&line, &mut n);
            continue;
        }
        if !in_rec {
            if line == "    {" {
                in_rec = true;
                buf.clear();
                buf.push('{');
            }
        } else if line == "    }," || line == "    }" {
            buf.push('}');
            process(&buf, &mut n);
            in_rec = false;
        } else {
            buf.push_str(line.trim());
        }
    }
    println!("records {}", n);
    if mode == "scale" {
        for ((c, b), e) in &scale {
            println!("SCALE {c} log10pts={b} n={} mean_points={:.1} mean_pieces={:.1} mean_successors={:.1} mean_seconds={:.4} succ_per_point={:.3} points_per_piece={:.1} sec_share={:.4}", e[0], e[1]/e[0], e[2]/e[0], e[3]/e[0], e[4]/e[0], e[3]/e[1].max(1.0), e[1]/e[2].max(1.0), e[4]);
        }
    }
    if mode == "cover" {
        println!("COVER nonmonotone_ids={nonmono}");
        for (k, e) in &cover2 { println!("COVER2 {k} domains={} successors={} seconds={:.1} points={:.4e} new_points={:.4e}", e[0], e[1], e[2], e[3], e[4]); }
        let mut ks: Vec<_> = cover.keys().cloned().collect();
        ks.sort();
        for k in ks {
            let c = cover[&k];
            println!("COVER {k} domains={} empty={} fully_covered={} fully_new={} partial={} mean_covered_frac_of_partial={:.3} partial_with_new_le_10pct={} sum_points={:.4e} new_points={:.4e}", c[0], c[1], c[2], c[3], c[4], if c[4] > 0.0 { c[7] / c[4] } else { 0.0 }, c[8], c[5], c[6]);
        }
    }
    let mut keys: Vec<_> = agg.keys().cloned().collect();
    keys.sort();
    for k in keys {
        let a = &agg[&k];
        println!("AGG {k} count={} seconds={:.1} successors={} points={:.4e} inf={} pieces={}", a.count, a.seconds, a.successors, a.points, a.inf, a.pieces);
    }
    let mut owners: Vec<_> = owner_agg.iter().collect();
    owners.sort_by(|a, b| b.1.seconds.partial_cmp(&a.1.seconds).unwrap());
    for (k, a) in owners.iter().take(40) {
        println!("OWN {k} {} count={} seconds={:.1} successors={} points={:.4e} inf={} pieces={}", class.get(k.split('|').next().unwrap()).map_or("-", |s| s), a.count, a.seconds, a.successors, a.points, a.inf, a.pieces);
    }
    if mode == "union" {
        let mut tot = 0usize;
        let mut v: Vec<_> = sets.iter().map(|(k, s)| (k.clone(), s.len())).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        for (k, l) in &v {
            tot += l;
            let a = owner_agg.get(&format!("{}|native_inspection", k));
            println!("UNION {k} distinct={l} sum_points={:.4e} inspections={}", a.map_or(0.0, |a| a.points), a.map_or(0.0, |a| a.count));
        }
        println!("UNION_TOTAL distinct={tot} enum_fail={enum_fail} owners={}", v.len());
        fn binom(n: i64, k: i64) -> f64 {
            if k < 0 || n < k || n < 0 { return 0.0; }
            let mut r = 1.0f64;
            for i in 0..k { r = r * (n - i) as f64 / (i + 1) as f64; }
            r
        }
        for (k, _) in &v {
            if !k.ends_with("|Apply") { continue; }
            let mask: Vec<bool> = k.split('|').next().unwrap().bytes().map(|b| b == b'1').collect();
            let n = mask.len();
            let t = mask.iter().filter(|&&b| b).count() as i64;
            let u = n as i64 - t;
            let mut levels: std::collections::BTreeMap<(i64, i64), u64> = Default::default();
            for &key in &sets[k] {
                let (mut a, mut r) = (t, 0i64);
                for i in 0..n {
                    let x = ((key >> (6 * (n - 1 - i))) & 63) as i64;
                    if mask[i] { a += x } else { r += x }
                }
                *levels.entry((a, r)).or_default() += 1;
            }
            let mut full = 0.0;
            let (mut amax, mut rmax) = (0, 0);
            for (&(a, r), _) in &levels {
                full += binom(a - 1, t - 1) * binom(r + u - 1, u - 1);
                amax = amax.max(a);
                rmax = rmax.max(r);
            }
            // staircase hull: all (a', r') dominated by some touched level
            let mut stair = 0.0;
            let mut stair_levels = 0;
            for a2 in t..=amax {
                for r2 in 0..=rmax {
                    if levels.keys().any(|&(a, r)| a2 <= a && r2 <= r) {
                        stair += binom(a2 - 1, t - 1) * binom(r2 + u - 1, u - 1);
                        stair_levels += 1;
                    }
                }
            }
            let distinct: u64 = levels.values().sum();
            println!("LEVELS {k} t={t} distinct={distinct} levels={} full_level_points={full:.4e} fill={:.3} amax={amax} rmax={rmax} staircase_levels={stair_levels} staircase_points={stair:.4e} stair_fill={:.3}", levels.len(), distinct as f64 / full, distinct as f64 / stair);
            let lv: Vec<String> = levels.iter().map(|(&(a, r), &c)| format!("{a}/{r}:{c}/{:.0}", binom(a - 1, t - 1) * binom(r + u - 1, u - 1))).collect();
            println!("LEVELLIST {k} {}", lv.join(" "));
        }
    }
}
