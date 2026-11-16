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
    #[serde(default)]
    pub chart: String,
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
            chart: v
                .get("chart")
                .and_then(|c| c.as_str())
                .unwrap_or_default()
                .to_string(),
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
    match header.get("node_order") {
        Some(Value::Object(_)) => Some(header["node_order"].to_string()),
        Some(Value::String(s)) if s.trim_start().starts_with('{') => Some(s.clone()),
        // a LEGACY table (order_name `legacy`, no node_order): the engine's identity order IS the legacy comparator
        // (`node/order.rs`: no coordinate priority = the legacy order)
        None if header.get("order_name").and_then(|o| o.as_str()) == Some("legacy")
            && header.get("order_is_program").and_then(|b| b.as_bool()) != Some(true) =>
        {
            Some(
                serde_json::json!({"name": "id", "slot_permutation": Value::Null,
                    "coordinate_priority": Value::Null, "source_order": "Input"})
                .to_string(),
            )
        }
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
    /// `replaced`, `winner`, or why not (`not_smaller`, `exceptions_not_subset`, `no_rule`, `lost`, ...); pole objective:
    /// `pole_score_pending (...)`, `replaced_poles`, `replaced_size`, `poles_not_better (...)`, `size_guard (...)`.
    pub verdict: String,
    /// The candidate job's sealed table (rule scope; what a pole scorer evaluates).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    /// Its route-pole score at the keys the table's rule serves (scope `rule_poles`; section 6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pole: Option<crate::economy::polescore::PoleScore>,
    /// Pointer to the EFFECTIVE DERIVATION of the candidate's rule(s): `<derivations file>#<job>.a<attempt>`
    /// (`rustred.campaign.rule-derivation.v1` lines; TRICKS 31 / 35).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derivation: Option<String>,
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
    /// The route-pole score of the rule now in the table (pole objective runs; None = not scored).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pole: Option<crate::economy::polescore::PoleScore>,
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
                    pole: None,
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
                    pole: None,
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
    /// The SIZE MEASURE of the run: what "largest" ranks by AND what a replacement must reduce: `monomials` (default:
    /// fewer monomials, not more bytes), `bytes` (fewer record bytes; the heaviest rules of a legacy table such as T266
    /// carry huge coefficients on few monomials: rule 7181, 210.7 MB on 3,195 monomials, regenerates as a complete
    /// 17.9 MB sub-table of 1.0 M monomials) or `use_cost` (the repair flow's cost model, `cost`).
    pub by: RankBy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankBy {
    Monomials,
    Bytes,
    UseCost,
}

/// The size of rules under a measure (`use_cost`: load_ns_per_byte x bytes + per rule apply_ns_base +
/// apply_ns_per_monomial x monomials, rounded to ns).
pub fn size_of(rules: &[&TableRule], by: RankBy, cost: &crate::economy::genchoice::CostModel) -> u64 {
    match by {
        RankBy::Monomials => rules.iter().map(|r| r.monomials).sum(),
        RankBy::Bytes => rules.iter().map(|r| r.bytes).sum(),
        RankBy::UseCost => rules
            .iter()
            .map(|r| {
                cost.load_ns_per_byte * r.bytes as f64
                    + cost.apply_ns_base
                    + cost.apply_ns_per_monomial * r.monomials as f64
            })
            .sum::<f64>()
            .round() as u64,
    }
}

impl Default for RuleFilter {
    fn default() -> Self {
        RuleFilter {
            largest: Some(20),
            min_monomials: None,
            by: RankBy::Monomials,
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
    /// SUB-TREE mode (default on): the candidate job solves the target case AND its children (no `--descend open`) under
    /// a cases budget; a COMPLETE sub-table covers the whole case domain, so it may replace the old rule at its position
    /// even when its exception branches differ (e.g. guard-v2 witness branches on a pre-guard legacy rule) if its summed
    /// monomials / bytes are smaller. Off: single-case replacement under the exception-subset test.
    pub subtree: bool,
    /// Cases budget of a sub-tree job (`budgets.work.cases`; a sub-tree that does not complete within it never
    /// replaces anything).
    pub subtree_max_cases: u64,
    /// Cost model of `filter.by: use_cost` (genchoice's; repairflow.md 2).
    pub cost: crate::economy::genchoice::CostModel,
    /// TRANCHE YIELD STOP (implementer 2026-11-16): the run stops starting candidates once its yield (bytes saved per
    /// CPU-h, by the qualifying candidates so far) falls below `min_fraction` x the reference yield (the record's first
    /// improve run that saved bytes, unless given); None = no yield stop.
    pub yield_stop: Option<YieldStop>,
    /// IMPROVE POLES (section 6): the pole objective instead of size (None = size). Targets = the rules whose own
    /// rule-scope score has a pole (worst first); candidates run single-case (`--descend open`); a fresh candidate is
    /// `pole_score_pending` until its rule-scope score is supplied (node `pole_scores`), then decided by
    /// [`crate::economy::polescore::rule_pole_verdict`] without being generated again.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poles: Option<crate::economy::polescore::PolePolicy>,
}

/// See [`ImprovePolicy::yield_stop`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct YieldStop {
    /// Stop below this fraction of the reference yield (default 1/3: "within a factor 3 of the first tranche").
    pub min_fraction: f64,
    /// Reference yield in bytes per CPU-h; None = from the record ([`reference_yield`]); no reference = no stop.
    pub reference_bytes_per_cpu_h: Option<f64>,
    /// No stop before this much CPU is spent in the run (noise of the first jobs).
    pub min_cpu_s: u64,
}

impl Default for YieldStop {
    fn default() -> Self {
        YieldStop {
            min_fraction: 1.0 / 3.0,
            reference_bytes_per_cpu_h: None,
            min_cpu_s: 3600,
        }
    }
}

/// The reference yield of a record: bytes saved per CPU-h of its first improve run that saved bytes.
pub fn reference_yield(rec: &ImproveRecord) -> Option<f64> {
    rec.runs.iter().filter(|r| r.mode == "improve" && r.spent_cpu_ms > 0).find_map(|r| {
        let b = r.totals.get("table_bytes_before")?.as_u64()?;
        let a = r.totals.get("table_bytes_after")?.as_u64()?;
        (b > a).then(|| (b - a) as f64 / (r.spent_cpu_ms as f64 / 3.6e6))
    })
}

/// Yield of a run so far and whether it is below the stop line: (bytes per CPU-h, stop).
pub fn yield_check(saved_bytes: u64, spent_cpu_ms: u64, reference: Option<f64>, y: &YieldStop) -> (Option<f64>, bool) {
    if spent_cpu_ms == 0 {
        return (None, false);
    }
    let v = saved_bytes as f64 / (spent_cpu_ms as f64 / 3.6e6);
    let stop = spent_cpu_ms >= y.min_cpu_s.saturating_mul(1000)
        && reference.is_some_and(|r| v < y.min_fraction * r);
    (Some(v), stop)
}

/// The rules inside the filter window that the record already holds completely (every candidate tried): skipped.
pub fn skipped_by_record(rules: &[TableRule], rec: &ImproveRecord, p: &ImprovePolicy) -> Vec<i64> {
    let mut idx: Vec<usize> = (0..rules.len()).collect();
    let size = |i: usize| size_of(&[&rules[i]], p.filter.by, &p.cost);
    idx.sort_by(|&a, &b| size(b).cmp(&size(a)).then(a.cmp(&b)));
    idx.into_iter()
        .filter(|&i| p.filter.min_monomials.is_none_or(|m| rules[i].monomials >= m))
        .take(p.filter.largest.unwrap_or(usize::MAX))
        .filter(|&i| {
            let key = rules[i].key();
            p.candidates.iter().all(|c| rec.tried(&key, &c.name))
        })
        .map(|i| rules[i].index)
        .collect()
}

impl ImprovePolicy {
    /// The pole objective is in force (configured and not ignored).
    pub fn poles_on(&self) -> bool {
        self.poles.as_ref().is_some_and(|p| !p.is_ignored())
    }
    /// Candidate jobs solve the target case only (single mode): sub-tree off, or the pole objective.
    pub fn single(&self) -> bool {
        !self.subtree || self.poles_on()
    }
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
            subtree: true,
            subtree_max_cases: 64,
            cost: crate::economy::genchoice::CostModel::default(),
            yield_stop: Some(YieldStop::default()),
            poles: None,
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
    let size = |i: usize| size_of(&[&rules[i]], p.filter.by, &p.cost);
    idx.sort_by(|&a, &b| size(b).cmp(&size(a)).then(a.cmp(&b)));
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

/// What an IMPROVE POLES run does with one (rule, candidate).
#[derive(Clone, Debug, PartialEq)]
pub enum PoleStep {
    /// Generate the candidate (not tried on this rule yet).
    Generate,
    /// Decide from its supplied rule-scope score (generated earlier; never generated again).
    Decide(crate::economy::polescore::ScoreLine),
    /// Generated earlier, no score supplied yet: nothing to do in this run (listed as untried).
    AwaitScore,
}

/// IMPROVE POLES targets: the rules whose own score (`original` line of `scores` at their index) is decided with a
/// pole, worst first (max order, keys with a pole, monomials, index), at most `filter.largest`; per rule the candidates
/// with something to do. A (rule, candidate) already decided under the pole objective (scope `rule_poles`) is done:
/// a second run with unchanged scores does nothing. Rules without a decided own score are returned in `unscored`.
pub fn pole_targets(
    rules: &[TableRule],
    rec: &ImproveRecord,
    p: &ImprovePolicy,
    scores: &[crate::economy::polescore::ScoreLine],
) -> (Vec<(usize, Vec<(String, PoleStep)>)>, Vec<i64>) {
    let own = |idx: i64| {
        scores
            .iter()
            .find(|s| s.scope == "rule" && s.cand == "original" && s.rule_index == Some(idx))
            .map(|s| &s.score)
    };
    let mut unscored = vec![];
    let mut idx: Vec<(usize, u32, u64)> = vec![];
    for (i, r) in rules.iter().enumerate() {
        match own(r.index) {
            Some(s) if s.decided() => {
                let m = s.max_order.unwrap_or(0);
                if m > 0 {
                    idx.push((i, m, s.keys_with_pole));
                }
            }
            _ => unscored.push(r.index),
        }
    }
    idx.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then(b.2.cmp(&a.2))
            .then(rules[b.0].monomials.cmp(&rules[a.0].monomials))
            .then(rules[a.0].index.cmp(&rules[b.0].index))
    });
    let out = idx
        .into_iter()
        .take(p.filter.largest.unwrap_or(usize::MAX))
        .filter_map(|(i, _, _)| {
            let key = rules[i].key();
            let e = rec.entry(&key);
            let steps: Vec<(String, PoleStep)> = p
                .candidates
                .iter()
                .filter(|c| {
                    !e.is_some_and(|e| {
                        e.tried.iter().any(|t| t.scope == "rule_poles" && t.cand == c.name)
                    })
                })
                .map(|c| {
                    let sc = scores.iter().find(|s| {
                        s.scope == "rule" && s.cand == c.name && s.rule_index == Some(rules[i].index)
                            && s.table.is_some()
                    });
                    let step = match sc {
                        Some(s) => PoleStep::Decide(s.clone()),
                        None if rec.tried(&key, &c.name) => PoleStep::AwaitScore,
                        None => PoleStep::Generate,
                    };
                    (c.name.clone(), step)
                })
                .collect();
            // a rule whose candidates all await a score has nothing to do in this run
            steps
                .iter()
                .any(|(_, s)| *s != PoleStep::AwaitScore)
                .then_some((i, steps))
        })
        .collect();
    (out, unscored)
}

/// The STRUCTURAL half of a replacement (shared by the size and the pole objective): same target and case, exception
/// set a subset of the old one (its domain contains the old domain).
pub fn structural_verdict(old: &TableRule, new: &TableRule) -> Result<(), String> {
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
    Ok(())
}

/// May `new` replace `old` at old's dispatch position? Same target and case, exception set a subset (domain contains
/// the old domain), strictly fewer monomials (and <= old x min_gain), not more bytes.
pub fn replacement_verdict(old: &TableRule, new: &TableRule, min_gain: f64) -> Result<(), String> {
    structural_verdict(old, new)?;
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

/// [`replacement_verdict`] under the run's size measure (`by`): structural test, then the measure strictly smaller
/// (<= old x min_gain); `monomials` keeps the legacy rule (also not more bytes).
pub fn replacement_verdict_by(
    old: &TableRule,
    new: &TableRule,
    min_gain: f64,
    by: RankBy,
    cost: &crate::economy::genchoice::CostModel,
) -> Result<(), String> {
    if by == RankBy::Monomials {
        return replacement_verdict(old, new, min_gain);
    }
    structural_verdict(old, new)?;
    let (o, n) = (size_of(&[old], by, cost), size_of(&[new], by, cost));
    if n >= o || (n as f64) > o as f64 * min_gain {
        return Err(format!("not_smaller ({n} vs {o} {})", by_word(by)));
    }
    Ok(())
}

/// [`subtree_verdict`] under the run's size measure.
pub fn subtree_verdict_by(
    old: &TableRule,
    new: &[TableRule],
    min_gain: f64,
    by: RankBy,
    cost: &crate::economy::genchoice::CostModel,
) -> Result<(), String> {
    if by == RankBy::Monomials {
        return subtree_verdict(old, new, min_gain);
    }
    let Some(first) = new.iter().find(|r| r.case == old.case) else {
        return Err("no_rule_for_the_case".into());
    };
    if first.target != old.target {
        return Err(format!("other_target ({} vs {})", first.target, old.target));
    }
    let nr: Vec<&TableRule> = new.iter().collect();
    let (o, n) = (size_of(&[old], by, cost), size_of(&nr, by, cost));
    if n >= o || (n as f64) > o as f64 * min_gain {
        return Err(format!("not_smaller (sub-table {n} vs {o} {})", by_word(by)));
    }
    Ok(())
}

fn by_word(by: RankBy) -> &'static str {
    match by {
        RankBy::Monomials => "monomials",
        RankBy::Bytes => "bytes",
        RankBy::UseCost => "use-cost ns",
    }
}

/// SUB-TREE verdict: the complete sub-table `new` (its first rule must be the target case) may replace `old` when its
/// summed monomials are smaller (<= old x min_gain) and its summed bytes not larger.
pub fn subtree_verdict(old: &TableRule, new: &[TableRule], min_gain: f64) -> Result<(), String> {
    let Some(first) = new.iter().find(|r| r.case == old.case) else {
        return Err("no_rule_for_the_case".into());
    };
    if first.target != old.target {
        return Err(format!("other_target ({} vs {})", first.target, old.target));
    }
    let m: u64 = new.iter().map(|r| r.monomials).sum();
    let b: u64 = new.iter().map(|r| r.bytes).sum();
    if m >= old.monomials || (m as f64) > old.monomials as f64 * min_gain {
        return Err(format!(
            "not_smaller (sub-table {m} vs {} monomials)",
            old.monomials
        ));
    }
    if b > old.bytes {
        return Err(format!("more_bytes (sub-table {b} vs {})", old.bytes));
    }
    Ok(())
}

/// The lines replacing one rule by its sub-table: the first keeps the old `index` (same dispatch position), the others
/// take fresh indices from `next_free` (unique labels; first match follows the file order, not the index).
pub fn subtree_lines(
    sub: &[String],
    old_index: i64,
    next_free: &mut i64,
) -> Result<Vec<String>, String> {
    let mut out = Vec::with_capacity(sub.len());
    for (k, n) in sub.iter().enumerate() {
        let mut v: Value = serde_json::from_str(n).map_err(|e| e.to_string())?;
        v["type"] = Value::from("rule");
        v["index"] = Value::from(if k == 0 {
            old_index
        } else {
            let i = *next_free;
            *next_free += 1;
            i
        });
        out.push(v.to_string());
    }
    Ok(out)
}

/// A completion marker restated for the new rule counts (`end` / `assembly_end`: `total`; `chart_end` /
/// `chart_partial`: its chart's count); any other line unchanged.
pub fn restate_marker(line: &str, total: u64, per_chart: &BTreeMap<String, u64>) -> String {
    let Ok(mut v) = serde_json::from_str::<Value>(line) else {
        return line.to_string();
    };
    let kind = v
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or_default()
        .to_string();
    let n = match kind.as_str() {
        "end" | "assembly_end" => Some(total),
        "chart_end" | "chart_partial" => v
            .get("chart")
            .and_then(|c| c.as_str())
            .map(|c| per_chart.get(c).copied().unwrap_or(0)),
        _ => None,
    };
    match n {
        Some(n) if v.get("rules").is_some() => {
            v["rules"] = Value::from(n);
            v.to_string()
        }
        _ => line.to_string(),
    }
}

/// The improved table with SUB-TREE replacements (in memory; the supervisor streams the same steps): each rule at an
/// index in `repl` replaced by [`subtree_lines`], the completion markers restated; returns (lines, replaced old lines).
pub fn apply_subtrees(
    lines: &[String],
    repl: &BTreeMap<i64, Vec<String>>,
) -> Result<(Vec<String>, Vec<String>), String> {
    let rules: Vec<TableRule> = lines
        .iter()
        .filter_map(|l| TableRule::from_line(l).ok().flatten())
        .collect();
    let mut next_free = rules.iter().map(|r| r.index).max().unwrap_or(-1) + 1;
    let mut out = Vec::with_capacity(lines.len());
    let mut old = Vec::new();
    for l in lines {
        match TableRule::from_line(l)? {
            Some(r) if repl.contains_key(&r.index) => {
                out.extend(subtree_lines(&repl[&r.index], r.index, &mut next_free)?);
                old.push(l.clone());
            }
            _ => out.push(l.clone()),
        }
    }
    let mut per_chart: BTreeMap<String, u64> = BTreeMap::new();
    let mut total = 0u64;
    for l in &out {
        if let Ok(v) = serde_json::from_str::<Value>(l)
            && v.get("type").and_then(|t| t.as_str()) == Some("rule")
        {
            total += 1;
            let c = v
                .get("chart")
                .and_then(|c| c.as_str())
                .unwrap_or_default()
                .to_string();
            *per_chart.entry(c).or_default() += 1;
        }
    }
    let out = out
        .iter()
        .map(|l| restate_marker(l, total, &per_chart))
        .collect();
    Ok((out, old))
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
            table: None,
            pole: None,
            derivation: None,
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
                table: None,
                pole: None,
                derivation: None,
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
    /// Rule-scope pole scores (`rustred.campaign.pole-score.v1`: `original` per rule index of this table, and
    /// candidates with their sealed table) for the pole objective (section 6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pole_scores: Option<crate::economy::genchoice::FileEntry>,
    /// The dependence table of published symbols (sha256-bound; absent = upper bound), as in genchoice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependences: Option<crate::economy::genchoice::FileEntry>,
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
        if let Some(p) = &c.improve.poles
            && !p.is_ignored()
        {
            p.check().map_err(|e| format!("improve config: {e}"))?;
        }
        Ok(c)
    }
}
