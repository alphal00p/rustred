//! The rr_proof kinds run as LEGACY children UNDER THE SUPERVISOR (lane closeloop2; interfaces/closeloop.md 13.1;
//! child-job.md R-d): `proof-instance-span` (`rr_proof --mode instance-span`, one rule record at many keys) and
//! `proof-instantiate` (`rr_proof instantiate`, exact facts). rr_proof has no job mode: the argv is DERIVED FROM THE
//! SPEC'S INPUTS ([`argv`]), and after the launcher reaped the child the supervisor calls [`wrap`] BEFORE the contract
//! classifies the attempt: legacy exit 0 -> every answer is checked against the REQUEST (record_sha256 + rule echo +
//! one of the requested keys; every requested (record, key) exactly once) and only then `out/results.jsonl` is sealed
//! (one canonical line per answer + the end record) and OUTCOME written; a failed check writes OUTCOME WITHOUT a sealed
//! stream (row 22: nothing accepted); legacy exit 2 -> 41 + an error OUTCOME (row 13); anything else stays as it is
//! (no OUTCOME: rows 18-21, 23). The answers then reach the work source only through the contract's VerifiedHandle.
//!
//! Generic (L8): no node, arity or path is known here; the arity of a key is the length of the requested key.

use super::proof::{
    ANSWER_SCHEMA as INSTANTIATE_SCHEMA, CHILD_OUT as INSTANTIATE_OUT, legacy_wait,
};
use rustred_campaign_contract::classify::WaitStatus;
use rustred_campaign_contract::exit::ExitCode;
use rustred_campaign_contract::layout::{RESULTS, RunLayout};
use rustred_campaign_contract::outcome::{
    EndKind, ErrorClass, ErrorInfo, ExtraOutput, JobOutcome, Outcome, ProgramInfo, StopInfo,
};
use rustred_campaign_contract::seal::RecordWriter;
use rustred_campaign_contract::spec::{InputRef, JobKind, JobSpec};
use rustred_campaign_contract::stop::StopCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Schema of the `request` input of a `proof-instance-span` job.
pub const SPAN_REQUEST_SCHEMA: &str = "rustred.campaign.instance-span-job.v1";
/// The child's output file in `out/` for kind `proof-instance-span`.
pub const SPAN_OUT: &str = "span.jsonl";
/// `schema` of an rr_proof instance-span record.
pub const SPAN_SCHEMA: &str = "rustred.proof-kernel.span.v1";
/// The child's output file in `out/` for kind `coefficient-facts` (lane closeloop3; closeloop.md 16).
pub const COEF_OUT: &str = "coefficient_facts.jsonl";
/// lane zeros2: schema of the per-(record, key) answer of a zeros request (closeloop.md 23).
pub const ZERO_VERDICT_SCHEMA: &str = "rustred.campaign.zero-verdict.v1";
/// `producer` every `coefficient-facts` answer must state (rr_coef_facts: exact evaluation of the record's own text).
pub const COEF_PRODUCER: &str = "exact_eval";

/// The `request` input of a `proof-instance-span` job (what the record is and how to prove it).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpanRequest {
    pub schema: String,
    /// The record's `index` (rr_proof `--rule`).
    pub rule: i64,
    /// Byte offset of the record line in the `table` input.
    pub offset: u64,
    /// SHA-256 (hex) of the raw record line without its newline (rr_proof's answer echo).
    pub record_sha256: String,
    /// `engine` (rows from the `family` input) | `cert` (rows from the `cert` input).
    pub rows: String,
    #[serde(default)]
    pub drop_zero_sectors: bool,
    /// `auto` | `on` | `off` (None: rr_proof's default).
    #[serde(default)]
    pub symmetry: Option<String>,
    #[serde(default)]
    pub max_primes: Option<u32>,
}

/// Path of an input of the spec (relative paths are relative to the run directory).
fn input_path(spec: &JobSpec, role: &str) -> Option<PathBuf> {
    spec.inputs.iter().find(|i| i.role == role).map(|i| {
        let p = PathBuf::from(&i.path);
        if p.is_absolute() {
            p
        } else {
            Path::new(&spec.run_dir).join(p)
        }
    })
}

fn input_ref<'a>(spec: &'a JobSpec, role: &str) -> Option<&'a InputRef> {
    spec.inputs.iter().find(|i| i.role == role)
}

/// The bytes of an input, its BLAKE3 checked against the spec (a changed input is refused, never re-read silently).
fn input_bytes(spec: &JobSpec, role: &str) -> Result<Vec<u8>, String> {
    let r = input_ref(spec, role).ok_or_else(|| format!("no input of role {role}"))?;
    let p = input_path(spec, role).expect("present");
    let b = std::fs::read(&p).map_err(|e| format!("input {role} {}: {e}", p.display()))?;
    let d = rustred_campaign_contract::blake3_hex(&b);
    if d != r.blake3 {
        return Err(format!(
            "input {role} {} changed (blake3 {d}, spec {})",
            p.display(),
            r.blake3
        ));
    }
    Ok(b)
}

/// The span request of a `proof-instance-span` spec.
pub fn span_request(spec: &JobSpec) -> Result<SpanRequest, String> {
    let r: SpanRequest = serde_json::from_slice(&input_bytes(spec, "request")?)
        .map_err(|e| format!("request: {e}"))?;
    if r.schema != SPAN_REQUEST_SCHEMA {
        return Err(format!("request schema {:?}", r.schema));
    }
    if !matches!(r.rows.as_str(), "engine" | "cert") {
        return Err(format!("request rows {:?} (engine | cert)", r.rows));
    }
    Ok(r)
}

/// One key in rr_proof's `n0=..,n1=..` form (every slot named).
pub fn key_text_named(k: &[i64]) -> String {
    k.iter()
        .enumerate()
        .map(|(i, v)| format!("n{i}={v}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Parse a `n0=..,..` key (every slot 0..n named exactly once).
pub fn parse_key_named(t: &str) -> Option<Vec<i64>> {
    let mut m = BTreeMap::new();
    for part in t.trim().split(',') {
        let (n, v) = part.trim().split_once('=')?;
        let i: usize = n.trim().strip_prefix('n')?.parse().ok()?;
        if m.insert(i, v.trim().parse::<i64>().ok()?).is_some() {
            return None;
        }
    }
    let n = m.len();
    (0..n)
        .map(|i| m.get(&i).copied())
        .collect::<Option<Vec<i64>>>()
}

/// The requested keys of a `proof-instance-span` spec (its `keys` input, `#` comments and blank lines skipped).
pub fn span_keys(spec: &JobSpec) -> Result<Vec<Vec<i64>>, String> {
    let b = input_bytes(spec, "keys")?;
    let mut v = vec![];
    for (i, l) in String::from_utf8_lossy(&b).lines().enumerate() {
        let t = l.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        v.push(parse_key_named(t).ok_or_else(|| format!("keys line {}: {t:?}", i + 1))?);
    }
    if v.is_empty() {
        return Err("no key requested".into());
    }
    Ok(v)
}

/// The argv of a legacy-wrapped kind, derived from the spec ONLY (cwd = the run directory).
pub fn argv(spec: &JobSpec) -> Result<Vec<String>, String> {
    let p = |role: &str| {
        input_path(spec, role)
            .map(|x| x.display().to_string())
            .ok_or_else(|| format!("no input of role {role}"))
    };
    match spec.kind {
        JobKind::ProofInstanceSpan => {
            let r = span_request(spec)?;
            let mut a = vec![
                "--mode".to_string(),
                "instance-span".into(),
                "--table".into(),
                p("table")?,
                "--rule".into(),
                r.rule.to_string(),
                "--offset".into(),
                r.offset.to_string(),
                "--keys".into(),
                p("keys")?,
                "--rows".into(),
                r.rows.clone(),
            ];
            if r.rows == "engine" {
                a.extend(["--family".into(), p("family")?]);
            } else {
                a.extend(["--cert".into(), p("cert")?]);
            }
            if r.drop_zero_sectors {
                a.push("--drop-zero-sectors".into());
            }
            if let Some(s) = &r.symmetry {
                a.extend(["--symmetry".into(), s.clone()]);
            }
            if input_ref(spec, "symmetry_group").is_some() {
                a.extend(["--symmetry-group".into(), p("symmetry_group")?]);
            }
            if let Some(m) = r.max_primes {
                a.extend(["--max-primes".into(), m.to_string()]);
            }
            a.extend(["--out".into(), format!("out/{SPAN_OUT}")]);
            Ok(a)
        }
        JobKind::ProofInstantiate => {
            let mut a = vec![
                "instantiate".into(),
                "--requests".into(),
                p("requests")?,
                "--out".into(),
                format!("out/{INSTANTIATE_OUT}"),
            ];
            // lane closeloop4 (line 7, coord-ec2-instantiate.md 2d): `--elided-loci facts` passed through when the
            // spec's args name it (allow-list of the kind); absent = rr_proof's default `refuse` (R-f)
            if spec.elided_loci() == rustred_campaign_contract::spec::ElidedLoci::Facts {
                a.extend(["--elided-loci".into(), "facts".into()]);
            }
            Ok(a)
        }
        // lane closeloop3: rr_coef_facts reads the same request form (A)
        JobKind::CoefficientFacts => Ok(vec![
            "facts".into(),
            "--requests".into(),
            p("requests")?,
            "--out".into(),
            format!("out/{COEF_OUT}"),
        ]),
        k => Err(format!("kind {k:?} is not legacy-wrapped")),
    }
}

/// SHA-256 (hex) of the line at `offset` of `path` (without its newline) and its `index`.
pub fn record_at(path: &Path, offset: u64) -> Result<(String, Option<i64>), String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    f.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut line = Vec::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if let Some(p) = buf[..n].iter().position(|&c| c == b'\n') {
            line.extend_from_slice(&buf[..p]);
            break;
        }
        line.extend_from_slice(&buf[..n]);
    }
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(&line);
    let idx = serde_json::from_slice::<Value>(&line)
        .ok()
        .and_then(|v| v["index"].as_i64());
    Ok((h.finish_hex(), idx))
}

/// A key object of an answer (`{"n0":..}` with every slot, or an array) of the requested arity.
fn answer_key(v: &Value, arity: usize) -> Option<Vec<i64>> {
    match v {
        Value::Array(a) => {
            let k: Vec<i64> = a.iter().map(Value::as_i64).collect::<Option<_>>()?;
            (k.len() == arity).then_some(k)
        }
        Value::Object(o) => {
            if o.len() != arity {
                return None;
            }
            (0..arity)
                .map(|i| o.get(&format!("n{i}")).and_then(Value::as_i64))
                .collect()
        }
        Value::String(s) => parse_key_named(s).filter(|k| k.len() == arity),
        _ => None,
    }
}

/// What a request asks: (record_sha256, key) -> rule index.
type Wanted = BTreeMap<(String, Vec<i64>), i64>;

/// The checked answers of `proof-instance-span`: one canonical compact line per (record, key) (import.md 13.2 form:
/// record_sha256, key array, verdict word; plus rule, status text, mode).
fn check_span(spec: &JobSpec, out: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let r = span_request(spec)?;
    let keys = span_keys(spec)?;
    let arity = keys[0].len();
    let mut want: Wanted = BTreeMap::new();
    for k in &keys {
        want.insert((r.record_sha256.clone(), k.clone()), r.rule);
    }
    let recs: Vec<&[u8]> = out
        .split(|&c| c == b'\n')
        .filter(|l| !l.iter().all(u8::is_ascii_whitespace))
        .collect();
    let [rec] = recs.as_slice() else {
        return Err(format!(
            "{} records in out/{SPAN_OUT}, expected one",
            recs.len()
        ));
    };
    let v: Value = serde_json::from_slice(rec).map_err(|e| format!("span record: {e}"))?;
    if v["schema"] != SPAN_SCHEMA {
        return Err(format!("span record schema {}", v["schema"]));
    }
    if v["record_sha256"].as_str() != Some(r.record_sha256.as_str()) {
        return Err(format!(
            "span record about record {} (requested {})",
            v["record_sha256"], r.record_sha256
        ));
    }
    if v["rule"].as_i64() != Some(r.rule) {
        return Err(format!(
            "span record about rule {} (requested {})",
            v["rule"], r.rule
        ));
    }
    let entries = v["keys"].as_array().ok_or("span record without keys")?;
    let mut seen = BTreeSet::new();
    let mut lines = vec![];
    for (i, e) in entries.iter().enumerate() {
        let sha = e["record_sha256"].as_str().unwrap_or_default().to_string();
        let key = answer_key(&e["key"], arity).ok_or_else(|| format!("entry {i}: key"))?;
        let id = (sha.clone(), key.clone());
        let Some(&rule) = want.get(&id) else {
            return Err(format!(
                "entry {i}: an answer for a record / key no request asked for ({sha}, {key:?})"
            ));
        };
        if e["rule"].as_i64() != Some(rule) {
            return Err(format!("entry {i}: rule echo {} is not {rule}", e["rule"]));
        }
        if !seen.insert(id) {
            return Err(format!("entry {i}: (record, key) answered twice"));
        }
        let status = e["status"].as_str().unwrap_or("").to_string();
        let word = status.split_whitespace().next().unwrap_or("").to_string();
        let l = json!({"record_sha256": sha, "rule": rule, "key": key, "verdict": word, "status": status,
                       "mode": "instance-span", "seconds": e["seconds"].clone()});
        lines.push(
            rustred_campaign_contract::canon::canonical_line(l.to_string().as_bytes())
                .map_err(|e| e.to_string())?,
        );
    }
    if seen.len() != want.len() {
        return Err(format!(
            "{} of {} requested keys answered",
            seen.len(),
            want.len()
        ));
    }
    Ok(lines)
}

/// The checked answers of `proof-instantiate` (form (A) requests; record_sha256 recomputed from the table at the
/// request's offset, its index checked): the answer lines, canonical. `coefficient-facts` (lane closeloop3) is
/// checked the same way, with its own answer schema (`rustred.campaign.fact-verdict.v1`) and `producer: exact_eval`.
fn check_instantiate(spec: &JobSpec, out: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let coef = spec.kind == JobKind::CoefficientFacts;
    let schema = if coef {
        rustred_campaign_core::import::evidence::FACT_VERDICT_SCHEMA
    } else {
        INSTANTIATE_SCHEMA
    };
    let req = input_bytes(spec, "requests")?;
    let mut want: Wanted = BTreeMap::new();
    let mut shas: BTreeMap<(String, u64), String> = BTreeMap::new();
    // lane zeros2 (closeloop.md 23): a coefficient-facts request with `terms` is a ZEROS request: its answers are zero
    // verdict lines (the asked terms of each key) and structural proof lines of its records
    let mut zeros_terms: BTreeMap<(String, Vec<i64>), Vec<u64>> = BTreeMap::new();
    let mut zeros_records: BTreeMap<String, i64> = BTreeMap::new();
    for (i, l) in String::from_utf8_lossy(&req).lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(l).map_err(|e| format!("request {}: {e}", i + 1))?;
        let (Some(table), Some(offset), Some(rule)) = (
            v["table"].as_str(),
            v["offset"].as_u64(),
            v["rule"].as_i64(),
        ) else {
            return Err(format!("request {}: table / offset / rule", i + 1));
        };
        let sha = match shas.get(&(table.to_string(), offset)) {
            Some(s) => s.clone(),
            None => {
                let (s, idx) = record_at(Path::new(table), offset)?;
                if idx != Some(rule) {
                    return Err(format!(
                        "request {}: the record at offset {offset} of {table} is not rule {rule}",
                        i + 1
                    ));
                }
                shas.insert((table.to_string(), offset), s.clone());
                s
            }
        };
        let keys: Vec<Vec<i64>> = serde_json::from_value(v["keys"].clone())
            .map_err(|e| format!("request {}: keys: {e}", i + 1))?;
        if coef && v.get("terms").is_some() {
            let terms: Vec<Vec<u64>> = serde_json::from_value(v["terms"].clone())
                .map_err(|e| format!("request {}: terms: {e}", i + 1))?;
            if terms.len() != keys.len() {
                return Err(format!("request {}: terms not parallel to keys", i + 1));
            }
            zeros_records.insert(sha.clone(), rule);
            for (k, ts) in keys.iter().zip(terms) {
                zeros_terms.insert((sha.clone(), k.clone()), ts);
            }
        }
        for k in keys {
            want.insert((sha.clone(), k), rule);
        }
    }
    let mut seen = BTreeSet::new();
    let mut lines = vec![];
    for (i, l) in out.split(|&c| c == b'\n').enumerate() {
        if l.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let v: Value = serde_json::from_slice(l).map_err(|e| format!("answer {}: {e}", i + 1))?;
        // lane zeros2: a structural proof line of a record a zeros request names (no key: not a pair answer)
        if coef && v["schema"] == rustred_campaign_core::import::zero_proofs::ZERO_PROOF_SCHEMA {
            let sha = v["record_sha256"].as_str().unwrap_or("");
            if zeros_records.get(sha).copied() != v["rule"].as_i64() || v["rule"].is_null() {
                return Err(format!(
                    "answer {}: a proof line of record {sha} / rule {} no zeros request names",
                    i + 1,
                    v["rule"]
                ));
            }
            lines.push(
                rustred_campaign_contract::canon::canonical_line(l).map_err(|e| e.to_string())?,
            );
            continue;
        }
        let zeros_line = coef && v["schema"] == ZERO_VERDICT_SCHEMA;
        if v["schema"] != schema && !zeros_line {
            return Err(format!("answer {}: schema {}", i + 1, v["schema"]));
        }
        if !coef {
            // the ONE elided-loci rule: the contract's (line 7, coord-ec2-instantiate.md 2d)
            rustred_campaign_contract::instantiate::instantiate_answer_use(&v, spec.elided_loci())
                .map_err(|m| format!("answer {}: {m}", i + 1))?;
        }
        if coef && v["producer"] != COEF_PRODUCER {
            return Err(format!(
                "answer {}: producer {} (expected {COEF_PRODUCER})",
                i + 1,
                v["producer"]
            ));
        }
        let sha = v["record_sha256"]
            .as_str()
            .ok_or_else(|| format!("answer {}: no record_sha256 echo", i + 1))?
            .to_string();
        let key: Vec<i64> = serde_json::from_value(v["key"].clone())
            .map_err(|e| format!("answer {}: key: {e}", i + 1))?;
        if coef {
            // a zeros request is answered by zero verdict lines of exactly its asked terms, an ordinary one by facts
            match zeros_terms.get(&(sha.clone(), key.clone())) {
                Some(asked) => {
                    let got: Vec<u64> = v["terms"]
                        .as_array()
                        .map(|a| a.iter().filter_map(|x| x[0].as_u64()).collect())
                        .unwrap_or_default();
                    if !zeros_line || &got != asked {
                        return Err(format!(
                            "answer {}: a zeros request needs a zero verdict line of its asked terms",
                            i + 1
                        ));
                    }
                }
                None if zeros_line => {
                    return Err(format!(
                        "answer {}: a zero verdict line for a facts request",
                        i + 1
                    ));
                }
                None => {}
            }
        }
        let id = (sha.clone(), key);
        let Some(&rule) = want.get(&id) else {
            return Err(format!(
                "answer {}: a record / key no request asked for",
                i + 1
            ));
        };
        if v["rule"].as_i64() != Some(rule) {
            return Err(format!("answer {}: rule echo {}", i + 1, v["rule"]));
        }
        if !seen.insert(id) {
            return Err(format!("answer {}: (record, key) answered twice", i + 1));
        }
        lines.push(rustred_campaign_contract::canon::canonical_line(l).map_err(|e| e.to_string())?);
    }
    if seen.len() != want.len() {
        return Err(format!(
            "{} of {} requested answers present",
            seen.len(),
            want.len()
        ));
    }
    Ok(lines)
}

/// The child's raw output file of a legacy-wrapped kind.
pub fn child_out(kind: JobKind) -> &'static str {
    match kind {
        JobKind::ProofInstanceSpan => SPAN_OUT,
        JobKind::CoefficientFacts => COEF_OUT,
        _ => INSTANTIATE_OUT,
    }
}

/// THE WRAPPER (called by the supervisor's settle BEFORE the contract classifies; a no-op for other kinds, for an
/// unknown wait status and when OUTCOME already exists, e.g. written before a supervisor crash). Returns the wait
/// status the contract's table must see.
pub fn wrap(layout: &RunLayout, spec: &JobSpec, spec_blake3: &str, wait: WaitStatus) -> WaitStatus {
    if !spec.kind.legacy_wrapped() || matches!(wait, WaitStatus::Unknown) {
        return wait;
    }
    let mapped = legacy_wait(wait);
    if layout.outcome().exists() {
        return mapped;
    }
    let program = ProgramInfo {
        name: spec.program.name.clone(),
        version: "legacy".into(),
        git_commit: rustred_campaign_contract::UNKNOWN_COMMIT.into(),
        features: {
            let mut f = vec!["legacy-wrapped".to_string()];
            f.extend(spec.program.features_required.iter().cloned());
            f
        },
        exe: None,
    };
    let write = |j: JobOutcome| {
        if let Err(e) = Outcome::new(j).write_new(&layout.outcome()) {
            eprintln!("legacy wrapper: OUTCOME: {e}");
        }
    };
    match mapped {
        WaitStatus::Exited(0) => {
            let raw =
                std::fs::read(layout.out_dir().join(child_out(spec.kind))).unwrap_or_default();
            let mut j = JobOutcome::for_spec(
                spec,
                spec_blake3,
                program,
                ExitCode::Complete,
                StopInfo::none(),
            );
            let checked = match spec.kind {
                JobKind::ProofInstanceSpan => check_span(spec, &raw),
                _ => check_instantiate(spec, &raw),
            };
            match checked {
                Ok(lines) => {
                    j.counters.cases = lines.len() as u64;
                    let req_role = if spec.kind == JobKind::ProofInstanceSpan {
                        "request"
                    } else {
                        "requests"
                    };
                    let req_b3 = input_ref(spec, req_role).map(|i| i.blake3.clone());
                    let sealed = (|| -> Result<_, String> {
                        let mut w =
                            RecordWriter::create(&layout.results()).map_err(|e| e.to_string())?;
                        for l in &lines {
                            w.append(l).map_err(|e| e.to_string())?;
                        }
                        let end = json!({"type": "end", "records": lines.len(), "answers": lines.len(),
                                         "requests_blake3": req_b3});
                        w.seal(end.to_string().as_bytes())
                            .map_err(|e| e.to_string())
                    })();
                    match sealed {
                        Ok(s) => j.outputs.extra.push(ExtraOutput {
                            role: "results".into(),
                            path: RunLayout::out_rel(RESULTS),
                            bytes: s.bytes,
                            blake3: s.blake3,
                            records: Some(s.records),
                            canonical_blake3: Some(s.canonical_blake3),
                            end: Some(EndKind::End),
                            sealed: true,
                        }),
                        Err(e) => eprintln!("legacy wrapper: sealing failed: {e}"),
                    }
                    write(j);
                }
                Err(why) => {
                    // the answers do not check against the request: OUTCOME without a sealed stream (row 22)
                    eprintln!("legacy wrapper: answers refused: {why}");
                    let _ = std::fs::write(layout.sup_dir().join("wrapper-refusal.txt"), &why);
                    write(j);
                }
            }
            mapped
        }
        WaitStatus::Exited(41) => {
            let mut j = JobOutcome::for_spec(
                spec,
                spec_blake3,
                program,
                ExitCode::Input,
                StopInfo {
                    code: Some(StopCode::InputError),
                    ..StopInfo::none()
                },
            );
            let msg = std::fs::read_to_string(layout.stdout_log())
                .ok()
                .and_then(|s| s.lines().last().map(str::to_string))
                .unwrap_or_else(|| "rr_proof exit 2".into());
            j.error = Some(ErrorInfo::new(ErrorClass::Input, msg.clone()));
            // decision row 15b (lane int5): a refusal that names a HOST resource errno is not an input error: exit 51
            // (io) with that message -> lost, retried after back-off, then attention; never evidence about the record
            if rustred_campaign_contract::classify::resource_errno_of(&j).is_some() {
                let mut io = JobOutcome::for_spec(
                    spec,
                    spec_blake3,
                    j.program.clone(),
                    ExitCode::Io,
                    StopInfo {
                        code: Some(StopCode::IoError),
                        ..StopInfo::none()
                    },
                );
                io.error = Some(ErrorInfo::new(ErrorClass::Io, msg));
                write(io);
                return WaitStatus::Exited(ExitCode::Io.code());
            }
            write(j);
            mapped
        }
        w => w,
    }
}

/// Why the wrapper refused an attempt's answers (`RUN/sup/wrapper-refusal.txt`), if it did.
pub fn refusal(layout: &RunLayout) -> Option<String> {
    std::fs::read_to_string(layout.sup_dir().join("wrapper-refusal.txt")).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A prepared proof-instance-span attempt in a fresh temp directory (no process: the child's ending is written by
    /// hand): its layout, spec and spec digest.
    fn attempt(tag: &str) -> (std::path::PathBuf, RunLayout, JobSpec, String) {
        use crate::supervisor::worksource::JobRequest;
        let root = std::env::temp_dir().join(format!("cl2-legacy-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let file = |name: &str, body: &str, role: &str| {
            let p = root.join(name);
            std::fs::write(&p, body).unwrap();
            InputRef {
                role: role.into(),
                path: p.display().to_string(),
                blake3: rustred_campaign_contract::blake3_hex(body.as_bytes()),
                upto: None,
            }
        };
        let req = format!(
            "{}\n",
            json!({"schema": SPAN_REQUEST_SCHEMA, "rule": 0, "offset": 0, "record_sha256": "aa", "rows": "engine"})
        );
        let r = JobRequest {
            job: "n.p1.0-t0".into(),
            item: "n.p1.0".into(),
            rung: crate::state::Rung(0),
            kind: JobKind::ProofInstanceSpan,
            priority: 0,
            cap_bytes: 1 << 28,
            expected_peak_bytes: 1 << 26,
            cores: 1,
            heartbeat_s: 10,
            stop_grace_s: 5,
            program: "rr_proof".into(),
            args: vec![],
            inputs: vec![
                file("request.json", &req, "request"),
                file("t.jsonl", "{\"type\":\"rule\",\"index\":0}\n", "table"),
                file("keys.txt", "n0=1,n1=1\n", "keys"),
                file("fam.sym", "family\n", "family"),
            ],
            depends_on: vec![],
            budgets: Default::default(),
            sample_seed: 0,
            fake: None,
        };
        let run = root.join("run");
        let prog = root.join("rr_proof");
        std::fs::write(&prog, "#!/bin/sh\n").unwrap();
        let spec = super::super::job_spec(
            super::super::AdapterKind::Contract,
            &r,
            &super::super::AdapterInput {
                run_dir: &run,
                attempt: 1,
                program_name: "rr_proof",
                program_path: &prog,
                program_blake3: "00",
                resume: None,
            },
        );
        let (_, digest) = super::super::prepare_run(&spec, &prog).unwrap();
        let layout = RunLayout::new(&spec.run_dir);
        (root, layout, spec, digest)
    }

    /// Decision row 15b (lane int5): an rr_proof refusal (exit 2) whose ERROR names a HOST resource errno becomes an
    /// io end (51) the contract classifies as a resource failure (row 15: lost, retried after back-off): never an
    /// input refusal of the record (row 13). A plain refusal stays 41 / row 13.
    #[test]
    fn a_host_resource_refusal_is_row_15_never_an_input_refusal() {
        use rustred_campaign_contract::classify::{KillReason, assess, classify};
        let (root, layout, spec, digest) = attempt("res");
        std::fs::write(
            layout.stdout_log(),
            "{\"verdict\":\"ERROR\",\"error\":\"open table: Too many open files (os error 24)\"}\n",
        )
        .unwrap();
        let w = wrap(&layout, &spec, &digest, WaitStatus::Exited(2));
        assert_eq!(w, WaitStatus::Exited(51));
        let c = classify(&assess(&layout, &spec, &digest, w, KillReason::None, false));
        assert_eq!((c.row, c.sub.as_str()), (15, "b"), "{}", c.why);
        let _ = std::fs::remove_dir_all(&root);
        let (root, layout, spec, digest) = attempt("inp");
        std::fs::write(
            layout.stdout_log(),
            "{\"verdict\":\"MISSING_SEEDS\",\"error\":\"the record carries no seeds\"}\n",
        )
        .unwrap();
        let w = wrap(&layout, &spec, &digest, WaitStatus::Exited(2));
        assert_eq!(w, WaitStatus::Exited(41));
        let c = classify(&assess(&layout, &spec, &digest, w, KillReason::None, false));
        assert_eq!(c.row, 13, "{}", c.why);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn named_keys_round_trip() {
        let k = vec![1, 2, 0, -1];
        assert_eq!(key_text_named(&k), "n0=1,n1=2,n2=0,n3=-1");
        assert_eq!(parse_key_named(&key_text_named(&k)), Some(k));
        assert_eq!(parse_key_named("n0=1,n2=3"), None);
        assert_eq!(parse_key_named("n0=1,n0=3"), None);
        assert_eq!(answer_key(&json!({"n0":1,"n1":2}), 2), Some(vec![1, 2]));
        assert_eq!(answer_key(&json!({"n0":1}), 2), None);
    }
}
