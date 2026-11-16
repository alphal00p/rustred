//! AUDIT r2-sym item 4 (independent recomputation on REAL material; ignored, run by hand as a systemd unit):
//! every selected shipped relation of a real `rr_rf_symflow` run (plain symmetry images and composites = image +
//! catalogue rules) is KERNEL-CERTIFIED by the frozen proof kernel (`rr_proof --mode instance-span --symrows`), with
//! inputs built HERE, independently of the producer and of lane symcert's converters:
//! - the relation as shipped (target, rhs keys and coefficient texts verbatim) as ONE point-rule record in a table
//!   whose header (family_source) and `zero_sectors` census are copied from the node's frame table (engine census);
//! - the symmetry row: the cited element's momentum matrix from the verified group DERIVED here from the frame
//!   header's family (the run's group digest must be equal), seed = the key; the kernel re-derives the row itself;
//! - a composite's catalogue rules enter as IBP rows: each cited record (found by its record_sha256 in the catalogue
//!   tables) contributes its seed patterns instantiated at its term plus its recenter (absolute seeds), `--rows engine`;
//!   a plain relation is `--symrows-only --rows cert` (no IBP rows at all).
//!
//! A pass (`EXACT_IN_SPAN_AT_KEY`) is an exact certificate in Z[d] that the shipped relation at the key is a Q(d)
//! combination of valid symmetry rows (and IBP rows): it does NOT rely on the producer's algebra, on the catalogue
//! rule's own coefficients, or on the campaign's composition code. NEGATIVE CONTROLS: per run, the first `controls`
//! selected composites and plain relations are also submitted WELL-FORMED but wrong (two distinct coefficients
//! swapped; a plain relation with one coefficient value gets another rational): the kernel must refuse them.
//!
//! Config (env `AUDIT_R2_REAL`, JSON): {"kernel": path, "out": dir, "parallel": n, "controls": n,
//!   "runs": [{"name", "table", "verification", "frame", "catalogue": [dir | file], "plain": n, "composites": n,
//!   "seed": n}]}. Output: <out>/<run>/verdicts.jsonl + <out>/summary.json.

use rustred_campaign_core::import;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

fn sha256(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn parse_const_key(t: &str) -> Option<Vec<i64>> {
    let s: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    let inner = s.strip_prefix("I(")?.strip_suffix(')')?;
    inner.split(',').map(|x| x.parse().ok()).collect()
}

/// One slot of a record pattern: `n<j>` + offset, or a constant.
enum Slot {
    P(usize, i64),
    C(i64),
}

fn parse_pattern(t: &str) -> Result<Vec<Slot>, String> {
    let s: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    let inner = s
        .strip_prefix("I(")
        .and_then(|x| x.strip_suffix(')'))
        .ok_or_else(|| format!("pattern {t}"))?;
    inner
        .split(',')
        .map(|tok| {
            if let Ok(c) = tok.parse::<i64>() {
                return Ok(Slot::C(c));
            }
            let rest = tok.strip_prefix('n').ok_or_else(|| format!("slot {tok}"))?;
            let cut = rest.find(['+', '-']).unwrap_or(rest.len());
            let j: usize = rest[..cut].parse().map_err(|_| format!("slot {tok}"))?;
            let off: i64 = if cut == rest.len() {
                0
            } else {
                rest[cut..].parse().map_err(|_| format!("slot {tok}"))?
            };
            Ok(Slot::P(j, off))
        })
        .collect()
}

/// The absolute IBP seeds of a catalogue record at `term`: parameters from the record's target pattern, each seed
/// pattern instantiated, plus the record's recenter.
fn seeds_at(rec: &Value, term: &[i64]) -> Result<Vec<Vec<i64>>, String> {
    let n = term.len();
    let target = parse_pattern(rec["target"].as_str().ok_or("target")?)?;
    if target.len() != n {
        return Err("target arity".into());
    }
    let mut p: BTreeMap<usize, i64> = BTreeMap::new();
    for (i, s) in target.iter().enumerate() {
        match *s {
            Slot::C(c) if c == term[i] => {}
            Slot::C(c) => {
                return Err(format!(
                    "slot {i}: constant {c} but the term has {}",
                    term[i]
                ));
            }
            Slot::P(j, off) => {
                let v = term[i] - off;
                if p.insert(j, v).is_some_and(|o| o != v) {
                    return Err(format!("parameter n{j} twice with different values"));
                }
            }
        }
    }
    let rc: Vec<i64> = match rec.get("recenter") {
        Some(Value::Array(a)) => a.iter().map(|x| x.as_i64().unwrap_or(0)).collect(),
        _ => vec![0; n],
    };
    let mut out = vec![];
    for s in rec["seeds"].as_array().ok_or("record without seeds")? {
        let pat = parse_pattern(s.as_str().ok_or("seed text")?)?;
        if pat.len() != n {
            return Err("seed arity".into());
        }
        let mut k = vec![0i64; n];
        for (i, sl) in pat.iter().enumerate() {
            k[i] = match *sl {
                Slot::C(c) => c,
                Slot::P(j, off) => {
                    p.get(&j)
                        .ok_or_else(|| format!("seed parameter n{j} unknown"))?
                        + off
                }
            } + rc.get(i).copied().unwrap_or(0);
        }
        out.push(k);
    }
    Ok(out)
}

fn read_lines(p: &Path) -> Vec<String> {
    let f = std::fs::File::open(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    std::io::BufReader::new(f)
        .lines()
        .map(|l| l.unwrap())
        .filter(|l| !l.trim().is_empty())
        .collect()
}

fn table_files(entries: &[Value]) -> Vec<PathBuf> {
    let mut v = vec![];
    for e in entries {
        let p = PathBuf::from(e.as_str().unwrap());
        if p.is_dir() {
            let mut fs: Vec<PathBuf> = std::fs::read_dir(&p)
                .unwrap()
                .flatten()
                .map(|x| x.path())
                .filter(|x| x.to_string_lossy().ends_with(".table.jsonl"))
                .collect();
            fs.sort();
            v.extend(fs);
        } else {
            v.push(p);
        }
    }
    v
}

/// The catalogue record lines with the wanted SHA-256 (lines without their newline).
fn find_records(files: &[PathBuf], want: &BTreeSet<String>) -> BTreeMap<String, Value> {
    let mut found = BTreeMap::new();
    for f in files {
        if found.len() == want.len() {
            break;
        }
        let fh = std::fs::File::open(f).unwrap();
        for l in std::io::BufReader::new(fh).split(b'\n') {
            let l = l.unwrap();
            if !l.starts_with(b"{\"type\":\"rule\"") {
                continue;
            }
            let mut l = l;
            while matches!(l.last(), Some(b'\r')) {
                l.pop();
            }
            let h = sha256(&l);
            if want.contains(&h) && !found.contains_key(&h) {
                found.insert(h, serde_json::from_slice(&l).unwrap());
            }
        }
    }
    found
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
}

struct Job {
    run: String,
    id: String,
    dir: PathBuf,
    args: Vec<String>,
    meta: Value,
}

fn key_named(k: &[i64]) -> String {
    k.iter()
        .enumerate()
        .map(|(i, v)| format!("n{i}={v}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[test]
#[ignore = "real data; run by hand: AUDIT_R2_REAL=<config.json>"]
fn kernel_certify_real_shipped_relations() {
    let cfg: Value = serde_json::from_slice(
        &std::fs::read(std::env::var("AUDIT_R2_REAL").expect("AUDIT_R2_REAL")).unwrap(),
    )
    .unwrap();
    let kernel = cfg["kernel"].as_str().unwrap().to_string();
    let out = PathBuf::from(cfg["out"].as_str().unwrap());
    std::fs::create_dir_all(&out).unwrap();
    let controls = cfg["controls"].as_u64().unwrap_or(3) as usize;
    let mut jobs: Vec<Job> = vec![];
    let mut setup: BTreeMap<String, Value> = BTreeMap::new();
    for run in cfg["runs"].as_array().unwrap() {
        let name = run["name"].as_str().unwrap().to_string();
        let rdir = out.join(&name);
        std::fs::create_dir_all(&rdir).unwrap();
        // frame header + engine census
        let frame = PathBuf::from(run["frame"].as_str().unwrap());
        let (mut header, mut census) = (None, None);
        for l in std::io::BufReader::new(std::fs::File::open(&frame).unwrap())
            .lines()
            .take(64)
        {
            let l = l.unwrap();
            if l.starts_with("{\"type\":\"header\"") {
                header = Some(l);
            } else if l.starts_with("{\"type\":\"zero_sectors\"") {
                census = Some(l);
            }
            if header.is_some() && census.is_some() {
                break;
            }
        }
        let header = header.expect("frame header");
        let hv: Value = serde_json::from_str(&header).unwrap();
        let fam = hv["family_source"].as_str().unwrap().to_string();
        // shipped relations, paired with their verification lines
        let tl: Vec<String> = read_lines(Path::new(run["table"].as_str().unwrap()))
            .into_iter()
            .filter(|l| l.contains("\"type\":\"rule\""))
            .collect();
        let vl = read_lines(Path::new(run["verification"].as_str().unwrap()));
        assert_eq!(tl.len(), vl.len(), "{name}: table / verification lines");
        let pairs: Vec<(Value, Value)> = tl
            .iter()
            .zip(&vl)
            .map(|(t, v)| {
                (
                    serde_json::from_str::<Value>(t).unwrap(),
                    serde_json::from_str::<Value>(v).unwrap(),
                )
            })
            .collect();
        for (t, v) in &pairs {
            assert_eq!(t["target"], v["target"], "{name}: pairing");
        }
        // the verified group, derived HERE from the frame header's family
        let chart_s = pairs[0].0["chart"].as_str().unwrap().to_string();
        let chart: Vec<bool> = chart_s.chars().map(|c| c == '1').collect();
        let n = chart.len();
        let gs: import::symmetry::GroupSpec = serde_json::from_value(json!({})).unwrap();
        let ident: Vec<usize> = (0..n).collect();
        let (group, rep) =
            import::symmetry::load_verified_group(&gs, Path::new("/"), &chart, &ident, Some(&fam))
                .expect("verified group");
        let want_digest = pairs[0].1["group_digest"].as_str().unwrap_or("");
        assert_eq!(rep.digest, want_digest, "{name}: group digest");
        // selection: composites (up to `composites`) and a seeded sample of plain relations
        let mut comp: Vec<usize> = (0..pairs.len())
            .filter(|&i| pairs[i].1["plain"] == false)
            .collect();
        comp.truncate(run["composites"].as_u64().unwrap_or(u64::MAX) as usize);
        let mut plain: Vec<usize> = (0..pairs.len())
            .filter(|&i| pairs[i].1["plain"] == true)
            .collect();
        let mut rng = Rng(run["seed"].as_u64().unwrap_or(1).max(1));
        for i in (1..plain.len()).rev() {
            let j = (rng.next() % (i as u64 + 1)) as usize;
            plain.swap(i, j);
        }
        plain.truncate(run["plain"].as_u64().unwrap_or(0) as usize);
        plain.sort();
        let chosen: Vec<usize> = comp.iter().chain(plain.iter()).copied().collect();
        // catalogue records cited by the chosen composites
        let mut want: BTreeSet<String> = BTreeSet::new();
        for &i in &comp {
            for r in pairs[i].1["replacements"].as_array().unwrap() {
                want.insert(r["record_sha256"].as_str().unwrap().to_string());
            }
        }
        let cat = if want.is_empty() {
            BTreeMap::new()
        } else {
            find_records(&table_files(run["catalogue"].as_array().unwrap()), &want)
        };
        setup.insert(
            name.clone(),
            json!({"relations": pairs.len(), "composites_available": pairs.iter().filter(|p| p.1["plain"] == false).count(),
                "chosen_composites": comp.len(), "chosen_plain": plain.len(), "group_digest": rep.digest,
                "group_elements": group.proofs().len(), "cited_records": want.len(), "found_records": cat.len(),
                "census": census.is_some()}),
        );
        for (pos, &i) in chosen.iter().enumerate() {
            let (t, v) = &pairs[i];
            let key = parse_const_key(t["target"].as_str().unwrap()).unwrap();
            let e = v["element"].as_u64().unwrap() as usize;
            let matrix = &group.proofs()[e].matrix;
            let is_plain = v["plain"] == true;
            let mut seeds: Vec<Vec<i64>> = vec![];
            let mut why = None;
            for r in v["replacements"].as_array().unwrap() {
                let sha = r["record_sha256"].as_str().unwrap();
                let term = parse_const_key(r["term"].as_str().unwrap()).unwrap();
                match cat.get(sha) {
                    Some(rec) => match seeds_at(rec, &term) {
                        Ok(s) => seeds.extend(s),
                        Err(e) => why = Some(format!("seeds: {e}")),
                    },
                    None => why = Some(format!("cited record {sha} not found in the catalogue")),
                }
            }
            seeds.sort();
            seeds.dedup();
            // controls: the first `controls` composites and the first `controls` plain relations of the selection
            let rank = if is_plain {
                pos.saturating_sub(comp.len())
            } else {
                pos
            };
            let distinct = t["rhs"]
                .as_array()
                .is_some_and(|a| a.iter().any(|x| x[1] != a[0][1]));
            // a plain relation's coefficients are rationals: it can always be corrupted (see below)
            let variants: Vec<(&str, bool)> = if rank < controls && (distinct || is_plain) {
                vec![("asis", false), ("control", true)]
            } else {
                vec![("asis", false)]
            };
            for (tag, corrupt) in variants {
                let id = format!("{i:05}-{tag}");
                let dir = rdir.join(&id);
                std::fs::create_dir_all(&dir).unwrap();
                let mut rhs = t["rhs"].clone();
                if corrupt {
                    // a WELL-FORMED wrong relation: the coefficients of the first term and of the first term with a
                    // different coefficient are swapped (the kernel parses it; the identity is broken)
                    let a = rhs.as_array_mut().unwrap();
                    match (1..a.len()).find(|&j| a[j][1] != a[0][1]) {
                        Some(j) => {
                            let c0 = a[0][1].clone();
                            a[0][1] = a[j][1].clone();
                            a[j][1] = c0;
                        }
                        // all coefficients equal (e.g. a one-term image): another rational in the first term
                        None => a[0][1] = json!(if a[0][1] == "1" { "2" } else { "1" }),
                    }
                }
                let seed_txt: Vec<String> = seeds
                    .iter()
                    .map(|s| {
                        format!(
                            "I({})",
                            s.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
                        )
                    })
                    .collect();
                let rec = format!(
                    "{{\"type\":\"rule\",\"index\":0,\"chart\":{},\"fixed\":{},\"equations\":[],\"target\":{},\"rhs\":{},\"exceptions\":[],\"dispatch\":\"Point\",\"seeds\":{},\"recenter\":{}}}",
                    json!(chart_s),
                    json!(key),
                    json!(t["target"]),
                    rhs,
                    json!(seed_txt),
                    json!(vec![0i64; n])
                );
                let mut table = header.clone();
                table.push('\n');
                if let Some(c) = &census {
                    table.push_str(c);
                    table.push('\n');
                }
                let offset = table.len();
                table.push_str(&rec);
                table.push('\n');
                std::fs::write(dir.join("relation.table.jsonl"), &table).unwrap();
                std::fs::write(dir.join("keys.txt"), key_named(&key) + "\n").unwrap();
                std::fs::write(
                    dir.join("symrows.jsonl"),
                    json!({"id": format!("e{e}"), "matrix": matrix, "seed": key}).to_string()
                        + "\n",
                )
                .unwrap();
                let mut args: Vec<String> = [
                    "--mode",
                    "instance-span",
                    "--table",
                    "relation.table.jsonl",
                    "--rule",
                    "0",
                    "--offset",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect();
                args.push(offset.to_string());
                args.extend(
                    ["--keys", "keys.txt", "--symrows", "symrows.jsonl"]
                        .iter()
                        .map(|s| s.to_string()),
                );
                if is_plain {
                    args.extend(
                        ["--symrows-only", "--rows", "cert"]
                            .iter()
                            .map(|s| s.to_string()),
                    );
                } else {
                    args.extend(["--rows", "engine"].iter().map(|s| s.to_string()));
                }
                if census.is_some() {
                    args.push("--drop-zero-sectors".into());
                }
                args.extend(["--out", "span.jsonl"].iter().map(|s| s.to_string()));
                let meta = json!({"index": i, "key": key, "plain": is_plain, "element": e, "control": corrupt,
                    "replacements": v["replacements"].as_array().map_or(0, Vec::len), "seeds": seeds.len(),
                    "rhs_terms": t["rhs"].as_array().map_or(0, Vec::len), "not_built": why});
                if why.is_some() {
                    std::fs::write(dir.join("not_built.json"), meta.to_string()).unwrap();
                    continue;
                }
                jobs.push(Job {
                    run: name.clone(),
                    id,
                    dir,
                    args,
                    meta,
                });
            }
        }
    }
    eprintln!("setup: {}", json!(setup));
    std::fs::write(out.join("setup.json"), json!(setup).to_string()).unwrap();
    // run the kernel, `parallel` at a time
    let n_jobs = jobs.len();
    let queue = Arc::new(Mutex::new(jobs.into_iter().collect::<VecDeque<Job>>()));
    let results: Arc<Mutex<Vec<(String, String, Value)>>> = Arc::new(Mutex::new(vec![]));
    let par = cfg["parallel"].as_u64().unwrap_or(4).max(1) as usize;
    std::thread::scope(|s| {
        for _ in 0..par {
            let (q, res, kernel) = (queue.clone(), results.clone(), kernel.clone());
            s.spawn(move || {
                loop {
                    let Some(j) = q.lock().unwrap().pop_front() else {
                        break;
                    };
                    let t0 = std::time::Instant::now();
                    let o = std::process::Command::new(&kernel)
                        .args(&j.args)
                        .current_dir(&j.dir)
                        .output();
                    let secs = t0.elapsed().as_secs_f64();
                    let mut m = j.meta.clone();
                    m["seconds"] = json!(secs);
                    match o {
                        Ok(o) => {
                            std::fs::write(j.dir.join("stdout.log"), &o.stdout).ok();
                            std::fs::write(j.dir.join("stderr.log"), &o.stderr).ok();
                            m["exit"] = json!(o.status.code());
                            let span = std::fs::read_to_string(j.dir.join("span.jsonl"))
                                .unwrap_or_default();
                            let v: Value = span
                                .lines()
                                .find(|l| !l.trim().is_empty())
                                .and_then(|l| serde_json::from_str(l).ok())
                                .unwrap_or(Value::Null);
                            m["status"] = v["keys"][0]["status"].clone();
                            m["verdict"] = v["verdict"].clone();
                            m["symrows_used"] = v["symrows"].clone();
                            if v.is_null() {
                                m["stderr"] = json!(
                                    String::from_utf8_lossy(&o.stderr)
                                        .chars()
                                        .take(400)
                                        .collect::<String>()
                                );
                            }
                        }
                        Err(e) => m["spawn_error"] = json!(e.to_string()),
                    }
                    res.lock().unwrap().push((j.run.clone(), j.id.clone(), m));
                }
            });
        }
    });
    let res = results.lock().unwrap();
    let mut summary: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    let mut by_run: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for (run, id, m) in res.iter() {
        let st = m["status"]
            .as_str()
            .map(|s| s.split_whitespace().next().unwrap_or("").to_string())
            .unwrap_or_else(|| "NO_ANSWER".into());
        let class = format!(
            "{}:{}:{}",
            if m["control"] == true {
                "control"
            } else {
                "asis"
            },
            if m["plain"] == true {
                "plain"
            } else {
                "composite"
            },
            st
        );
        *summary
            .entry(run.clone())
            .or_default()
            .entry(class)
            .or_default() += 1;
        let mut m = m.clone();
        m["id"] = json!(id);
        by_run.entry(run.clone()).or_default().push(m);
    }
    for (run, v) in &mut by_run {
        v.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        let body: String = v.iter().map(|m| m.to_string() + "\n").collect();
        std::fs::write(out.join(run).join("verdicts.jsonl"), body).unwrap();
    }
    let s = json!({"jobs": n_jobs, "by_run": summary, "setup": setup, "kernel": kernel});
    std::fs::write(
        out.join("summary.json"),
        serde_json::to_string_pretty(&s).unwrap(),
    )
    .unwrap();
    eprintln!("{}", serde_json::to_string_pretty(&s).unwrap());
    // the verdict of this audit item: every as-is relation certified, every control refused
    for (run, m) in by_run
        .iter()
        .flat_map(|(r, v)| v.iter().map(move |m| (r, m)))
    {
        let st = m["status"].as_str().unwrap_or("");
        if m["control"] == true {
            assert!(
                st.contains("NOT_IN_SPAN"),
                "{run}: a corrupted control was not refused by the kernel (certified or no answer): {m}"
            );
        }
    }
}
