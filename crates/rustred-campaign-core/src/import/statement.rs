//! OUR statement (canonical JSON lines, summary, terminals, SHA256SUMS) and the comparison against a hand statement
//! read through a column map ([`HandFormat`], DATA). Differences are typed and EVERY key is listed.

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, Write};
use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};

use super::check::{CheckResult, KeyRow, RowStatus, Server};
use super::error::{ImportError, io_err};
use super::evidence::Evidence;
use super::lines::{key_text, parse_key_text};
use super::load::ImportedSet;
use super::spec::{Col, HandFormat};
use crate::closure::sha256::Sha256;
use crate::schema::canon::canonical_json;

/// One row of a hand statement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandRow {
    pub key: Vec<i32>,
    /// None: the statement does not state it.
    pub in_box: Option<bool>,
    pub label: String,
    pub rule: Option<String>,
    pub round: Option<i64>,
}

/// A hand statement.
#[derive(Clone, Debug, Default)]
pub struct HandStatement {
    pub rows: Vec<HandRow>,
    pub terminal_label: String,
    pub path: String,
    pub sha256: String,
    /// The statement lists only some closure keys (`HandFormat::partial`).
    pub partial: bool,
}

impl HandStatement {
    pub fn terminals(&self) -> Vec<Vec<i32>> {
        let mut v: Vec<Vec<i32>> = self
            .rows
            .iter()
            .filter(|r| r.label == self.terminal_label)
            .map(|r| r.key.clone())
            .collect();
        v.sort();
        v
    }
    pub fn rounds(&self) -> Option<i64> {
        self.rows
            .iter()
            .filter_map(|r| r.round)
            .max()
            .map(|m| m + 1)
    }
}

/// Read a hand statement (JSONL of arrays) through its column map.
pub fn read_hand_statement(
    path: &Path,
    fmt: &HandFormat,
    arity: usize,
) -> Result<HandStatement, ImportError> {
    let f = std::fs::File::open(path).map_err(|e| io_err(path, e))?;
    let mut h = Sha256::new();
    let mut rows = vec![];
    for (i, l) in std::io::BufReader::with_capacity(1 << 20, f)
        .lines()
        .enumerate()
    {
        let l = l.map_err(|e| io_err(path, e))?;
        h.update(l.as_bytes());
        h.update(b"\n");
        if l.trim().is_empty() {
            continue;
        }
        let bad = |m: String| ImportError::Evidence {
            path: path.display().to_string(),
            line: i + 1,
            msg: m,
        };
        let v: Value = serde_json::from_str(&l).map_err(|e| bad(e.to_string()))?;
        let col = |c: &Col| -> Result<&Value, ImportError> {
            match (c, &v) {
                (Col::Index(i), Value::Array(a)) => a.get(*i),
                (Col::Field(f), Value::Object(o)) => o.get(f),
                _ => None,
            }
            .ok_or_else(|| bad(format!("no column {c:?}")))
        };
        let key = match col(&fmt.key)? {
            Value::String(s) => parse_key_text(s, arity),
            k @ Value::Array(_) => serde_json::from_value::<Vec<i32>>(k.clone())
                .ok()
                .filter(|k| k.len() == arity),
            _ => None,
        }
        .ok_or_else(|| bad("key column".into()))?;
        let in_box = match &fmt.in_box {
            None => None,
            Some(c) => Some(
                col(c)?
                    .as_bool()
                    .ok_or_else(|| bad("in_box column".into()))?,
            ),
        };
        let label = col(&fmt.label)?
            .as_str()
            .ok_or_else(|| bad("label column".into()))?
            .to_string();
        let rule = match (&fmt.rule, &v) {
            // an object line may omit the rule field (e.g. a terminal row)
            (Col::Field(f), Value::Object(o)) if !o.contains_key(f) => &Value::Null,
            (c, _) => col(c)?,
        };
        let rule = match rule {
            Value::Null => None,
            Value::String(s) => Some(s.clone()),
            o => Some(o.to_string()),
        };
        let round = match &fmt.round {
            None => None,
            Some(c) => {
                let v = col(c)?;
                match (fmt.round_index, v) {
                    (Some(j), Value::Array(x)) => x.get(j).and_then(Value::as_i64),
                    (None, x) => x.as_i64(),
                    _ => None,
                }
            }
        };
        rows.push(HandRow {
            key,
            in_box,
            label,
            rule,
            round,
        });
    }
    Ok(HandStatement {
        rows,
        terminal_label: fmt.terminal_label.clone(),
        path: path.display().to_string(),
        sha256: h.finish_hex(),
        partial: fmt.partial,
    })
}

/// The hand name of our server of a row.
pub fn server_hand_name(set: &ImportedSet, row: &KeyRow) -> Option<String> {
    match row.server.as_ref()? {
        Server::Rule { table, record } => {
            let i = &set.rules[*table][*record];
            Some(
                i.hand_name
                    .clone()
                    .unwrap_or_else(|| format!("{}#{}", i.source, i.label)),
            )
        }
        Server::Credit { table, record } => {
            let i = &set.rules[*table][*record];
            Some(format!(
                "credit:{}",
                i.hand_name
                    .clone()
                    .unwrap_or_else(|| format!("{}#{}", i.source, i.label))
            ))
        }
        Server::Relation { set: name, recipe } => {
            if *recipe {
                Some(name.clone())
            } else {
                set.supplied_info
                    .get(name)
                    .and_then(|m| m.get(&row.key))
                    .map(|s| {
                        s.hand_name
                            .clone()
                            .unwrap_or_else(|| format!("{}#{}", s.source, s.ordinal))
                    })
            }
        }
    }
}

fn server_json(set: &ImportedSet, row: &KeyRow) -> Value {
    let hn = server_hand_name(set, row);
    match &row.server {
        None => Value::Null,
        Some(Server::Rule { table, record }) | Some(Server::Credit { table, record }) => {
            let i = &set.rules[*table][*record];
            let mut v = json!({"kind": if matches!(row.server, Some(Server::Credit{..})) {"credit"} else {"rule"},
                   "source": i.source, "class": i.class, "label": i.label, "ordinal": i.ordinal,
                   "record_sha256": i.record_sha256, "recipe_sha256": null, "hand_name": hn});
            // lane import5 (18.4): a sym-served row says so (other rows unchanged)
            if i.sym.is_some() && matches!(row.server, Some(Server::Rule { .. })) {
                v["sym"] = Value::Bool(true);
                v["image"] = json!(row.image);
            }
            v
        }
        Some(Server::Relation { set: name, recipe }) => {
            if *recipe {
                let rc = set.recipes.iter().find(|r| &r.id == name);
                json!({"kind":"relation","source": name, "class": rc.map(|r| r.class.clone()),
                       "label": null, "ordinal": null, "record_sha256": null,
                       "recipe_sha256": rc.map(|r| r.recipe_sha256.clone()), "hand_name": hn})
            } else {
                let s = set.supplied_info.get(name).and_then(|m| m.get(&row.key));
                // a credit through a relation-served image: the set's class (the entry is the image's)
                let class = s
                    .map(|s| s.class.clone())
                    .or_else(|| set.classes.get(name).cloned());
                json!({"kind":"relation","source": name, "class": class,
                       "label": s.and_then(|s| s.label), "ordinal": s.map(|s| s.ordinal),
                       "record_sha256": s.map(|s| s.record_sha256.clone()), "recipe_sha256": null,
                       "hand_name": hn})
            }
        }
    }
}

fn rule_json(set: &ImportedSet, tr: Option<(usize, usize)>) -> Value {
    match tr {
        None => Value::Null,
        Some((t, r)) => {
            let i = &set.rules[t][r];
            json!({"source": i.source, "label": i.label, "record_sha256": i.record_sha256,
                   "hand_name": i.hand_name})
        }
    }
}

/// The `facts` object of a statement row (lane closeloop3: `basis` = who produced the key's fact line, only when one
/// is bound: `rr_proof` | `exact_eval` | `compact`).
fn facts_json(row: &KeyRow) -> Value {
    let mut f = json!({"regularity_unknown": row.regularity_unknown, "coefficient_unknown": row.coefficient_unknown});
    if let Some(b) = row.fact_basis {
        f["basis"] = Value::from(b);
    }
    f
}

/// The JSON of one statement row.
pub fn row_json(set: &ImportedSet, row: &KeyRow) -> Value {
    let status = match row.status {
        RowStatus::Served => "served",
        RowStatus::Terminal => "terminal",
        RowStatus::Open => "open",
    };
    let open = row.open_status.as_ref().map(
        |s| json!({"status": s, "reason": row.open_reason, "rule": rule_json(set, row.open_rule)}),
    );
    let mut v = json!({
        "key": row.key,
        "in_start": row.in_start,
        "status": status,
        "server": if row.status == RowStatus::Served { server_json(set, row) } else { Value::Null },
        "evidence": if row.status == RowStatus::Served { row.evidence.map(|e| e.word()) } else { None },
        "basis": row.basis,
        "facts": facts_json(row),
        "open": open,
    });
    // lane import5 (18.2): a terminal DECLARED BY A TABLE carries its provenance (rows of terminals declared by a
    // `terminals` source are unchanged)
    if row.status == RowStatus::Terminal
        && let Some(d) = set.terminal_provenance.get(&row.key)
    {
        v["terminal"] = json!({"provenance": d.provenance, "source": d.source, "path": d.path,
                                "line": d.line});
    }
    v
}

/// Digests of the written statement files.
#[derive(Clone, Debug, Serialize)]
pub struct StatementDigests {
    pub files: BTreeMap<String, String>,
}

fn write_canon(
    dir: &Path,
    name: &str,
    v: &Value,
    sums: &mut BTreeMap<String, String>,
) -> Result<(), ImportError> {
    let t = canonical_json(v, false).map_err(ImportError::Core)?;
    let p = dir.join(name);
    std::fs::write(&p, format!("{t}\n")).map_err(|e| io_err(&p, e))?;
    let mut h = Sha256::new();
    h.update(t.as_bytes());
    h.update(b"\n");
    sums.insert(name.to_string(), h.finish_hex());
    Ok(())
}

/// Write OUR statement into `dir` (created; existing files of the same names are replaced).
pub fn write_statement(
    set: &ImportedSet,
    ev: &Evidence,
    res: &CheckResult,
    dir: &Path,
    timing: Option<Value>,
) -> Result<StatementDigests, ImportError> {
    std::fs::create_dir_all(dir).map_err(|e| io_err(dir, e))?;
    let mut sums = BTreeMap::new();
    // statement.jsonl
    let p = dir.join("statement.jsonl");
    let f = std::fs::File::create(&p).map_err(|e| io_err(&p, e))?;
    let mut w = std::io::BufWriter::with_capacity(1 << 20, f);
    let mut h = Sha256::new();
    match &res.streamed {
        // check_streamed wrote every row already (closure order): record that file
        Some(st) => {
            drop(w);
            let _ = std::fs::remove_file(&p);
            if Path::new(&st.path) != p.as_path() {
                std::fs::rename(&st.path, &p)
                    .or_else(|_| std::fs::copy(&st.path, &p).map(|_| ()))
                    .map_err(|e| io_err(&p, e))?;
            }
            sums.insert("statement.jsonl".to_string(), st.sha256.clone());
        }
        None => {
            for row in &res.rows {
                let t = canonical_json(&row_json(set, row), false).map_err(ImportError::Core)?;
                w.write_all(t.as_bytes()).map_err(|e| io_err(&p, e))?;
                w.write_all(b"\n").map_err(|e| io_err(&p, e))?;
                h.update(t.as_bytes());
                h.update(b"\n");
            }
            w.flush().map_err(|e| io_err(&p, e))?;
            sums.insert("statement.jsonl".to_string(), h.finish_hex());
        }
    }
    // summary
    let counts = res.counts();
    let summary = json!({
        "schema": super::STATEMENT_SCHEMA,
        "spec_digest": res.spec_digest,
        "node": set.spec.node,
        "start": {"keys": set.start.len(), "digest": set.start.digest()},
        "key_set_digest": res.key_set_digest,
        "keys": res.summary.keys_total,
        "in_start": res.summary.keys_in_start,
        "rows_streamed": res.streamed,
        "counts": counts,
        "evidence_by_class": res.evidence_by_class(set),
        "required_evidence": res.required.word(),
        "tier": format!("{:?}", res.tier),
        "closure": serde_json::to_value(&res.summary).unwrap_or(Value::Null),
        "blocking": res.blocking(),
        "requests": {"solve": res.solve_requests.len(), "facts": res.fact_requests.len(),
                     "proofs": res.proof_requests.len(),
                     "fact_keys": res.fact_requests.iter().map(|r| r.keys.len()).sum::<usize>(),
                     "proof_keys": res.proof_requests.iter().map(|r| r.keys.len()).sum::<usize>()},
        "solve_refusals": res.solve_refusals,
        "evidence_import": serde_json::to_value(&ev.counts).unwrap_or(Value::Null),
        "load": serde_json::to_value(set.report()).unwrap_or(Value::Null),
    });
    write_canon(dir, "summary.json", &summary, &mut sums)?;
    if let Some(t) = timing {
        // not identifying: outside SHA256SUMS on purpose
        let p = dir.join("timing.json");
        std::fs::write(
            &p,
            format!("{}\n", canonical_json(&t, true).map_err(ImportError::Core)?),
        )
        .map_err(|e| io_err(&p, e))?;
    }
    let terms: Vec<Value> = res
        .rows
        .iter()
        .filter(|r| r.status == RowStatus::Terminal)
        .map(|r| json!(key_text(&r.key)))
        .collect();
    let open: Vec<Value> = res
        .rows
        .iter()
        .filter(|r| r.status == RowStatus::Open)
        .map(|r| json!({"key": key_text(&r.key), "status": r.open_status, "reason": r.open_reason}))
        .collect();
    let mut tj = json!({"terminals": terms, "open": open});
    if !set.terminal_provenance.is_empty() {
        // lane import5 (18.2): the terminals declared by tables (only when there are some: earlier digests unchanged)
        tj["declared_by_tables"] = set
            .terminal_provenance
            .iter()
            .map(|(k, d)| {
                json!({"key": key_text(k), "provenance": d.provenance, "source": d.source,
                       "path": d.path, "line": d.line})
            })
            .collect::<Vec<_>>()
            .into();
    }
    write_canon(dir, "terminals.json", &tj, &mut sums)?;
    // requests (JSONL, canonical)
    let jsonl = |name: &str,
                 vals: Vec<Value>,
                 sums: &mut BTreeMap<String, String>|
     -> Result<(), ImportError> {
        let p = dir.join(name);
        let mut out = String::new();
        for v in vals {
            out.push_str(&canonical_json(&v, false).map_err(ImportError::Core)?);
            out.push('\n');
        }
        std::fs::write(&p, &out).map_err(|e| io_err(&p, e))?;
        let mut h = Sha256::new();
        h.update(out.as_bytes());
        sums.insert(name.to_string(), h.finish_hex());
        Ok(())
    };
    jsonl(
        "solve-requests.jsonl",
        res.solve_requests
            .iter()
            .map(|r| serde_json::to_value(r).unwrap_or(Value::Null))
            .collect(),
        &mut sums,
    )?;
    jsonl(
        "fact-requests.jsonl",
        res.fact_requests
            .iter()
            .map(|r| serde_json::to_value(r).unwrap_or(Value::Null))
            .collect(),
        &mut sums,
    )?;
    jsonl(
        "proof-requests.jsonl",
        res.proof_requests
            .iter()
            .map(|r| serde_json::to_value(r).unwrap_or(Value::Null))
            .collect(),
        &mut sums,
    )?;
    let p = dir.join("SHA256SUMS");
    let mut s = String::new();
    for (n, d) in &sums {
        s.push_str(&format!("{d}  {n}\n"));
    }
    std::fs::write(&p, s).map_err(|e| io_err(&p, e))?;
    Ok(StatementDigests { files: sums })
}

/// Kinds of differences (interfaces/import.md 7.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    OnlyHand,
    OnlyOurs,
    InBoxDiffers,
    ServerDiffers,
    HandServedOursOpen,
    HandServedOursTerminal,
    HandTerminalOursServed,
    HandTerminalOursOpen,
}

/// One difference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Difference {
    pub kind: DiffKind,
    pub key: Vec<i32>,
    pub hand_label: Option<String>,
    pub hand_rule: Option<String>,
    pub ours: Option<String>,
    pub detail: Option<String>,
}

/// The comparison.
#[derive(Clone, Debug, Serialize)]
pub struct Comparison {
    pub hand_path: String,
    pub hand_sha256: String,
    pub keys_hand: usize,
    pub keys_ours: usize,
    pub key_sets_equal: bool,
    pub same_server: usize,
    pub terminals_hand: Vec<String>,
    pub terminals_ours: Vec<String>,
    pub terminals_equal: bool,
    pub rounds_hand: Option<i64>,
    pub rounds_ours: u32,
    pub counts: BTreeMap<String, usize>,
    /// (hand label, our class / status) -> keys.
    pub label_matrix: BTreeMap<String, usize>,
    /// Hand rule names our set produces nowhere (per name, keys).
    pub unmapped_hand_rules: BTreeMap<String, usize>,
    pub differences: Vec<Difference>,
    /// lane closeloop4 (audit-closeloop M2): every row of ours was compared (false: `incomplete_why`; never equal).
    pub complete: bool,
    pub incomplete_why: Option<String>,
}

impl Comparison {
    pub fn count(&self, k: DiffKind) -> usize {
        self.differences.iter().filter(|d| d.kind == k).count()
    }
    /// Write `comparison.json` (without the list) and `differences.jsonl` (every difference, canonical).
    pub fn write(&self, dir: &Path) -> Result<(), ImportError> {
        std::fs::create_dir_all(dir).map_err(|e| io_err(dir, e))?;
        let mut v = serde_json::to_value(self).map_err(|e| ImportError::Core(e.to_string()))?;
        if let Some(o) = v.as_object_mut() {
            o.remove("differences");
            o.insert("differences_listed".into(), json!(self.differences.len()));
        }
        let p = dir.join("comparison.json");
        std::fs::write(
            &p,
            format!(
                "{}\n",
                canonical_json(&v, false).map_err(ImportError::Core)?
            ),
        )
        .map_err(|e| io_err(&p, e))?;
        let p = dir.join("differences.jsonl");
        let mut out = String::new();
        for d in &self.differences {
            let v = serde_json::to_value(d).map_err(|e| ImportError::Core(e.to_string()))?;
            out.push_str(&canonical_json(&v, false).map_err(ImportError::Core)?);
            out.push('\n');
        }
        std::fs::write(&p, out).map_err(|e| io_err(&p, e))
    }
}

fn ours_word(set: &ImportedSet, row: &KeyRow) -> String {
    match row.status {
        RowStatus::Terminal => "terminal".into(),
        RowStatus::Open => format!(
            "open:{}:{}",
            row.open_status.clone().unwrap_or_default(),
            row.open_reason.clone().unwrap_or_default()
        ),
        RowStatus::Served => server_hand_name(set, row).unwrap_or_default(),
    }
}

fn ours_class(set: &ImportedSet, row: &KeyRow) -> String {
    match (&row.status, &row.server) {
        (RowStatus::Terminal, _) => "terminal".into(),
        (RowStatus::Open, _) => format!("open:{}", row.open_status.clone().unwrap_or_default()),
        (_, Some(Server::Rule { table, record })) | (_, Some(Server::Credit { table, record })) => {
            set.rules[*table][*record].class.clone()
        }
        (_, Some(Server::Relation { set: name, .. })) => set
            .classes
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.clone()),
        _ => "?".into(),
    }
}

/// Compare OUR check result with a hand statement.
/// lane closeloop4 (audit-closeloop M2): every row of a check: the kept rows, plus (streamed check) every PLAIN rule row
/// read back from the rows file (a plain row is served by a table rule; credits / relations / open / terminal rows are
/// kept in memory). Err = the file cannot be read in full (the comparison is then incomplete, never "equal").
pub fn all_rows(set: &ImportedSet, res: &CheckResult) -> Result<Vec<KeyRow>, String> {
    let Some(st) = &res.streamed else {
        return Ok(res.rows.clone());
    };
    let kept: std::collections::HashSet<&[i32]> =
        res.rows.iter().map(|r| r.key.as_slice()).collect();
    let mut out = res.rows.clone();
    let f = std::fs::File::open(&st.path).map_err(|e| format!("{}: {e}", st.path))?;
    let mut n = 0u64;
    for l in std::io::BufRead::lines(std::io::BufReader::with_capacity(1 << 20, f)) {
        let l = l.map_err(|e| format!("{}: {e}", st.path))?;
        if l.trim().is_empty() {
            continue;
        }
        n += 1;
        let v: Value = serde_json::from_str(&l).map_err(|e| format!("{}: {e}", st.path))?;
        let key: Vec<i32> = serde_json::from_value(v["key"].clone())
            .map_err(|e| format!("{}: key {e}", st.path))?;
        if kept.contains(key.as_slice()) {
            continue;
        }
        let server = match (v["status"].as_str(), v["server"]["kind"].as_str()) {
            (Some("served"), Some("rule")) => v["server"]["record_sha256"]
                .as_str()
                .and_then(|h| set.find_sha(h))
                .map(|(table, record)| Server::Rule { table, record }),
            _ => None,
        };
        let Some(server) = server else {
            return Err(format!(
                "{}: row {key:?} is neither kept nor a plain rule row",
                st.path
            ));
        };
        out.push(KeyRow {
            key,
            in_start: v["in_start"] == true,
            status: RowStatus::Served,
            server: Some(server),
            evidence: serde_json::from_value(v["evidence"].clone()).ok(),
            basis: None,
            regularity_unknown: false,
            coefficient_unknown: false,
            fact_basis: None,
            open_status: None,
            open_reason: None,
            open_rule: None,
            image: None,
        });
    }
    if n != st.rows {
        return Err(format!(
            "{}: {n} rows read, the check wrote {}",
            st.path, st.rows
        ));
    }
    Ok(out)
}

pub fn compare(set: &ImportedSet, res: &CheckResult, hand: &HandStatement) -> Comparison {
    // lane closeloop4 (audit-closeloop M2): a streamed check is compared over EVERY row (read back from its rows
    // file); a comparison that cannot read them all says so (`complete: false`) and is never "equal"
    let (rows_all, incomplete): (Vec<KeyRow>, Option<String>) = match all_rows(set, res) {
        Ok(v) => (v, None),
        Err(e) => (res.rows.clone(), Some(e)),
    };
    let ours: HashMap<&[i32], &KeyRow> = rows_all.iter().map(|r| (r.key.as_slice(), r)).collect();
    let mut produced: std::collections::HashSet<String> = std::collections::HashSet::new();
    for t in &set.rules {
        for i in t {
            produced.insert(
                i.hand_name
                    .clone()
                    .unwrap_or_else(|| format!("{}#{}", i.source, i.label)),
            );
        }
    }
    for m in set.supplied_info.values() {
        for s in m.values() {
            produced.insert(
                s.hand_name
                    .clone()
                    .unwrap_or_else(|| format!("{}#{}", s.source, s.ordinal)),
            );
        }
    }
    for rc in &set.recipes {
        produced.insert(rc.id.clone());
    }
    let mut diffs = vec![];
    let mut same = 0usize;
    let mut matrix: BTreeMap<String, usize> = BTreeMap::new();
    let mut unmapped: BTreeMap<String, usize> = BTreeMap::new();
    let mut hand_keys: std::collections::HashSet<&[i32]> = std::collections::HashSet::new();
    for h in &hand.rows {
        hand_keys.insert(h.key.as_slice());
        let d = |kind: DiffKind, ours: Option<String>, detail: Option<String>| Difference {
            kind,
            key: h.key.clone(),
            hand_label: Some(h.label.clone()),
            hand_rule: h.rule.clone(),
            ours,
            detail,
        };
        if let Some(r) = &h.rule
            && !produced.contains(r)
        {
            *unmapped.entry(r.clone()).or_default() += 1;
        }
        let Some(o) = ours.get(h.key.as_slice()) else {
            if incomplete.is_some() && res.streamed.is_some() && !res.rows_complete_for(&h.key) {
                // a streamed check keeps only the exceptional rows: a key served by a plain rule is in the file
                *matrix
                    .entry(format!("{} -> (streamed rule row)", h.label))
                    .or_default() += 1;
                continue;
            }
            diffs.push(d(
                DiffKind::OnlyHand,
                None,
                Some("not reached by our closure".into()),
            ));
            *matrix
                .entry(format!("{} -> (absent)", h.label))
                .or_default() += 1;
            continue;
        };
        *matrix
            .entry(format!("{} -> {}", h.label, ours_class(set, o)))
            .or_default() += 1;
        if h.in_box.is_some_and(|b| b != o.in_start) {
            diffs.push(d(
                DiffKind::InBoxDiffers,
                Some(format!("in_start={}", o.in_start)),
                None,
            ));
        }
        let hand_terminal = h.label == hand.terminal_label;
        let word = ours_word(set, o);
        match (hand_terminal, o.status) {
            (true, RowStatus::Terminal) => same += 1,
            (true, RowStatus::Served) => {
                diffs.push(d(DiffKind::HandTerminalOursServed, Some(word), None))
            }
            (true, RowStatus::Open) => {
                diffs.push(d(DiffKind::HandTerminalOursOpen, Some(word), None))
            }
            (false, RowStatus::Terminal) => {
                diffs.push(d(DiffKind::HandServedOursTerminal, Some(word), None))
            }
            (false, RowStatus::Open) => {
                diffs.push(d(DiffKind::HandServedOursOpen, Some(word), None))
            }
            (false, RowStatus::Served) => {
                if h.rule.as_deref() == server_hand_name(set, o).as_deref() {
                    same += 1;
                } else {
                    diffs.push(d(DiffKind::ServerDiffers, Some(word), None));
                }
            }
        }
    }
    for r in rows_all.iter().filter(|_| !hand.partial) {
        if !hand_keys.contains(r.key.as_slice()) {
            diffs.push(Difference {
                kind: DiffKind::OnlyOurs,
                key: r.key.clone(),
                hand_label: None,
                hand_rule: None,
                ours: Some(ours_word(set, r)),
                detail: None,
            });
        }
    }
    diffs.sort_by(|a, b| (a.kind, &a.key).cmp(&(b.kind, &b.key)));
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for d in &diffs {
        *counts
            .entry(
                serde_json::to_value(d.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default(),
            )
            .or_default() += 1;
    }
    let th: Vec<String> = hand.terminals().iter().map(|k| key_text(k)).collect();
    let mut to: Vec<Vec<i32>> = res
        .rows
        .iter()
        .filter(|r| r.status == RowStatus::Terminal)
        .map(|r| r.key.clone())
        .collect();
    to.sort();
    let to: Vec<String> = to.iter().map(|k| key_text(k)).collect();
    // a partial hand statement never establishes key-set equality
    let key_sets_equal = !hand.partial
        && incomplete.is_none()
        && !counts.contains_key("only_hand")
        && !counts.contains_key("only_ours");
    Comparison {
        hand_path: hand.path.clone(),
        hand_sha256: hand.sha256.clone(),
        keys_hand: hand.rows.len(),
        keys_ours: res.summary.keys_total as usize,
        key_sets_equal,
        same_server: same,
        terminals_equal: th == to,
        terminals_hand: th,
        terminals_ours: to,
        rounds_hand: hand.rounds(),
        rounds_ours: res.summary.rounds,
        counts,
        label_matrix: matrix,
        unmapped_hand_rules: unmapped,
        differences: diffs,
        complete: incomplete.is_none(),
        incomplete_why: incomplete,
    }
}
