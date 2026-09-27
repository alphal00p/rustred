// Read-only: how many staircase cells (phase, owner, rank, A max, D min, D max) would the admitted domains widen to?
// usage: keys <records.jsonl>...
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
#[derive(Deserialize, Default, Clone, Copy)]
struct PB { max_positive_power: Option<u64>, min_power_difference: Option<i64>, max_power_difference: Option<i64> }
#[derive(Deserialize)]
struct Rec { owner: String, phase: Option<String>, rank: Option<u32>, power_bounds: Option<PB>, record_kind: String }
type K = (Option<u32>, Option<u64>, Option<i64>, Option<i64>);
fn le_u<T: Ord + Copy>(a: Option<T>, b: Option<T>) -> bool { match (a, b) { (_, None) => true, (None, Some(_)) => false, (Some(x), Some(y)) => x <= y } }
fn ge_l(a: Option<i64>, b: Option<i64>) -> bool { match (a, b) { (_, None) => true, (None, Some(_)) => false, (Some(x), Some(y)) => x >= y } }
fn contained(k: &K, c: &K) -> bool { le_u(k.0, c.0) && le_u(k.1, c.1) && ge_l(k.2, c.2) && le_u(k.3, c.3) }
fn main() {
    let mut buckets: HashMap<(String, String), HashSet<K>> = HashMap::new();
    let mut per_kind: HashMap<String, usize> = HashMap::new();
    let mut n = 0usize;
    for f in std::env::args().skip(1) {
        let r = BufReader::with_capacity(1 << 24, std::fs::File::open(&f).unwrap());
        let pretty = !f.ends_with(".jsonl");
        let mut buf = String::new();
        let mut in_rec = false;
        for line in r.lines() {
            let line = line.unwrap();
            let text: String;
            if pretty {
                if !in_rec { if line == "    {" { in_rec = true; buf.clear(); buf.push('{'); } continue; }
                if line == "    }," || line == "    }" { buf.push('}'); in_rec = false; text = buf.clone(); } else { buf.push_str(line.trim()); continue; }
            } else { text = line; }
            let v: Rec = match serde_json::from_str(&text) { Ok(v) => v, Err(_) => continue };
            n += 1;
            *per_kind.entry(format!("{}|{}", v.record_kind, v.phase.clone().unwrap_or_default())).or_default() += 1;
            let pb = v.power_bounds.unwrap_or_default();
            buckets.entry((v.phase.unwrap_or_default(), v.owner)).or_default().insert((v.rank, pb.max_positive_power, pb.min_power_difference, pb.max_power_difference));
        }
        eprintln!("{f} done, records {n}");
    }
    let (mut distinct, mut maximal) = (HashMap::<String, usize>::new(), HashMap::<String, usize>::new());
    let mut nb = HashMap::<String, usize>::new();
    let mut rank_max = HashMap::<String, u32>::new();
    for ((ph, _o), keys) in &buckets {
        *nb.entry(ph.clone()).or_default() += 1;
        *distinct.entry(ph.clone()).or_default() += keys.len();
        let ks: Vec<&K> = keys.iter().collect();
        let mut m = 0;
        for k in &ks { if !ks.iter().any(|c| c != k && contained(k, c)) { m += 1; } }
        *maximal.entry(ph.clone()).or_default() += m;
        for k in &ks { if let Some(r) = k.0 { let e = rank_max.entry(ph.clone()).or_default(); *e = (*e).max(r); } }
    }
    println!("records={n} kinds={per_kind:?}");
    for ph in nb.keys() { println!("PHASE {ph} buckets={} distinct_cells={} maximal_cells={} max_rank={:?}", nb[ph], distinct[ph], maximal[ph], rank_max.get(ph)); }
}
