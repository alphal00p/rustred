//! The spec section `regular_basis` (`rustred.campaign.regular-basis-spec.v1`): every bound is data with a documented
//! default (L8); unknown members are refused.
//!
//! | field | default | meaning |
//! |---|---|---|
//! | `d0` | `"4"` | rational sampling dimension (text `a` or `a/b`) |
//! | `base_order` | `0` | eps order to which a reduced integral is needed (BOXDEF: eps^(5-L) at L loops); need = base + pole |
//! | `candidates.terminals_first` | `true` | the closure's terminals head the preference order (kept where possible) |
//! | `candidates.max_dots` | `2` | candidate family: top-sector keys with dots <= this ... |
//! | `candidates.max_rank` | `1` | ... rank <= this ... |
//! | `candidates.max_weight` | `2` | ... and dots + rank <= this (then the fallback: pivot keys of the band pass) |
//! | `candidates.order` | `"weight_then_node"` | `weight_then_node` (dots + rank, then the node comparator) or `node` (comparator only) |
//! | `certify.lattice` | `true` | certificate c1: the lattice of R equals the band lattice (index 1) |
//! | `certify.keys` | `"all"` | certificate c2: `all` band keys re-checked as c(K) M (streamed) or `none` |
//! | `chunk_keys` | `4096` | keys per composition batch (streaming; memory bound) |
//! | `tower.enabled` | `true` | run the tower test (dot-free keys) |
//! | `tower.max_rank` | `null` | highest rank of the tower (`null` = the band's highest rank) |
//! | `tower.max_keys_per_rank` | `null` | cap of dot-free keys composed per rank (`null` = all) |
//! | `tower.max_seconds` | `null` | wall cap of the tower (time-decided results are labelled) |
//! | `tower.seed_with_band` | `true` | seed the tower lattice with the certified band lattice (exact on those ranks) |

use serde_json::{Map, Value, json};

/// Order of the candidate family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateOrder {
    /// dots + rank ascending, then the node comparator.
    WeightThenNode,
    /// the node comparator alone.
    Node,
}

impl CandidateOrder {
    pub fn word(&self) -> &'static str {
        match self {
            CandidateOrder::WeightThenNode => "weight_then_node",
            CandidateOrder::Node => "node",
        }
    }
}

/// Which band keys certificate c2 re-checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertifyKeys {
    All,
    None,
}

impl CertifyKeys {
    pub fn word(&self) -> &'static str {
        match self {
            CertifyKeys::All => "all",
            CertifyKeys::None => "none",
        }
    }
}

/// The parsed spec section.
#[derive(Clone, Debug, PartialEq)]
pub struct RegBasisSpec {
    pub d0: String,
    pub base_order: i64,
    pub terminals_first: bool,
    pub max_dots: u32,
    pub max_rank: u32,
    pub max_weight: u32,
    pub order: CandidateOrder,
    pub certify_lattice: bool,
    pub certify_keys: CertifyKeys,
    pub chunk_keys: usize,
    pub tower_enabled: bool,
    pub tower_max_rank: Option<u32>,
    pub tower_max_keys_per_rank: Option<usize>,
    pub tower_max_seconds: Option<f64>,
    pub tower_seed_with_band: bool,
}

impl Default for RegBasisSpec {
    fn default() -> Self {
        RegBasisSpec {
            d0: "4".into(),
            base_order: 0,
            terminals_first: true,
            max_dots: 2,
            max_rank: 1,
            max_weight: 2,
            order: CandidateOrder::WeightThenNode,
            certify_lattice: true,
            certify_keys: CertifyKeys::All,
            chunk_keys: 4096,
            tower_enabled: true,
            tower_max_rank: None,
            tower_max_keys_per_rank: None,
            tower_max_seconds: None,
            tower_seed_with_band: true,
        }
    }
}

fn obj<'a>(v: &'a Value, what: &str, allowed: &[&str]) -> Result<&'a Map<String, Value>, String> {
    let o = v
        .as_object()
        .ok_or_else(|| format!("{what}: expected an object"))?;
    for k in o.keys() {
        if !allowed.contains(&k.as_str()) {
            return Err(format!(
                "{what}: unknown member {k:?} (allowed: {allowed:?})"
            ));
        }
    }
    Ok(o)
}

fn u32_of(v: &Value, what: &str) -> Result<u32, String> {
    v.as_u64()
        .and_then(|x| u32::try_from(x).ok())
        .ok_or_else(|| format!("{what}: expected a non-negative integer, got {v}"))
}

fn opt_u32(v: Option<&Value>, what: &str) -> Result<Option<u32>, String> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(x) => u32_of(x, what).map(Some),
    }
}

fn bool_of(v: &Value, what: &str) -> Result<bool, String> {
    v.as_bool()
        .ok_or_else(|| format!("{what}: expected a boolean, got {v}"))
}

/// `a` or `a/b` with decimal digits, `b` non-zero (the algebra crate normalises with lane poles' `D0::parse`).
fn check_rational_text(s: &str) -> Result<(), String> {
    let (a, b) = s.split_once('/').unwrap_or((s, "1"));
    let a = a.strip_prefix('-').unwrap_or(a);
    let ok = |x: &str| !x.is_empty() && x.bytes().all(|c| c.is_ascii_digit());
    if !ok(a) || !ok(b) || b.bytes().all(|c| c == b'0') {
        return Err(format!("d0 {s:?}: not a rational a or a/b"));
    }
    Ok(())
}

impl RegBasisSpec {
    /// Parse the section (absent members take their defaults; `null` = the whole section by default).
    pub fn from_value(v: &Value) -> Result<RegBasisSpec, String> {
        let mut s = RegBasisSpec::default();
        if v.is_null() {
            return Ok(s);
        }
        let o = obj(
            v,
            "regular_basis",
            &[
                "schema",
                "d0",
                "base_order",
                "candidates",
                "certify",
                "chunk_keys",
                "tower",
            ],
        )?;
        if let Some(x) = o.get("schema")
            && x.as_str() != Some(SPEC_SCHEMA)
        {
            return Err(format!(
                "regular_basis.schema: expected {SPEC_SCHEMA:?}, got {x}"
            ));
        }
        if let Some(x) = o.get("d0") {
            let t = x.as_str().ok_or("regular_basis.d0: expected a string")?;
            check_rational_text(t)?;
            s.d0 = t.to_string();
        }
        if let Some(x) = o.get("base_order") {
            s.base_order = x
                .as_i64()
                .ok_or("regular_basis.base_order: expected an integer")?;
        }
        if let Some(c) = o.get("candidates") {
            let c = obj(
                c,
                "regular_basis.candidates",
                &[
                    "terminals_first",
                    "max_dots",
                    "max_rank",
                    "max_weight",
                    "order",
                ],
            )?;
            if let Some(x) = c.get("terminals_first") {
                s.terminals_first = bool_of(x, "candidates.terminals_first")?;
            }
            if let Some(x) = c.get("max_dots") {
                s.max_dots = u32_of(x, "candidates.max_dots")?;
            }
            if let Some(x) = c.get("max_rank") {
                s.max_rank = u32_of(x, "candidates.max_rank")?;
            }
            if let Some(x) = c.get("max_weight") {
                s.max_weight = u32_of(x, "candidates.max_weight")?;
            }
            if let Some(x) = c.get("order") {
                s.order = match x.as_str() {
                    Some("weight_then_node") => CandidateOrder::WeightThenNode,
                    Some("node") => CandidateOrder::Node,
                    _ => {
                        return Err(format!(
                            "candidates.order: weight_then_node | node, got {x}"
                        ));
                    }
                };
            }
        }
        if let Some(c) = o.get("certify") {
            let c = obj(c, "regular_basis.certify", &["lattice", "keys"])?;
            if let Some(x) = c.get("lattice") {
                s.certify_lattice = bool_of(x, "certify.lattice")?;
            }
            if let Some(x) = c.get("keys") {
                s.certify_keys = match x.as_str() {
                    Some("all") => CertifyKeys::All,
                    Some("none") => CertifyKeys::None,
                    _ => return Err(format!("certify.keys: all | none, got {x}")),
                };
            }
        }
        if let Some(x) = o.get("chunk_keys") {
            let n = u32_of(x, "chunk_keys")? as usize;
            if n == 0 {
                return Err("chunk_keys: must be >= 1".into());
            }
            s.chunk_keys = n;
        }
        if let Some(t) = o.get("tower") {
            let t = obj(
                t,
                "regular_basis.tower",
                &[
                    "enabled",
                    "max_rank",
                    "max_keys_per_rank",
                    "max_seconds",
                    "seed_with_band",
                ],
            )?;
            if let Some(x) = t.get("enabled") {
                s.tower_enabled = bool_of(x, "tower.enabled")?;
            }
            s.tower_max_rank = opt_u32(t.get("max_rank"), "tower.max_rank")?;
            s.tower_max_keys_per_rank =
                opt_u32(t.get("max_keys_per_rank"), "tower.max_keys_per_rank")?.map(|x| x as usize);
            s.tower_max_seconds = match t.get("max_seconds") {
                None | Some(Value::Null) => None,
                Some(x) => {
                    let f = x.as_f64().filter(|f| f.is_finite() && *f > 0.0);
                    Some(f.ok_or_else(|| format!("tower.max_seconds: positive number, got {x}"))?)
                }
            };
            if let Some(x) = t.get("seed_with_band") {
                s.tower_seed_with_band = bool_of(x, "tower.seed_with_band")?;
            }
        }
        Ok(s)
    }

    /// The effective section (every member written; recorded in the export object). Floats are not used in the
    /// canonical record: `max_seconds` is written as text.
    pub fn to_value(&self) -> Value {
        json!({
            "schema": SPEC_SCHEMA,
            "d0": self.d0,
            "base_order": self.base_order,
            "candidates": {"terminals_first": self.terminals_first, "max_dots": self.max_dots,
                           "max_rank": self.max_rank, "max_weight": self.max_weight, "order": self.order.word()},
            "certify": {"lattice": self.certify_lattice, "keys": self.certify_keys.word()},
            "chunk_keys": self.chunk_keys,
            "tower": {"enabled": self.tower_enabled, "max_rank": self.tower_max_rank,
                      "max_keys_per_rank": self.tower_max_keys_per_rank,
                      "max_seconds": self.tower_max_seconds.map(|f| format!("{f}")),
                      "seed_with_band": self.tower_seed_with_band},
        })
    }
}

/// Schema name of the spec section.
pub const SPEC_SCHEMA: &str = "rustred.campaign.regular-basis-spec.v1";
