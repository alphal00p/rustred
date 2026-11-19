//! `rr_campaign regularity-pairs` (round 6 J3, lane rulecost's request: O4 item 6): the d = 4 regularity measure of contract
//! section 13 applied to (key, image) PAIRS of an owner whose table is on disk. A key K served today by the first-match rule
//! R(K) may be sent by orbit canonicalisation to an exact one-key image K' = sigma(K) (I(K) = sign * I(K')), which is then
//! reduced by ITS first-match rule R(K'). The pair is WORSE when the relation used through the image is less regular at d = 4
//! than the relation used today: pole(R(K') at K') > pole(R(K) at K), or the image's relation is undefined where the key's is
//! not. An image that is a DECLARED TERMINAL of the owner is exact and regular (pole 0). Same exact measure as section 13
//! (`emission::regularity`: Taylor shift at d = 4, daughters merged, 0/0 resolved). The sign does not change pole orders.
//!
//! Which image the engine takes is its order key's least image (not computable offline for Custom owners); instead the command
//! writes a DESCENT TABLE of one-term relations K -> sign * K' (one rule per pair, the owner table's header and order) for
//! `rr_node descent`, and with `--descent-out` restricts the verdict to the images that are LOWER than their key: the keys whose
//! every lower image is not worse are safe whatever image the engine picks.
use super::regcheck::dispatcher_of;
use rustred_campaign_core::emission::body::{string_member, strip_derivation};
use rustred_campaign_core::emission::regularity::{KeyRegularity, Relation, regularity_at};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};

fn key_text(k: &[i64]) -> String {
    format!("I({})", k.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","))
}

/// One pair.
#[derive(Clone, Debug)]
pub struct Pair {
    pub key: Vec<i64>,
    pub image: Vec<i64>,
    pub sign: i64,
    pub image_is_terminal: bool,
}

/// The measure of every pair; returns (summary, per-pair rows).
pub fn measure(table: &Path, pairs: &[Pair], threads: usize, not_lower: Option<&BTreeSet<usize>>) -> Result<(Value, Vec<Value>), String> {
    let (disp, chart) = dispatcher_of(table)?;
    // the rule bodies by index (the table's own index = position)
    let mut rels: HashMap<i64, Relation> = HashMap::new();
    for l in std::fs::read_to_string(table).map_err(|e| format!("{}: {e}", table.display()))?.lines() {
        if string_member(l, "type").as_deref() != Some("rule") {
            continue;
        }
        let (b, _) = strip_derivation(l)?;
        let v: Value = serde_json::from_str(&b).map_err(|e| e.to_string())?;
        let ix = v["index"].as_i64().ok_or("a rule without index")?;
        rels.insert(ix, Relation::parse(&b).map_err(|e| format!("rule {ix}: {e}"))?);
    }
    let first = |k: &[i64]| -> Option<i64> {
        let k32: Vec<i32> = k.iter().map(|&x| x as i32).collect();
        disp.first_match(&k32).ok().and_then(|d| d.winner()).map(|w| w.label as i64)
    };
    let jobs: Vec<(usize, Option<i64>, Option<i64>)> =
        pairs.iter().enumerate().map(|(i, p)| (i, first(&p.key), if p.image_is_terminal { None } else { first(&p.image) })).collect();
    let threads = threads.max(1).min(jobs.len().max(1));
    let chunk = jobs.len().div_ceil(threads).max(1);
    type M = (usize, Option<i64>, Option<i64>, Option<Result<KeyRegularity, String>>, Option<Result<KeyRegularity, String>>);
    let results: Vec<M> = std::thread::scope(|s| {
        let hs: Vec<_> = jobs
            .chunks(chunk)
            .map(|part| {
                let rels = &rels;
                s.spawn(move || {
                    part.iter()
                        .map(|&(i, rk, ri)| {
                            let p = &pairs[i];
                            let old = rk.and_then(|r| rels.get(&r)).map(|rel| regularity_at(rel, &p.key));
                            let new = ri.and_then(|r| rels.get(&r)).map(|rel| regularity_at(rel, &p.image));
                            (i, rk, ri, old, new)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    let mut rows = Vec::with_capacity(results.len());
    let mut c: BTreeMap<&str, u64> = BTreeMap::new();
    let mut hist: BTreeMap<(u32, u32), u64> = BTreeMap::new();
    let mut by_key: BTreeMap<Vec<i64>, (u64, u64, u64)> = BTreeMap::new(); // (lower images measured, worse, not lower)
    for (i, rk, ri, old, new) in results {
        let p = &pairs[i];
        let lower = not_lower.is_none_or(|nl| !nl.contains(&i));
        let (status, worse, op, np) = match (&old, p.image_is_terminal, &new) {
            (None, _, _) => ("key_unserved", false, None, None),
            (Some(Err(e)), _, _) => {
                rows.push(json!({"pair": i, "key": p.key, "image": p.image, "status": "error", "error": e}));
                *c.entry("error").or_default() += 1;
                continue;
            }
            (Some(Ok(o)), true, _) => ("image_terminal", false, Some(o.clone()), None),
            (Some(Ok(_)), false, None) => ("image_unserved", true, None, None),
            (Some(Ok(o)), false, Some(Err(e))) => {
                rows.push(json!({"pair": i, "key": p.key, "image": p.image, "status": "error", "error": e, "old_pole": o.max_pole}));
                *c.entry("error").or_default() += 1;
                continue;
            }
            (Some(Ok(o)), false, Some(Ok(n))) => {
                let w = n.max_pole > o.max_pole || (n.undefined && !o.undefined);
                ("compared", w, Some(o.clone()), Some(n.clone()))
            }
        };
        let (opole, npole) = (op.as_ref().map(|x| x.max_pole), np.as_ref().map(|x| x.max_pole).or(if status == "image_terminal" { Some(0) } else { None }));
        *c.entry(status).or_default() += 1;
        if status == "compared" {
            let (o, n) = (opole.unwrap_or(0), npole.unwrap_or(0));
            *hist.entry((o, n)).or_default() += 1;
            *c.entry(if worse { "worse" } else if n < o { "better" } else { "equal" }).or_default() += 1;
            if o == 0 && n > 0 {
                *c.entry("regular_to_singular").or_default() += 1;
            }
            if o > 0 && n == 0 {
                *c.entry("singular_to_regular").or_default() += 1;
            }
        }
        if status != "key_unserved" {
            let e = by_key.entry(p.key.clone()).or_default();
            if lower {
                e.0 += 1;
                if worse {
                    e.1 += 1;
                }
            } else {
                e.2 += 1;
            }
        }
        rows.push(json!({"pair": i, "key": p.key, "image": p.image, "sign": p.sign, "rule_key": rk, "rule_image": ri,
                         "status": status, "old_pole": opole, "new_pole": npole, "worse": worse,
                         "new_undefined": np.as_ref().map(|x| x.undefined), "image_lower": not_lower.map(|_| lower)}));
    }
    rows.sort_by_key(|r| r["pair"].as_u64().unwrap_or(0));
    let keys_any_worse = by_key.values().filter(|e| e.1 > 0).count();
    let keys_safe = by_key.values().filter(|e| e.0 > 0 && e.1 == 0).count();
    let keys_no_lower = by_key.values().filter(|e| e.0 == 0).count();
    let summary = json!({"schema": "rustred.campaign.regularity-pairs.v1", "table": table.display().to_string(), "chart": chart,
        "pairs": pairs.len(), "counts": c,
        "pole_histogram_old_new": hist.iter().map(|((o, n), k)| json!({"old": o, "new": n, "pairs": k})).collect::<Vec<_>>(),
        "keys": by_key.len(), "keys_with_a_worse_image": keys_any_worse,
        "keys_every_image_not_worse": keys_safe, "keys_without_a_lower_image": if not_lower.is_some() { json!(keys_no_lower) } else { Value::Null },
        "restricted_to_lower_images": not_lower.is_some(),
        "definition": "pair WORSE = pole order at d = 4 of the image's first-match rule at the image > that of the key's first-match \
                       rule at the key (or undefined there only); a declared terminal image is regular; contract section 13 measure",
        "verdict": if keys_any_worse == 0 { "NOT WORSE at every key" } else { "WORSE at some keys" }});
    Ok((summary, rows))
}

/// The descent table of the pairs: header of `table` + one one-term rule K -> sign * K' per pair (index = pair number).
pub fn descent_table(table: &Path, pairs: &[Pair], out: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(table).map_err(|e| format!("{}: {e}", table.display()))?;
    let header = text.lines().find(|l| string_member(l, "type").as_deref() == Some("header")).ok_or("no header")?;
    let chart = text
        .lines()
        .find(|l| string_member(l, "type").as_deref() == Some("rule"))
        .and_then(|l| string_member(l, "chart"))
        .ok_or("no rule")?;
    let mut f = std::io::BufWriter::new(std::fs::File::create(out).map_err(|e| format!("{}: {e}", out.display()))?);
    writeln!(f, "{header}").map_err(|e| e.to_string())?;
    for (i, p) in pairs.iter().enumerate() {
        let r = json!({"type": "rule", "chart": chart, "index": i, "case": format!("pair {i}"), "fixed": p.key, "equations": [],
                       "target": key_text(&p.key), "rhs": [[key_text(&p.image), p.sign.to_string()]], "exceptions": []});
        writeln!(f, "{r}").map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// CLI `regularity-pairs --table T --pairs P.jsonl --out DIR [--threads N] [--descent-out FILE]`: writes DIR/summary.json,
/// DIR/pairs.out.jsonl and DIR/descent-input.table.jsonl. Exit 0 not worse at every key, 3 otherwise.
pub fn cmd(rest: &[String]) -> Result<(String, bool), String> {
    let (mut t, mut p, mut o, mut d) = (None::<PathBuf>, None::<PathBuf>, None::<PathBuf>, None::<PathBuf>);
    let mut threads = 1usize;
    let mut i = 0;
    let val = |i: usize| rest.get(i + 1).cloned().ok_or(format!("{} needs a value", rest[i]));
    while i < rest.len() {
        match rest[i].as_str() {
            "--table" => t = Some(val(i)?.into()),
            "--pairs" => p = Some(val(i)?.into()),
            "--out" => o = Some(val(i)?.into()),
            "--descent-out" => d = Some(val(i)?.into()),
            "--threads" => threads = val(i)?.parse().map_err(|e| format!("--threads: {e}"))?,
            x => return Err(format!("unknown flag {x}")),
        }
        i += 2;
    }
    let (Some(t), Some(p), Some(o)) = (t, p, o) else {
        return Err("regularity-pairs --table T --pairs P.jsonl --out DIR [--threads N] [--descent-out FILE]".into());
    };
    let mut pairs = vec![];
    for l in std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?.lines().filter(|l| !l.trim().is_empty()) {
        let v: Value = serde_json::from_str(l).map_err(|e| format!("pairs: {e}"))?;
        let k = |m: &str| -> Result<Vec<i64>, String> {
            v[m].as_array().ok_or(format!("pair without {m}"))?.iter().map(|x| x.as_i64().ok_or(format!("{m}: not an integer"))).collect()
        };
        pairs.push(Pair { key: k("key")?, image: k("image")?, sign: v["sign"].as_i64().unwrap_or(1),
                          image_is_terminal: v["image_is_terminal"].as_bool().unwrap_or(false) });
    }
    std::fs::create_dir_all(&o).map_err(|e| e.to_string())?;
    descent_table(&t, &pairs, &o.join("descent-input.table.jsonl"))?;
    // rr_node descent output: failing_rules "chart#i" = pair i's image is NOT lower than its key
    let nl: Option<BTreeSet<usize>> = match &d {
        Some(f) => {
            let text = std::fs::read_to_string(f).map_err(|e| format!("{}: {e}", f.display()))?;
            let line = text.lines().rev().find(|l| l.trim_start().starts_with('{')).ok_or("descent output without JSON")?;
            let v: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
            if v["rules"].as_u64() != Some(pairs.len() as u64) {
                return Err(format!("the descent output is of {} rules, there are {} pairs", v["rules"], pairs.len()));
            }
            Some(v["failing_rules"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()?.rsplit('#').next()?.parse().ok()).collect()).unwrap_or_default())
        }
        None => None,
    };
    let t0 = std::time::Instant::now();
    let (mut s, rows) = measure(&t, &pairs, threads, nl.as_ref())?;
    s["seconds"] = json!(t0.elapsed().as_secs_f64());
    s["images_not_lower"] = nl.as_ref().map_or(Value::Null, |x| json!(x.len()));
    let mut f = std::io::BufWriter::new(std::fs::File::create(o.join("pairs.out.jsonl")).map_err(|e| e.to_string())?);
    for r in &rows {
        writeln!(f, "{r}").map_err(|e| e.to_string())?;
    }
    std::fs::write(o.join("summary.json"), serde_json::to_string_pretty(&s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let ok = s["keys_with_a_worse_image"] == 0;
    Ok((s.to_string(), ok))
}
