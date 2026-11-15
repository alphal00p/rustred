//! The export object `rustred.campaign.regular-basis.v1` and the tower record `rustred.campaign.regular-basis-tower.v1`
//! (interfaces/regbasis.md 5): canonical JSON (the crate's one rule, `schema::canon`, integers only), a BLAKE3 digest over
//! the canonical text without the `digest` member, BOUND to the node identity, the box, the closure statement digest and
//! d0 (the `binding` member). The member list is documented in the interface; the algebra crate fills it.
//!
//! Conventions (aligned with pole-diagnostics.md 3 / 9 and the coordinator's hand objects, comparison only):
//! `T_i = sum_j M[i][j] R_j (+ subsector terms)`, `R_j = sum_i Minv[j][i] T_i`; orders at d0: `> 0` order of the zero,
//! `< 0` minus the pole order, `null` identically zero.

use serde_json::{Map, Value, json};

use crate::boxdef::blake3_hex;
use crate::schema::canon::canonical_json;

pub const RECORD_SCHEMA: &str = "rustred.campaign.regular-basis.v1";
pub const TOWER_SCHEMA: &str = "rustred.campaign.regular-basis-tower.v1";
/// The lattice convention used for R (the brief / Theorem 1: the closure's terminals must stay finite, M regular).
pub const CONVENTION_BAND_PLUS_TERMINALS: &str = "band_plus_terminals";
/// The matrix convention of the record.
pub const MATRIX_CONVENTION: &str = "T_i = sum_j M[i][j] R_j (+ subsector terms); R_j = sum_i Minv[j][i] T_i; modulo subsectors; orders at d0: >0 zero order, <0 pole order, null = identically zero";

/// What the object is bound to (all members required, non-empty).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub node_id: String,
    /// Digest of the node identity (family / frame / order the caller identifies the node by).
    pub node_identity_digest: String,
    pub box_id: String,
    pub box_digest: String,
    pub closure_statement_digest: String,
    /// Canonical d0 text (`4`, `17/7`).
    pub d0: String,
}

impl Binding {
    pub fn validate(&self) -> Result<(), String> {
        for (n, v) in [
            ("node_id", &self.node_id),
            ("node_identity_digest", &self.node_identity_digest),
            ("box_id", &self.box_id),
            ("box_digest", &self.box_digest),
            ("closure_statement_digest", &self.closure_statement_digest),
            ("d0", &self.d0),
        ] {
            if v.trim().is_empty() {
                return Err(format!("binding.{n} is empty"));
            }
        }
        Ok(())
    }

    pub fn to_value(&self) -> Value {
        json!({
            "node": {"id": self.node_id, "identity_digest": self.node_identity_digest},
            "box": {"id": self.box_id, "digest": self.box_digest},
            "closure_statement_digest": self.closure_statement_digest,
            "d0": self.d0,
        })
    }
}

fn digest_of(o: &Map<String, Value>) -> Result<String, String> {
    let mut c = o.clone();
    c.remove("digest");
    let text = canonical_json(&Value::Object(c), false)?;
    Ok(blake3_hex(text.as_bytes()))
}

/// Seal an object: set `digest` = BLAKE3 of its canonical JSON without `digest`. Returns (object, canonical text).
pub fn seal(v: Value) -> Result<(Value, String), String> {
    let mut o = match v {
        Value::Object(o) => o,
        _ => return Err("record: expected an object".into()),
    };
    match o.get("schema").and_then(Value::as_str) {
        Some(RECORD_SCHEMA) | Some(TOWER_SCHEMA) => {}
        other => return Err(format!("record: unknown schema {other:?}")),
    }
    if o.get("binding").is_none() {
        return Err("record: missing binding".into());
    }
    let d = digest_of(&o)?;
    o.insert("digest".into(), Value::String(d));
    let v = Value::Object(o);
    let text = canonical_json(&v, false)?;
    Ok((v, text))
}

/// Check a sealed object's digest.
pub fn verify(v: &Value) -> Result<(), String> {
    let o = v.as_object().ok_or("record: expected an object")?;
    let stated = o
        .get("digest")
        .and_then(Value::as_str)
        .ok_or("record: no digest")?;
    let d = digest_of(o)?;
    if d != stated {
        return Err(format!(
            "record digest mismatch: stated {stated}, computed {d}"
        ));
    }
    Ok(())
}

/// The band verdict text: `full band (through rank r_max)` or `regular through rank r (lower bound)` or `no rank
/// complete`.
pub fn band_verdict(r_complete: Option<u32>, r_max: u32, regular: bool) -> String {
    let what = if regular { "regular" } else { "NOT regular" };
    match r_complete {
        Some(r) if r >= r_max => format!("{what} on the full band (through rank {r_max})"),
        Some(r) => format!("{what} through rank {r} (lower bound; band r_max {r_max})"),
        None => format!("{what} on no complete rank (band r_max {r_max}): lower bound only"),
    }
}
