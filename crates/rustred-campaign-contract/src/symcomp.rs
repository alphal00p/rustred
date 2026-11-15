//! Kind `symmetry-composite` (lane symjob; interfaces/symmetry-composite-job.md): the repair flow's symmetry COMPOSITE
//! step run by `rr_rf_symflow` as a LEGACY child. This module is the contract side:
//!
//! - the child's own SEAL (directory `out/symflow/`: five files, `SHA256SUMS`, `result.json` written last) and its
//!   check against the job spec ([`check_child`]): request digest, input PINS, tool digest, every file digest and line
//!   count, keys against the keys input, relation counts, the child's own independent re-check count;
//! - the wrapper's sealed stream `out/results.jsonl` ([`results_records`], [`end_record`]) and its re-check by the
//!   classifier ([`tally`]: S of the kind), the per-key outcomes ([`SymKeyOutcome`]) and their failure classes;
//! - `effective.node_options` of the kind ([`SymmetryCompositeNodeOptions`], [`effective`]).
//!
//! The crate has no SHA-256 (ruling R-a: serde, serde_json, blake3 only): the callers pass one ([`Sha256Fn`]; the
//! supervisor passes campaign-core's std-only implementation). Generic (L8): no node, arity, count or path is known
//! here; the arity of a key is the length of the keys the child reports.

use crate::outcome::{Effective, LICENCE_PRESENT};
use crate::recipe::{FailureClass, RelationTally};
use crate::spec::{InputRef, JobKind, JobSpec};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Schema of the `request` input (the tool's).
pub const REQUEST_SCHEMA: &str = "rustred.campaign.symmetry-composite.request.v1";
/// Schema of the child's `result.json`.
pub const RESULT_SCHEMA: &str = "rustred.campaign.symmetry-composite.result.v1";
/// Schema of a verification line (`composite.verification.jsonl`).
pub const VERIFICATION_SCHEMA: &str = "rustred.campaign.symmetry-composite.v1";
/// The child's output directory under `out/`.
pub const CHILD_DIR: &str = "symflow";
/// The child's sealed files, in the order of `SHA256SUMS` and `result.outputs`.
pub const CHILD_FILES: [&str; 5] = [
    "choices.jsonl",
    "keys.jsonl",
    "composite.table.jsonl",
    "composite.verification.jsonl",
    "report.json",
];
/// The shipped table and its verification lines.
pub const TABLE_FILE: &str = "composite.table.jsonl";
pub const VERIFICATION_FILE: &str = "composite.verification.jsonl";
pub const KEYS_FILE: &str = "keys.jsonl";
/// The child's digest list and its seal (written last).
pub const SUMS_FILE: &str = "SHA256SUMS";
pub const RESULT_FILE: &str = "result.json";
/// The request member the supervisor adds: SHA-256 of the program file it launches (the tool ignores it; the wrapper
/// compares it with `result.tool.sha256`).
pub const PROGRAM_SHA256: &str = "program_sha256";
/// The word an OUTCOME error names when the child's sealed result is bound to OTHER inputs than the job's (row 36).
pub const PINNED_MISMATCH: &str = "PINNED_INPUTS_MISMATCH";
/// The input roles the request pins (besides itself), in the child's `result.inputs` order.
pub const PINNED_ROLES: [&str; 3] = ["import_spec", "keys", "policy"];
/// Prefix of the tally's determinism id.
pub const DETERMINISM_PREFIX: &str = "rustred.symmetry-composite.v1/";

/// A SHA-256 (lower-case hex) of bytes, supplied by the caller.
pub type Sha256Fn<'a> = &'a dyn Fn(&[u8]) -> String;

/// The outcome class of one key (closed list).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymClass {
    /// The image alone descends (route 0b).
    Plain,
    /// Image + catalogue rule(s) at the non-descending image terms (TRICKS 1 + 3).
    Composite,
    /// No element gives a descending composed relation.
    None,
    /// The symmetry row expansion exceeded the spec bound (`route0.max_terms`).
    OverCap,
    /// The key could not be judged (any other per-key error of the child).
    Error,
}

impl SymClass {
    /// A shipped relation exists for the key.
    pub const fn has_relation(self) -> bool {
        matches!(self, SymClass::Plain | SymClass::Composite)
    }
    /// The failure class of a key without a relation (typed per-key outcome, never a job failure).
    pub const fn failure_class(self) -> Option<FailureClass> {
        match self {
            SymClass::Plain | SymClass::Composite => None,
            SymClass::None => Some(FailureClass::NoSymmetryRelation),
            SymClass::OverCap => Some(FailureClass::OverCapRowExpansion),
            SymClass::Error => Some(FailureClass::Error),
        }
    }
    fn of_word(s: &str) -> Option<SymClass> {
        serde_json::from_value(Value::String(s.to_string())).ok()
    }
    pub fn word(self) -> &'static str {
        match self {
            SymClass::Plain => "plain",
            SymClass::Composite => "composite",
            SymClass::None => "none",
            SymClass::OverCap => "over_cap",
            SymClass::Error => "error",
        }
    }
}

/// One key as the child reports it (keys.jsonl) and its class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymKey {
    pub key: Vec<i64>,
    pub face: String,
    pub status: String,
    pub class: SymClass,
}

/// One shipped relation (a table line and its verification line), bound by their SHA-256.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymRelation {
    pub index: u64,
    pub target: Vec<i64>,
    /// SHA-256 of the table line (without its newline).
    pub line_sha256: String,
    /// SHA-256 of the verification line (without its newline).
    pub verification_sha256: String,
    pub element: u64,
    pub group_digest: String,
    /// record_sha256 of every catalogue rule the composition used (replacement order).
    pub records: Vec<String>,
    pub plain: bool,
    pub rhs_terms: u64,
}

/// What the wrapper established from the child's seal ([`check_child`]).
#[derive(Debug, Clone, PartialEq)]
pub struct ChildSeal {
    pub keys: Vec<SymKey>,
    /// Index into `relations` for each key with a relation.
    pub relation_of: Vec<Option<u64>>,
    pub relations: Vec<SymRelation>,
    /// (file, sha256, lines) of the five files and `result.json`.
    pub files: Vec<(String, String, u64)>,
    pub request_sha256: String,
    pub request_blake3: String,
    pub tool_sha256: String,
    pub group_digest: String,
    pub spec_digest: Value,
    pub policy_digest: Value,
    pub independently_verified: u64,
}

impl ChildSeal {
    /// Keys without a relation.
    pub fn unsolved(&self) -> usize {
        self.keys.iter().filter(|k| !k.class.has_relation()).count()
    }
}

/// Why the wrapper refuses the child's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildRefusal {
    /// The result (or the request) is bound to OTHER inputs than the job's (row 36: refused, attention).
    PinnedInputs(String),
    /// Unreadable, unsealed, truncated, inconsistent, or the child's own re-check disagrees (row 22: nothing).
    Unsealed(String),
}

impl std::fmt::Display for ChildRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChildRefusal::PinnedInputs(m) => write!(f, "{PINNED_MISMATCH}: {m}"),
            ChildRefusal::Unsealed(m) => write!(f, "unsealed: {m}"),
        }
    }
}

fn input_of<'a>(spec: &'a JobSpec, role: &str) -> Option<&'a InputRef> {
    spec.inputs.iter().find(|i| i.role == role)
}

fn strip_sha(s: &str) -> String {
    s.trim_start_matches("sha256:").to_ascii_lowercase()
}

/// The request input's bytes and JSON (its BLAKE3 = the spec's, its schema the tool's).
pub fn request_of(spec: &JobSpec) -> Result<(Vec<u8>, Value), ChildRefusal> {
    let pin = |m: String| ChildRefusal::PinnedInputs(m);
    let r = input_of(spec, "request").ok_or_else(|| pin("no request input".into()))?;
    let b = std::fs::read(spec.input_path(r)).map_err(|e| pin(format!("request input: {e}")))?;
    if crate::blake3_hex(&b) != r.blake3 {
        return Err(pin("the request input changed (BLAKE3 differs from the spec's)".into()));
    }
    let v: Value = serde_json::from_slice(&b).map_err(|e| pin(format!("request: {e}")))?;
    if v["schema"] != REQUEST_SCHEMA {
        return Err(pin(format!("request schema {} (expected {REQUEST_SCHEMA})", v["schema"])));
    }
    Ok((b, v))
}

/// PIN RULE (interfaces/symmetry-composite-job.md 2): for every pinned role the request names it iff the spec has an
/// input of that role, with the same (absolute) path, and the SHA-256 the request states is the SHA-256 of the bytes
/// whose BLAKE3 is the spec's. Returns the pins as (role, path, sha256), in [`PINNED_ROLES`] order.
pub fn check_pins(
    spec: &JobSpec,
    request: &Value,
    sha256: Sha256Fn,
) -> Result<Vec<(String, String, String)>, ChildRefusal> {
    let pin = |m: String| ChildRefusal::PinnedInputs(m);
    let mut out = vec![];
    for role in PINNED_ROLES {
        let want = &request[role];
        match (input_of(spec, role), want.is_null()) {
            (None, true) => continue,
            (None, false) => return Err(pin(format!("the request pins {role}, the spec has no such input"))),
            (Some(_), true) => return Err(pin(format!("the spec has input {role}, the request does not pin it"))),
            (Some(i), false) => {
                let p = spec.input_path(i);
                let rp = want["path"].as_str().unwrap_or_default();
                if Path::new(rp) != p.as_path() {
                    return Err(pin(format!(
                        "{role}: the request names {rp:?}, the spec {}",
                        p.display()
                    )));
                }
                let b = std::fs::read(&p).map_err(|e| pin(format!("{role} {}: {e}", p.display())))?;
                if crate::blake3_hex(&b) != i.blake3 {
                    return Err(pin(format!("{role}: BLAKE3 differs from the spec's")));
                }
                let h = sha256(&b);
                let stated = want["sha256"].as_str().map(strip_sha);
                if stated.as_deref() != Some(h.as_str()) {
                    return Err(pin(format!(
                        "{role}: the request states sha256 {stated:?}, the file has {h}"
                    )));
                }
                out.push((role.to_string(), rp.to_string(), h));
            }
        }
    }
    Ok(out)
}

/// The keys of the `keys` input (one JSON array per non-empty line), if the spec has one.
pub fn keys_input(spec: &JobSpec) -> Result<Option<Vec<Vec<i64>>>, String> {
    let Some(i) = input_of(spec, "keys") else {
        return Ok(None);
    };
    let b = std::fs::read(spec.input_path(i)).map_err(|e| format!("keys input: {e}"))?;
    if crate::blake3_hex(&b) != i.blake3 {
        return Err("keys input: BLAKE3 differs from the spec's".into());
    }
    let mut v = vec![];
    for (n, l) in String::from_utf8_lossy(&b).lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        v.push(serde_json::from_str(l).map_err(|e| format!("keys input line {}: {e}", n + 1))?);
    }
    Ok(Some(v))
}

/// Lines of a file without their newline (a missing final newline is refused: every child file ends with one).
fn lines_of(b: &[u8]) -> Result<Vec<&[u8]>, String> {
    if b.is_empty() {
        return Ok(vec![]);
    }
    if b.last() != Some(&b'\n') {
        return Err("no final newline (truncated?)".into());
    }
    Ok(b[..b.len() - 1].split(|&c| c == b'\n').collect())
}

fn key_of_text(t: &str) -> Option<Vec<i64>> {
    let inner = t.trim().strip_prefix("I(")?.strip_suffix(')')?;
    inner.split(',').map(|x| x.trim().parse().ok()).collect()
}

/// THE CHILD-SEAL CHECK (interfaces/symmetry-composite-job.md 3): `out/symflow/` of the spec's run directory against
/// the spec and the request. `PinnedInputs` = bound to other inputs (row 36); `Unsealed` = nothing accepted (row 22).
pub fn check_child(spec: &JobSpec, sha256: Sha256Fn) -> Result<ChildSeal, ChildRefusal> {
    if spec.kind != JobKind::SymmetryComposite {
        return Err(ChildRefusal::Unsealed(format!("kind {:?}", spec.kind)));
    }
    let uns = |m: String| ChildRefusal::Unsealed(m);
    let pin = |m: String| ChildRefusal::PinnedInputs(m);
    let (req_bytes, req) = request_of(spec)?;
    let pins = check_pins(spec, &req, sha256)?;
    let dir: PathBuf = Path::new(&spec.run_dir).join("out").join(CHILD_DIR);
    let rb = std::fs::read(dir.join(RESULT_FILE)).map_err(|e| uns(format!("{RESULT_FILE}: {e}")))?;
    let res: Value = serde_json::from_slice(&rb).map_err(|e| uns(format!("{RESULT_FILE}: {e}")))?;
    if res["schema"] != RESULT_SCHEMA || res["verdict"] != "DONE" {
        return Err(uns(format!(
            "{RESULT_FILE}: schema {} verdict {}",
            res["schema"], res["verdict"]
        )));
    }
    // bound to THIS request, these inputs, this program
    let req_sha = sha256(&req_bytes);
    if res["request_sha256"].as_str().map(strip_sha).as_deref() != Some(req_sha.as_str()) {
        return Err(pin(format!(
            "result.request_sha256 {} is not the request's {req_sha}",
            res["request_sha256"]
        )));
    }
    let got_inputs: Vec<(String, String, String)> = res["inputs"]
        .as_array()
        .ok_or_else(|| uns("result.inputs".into()))?
        .iter()
        .map(|i| {
            (
                i["role"].as_str().unwrap_or_default().to_string(),
                i["path"].as_str().unwrap_or_default().to_string(),
                i["sha256"].as_str().map(strip_sha).unwrap_or_default(),
            )
        })
        .collect();
    if got_inputs != pins {
        return Err(pin(format!(
            "result.inputs {got_inputs:?} are not the job's pins {pins:?}"
        )));
    }
    let want_tool = req[PROGRAM_SHA256].as_str().map(strip_sha).unwrap_or_default();
    let tool = res["tool"]["sha256"].as_str().map(strip_sha).unwrap_or_default();
    if want_tool.is_empty() || tool != want_tool {
        return Err(pin(format!(
            "result.tool.sha256 {tool:?} is not the launched program's {want_tool:?}"
        )));
    }
    // every sealed file: digest and line count recomputed, SHA256SUMS exactly these, result.outputs exactly these
    let outs = res["outputs"].as_array().ok_or_else(|| uns("result.outputs".into()))?;
    if outs.len() != CHILD_FILES.len() {
        return Err(uns(format!("result.outputs lists {} files", outs.len())));
    }
    let mut bytes: BTreeMap<&str, Vec<u8>> = BTreeMap::new();
    let mut files = vec![];
    let mut sums = String::new();
    for (o, name) in outs.iter().zip(CHILD_FILES) {
        if o["file"] != name {
            return Err(uns(format!("result.outputs names {} where {name} belongs", o["file"])));
        }
        let b = std::fs::read(dir.join(name)).map_err(|e| uns(format!("{name}: {e}")))?;
        let h = sha256(&b);
        let lines = b.iter().filter(|&&c| c == b'\n').count() as u64;
        if o["sha256"].as_str().map(strip_sha).as_deref() != Some(h.as_str())
            || o["lines"].as_u64() != Some(lines)
        {
            return Err(uns(format!(
                "{name}: sha256 {h} / {lines} lines differ from result.outputs"
            )));
        }
        sums.push_str(&format!("{h}  {name}\n"));
        files.push((name.to_string(), h, lines));
        bytes.insert(name, b);
    }
    let sb = std::fs::read(dir.join(SUMS_FILE)).map_err(|e| uns(format!("{SUMS_FILE}: {e}")))?;
    if sb != sums.as_bytes() {
        return Err(uns(format!("{SUMS_FILE} is not the digest list of the five files")));
    }
    files.push((
        RESULT_FILE.to_string(),
        sha256(&rb),
        rb.iter().filter(|&&c| c == b'\n').count() as u64,
    ));
    // keys
    let kl = lines_of(&bytes[KEYS_FILE]).map_err(|e| uns(format!("{KEYS_FILE}: {e}")))?;
    let mut keys = vec![];
    for (n, l) in kl.iter().enumerate() {
        let v: Value =
            serde_json::from_slice(l).map_err(|e| uns(format!("{KEYS_FILE} line {}: {e}", n + 1)))?;
        let key: Vec<i64> = serde_json::from_value(v["key"].clone())
            .map_err(|e| uns(format!("{KEYS_FILE} line {}: key: {e}", n + 1)))?;
        let status = v["status"].as_str().unwrap_or_default().to_string();
        let class = if status == "ok" {
            match &v["best"] {
                Value::Null => SymClass::None,
                b if b["plain"] == true => SymClass::Plain,
                b if b["plain"] == false => SymClass::Composite,
                _ => return Err(uns(format!("{KEYS_FILE} line {}: best without plain", n + 1))),
            }
        } else if status.starts_with("over_cap") {
            SymClass::OverCap
        } else if status.starts_with("error") {
            SymClass::Error
        } else {
            return Err(uns(format!("{KEYS_FILE} line {}: status {status:?}", n + 1)));
        };
        if !class.has_relation() && !v["best"].is_null() {
            return Err(uns(format!("{KEYS_FILE} line {}: a best without status ok", n + 1)));
        }
        keys.push(SymKey {
            key,
            face: v["face"].as_str().unwrap_or_default().to_string(),
            status,
            class,
        });
    }
    if res["keys"].as_u64() != Some(keys.len() as u64) {
        return Err(uns(format!("result.keys {} but {} key lines", res["keys"], keys.len())));
    }
    if let Some(want) = keys_input(spec).map_err(&uns)? {
        let want: Vec<Vec<i64>> = match req["limit"].as_u64() {
            Some(n) => want.into_iter().take(n as usize).collect(),
            None => want,
        };
        if keys.iter().map(|k| &k.key).ne(want.iter()) {
            return Err(uns("the child's keys are not the keys input (order, set)".into()));
        }
    }
    // shipped relations: table (header + one line each) and verification lines, 1:1 with the keys that have one
    let tl = lines_of(&bytes[TABLE_FILE]).map_err(|e| uns(format!("{TABLE_FILE}: {e}")))?;
    let vl = lines_of(&bytes[VERIFICATION_FILE]).map_err(|e| uns(format!("{VERIFICATION_FILE}: {e}")))?;
    let Some((head, tl)) = tl.split_first() else {
        return Err(uns(format!("{TABLE_FILE}: no header")));
    };
    let h: Value = serde_json::from_slice(head).map_err(|e| uns(format!("{TABLE_FILE} header: {e}")))?;
    if h["type"] != "header" || h["kind"] != "symmetry-composite" {
        return Err(uns(format!("{TABLE_FILE}: line 1 is not the symmetry-composite header")));
    }
    let with: Vec<usize> = (0..keys.len()).filter(|&i| keys[i].class.has_relation()).collect();
    let shipped = res["shipped_relations"].as_u64();
    if tl.len() != vl.len() || tl.len() != with.len() || shipped != Some(tl.len() as u64) {
        return Err(uns(format!(
            "{} table lines, {} verification lines, {} keys with a relation, result.shipped_relations {:?}",
            tl.len(),
            vl.len(),
            with.len(),
            shipped
        )));
    }
    let group_digest = res["group"]["digest"].as_str().unwrap_or_default().to_string();
    let mut relations = vec![];
    let mut relation_of = vec![None; keys.len()];
    for (j, ((t, v), &ki)) in tl.iter().zip(&vl).zip(&with).enumerate() {
        let at = |m: String| uns(format!("relation {j}: {m}"));
        let tv: Value = serde_json::from_slice(t).map_err(|e| at(format!("table line: {e}")))?;
        let vv: Value = serde_json::from_slice(v).map_err(|e| at(format!("verification line: {e}")))?;
        let target: Vec<i64> = serde_json::from_value(tv["fixed"].clone())
            .map_err(|e| at(format!("table line fixed: {e}")))?;
        if target != keys[ki].key
            || tv["target"].as_str().and_then(key_of_text).as_ref() != Some(&target)
            || vv["target"].as_str().and_then(key_of_text).as_ref() != Some(&target)
        {
            return Err(at("the table / verification target is not the key's".into()));
        }
        if tv["type"] != "rule" || tv["index"].as_u64() != Some(j as u64) {
            return Err(at("table line is not rule record j".into()));
        }
        if vv["schema"] != VERIFICATION_SCHEMA || vv["group_digest"] != group_digest.as_str() {
            return Err(at("verification schema / group digest".into()));
        }
        let plain = vv["plain"].as_bool().ok_or_else(|| at("plain".into()))?;
        if plain != (keys[ki].class == SymClass::Plain) {
            return Err(at("plain differs from the key's class".into()));
        }
        let records: Vec<String> = vv["replacements"]
            .as_array()
            .ok_or_else(|| at("replacements".into()))?
            .iter()
            .map(|r| r["record_sha256"].as_str().unwrap_or_default().to_string())
            .collect();
        if plain != records.is_empty() {
            return Err(at("a plain relation with replacements or a composite without".into()));
        }
        let rhs_terms = tv["rhs"].as_array().map_or(0, Vec::len) as u64;
        if vv["rhs_terms"].as_u64() != Some(rhs_terms) {
            return Err(at("rhs_terms differs from the table line".into()));
        }
        relation_of[ki] = Some(j as u64);
        relations.push(SymRelation {
            index: j as u64,
            target,
            line_sha256: sha256(t),
            verification_sha256: sha256(v),
            element: vv["element"].as_u64().ok_or_else(|| at("element".into()))?,
            group_digest: group_digest.clone(),
            records,
            plain,
            rhs_terms,
        });
    }
    let independently_verified = res["independently_verified"].as_u64().unwrap_or(0);
    if independently_verified != relations.len() as u64 {
        return Err(uns(format!(
            "the child's own re-check verified {independently_verified} of {} shipped relations",
            relations.len()
        )));
    }
    Ok(ChildSeal {
        keys,
        relation_of,
        relations,
        files,
        request_sha256: req_sha,
        request_blake3: input_of(spec, "request").map(|i| i.blake3.clone()).unwrap_or_default(),
        tool_sha256: tool,
        group_digest,
        spec_digest: res["spec_digest"].clone(),
        policy_digest: res["policy_digest"].clone(),
        independently_verified,
    })
}

/// The records of `out/results.jsonl` (compact JSON; the writer canonicalises): key records, relation records, file
/// records (interfaces/symmetry-composite-job.md 4).
pub fn results_records(s: &ChildSeal) -> Vec<Value> {
    let mut v = vec![];
    for (i, (k, r)) in s.keys.iter().zip(&s.relation_of).enumerate() {
        v.push(json!({"type": "key", "ordinal": i, "key": k.key, "face": k.face, "outcome": k.class.word(),
            "relation": r, "status": k.status}));
    }
    for r in &s.relations {
        v.push(json!({"type": "relation", "index": r.index, "target": r.target, "line_sha256": r.line_sha256,
            "verification_sha256": r.verification_sha256, "element": r.element, "group_digest": r.group_digest,
            "records": r.records, "plain": r.plain, "rhs_terms": r.rhs_terms}));
    }
    for (f, h, l) in &s.files {
        v.push(json!({"type": "file", "file": f, "sha256": h, "lines": l}));
    }
    v
}

/// The end record of `out/results.jsonl`.
pub fn end_record(s: &ChildSeal) -> Value {
    json!({"type": "end", "records": s.keys.len() + s.relations.len() + s.files.len(), "keys": s.keys.len(),
        "relations": s.relations.len(), "request_sha256": s.request_sha256, "request_blake3": s.request_blake3,
        "tool_sha256": s.tool_sha256, "group_digest": s.group_digest, "spec_digest": s.spec_digest,
        "policy_digest": s.policy_digest, "independently_verified": s.independently_verified})
}

/// One key's typed outcome as the classifier re-reads it from the sealed results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymKeyOutcome {
    pub ordinal: u64,
    pub key: Vec<i64>,
    pub face: String,
    pub class: SymClass,
    /// The relation record's index (keys with a relation).
    pub relation: Option<u64>,
    pub status: String,
}

impl SymKeyOutcome {
    pub fn failure_class(&self) -> Option<FailureClass> {
        self.class.failure_class()
    }
}

/// The sealed results re-read (S of the kind; [`tally`]).
#[derive(Debug, Clone, PartialEq)]
pub struct SymResults {
    pub keys: Vec<SymKeyOutcome>,
    pub relations: Vec<SymRelation>,
    pub files: BTreeMap<String, (String, u64)>,
    pub end: Value,
}

/// S of kind symmetry-composite: the canonical lines of the verified `out/results.jsonl` re-read and checked (key
/// records contiguous, each class in the closed list, relations 1:1 with the keys that have one and bound to them,
/// file records exactly the child's six files, end figures, the request's BLAKE3 = the spec's, keys = the keys input
/// when given). The tally: records = keys, accepted = shipped relations (CANDIDATES: accept `supplied_relations`),
/// unsolved per failure class.
pub fn tally(lines: &[Vec<u8>], spec: &JobSpec) -> Result<(SymResults, RelationTally), String> {
    let mut keys: Vec<SymKeyOutcome> = vec![];
    let mut relations: Vec<SymRelation> = vec![];
    let mut files: BTreeMap<String, (String, u64)> = BTreeMap::new();
    let mut end: Option<Value> = None;
    for (n, l) in lines.iter().enumerate() {
        let v: Value = serde_json::from_slice(l).map_err(|e| format!("results line {}: {e}", n + 1))?;
        if end.is_some() {
            return Err(format!("results line {}: a record after the end record", n + 1));
        }
        match v["type"].as_str() {
            Some("key") => {
                if !relations.is_empty() || !files.is_empty() {
                    return Err(format!("results line {}: key record out of order", n + 1));
                }
                let class = v["outcome"]
                    .as_str()
                    .and_then(SymClass::of_word)
                    .ok_or_else(|| format!("results line {}: outcome {}", n + 1, v["outcome"]))?;
                let relation = v["relation"].as_u64();
                if v["ordinal"].as_u64() != Some(keys.len() as u64)
                    || relation.is_some() != class.has_relation()
                    || (relation.is_none() && !v["relation"].is_null())
                {
                    return Err(format!("results line {}: key record ordinal / relation", n + 1));
                }
                keys.push(SymKeyOutcome {
                    ordinal: keys.len() as u64,
                    key: serde_json::from_value(v["key"].clone())
                        .map_err(|e| format!("results line {}: key: {e}", n + 1))?,
                    face: v["face"].as_str().unwrap_or_default().to_string(),
                    class,
                    relation,
                    status: v["status"].as_str().unwrap_or_default().to_string(),
                });
            }
            Some("relation") => {
                if !files.is_empty() {
                    return Err(format!("results line {}: relation record out of order", n + 1));
                }
                let r: SymRelation = serde_json::from_value(json!({"index": v["index"], "target": v["target"],
                    "line_sha256": v["line_sha256"], "verification_sha256": v["verification_sha256"],
                    "element": v["element"], "group_digest": v["group_digest"], "records": v["records"],
                    "plain": v["plain"], "rhs_terms": v["rhs_terms"]}))
                .map_err(|e| format!("results line {}: relation: {e}", n + 1))?;
                if r.index != relations.len() as u64 {
                    return Err(format!("results line {}: relation index", n + 1));
                }
                relations.push(r);
            }
            Some("file") => {
                let f = v["file"].as_str().unwrap_or_default().to_string();
                let h = v["sha256"].as_str().unwrap_or_default().to_string();
                if files.insert(f.clone(), (h, v["lines"].as_u64().unwrap_or(0))).is_some() {
                    return Err(format!("results line {}: file {f} twice", n + 1));
                }
            }
            Some("end") => end = Some(v),
            t => return Err(format!("results line {}: record type {t:?}", n + 1)),
        }
    }
    let end = end.ok_or("results: no end record")?;
    // relations 1:1 with the keys that have one, in key order, bound by target
    let mut j = 0u64;
    for k in &keys {
        if let Some(r) = k.relation {
            if r != j {
                return Err(format!("key {}: relation {r}, expected {j}", k.ordinal));
            }
            let rel = relations.get(r as usize).ok_or("a key names a missing relation")?;
            if rel.target != k.key || rel.plain != (k.class == SymClass::Plain) {
                return Err(format!("relation {r}: target / plain is not key {}'s", k.ordinal));
            }
            j += 1;
        }
    }
    if j != relations.len() as u64 {
        return Err(format!("{} relations, {j} keys with a relation", relations.len()));
    }
    let names: Vec<&str> = CHILD_FILES.iter().copied().chain([RESULT_FILE]).collect();
    if files.len() != names.len() || names.iter().any(|f| !files.contains_key(*f)) {
        return Err(format!("file records {:?} are not the child's files", files.keys()));
    }
    let total = keys.len() + relations.len() + files.len();
    if end["records"].as_u64() != Some(total as u64)
        || end["keys"].as_u64() != Some(keys.len() as u64)
        || end["relations"].as_u64() != Some(relations.len() as u64)
        || end["independently_verified"].as_u64() != Some(relations.len() as u64)
    {
        return Err("results end record figures differ from the records".into());
    }
    let rq = input_of(spec, "request").ok_or("no request input")?;
    if end["request_blake3"].as_str() != Some(rq.blake3.as_str()) {
        return Err("results end request_blake3 is not the spec's request input".into());
    }
    if let Some(want) = keys_input(spec)? {
        let limit = request_of(spec)
            .map_err(|e| e.to_string())?
            .1["limit"]
            .as_u64();
        let want: Vec<Vec<i64>> = match limit {
            Some(n) => want.into_iter().take(n as usize).collect(),
            None => want,
        };
        if keys.iter().map(|k| &k.key).ne(want.iter()) {
            return Err("results keys are not the keys input".into());
        }
    }
    let mut unsolved: BTreeMap<FailureClass, u64> = BTreeMap::new();
    for k in &keys {
        if let Some(c) = k.failure_class() {
            *unsolved.entry(c).or_default() += 1;
        }
    }
    let t = RelationTally {
        records: keys.len() as u64,
        accepted: relations.len() as u64,
        unsolved,
        recipe_digest: None,
        determinism_id: Some(format!(
            "{DETERMINISM_PREFIX}{}",
            end["request_sha256"].as_str().unwrap_or_default()
        )),
        probe: false,
        probes: BTreeMap::new(),
    };
    Ok((
        SymResults {
            keys,
            relations,
            files,
            end,
        },
        t,
    ))
}

/// `effective.node_options` of kind symmetry-composite: exactly the request's result parameters (threads included as
/// the other kinds do, although the child's sealed files do not depend on it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SymmetryCompositeNodeOptions {
    pub request_sha256: String,
    pub node: Option<String>,
    pub import_base: String,
    pub limit: Option<u64>,
    pub threads: u64,
    pub program_sha256: String,
}

impl SymmetryCompositeNodeOptions {
    /// From the request JSON and its SHA-256.
    pub fn of_request(request: &Value, request_sha256: &str) -> SymmetryCompositeNodeOptions {
        SymmetryCompositeNodeOptions {
            request_sha256: request_sha256.to_string(),
            node: request["node"].as_str().map(str::to_string),
            import_base: request["import_base"].as_str().unwrap_or("/").to_string(),
            limit: request["limit"].as_u64(),
            threads: request["threads"].as_u64().unwrap_or(1).max(1),
            program_sha256: request[PROGRAM_SHA256].as_str().unwrap_or_default().to_string(),
        }
    }
}

/// The licence variable the tool needs (presence only is recorded).
pub const LICENCE_ENV: &str = "SYMBOLICA_LICENSE";

/// `job.effective` of a symmetry-composite attempt: subcommand `run`, no args (empty allow-list), the node options,
/// order null (the order is the import spec's), env = the licence's presence. `blake3` = the contract's digest.
pub fn effective(
    opts: &SymmetryCompositeNodeOptions,
    licence_present: bool,
) -> Result<Effective, crate::canon::CanonError> {
    let mut env = BTreeMap::new();
    if licence_present {
        env.insert(LICENCE_ENV.to_string(), LICENCE_PRESENT.to_string());
    }
    Effective {
        subcommand: "run".into(),
        args: vec![],
        node_options: serde_json::to_value(opts).expect("node options serialise"),
        order: Value::Null,
        env,
        blake3: String::new(),
    }
    .sealed()
}

/// The legacy argv of the kind, from the spec only: `run --request <request input> --out <RUN>/out/symflow`.
pub fn argv(spec: &JobSpec) -> Result<Vec<String>, String> {
    if spec.kind != JobKind::SymmetryComposite {
        return Err(format!("kind {:?} is not symmetry-composite", spec.kind));
    }
    let r = input_of(spec, "request").ok_or("no request input")?;
    Ok(vec![
        "run".into(),
        "--request".into(),
        spec.input_path(r).display().to_string(),
        "--out".into(),
        Path::new(&spec.run_dir)
            .join("out")
            .join(CHILD_DIR)
            .display()
            .to_string(),
    ])
}

/// Whether an OUTCOME error message names [`PINNED_MISMATCH`] as a word (row 36).
pub fn names_pinned_mismatch(message: &str) -> bool {
    message
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .any(|w| w == PINNED_MISMATCH)
}

/// Where the wrapper writes why it refused the child's result (diagnostics; never read as evidence).
pub const REFUSAL_FILE: &str = "wrapper-refusal.txt";

/// The legacy exit mapping of the kind: 0 -> checked (0 / 10 / 41 / unsealed 0), 2 -> 41 (51 when the refusal names
/// a host-resource errno, row 15b); anything else stays as it is (no OUTCOME: rows 18-21, 23).
fn legacy_code(w: crate::classify::WaitStatus) -> crate::classify::WaitStatus {
    use crate::classify::WaitStatus;
    match w {
        WaitStatus::Exited(2) => WaitStatus::Exited(crate::exit::ExitCode::Input.code()),
        w => w,
    }
}

/// THE WRAPPER of kind symmetry-composite (interfaces/symmetry-composite-job.md 5), called by the supervisor's settle
/// BEFORE the contract classifies (a no-op for other kinds, for an unknown wait status and when OUTCOME exists).
/// Child exit 0: [`check_child`]; Ok -> `out/results.jsonl` sealed + OUTCOME exit 0 (every key has a relation) or 10
/// stop `keys_unsolved`; `PinnedInputs` -> OUTCOME exit 41 naming [`PINNED_MISMATCH`] (row 36); `Unsealed` -> OUTCOME
/// exit 0 WITHOUT a sealed stream (row 22). Child exit 2 -> OUTCOME 41 (row 13), or 51 when its ERROR line names a
/// host-resource errno (row 15b). Returns the wait status the decision table must see.
pub fn wrap(
    layout: &crate::layout::RunLayout,
    spec: &JobSpec,
    spec_blake3: &str,
    wait: crate::classify::WaitStatus,
    sha256: Sha256Fn,
    licence_present: bool,
) -> crate::classify::WaitStatus {
    use crate::classify::WaitStatus;
    use crate::exit::ExitCode;
    use crate::outcome::{
        EndKind, ErrorClass, ErrorInfo, ExtraOutput, JobOutcome, Outcome, ProgramInfo, StopInfo,
    };
    use crate::stop::StopCode;
    if spec.kind != JobKind::SymmetryComposite || matches!(wait, WaitStatus::Unknown) {
        return wait;
    }
    let mapped = legacy_code(wait);
    if layout.outcome().exists() {
        return mapped;
    }
    let program = ProgramInfo {
        name: spec.program.name.clone(),
        version: "legacy".into(),
        git_commit: crate::UNKNOWN_COMMIT.into(),
        features: std::iter::once("legacy-wrapped".to_string())
            .chain(spec.program.features_required.iter().cloned())
            .collect(),
        exe: None,
    };
    let write = |j: JobOutcome| {
        if let Err(e) = Outcome::new(j).write_new(&layout.outcome()) {
            eprintln!("symmetry-composite wrapper: OUTCOME: {e}");
        }
    };
    let refusal = |why: &str| {
        let _ = std::fs::write(layout.sup_dir().join(REFUSAL_FILE), why);
    };
    let errored = |code: ExitCode, stop: StopCode, class: ErrorClass, msg: String| {
        let mut j = JobOutcome::for_spec(
            spec,
            spec_blake3,
            program.clone(),
            code,
            StopInfo {
                code: Some(stop),
                ..StopInfo::none()
            },
        );
        j.error = Some(ErrorInfo::new(class, msg));
        j
    };
    match mapped {
        WaitStatus::Exited(0) => match check_child(spec, sha256) {
            Ok(seal) => {
                let unsolved = seal.unsolved();
                let (code, stop) = if unsolved == 0 {
                    (ExitCode::Complete, StopInfo::none())
                } else {
                    (
                        ExitCode::Incomplete,
                        StopInfo {
                            code: Some(StopCode::KeysUnsolved),
                            ..StopInfo::none()
                        },
                    )
                };
                let mut j = JobOutcome::for_spec(spec, spec_blake3, program, code, stop);
                j.counters.cases = seal.keys.len() as u64;
                j.counters.rules_committed = seal.relations.len() as u64;
                if let Ok((_, req)) = request_of(spec) {
                    j.effective = effective(
                        &SymmetryCompositeNodeOptions::of_request(&req, &seal.request_sha256),
                        licence_present,
                    )
                    .ok();
                }
                let sealed = (|| -> Result<_, String> {
                    let mut w = crate::seal::RecordWriter::create(&layout.results())
                        .map_err(|e| e.to_string())?;
                    for r in results_records(&seal) {
                        let l = crate::canon::canonical_line(r.to_string().as_bytes())
                            .map_err(|e| e.to_string())?;
                        w.append(&l).map_err(|e| e.to_string())?;
                    }
                    w.seal(end_record(&seal).to_string().as_bytes())
                        .map_err(|e| e.to_string())
                })();
                match sealed {
                    Ok(s) => j.outputs.extra.push(ExtraOutput {
                        role: "results".into(),
                        path: crate::layout::RunLayout::out_rel(crate::layout::RESULTS),
                        bytes: s.bytes,
                        blake3: s.blake3,
                        records: Some(s.records),
                        canonical_blake3: Some(s.canonical_blake3),
                        end: Some(EndKind::End),
                        sealed: true,
                    }),
                    Err(e) => {
                        refusal(&format!("sealing failed: {e}"));
                        eprintln!("symmetry-composite wrapper: sealing failed: {e}");
                    }
                }
                write(j);
                WaitStatus::Exited(code.code())
            }
            Err(ChildRefusal::PinnedInputs(m)) => {
                let why = format!("{PINNED_MISMATCH}: {m}");
                refusal(&why);
                write(errored(
                    ExitCode::Input,
                    StopCode::InputError,
                    ErrorClass::Input,
                    why,
                ));
                WaitStatus::Exited(ExitCode::Input.code())
            }
            Err(ChildRefusal::Unsealed(m)) => {
                // the child's result is not sealed / not consistent: an OUTCOME without a sealed stream (row 22)
                refusal(&m);
                write(JobOutcome::for_spec(
                    spec,
                    spec_blake3,
                    program,
                    ExitCode::Complete,
                    StopInfo::none(),
                ));
                mapped
            }
        },
        WaitStatus::Exited(41) => {
            let msg = std::fs::read_to_string(layout.stdout_log())
                .ok()
                .and_then(|s| s.lines().last().map(str::to_string))
                .unwrap_or_else(|| "rr_rf_symflow exit 2".into());
            let j = errored(ExitCode::Input, StopCode::InputError, ErrorClass::Input, msg.clone());
            // row 15b: a refusal naming a HOST resource errno is a resource failure (exit 51), never an input refusal
            if crate::classify::resource_errno_of(&j).is_some() {
                write(errored(ExitCode::Io, StopCode::IoError, ErrorClass::Io, msg));
                return WaitStatus::Exited(ExitCode::Io.code());
            }
            write(j);
            mapped
        }
        w => w,
    }
}
