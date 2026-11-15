//! Kind `proof-symrows` (lane symcert; interfaces/proof-symrows-job.md): rr_proof `--mode instance-span --symrows FILE
//! [--symrows-only]` (interfaces/proof-symrows.md) run as a LEGACY child. ONE relation per job: the supervisor writes
//! the relation as a point-rule record (pinned `table`), its key (`keys`), the symmetry rows that derive it (`symrows`:
//! seed + momentum matrix per row) and the pinned `request`; the kernel answers whether the relation vector at the key
//! is in the span of those rows (plus the IBP rows at the record's seeds unless `symrows_only`). The supervisor-side
//! wrapper checks the kernel's record against the request and seals ONE certificate line per requested key
//! (`out/results.jsonl`); THIS module also re-checks every sealed certificate against the pinned request and table in
//! [`crate::classify::assess`] (S of the kind), so a certificate with a valid seal that does not echo the asked
//! relation is refused (row 43) even when a child wrote its own OUTCOME.
//!
//! Everything here is pure (bytes in, verdicts out) plus plain reads of the spec's pinned inputs. Generic (L8): a key
//! is an integer vector of any length; no node, arity or path is known here. The contract crate has no SHA-256: the
//! table line is bound by its BLAKE3 here and by its SHA-256 (rr_proof's echo) in the supervisor's wrapper.

use crate::spec::JobSpec;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

/// `schema` of the `request` input (written by the supervisor).
pub const REQUEST_SCHEMA: &str = "rustred.campaign.proof-symrows-job.v1";
/// `schema` of a sealed certificate line (written by the wrapper).
pub const CERT_SCHEMA: &str = "rustred.campaign.symrows-certificate.v1";
/// `schema` of the canonical relation object whose BLAKE3 is the relation digest.
pub const RELATION_SCHEMA: &str = "rustred.campaign.symrows-relation.v1";
/// `schema` of rr_proof's instance-span record.
pub const SPAN_SCHEMA: &str = "rustred.proof-kernel.span.v1";
/// The child's raw output file in `out/`.
pub const CHILD_OUT: &str = "span.jsonl";
/// The kernel's typed refusal of a symmetry row (interfaces/proof-symrows.md "Implemented").
pub const SYMROW_REFUSED: &str = "SYMROW_REFUSED";
/// The certificate classes (closed list).
pub const CLASSES: [&str; 4] = ["certified", "kernel_refused", "undecided", "not_applicable"];

/// A catalogue rule whose IBP rows (at its seeds, at its term) enter the relation's record as absolute seeds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogueUse {
    pub source: String,
    pub label: Value,
    pub record_sha256: String,
}

/// The zero-sector census written into the table (when the producer pruned zero-sector terms).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZeroCensus {
    /// Who computed the census (e.g. `campaign-core lee-criterion`, `rr_node symface`).
    pub producer: String,
    pub masks: u64,
    /// BLAKE3 of the census record line as written into the table.
    pub blake3: String,
}

/// The `request` input of a `proof-symrows` job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SymrowsRequest {
    pub schema: String,
    /// [`relation_digest`] of the relation the job is about.
    pub relation_digest: String,
    /// The record's `index` (rr_proof `--rule`).
    pub rule: i64,
    /// Byte offset of the record line in the `table` input.
    pub offset: u64,
    /// SHA-256 (hex) of the record line without its newline (rr_proof's echo).
    pub record_sha256: String,
    /// BLAKE3 (hex) of the same line (checked by the contract).
    pub record_blake3: String,
    /// The key the relation is certified at.
    pub key: Vec<i64>,
    /// Which producer shipped the relation (free text, e.g. `closeloop:symmetry-at-key`).
    pub producer: String,
    /// SHA-256 of the producer's own record line of the relation (the import binds evidence to it), if any.
    #[serde(default)]
    pub producer_record_sha256: Option<String>,
    /// Digest of the producer's verified symmetry group the rows' matrices were taken from.
    #[serde(default)]
    pub group_digest: Option<String>,
    /// Element indices (into that group) of the symmetry rows, in symrows order.
    pub elements: Vec<u64>,
    /// Number of non-empty lines of the `symrows` input.
    pub symrows_lines: u64,
    /// `--symrows-only`: no IBP rows.
    pub symrows_only: bool,
    /// `engine` (IBP rows from the family: `--rows engine`) | `none` (`--rows cert` without a certificate: no IBP rows).
    pub rows: String,
    #[serde(default)]
    pub drop_zero_sectors: bool,
    #[serde(default)]
    pub zero_census: Option<ZeroCensus>,
    /// Catalogue rules whose seeds are in the record (composites).
    #[serde(default)]
    pub catalogue: Vec<CatalogueUse>,
    /// SHA-256 of the kernel binary the supervisor launches.
    pub kernel_sha256: String,
    /// BLAKE3 of the presented family text written as the table header's `family_source` (the frame the keys, the
    /// rows' momenta and the IBP rows are in): binds the certificate to that family.
    pub family_blake3: String,
    #[serde(default)]
    pub max_primes: Option<u32>,
}

impl SymrowsRequest {
    /// Internal consistency (refused requests are never launched).
    pub fn check(&self) -> Result<(), String> {
        if self.schema != REQUEST_SCHEMA {
            return Err(format!("request schema {:?}", self.schema));
        }
        if !matches!(self.rows.as_str(), "engine" | "none") {
            return Err(format!("request rows {:?} (engine | none)", self.rows));
        }
        if self.rows == "none" && !self.symrows_only {
            return Err("rows none needs symrows_only (no IBP rows at all)".into());
        }
        if self.key.is_empty() {
            return Err("empty key".into());
        }
        if self.elements.len() as u64 != self.symrows_lines {
            return Err(format!(
                "{} elements for {} symrows lines",
                self.elements.len(),
                self.symrows_lines
            ));
        }
        if self.drop_zero_sectors != self.zero_census.is_some() {
            return Err("drop_zero_sectors iff a zero census is written".into());
        }
        for (n, d) in [
            ("relation_digest", &self.relation_digest),
            ("record_blake3", &self.record_blake3),
            ("family_blake3", &self.family_blake3),
        ] {
            if !crate::is_blake3_hex(d) {
                return Err(format!("{n} is not a hex digest"));
            }
        }
        Ok(())
    }
}

/// Parse a constant pattern `I(a,b,..)` (integers only).
pub fn parse_constant_key(t: &str) -> Option<Vec<i64>> {
    let s: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    let inner = s.strip_prefix("I(")?.strip_suffix(')')?;
    inner.split(',').map(|x| x.parse::<i64>().ok()).collect()
}

/// The constant pattern text of a key.
pub fn key_pattern(k: &[i64]) -> String {
    format!(
        "I({})",
        k.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
    )
}

/// The RELATION DIGEST: BLAKE3 of the canonical JSON `{"schema": RELATION_SCHEMA, "key": K, "rhs": [[k, text], ..]}`
/// with the rhs sorted by key (coefficient texts byte-exact as the producer wrote them). A relation with a repeated
/// rhs key is refused (a producer merges equal integrals).
pub fn relation_digest(key: &[i64], rhs: &[(Vec<i64>, String)]) -> Result<String, String> {
    let mut r: Vec<&(Vec<i64>, String)> = rhs.iter().collect();
    r.sort_by(|a, b| a.0.cmp(&b.0));
    if r.windows(2).any(|w| w[0].0 == w[1].0) {
        return Err("a repeated right-hand-side key".into());
    }
    let v = json!({"schema": RELATION_SCHEMA, "key": key,
        "rhs": r.iter().map(|(k, c)| json!([k, c])).collect::<Vec<_>>()});
    let c = crate::canon::canonical_line(v.to_string().as_bytes()).map_err(|e| format!("{e:?}"))?;
    Ok(crate::blake3_hex(&c))
}

/// A relation: its key and its right-hand side (key, the producer's exact coefficient text).
pub type Relation = (Vec<i64>, Vec<(Vec<i64>, String)>);

/// The relation of a point-rule record line: (key from `target`, rhs [(key, text)]); `fixed` must equal the key.
pub fn relation_of_record(line: &[u8]) -> Result<Relation, String> {
    let v: Value = serde_json::from_slice(line).map_err(|e| format!("record: {e}"))?;
    if v["type"] != "rule" {
        return Err("not a rule record".into());
    }
    let key = v["target"]
        .as_str()
        .and_then(parse_constant_key)
        .ok_or("target is not a constant key")?;
    let fixed: Option<Vec<i64>> = serde_json::from_value(v["fixed"].clone()).ok();
    if fixed.as_ref() != Some(&key) {
        return Err("fixed is not the target key (not a point record)".into());
    }
    let mut rhs = vec![];
    for t in v["rhs"].as_array().ok_or("rhs")? {
        let k = t[0]
            .as_str()
            .and_then(parse_constant_key)
            .ok_or("rhs key is not constant")?;
        if k.len() != key.len() {
            return Err("rhs key of another arity".into());
        }
        let c = t[1].as_str().ok_or("rhs coefficient is not a text")?;
        rhs.push((k, c.to_string()));
    }
    Ok((key, rhs))
}

/// One key in rr_proof's `n0=..,n1=..` form.
pub fn key_text_named(k: &[i64]) -> String {
    k.iter()
        .enumerate()
        .map(|(i, v)| format!("n{i}={v}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn input<'a>(spec: &'a JobSpec, role: &str) -> Result<&'a crate::spec::InputRef, String> {
    spec.inputs
        .iter()
        .find(|i| i.role == role)
        .ok_or_else(|| format!("no input of role {role}"))
}

/// The bytes of a pinned input, its BLAKE3 checked against the spec.
pub fn pinned(spec: &JobSpec, role: &str) -> Result<Vec<u8>, String> {
    let i = input(spec, role)?;
    let p = spec.input_path(i);
    let b = std::fs::read(&p).map_err(|e| format!("input {role} {}: {e}", p.display()))?;
    if crate::blake3_hex(&b) != i.blake3 {
        return Err(format!("input {role} changed (BLAKE3 is not the spec's)"));
    }
    Ok(b)
}

/// The request of a `proof-symrows` spec (pinned, parsed, checked).
pub fn request(spec: &JobSpec) -> Result<SymrowsRequest, String> {
    let r: SymrowsRequest =
        serde_json::from_slice(&pinned(spec, "request")?).map_err(|e| format!("request: {e}"))?;
    r.check()?;
    Ok(r)
}

/// The line (without newline) at `offset` of `bytes`.
pub fn line_at(bytes: &[u8], offset: u64) -> Result<&[u8], String> {
    let o = usize::try_from(offset).map_err(|_| "offset")?;
    let rest = bytes.get(o..).ok_or("offset beyond the table")?;
    if o > 0 && bytes[o - 1] != b'\n' {
        return Err("offset is not at a line start".into());
    }
    let end = rest.iter().position(|&c| c == b'\n').unwrap_or(rest.len());
    Ok(&rest[..end])
}

/// The relation the pinned table states at the request's offset, checked: BLAKE3 of the line = `record_blake3`, the
/// line starts with `{"type":"rule"` (rr_proof's offset rule), its `index` = `rule`, its key = the request's key, its
/// relation digest = `relation_digest`.
pub fn table_relation(table: &[u8], r: &SymrowsRequest) -> Result<Vec<u8>, String> {
    // the header (line 1) carries the family the request is bound to
    let head = line_at(table, 0)?;
    let h: Value = serde_json::from_slice(head).map_err(|e| format!("table header: {e}"))?;
    let fam = h["family_source"]
        .as_str()
        .ok_or("the table header has no family_source")?;
    if h["type"] != "header" || crate::blake3_hex(fam.as_bytes()) != r.family_blake3 {
        return Err("the table header's family_source is not the request's family (BLAKE3)".into());
    }
    let line = line_at(table, r.offset)?;
    if crate::blake3_hex(line) != r.record_blake3 {
        return Err("the table line at the offset is not the request's record (BLAKE3)".into());
    }
    if !line.starts_with(b"{\"type\":\"rule\"") {
        return Err("the record line does not start with {\"type\":\"rule\"".into());
    }
    let v: Value = serde_json::from_slice(line).map_err(|e| format!("record: {e}"))?;
    if v["index"].as_i64() != Some(r.rule) {
        return Err(format!(
            "record index {} is not rule {}",
            v["index"], r.rule
        ));
    }
    let (key, rhs) = relation_of_record(line)?;
    if key != r.key {
        return Err("the record's key is not the request's".into());
    }
    if relation_digest(&key, &rhs)? != r.relation_digest {
        return Err("the record's relation digest is not the request's".into());
    }
    Ok(line.to_vec())
}

/// The legacy argv, derived from the spec ONLY (cwd = the run directory).
pub fn argv(spec: &JobSpec) -> Result<Vec<String>, String> {
    let r = request(spec)?;
    let p = |role: &str| -> Result<String, String> {
        Ok(spec.input_path(input(spec, role)?).display().to_string())
    };
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
        "--symrows".into(),
        p("symrows")?,
    ];
    if r.symrows_only {
        a.push("--symrows-only".into());
    }
    if r.rows == "engine" {
        a.extend(["--rows".into(), "engine".into()]);
        if spec.inputs.iter().any(|i| i.role == "family") {
            a.extend(["--family".into(), p("family")?]);
        }
    } else {
        a.extend(["--rows".into(), "cert".into()]);
    }
    if r.drop_zero_sectors {
        a.push("--drop-zero-sectors".into());
    }
    if let Some(m) = r.max_primes {
        a.extend(["--max-primes".into(), m.to_string()]);
    }
    a.extend(["--out".into(), format!("out/{CHILD_OUT}")]);
    Ok(a)
}

/// The certificate class of a kernel per-key status.
pub fn class_of_status(status: &str) -> &'static str {
    let w = status.split_whitespace().next().unwrap_or("");
    match w {
        "EXACT_IN_SPAN_AT_KEY" => "certified",
        "EXACT_NOT_IN_SPAN_AT_KEY" | "NOT_IN_SPAN_MODULAR" => "kernel_refused",
        "NOT_IN_CASE" | "NOT_IN_CHART" | "ON_EXCEPTION_BRANCH" | "IRREGULAR_KEY" => {
            "not_applicable"
        }
        _ => "undecided",
    }
}

/// Whether an rr_proof refusal message is the typed symmetry-row refusal (a whole word).
pub fn names_symrow_refusal(msg: &str) -> bool {
    msg.match_indices(SYMROW_REFUSED).any(|(i, _)| {
        let word = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        !word(msg[..i].chars().next_back()) && !word(msg[i + SYMROW_REFUSED.len()..].chars().next())
    })
}

/// A key object of the kernel (`{"n0":..}` with every slot) of the requested arity.
fn kernel_key(v: &Value, arity: usize) -> Option<Vec<i64>> {
    let o = v.as_object()?;
    if o.len() != arity {
        return None;
    }
    (0..arity)
        .map(|i| o.get(&format!("n{i}")).and_then(Value::as_i64))
        .collect()
}

/// Check the child's raw output (ONE span.v1 record) against the request: record digest, rule, symrows count / only
/// flag, zero-sector flag, exactly one key entry for the requested key echoing rule + record digest. Returns that key
/// entry.
pub fn check_span_output(r: &SymrowsRequest, raw: &[u8]) -> Result<Value, String> {
    let recs: Vec<&[u8]> = raw
        .split(|&c| c == b'\n')
        .filter(|l| !l.iter().all(u8::is_ascii_whitespace))
        .collect();
    let [rec] = recs.as_slice() else {
        return Err(format!(
            "{} records in out/{CHILD_OUT}, expected one",
            recs.len()
        ));
    };
    let v: Value = serde_json::from_slice(rec).map_err(|e| format!("span record: {e}"))?;
    if v["schema"] != SPAN_SCHEMA {
        return Err(format!("span record schema {}", v["schema"]));
    }
    if v["record_sha256"].as_str() != Some(r.record_sha256.as_str()) {
        return Err("span record about another record (record_sha256)".into());
    }
    if v["rule"].as_i64() != Some(r.rule) {
        return Err(format!("span record about rule {}", v["rule"]));
    }
    if v["symrows"]["given"].as_u64() != Some(r.symrows_lines)
        || v["symrows"]["only"].as_bool() != Some(r.symrows_only)
    {
        return Err(format!(
            "span record symrows {} is not the request's ({} rows, only {})",
            v["symrows"], r.symrows_lines, r.symrows_only
        ));
    }
    if v["zero_sectors"]["dropped_from_identity"].as_bool() != Some(r.drop_zero_sectors) {
        return Err("span record zero-sector flag is not the request's".into());
    }
    let entries = v["keys"].as_array().ok_or("span record without keys")?;
    let [e] = entries.as_slice() else {
        return Err(format!("{} key entries, expected one", entries.len()));
    };
    if kernel_key(&e["key"], r.key.len()).as_ref() != Some(&r.key) {
        return Err("the key entry is not the requested key".into());
    }
    if e["rule"].as_i64() != Some(r.rule)
        || e["record_sha256"].as_str() != Some(r.record_sha256.as_str())
    {
        return Err("the key entry does not echo the rule and record".into());
    }
    if e["status"].as_str().is_none() {
        return Err("the key entry has no status".into());
    }
    Ok(e.clone())
}

/// The canonical certificate line of a checked key entry (the wrapper seals it).
pub fn certificate_line(
    r: &SymrowsRequest,
    symrows_blake3: &str,
    entry: &Value,
) -> Result<Vec<u8>, String> {
    let status = entry["status"].as_str().unwrap_or("").to_string();
    let verdict = status.split_whitespace().next().unwrap_or("").to_string();
    let v = json!({"schema": CERT_SCHEMA, "relation_digest": r.relation_digest,
        "record_sha256": r.record_sha256, "rule": r.rule, "key": r.key, "verdict": verdict,
        "status": status, "class": class_of_status(&status), "group_digest": r.group_digest,
        "elements": r.elements, "symrows_blake3": symrows_blake3, "symrows_given": r.symrows_lines,
        "symrows_only": r.symrows_only, "rows": r.rows, "drop_zero_sectors": r.drop_zero_sectors,
        "zero_census": r.zero_census, "catalogue": r.catalogue, "kernel_sha256": r.kernel_sha256,
        "family_blake3": r.family_blake3,
        "producer": r.producer, "producer_record_sha256": r.producer_record_sha256,
        "instance": entry["instance"].clone(), "seconds": entry["seconds"].clone()});
    crate::canon::canonical_line(v.to_string().as_bytes()).map_err(|e| format!("{e:?}"))
}

/// One checked certificate (sealed, re-checked against the pinned request and table).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Certificate {
    pub relation_digest: String,
    pub record_sha256: String,
    pub producer_record_sha256: Option<String>,
    pub key: Vec<i64>,
    pub verdict: String,
    pub status: String,
    pub class: String,
    pub kernel_sha256: String,
    pub family_blake3: String,
    /// The canonical sealed line.
    pub canonical: Vec<u8>,
}

/// Check ONE sealed certificate line against the request (every bound member equal).
pub fn check_certificate(
    r: &SymrowsRequest,
    symrows_blake3: &str,
    line: &[u8],
) -> Result<Certificate, String> {
    let v: Value = serde_json::from_slice(line).map_err(|e| format!("certificate: {e}"))?;
    let want = |m: &str, x: Value| -> Result<(), String> {
        if v[m] != x {
            return Err(format!("certificate {m} {} is not the request's {x}", v[m]));
        }
        Ok(())
    };
    want("schema", json!(CERT_SCHEMA))?;
    want("relation_digest", json!(r.relation_digest))?;
    want("record_sha256", json!(r.record_sha256))?;
    want("rule", json!(r.rule))?;
    want("key", json!(r.key))?;
    want("group_digest", json!(r.group_digest))?;
    want("elements", json!(r.elements))?;
    want("symrows_blake3", json!(symrows_blake3))?;
    want("symrows_given", json!(r.symrows_lines))?;
    want("symrows_only", json!(r.symrows_only))?;
    want("rows", json!(r.rows))?;
    want("drop_zero_sectors", json!(r.drop_zero_sectors))?;
    want("zero_census", json!(r.zero_census))?;
    want("catalogue", json!(r.catalogue))?;
    want("kernel_sha256", json!(r.kernel_sha256))?;
    want("family_blake3", json!(r.family_blake3))?;
    want("producer", json!(r.producer))?;
    want("producer_record_sha256", json!(r.producer_record_sha256))?;
    let status = v["status"].as_str().ok_or("certificate without status")?;
    let verdict = status.split_whitespace().next().unwrap_or("");
    want("verdict", json!(verdict))?;
    want("class", json!(class_of_status(status)))?;
    Ok(Certificate {
        relation_digest: r.relation_digest.clone(),
        record_sha256: r.record_sha256.clone(),
        producer_record_sha256: r.producer_record_sha256.clone(),
        key: r.key.clone(),
        verdict: verdict.to_string(),
        status: status.to_string(),
        class: class_of_status(status).to_string(),
        kernel_sha256: r.kernel_sha256.clone(),
        family_blake3: r.family_blake3.clone(),
        canonical: crate::canon::canonical_line(line).map_err(|e| format!("{e:?}"))?,
    })
}

/// S of the kind: the verified canonical lines of the sealed results (certificates, end record last) checked against
/// the pinned request, table and symrows input (each read and digest-checked here): ONE certificate for the
/// requested key, bound to exactly the asked relation.
pub fn check_results(lines: &[Vec<u8>], spec: &JobSpec) -> Result<Vec<Certificate>, String> {
    let r = request(spec)?;
    let table = pinned(spec, "table")?;
    table_relation(&table, &r)?;
    let symrows = pinned(spec, "symrows")?;
    let n = symrows
        .split(|&c| c == b'\n')
        .filter(|l| !l.iter().all(u8::is_ascii_whitespace))
        .count() as u64;
    if n != r.symrows_lines {
        return Err(format!(
            "{n} symrows lines, the request says {}",
            r.symrows_lines
        ));
    }
    let sym_b3 = input(spec, "symrows")?.blake3.clone();
    let (end, body) = lines.split_last().ok_or("results without records")?;
    let end: Value = serde_json::from_slice(end).map_err(|e| format!("end record: {e}"))?;
    if end["type"] != "end" || end["records"].as_u64() != Some(body.len() as u64) {
        return Err("end record does not count the certificates".into());
    }
    if end["request_blake3"].as_str() != Some(input(spec, "request")?.blake3.as_str()) {
        return Err("end record names another request".into());
    }
    let [one] = body else {
        return Err(format!("{} certificates, expected one", body.len()));
    };
    Ok(vec![check_certificate(&r, &sym_b3, one)?])
}

/// S of the kind through the run directory's verified results stream (see [`check_results`]).
pub fn certificates_of_run(
    root: &Path,
    spec: &JobSpec,
    v: &crate::classify::VerifiedOutputs,
) -> Result<Vec<Certificate>, String> {
    let rel = crate::layout::RunLayout::out_rel(crate::layout::RESULTS);
    let s = v
        .streams
        .iter()
        .find(|s| s.rel == rel)
        .ok_or("no verified results stream")?;
    let lines = crate::seal::VerifiedHandle::reopen(root, s)
        .and_then(|h| h.canonical_lines())
        .map_err(|e| format!("results: {e}"))?;
    check_results(&lines, spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_is_order_free_and_text_exact() {
        let a = relation_digest(
            &[1, 1, -1],
            &[(vec![1, 0, -1], "1/2".into()), (vec![1, 1, 0], "-2".into())],
        )
        .unwrap();
        let b = relation_digest(
            &[1, 1, -1],
            &[(vec![1, 1, 0], "-2".into()), (vec![1, 0, -1], "1/2".into())],
        )
        .unwrap();
        assert_eq!(a, b);
        let c = relation_digest(
            &[1, 1, -1],
            &[(vec![1, 1, 0], "-2".into()), (vec![1, 0, -1], "2/4".into())],
        )
        .unwrap();
        assert_ne!(a, c);
        assert!(relation_digest(&[1], &[(vec![0], "1".into()), (vec![0], "2".into())]).is_err());
    }

    #[test]
    fn statuses_and_refusals() {
        assert_eq!(class_of_status("EXACT_IN_SPAN_AT_KEY"), "certified");
        assert_eq!(
            class_of_status("EXACT_NOT_IN_SPAN_AT_KEY (exact certificate ...)"),
            "kernel_refused"
        );
        assert_eq!(class_of_status("NOT_IN_SPAN_MODULAR (x)"), "kernel_refused");
        assert_eq!(class_of_status("UNDECIDED (x)"), "undecided");
        assert_eq!(class_of_status("NOT_IN_CHART (x)"), "not_applicable");
        assert!(names_symrow_refusal(
            "SYMROW_REFUSED: loop part not unimodular"
        ));
        assert!(!names_symrow_refusal("XSYMROW_REFUSED_Y"));
        assert_eq!(parse_constant_key("I(1, -2,0)"), Some(vec![1, -2, 0]));
        assert_eq!(parse_constant_key("I(n0,1)"), None);
        assert_eq!(key_pattern(&[1, -2]), "I(1,-2)");
    }
}
