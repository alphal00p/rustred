//! Identity evidence at the served keys (check 5 of the chain), two back-ends behind [`EvidenceProver`]:
//! - [`ReplayProver`] (level `replay` = A2, in crate): `ibp_rows::replay_evidence` for EXACTLY the served key set,
//!   identity replayed at EVERY served key (`max_identity_keys = 0`), behind the one adapter [`replay_adapter`].
//! - [`FilePerKeyProver`] (level `per-key` = A3): writes the request for `rr_proof --mode instance-span`, reads its
//!   per-key statuses (never the tool-level verdict); typed `Unavailable` where the tool cannot answer today.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;
use serde_json::{Value, json};

use super::RuleSource;
use super::facts::{AnswerBinding, check_echo};
use super::rule::{RuleText, WitnessStatus, sha256_tag};
use crate::ibp_rows::{
    PointSpec, PointStatus, ReplayLevel, ReplayPolicy, RowsOptions, RowsRule, TableBinding,
};
use crate::key::Key;
use crate::poly::Poly;
use crate::replay_base::family::Family;

/// Evidence levels the chain knows (ordered: `PerKey` satisfies a `Replay` requirement, never the reverse).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Evidence {
    /// A1: d1-d4 at the served keys (computed by the chain itself; never enough to accept).
    Geometry,
    /// IMPORTED evidence (a `rustred.campaign.accept.v1` provenance line of an imported table, producer unknown or
    /// known): never above this, whatever label it carries; satisfies no required level (addendum 3).
    Imported,
    /// A2 `identity_tested`: identity replay (ReplayRecord Established on exactly the served set).
    Replay,
    /// A3 `identity_proved`: exact per-key proof at exactly the served keys.
    PerKey,
}

impl Evidence {
    /// The ledger's word (`EvidenceLevel`).
    pub fn ledger_word(self) -> &'static str {
        match self {
            Evidence::Geometry => "geometry_accepted",
            Evidence::Imported => "imported",
            Evidence::Replay => "identity_tested",
            Evidence::PerKey => "identity_proved",
        }
    }
    /// The plan's level name.
    pub fn plan_level(self) -> &'static str {
        match self {
            Evidence::Geometry => "A1",
            Evidence::Imported => "A0-imported",
            Evidence::Replay => "A2",
            Evidence::PerKey => "A3",
        }
    }
}

/// The per-node spec value (R40 U-12, DEV-7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RequiredEvidence {
    Replay,
    PerKey,
}

impl RequiredEvidence {
    pub fn satisfied_by(self, e: Evidence) -> bool {
        match self {
            RequiredEvidence::Replay => e >= Evidence::Replay,
            RequiredEvidence::PerKey => e >= Evidence::PerKey,
        }
    }
    pub fn as_evidence(self) -> Evidence {
        match self {
            RequiredEvidence::Replay => Evidence::Replay,
            RequiredEvidence::PerKey => Evidence::PerKey,
        }
    }
}

/// A key-level identity failure with its stored disproof (plan 2.5.3: FAIL only with a disproof).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyFailure {
    /// Not in the span of the rows of the recorded seeds at the key (NON-MEMBERSHIP / FAIL_AT_KEY).
    NonMembership { disproof: String },
    /// An RHS denominator vanishes identically in d at the key.
    Irregular { detail: String },
    /// A hidden weight pole at the key.
    Hidden { detail: String },
}

/// The prover's status at one served key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyStatus {
    Reached(Evidence),
    Failed(KeyFailure),
    /// The tool says the key is not in the case / chart / on an exception branch (d1 decided otherwise: an error).
    NotCredited(String),
    Unsupported(String),
    Unknown(String),
}

/// The artefact a level cites (the ledger's `LevelEvidence` points at the file the caller writes from `body`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EvidenceArtefact {
    pub schema: String,
    /// `sha256:` of the RULE LINE this artefact is evidence for (audit-accept-B M1: an artefact names its rule; the
    /// chain refuses an artefact of another rule).
    pub rule_sha256: String,
    /// `sha256:` of the canonical JSON of `body` (timing fields removed), or of the answer text for file back-ends.
    pub digest: String,
    /// Digest of the key set the artefact covers (`closure::KeySet` canonical digest).
    pub keys_digest: String,
    pub tool: String,
    /// The artefact itself when produced in process (replay record); `None` for file back-ends (the answer file is
    /// the artefact).
    #[serde(skip)]
    pub body: Option<Value>,
}

/// A request file the caller writes before running an external tool as a child job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ExternalRequest {
    /// "rr_proof instance-span", "rr_proof instantiate".
    pub tool: String,
    /// Suggested file name (relative).
    pub path_hint: String,
    /// The file content (JSONL).
    pub text: String,
    /// The suggested argv tail of the tool (the supervisor substitutes its own paths).
    pub argv: Vec<String>,
}

/// What a prover answers for (rule, served keys).
#[derive(Clone, Debug, PartialEq)]
pub enum ProverAnswer {
    PerKey {
        keys: Vec<(Key, KeyStatus)>,
        artefact: Option<EvidenceArtefact>,
        /// A failure that concerns the RULE, not a key (identity Fail at a generic point, family refused, loci
        /// untested): every key is then not reached.
        rule_level: Option<RuleLevel>,
    },
    NeedsExternal(Vec<ExternalRequest>),
    /// The back-end cannot answer for this rule at all (typed reason).
    Unavailable(String),
}

/// A rule-level evidence failure. `disproof` = a stored disproof of the rule as written (identity Fail at a generic
/// point at both primes: not derived from its recorded seeds on its case); else a reason why nothing is established.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RuleLevel {
    pub disproof: bool,
    pub text: String,
}

/// Identity evidence at the served keys.
pub trait EvidenceProver {
    fn level(&self) -> Evidence;
    fn prove(&mut self, rule: &RuleText, source: &RuleSource, served: &[Key]) -> ProverAnswer;
}

/// Canonical digest of a served key set (sorted, deduplicated; `closure::KeySet` digest rule, closure.md s.4).
pub fn keys_digest(arity: usize, keys: &[Key]) -> String {
    let mut v: Vec<&Key> = keys.iter().collect();
    v.sort();
    v.dedup();
    let mut d = crate::closure::keyset::DigestStream::new(arity);
    for k in v {
        d.key(k);
    }
    d.finish()
}

fn strip_timing(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.remove("seconds");
            for x in m.values_mut() {
                strip_timing(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(strip_timing),
        _ => {}
    }
}

/// `sha256:` of the canonical JSON of a serialisable artefact with its timing fields removed.
pub fn artefact_digest(v: &Value) -> (Value, String) {
    let mut v = v.clone();
    strip_timing(&mut v);
    let text = crate::schema::canon::canonical_json(&v, true).unwrap_or_default();
    (v, sha256_tag(text.as_bytes()))
}

/// The ONE adapter onto the identity test (I-ROWS): when `replay_evidence` v2 (rows2) lands, only this module
/// changes.
pub mod replay_adapter {
    use super::*;

    /// Run `replay` for exactly `served` and map it per key.
    pub fn run(
        family: &Family,
        rule: &RuleText,
        served: &[Key],
        policy: &ReplayPolicy,
        opts: &RowsOptions,
    ) -> ProverAnswer {
        let arity = rule.record.fixed.len();
        let rr = match RowsRule::from_value(rule.value.clone(), arity) {
            Ok(r) => r,
            Err(e) => {
                return ProverAnswer::Unavailable(format!("identity test refuses the record: {e}"));
            }
        };
        let keys64: Vec<Vec<i64>> = served
            .iter()
            .map(|k| k.iter().map(|&x| i64::from(x)).collect())
            .collect();
        let mut pol = policy.clone();
        pol.max_identity_keys = 0; // EVERY served key (ibp-rows.md s.9 item 2); a sample is never accepted
        let rec = crate::ibp_rows::replay_evidence(family, &rr, &keys64, &pol, opts);
        let value = serde_json::to_value(&rec).unwrap_or(Value::Null);
        let (body, digest) = artefact_digest(&value);
        let artefact = EvidenceArtefact {
            schema: rec.schema.to_string(),
            rule_sha256: rule.line_sha256.clone(),
            digest,
            keys_digest: keys_digest(arity, served),
            tool: rec.tool_version.to_string(),
            body: Some(body),
        };
        let served_set: BTreeMap<Vec<i64>, usize> = keys64
            .iter()
            .enumerate()
            .map(|(i, k)| (k.clone(), i))
            .collect();
        let mut status: Vec<Option<KeyStatus>> = vec![None; served.len()];
        let mut rule_level: Vec<String> = Vec::new();
        let mut disproof = false;
        for p in &rec.identity.points {
            let at = match &p.spec {
                PointSpec::Key(k) => served_set.get(k).copied(),
                PointSpec::Generic => None,
            };
            let bad = match &p.status {
                PointStatus::Pass => None,
                PointStatus::Fail => Some(KeyStatus::Failed(KeyFailure::NonMembership {
                    disproof: p
                        .cells
                        .iter()
                        .find(|c| !c.in_span)
                        .map(|c| {
                            format!(
                                "prime {} d {} column {}",
                                c.prime,
                                c.d,
                                c.witness.clone().unwrap_or_default()
                            )
                        })
                        .unwrap_or_else(|| "not in span".into()),
                })),
                PointStatus::Pole { coefficient } => {
                    Some(KeyStatus::Failed(KeyFailure::Irregular {
                        detail: format!("identity test: pole of coefficient {coefficient}"),
                    }))
                }
                other => Some(KeyStatus::Unknown(format!("identity point: {other:?}"))),
            };
            match (at, bad) {
                (_, None) => {}
                (Some(i), Some(s)) => status[i] = Some(s),
                (None, Some(s)) => {
                    disproof |= p.status == PointStatus::Fail && p.spec == PointSpec::Generic;
                    rule_level.push(format!("{:?}: {s:?}", p.spec))
                }
            }
        }
        for h in &rec.pole_scan.poles {
            if let Some(&i) = served_set.get(&h.key) {
                status[i] = Some(KeyStatus::Failed(KeyFailure::Irregular {
                    detail: format!("pole scan: coefficient {}", h.coefficient),
                }));
            }
        }
        for (k, why) in &rec.pole_scan.undecided {
            if let Some(&i) = served_set.get(k) {
                status[i].get_or_insert(KeyStatus::Unknown(format!("pole scan: {why}")));
            }
        }
        if rec.pole_scan.keys_applicable != served.len() {
            rule_level.push(format!(
                "identity test finds {} of {} served keys applicable (domain disagreement)",
                rec.pole_scan.keys_applicable,
                served.len()
            ));
        }
        let established = rec.level == ReplayLevel::Established;
        let key_failures = status.iter().any(Option::is_some);
        if !established && !key_failures && rule_level.is_empty() {
            match &rec.level {
                ReplayLevel::NotEstablished { reasons } => rule_level.extend(reasons.iter().cloned()),
                // rows3: a record without a family binding, of a diagnostic backend, with caller overrides or a
                // sampled replay is never the level (named, so the chain says why)
                other => rule_level.push(format!("replay level {other:?} is not Established")),
            }
        }
        let keys = served
            .iter()
            .zip(status)
            .map(|(k, s)| {
                let s = match s {
                    Some(s) => s,
                    None if established && rule_level.is_empty() => {
                        KeyStatus::Reached(Evidence::Replay)
                    }
                    None if rule_level.is_empty() => {
                        KeyStatus::Unknown("replay not established on the whole served set".into())
                    }
                    None => KeyStatus::Unknown(rule_level.join("; ")),
                };
                (k.clone(), s)
            })
            .collect();
        ProverAnswer::PerKey {
            keys,
            artefact: Some(artefact),
            rule_level: (!rule_level.is_empty()).then(|| RuleLevel {
                disproof,
                text: rule_level.join("; "),
            }),
        }
    }
}

/// Level `replay` in process (the identity test of this crate). The family binding is MANDATORY (rows3 NEW-M1: a replay
/// without it is `Unbound`, never the level): [`ReplayProver::new`] takes it, and a prover whose options lost it answers
/// `Unavailable` (nothing is ever certified at `Unbound`).
pub struct ReplayProver {
    pub family: Family,
    pub policy: ReplayPolicy,
    opts: RowsOptions,
}

impl ReplayProver {
    /// `binding` = the node table's header (`TableBinding::from_header`, family_source compared with the family text)
    /// or the campaign spec's family identity (`TableBinding::from_family_digest`); it replaces `opts.table`.
    pub fn new(family: Family, binding: TableBinding, mut opts: RowsOptions) -> Self {
        opts.table = Some(binding);
        ReplayProver {
            family,
            policy: ReplayPolicy::default(),
            opts,
        }
    }
    pub fn opts(&self) -> &RowsOptions {
        &self.opts
    }
}

impl EvidenceProver for ReplayProver {
    fn level(&self) -> Evidence {
        Evidence::Replay
    }
    fn prove(&mut self, rule: &RuleText, _source: &RuleSource, served: &[Key]) -> ProverAnswer {
        if self.opts.table.is_none() {
            return ProverAnswer::Unavailable(
                "replay without a family binding (RowsOptions.table) would be Unbound".into(),
            );
        }
        replay_adapter::run(&self.family, rule, served, &self.policy, &self.opts)
    }
}

/// Schema of the per-key prover request lines.
pub const INSTANCE_SPAN_REQUEST_SCHEMA: &str = "rustred.campaign.instance-span-request.v1";
/// Schema of rr_proof's instance-span answer line.
pub const SPAN_ANSWER_SCHEMA: &str = "rustred.proof-kernel.span.v1";

#[derive(Clone, Debug)]
struct SpanAnswer {
    per_key: BTreeMap<Vec<i32>, String>,
    refusal: Option<String>,
    digest: String,
    binding: AnswerBinding,
}

/// Level `per-key` through the file-backed `rr_proof --mode instance-span` (one child job per rule).
#[derive(Clone, Debug, Default)]
pub struct FilePerKeyProver {
    /// Arities the tool was built for (DATA: e.g. the std build of 2026-11-14 is arity-generic or 20-only).
    pub arities: Vec<usize>,
    /// Certificate / ROWS file passed as `--cert` (rows `cert`), if any.
    pub cert: Option<String>,
    /// How an answer file must name its rule (default [`AnswerBinding::DigestEcho`]).
    pub binding: AnswerBinding,
    answers: HashMap<String, SpanAnswer>,
}

/// Parse a key of an rr_proof answer: an array of the full key, or an object of slots `{"n<i>": v}` completed from the
/// record's fixed slots.
fn answer_key(v: &Value, fixed: &[Option<i64>]) -> Option<Vec<i32>> {
    match v {
        Value::Array(a) => a
            .iter()
            .map(|x| x.as_i64().and_then(|y| i32::try_from(y).ok()))
            .collect(),
        Value::Object(m) => {
            let mut k: Vec<Option<i32>> = fixed
                .iter()
                .map(|f| f.and_then(|x| i32::try_from(x).ok()))
                .collect();
            for (name, x) in m {
                let i: usize = name.strip_prefix('n')?.parse().ok()?;
                *k.get_mut(i)? = Some(i32::try_from(x.as_i64()?).ok()?);
            }
            k.into_iter().collect()
        }
        _ => None,
    }
}

fn unit_pivot(eq: &str, arity: usize) -> Result<bool, String> {
    let p = Poly::parse(eq, arity, false).map_err(|e| format!("{e:?}"))?;
    let mut linear = true;
    let mut unit = false;
    for (m, c) in p.terms() {
        match m.as_slice() {
            [] => {}
            [(_, 1)] => unit |= *c == 1 || *c == -1,
            _ => linear = false,
        }
    }
    Ok(linear && unit)
}

impl FilePerKeyProver {
    pub fn new(arities: Vec<usize>, cert: Option<String>) -> Self {
        FilePerKeyProver {
            arities,
            cert,
            binding: AnswerBinding::DigestEcho,
            answers: HashMap::new(),
        }
    }

    /// Why the tool cannot answer for this rule (checked BEFORE any request; coord-ec2-instantiate.md s.2, s.8).
    pub fn unavailable(&self, rule: &RuleText) -> Option<String> {
        let arity = rule.record.fixed.len();
        if rule.sym_rows {
            return Some("symmetry rows: instance-span has no symmetry rows (R46)".into());
        }
        if !self.arities.contains(&arity) {
            return Some(format!(
                "arity {arity}: the prover is built for {:?}",
                self.arities
            ));
        }
        if rule.witness_status() == WitnessStatus::Elided {
            return Some("witness loci elided".into());
        }
        for e in &rule.record.equations {
            match unit_pivot(e, arity) {
                Ok(true) => {}
                Ok(false) => {
                    return Some(format!(
                        "affine case equation without a unit coefficient: {e}"
                    ));
                }
                Err(err) => return Some(format!("case equation unreadable: {e}: {err}")),
            }
        }
        None
    }

    /// The request for `keys` of one rule.
    pub fn request(&self, rule: &RuleText, source: &RuleSource, keys: &[Key]) -> ExternalRequest {
        let index = rule.index().unwrap_or(-1);
        let line = json!({
            "schema": INSTANCE_SPAN_REQUEST_SCHEMA,
            "table": source.table_path,
            "offset": source.byte_offset,
            "rule": index,
            "rule_sha256": rule.line_sha256,
            "keys": keys,
            "rows": "cert",
            "cert": self.cert,
        });
        let mut argv = vec![
            "--mode".to_string(),
            "instance-span".into(),
            "--table".into(),
            source.table_path.clone(),
            "--rule".into(),
            index.to_string(),
        ];
        if let Some(o) = source.byte_offset {
            argv.extend(["--offset".to_string(), o.to_string()]);
        }
        if let Some(c) = &self.cert {
            argv.extend(["--cert".to_string(), c.clone()]);
        }
        argv.extend([
            "--keys".to_string(),
            "KEYS".into(),
            "--out".into(),
            "OUT".into(),
        ]);
        ExternalRequest {
            tool: "rr_proof instance-span".into(),
            path_hint: format!("instance-span/{}.request.jsonl", &rule.line_sha256[7..23]),
            text: format!("{line}\n"),
            argv,
        }
    }

    /// Load rr_proof's answer text for ONE rule (one child job per rule), BOUND to that rule (audit-accept-A M5, B C1):
    /// every span line must echo the rule's index and, under [`AnswerBinding::DigestEcho`] (default), its
    /// `rule_sha256`; every structural echo present (`record_bytes`, `arity`, `chart`, `rule_sizes`) must equal the
    /// candidate's. A refusal line (`verdict` ERROR / UNSUPPORTED / MISSING_*) makes every key Unsupported; any other
    /// line is an error (no silent drop). Returns the number of keys answered.
    pub fn load_answer(&mut self, rule: &RuleText, text: &str) -> Result<usize, String> {
        let mut per_key = BTreeMap::new();
        let mut refusal = None;
        for (no, line) in text.lines().enumerate() {
            let no = no + 1;
            if line.trim().is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(line).map_err(|e| format!("line {no}: {e}"))?;
            let verdict = v.get("verdict").and_then(Value::as_str).unwrap_or("");
            if v.get("schema").and_then(Value::as_str) != Some(SPAN_ANSWER_SCHEMA) {
                if matches!(verdict, "ERROR" | "UNSUPPORTED") || verdict.starts_with("MISSING_") {
                    refusal = Some(format!(
                        "{verdict}: {}",
                        v.get("error").and_then(Value::as_str).unwrap_or("")
                    ));
                    continue;
                }
                return Err(format!(
                    "line {no}: neither a {SPAN_ANSWER_SCHEMA} line nor a refusal"
                ));
            }
            check_echo(&v, rule, self.binding, no)?;
            for k in v
                .get("keys")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let key = answer_key(k.get("key").unwrap_or(&Value::Null), &rule.record.fixed)
                    .ok_or_else(|| format!("line {no}: unreadable key {k}"))?;
                if key.len() != rule.record.fixed.len() {
                    return Err(format!("line {no}: key {key:?} of the wrong length"));
                }
                let st = k
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if let Some(old) = per_key.insert(key.clone(), st.clone())
                    && old != st
                {
                    return Err(format!("line {no}: conflicting statuses at {key:?}"));
                }
            }
        }
        let n = per_key.len();
        self.answers.insert(
            rule.line_sha256.clone(),
            SpanAnswer {
                per_key,
                refusal,
                digest: sha256_tag(text.as_bytes()),
                binding: self.binding,
            },
        );
        Ok(n)
    }
}

/// Map an rr_proof per-key status word (the text before the first space) to a key status.
pub fn map_span_status(status: &str) -> KeyStatus {
    let word = status.split_whitespace().next().unwrap_or("");
    match word {
        "EXACT_IN_SPAN_AT_KEY" | "EXACT_PROVED_AT_KEY" => KeyStatus::Reached(Evidence::PerKey),
        "EXACT_NOT_IN_SPAN_AT_KEY" | "FAIL_AT_KEY" => {
            KeyStatus::Failed(KeyFailure::NonMembership {
                disproof: status.to_string(),
            })
        }
        "IRREGULAR_KEY" => KeyStatus::Failed(KeyFailure::Irregular {
            detail: status.to_string(),
        }),
        "HIDDEN_KEY" => KeyStatus::Failed(KeyFailure::Hidden {
            detail: status.to_string(),
        }),
        "ON_EXCEPTION_BRANCH" | "NOT_IN_CASE" | "NOT_IN_CHART" => {
            KeyStatus::NotCredited(word.to_string())
        }
        "" => KeyStatus::Unknown("no status".into()),
        other => KeyStatus::Unknown(other.to_string()),
    }
}

impl EvidenceProver for FilePerKeyProver {
    fn level(&self) -> Evidence {
        Evidence::PerKey
    }
    fn prove(&mut self, rule: &RuleText, source: &RuleSource, served: &[Key]) -> ProverAnswer {
        if let Some(why) = self.unavailable(rule) {
            return ProverAnswer::Unavailable(why);
        }
        let arity = rule.record.fixed.len();
        let Some(ans) = self.answers.get(&rule.line_sha256) else {
            return ProverAnswer::NeedsExternal(vec![self.request(rule, source, served)]);
        };
        if let Some(r) = &ans.refusal {
            return ProverAnswer::PerKey {
                keys: served
                    .iter()
                    .map(|k| (k.clone(), KeyStatus::Unsupported(r.clone())))
                    .collect(),
                artefact: None,
                rule_level: Some(RuleLevel {
                    disproof: false,
                    text: r.clone(),
                }),
            };
        }
        let missing: Vec<Key> = served
            .iter()
            .filter(|k| !ans.per_key.contains_key(*k))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return ProverAnswer::NeedsExternal(vec![self.request(rule, source, &missing)]);
        }
        let keys = served
            .iter()
            .map(|k| (k.clone(), map_span_status(&ans.per_key[k])))
            .collect();
        ProverAnswer::PerKey {
            keys,
            artefact: Some(EvidenceArtefact {
                schema: SPAN_ANSWER_SCHEMA.into(),
                rule_sha256: rule.line_sha256.clone(),
                digest: ans.digest.clone(),
                keys_digest: keys_digest(arity, served),
                tool: format!(
                    "rr_proof --mode instance-span;binding={}",
                    ans.binding.tag()
                ),
                body: None,
            }),
            rule_level: None,
        }
    }
}

/// One imported provenance record (`rustred.campaign.accept.v1`, e.g. the bank converter's lines).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ImportedInfo {
    pub rule_id: String,
    pub label: String,
    pub original_verdict: String,
    pub producer_known: bool,
    pub level_after: String,
    /// `sha256:` of the canonical import line (the artefact the evidence cites).
    pub line_digest: String,
}

/// Imported evidence keyed by the rule line's sha256 (hex, of the line with AND without its LF: the bank converter
/// records the LF-terminated digest).
#[derive(Clone, Debug, Default)]
pub struct ImportedEvidence {
    by_line: HashMap<String, ImportedInfo>,
}

impl ImportedEvidence {
    /// Read accept.v1 lines; lines of other schemas are skipped. The line digest is taken from the extension
    /// `bank.rule_line_sha256` (LF-terminated line) or `rule.rule_line_sha256` when present.
    pub fn read(text: &str) -> Result<ImportedEvidence, String> {
        let mut e = ImportedEvidence::default();
        for (no, l) in text.lines().enumerate() {
            if l.trim().is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(l).map_err(|x| format!("line {}: {x}", no + 1))?;
            if v["schema"] != crate::schema::ACCEPT_SCHEMA {
                continue; // classified: another record kind of the import file
            }
            let Some(d) = v["bank"]["rule_line_sha256"]
                .as_str()
                .or(v["rule"]["rule_line_sha256"].as_str())
            else {
                // no silent drop: an accept line that names no rule line can not be used and is an error
                return Err(format!(
                    "line {}: accept.v1 line without rule_line_sha256 (bank.* or rule.*)",
                    no + 1
                ));
            };
            let info = ImportedInfo {
                rule_id: v["rule"]["id"].as_str().unwrap_or("").into(),
                label: v["imported_from"]["label"].as_str().unwrap_or("").into(),
                original_verdict: v["imported_from"]["original_verdict"]
                    .as_str()
                    .unwrap_or("")
                    .into(),
                producer_known: v["rule"]["producer"]["bin_blake3"].as_str().is_some(),
                level_after: v["level_after"].as_str().unwrap_or("").into(),
                line_digest: sha256_tag(
                    crate::schema::canon::canonical_json(&v, true)
                        .unwrap_or_default()
                        .as_bytes(),
                ),
            };
            e.by_line.insert(d.to_string(), info);
        }
        Ok(e)
    }
    pub fn len(&self) -> usize {
        self.by_line.len()
    }
    pub fn is_empty(&self) -> bool {
        self.by_line.is_empty()
    }
    /// The import record of a rule (by its line, with or without the LF).
    pub fn get(&self, rule: &RuleText) -> Option<&ImportedInfo> {
        let bare = rule.line_sha256.trim_start_matches("sha256:");
        let mut with_lf = rule.line.clone();
        with_lf.push(b'\n');
        let lf = sha256_tag(&with_lf);
        self.by_line
            .get(bare)
            .or_else(|| self.by_line.get(lf.trim_start_matches("sha256:")))
    }
}

/// Evidence from imports only: every served key of a rule with an import record reaches [`Evidence::Imported`]
/// (never more: an imported label never substitutes for a replay record or a per-key proof).
pub struct ImportedProver {
    pub imports: ImportedEvidence,
}

impl EvidenceProver for ImportedProver {
    fn level(&self) -> Evidence {
        Evidence::Imported
    }
    fn prove(&mut self, rule: &RuleText, _source: &RuleSource, served: &[Key]) -> ProverAnswer {
        let Some(info) = self.imports.get(rule) else {
            return ProverAnswer::Unavailable("no import record for this rule line".into());
        };
        ProverAnswer::PerKey {
            keys: served
                .iter()
                .map(|k| (k.clone(), KeyStatus::Reached(Evidence::Imported)))
                .collect(),
            artefact: Some(EvidenceArtefact {
                schema: crate::schema::ACCEPT_SCHEMA.into(),
                rule_sha256: rule.line_sha256.clone(),
                digest: info.line_digest.clone(),
                keys_digest: keys_digest(rule.record.fixed.len(), served),
                tool: format!("imported:{}:{}", info.rule_id, info.label),
                body: None,
            }),
            rule_level: None,
        }
    }
}
