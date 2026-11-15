//! IMPROVEMENT RECORD and IMPROVE MODE (lane sizegen; interfaces/genchoice.md section 5; USER guideline 2026-11-15: the
//! size search must be ITERATABLE: "retain enough information that the user can launch your implementation again on
//! an existing solve to try and improve it further later"). PURE: no process, no file, no clock, no ledger.
//!
//! The record (schema [`RECORD_SCHEMA`], [`RECORD_VERSION`]) is bound to the sha256 of the table it describes and holds,
//! per rule target, every candidate tried (scope `node`: a whole-node generation choice; scope `rule`: a single-rule
//! regeneration under the table's own order), its outcome, the chosen one and why, and what was not tried; per run the
//! settings, budget and totals; and the ranking of the rules by bytes ("where the next CPU-hour is best spent").
//! IMPROVE MODE regenerates the targeted rules one by one and replaces a rule only by a STRUCTURALLY verified smaller one
//! ([`replacement_verdict`]): same target and case, exception set a SUBSET of the old one (its domain contains the old
//! domain: coverage at the same dispatch position cannot shrink), fewer monomials and not more bytes; the node's closing
//! check (closure, terminals, statement set unchanged; rr_proof green) is the acceptance of the improved table.

use crate::economy::genchoice::NamedArgs;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const RECORD_SCHEMA: &str = "rustred.campaign.improve-record.v1";
/// Bumped when the record's meaning changes (simple versioning, no fingerprints).
pub const RECORD_VERSION: u32 = 1;

/// One rule record of a table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRule {
    pub index: i64,
    pub target: String,
    /// Case key (fixed values + equations, as genchoice's case keys).
    pub case: String,
    pub fixed: Vec<Option<i64>>,
    pub equations: Vec<String>,
    /// Canonical exception branches (each branch: its polynomials sorted and joined by `&`).
    pub exceptions: Vec<String>,
    pub monomials: u64,
    pub bytes: u64,
}

impl TableRule {
    pub fn key(&self) -> String {
        format!("{}|{}", self.target, self.case)
    }

    /// From one rule line (None for other record kinds).
    pub fn from_line(line: &str) -> Result<Option<TableRule>, String> {
        let v: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        if v.get("type").and_then(|t| t.as_str()) != Some("rule") {
            return Ok(None);
        }
        let m = crate::economy::RuleMeasure::from_record_json(line)?;
        let fixed: Vec<Option<i64>> = v
            .get("fixed")
            .and_then(|f| f.as_array())
            .map(|a| a.iter().map(|x| x.as_i64()).collect())
            .unwrap_or_default();
        let mut equations: Vec<String> = v
            .get("equations")
            .and_then(|f| f.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        equations.sort();
        let mut exceptions: Vec<String> = v
            .get("exceptions")
            .and_then(|e| e.as_array())
            .map(|a| {
                a.iter()
                    .map(|b| {
                        let mut ps: Vec<String> = b
                            .as_array()
                            .map(|q| {
                                q.iter()
                                    .map(|x| {
                                        x.as_str().map_or_else(|| x.to_string(), str::to_string)
                                    })
                                    .collect()
                            })
                            .unwrap_or_else(|| vec![b.to_string()]);
                        ps.sort();
                        ps.join("&")
                    })
                    .collect()
            })
            .unwrap_or_default();
        exceptions.sort();
        exceptions.dedup();
        let f: Vec<String> = fixed
            .iter()
            .map(|x| x.map_or("_".to_string(), |v| v.to_string()))
            .collect();
        Ok(Some(TableRule {
            index: v.get("index").and_then(|i| i.as_i64()).unwrap_or(-1),
            target: v
                .get("target")
                .and_then(|t| t.as_str())
                .unwrap_or_default()
                .to_string(),
            case: format!("{}|{}", f.join(","), equations.join(";")),
            fixed,
            equations,
            exceptions,
            monomials: m.monomials,
            bytes: m.bytes,
        }))
    }
}

/// The header and the rules of a table given as lines.
pub fn table_rules<'a>(
    lines: impl IntoIterator<Item = &'a str>,
) -> Result<(Option<Value>, Vec<TableRule>), String> {
    let mut header = None;
    let mut rules = Vec::new();
    for (n, l) in lines.into_iter().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        if n == 0 && l.contains("\"header\"") {
            header = serde_json::from_str(l).ok();
            continue;
        }
        if let Some(r) = TableRule::from_line(l).map_err(|e| format!("line {}: {e}", n + 1))? {
            rules.push(r);
        }
    }
    Ok((header, rules))
}

/// The node order of a table header as the engine's `--order` JSON (`node_order` object), None when absent.
pub fn header_order(header: &Value) -> Option<String> {
    match header.get("node_order")? {
        Value::Object(_) => Some(header["node_order"].to_string()),
        Value::String(s) if s.trim_start().starts_with('{') => Some(s.clone()),
        _ => None,
    }
}

/// One candidate tried on a rule target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tried {
    pub cand: String,
    /// `node` (whole-node generation choice) or `rule` (single-rule regeneration under the table's order).
    pub scope: String,
    pub run: u32,
    #[serde(default)]
    pub args: Vec<String>,
    /// How the job ended (exit class, cap / wall / memory) or `in_table` for node-scope comparisons.
    pub status: String,
    pub monomials: Option<u64>,
    pub bytes: Option<u64>,
    pub cpu_ms: Option<u64>,
    /// `replaced`, `winner`, or why not (`not_smaller`, `exceptions_not_subset`, `no_rule`, `lost`, ...).
    pub verdict: String,
}

/// The record of one rule target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuleEntry {
    pub key: String,
    pub index: i64,
    pub target: String,
    pub case: String,
    pub monomials: u64,
    pub bytes: u64,
    pub tried: Vec<Tried>,
    /// The candidate whose rule is in the table now (`original` for a table without a record).
    pub chosen: String,
    pub why: String,
    /// Candidates known but not tried (economy, filter, budget), with the reason.
    #[serde(default)]
    pub untried: Vec<String>,
}

/// One run that wrote / appended the record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunEntry {
    pub run: u32,
    /// `generate` or `improve`.
    pub mode: String,
    pub settings: Value,
    pub table_in_sha256: Option<String>,
    pub table_out_sha256: String,
    pub budget_cpu_ms: Option<u64>,
    pub spent_cpu_ms: u64,
    pub totals: Value,
}

/// The improvement record of a node's table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImproveRecord {
    pub schema: String,
    pub version: u32,
    pub node: String,
    /// sha256 of the table this record describes (the latest run's output).
    pub table_sha256: String,
    pub runs: Vec<RunEntry>,
    pub rules: Vec<RuleEntry>,
    /// Rule keys by bytes, largest first (with bytes and monomials).
    pub ranking_by_bytes: Vec<(String, u64, u64)>,
}

impl ImproveRecord {
    /// A record for a table without one: every rule `original`, nothing tried.
    pub fn empty(node: &str, table_sha256: &str, rules: &[TableRule]) -> ImproveRecord {
        let mut r = ImproveRecord {
            schema: RECORD_SCHEMA.into(),
            version: RECORD_VERSION,
            node: node.into(),
            table_sha256: table_sha256.into(),
            runs: vec![],
            rules: rules
                .iter()
                .map(|t| RuleEntry {
                    key: t.key(),
                    index: t.index,
                    target: t.target.clone(),
                    case: t.case.clone(),
                    monomials: t.monomials,
                    bytes: t.bytes,
                    tried: vec![],
                    chosen: "original".into(),
                    why: "no improvement record: the table's own rule, nothing tried".into(),
                    untried: vec![],
                })
                .collect(),
            ranking_by_bytes: vec![],
        };
        r.rank();
        r
    }

    pub fn parse(text: &str) -> Result<ImproveRecord, String> {
        let r: ImproveRecord =
            serde_json::from_str(text).map_err(|e| format!("improvement record: {e}"))?;
        if r.schema != RECORD_SCHEMA {
            return Err(format!("improvement record: schema {:?}", r.schema));
        }
        if r.version != RECORD_VERSION {
            return Err(format!(
                "improvement record: version {} (this build reads {RECORD_VERSION})",
                r.version
            ));
        }
        Ok(r)
    }

    pub fn rank(&mut self) {
        let mut v: Vec<(String, u64, u64)> = self
            .rules
            .iter()
            .map(|r| (r.key.clone(), r.bytes, r.monomials))
            .collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        self.ranking_by_bytes = v;
    }

    pub fn entry(&self, key: &str) -> Option<&RuleEntry> {
        self.rules.iter().find(|r| r.key == key)
    }

    /// Was this rule-scope candidate tried on this rule (any earlier run)?
    pub fn tried(&self, key: &str, cand: &str) -> bool {
        self.entry(key)
            .is_some_and(|e| e.tried.iter().any(|t| t.scope == "rule" && t.cand == cand))
    }

    pub fn next_run(&self) -> u32 {
        self.runs.iter().map(|r| r.run).max().map_or(1, |m| m + 1)
    }

    /// Bring the record onto the rules of `rules` (a table whose records match by key; new keys get empty entries).
    pub fn align(&mut self, rules: &[TableRule]) {
        let mut out = Vec::with_capacity(rules.len());
        for t in rules {
            let e = match self.rules.iter().find(|e| e.key == t.key()) {
                Some(e) => RuleEntry {
                    index: t.index,
                    monomials: t.monomials,
                    bytes: t.bytes,
                    ..e.clone()
                },
                None => RuleEntry {
                    key: t.key(),
                    index: t.index,
                    target: t.target.clone(),
                    case: t.case.clone(),
                    monomials: t.monomials,
                    bytes: t.bytes,
                    tried: vec![],
                    chosen: "original".into(),
                    why: "not in the record: nothing tried".into(),
                    untried: vec![],
                },
            };
            out.push(e);
        }
        self.rules = out;
        self.rank();
    }
}

/// Which rules an improve run targets: the `largest` rules by monomials and / or those with at least `min_monomials`
/// (both given: both conditions).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RuleFilter {
    pub largest: Option<usize>,
    pub min_monomials: Option<u64>,
}

impl Default for RuleFilter {
    fn default() -> Self {
        RuleFilter {
            largest: Some(20),
            min_monomials: None,
        }
    }
}

/// IMPROVE MODE settings (config `policy.improve`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ImprovePolicy {
    pub filter: RuleFilter,
    /// Rule-scope candidates (named generate flag sets, applied under the table's own order with `--descend open`).
    pub candidates: Vec<NamedArgs>,
    /// Safety net per candidate job (time-decided; a job stopped by it is `wall` and never replaces anything).
    pub wall_case_s: Option<u64>,
    /// Total CPU budget of the run (supervisor cpu_ms of the finished candidate jobs); None = no total cap.
    pub budget_cpu_s: Option<u64>,
    /// A replacement needs monomials <= old x min_gain (1.0: strictly fewer is required anyway).
    pub min_gain: f64,
}

impl Default for ImprovePolicy {
    fn default() -> Self {
        let n = |name: &str, args: &[&str]| NamedArgs {
            name: name.into(),
            args: args.iter().map(|x| x.to_string()).collect(),
        };
        ImprovePolicy {
            filter: RuleFilter::default(),
            candidates: vec![
                n("rung", &["--seed-pose-rung"]),
                n("tight", &["--seed-layer-cap", "0,0,1", "--max-depth", "3"]),
                n(
                    "rung_tight",
                    &[
                        "--seed-pose-rung",
                        "--seed-layer-cap",
                        "0,0,1",
                        "--max-depth",
                        "3",
                    ],
                ),
            ],
            wall_case_s: Some(1800),
            budget_cpu_s: None,
            min_gain: 1.0,
        }
    }
}

/// The targeted rules (positions in `rules`, largest first) with their untried candidates; rules with nothing left
/// to try are omitted (a second run with unchanged settings does nothing).
pub fn targets(
    rules: &[TableRule],
    rec: &ImproveRecord,
    p: &ImprovePolicy,
) -> Vec<(usize, Vec<String>)> {
    let mut idx: Vec<usize> = (0..rules.len()).collect();
    idx.sort_by(|&a, &b| rules[b].monomials.cmp(&rules[a].monomials).then(a.cmp(&b)));
    let idx: Vec<usize> = idx
        .into_iter()
        .filter(|&i| {
            p.filter
                .min_monomials
                .is_none_or(|m| rules[i].monomials >= m)
        })
        .take(p.filter.largest.unwrap_or(usize::MAX))
        .collect();
    idx.into_iter()
        .filter_map(|i| {
            let key = rules[i].key();
            let c: Vec<String> = p
                .candidates
                .iter()
                .map(|c| c.name.clone())
                .filter(|c| !rec.tried(&key, c))
                .collect();
            (!c.is_empty()).then_some((i, c))
        })
        .collect()
}

/// May `new` replace `old` at old's dispatch position? Same target and case, exception set a subset (domain contains
/// the old domain), strictly fewer monomials (and <= old x min_gain), not more bytes.
pub fn replacement_verdict(old: &TableRule, new: &TableRule, min_gain: f64) -> Result<(), String> {
    if new.target != old.target {
        return Err(format!("other_target ({} vs {})", new.target, old.target));
    }
    if new.case != old.case {
        return Err("other_case".into());
    }
    let olde: BTreeSet<&String> = old.exceptions.iter().collect();
    if let Some(x) = new.exceptions.iter().find(|x| !olde.contains(x)) {
        return Err(format!("exceptions_not_subset (new branch {x})"));
    }
    if new.monomials >= old.monomials || (new.monomials as f64) > old.monomials as f64 * min_gain {
        return Err(format!(
            "not_smaller ({} vs {} monomials)",
            new.monomials, old.monomials
        ));
    }
    if new.bytes > old.bytes {
        return Err(format!("more_bytes ({} vs {})", new.bytes, old.bytes));
    }
    Ok(())
}

/// The improved table: every rule line whose `index` is in `repl` replaced by the new record with the OLD index (same
/// dispatch position); returns (lines, the replaced old lines).
pub fn apply(
    lines: &[String],
    repl: &BTreeMap<i64, String>,
) -> Result<(Vec<String>, Vec<String>), String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut old = Vec::new();
    for l in lines {
        let rep = match TableRule::from_line(l)? {
            Some(r) => repl.get(&r.index).map(|n| (r.index, n)),
            None => None,
        };
        match rep {
            Some((index, n)) => {
                let mut v: Value = serde_json::from_str(n).map_err(|e| e.to_string())?;
                v["index"] = Value::from(index);
                v["type"] = Value::from("rule");
                out.push(v.to_string());
                old.push(l.clone());
            }
            None => out.push(l.clone()),
        }
    }
    Ok((out, old))
}

/// The record of a generation run: per winner rule the same case in every other candidate's table (scope `node`).
pub fn generation_record(
    node: &str,
    table_sha256: &str,
    winner: &str,
    winner_rules: &[TableRule],
    others: &[(String, Vec<TableRule>)],
    untried: &[String],
    run: RunEntry,
) -> ImproveRecord {
    let mut rec = ImproveRecord::empty(node, table_sha256, winner_rules);
    let maps: Vec<BTreeMap<String, &TableRule>> = others
        .iter()
        .map(|(_, rs)| {
            let mut m = BTreeMap::new();
            for r in rs {
                m.entry(r.case.clone()).or_insert(r);
            }
            m
        })
        .collect();
    for e in rec.rules.iter_mut() {
        e.chosen = winner.into();
        e.why = "node-level generation choice (genchoice): the selected full table".into();
        e.tried.push(Tried {
            cand: winner.into(),
            scope: "node".into(),
            run: run.run,
            args: vec![],
            status: "in_table".into(),
            monomials: Some(e.monomials),
            bytes: Some(e.bytes),
            cpu_ms: None,
            verdict: "winner".into(),
        });
        for ((id, _), m) in others.iter().zip(&maps) {
            let r = m.get(&e.case);
            e.tried.push(Tried {
                cand: id.clone(),
                scope: "node".into(),
                run: run.run,
                args: vec![],
                status: if r.is_some() {
                    "in_table"
                } else {
                    "case_absent"
                }
                .into(),
                monomials: r.map(|r| r.monomials),
                bytes: r.map(|r| r.bytes),
                cpu_ms: None,
                verdict: "lost_node_choice".into(),
            });
        }
        e.untried = untried.to_vec();
    }
    rec.runs.push(run);
    rec
}

pub const IMPROVE_CONFIG_SCHEMA: &str = "rustred.campaign.improve.v1";

fn default_common_args() -> Vec<String> {
    vec![]
}

/// A node of an improve run: its family and its EXISTING table (+ optional record, + optional box for the import spec).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImproveNode {
    pub node: String,
    pub family: String,
    pub family_sha256: String,
    pub table: crate::economy::genchoice::FileEntry,
    #[serde(default)]
    pub record: Option<crate::economy::genchoice::FileEntry>,
    #[serde(default)]
    pub r#box: Option<crate::economy::genchoice::FileEntry>,
}

/// The improve config (campaign input of role `genchoice`, schema [`IMPROVE_CONFIG_SCHEMA`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImproveConfig {
    pub schema: String,
    pub program: String,
    pub nodes: Vec<ImproveNode>,
    #[serde(default)]
    pub improve: ImprovePolicy,
    #[serde(default = "default_common_args")]
    pub common_args: Vec<String>,
    #[serde(default)]
    pub job: crate::economy::genchoice::JobPolicy,
}

impl ImproveConfig {
    pub fn parse(text: &str) -> Result<ImproveConfig, String> {
        let c: ImproveConfig =
            serde_json::from_str(text).map_err(|e| format!("improve config: {e}"))?;
        if c.schema != IMPROVE_CONFIG_SCHEMA {
            return Err(format!("improve config: schema {:?}", c.schema));
        }
        if c.nodes.is_empty() || c.improve.candidates.is_empty() {
            return Err("improve config: no nodes or no candidates".into());
        }
        let mut names = BTreeSet::new();
        for k in &c.improve.candidates {
            if k.name.is_empty()
                || !k
                    .name
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                || !names.insert(&k.name)
            {
                return Err(format!(
                    "improve config: candidate name {:?} ([A-Za-z0-9_]+, unique)",
                    k.name
                ));
            }
        }
        Ok(c)
    }
}
