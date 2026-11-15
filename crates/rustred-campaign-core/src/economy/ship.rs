//! SHIP ECONOMY (lane shipecon; interfaces/shipecon.md; coordinator request 2026-11-15 item 1, USER rulings: "recipes may
//! take of the order of seconds more than the table" (implementer default: 10 s per key, first use, cached afterwards);
//! the tool "leverages recipes when warranted to keep the size of the close under control"; the decision is FULLY
//! PARAMETRIC with defaults from experiments; ITERABLE: a later run with other settings continues from the records).
//!
//! A phase AFTER a node is closed: per TABLE RULE of the shipped material, the bytes it costs, the certified keys it
//! serves (inside the start set / in the closure outside it), and the alternatives "bounded recipe at the key" and
//! "stored relation per key", each MEASURED (seconds added per key at first use, bytes of what replaces it). A rule is
//! migrated only under THE GUARD: the bounded recipe (stated steps = seed sets, shell depth, centre, work and wall
//! bounds) was SOLVED, certified and recorded at EVERY certified key the rule serves ([`GuardRecord`], bound to the table
//! and record sha256). A rule serving no certified key is moved to the "generated data kept aside" (the table stays as
//! data; the closure cannot change: no certified key's first match moves). Typed outcome per rule ([`Outcome`]).
//!
//! Pure library: no process, no file, no clock. The caller measures (engine solve-recipe jobs through the process
//! layer, the table time through the algebra crate), runs the closing check on the shipped spec ([`shipped_spec`]) and
//! compares ([`ClosingCheck::compare`]). Generic (L8): nothing here knows a node, an arity, a count or a path.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::closure::sha256::Sha256;
use crate::import::spec::{FileRef, Removed, SourceKind, SourceSpec, TableSetSpec};
use crate::schema::canon::canonical_json;

/// Schema of the policy (campaign input / driver config section `ship`).
pub const SHIP_POLICY_SCHEMA: &str = "rustred.campaign.shipecon.v1";
/// The economy record family shared with lane sizegen (generation choices: kind `generation-choice`).
pub const ECONOMY_RECORD_SCHEMA: &str = "rustred.campaign.economy-record.v1";
/// `kind` of a ship-economy record.
pub const RECORD_KIND: &str = "ship-rule";
/// Schema of the guard record of one rule domain.
pub const GUARD_SCHEMA: &str = "rustred.campaign.shipecon-guard.v1";
/// Schema of the report of one run.
pub const REPORT_SCHEMA: &str = "rustred.campaign.shipecon-report.v1";
/// Version of the decision procedure (bumped when [`decide`] changes meaning: old records are then re-decided).
pub const DECISION_VERSION: u32 = 1;

fn sha256_hex(b: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// sha256 of the canonical JSON of `v` (floats allowed: policies carry seconds).
pub fn digest_of(v: &Value) -> String {
    sha256_hex(
        canonical_json(v, true)
            .unwrap_or_else(|_| v.to_string())
            .as_bytes(),
    )
}

// ------------------------------------------------------------------------------------------------------------------
// policy

/// Report (default): the proposal is written, nothing changes. Apply: the shipped artifact is written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShipMode {
    #[default]
    Report,
    Apply,
}

/// Which rules are examined (the guard costs engine solves: a budget).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Candidates {
    /// Rules below this size are not examined (the saving cannot pay a solve).
    pub min_rule_bytes: u64,
    /// Rules serving more certified keys are not examined (few-key heavy rules are the lever).
    pub max_keys_per_rule: u64,
    /// At most this many rules examined per run (largest first); None = no cap.
    pub max_rules: Option<u64>,
    /// At most this many guard keys (solves) per run in total.
    pub max_guard_keys: u64,
}

impl Default for Candidates {
    fn default() -> Self {
        Candidates {
            min_rule_bytes: 1 << 20,
            max_keys_per_rule: 64,
            max_rules: None,
            max_guard_keys: 4096,
        }
    }
}

/// The bars of the decision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Bars {
    /// ABSOLUTE bar: a key is served by its recipe only if its first-use solve took at most this (seconds).
    pub max_recipe_seconds_per_key: f64,
    /// RELATIVE bar: recipe seconds / measured table seconds at the key; None = unbounded (only the absolute bar).
    pub max_recipe_to_table_ratio: Option<f64>,
    /// A migration must save at least this many bytes per second added (summed first-use seconds of its keys).
    pub min_bytes_saved_per_second: f64,
    /// A migration must save at least this many bytes in all.
    pub min_bytes_saved: u64,
}

impl Default for Bars {
    fn default() -> Self {
        Bars {
            max_recipe_seconds_per_key: 10.0,
            max_recipe_to_table_ratio: None,
            min_bytes_saved_per_second: 1_048_576.0,
            min_bytes_saved: 1 << 20,
        }
    }
}

/// The bounded recipe the guard proves (the engine's run-time bounds by default).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RecipeBound {
    /// Step names (import.md 14 grammar), tried in order per key; the first that certifies wins.
    pub steps: Vec<String>,
    /// Deterministic work bound of one solve (`--max-trace-rows`; also written into the shipped recipe record).
    pub max_trace_rows: Option<u64>,
    /// Wall bound of one solve (`--key-budget-s`, time-decided).
    pub key_budget_s: Option<u64>,
    /// `m1_first` | `first_admissible`.
    pub relation_rule: String,
    /// Centre priorities; None = the node order's coordinate priority.
    pub line_priority: Option<Vec<usize>>,
    pub numerator_priority: Option<Vec<usize>>,
    /// Keys per solve job.
    pub max_keys_per_job: usize,
}

impl Default for RecipeBound {
    fn default() -> Self {
        RecipeBound {
            // T266 first measurement (coordinator addendum 2026-11-15T21:01Z, lane vk5l-wp2,
            // artefacts/size/recipe_vs_table_T266.first5.json): centre-on-key depth 2 found NO relation on 5/5 heavy
            // few-key rules; centre one step down the first dotted line + floor + depth 2 + symmetry rows + m1_first
            // solved; then the repair flow's depth-2 face steps (repairflow.md 9)
            steps: vec![
                "d2_cfirst_fon_ssym_mM1C".into(),
                "d2_clast_fon_mM1C".into(),
                "d2_cunion_fon_mM1C".into(),
            ],
            max_trace_rows: Some(200_000),
            key_budget_s: Some(120),
            relation_rule: "m1_first".into(),
            line_priority: None,
            numerator_priority: None,
            max_keys_per_job: 16,
        }
    }
}

/// Stored relations (the certified recipe relation at the key shipped as a `rustred.relation-entry.v1`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Stored {
    pub enabled: bool,
    /// The stored relations of a rule may cost at most this fraction of the rule's bytes.
    pub max_bytes_ratio: f64,
}

impl Default for Stored {
    fn default() -> Self {
        Stored {
            enabled: true,
            max_bytes_ratio: 0.5,
        }
    }
}

/// Rules serving no certified key: moved to the generated data kept aside.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct KeptAside {
    pub enabled: bool,
    pub min_bytes: u64,
}

impl Default for KeptAside {
    fn default() -> Self {
        KeptAside {
            enabled: true,
            min_bytes: 0,
        }
    }
}

/// Table cost ESTIMATE (used only where the table time was not measured; names shared with the repair flow and
/// genchoice: `cost.*`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct TableCost {
    pub load_ns_per_byte: f64,
    pub apply_ns_base: f64,
    pub apply_ns_per_monomial: f64,
    /// Prior of the recipe solve seconds per key (candidate ranking only; decisions use measured seconds).
    pub recipe_solve_s: f64,
}

impl Default for TableCost {
    fn default() -> Self {
        // repairflow policy.rs CostModel (MEASURED there: rr_proof instantiate on T178 kept rules: load ~15 ns/B,
        // apply 1.3 ms + 82 ns per monomial); recipe prior = coord-recipe-solve.md 8 depth-2 medians 0.6-6.4 s/key
        TableCost {
            load_ns_per_byte: 15.0,
            apply_ns_base: 1_300_000.0,
            apply_ns_per_monomial: 82.0,
            recipe_solve_s: 5.0,
        }
    }
}

/// The closing check of the shipped spec.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct CheckPolicy {
    /// Rounds of "revert the rules behind a failure and check again".
    pub max_iterations: u32,
}

impl Default for CheckPolicy {
    fn default() -> Self {
        CheckPolicy { max_iterations: 4 }
    }
}

/// The ship-economy policy (`rustred.campaign.shipecon.v1`; every field optional, unknown fields refused).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ShipPolicy {
    pub schema: String,
    pub enabled: bool,
    pub mode: ShipMode,
    pub candidates: Candidates,
    pub bars: Bars,
    pub recipe: RecipeBound,
    pub stored: Stored,
    pub kept_aside: KeptAside,
    pub cost: TableCost,
    pub check: CheckPolicy,
}

impl Default for ShipPolicy {
    fn default() -> Self {
        ShipPolicy {
            schema: SHIP_POLICY_SCHEMA.into(),
            enabled: true,
            mode: ShipMode::Report,
            candidates: Candidates::default(),
            bars: Bars::default(),
            recipe: RecipeBound::default(),
            stored: Stored::default(),
            kept_aside: KeptAside::default(),
            cost: TableCost::default(),
            check: CheckPolicy::default(),
        }
    }
}

impl ShipPolicy {
    /// Parse a policy (defaults for absent fields; unknown fields and invalid values refused).
    pub fn parse(text: &str) -> Result<ShipPolicy, String> {
        let p: ShipPolicy = serde_json::from_str(text).map_err(|e| format!("ship policy: {e}"))?;
        p.validate()?;
        Ok(p)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SHIP_POLICY_SCHEMA {
            return Err(format!(
                "ship policy: schema {:?}, expected {SHIP_POLICY_SCHEMA}",
                self.schema
            ));
        }
        let b = &self.bars;
        if !(b.max_recipe_seconds_per_key.is_finite() && b.max_recipe_seconds_per_key >= 0.0) {
            return Err("bars.max_recipe_seconds_per_key: a finite number >= 0".into());
        }
        if let Some(r) = b.max_recipe_to_table_ratio
            && !(r.is_finite() && r > 0.0)
        {
            return Err("bars.max_recipe_to_table_ratio: a finite number > 0 or null".into());
        }
        if !(b.min_bytes_saved_per_second.is_finite() && b.min_bytes_saved_per_second >= 0.0) {
            return Err("bars.min_bytes_saved_per_second: a finite number >= 0".into());
        }
        if !(self.stored.max_bytes_ratio.is_finite() && self.stored.max_bytes_ratio >= 0.0) {
            return Err("stored.max_bytes_ratio: a finite number >= 0".into());
        }
        if self.recipe.steps.is_empty() {
            return Err("recipe.steps: at least one step (the guard needs a bounded recipe)".into());
        }
        for s in &self.recipe.steps {
            crate::import::recipe_steps::parse_step(s).map_err(|e| format!("recipe.steps: {e}"))?;
        }
        if !matches!(
            self.recipe.relation_rule.as_str(),
            "m1_first" | "first_admissible"
        ) {
            return Err("recipe.relation_rule: m1_first | first_admissible".into());
        }
        if self.recipe.max_keys_per_job == 0 {
            return Err("recipe.max_keys_per_job: >= 1".into());
        }
        Ok(())
    }

    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// Digest of the effective policy (every field).
    pub fn digest(&self) -> String {
        digest_of(&self.to_value())
    }

    /// Digest of what decides a rule (bars, stored, kept-aside, recipe bound, decision version): a record with the
    /// same decision digest and the same inputs is not decided again.
    pub fn decision_digest(&self) -> String {
        digest_of(&json!({"v": DECISION_VERSION, "bars": self.bars, "stored": self.stored,
            "kept_aside": self.kept_aside, "recipe": self.bound_value()}))
    }

    /// The bound of a trial: a trial measured under the same bound (or, for deterministic verdicts, any bound) is
    /// reused.
    pub fn bound_value(&self) -> Value {
        json!({"max_trace_rows": self.recipe.max_trace_rows, "key_budget_s": self.recipe.key_budget_s,
               "relation_rule": self.recipe.relation_rule})
    }
}

// ------------------------------------------------------------------------------------------------------------------
// census

/// A table rule of the shipped material (identity = record sha256 inside a table of a given sha256).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RuleRef {
    /// The import source id (for `removed`).
    pub source: String,
    pub table: String,
    pub label: i64,
    pub ordinal: usize,
    pub record_sha256: String,
    pub table_sha256: Option<String>,
}

impl RuleRef {
    pub fn id(&self) -> String {
        format!("{}#{}", self.source, self.label)
    }
}

/// What one rule costs and serves in the closure of the statement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuleService {
    pub rule: RuleRef,
    pub bytes: u64,
    /// Certified keys it serves (first-match winner) inside the start set / in the closure outside it.
    pub served_in_start: u64,
    pub served_outside_start: u64,
    /// The keys (kept only while the rule serves at most the builder's key cap; else empty and `keys_complete` false).
    pub keys: Vec<(Vec<i32>, bool)>,
    pub keys_complete: bool,
    /// Optional record measures (from the record when the caller read them).
    pub rhs_terms: Option<u64>,
    pub monomials: Option<u64>,
}

impl RuleService {
    pub fn served(&self) -> u64 {
        self.served_in_start + self.served_outside_start
    }
}

/// Builds the census from the rules of the dispatch and the statement's rows, streaming (memory: the key lists of the
/// rules with at most `key_cap` keys).
pub struct CensusBuilder {
    rules: Vec<RuleService>,
    by_sha: BTreeMap<String, usize>,
    key_cap: u64,
    pub rows: u64,
    pub rows_not_rule: u64,
    pub rows_unknown_rule: u64,
}

impl CensusBuilder {
    /// `rules`: every table rule of the dispatch (identity, bytes); `key_cap`: keys kept per rule.
    pub fn new(rules: Vec<(RuleRef, u64)>, key_cap: u64) -> Result<CensusBuilder, String> {
        let mut by_sha = BTreeMap::new();
        let mut out = Vec::with_capacity(rules.len());
        for (i, (r, b)) in rules.into_iter().enumerate() {
            if by_sha.insert(r.record_sha256.clone(), i).is_some() {
                return Err(format!(
                    "record sha256 {} twice in the dispatch",
                    r.record_sha256
                ));
            }
            out.push(RuleService {
                rule: r,
                bytes: b,
                served_in_start: 0,
                served_outside_start: 0,
                keys: vec![],
                keys_complete: true,
                rhs_terms: None,
                monomials: None,
            });
        }
        Ok(CensusBuilder {
            rules: out,
            by_sha,
            key_cap,
            rows: 0,
            rows_not_rule: 0,
            rows_unknown_rule: 0,
        })
    }

    /// One closure row: the key, whether it is in the start set, and its serving table record (None: terminal,
    /// relation, credit, open).
    pub fn add(&mut self, key: &[i32], in_start: bool, server_record_sha256: Option<&str>) {
        self.rows += 1;
        let Some(sha) = server_record_sha256 else {
            self.rows_not_rule += 1;
            return;
        };
        let Some(&i) = self.by_sha.get(sha) else {
            self.rows_unknown_rule += 1;
            return;
        };
        let r = &mut self.rules[i];
        if in_start {
            r.served_in_start += 1;
        } else {
            r.served_outside_start += 1;
        }
        if r.keys_complete {
            if (r.keys.len() as u64) < self.key_cap {
                r.keys.push((key.to_vec(), in_start));
            } else {
                r.keys.clear();
                r.keys_complete = false;
            }
        }
    }

    /// Attach record measures (rhs terms, monomials) by record sha256.
    pub fn measures(&mut self, sha: &str, rhs_terms: Option<u64>, monomials: Option<u64>) {
        if let Some(&i) = self.by_sha.get(sha) {
            self.rules[i].rhs_terms = rhs_terms;
            self.rules[i].monomials = monomials;
        }
    }

    pub fn finish(mut self) -> Census {
        for r in &mut self.rules {
            r.keys.sort();
        }
        let total_bytes = self.rules.iter().map(|r| r.bytes).sum();
        Census {
            rules: self.rules,
            rows: self.rows,
            rows_not_rule: self.rows_not_rule,
            rows_unknown_rule: self.rows_unknown_rule,
            total_bytes,
        }
    }
}

/// Per rule: bytes and keys served (every rule of the dispatch).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Census {
    pub rules: Vec<RuleService>,
    pub rows: u64,
    pub rows_not_rule: u64,
    pub rows_unknown_rule: u64,
    pub total_bytes: u64,
}

impl Census {
    /// Totals: bytes of rules serving no certified key, serving only keys outside the start set, serving start keys.
    pub fn totals(&self) -> Value {
        let mut b = [0u64; 3];
        let mut n = [0u64; 3];
        for r in &self.rules {
            let c = if r.served() == 0 {
                0
            } else if r.served_in_start == 0 {
                1
            } else {
                2
            };
            b[c] += r.bytes;
            n[c] += 1;
        }
        json!({"rules": self.rules.len(), "bytes": self.total_bytes, "rows": self.rows,
            "rows_not_rule": self.rows_not_rule, "rows_unknown_rule": self.rows_unknown_rule,
            "serving_no_certified_key": {"rules": n[0], "bytes": b[0]},
            "serving_only_outside_start": {"rules": n[1], "bytes": b[1]},
            "serving_start_keys": {"rules": n[2], "bytes": b[2]}})
    }
}

// ------------------------------------------------------------------------------------------------------------------
// candidacy

/// What the run does with a rule before any measurement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "class", rename_all = "snake_case")]
pub enum Candidacy {
    /// Serves no certified key: kept aside (no measurement needed).
    KeptAside,
    /// Examined: the guard runs at its keys.
    Examine,
    /// Not examined, with the reason (a later run with other settings may examine it).
    NotExamined { reason: String },
}

/// Candidacy of every rule of the census (same order), with the run's guard budget applied (largest bytes first).
pub fn candidacy(p: &ShipPolicy, census: &Census) -> Vec<Candidacy> {
    let c = &p.candidates;
    let mut out: Vec<Candidacy> = census
        .rules
        .iter()
        .map(|r| {
            if r.served() == 0 {
                if p.kept_aside.enabled && r.bytes >= p.kept_aside.min_bytes {
                    Candidacy::KeptAside
                } else {
                    Candidacy::NotExamined {
                        reason: "serves no certified key; kept_aside disabled or below kept_aside.min_bytes"
                            .into(),
                    }
                }
            } else if r.bytes < c.min_rule_bytes {
                Candidacy::NotExamined {
                    reason: format!("{} bytes < candidates.min_rule_bytes", r.bytes),
                }
            } else if r.served() > c.max_keys_per_rule || !r.keys_complete {
                Candidacy::NotExamined {
                    reason: format!(
                        "serves {} certified keys > candidates.max_keys_per_rule",
                        r.served()
                    ),
                }
            } else {
                Candidacy::Examine
            }
        })
        .collect();
    let mut idx: Vec<usize> = (0..out.len())
        .filter(|&i| out[i] == Candidacy::Examine)
        .collect();
    // largest first; ties: fewer keys, then the rule id (deterministic)
    idx.sort_by(|&a, &b| {
        let (ra, rb) = (&census.rules[a], &census.rules[b]);
        rb.bytes
            .cmp(&ra.bytes)
            .then(ra.served().cmp(&rb.served()))
            .then(ra.rule.cmp(&rb.rule))
    });
    let mut keys = 0u64;
    let mut rules = 0u64;
    for i in idx {
        let s = census.rules[i].served();
        let over_rules = c.max_rules.is_some_and(|m| rules >= m);
        if over_rules || keys + s > c.max_guard_keys {
            out[i] = Candidacy::NotExamined {
                reason: if over_rules {
                    "over candidates.max_rules (largest first)".into()
                } else {
                    "over candidates.max_guard_keys (largest first)".into()
                },
            };
        } else {
            keys += s;
            rules += 1;
        }
    }
    out
}

// ------------------------------------------------------------------------------------------------------------------
// measurements

/// One bounded-recipe trial at one key (one step), as the accept chain decided it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyTrial {
    pub key: Vec<i32>,
    pub step: String,
    pub recipe_id: String,
    pub recipe_sha256: String,
    pub recipe_digest: Option<String>,
    /// Engine verdict (`SOLVED`, `NO_RELATION_IN_SEEDS`, `BUDGET_WALL`, ...).
    pub verdict: String,
    /// The accept chain certified the relation (exact proof, regularity, descent under the full comparator).
    pub certified: bool,
    pub relation_digest: Option<String>,
    /// The engine's per-key solve seconds (first use; volatile, wall).
    pub seconds: Option<f64>,
    pub rhs_terms: Option<u64>,
    /// Bytes of the relation as a `rustred.relation-entry.v1` line (what a stored relation costs).
    pub entry_bytes: Option<u64>,
    pub pole_order_d4: Option<u32>,
    /// Why not certified (failure class / refusal), if so.
    pub why: Option<String>,
    /// The bound the trial ran under ([`ShipPolicy::bound_value`]).
    pub bound: Value,
}

impl KeyTrial {
    /// A verdict that does not depend on the work / wall bound (the seed set decides): reusable under any bound.
    pub fn deterministic(&self) -> bool {
        self.certified
            || matches!(
                self.verdict.as_str(),
                "NO_RELATION_IN_SEEDS" | "NOT_IN_DOMAIN" | "NOT_LOWER"
            )
    }
}

/// The table time of one rule (load share + application per key), MEASURED (algebra crate: the record's coefficient
/// texts parsed and evaluated exactly at each key) or ESTIMATED from the cost model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableTime {
    pub measured: bool,
    /// Seconds to read and parse the record once (the load share, paid once per process).
    pub load_s: f64,
    /// Per key: seconds to apply the rule at the key.
    pub apply_s: BTreeMap<String, f64>,
    /// Per key: bytes of the instantiated relation (point relation in Q(d)), when measured.
    pub instantiated_bytes: BTreeMap<String, u64>,
    pub tool: Option<String>,
}

impl TableTime {
    pub fn estimate(p: &TableCost, bytes: u64, monomials: Option<u64>) -> TableTime {
        TableTime {
            measured: false,
            load_s: p.load_ns_per_byte * bytes as f64 * 1e-9,
            apply_s: BTreeMap::new(),
            instantiated_bytes: BTreeMap::new(),
            tool: Some(format!(
                "estimate: cost.load_ns_per_byte x bytes; apply cost.apply_ns_base + cost.apply_ns_per_monomial x {}",
                monomials.map_or("?".into(), |m| m.to_string())
            )),
        }
        .with_apply_estimate(p, monomials)
    }

    fn with_apply_estimate(mut self, p: &TableCost, monomials: Option<u64>) -> TableTime {
        let a = (p.apply_ns_base + p.apply_ns_per_monomial * monomials.unwrap_or(0) as f64) * 1e-9;
        self.apply_s.insert(String::new(), a);
        self
    }

    /// Table seconds at `key` for a rule serving `served` keys: application + the load share amortised over them.
    pub fn per_key(&self, key: &[i32], served: u64) -> f64 {
        let a = self
            .apply_s
            .get(&key_text(key))
            .or_else(|| self.apply_s.get(""))
            .copied()
            .unwrap_or(0.0);
        a + self.load_s / served.max(1) as f64
    }
}

/// `n0,n1,..` text of a key (the record maps' key).
pub fn key_text(k: &[i32]) -> String {
    k.iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

// ------------------------------------------------------------------------------------------------------------------
// guard and decision

/// The enumerated proof of a rule domain: the bounded recipe certified at EVERY certified key the rule serves.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GuardRecord {
    pub schema: String,
    pub rule: RuleRef,
    pub bound: Value,
    pub steps: Vec<String>,
    pub keys: Vec<GuardKey>,
    /// Every key certified by one of the steps.
    pub holds: bool,
    pub failed: Vec<(Vec<i32>, String)>,
    /// sha256 of the canonical record without this field.
    pub digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GuardKey {
    pub key: Vec<i32>,
    pub in_start: bool,
    pub step: String,
    pub recipe_id: String,
    pub recipe_sha256: String,
    pub recipe_digest: Option<String>,
    pub relation_digest: Option<String>,
    pub seconds: Option<f64>,
    pub rhs_terms: Option<u64>,
    pub entry_bytes: Option<u64>,
    pub pole_order_d4: Option<u32>,
}

/// The guard of a rule from its trials: per key the FIRST step (policy order) whose trial certified.
pub fn guard(p: &ShipPolicy, svc: &RuleService, trials: &[KeyTrial]) -> GuardRecord {
    let mut keys = vec![];
    let mut failed = vec![];
    for (k, in_start) in &svc.keys {
        let found = p.recipe.steps.iter().find_map(|s| {
            trials
                .iter()
                .find(|t| &t.key == k && &t.step == s && t.certified)
        });
        match found {
            Some(t) => keys.push(GuardKey {
                key: k.clone(),
                in_start: *in_start,
                step: t.step.clone(),
                recipe_id: t.recipe_id.clone(),
                recipe_sha256: t.recipe_sha256.clone(),
                recipe_digest: t.recipe_digest.clone(),
                relation_digest: t.relation_digest.clone(),
                seconds: t.seconds,
                rhs_terms: t.rhs_terms,
                entry_bytes: t.entry_bytes,
                pole_order_d4: t.pole_order_d4,
            }),
            None => {
                let why: Vec<String> = p
                    .recipe
                    .steps
                    .iter()
                    .map(|s| {
                        let v = trials
                            .iter()
                            .find(|t| &t.key == k && &t.step == s)
                            .map_or("not_tried".to_string(), |t| {
                                t.why.clone().unwrap_or_else(|| t.verdict.clone())
                            });
                        format!("{s}: {v}")
                    })
                    .collect();
                failed.push((k.clone(), why.join("; ")));
            }
        }
    }
    let holds = failed.is_empty() && svc.keys_complete && !svc.keys.is_empty();
    let mut g = GuardRecord {
        schema: GUARD_SCHEMA.into(),
        rule: svc.rule.clone(),
        bound: p.bound_value(),
        steps: p.recipe.steps.clone(),
        keys,
        holds,
        failed,
        digest: String::new(),
    };
    g.digest = digest_of(&serde_json::to_value(&g).unwrap_or(Value::Null));
    g
}

/// Typed outcome of one rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Every certified key served by the bounded recipe (shipped as recipe records with certified digests).
    Recipe,
    /// Recipe at the keys within the bars, stored relations at the others.
    Mixed,
    /// Every certified key served by a stored relation (the certified recipe relation shipped as an entry).
    StoredRelations,
    /// The rule stays in the shipped table (`why` says which test kept it).
    Table,
    /// Serves no certified key: moved to the generated data kept aside.
    KeptAside,
    /// Not examined in this run (`why`).
    NotExamined,
}

impl Outcome {
    pub fn migrates(self) -> bool {
        matches!(
            self,
            Outcome::Recipe | Outcome::Mixed | Outcome::StoredRelations | Outcome::KeptAside
        )
    }
}

/// What serves one key after the decision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyChoice {
    pub key: Vec<i32>,
    pub in_start: bool,
    /// `recipe` | `stored` | `table`.
    pub form: String,
    pub step: Option<String>,
    pub recipe_id: Option<String>,
    pub relation_digest: Option<String>,
    pub recipe_seconds: Option<f64>,
    pub table_seconds: Option<f64>,
    pub ratio_recipe_to_table: Option<f64>,
    pub bytes: u64,
}

/// The decision of one rule, with every number that decided it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuleDecision {
    pub rule: RuleRef,
    pub outcome: Outcome,
    pub why: String,
    pub bytes_before: u64,
    pub bytes_after: u64,
    pub bytes_saved: i64,
    /// Summed first-use recipe seconds of the keys served by the recipe.
    pub seconds_added_total: f64,
    pub max_recipe_seconds_per_key: Option<f64>,
    pub bytes_saved_per_second: Option<f64>,
    pub table_time_measured: bool,
    pub served_in_start: u64,
    pub served_outside_start: u64,
    pub keys: Vec<KeyChoice>,
    pub guard_digest: Option<String>,
    pub guard_holds: Option<bool>,
}

/// Bytes a shipped recipe record costs per key it carries (key + certified relation digest in the record), and per
/// record (the recipe JSON; the caller passes the measured size when it has the file).
pub const RECIPE_BYTES_PER_KEY: u64 = 160;

/// Decide one EXAMINED rule from its guard, its table time and the recipe files' sizes (`recipe_bytes`: bytes of each
/// distinct recipe file the guard's keys use, by recipe id).
pub fn decide(
    p: &ShipPolicy,
    svc: &RuleService,
    g: &GuardRecord,
    tt: &TableTime,
    recipe_bytes: &BTreeMap<String, u64>,
) -> RuleDecision {
    let served = svc.served();
    let base = |outcome: Outcome, why: String, keys: Vec<KeyChoice>| RuleDecision {
        rule: svc.rule.clone(),
        outcome,
        why,
        bytes_before: svc.bytes,
        bytes_after: svc.bytes,
        bytes_saved: 0,
        seconds_added_total: 0.0,
        max_recipe_seconds_per_key: None,
        bytes_saved_per_second: None,
        table_time_measured: tt.measured,
        served_in_start: svc.served_in_start,
        served_outside_start: svc.served_outside_start,
        keys,
        guard_digest: Some(g.digest.clone()),
        guard_holds: Some(g.holds),
    };
    if !g.holds {
        return base(
            Outcome::Table,
            format!(
                "guard: {} of {} certified keys without a relation under the bound ({})",
                g.failed.len(),
                svc.keys.len(),
                g.failed
                    .first()
                    .map(|(k, w)| format!("{k:?}: {w}"))
                    .unwrap_or_default()
            ),
            vec![],
        );
    }
    // per key: recipe if within the bars, else stored if enabled and measured, else the rule stays
    let mut keys = vec![];
    let mut stuck: Option<String> = None;
    for gk in &g.keys {
        let ts = tt.per_key(&gk.key, served);
        let rs = gk.seconds;
        let ratio = rs.map(|s| if ts > 0.0 { s / ts } else { f64::INFINITY });
        let within_abs = rs.is_some_and(|s| s <= p.bars.max_recipe_seconds_per_key);
        let within_rel = match (p.bars.max_recipe_to_table_ratio, ratio) {
            (None, _) => true,
            (Some(m), Some(r)) => r <= m,
            (Some(_), None) => false,
        };
        let (form, bytes) = if within_abs && within_rel {
            ("recipe", RECIPE_BYTES_PER_KEY)
        } else if p.stored.enabled && gk.entry_bytes.is_some() {
            ("stored", gk.entry_bytes.unwrap_or(0))
        } else {
            stuck.get_or_insert_with(|| {
                format!(
                    "key {:?}: recipe {:?} s over the bars and no stored relation (stored.enabled = {})",
                    gk.key, rs, p.stored.enabled
                )
            });
            ("table", 0)
        };
        keys.push(KeyChoice {
            key: gk.key.clone(),
            in_start: gk.in_start,
            form: form.into(),
            step: Some(gk.step.clone()),
            recipe_id: Some(gk.recipe_id.clone()),
            relation_digest: gk.relation_digest.clone(),
            recipe_seconds: rs,
            table_seconds: Some(ts),
            ratio_recipe_to_table: ratio,
            bytes,
        });
    }
    if let Some(why) = stuck {
        return base(Outcome::Table, why, keys);
    }
    let recipe_ids: BTreeSet<&String> = keys
        .iter()
        .filter(|k| k.form == "recipe")
        .filter_map(|k| k.recipe_id.as_ref())
        .collect();
    let files: u64 = recipe_ids
        .iter()
        .map(|id| recipe_bytes.get(*id).copied().unwrap_or(2048))
        .sum();
    let after: u64 = keys.iter().map(|k| k.bytes).sum::<u64>() + files;
    let secs: f64 = keys
        .iter()
        .filter(|k| k.form == "recipe")
        .filter_map(|k| k.recipe_seconds)
        .sum();
    let max_s = keys
        .iter()
        .filter(|k| k.form == "recipe")
        .filter_map(|k| k.recipe_seconds)
        .fold(None, |m: Option<f64>, s| Some(m.map_or(s, |m| m.max(s))));
    let saved = svc.bytes as i64 - after as i64;
    let per_s = if secs > 0.0 {
        Some(saved as f64 / secs)
    } else {
        None
    };
    let n_recipe = keys.iter().filter(|k| k.form == "recipe").count();
    let n_stored = keys.len() - n_recipe;
    let stored_bytes: u64 = keys
        .iter()
        .filter(|k| k.form == "stored")
        .map(|k| k.bytes)
        .sum();
    let mut d = base(Outcome::Table, String::new(), keys);
    d.bytes_after = after;
    d.bytes_saved = saved;
    d.seconds_added_total = secs;
    d.max_recipe_seconds_per_key = max_s;
    d.bytes_saved_per_second = per_s;
    let fail = if saved < 0 || (saved as u64) < p.bars.min_bytes_saved {
        Some(format!(
            "saves {saved} bytes < bars.min_bytes_saved {}",
            p.bars.min_bytes_saved
        ))
    } else if per_s.is_some_and(|x| x < p.bars.min_bytes_saved_per_second) {
        Some(format!(
            "saves {:.0} bytes per second added < bars.min_bytes_saved_per_second {:.0}",
            per_s.unwrap_or(0.0),
            p.bars.min_bytes_saved_per_second
        ))
    } else if n_stored > 0
        && stored_bytes as f64 > p.stored.max_bytes_ratio * svc.bytes as f64
    {
        Some(format!(
            "stored relations {stored_bytes} bytes > stored.max_bytes_ratio x {} rule bytes",
            svc.bytes
        ))
    } else {
        None
    };
    match fail {
        Some(why) => {
            d.why = why;
            d.bytes_after = svc.bytes;
            d.bytes_saved = 0;
        }
        None => {
            d.outcome = match (n_recipe, n_stored) {
                (_, 0) => Outcome::Recipe,
                (0, _) => Outcome::StoredRelations,
                _ => Outcome::Mixed,
            };
            d.why = format!(
                "guard holds at {} keys; {n_recipe} recipe ({secs:.2} s first use in all), {n_stored} stored; saves {saved} bytes",
                g.keys.len()
            );
        }
    }
    d
}

/// The decision of a rule that is not examined (kept aside or not examined).
pub fn decide_unexamined(svc: &RuleService, c: &Candidacy) -> RuleDecision {
    let (outcome, why, after) = match c {
        Candidacy::KeptAside => (
            Outcome::KeptAside,
            "serves no certified key: moved to the generated data kept aside (no certified key's first match moves)"
                .to_string(),
            0,
        ),
        Candidacy::NotExamined { reason } => (Outcome::NotExamined, reason.clone(), svc.bytes),
        Candidacy::Examine => (
            Outcome::NotExamined,
            "examined without measurements".into(),
            svc.bytes,
        ),
    };
    RuleDecision {
        rule: svc.rule.clone(),
        outcome,
        why,
        bytes_before: svc.bytes,
        bytes_after: after,
        bytes_saved: svc.bytes as i64 - after as i64,
        seconds_added_total: 0.0,
        max_recipe_seconds_per_key: None,
        bytes_saved_per_second: None,
        table_time_measured: false,
        served_in_start: svc.served_in_start,
        served_outside_start: svc.served_outside_start,
        keys: vec![],
        guard_digest: None,
        guard_holds: None,
    }
}

/// Revert the decision of a rule to TABLE (the closing check of the shipped spec failed behind it).
pub fn revert(d: &mut RuleDecision, why: &str) {
    d.outcome = Outcome::Table;
    d.why = format!("reverted by the closing check: {why} (was: {})", d.why);
    d.bytes_after = d.bytes_before;
    d.bytes_saved = 0;
    d.seconds_added_total = 0.0;
    for k in &mut d.keys {
        k.form = "table".into();
        k.bytes = 0;
    }
}

// ------------------------------------------------------------------------------------------------------------------
// the iterable record (shared family with genchoice)

/// What a later run may still try (iteration: "a deeper bounded search", "other centres", "a higher bar").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NotTried {
    pub what: String,
    pub why: String,
}

/// One rule's economy record (`rustred.campaign.economy-record.v1`, kind `ship-rule`): inputs, settings, every
/// measurement, the decision and what was not tried. A run with the same `inputs_digest` and `decision_digest` does
/// nothing for the rule; a run with other settings re-decides from `trials` / `table_time` and measures only what is new.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EconomyRecord {
    pub schema: String,
    pub kind: String,
    pub node: String,
    pub subject: RuleRef,
    pub inputs_digest: String,
    pub decision_digest: String,
    pub policy_digest: String,
    pub trials: Vec<KeyTrial>,
    pub table_time: Option<TableTime>,
    pub decision: RuleDecision,
    pub guard: Option<GuardRecord>,
    pub not_tried: Vec<NotTried>,
    /// How to revert: the rule is in the kept table (path / sha256 of the original source table).
    pub revert: Value,
}

/// Digest of what a rule's decision depends on besides the settings: its record, its table and its served keys.
pub fn inputs_digest(svc: &RuleService) -> String {
    digest_of(&json!({"rule": svc.rule, "bytes": svc.bytes, "in": svc.served_in_start,
        "out": svc.served_outside_start, "keys": svc.keys, "complete": svc.keys_complete}))
}

/// What a run has to do for one examined rule given its previous record.
#[derive(Clone, Debug, PartialEq)]
pub enum Reuse {
    /// Same inputs and same decision settings: nothing to do (the previous decision stands).
    Unchanged,
    /// Re-decide from the recorded measurements; measure only these (step, key) trials (may be empty).
    Continue { trials: Vec<KeyTrial>, to_try: Vec<(String, Vec<i32>)> },
    /// No usable record: measure everything the guard needs.
    Fresh,
}

/// Plan one rule: which recorded trials are reusable under `p`, and which (step, key) trials the guard still needs
/// (first-certified-step semantics: a key with a certified earlier step needs no later step).
pub fn plan(p: &ShipPolicy, svc: &RuleService, prev: Option<&EconomyRecord>) -> Reuse {
    let Some(prev) = prev else {
        return Reuse::Fresh;
    };
    let ind = inputs_digest(svc);
    if prev.inputs_digest != ind {
        return Reuse::Fresh;
    }
    if prev.decision_digest == p.decision_digest() {
        return Reuse::Unchanged;
    }
    let bound = p.bound_value();
    let usable: Vec<KeyTrial> = prev
        .trials
        .iter()
        .filter(|t| t.deterministic() || t.bound == bound)
        .cloned()
        .collect();
    Reuse::Continue {
        to_try: missing_trials(p, svc, &usable),
        trials: usable,
    }
}

/// The (step, key) trials still needed: per key, the steps in order up to the first certified one, skipping those
/// already decided in `have`.
pub fn missing_trials(
    p: &ShipPolicy,
    svc: &RuleService,
    have: &[KeyTrial],
) -> Vec<(String, Vec<i32>)> {
    let mut out = vec![];
    for (k, _) in &svc.keys {
        for s in &p.recipe.steps {
            match have.iter().find(|t| &t.key == k && &t.step == s) {
                Some(t) if t.certified => break,
                Some(_) => continue,
                None => {
                    out.push((s.clone(), k.clone()));
                    break;
                }
            }
        }
    }
    out
}

/// The not-tried list of a rule under `p` (what a later run could change).
pub fn not_tried(p: &ShipPolicy, d: &RuleDecision, g: Option<&GuardRecord>) -> Vec<NotTried> {
    let mut v = vec![];
    if let Some(g) = g
        && !g.holds
    {
        v.push(NotTried {
            what: "deeper or other bounded steps (depth 3, other centres, symmetry rows, a seed list minimised per key)"
                .into(),
            why: format!(
                "the guard failed at {} keys under steps {:?}",
                g.failed.len(),
                p.recipe.steps
            ),
        });
        if p.recipe.max_trace_rows.is_some() || p.recipe.key_budget_s.is_some() {
            v.push(NotTried {
                what: "a larger work / wall bound".into(),
                why: format!("bound {}", p.bound_value()),
            });
        }
    }
    if d.outcome == Outcome::Table && d.keys.iter().any(|k| k.form == "table") {
        v.push(NotTried {
            what: "a higher bars.max_recipe_seconds_per_key or stored relations".into(),
            why: d.why.clone(),
        });
    }
    v.push(NotTried {
        what: "stored relation from the TABLE RULE instantiated at the key (same right-hand side as the table)".into(),
        why: "no verification route for imported table-rule instantiations in the import (supplied relations are \
              served only when verified at load)"
            .into(),
    });
    v
}

// ------------------------------------------------------------------------------------------------------------------
// the shipped spec (closing check emulation of the engine's dispatch)

/// One recipe source of the shipped spec: a `rustred.recipe.v1` file with `domain.keys` (its certified keys).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShippedRecipe {
    pub id: String,
    pub path: String,
    pub sha256: String,
    /// `recipe` (served by the recipe at run time) | `stored` (the same certified relations shipped as entries).
    pub form: String,
    /// The per-key assignment side file (import.md 13.5): the recipe certifies ONLY these keys.
    pub assignment: Option<FileRef>,
}

/// The shipped spec: the original sources with the migrated and kept-aside rules REMOVED (reason recorded), then the
/// recipe / stored-relation sets at the END (relation sets after every table = the engine's order: table first match,
/// then recipe records, then shipped entries, then the typed refusal; a removed rule's key falls through to a LATER
/// table rule first, exactly as in the engine), and the guard's solved relations as evidence.
pub fn shipped_spec(
    orig: &TableSetSpec,
    decisions: &[RuleDecision],
    recipes: &[ShippedRecipe],
    solved: &[FileRef],
) -> Result<TableSetSpec, String> {
    let mut s = orig.clone();
    let ids: BTreeSet<String> = s.sources.iter().map(|x| x.id.clone()).collect();
    for d in decisions.iter().filter(|d| d.outcome.migrates()) {
        if !ids.contains(&d.rule.source) {
            return Err(format!(
                "rule {} names source {:?} not in the spec",
                d.rule.id(),
                d.rule.source
            ));
        }
    }
    for src in &mut s.sources {
        if !matches!(src.kind, SourceKind::EngineTable | SourceKind::TableSet) {
            continue;
        }
        let have: BTreeSet<i64> = src.removed.iter().map(|r| r.label).collect();
        for d in decisions
            .iter()
            .filter(|d| d.outcome.migrates() && d.rule.source == src.id)
        {
            if have.contains(&d.rule.label) {
                continue;
            }
            src.removed.push(Removed {
                label: d.rule.label,
                reason: format!("shipecon: {:?}: {}", d.outcome, d.why),
            });
        }
        src.removed.sort_by_key(|r| r.label);
    }
    for r in recipes {
        if ids.contains(&r.id) {
            return Err(format!("recipe source id {} already in the spec", r.id));
        }
        let mut sv = json!({
            "id": r.id, "kind": "recipe", "class": format!("shipecon-{}", r.form),
            "path": r.path, "sha256": r.sha256});
        if let Some(a) = &r.assignment {
            sv["assignment"] = serde_json::to_value(a).map_err(|e| e.to_string())?;
        }
        let src: SourceSpec = serde_json::from_value(sv)
        .map_err(|e| format!("recipe source {}: {e}", r.id))?;
        s.sources.push(src);
    }
    s.evidence.solved.extend(solved.iter().cloned());
    Ok(s)
}

/// The closing check of the shipped spec against the statement it replaces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClosingCheck {
    /// Certified closed (no blocking word) on the shipped spec.
    pub closed: bool,
    pub blocking: Vec<String>,
    pub same_start: bool,
    pub same_terminals: bool,
    pub terminals_before: Vec<Vec<i32>>,
    pub terminals_after: Vec<Vec<i32>>,
    pub start_keys: u64,
    pub closure_before: u64,
    pub closure_after: u64,
    /// Keys in the new closure that were not in the old one / old keys no longer reached.
    pub new_keys: u64,
    pub dropped_keys: u64,
    pub open_after: u64,
    /// The verdict: closed AND same start AND same terminals.
    pub holds: bool,
}

/// Summary of one closure for [`ClosingCheck::compare`] (keys sorted).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClosureView {
    pub keys: Vec<Vec<i32>>,
    pub start: Vec<Vec<i32>>,
    pub terminals: Vec<Vec<i32>>,
    pub open: u64,
    pub blocking: Vec<String>,
}

impl ClosingCheck {
    pub fn compare(before: &ClosureView, after: &ClosureView) -> ClosingCheck {
        let b: BTreeSet<&Vec<i32>> = before.keys.iter().collect();
        let a: BTreeSet<&Vec<i32>> = after.keys.iter().collect();
        let mut tb = before.terminals.clone();
        let mut ta = after.terminals.clone();
        tb.sort();
        ta.sort();
        let mut sb = before.start.clone();
        let mut sa = after.start.clone();
        sb.sort();
        sa.sort();
        let closed = after.blocking.is_empty() && after.open == 0;
        let same_start = sb == sa;
        let same_terminals = tb == ta;
        ClosingCheck {
            closed,
            blocking: after.blocking.clone(),
            same_start,
            same_terminals,
            terminals_before: tb,
            terminals_after: ta,
            start_keys: sa.len() as u64,
            closure_before: b.len() as u64,
            closure_after: a.len() as u64,
            new_keys: a.difference(&b).count() as u64,
            dropped_keys: b.difference(&a).count() as u64,
            open_after: after.open,
            holds: closed && same_start && same_terminals,
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------
// report

/// Totals of a run (bytes before / after over the decisions, seconds added per key, guard failures).
pub fn totals(census: &Census, decisions: &[RuleDecision]) -> Value {
    let mut by: BTreeMap<String, (u64, u64, i64)> = BTreeMap::new();
    let mut secs: Vec<f64> = vec![];
    let mut guard_failed = 0u64;
    let mut guard_failed_keys = 0u64;
    for d in decisions {
        let e = by
            .entry(format!("{:?}", d.outcome).to_lowercase())
            .or_default();
        e.0 += 1;
        e.1 += d.bytes_before;
        e.2 += d.bytes_saved;
        if d.outcome.migrates() {
            secs.extend(
                d.keys
                    .iter()
                    .filter(|k| k.form == "recipe")
                    .filter_map(|k| k.recipe_seconds),
            );
        }
        if d.guard_holds == Some(false) {
            guard_failed += 1;
            guard_failed_keys += d.keys.iter().filter(|k| k.form == "table").count() as u64;
        }
    }
    let saved: i64 = decisions
        .iter()
        .filter(|d| d.outcome.migrates())
        .map(|d| d.bytes_saved)
        .sum();
    secs.sort_by(|a, b| a.total_cmp(b));
    let q = |f: f64| -> Value {
        if secs.is_empty() {
            Value::Null
        } else {
            json!(secs[((secs.len() - 1) as f64 * f).round() as usize])
        }
    };
    json!({
        "bytes_before": census.total_bytes,
        "bytes_after": census.total_bytes as i64 - saved,
        "bytes_saved": saved,
        "by_outcome": by.iter().map(|(k, (n, b, s))| (k.clone(), json!({"rules": n, "bytes": b, "saved": s})))
            .collect::<serde_json::Map<String, Value>>(),
        "recipe_seconds_per_key": {"keys": secs.len(), "min": q(0.0), "median": q(0.5), "p90": q(0.9), "max": q(1.0),
                                   "sum": secs.iter().sum::<f64>()},
        "guard_failed_rules": guard_failed,
        "guard_failed_keys_reported": guard_failed_keys,
    })
}
