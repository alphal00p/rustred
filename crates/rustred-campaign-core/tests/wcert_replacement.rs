//! Lane wcertproof (implementer item 4, "certify by replacement"): the replacement pieces of one rule record PARTITION
//! its domain, EXACTLY on the whole chart orthant, decided by the library's first-match liveness
//! (`parametric::prune::record_liveness`, region `Cone::orthant()`):
//!   (1) cover:     in the dispatcher [P_1 .. P_k, O] the original O is DEAD  <=>  D(O) is contained in the union of the D(P_i);
//!   (2) contained: in the dispatcher [O, P_1 .. P_k] every P_i is DEAD      <=>  D(P_i) is contained in D(O) + earlier pieces,
//!                  hence (induction on i) every D(P_i) is contained in D(O);
//!   (3) disjoint:  for every pair i < j the CONJUNCTION record Z_ij (fixed slots merged, case equations, exception branches
//!                  and bounds of both, same chart) alone is DEAD  <=>  D(P_i) and D(P_j) do not meet.
//! With (1) and (2) the pieces placed contiguously AT O's position leave the first match of every key outside D(O)
//! unchanged and give every key of D(O) to a piece: no other record's first-match set changes. (3) makes it a partition.
//! Run with the witness check of the domain options both off (pure case domains) and on (the importer's default).
//!
//! Env (ignored test; the inputs are TEST DATA, never constants of the code):
//!   WCERT_REPL_ORIG / WCERT_REPL_ORIG_INDEX        table (or rhs-stripped domain copy) with the replaced record, its index
//!   WCERT_REPL_PIECES / WCERT_REPL_PIECE_INDICES   table with the pieces, their indices "a,b,c" (dispatch order)
//!   WCERT_REPL_OUT                                 JSON summary; temporary tables go to TMPDIR
use std::io::BufRead;

use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::DomainOptions;
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::parametric::cone::{Cone, ConeOptions};
use rustred_campaign_core::parametric::prune::{Liveness, record_liveness};
use rustred_campaign_core::record::{ReadOptions, RhsMode, read_table_file};
use serde_json::{Value, json};

struct Src {
    header: String,
    charts: Vec<String>,
    records: Vec<Value>,
}

/// Header, chart lines and the rule records with the wanted indices (in the order asked).
fn read_src(path: &str, wanted: &[i64]) -> Src {
    let f = std::io::BufReader::new(
        std::fs::File::open(path).unwrap_or_else(|e| panic!("{path}: {e}")),
    );
    let (mut header, mut charts) = (String::new(), vec![]);
    let mut found: Vec<Option<Value>> = vec![None; wanted.len()];
    for line in f.lines() {
        let line = line.unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        match v["type"].as_str() {
            Some("header") => header = line,
            Some("chart") => charts.push(line),
            Some("rule") => {
                if let Some(i) = v["index"]
                    .as_i64()
                    .and_then(|i| wanted.iter().position(|&w| w == i))
                {
                    found[i] = Some(v);
                }
            }
            _ => {}
        }
    }
    let records = found
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            r.unwrap_or_else(|| panic!("{path}: no rule record with index {}", wanted[i]))
        })
        .collect();
    Src {
        header,
        charts,
        records,
    }
}

/// Write a small table (header, charts, the records re-indexed 0..n-1 in the given order, end).
fn write_table(path: &std::path::Path, src: &Src, recs: &[Value]) {
    let mut s = String::new();
    s.push_str(&src.header);
    s.push('\n');
    for c in &src.charts {
        s.push_str(c);
        s.push('\n');
    }
    for (i, r) in recs.iter().enumerate() {
        let mut r = r.clone();
        r["index"] = json!(i);
        r["rhs"] = json!([]);
        s.push_str(&r.to_string());
        s.push('\n');
    }
    s.push_str(
        &json!({"type": "end", "charts": src.charts.len(), "rules": recs.len(), "residuals": 0})
            .to_string(),
    );
    s.push('\n');
    std::fs::write(path, s).unwrap();
}

/// Value of a target slot text at a fixed key value: "n<s>" -> v, "n<s>+k" -> v + k, a number stays.
fn target_slot_at(t: &str, v: i64) -> String {
    let t = t.trim();
    match t.strip_prefix('n') {
        None => t.to_string(),
        Some(r) => {
            let p = r.find(['+', '-']).unwrap_or(r.len());
            let off: i64 = if p < r.len() {
                r[p..].trim_start_matches('+').parse().unwrap()
            } else {
                0
            };
            (v + off).to_string()
        }
    }
}

/// The conjunction record of X and Y (their domains' intersection), or None when two fixed slots disagree (empty).
fn conjunction(x: &Value, y: &Value) -> Option<Value> {
    assert_eq!(x["chart"], y["chart"], "pieces of different charts");
    let fx = x["fixed"].as_array().unwrap();
    let fy = y["fixed"].as_array().unwrap();
    let n = fx.len();
    let mut fixed = vec![Value::Null; n];
    for s in 0..n {
        fixed[s] = match (fx[s].as_i64(), fy[s].as_i64()) {
            (Some(a), Some(b)) if a != b => return None,
            (Some(a), _) | (None, Some(a)) => json!(a),
            (None, None) => Value::Null,
        };
    }
    let inner = x["target"].as_str().unwrap();
    let slots: Vec<&str> = inner[2..inner.len() - 1].split(',').collect();
    let target: Vec<String> = (0..n)
        .map(|s| match fixed[s].as_i64() {
            Some(v) if x["fixed"][s].is_null() => target_slot_at(slots[s], v),
            _ => slots[s].to_string(),
        })
        .collect();
    let cat = |k: &str| -> Value {
        let mut v: Vec<Value> = x[k].as_array().cloned().unwrap_or_default();
        for e in y[k].as_array().cloned().unwrap_or_default() {
            if !v.contains(&e) {
                v.push(e);
            }
        }
        Value::Array(v)
    };
    let side = |r: &Value, k: &str, s: usize| r["domain_bounds"][k].get(s).and_then(Value::as_i64);
    let has_b = !x["domain_bounds"].is_null() || !y["domain_bounds"].is_null();
    let mut z = x.clone();
    z["fixed"] = Value::Array(fixed);
    z["target"] = json!(format!("I({})", target.join(",")));
    z["equations"] = cat("equations");
    z["exceptions"] = cat("exceptions");
    z["witness_loci"] = cat("witness_loci");
    if has_b {
        let lo: Vec<Value> = (0..n)
            .map(|s| match (side(x, "min", s), side(y, "min", s)) {
                (Some(a), Some(b)) => json!(a.max(b)),
                (Some(a), None) | (None, Some(a)) => json!(a),
                _ => Value::Null,
            })
            .collect();
        let hi: Vec<Value> = (0..n)
            .map(|s| match (side(x, "max", s), side(y, "max", s)) {
                (Some(a), Some(b)) => json!(a.min(b)),
                (Some(a), None) | (None, Some(a)) => json!(a),
                _ => Value::Null,
            })
            .collect();
        z["domain_bounds"] = json!({"min": lo, "max": hi});
    }
    z["case"] = json!(format!("conjunction of {} and {}", x["index"], y["index"]));
    Some(z)
}

fn liveness(
    path: &std::path::Path,
    pos: usize,
    witness: bool,
    enum_cap: u64,
) -> (String, Option<Vec<i32>>, String, f64) {
    let t0 = std::time::Instant::now();
    let tf = read_table_file(path, ReadOptions { rhs: RhsMode::Skip }).unwrap();
    let frame = NodeFrame::from_header(tf.header.as_ref().unwrap()).unwrap();
    let opts = DomainOptions {
        witness,
        ..DomainOptions::default()
    };
    let t = Table::from_table_file("t", path.display().to_string(), &tf, opts).unwrap();
    let d = Dispatcher::new(vec![t], vec![0]).unwrap();
    let (v, k, why) = match record_liveness(
        &d,
        &frame,
        &Cone::orthant(),
        pos,
        &ConeOptions::default(),
        enum_cap,
    ) {
        Liveness::Dead => ("dead".to_string(), None, String::new()),
        Liveness::Live(k) => ("live".to_string(), Some(k), String::new()),
        Liveness::Undecided(y) => ("undecided".to_string(), None, y),
    };
    (v, k, why, t0.elapsed().as_secs_f64())
}

#[test]
#[ignore]
fn replacement_pieces_partition_the_replaced_domain() {
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("{k}"));
    let orig_index: i64 = env("WCERT_REPL_ORIG_INDEX").parse().unwrap();
    let piece_idx: Vec<i64> = env("WCERT_REPL_PIECE_INDICES")
        .split(',')
        .map(|x| x.trim().parse().unwrap())
        .collect();
    let orig = read_src(&env("WCERT_REPL_ORIG"), &[orig_index]);
    let pieces = read_src(&env("WCERT_REPL_PIECES"), &piece_idx);
    assert_eq!(
        serde_json::from_str::<Value>(&orig.header).unwrap()["mask"],
        serde_json::from_str::<Value>(&pieces.header).unwrap()["mask"],
        "the two tables have different frames"
    );
    let tmp = std::path::PathBuf::from(std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into()));
    let o = orig.records[0].clone();
    let k = pieces.records.len();
    let enum_cap = 1_000_000;
    let mut out = vec![];
    let mut all_dead = true;
    let mut controls_live = true;
    for witness in [false, true] {
        // CONTROLS (the dead verdicts below are not vacuous): the original alone, every piece alone, and the original after
        // all pieces but the last are LIVE
        let pc = tmp.join(format!("wcert-repl-control-{witness}.table.jsonl"));
        let mut recs = vec![o.clone()];
        recs.extend(pieces.records.iter().cloned());
        for (i, r) in recs.iter().enumerate() {
            write_table(&pc, &pieces, std::slice::from_ref(r));
            let (v, w, why, s) = liveness(&pc, 0, witness, enum_cap);
            controls_live &= v == "live";
            out.push(json!({"check": format!("control: record {} alone (expected live)", if i == 0 { "original".to_string() } else { format!("piece {}", i - 1) }), "witness": witness, "verdict": v, "witness_key": w, "why": why, "seconds": s}));
        }
        if k >= 1 {
            let mut recs: Vec<Value> = pieces.records[..k - 1].to_vec();
            recs.push(o.clone());
            write_table(&pc, &pieces, &recs);
            let (v, w, why, s) = liveness(&pc, k - 1, witness, enum_cap);
            controls_live &= v == "live";
            out.push(json!({"check": "control: original after all pieces but the last (expected live)", "witness": witness, "verdict": v, "witness_key": w, "why": why, "seconds": s}));
        }
        // (1) cover
        let p1 = tmp.join(format!("wcert-repl-cover-{witness}.table.jsonl"));
        let mut recs = pieces.records.clone();
        recs.push(o.clone());
        write_table(&p1, &pieces, &recs);
        let (v, w, why, s) = liveness(&p1, k, witness, enum_cap);
        all_dead &= v == "dead";
        out.push(json!({"check": "cover: original after the pieces", "witness": witness, "verdict": v, "witness_key": w, "why": why, "seconds": s}));
        // (2) contained
        let p2 = tmp.join(format!("wcert-repl-contained-{witness}.table.jsonl"));
        let mut recs = vec![o.clone()];
        recs.extend(pieces.records.iter().cloned());
        write_table(&p2, &pieces, &recs);
        for (i, idx) in piece_idx.iter().enumerate().take(k) {
            let (v, w, why, s) = liveness(&p2, 1 + i, witness, enum_cap);
            all_dead &= v == "dead";
            out.push(json!({"check": format!("contained: piece {} (index {}) after the original", i, idx), "witness": witness, "verdict": v, "witness_key": w, "why": why, "seconds": s}));
        }
        // (3) disjoint
        for i in 0..k {
            for j in i + 1..k {
                match conjunction(&pieces.records[i], &pieces.records[j]) {
                    None => out.push(json!({"check": format!("disjoint: pieces {i} and {j}"), "witness": witness, "verdict": "dead", "why": "fixed slots disagree"})),
                    Some(z) => {
                        let p3 = tmp.join(format!("wcert-repl-meet-{i}-{j}-{witness}.table.jsonl"));
                        write_table(&p3, &pieces, &[z]);
                        let (v, w, why, s) = liveness(&p3, 0, witness, enum_cap);
                        all_dead &= v == "dead";
                        out.push(json!({"check": format!("disjoint: pieces {i} and {j} (conjunction record alone)"), "witness": witness, "verdict": v, "witness_key": w, "why": why, "seconds": s}));
                    }
                }
            }
        }
    }
    let summary = json!({"schema": "wcertproof.replacement-partition.v1", "original": {"table": env("WCERT_REPL_ORIG"), "index": orig_index},
        "pieces": {"table": env("WCERT_REPL_PIECES"), "indices": piece_idx}, "region": "chart orthant (Cone::orthant), exact",
        "partition": all_dead && controls_live, "controls_live": controls_live, "checks": out});
    let text = serde_json::to_string_pretty(&summary).unwrap();
    eprintln!("{text}");
    if let Ok(p) = std::env::var("WCERT_REPL_OUT") {
        std::fs::write(p, &text).unwrap();
    }
    assert!(
        controls_live,
        "a control is not live: the dead verdicts would be vacuous (see the summary)"
    );
    assert!(
        all_dead,
        "the pieces do not partition the original's domain (see the summary)"
    );
}

/// The conjunction of two records merges fixed slots, branches and bounds; disagreeing fixed slots give None.
#[test]
fn conjunction_record_merges_both_domains() {
    let x = json!({"type": "rule", "chart": "10", "index": 3, "fixed": [null, 0], "equations": [], "target": "I(n0,0)",
        "exceptions": [["-1+n0"]], "domain_bounds": {"min": [2, null], "max": [null, null]}});
    let y = json!({"type": "rule", "chart": "10", "index": 4, "fixed": [5, null], "equations": [], "target": "I(5,n1)",
        "exceptions": [["n1"]], "domain_bounds": {"min": [1, null], "max": [null, -1]}});
    let z = conjunction(&x, &y).unwrap();
    assert_eq!(z["fixed"], json!([5, 0]));
    assert_eq!(z["target"], json!("I(5,0)"));
    assert_eq!(z["exceptions"], json!([["-1+n0"], ["n1"]]));
    assert_eq!(
        z["domain_bounds"],
        json!({"min": [2, null], "max": [null, -1]})
    );
    let w = json!({"type": "rule", "chart": "10", "index": 5, "fixed": [6, null], "target": "I(6,n1)", "exceptions": []});
    assert!(conjunction(&y, &w).is_none());
    assert_eq!(target_slot_at("n7+1", 3), "4");
    assert_eq!(target_slot_at("n7-2", 3), "1");
}
