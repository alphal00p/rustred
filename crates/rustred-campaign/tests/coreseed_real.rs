//! Lane coreseed: the REAL IBP-core flow on given face cores (interfaces/coreseed.md), engine children through the
//! adapter and the process layer, the closing loop's accept chain on the node's import set. `#[ignore]`: run by hand
//! INSIDE A PRIVATE PID NAMESPACE (rule L10) after `source WS/ops/env.sh`, with `CORESEED_REAL_CONFIG` = a JSON file:
//! `{"node", "import_spec", "import_base", "program", "family", "order": "<name>" | {order JSON}, "valuation": path |
//!   null, "policy": {..} | null, "faces": [{"name", "keys": path (lines "I(..)" or "[..]", optional TAB + text)}],
//!   "cpus": [..] | null (HELD_CPUS), "work_root", "out_dir", "keys_per_job", "cap_bytes", "key_budget_s",
//!   "wall_cap_s"}`.
//! Per face: `out_dir/<face>/{report.json, journal.jsonl, costs.jsonl, progress.jsonl}` and one line of
//! `out_dir/summary.jsonl`. Re-running with the same out_dir resumes from the journals.

use rustred_campaign::adapters::job::{Invocation, Program};
use rustred_campaign::closeloop::coreseed::{ChildSolver, CoreDriverConfig, DriverJudge};
use rustred_campaign::resources::{Sampler, SamplerConfig};
use rustred_campaign_core::coreseed::spec::CoreSeedPolicy;
use rustred_campaign_core::coreseed::{Key, solve_core};
use rustred_campaign_core::import::TableSetSpec;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn read_keys(p: &Path) -> Vec<Key> {
    let f = std::fs::File::open(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let mut out = vec![];
    for l in std::io::BufReader::new(f).lines().map_while(Result::ok) {
        let t = l.split('\t').next().unwrap_or("").trim().to_string();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let k: Key = if let Some(inner) = t.strip_prefix("I(").and_then(|x| x.strip_suffix(')')) {
            inner
                .split(',')
                .map(|x| x.trim().parse().unwrap())
                .collect()
        } else {
            serde_json::from_str(&t).unwrap_or_else(|e| panic!("{t}: {e}"))
        };
        out.push(k);
    }
    out
}

fn utc() -> String {
    let o = std::process::Command::new("date")
        .args(["-u", "+%FT%TZ"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    o
}

#[test]
#[ignore = "real children: CORESEED_REAL_CONFIG, private pid namespace"]
fn real_core_faces() {
    assert!(
        rustred_campaign::launch::in_private_namespace(),
        "run inside a private pid namespace (L10)"
    );
    let p = std::env::var("CORESEED_REAL_CONFIG").expect("CORESEED_REAL_CONFIG");
    let c: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    let s = |k: &str| c[k].as_str().unwrap_or_else(|| panic!("{k}")).to_string();
    let cpus: Vec<usize> = match c["cpus"].as_array() {
        Some(a) => a.iter().map(|x| x.as_u64().unwrap() as usize).collect(),
        None => std::env::var("HELD_CPUS")
            .expect("HELD_CPUS")
            .split(',')
            .map(|x| x.trim().parse().unwrap())
            .collect(),
    };
    let out = PathBuf::from(s("out_dir"));
    std::fs::create_dir_all(&out).unwrap();
    let order = match &c["order"] {
        Value::String(x) => x.clone(),
        v => v.to_string(),
    };
    let policy: CoreSeedPolicy = match &c["policy"] {
        Value::Null => CoreSeedPolicy::default(),
        v => serde_json::from_value(v.clone()).expect("policy"),
    };
    policy.validate().unwrap();
    let t0 = Instant::now();
    let spec = TableSetSpec::read(Path::new(&s("import_spec"))).unwrap();
    let mut set = rustred_campaign_core::import::load(&spec, Path::new(&s("import_base"))).unwrap();
    let lines: Vec<bool> = set.frame.mask().to_vec();
    eprintln!(
        "[coreseed {}] import set loaded in {:.1} s",
        utc(),
        t0.elapsed().as_secs_f64()
    );
    let sampler = Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(500),
            collect_foreign_pinning: false,
        })
        .unwrap(),
    );
    let program =
        Program::from_file("rr_node", Path::new(&s("program")), Invocation::Contract).unwrap();
    let valuation = c["valuation"].as_str().map(PathBuf::from);
    for face in c["faces"].as_array().unwrap() {
        let name = face["name"].as_str().unwrap().to_string();
        let fdir = out.join(rustred_campaign::closeloop::ids::safe(&name));
        std::fs::create_dir_all(&fdir).unwrap();
        let keys = read_keys(Path::new(face["keys"].as_str().unwrap()));
        let tf = Instant::now();
        eprintln!("[coreseed {}] face {name}: {} keys", utc(), keys.len());
        let cfg = CoreDriverConfig {
            node: s("node"),
            program: program.clone(),
            family: s("family").into(),
            order: order.clone(),
            cpus: cpus.clone(),
            work_root: PathBuf::from(s("work_root"))
                .join(rustred_campaign::closeloop::ids::safe(&name)),
            journal: fdir.join("journal.jsonl"),
            keys_per_job: c["keys_per_job"].as_u64().unwrap_or(4) as usize,
            cap_bytes: c["cap_bytes"].as_u64().unwrap_or(1_950_000_000),
            key_budget_s: c["key_budget_s"].as_u64().or(Some(1800)),
            max_trace_rows: c["max_trace_rows"].as_u64(),
            wall_cap_s: c["wall_cap_s"].as_u64().unwrap_or(7200),
            licence_env: std::env::var_os("SYMBOLICA_LICENSE")
                .map(|_| "SYMBOLICA_LICENSE".to_string()),
            d0: policy.d0.clone(),
            valuation: valuation.clone(),
            keep_trial_outputs: false,
            verbose: true,
            fake: None,
        };
        let judge = DriverJudge {
            valuation: valuation.clone(),
        };
        let mut solver = ChildSolver::new(cfg, &mut set, Box::new(judge), sampler.clone()).unwrap();
        let r = match solve_core(&keys, &lines, &policy, &mut solver) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[coreseed {}] face {name}: ERROR {e}", utc());
                let mut f = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(out.join("summary.jsonl"))
                    .unwrap();
                writeln!(f, "{}", json!({"face": name, "error": e, "utc": utc()})).unwrap();
                continue;
            }
        };
        std::fs::write(
            fdir.join("report.json"),
            serde_json::to_vec_pretty(&r).unwrap(),
        )
        .unwrap();
        // CPU time of the face from the cost file
        let mut cpu_s = 0.0;
        if let Ok(f) = std::fs::File::open(fdir.join("costs.jsonl")) {
            for l in std::io::BufReader::new(f).lines().map_while(Result::ok) {
                if let Ok(v) = serde_json::from_str::<Value>(&l) {
                    cpu_s += v["cpu_s"].as_f64().unwrap_or(0.0);
                }
            }
        }
        let mut by_first: BTreeMap<String, usize> = BTreeMap::new();
        let mut d0: BTreeMap<String, usize> = BTreeMap::new();
        let mut rhs: Vec<u64> = vec![];
        for k in &r.keys {
            *by_first
                .entry(
                    k.first_solved_by
                        .clone()
                        .unwrap_or_else(|| k.status.clone()),
                )
                .or_default() += 1;
            if let Some(i) = k.chosen {
                let a = &k.alternatives[i];
                *d0.entry(a.d0.word().into()).or_default() += 1;
                rhs.push(a.rhs_terms);
            }
        }
        rhs.sort();
        let sum = json!({"face": name, "keys": keys.len(), "totals": r.totals, "first_solved_by": by_first,
            "chosen_d0": d0, "chosen_rhs_min_med_max": [rhs.first(), rhs.get(rhs.len() / 2), rhs.last()],
            "pieces": r.groups.iter().map(|g| g.pieces.iter().map(|p| json!({"id": p.id, "kind": p.kind, "order": p.order,
                "seeds": p.list.len(), "domain": p.domain.len(), "failed": p.failed.len(), "d0": p.d0,
                "fixed_in_piece": p.fixed_in_piece})).collect::<Vec<_>>()).collect::<Vec<_>>(),
            "cpu_s": cpu_s, "wall_s": tf.elapsed().as_secs_f64(), "requests_solved": solver.requests_solved,
            "requests_journaled": solver.requests_journaled, "utc": utc()});
        eprintln!("[coreseed {}] FACE DONE {sum}", utc());
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(out.join("summary.jsonl"))
            .unwrap();
        writeln!(f, "{sum}").unwrap();
    }
}

/// The CORE of a face derived from the box bound, the tables and the group (TRICKS 4 / 14 b): every key of the face
/// signature with dots + rank <= `max_dr` that the node's dispatch does not serve and that has NO one-term symmetry
/// image strictly lower under the node comparator (a key with such an image is a hand-down, into another face or into
/// its own core). Only keys whose nonzero slots the element maps exactly (a line to a line, a numerator to a
/// numerator) are imaged; an element that is not exact on a nonzero slot is skipped (recorded). `CORESEED_DERIVE_CONFIG`:
/// `{"import_spec", "import_base", "face": {"dotted": [..], "numerators": [..]}, "max_dr", "out", "compare": path |
/// null (a key list: its keys must be exactly the derived core keys with dots + rank <= compare_dr), "compare_dr"}`.
#[test]
#[ignore = "real data: CORESEED_DERIVE_CONFIG"]
fn derive_face_core() {
    use rustred_campaign_core::replay_base::symrows::SlotImage;
    let p = std::env::var("CORESEED_DERIVE_CONFIG").expect("CORESEED_DERIVE_CONFIG");
    let c: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    let base = PathBuf::from(c["import_base"].as_str().unwrap());
    let spec = TableSetSpec::read(Path::new(c["import_spec"].as_str().unwrap())).unwrap();
    let set = rustred_campaign_core::import::load(&spec, &base).unwrap();
    let lines: Vec<bool> = set.frame.mask().to_vec();
    let n = lines.len();
    let dotted: Vec<usize> = serde_json::from_value(c["face"]["dotted"].clone()).unwrap();
    let nums: Vec<usize> = serde_json::from_value(c["face"]["numerators"].clone()).unwrap();
    let max_dr = c["max_dr"].as_i64().unwrap();
    let tm = rustred_campaign_core::termmin::TermminSpec::default();
    let group = rustred_campaign::closeloop::termmin::termmin_group(&tm, &spec, &base, &set)
        .unwrap()
        .expect("a verified group");
    let elements = group.group().elements.clone();
    // enumerate the face: dotted lines >= 2, numerators <= -1, other slots at the frame base; dots + rank <= max_dr
    let free: Vec<usize> = dotted.iter().chain(&nums).copied().collect();
    let mut keys: Vec<Key> = vec![];
    let mut cur = vec![0i64; free.len()];
    fn rec(i: usize, left: i64, cur: &mut Vec<i64>, out: &mut Vec<Vec<i64>>) {
        if i == cur.len() {
            out.push(cur.clone());
            return;
        }
        for v in 1..=left {
            cur[i] = v;
            rec(i + 1, left - v, cur, out);
        }
    }
    let mut steps = vec![];
    rec(0, max_dr, &mut cur, &mut steps);
    for st in steps {
        let mut k: Key = (0..n).map(|s| if lines[s] { 1 } else { 0 }).collect();
        for (j, &s) in free.iter().enumerate() {
            k[s] = if lines[s] {
                1 + st[j] as i32
            } else {
                -(st[j] as i32)
            };
        }
        keys.push(k);
    }
    keys.sort();
    let mut served = 0usize;
    let mut handed: Vec<(Key, Key, usize)> = vec![];
    let mut core: Vec<Key> = vec![];
    let mut skipped_elements = 0usize;
    for k in &keys {
        if set
            .disp
            .first_match(k)
            .map(|d| d.is_covered())
            .unwrap_or(false)
        {
            served += 1;
            continue;
        }
        let mut best: Option<(Key, usize)> = None;
        for (ei, e) in elements.iter().enumerate() {
            // the image starts at the frame base; every nonzero slot (and every line) is sent to its target
            let mut img: Key = (0..n).map(|s| if lines[s] { 1 } else { 0 }).collect();
            let mut ok = true;
            // lines at power 1 must also map exactly (their propagator stays); a zero numerator slot may map to anything
            for s in 0..n {
                if !lines[s] && k[s] == 0 {
                    continue;
                }
                match &e.action[s] {
                    SlotImage::Exact { target, .. } if lines[*target] == lines[s] => {
                        img[*target] = k[s]
                    }
                    _ => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                skipped_elements += 1;
                continue;
            }
            if set.cmp.cmp(&img, k) == std::cmp::Ordering::Less
                && best
                    .as_ref()
                    .is_none_or(|(b, _)| set.cmp.cmp(&img, b) == std::cmp::Ordering::Less)
            {
                best = Some((img, ei));
            }
        }
        match best {
            Some((img, ei)) => handed.push((k.clone(), img, ei)),
            None => core.push(k.clone()),
        }
    }
    let out = PathBuf::from(c["out"].as_str().unwrap());
    std::fs::create_dir_all(&out).unwrap();
    let mut f = std::fs::File::create(out.join("core.keys")).unwrap();
    for k in &core {
        writeln!(
            f,
            "I({})",
            k.iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(",")
        )
        .unwrap();
    }
    let dr = |k: &Key| -> i64 {
        k.iter()
            .zip(&lines)
            .map(|(&v, &l)| {
                if l {
                    (v - 1).max(0) as i64
                } else {
                    (-v).max(0) as i64
                }
            })
            .sum()
    };
    let mut by_layer: BTreeMap<i64, (usize, usize)> = BTreeMap::new();
    for k in &core {
        by_layer.entry(dr(k)).or_default().0 += 1;
    }
    for (k, _, _) in &handed {
        by_layer.entry(dr(k)).or_default().1 += 1;
    }
    let mut compare = Value::Null;
    if let Some(cp) = c["compare"].as_str() {
        let cdr = c["compare_dr"].as_i64().unwrap_or(max_dr);
        let given: std::collections::BTreeSet<Key> = read_keys(Path::new(cp)).into_iter().collect();
        let ours: std::collections::BTreeSet<Key> =
            core.iter().filter(|k| dr(k) <= cdr).cloned().collect();
        compare = json!({"given": given.len(), "ours_upto_dr": ours.len(), "equal": given == ours,
            "only_given": given.difference(&ours).take(10).collect::<Vec<_>>(),
            "only_ours": ours.difference(&given).take(10).collect::<Vec<_>>()});
    }
    let rep = json!({"face": c["face"], "max_dr": max_dr, "keys": keys.len(), "served": served,
        "handed_down": handed.len(), "core": core.len(), "skipped_element_applications": skipped_elements,
        "group_elements": elements.len(), "by_layer_core_handed": by_layer, "compare": compare,
        "handdown_examples": handed.iter().take(5).map(|(k, i, e)| json!({"key": k, "image": i, "element": e})).collect::<Vec<_>>()});
    std::fs::write(
        out.join("derive.json"),
        serde_json::to_vec_pretty(&rep).unwrap(),
    )
    .unwrap();
    eprintln!("{rep}");
}
