//! `rr_poles`: exact pole orders at d = d0 of relations (lane poles; interfaces/pole-diagnostics.md 2; the bridge for
//! callers without a dependency on this crate). A DIAGNOSTIC child: its answers are never rule or relation evidence.
//!
//! ```text
//! rr_poles regular-at --requests R.jsonl --out A.jsonl [--d0 4] [--threads N]
//! rr_poles regular-at --results results.jsonl --out A.jsonl [--d0 4] [--threads N]
//! rr_poles d1 --results results.jsonl --node NODE --out relations.jsonl [--d0 4] [--worst-max 20] [--threads N]
//! rr_poles census --statement-keys keys.jsonl --catalogue catalogue.json --out DIR [--d0 4] [--threads N]
//! rr_poles basis --map MAP.json --node N --label L --out REC.json [--d0 4] [--symmasters OUT.json --terminals 'K;..']
//! rr_poles version
//! ```
//! BASIS (B2): MAP.json = a master map `T = M R` (forms of `valuation::basis::read_master_map`); writes the
//! `rustred.campaign.pole-basis.v1` record: the exact check (route X: det order, M^-1 pole orders, rank of M(d0), null
//! directions) and, with `--symmasters`, the route E SCREEN from an `rr_node symmasters` report (terminals = --masters =
//! --prefer) through the node-basis adapter; the screen never decides.
//! CENSUS (B4, route poles): `keys.jsonl` = a closure statement's served keys, one JSON list per line
//! `[key text "I(..)", class, label | null, in_box, ..]`; `catalogue.json` = `{"entries": [{name, table, offset}, ..]}`
//! (the record of `label` is the JSON line at byte `offset` of `table`). Per served key: the serving record's
//! coefficients evaluated EXACTLY at the key (indices put in), its pole order at d0. Writes DIR/census.summary.json
//! (`rustred.campaign.route-pole-census.v1`), DIR/rules.jsonl (one line per used rule: record sha256, keys, keys with a
//! pole, max order), DIR/census.rows.jsonl (keys with a pole or undecided). Route poles only: never a statement about
//! the eps need of terminals.
//! REQUEST (one JSON object per line, `rustred.campaign.pole-request.v1`):
//! `{"id": "<caller's id>", "key": [..], "d0": "4"?, "record": {<relation record>}}` where the record is an engine
//! solve-recipe `relation` record, a table `rule` record (the key's indices are put in; the caller ensures the rule
//! applies at the key) or an unnormalised `{"target_coefficient", "rhs"}` relation (see `valuation::relation`). `d0`
//! defaults to `--d0`. `--results` reads an engine results file instead: every `"type":"relation"` record at its own key
//! (`id` = its `ordinal`, else the line number).
//! ANSWER (one line per request, in request order, `rustred.campaign.pole-answer.v1`):
//! `{"schema", "id", "key", "d0", "verdict": "regular"|"pole"|"undecided", "pole_order": p|null, "reason"?: "..",
//!   "relation": {relation_digest, entry_digest, recipe_digest, determinism_id, relation_rule_taken, seeds_used_sha256,
//!   evaluated_sha256}}` = the order of THIS relation (identified by those digests, incl. its recipe's seed / row order and
//! the digest of the very texts evaluated; TRICKS 17) at this key: never a statement about the key or the system of rows,
//! and never derived from a probe at d0. `d1` writes `rustred.campaign.pole-relation.v1` records instead.
//! Output: written to OUT.part, then renamed. Errors: one `{"verdict":"ERROR","error":..}` line on stdout, exit 2.
//! Exit 0 = every request answered (undecided is an answer). The Symbolica licence must be in the environment
//! (`source WS/ops/env.sh`); without it the program refuses (exit 2).

use rustred_campaign_algebra::licensed;
use rustred_campaign_algebra::valuation::D0;
use rustred_campaign_algebra::valuation::relation::{is_regular_at, relation_record};
use rustred_campaign_core::poles::record::{RegularAt, relation_identity};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

const ANSWER_SCHEMA: &str = "rustred.campaign.pole-answer.v1";

fn die(msg: &str) -> ! {
    println!("{}", json!({"verdict": "ERROR", "error": msg}));
    std::process::exit(2)
}

#[derive(Default)]
struct Args {
    requests: Option<PathBuf>,
    results: Option<PathBuf>,
    out: Option<PathBuf>,
    d0: String,
    node: Option<String>,
    worst_max: usize,
    threads: usize,
    statement_keys: Option<PathBuf>,
    catalogue: Option<PathBuf>,
    map: Option<PathBuf>,
    label: Option<String>,
    symmasters: Option<PathBuf>,
    terminals: Option<String>,
}

fn parse(args: &[String]) -> Args {
    let mut a = Args {
        d0: "4".into(),
        worst_max: 20,
        threads: 1,
        ..Args::default()
    };
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        let mut val = || {
            i += 1;
            args.get(i)
                .cloned()
                .unwrap_or_else(|| die(&format!("{flag} needs a value")))
        };
        match flag {
            "--requests" => a.requests = Some(PathBuf::from(val())),
            "--results" => a.results = Some(PathBuf::from(val())),
            "--out" => a.out = Some(PathBuf::from(val())),
            "--d0" => a.d0 = val(),
            "--node" => a.node = Some(val()),
            "--statement-keys" => a.statement_keys = Some(PathBuf::from(val())),
            "--catalogue" => a.catalogue = Some(PathBuf::from(val())),
            "--map" => a.map = Some(PathBuf::from(val())),
            "--label" => a.label = Some(val()),
            "--symmasters" => a.symmasters = Some(PathBuf::from(val())),
            "--terminals" => a.terminals = Some(val()),
            "--worst-max" => {
                a.worst_max = val()
                    .parse()
                    .unwrap_or_else(|_| die("--worst-max: not an integer"))
            }
            "--threads" => {
                a.threads = val()
                    .parse()
                    .unwrap_or_else(|_| die("--threads: not an integer"));
                if !(1..=4).contains(&a.threads) {
                    die("--threads must be 1..4");
                }
            }
            other => die(&format!("unknown argument {other:?}")),
        }
        i += 1;
    }
    a
}

/// (id, key, d0 text, record)
type Req = (Value, Vec<i64>, String, Value);

fn key_of(v: &Value) -> Option<Vec<i64>> {
    v.as_array()?.iter().map(Value::as_i64).collect()
}

fn read_requests(a: &Args) -> Vec<Req> {
    let (path, results) = match (&a.requests, &a.results) {
        (Some(p), None) => (p, false),
        (None, Some(p)) => (p, true),
        _ => die("give exactly one of --requests / --results"),
    };
    let f = std::fs::File::open(path).unwrap_or_else(|e| die(&format!("{}: {e}", path.display())));
    let mut out = vec![];
    for (n, line) in BufReader::new(f).lines().enumerate() {
        let line = line.unwrap_or_else(|e| die(&format!("{}: {e}", path.display())));
        if line.trim().is_empty() {
            continue;
        }
        let v: Value =
            serde_json::from_str(&line).unwrap_or_else(|e| die(&format!("line {}: {e}", n + 1)));
        if results {
            if v["type"] != "relation" {
                continue;
            }
            let key = key_of(&v["key"])
                .unwrap_or_else(|| die(&format!("line {}: relation without key", n + 1)));
            let id = if v["ordinal"].is_null() {
                json!(n + 1)
            } else {
                v["ordinal"].clone()
            };
            out.push((id, key, a.d0.clone(), v));
        } else {
            let key = key_of(&v["key"])
                .unwrap_or_else(|| die(&format!("line {}: request without key", n + 1)));
            let d0 = v["d0"].as_str().map_or(a.d0.clone(), str::to_string);
            if !v["record"].is_object() {
                die(&format!("line {}: request without record", n + 1));
            }
            out.push((v["id"].clone(), key, d0, v["record"].clone()));
        }
    }
    out
}

fn answer(r: &Req, d1: Option<(&str, usize)>) -> Value {
    let (id, key, d0t, rec) = r;
    let d0 = match D0::parse(d0t) {
        Ok(d) => d,
        Err(e) => {
            return json!({"schema": ANSWER_SCHEMA, "id": id, "key": key, "d0": d0t, "verdict": "undecided",
                "pole_order": null, "reason": e.to_string(), "relation": relation_identity(rec)});
        }
    };
    if let Some((node, worst)) = d1 {
        let source = json!({"kind": "recipe", "id": rec["recipe"]["id"], "recipe_digest": rec["recipe_digest"],
            "relation_rule_taken": rec["relation_rule_taken"], "ordinal": id});
        return relation_record(rec, key, &d0, node, source, worst).to_json();
    }
    let v = is_regular_at(rec, key, &d0);
    let mut a = json!({"schema": ANSWER_SCHEMA, "id": id, "key": key, "d0": d0.text(), "verdict": v.word(),
        "pole_order": v.pole_order(), "relation": relation_identity(rec)});
    if let RegularAt::Undecided { reason } = &v {
        a["reason"] = json!(reason);
    }
    a
}

fn run(a: &Args, d1: bool) {
    let out = a.out.clone().unwrap_or_else(|| die("--out is required"));
    let node = a.node.clone();
    if d1 && node.is_none() {
        die("d1 needs --node");
    }
    let reqs = read_requests(a);
    let d1_args = node.as_deref().filter(|_| d1).map(|n| (n, a.worst_max));
    let threads = a.threads.min(reqs.len().max(1));
    let mut answers: Vec<Option<Value>> = vec![None; reqs.len()];
    std::thread::scope(|sc| {
        let hs: Vec<_> = (0..threads)
            .map(|t| {
                let reqs = &reqs;
                sc.spawn(move || {
                    (t..reqs.len())
                        .step_by(threads)
                        .map(|i| (i, answer(&reqs[i], d1_args)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for h in hs {
            for (i, v) in h.join().expect("worker thread") {
                answers[i] = Some(v);
            }
        }
    });
    let part = out.with_extension("part");
    let mut f = std::io::BufWriter::new(
        std::fs::File::create(&part).unwrap_or_else(|e| die(&format!("{}: {e}", part.display()))),
    );
    let mut counts = [0usize; 3];
    for v in answers.into_iter().map(|v| v.expect("answered")) {
        let w = if d1 {
            v["verdict"]["relation"].clone()
        } else {
            v["verdict"].clone()
        };
        match w.as_str() {
            Some("regular") => counts[0] += 1,
            Some("pole") => counts[1] += 1,
            _ => counts[2] += 1,
        }
        writeln!(f, "{v}").unwrap_or_else(|e| die(&format!("{}: {e}", part.display())));
    }
    f.flush()
        .unwrap_or_else(|e| die(&format!("{}: {e}", part.display())));
    drop(f);
    std::fs::rename(&part, &out).unwrap_or_else(|e| die(&format!("rename: {e}")));
    println!(
        "{}",
        json!({"verdict": "OK", "answers": counts.iter().sum::<usize>(), "regular": counts[0], "pole": counts[1],
            "undecided": counts[2], "out": out.display().to_string()})
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("version") => {
            println!(
                "{}",
                json!({"program": "rr_poles", "package_version": env!("CARGO_PKG_VERSION"),
                "answer_schema": ANSWER_SCHEMA, "method": rustred_campaign_algebra::valuation::VALUATION_METHOD})
            );
        }
        Some(cmd @ ("regular-at" | "d1" | "census" | "basis")) => {
            if !licensed() {
                die("Symbolica licence absent (source WS/ops/env.sh): rr_poles refuses to run");
            }
            if cmd == "census" {
                census(&parse(&args[1..]));
            } else if cmd == "basis" {
                basis(&parse(&args[1..]));
            } else {
                run(&parse(&args[1..]), cmd == "d1");
            }
        }
        _ => die("usage: rr_poles regular-at|d1|census|basis|version ... (see the source header)"),
    }
}

fn parse_key_text(t: &str) -> Option<Vec<i64>> {
    let body = t.trim().strip_prefix("I(")?.strip_suffix(')')?;
    body.split(',').map(|x| x.trim().parse().ok()).collect()
}

fn read_line_at(path: &str, offset: u64) -> Result<Vec<u8>, String> {
    use std::io::{Seek, SeekFrom};
    let mut f = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
    f.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("{path}: {e}"))?;
    let mut line = Vec::new();
    BufReader::new(f)
        .read_until(b'\n', &mut line)
        .map_err(|e| format!("{path}: {e}"))?;
    while line.last().is_some_and(|c| *c == b'\n' || *c == b'\r') {
        line.pop();
    }
    Ok(line)
}

fn vm_hwm_kb() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    s.lines()
        .find(|l| l.starts_with("VmHWM:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

fn file_sha256(p: &std::path::Path) -> String {
    let b = std::fs::read(p).unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
    rustred_campaign_core::replay_base::sha256::sha256_hex(&b)
}

fn census(a: &Args) {
    use rustred_campaign_algebra::valuation::relation::PreparedRule;
    use rustred_campaign_core::poles::census::{CensusRow, summarise};
    use std::collections::BTreeMap;
    use std::sync::Mutex;
    let t0 = std::time::Instant::now();
    let kp = a
        .statement_keys
        .clone()
        .unwrap_or_else(|| die("census needs --statement-keys"));
    let cp = a
        .catalogue
        .clone()
        .unwrap_or_else(|| die("census needs --catalogue"));
    let out = a
        .out
        .clone()
        .unwrap_or_else(|| die("census needs --out DIR"));
    let d0 = D0::parse(&a.d0).unwrap_or_else(|e| die(&e.to_string()));
    let cat: Value = serde_json::from_slice(
        &std::fs::read(&cp).unwrap_or_else(|e| die(&format!("{}: {e}", cp.display()))),
    )
    .unwrap_or_else(|e| die(&format!("catalogue: {e}")));
    let mut where_: BTreeMap<String, (String, u64)> = BTreeMap::new();
    for e in cat["entries"]
        .as_array()
        .unwrap_or_else(|| die("catalogue without entries"))
    {
        let name = e["name"]
            .as_str()
            .unwrap_or_else(|| die("catalogue entry without name"));
        let table = e["table"]
            .as_str()
            .unwrap_or_else(|| die("catalogue entry without table"));
        let off = e["offset"]
            .as_u64()
            .unwrap_or_else(|| die("catalogue entry without offset"));
        where_.insert(name.to_string(), (table.to_string(), off));
    }
    // rows grouped by label: (key, class, in_box)
    let mut by_label: BTreeMap<String, Vec<(Vec<i64>, String, bool)>> = BTreeMap::new();
    let mut terminals: Vec<CensusRow> = vec![];
    let f = std::fs::File::open(&kp).unwrap_or_else(|e| die(&format!("{}: {e}", kp.display())));
    for (n, line) in BufReader::new(f).lines().enumerate() {
        let line = line.unwrap_or_else(|e| die(&e.to_string()));
        if line.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(&line)
            .unwrap_or_else(|e| die(&format!("keys line {}: {e}", n + 1)));
        let key = v[0]
            .as_str()
            .and_then(parse_key_text)
            .unwrap_or_else(|| die(&format!("keys line {}: key", n + 1)));
        let class = v[1].as_str().unwrap_or("").to_string();
        let in_box = v[3].as_bool().unwrap_or(false);
        match v[2].as_str() {
            None => terminals.push(CensusRow {
                key,
                label: None,
                class,
                in_box,
                order: Some(0),
                terms_with_pole: 0,
            }),
            Some(l) => {
                if !where_.contains_key(l) {
                    die(&format!(
                        "keys line {}: label {l} not in the catalogue",
                        n + 1
                    ));
                }
                by_label
                    .entry(l.to_string())
                    .or_default()
                    .push((key, class, in_box));
            }
        }
    }
    let mut labels: Vec<(String, usize)> =
        by_label.iter().map(|(l, v)| (l.clone(), v.len())).collect();
    labels.sort_by(|x, y| y.1.cmp(&x.1).then_with(|| x.0.cmp(&y.0)));
    let next = Mutex::new(0usize);
    let results: Mutex<Vec<(String, Value, Vec<CensusRow>)>> = Mutex::new(vec![]);
    std::thread::scope(|sc| {
        for _ in 0..a.threads {
            sc.spawn(|| {
                loop {
                    let i = {
                        let mut g = next.lock().expect("lock");
                        let i = *g;
                        *g += 1;
                        i
                    };
                    let Some((label, _)) = labels.get(i) else { break };
                    let (table, off) = &where_[label];
                    let keys = &by_label[label];
                    let line = read_line_at(table, *off);
                    let (rule_json, rows) = match line.as_ref().map_err(Clone::clone).and_then(|l| {
                        let rec: Value = serde_json::from_slice(l).map_err(|e| format!("record: {e}"))?;
                        let arity = keys[0].0.len();
                        let pr = PreparedRule::of_record(&rec, arity, &d0)?;
                        Ok((rec, pr))
                    }) {
                        Ok((rec, pr)) => {
                            let mut rows = Vec::with_capacity(keys.len());
                            for (k, class, in_box) in keys {
                                let (v, with) = pr.at_key(k);
                                rows.push(CensusRow {
                                    key: k.clone(),
                                    label: Some(label.clone()),
                                    class: class.clone(),
                                    in_box: *in_box,
                                    order: v.pole_order(),
                                    terms_with_pole: with,
                                });
                            }
                            let sha = rustred_campaign_core::replay_base::sha256::sha256_hex(line.as_ref().unwrap());
                            (json!({"label": label, "table": table, "offset": off, "record_sha256": sha,
                                "type": rec["type"], "index": rec["index"], "chart": rec["chart"], "terms": pr.terms()}), rows)
                        }
                        Err(e) => {
                            let rows = keys
                                .iter()
                                .map(|(k, class, in_box)| CensusRow {
                                    key: k.clone(),
                                    label: Some(label.clone()),
                                    class: class.clone(),
                                    in_box: *in_box,
                                    order: None,
                                    terms_with_pole: 0,
                                })
                                .collect();
                            (json!({"label": label, "table": table, "offset": off, "error": e}), rows)
                        }
                    };
                    results.lock().expect("lock").push((label.clone(), rule_json, rows));
                }
            });
        }
    });
    let mut results = results.into_inner().expect("lock");
    results.sort_by(|x, y| x.0.cmp(&y.0));
    let mut all_rows: Vec<CensusRow> = terminals;
    std::fs::create_dir_all(&out).unwrap_or_else(|e| die(&format!("{}: {e}", out.display())));
    let mut rules_f = std::io::BufWriter::new(
        std::fs::File::create(out.join("rules.jsonl.part")).unwrap_or_else(|e| die(&e.to_string())),
    );
    for (_, mut rj, rows) in results {
        let with: Vec<&CensusRow> = rows.iter().filter(|r| r.order != Some(0)).collect();
        rj["keys"] = json!(rows.len());
        rj["keys_in_box"] = json!(rows.iter().filter(|r| r.in_box).count());
        rj["keys_with_pole"] = json!(
            rows.iter()
                .filter(|r| r.order.is_some_and(|p| p > 0))
                .count()
        );
        rj["keys_with_pole_in_box"] = json!(
            rows.iter()
                .filter(|r| r.in_box && r.order.is_some_and(|p| p > 0))
                .count()
        );
        rj["undecided"] = json!(rows.iter().filter(|r| r.order.is_none()).count());
        rj["max_order"] = json!(rows.iter().filter_map(|r| r.order).max().unwrap_or(0));
        rj["d0"] = json!(d0.text());
        let _ = with;
        writeln!(rules_f, "{rj}").unwrap_or_else(|e| die(&e.to_string()));
        all_rows.extend(rows);
    }
    rules_f.flush().unwrap_or_else(|e| die(&e.to_string()));
    drop(rules_f);
    all_rows.sort_by(|x, y| x.key.cmp(&y.key));
    let mut rows_f = std::io::BufWriter::new(
        std::fs::File::create(out.join("census.rows.jsonl.part"))
            .unwrap_or_else(|e| die(&e.to_string())),
    );
    for r in all_rows.iter().filter(|r| r.order != Some(0)) {
        writeln!(
            rows_f,
            "{}",
            json!({"key": r.key, "label": r.label, "class": r.class, "in_box": r.in_box,
            "pole_order": r.order, "terms_with_pole": r.terms_with_pole})
        )
        .unwrap_or_else(|e| die(&e.to_string()));
    }
    rows_f.flush().unwrap_or_else(|e| die(&e.to_string()));
    drop(rows_f);
    let summary = summarise(&all_rows);
    let s = summary.to_json(
        d0.text(),
        json!({"statement_keys": kp.display().to_string(), "statement_keys_sha256": file_sha256(&kp),
            "catalogue": cp.display().to_string(), "catalogue_sha256": file_sha256(&cp),
            "method": rustred_campaign_algebra::valuation::VALUATION_METHOD, "threads": a.threads,
            "seconds": t0.elapsed().as_secs_f64(), "vmhwm_kb": vm_hwm_kb()}),
    );
    std::fs::write(out.join("census.summary.json.part"), format!("{s:#}\n"))
        .unwrap_or_else(|e| die(&e.to_string()));
    for f in ["rules.jsonl", "census.rows.jsonl", "census.summary.json"] {
        std::fs::rename(out.join(format!("{f}.part")), out.join(f))
            .unwrap_or_else(|e| die(&e.to_string()));
    }
    println!(
        "{}",
        json!({"verdict": "OK", "box": s["box"], "closure": s["closure"], "seconds": s["inputs"]["seconds"]})
    );
}

fn basis(a: &Args) {
    use rustred_campaign_algebra::valuation::basis::{check_master_map, read_master_map};
    use rustred_campaign_core::poles::basis::{
        TerminalSetRecord, node_basis_from_symmasters, screen_of_node_basis,
    };
    let mp = a.map.clone().unwrap_or_else(|| die("basis needs --map"));
    let out = a.out.clone().unwrap_or_else(|| die("basis needs --out"));
    let node = a.node.clone().unwrap_or_else(|| die("basis needs --node"));
    let label = a
        .label
        .clone()
        .unwrap_or_else(|| die("basis needs --label"));
    let d0 = D0::parse(&a.d0).unwrap_or_else(|e| die(&e.to_string()));
    let v: Value = serde_json::from_slice(
        &std::fs::read(&mp).unwrap_or_else(|e| die(&format!("{}: {e}", mp.display()))),
    )
    .unwrap_or_else(|e| die(&format!("map: {e}")));
    let map = read_master_map(&v).unwrap_or_else(|e| die(&e));
    let check = check_master_map(&map, &d0);
    let mut evidence = json!({"map": mp.display().to_string(), "map_sha256": file_sha256(&mp), "route": "exact (route X)"});
    let screen = match (&a.symmasters, &a.terminals) {
        (Some(sp), Some(t)) => {
            let doc: Value = serde_json::from_slice(
                &std::fs::read(sp).unwrap_or_else(|e| die(&format!("{}: {e}", sp.display()))),
            )
            .unwrap_or_else(|e| die(&format!("symmasters report: {e}")));
            let terms: Vec<String> = t
                .split(';')
                .filter(|x| !x.trim().is_empty())
                .map(|x| x.trim().to_string())
                .collect();
            let recs =
                node_basis_from_symmasters(&doc, &terms, d0.text()).unwrap_or_else(|e| die(&e));
            evidence["screen_source"] = json!(sp.display().to_string());
            evidence["screen_sha256"] = json!(file_sha256(sp));
            evidence["screen_box"] = doc["box"].clone();
            let mut s = screen_of_node_basis(&recs, d0.text());
            s["terminals"] = json!(terms);
            s
        }
        (None, None) => Value::Null,
        _ => die("--symmasters and --terminals go together"),
    };
    let rec = TerminalSetRecord {
        node,
        d0: d0.text().to_string(),
        label,
        terminals: map.rows.clone(),
        reference: map.cols.clone(),
        verdict: check.verdict.clone(),
        order: check.order,
        route_x: check.to_json(),
        screen,
        evidence,
    };
    let j = rec.to_json();
    let part = out.with_extension("part");
    std::fs::write(&part, format!("{j:#}\n")).unwrap_or_else(|e| die(&e.to_string()));
    std::fs::rename(&part, &out).unwrap_or_else(|e| die(&e.to_string()));
    println!(
        "{}",
        json!({"verdict": "OK", "terminal_set_verdict": j["verdict"], "order": j["order"],
        "det_order_at_d0": j["route_x"]["det_order_at_d0"], "rank_at_d0": j["route_x"]["rank_at_d0"], "out": out.display().to_string()})
    );
}
