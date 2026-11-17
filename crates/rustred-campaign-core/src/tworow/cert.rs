//! planfix (audit r2-planners, "tworow certificates"): a tworow relation can CARRY a reference to a KERNEL CERTIFICATE
//! (rr_proof `instance-span --symrows [--symrows-only]`, frozen engine-line7d; contract kind `proof-symrows`, lane
//! symcert, interfaces/proof-symrows-job.md) and the import can CHECK that reference against the relation itself.
//!
//! The binding is the RELATION DIGEST of lane symcert: BLAKE3 of the canonical JSON
//! `{"key":[..],"rhs":[[[..],"<text>"],..],"schema":"rustred.campaign.symrows-relation.v1"}` (rhs sorted by key, the
//! producer's exact coefficient texts), computed here from the combination's own right-hand side. A reference binds
//! iff its relation digest, key and schema match; it LIFTS the relation iff its class is `certified`; a sealed
//! certificate line, when given, must hash to the reference's `certificate_sha256` and echo its digest, key and class.
//!
//! The PRODUCER side (writing the proof-symrows job files, running the job, reading the sealed stream into a
//! [`KernelCertificateRef`]) is lane symcert's (`closeloop::symcert::certify_symmetry_relations`, unmerged at int/7
//! d1a12d5d): left to the next integration step. Nothing here runs a process (L8: nothing node specific).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::Combination;

/// Lane symcert's relation-object schema (the BLAKE3 of its canonical JSON is the relation digest).
pub const SYMROWS_RELATION_SCHEMA: &str = "rustred.campaign.symrows-relation.v1";
/// Lane symcert's sealed certificate line schema.
pub const SYMROWS_CERT_SCHEMA: &str = "rustred.campaign.symrows-certificate.v1";

/// A reference to a sealed kernel certificate of one relation at one key (the fields of lane symcert's certificate
/// line that bind it).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KernelCertificateRef {
    /// [`SYMROWS_CERT_SCHEMA`].
    pub schema: String,
    /// BLAKE3 relation digest ([`relation_digest`]).
    pub relation_digest: String,
    pub key: Vec<i32>,
    /// `certified` | `kernel_refused` | `undecided` | `not_applicable` (lane symcert's closed list).
    pub class: String,
    /// sha256 of the point-rule record line put to the kernel.
    pub record_sha256: String,
    /// sha256 of the kernel binary.
    pub kernel_sha256: String,
    /// sha256 (hex) of the sealed certificate line (canonical bytes, without the newline).
    pub certificate_sha256: String,
}

/// The import's verdict on a reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CertificateCheck {
    /// Bound to this relation at this key and `certified`: the relation is kernel-certified.
    Certified,
    /// Bound, the kernel REFUSED the relation (`kernel_refused`): a finding, never a lift.
    KernelRefused,
    /// Bound, no decision (`undecided` / `not_applicable`): no lift.
    NoDecision(String),
    /// Not bound to this relation (digest, key, schema, class word, or the sealed line differ): refused.
    NotBound(String),
}

impl CertificateCheck {
    pub fn certified(&self) -> bool {
        matches!(self, CertificateCheck::Certified)
    }
}

/// The canonical relation text of lane symcert's digest (members sorted, no whitespace; rhs sorted by key).
pub fn relation_canonical(key: &[i32], rhs: &[(Vec<i32>, String)]) -> Result<String, String> {
    let mut r: Vec<&(Vec<i32>, String)> = rhs.iter().collect();
    r.sort_by(|a, b| a.0.cmp(&b.0));
    if r.windows(2).any(|w| w[0].0 == w[1].0) {
        return Err("a repeated right-hand-side key".into());
    }
    let v = json!({"schema": SYMROWS_RELATION_SCHEMA, "key": key,
        "rhs": r.iter().map(|(k, c)| json!([k, c])).collect::<Vec<_>>()});
    crate::schema::canon::canonical_json(&v, false)
}

/// The relation digest (BLAKE3 hex) of a relation given by its key and its (key, exact coefficient text) terms.
pub fn relation_digest(key: &[i32], rhs: &[(Vec<i32>, String)]) -> Result<String, String> {
    Ok(crate::boxdef::blake3_hex(
        relation_canonical(key, rhs)?.as_bytes(),
    ))
}

impl Combination {
    /// The relation digest of this combination (its own exact right-hand-side texts).
    pub fn relation_digest(&self) -> Result<String, String> {
        relation_digest(&self.key, &self.rhs_text())
    }
    /// The record ([`Combination::to_json`]) with the kernel certificate reference (when given) in
    /// `engine_certificate` (an object) and the relation digest; without one `engine_certificate` stays the word
    /// `none (...)`.
    pub fn to_json_with(&self, cert: Option<&KernelCertificateRef>) -> Value {
        let mut v = self.to_json();
        if let Ok(d) = self.relation_digest() {
            v["relation_digest"] = json!(d);
        }
        if let Some(c) = cert {
            v["engine_certificate"] = serde_json::to_value(c).unwrap_or(Value::Null);
        }
        v
    }
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = crate::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// Check a certificate reference against the combination it is attached to (and, when given, the SEALED certificate
/// line it names: sha256 equal to `certificate_sha256`, schema, relation digest, key and class echoed).
pub fn check_certificate(
    comb: &Combination,
    cert: &KernelCertificateRef,
    sealed_line: Option<&str>,
) -> CertificateCheck {
    let nb = |m: String| CertificateCheck::NotBound(m);
    if cert.schema != SYMROWS_CERT_SCHEMA {
        return nb(format!("schema {:?}", cert.schema));
    }
    let digest = match comb.relation_digest() {
        Ok(d) => d,
        Err(e) => return nb(format!("relation: {e}")),
    };
    if cert.relation_digest != digest {
        return nb(format!(
            "relation digest {} is not this relation's {digest}",
            cert.relation_digest
        ));
    }
    if cert.key != comb.key {
        return nb(format!("key {:?} is not {:?}", cert.key, comb.key));
    }
    if let Some(line) = sealed_line {
        let line = line.trim_end_matches('\n');
        if sha256_hex(line.as_bytes()) != cert.certificate_sha256 {
            return nb("the sealed certificate line does not hash to certificate_sha256".into());
        }
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => return nb(format!("sealed certificate line: {e}")),
        };
        let echoes = v["schema"] == SYMROWS_CERT_SCHEMA
            && v["relation_digest"] == cert.relation_digest.as_str()
            && v["key"] == json!(cert.key)
            && v["class"] == cert.class.as_str()
            && v["record_sha256"] == cert.record_sha256.as_str()
            && v["kernel_sha256"] == cert.kernel_sha256.as_str();
        if !echoes {
            return nb("the sealed certificate line does not echo the reference".into());
        }
    }
    match cert.class.as_str() {
        "certified" => CertificateCheck::Certified,
        "kernel_refused" => CertificateCheck::KernelRefused,
        "undecided" | "not_applicable" => CertificateCheck::NoDecision(cert.class.clone()),
        w => nb(format!("class {w:?} is not a certificate class")),
    }
}
