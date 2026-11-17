//! Key lists for the brute-force cross-checks: exhaustive enumeration of every chart key with D + r <= R0, random keys
//! by D + r strata, adversarial keys (rule-domain boundaries, exception-branch loci, given faces / loci), and the
//! comparison of the verifier's first match with the engine's `rr_node first-match` output.

use std::io::{BufRead, BufReader, Write};

use serde_json::{Value, json};

use crate::dispatch;
use crate::table::Table;
use crate::{Args, die};

pub fn read_keys(path: &str) -> Result<Vec<Vec<i64>>, String> {
    let f = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
    let mut out = vec![];
    for l in BufReader::new(f).lines() {
        let l = l.map_err(|e| e.to_string())?;
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(&l).map_err(|e| format!("{path}: {e}"))?;
        let k = if v.is_array() { v } else { v["key"].clone() };
        out.push(serde_json::from_value(k).map_err(|e| format!("{path}: {e}"))?);
    }
    Ok(out)
}

/// Visit every chart key with D + r <= r0 (lexicographic in the excess vector y, y_i = k_i - 1 or -k_i).
pub fn for_each_key(mask: &[bool], r0: i64, f: &mut dyn FnMut(&[i64])) {
    fn rec(mask: &[bool], i: usize, left: i64, k: &mut Vec<i64>, f: &mut dyn FnMut(&[i64])) {
        if i == mask.len() {
            f(k);
            return;
        }
        for y in 0..=left {
            k[i] = if mask[i] { 1 + y } else { -y };
            rec(mask, i + 1, left - y, k, f);
        }
    }
    let mut k = vec![0i64; mask.len()];
    rec(mask, 0, r0, &mut k, f);
}

/// `enum --table T --r0 R0 --out-prefix P [--chunk M] [--threads T]`: every key with D + r <= R0; the verifier's first
/// match of each, the uncovered ones, and key files of at most M keys for the engine (P.keys.NNN.jsonl) plus the
/// verifier's labels in the same order (P.mine.NNN.jsonl).
pub fn cmd_enum(mut a: Args) {
    let tp = a.need("--table");
    let r0: i64 = a.need("--r0").parse().unwrap_or_else(|_| die("--r0"));
    let prefix = a.need("--out-prefix");
    let chunk: usize = a
        .take("--chunk")
        .map(|x| x.parse().unwrap())
        .unwrap_or(2_000_000);
    let threads: usize = a.take("--threads").map(|x| x.parse().unwrap()).unwrap_or(4);
    let write_keys = !a.flag("--no-key-files");
    let stream = a.flag("--stream");
    a.done();
    let t = Table::load(&tp).unwrap_or_else(|e| die(&e));
    if stream {
        // no key storage: every thread walks the whole enumeration and dispatches its residue class
        let t0 = std::time::Instant::now();
        type Part = (u64, Vec<Vec<i64>>, std::collections::BTreeMap<usize, u64>);
        let results: Vec<Part> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..threads)
                .map(|tid| {
                    let t = &t;
                    s.spawn(move || {
                        let mut cnt = 0u64;
                        let mut i = 0usize;
                        let mut unc = vec![];
                        let mut hist = std::collections::BTreeMap::new();
                        for_each_key(&t.mask, r0, &mut |k| {
                            if i % threads == tid {
                                cnt += 1;
                                match dispatch::first_match(t, k) {
                                    None => unc.push(k.to_vec()),
                                    Some(p) => *hist.entry(p).or_insert(0u64) += 1,
                                }
                            }
                            i += 1;
                        });
                        (cnt, unc, hist)
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let keys: u64 = results.iter().map(|r| r.0).sum();
        let mut unc: Vec<Vec<i64>> = results.iter().flat_map(|r| r.1.clone()).collect();
        unc.sort();
        let mut hist = std::collections::BTreeMap::<usize, u64>::new();
        for r in &results {
            for (k, v) in &r.2 {
                *hist.entry(*k).or_default() += v;
            }
        }
        let masters: std::collections::BTreeSet<&Vec<i64>> = t.masters.iter().collect();
        let not_master: Vec<&Vec<i64>> = unc.iter().filter(|k| !masters.contains(k)).collect();
        let res = json!({"schema": "rr_pverify.enum.v1", "table": tp, "r0": r0, "keys": keys, "uncovered": unc,
            "uncovered_not_master": not_master, "rules_hit": hist.len(), "seconds": t0.elapsed().as_secs_f64(),
            "verdict": if not_master.is_empty() { "YES" } else { "NO" }});
        println!("{}", serde_json::to_string(&res).unwrap());
        let _ = prefix;
        let _ = chunk;
        let _ = write_keys;
        return;
    }
    let mut all: Vec<Vec<i64>> = vec![];
    for_each_key(&t.mask, r0, &mut |k| all.push(k.to_vec()));
    eprintln!("{} keys with D + r <= {r0}", all.len());
    let t0 = std::time::Instant::now();
    let mut labels: Vec<Option<usize>> = vec![None; all.len()];
    std::thread::scope(|s| {
        let parts: Vec<(usize, &mut [Option<usize>])> = {
            let per = all.len().div_ceil(threads.max(1));
            labels
                .chunks_mut(per.max(1))
                .enumerate()
                .map(|(i, c)| (i * per.max(1), c))
                .collect()
        };
        for (off, part) in parts {
            let t = &t;
            let all = &all;
            s.spawn(move || {
                for (j, slot) in part.iter_mut().enumerate() {
                    *slot = dispatch::first_match(t, &all[off + j]);
                }
            });
        }
    });
    let secs = t0.elapsed().as_secs_f64();
    let masters: std::collections::BTreeSet<&Vec<i64>> = t.masters.iter().collect();
    let mut uncovered = vec![];
    for (k, l) in all.iter().zip(&labels) {
        if l.is_none() {
            uncovered.push(k.clone());
        }
    }
    let not_master: Vec<&Vec<i64>> = uncovered.iter().filter(|k| !masters.contains(k)).collect();
    let mut hist = std::collections::BTreeMap::<usize, u64>::new();
    for l in labels.iter().flatten() {
        *hist.entry(*l).or_default() += 1;
    }
    if write_keys {
        for (ci, (ks, ls)) in all.chunks(chunk).zip(labels.chunks(chunk)).enumerate() {
            let mut fk = std::io::BufWriter::new(
                std::fs::File::create(format!("{prefix}.keys.{ci:03}.jsonl")).unwrap(),
            );
            let mut fm = std::io::BufWriter::new(
                std::fs::File::create(format!("{prefix}.mine.{ci:03}.jsonl")).unwrap(),
            );
            for (k, l) in ks.iter().zip(ls) {
                writeln!(fk, "{}", json!(k)).unwrap();
                writeln!(fm, "{}", json!({"key": k, "position": l})).unwrap();
            }
        }
    }
    let res = json!({
        "schema": "rr_pverify.enum.v1", "table": tp, "r0": r0, "keys": all.len(),
        "uncovered": uncovered, "uncovered_not_master": not_master, "rules_hit": hist.len(),
        "seconds": secs, "chunks": all.len().div_ceil(chunk),
        "verdict": if not_master.is_empty() { "YES" } else { "NO" },
    });
    println!("{}", serde_json::to_string(&res).unwrap());
}

/// Small deterministic PRNG (SplitMix64): reproducible key lists without a dependency.
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    /// uniform in [lo, hi]
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let span = (hi - lo) as u64 + 1;
        lo + (self.next() % span) as i64
    }
}

/// A uniformly random composition of `s` into the given slots (others at their chart base), as a chart key.
fn random_key(mask: &[bool], slots: &[usize], s: i64, rng: &mut Rng) -> Vec<i64> {
    let m = slots.len();
    let mut cuts: Vec<i64> = (0..m - 1).map(|_| rng.range(0, s + m as i64 - 1)).collect();
    // sample m-1 distinct cut points of s + m - 1 positions (stars and bars, with rejection of repeats)
    cuts.sort();
    cuts.dedup();
    while cuts.len() < m - 1 {
        cuts.push(rng.range(0, s + m as i64 - 1));
        cuts.sort();
        cuts.dedup();
    }
    let mut ys = vec![];
    let mut prev = -1i64;
    for &c in &cuts {
        ys.push(c - prev - 1);
        prev = c;
    }
    ys.push(s + m as i64 - 1 - prev - 1);
    let mut k: Vec<i64> = mask.iter().map(|&l| if l { 1 } else { 0 }).collect();
    for (&i, &y) in slots.iter().zip(&ys) {
        k[i] = if mask[i] { 1 + y } else { -y };
    }
    k
}

/// `random --table T --out FILE --seed S --per N`: random keys in D + r strata (uniform compositions, sparse supports
/// too), adversarial keys around every rule's fixed slots and one-variable exception loci, on given faces
/// (`--faces FILE`: JSON list of {name, fixed: [int|null], min: [int|null], max: [int|null]}) and on given loci
/// (`--loci FILE`: moved_loci.json-style list whose third entry is a list of polynomials). Each key with its stratum.
pub fn cmd_random(mut a: Args) {
    let tp = a.need("--table");
    let out = a.need("--out");
    let seed: u64 = a
        .take("--seed")
        .map(|x| x.parse().unwrap())
        .unwrap_or(20261116);
    let per: usize = a
        .take("--per")
        .map(|x| x.parse().unwrap())
        .unwrap_or(50_000);
    let faces = a.take("--faces");
    let loci = a.take("--loci");
    let rmax: i64 = a
        .need("--radius")
        .parse()
        .unwrap_or_else(|_| die("--radius: an integer"));
    a.done();
    let t = Table::load(&tp).unwrap_or_else(|e| die(&e));
    let n = t.n;
    let mut rng = Rng::new(seed);
    let mut f = std::io::BufWriter::new(std::fs::File::create(&out).unwrap());
    let mut count = 0usize;
    let mut emit = |k: &[i64], s: &str, f: &mut std::io::BufWriter<std::fs::File>| {
        if dispatch::in_chart(&t.mask, k) {
            writeln!(f, "{}", json!({"key": k, "stratum": s})).unwrap();
            count += 1;
        }
    };
    let all: Vec<usize> = (0..n).collect();
    let strata: Vec<(i64, i64, String)> = vec![
        (0, 10, "random D+r in [0,10]".into()),
        (11, 40, "random D+r in [11,40]".into()),
        (41, 150, "random D+r in [41,150]".into()),
        (151, rmax, format!("random D+r in [151,{rmax}]")),
        (
            rmax - 5,
            rmax,
            format!("random D+r in [{},{rmax}] (cone boundary)", rmax - 5),
        ),
        (
            rmax + 1,
            10 * rmax,
            format!("random D+r in [{},{}] (beyond)", rmax + 1, 10 * rmax),
        ),
        (
            10 * rmax + 1,
            2_000_000_000,
            "random D+r up to 2e9 (far)".into(),
        ),
    ];
    for (lo, hi, name) in &strata {
        for i in 0..per {
            // half full support, half a random sparse support of 1..4 slots
            let slots: Vec<usize> = if i % 2 == 0 {
                all.clone()
            } else {
                let m = rng.range(1, 4) as usize;
                let mut s: Vec<usize> = vec![];
                while s.len() < m {
                    let v = rng.range(0, n as i64 - 1) as usize;
                    if !s.contains(&v) {
                        s.push(v);
                    }
                }
                s
            };
            let s = if *hi > 1000 && i % 3 == 0 {
                // log-uniform
                let lg = (rng.next() % 1_000_000) as f64 / 1_000_000.0;
                ((*lo as f64) * ((*hi as f64) / (*lo as f64).max(1.0)).powf(lg)) as i64
            } else {
                rng.range(*lo, *hi)
            };
            let k = random_key(&t.mask, &slots, s.clamp(*lo, *hi), &mut rng);
            emit(&k, name, &mut f);
        }
    }
    // adversarial: rule-domain boundaries
    for r in &t.rules {
        for _ in 0..4 {
            let mut k: Vec<i64> = (0..n)
                .map(|i| match r.fixed[i] {
                    Some(x) => x,
                    None => {
                        let y = if rng.range(0, 2) == 0 {
                            0
                        } else {
                            rng.range(0, 6)
                        };
                        if t.mask[i] { 1 + y } else { -y }
                    }
                })
                .collect();
            // move one fixed slot off by one
            let fx: Vec<usize> = (0..n).filter(|&i| r.fixed[i].is_some()).collect();
            if !fx.is_empty() && rng.range(0, 1) == 0 {
                let i = fx[rng.range(0, fx.len() as i64 - 1) as usize];
                k[i] += if rng.range(0, 1) == 0 { 1 } else { -1 };
            }
            emit(&k, "rule-domain boundary", &mut f);
        }
        for br in &r.exceptions {
            for m in br {
                if let Some((c, v, aa)) = m.fast {
                    if c % aa != 0 {
                        continue;
                    }
                    let root = -c / aa;
                    for dv in [-1i64, 0, 1] {
                        let mut k: Vec<i64> = (0..n)
                            .map(|i| match r.fixed[i] {
                                Some(x) => x,
                                None => {
                                    let y = rng.range(0, 4);
                                    if t.mask[i] { 1 + y } else { -y }
                                }
                            })
                            .collect();
                        if r.fixed[v].is_none() {
                            k[v] = root as i64 + dv;
                        }
                        emit(&k, "exception locus (+-1)", &mut f);
                    }
                }
            }
        }
    }
    // faces
    if let Some(fp) = faces {
        let fs: Value = serde_json::from_str(&std::fs::read_to_string(&fp).unwrap()).unwrap();
        for face in fs.as_array().unwrap() {
            let name = face["name"].as_str().unwrap_or("face");
            let fixed: Vec<Option<i64>> = face["fixed"]
                .as_array()
                .unwrap()
                .iter()
                .map(Value::as_i64)
                .collect();
            let mn: Vec<Option<i64>> = face["min"]
                .as_array()
                .unwrap()
                .iter()
                .map(Value::as_i64)
                .collect();
            let mx: Vec<Option<i64>> = face["max"]
                .as_array()
                .unwrap()
                .iter()
                .map(Value::as_i64)
                .collect();
            let free: Vec<usize> = (0..n).filter(|&i| fixed[i].is_none()).collect();
            for i in 0..per {
                let s = match i % 4 {
                    0 => rng.range(0, 30),
                    1 => rng.range(0, rmax),
                    2 => rng.range(rmax - 10, rmax),
                    _ => rng.range(0, 100_000),
                };
                // free slot j = its face bound (min on a line, max on a numerator, else the chart base) + excess y_j
                let mut k: Vec<i64> = (0..n).map(|j| fixed[j].unwrap_or(0)).collect();
                let base = random_key(&t.mask, &free, s, &mut rng);
                for &j in &free {
                    let y = if t.mask[j] { base[j] - 1 } else { -base[j] };
                    k[j] = if t.mask[j] {
                        mn[j].unwrap_or(1) + y
                    } else {
                        mx[j].unwrap_or(0) - y
                    };
                }
                emit(&k, &format!("face {name}"), &mut f);
            }
        }
    }
    // loci: integer points of each moved locus near its rule's domain (small search)
    if let Some(lp) = loci {
        let ls: Value = serde_json::from_str(&std::fs::read_to_string(&lp).unwrap()).unwrap();
        for (li, l) in ls.as_array().unwrap().iter().enumerate() {
            let polys: Vec<crate::expr::Expr> = l[2]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| crate::expr::Expr::parse(p.as_str().unwrap(), n).unwrap())
                .collect();
            let mut found = 0;
            for _ in 0..200_000 {
                if found >= per / 10 {
                    break;
                }
                let s = rng.range(0, rmax);
                let m = rng.range(1, 3) as usize;
                let mut slots: Vec<usize> = vec![];
                // prefer the polynomial's variables
                let mut vs: Vec<usize> = (0..n)
                    .filter(|v| polys.iter().any(|p| p.text.contains(&format!("n{v}"))))
                    .collect();
                vs.retain(|v| {
                    let s = format!("n{v}");
                    polys.iter().any(|p| {
                        p.text.match_indices(&s).any(|(i, _)| {
                            !p.text[i + s.len()..].starts_with(|c: char| c.is_ascii_digit())
                        })
                    })
                });
                while slots.len() < m.min(vs.len()) {
                    let v = vs[rng.range(0, vs.len() as i64 - 1) as usize];
                    if !slots.contains(&v) {
                        slots.push(v);
                    }
                }
                if slots.is_empty() {
                    break;
                }
                let k = random_key(&t.mask, &slots, s, &mut rng);
                if polys
                    .iter()
                    .all(|p| p.eval_int(&k, 0).map(|v| v.is_zero()).unwrap_or(false))
                {
                    emit(&k, &format!("moved locus {li}"), &mut f);
                    found += 1;
                }
            }
        }
    }
    eprintln!("{count} keys written to {out}");
}

/// `compare --mine FILE --engine FILE [--claims FILE]`: line-by-line identity of key and position.
pub fn cmd_compare(mut a: Args) {
    let mine = a.need("--mine");
    let eng = a.need("--engine");
    a.done();
    let rd = |p: &str| -> Vec<Value> {
        BufReader::new(std::fs::File::open(p).unwrap_or_else(|e| die(&format!("{p}: {e}"))))
            .lines()
            .map(|l| serde_json::from_str(&l.unwrap()).unwrap())
            .collect()
    };
    let m = rd(&mine);
    let e = rd(&eng);
    let mut agree = 0usize;
    let mut diffs = vec![];
    if m.len() != e.len() {
        die(&format!("lengths differ: {} vs {}", m.len(), e.len()));
    }
    let mut open = 0usize;
    for (x, y) in m.iter().zip(&e) {
        if x["key"] != y["key"] {
            die("key order differs");
        }
        let px = x["position"].as_u64();
        let py = y["position"].as_u64();
        if px.is_none() {
            open += 1;
        }
        if px == py {
            agree += 1;
        } else if diffs.len() < 50 {
            diffs.push(json!({"key": x["key"], "mine": px, "engine": py}));
        }
    }
    println!(
        "{}",
        json!({"schema": "rr_pverify.compare.v1", "keys": m.len(), "agree": agree, "open": open,
               "differ": m.len() - agree, "first_differences": diffs,
               "verdict": if agree == m.len() { "YES" } else { "NO" }})
    );
}
