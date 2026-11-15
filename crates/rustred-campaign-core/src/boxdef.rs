//! Boxes of keys (schema `rustred.campaign.box.v1`, measure `dots-rank-v1`): data, membership, streaming enumeration,
//! exact counts, saturation, faces. Interface: `interfaces/campaign-core-domain.md` section 7.
//!
//! A box is DATA given by the user in the node's spec (R33, R35, R40 U-7): this module interprets it and never chooses
//! or widens one. The only derived box is the saturated box B* (plan 2.4.8), a new [`BoxDef`] whose `derived_from` is
//! the digest of the box it was saturated from. Nothing here knows a node, a loop count, an arity or a box shape: the
//! band presets are data ([`BandSpec`]).

use std::collections::BTreeMap;
use std::fmt;

use serde_json::{Map, Value, json};

use crate::key::{Key, NodeFrame};

pub use crate::winner;

/// Schema of a box document.
pub const BOX_SCHEMA: &str = "rustred.campaign.box.v1";
/// The one measure (the engine's boxed-node measure): D over the support lines, r over the other slots.
pub const BOX_MEASURE: &str = "dots-rank-v1";
/// Largest arity a box accepts (supports are tracked as bit sets elsewhere in the crate).
pub const MAX_BOX_ARITY: usize = 64;

/// Errors of box parsing, validation and conversion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoxError {
    Json(String),
    Schema(String),
    Invalid(String),
    Digest {
        expected: String,
        computed: String,
    },
    Arity {
        expected: usize,
        got: usize,
    },
    /// The box cannot be written as a boxed-node header box (it has sectors or explicit keys).
    NotHeaderable(String),
}

impl fmt::Display for BoxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BoxError::Json(s) => write!(f, "box JSON: {s}"),
            BoxError::Schema(s) => write!(f, "box schema: {s}"),
            BoxError::Invalid(s) => write!(f, "invalid box: {s}"),
            BoxError::Digest { expected, computed } => {
                write!(
                    f,
                    "box digest mismatch: stated {expected}, computed {computed}"
                )
            }
            BoxError::Arity { expected, got } => write!(f, "box arity {expected}, got {got}"),
            BoxError::NotHeaderable(s) => {
                write!(f, "box not expressible as a boxed_node header: {s}")
            }
        }
    }
}

impl std::error::Error for BoxError {}

/// Layers of one proper subsector (support = the line slots that are >= 1).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SectorLayers {
    pub support: Vec<usize>,
    pub layers: Vec<[u32; 2]>,
}

/// A box (see the module doc and interface section 7.1).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BoxDef {
    pub id: String,
    pub name: String,
    /// `chart[slot]` = the slot is a line.
    pub chart: Vec<bool>,
    /// Top-sector layers `[D, r_max]` (union semantics).
    pub layers: Vec<[u32; 2]>,
    pub sectors: Vec<SectorLayers>,
    /// Largest power of a support line (`None`: no cap).
    pub cap: Option<u32>,
    /// Optional per-slot caps (empty = none, else one entry per slot): for a support line the largest power, for
    /// any other slot the largest |value|.
    pub slot_caps: Vec<Option<u32>>,
    /// Explicit extra keys.
    pub keys: Vec<Key>,
    pub origin: Option<String>,
    /// Digest of the box this one was saturated from.
    pub derived_from: Option<String>,
}

fn merge_layers(layers: &[[u32; 2]]) -> Vec<[u32; 2]> {
    let mut m: BTreeMap<u32, u32> = BTreeMap::new();
    for &[d, r] in layers {
        let e = m.entry(d).or_insert(r);
        *e = (*e).max(r);
    }
    m.into_iter().map(|(d, r)| [d, r]).collect()
}

fn chart_string(chart: &[bool]) -> String {
    chart.iter().map(|&l| if l { '1' } else { '0' }).collect()
}

fn parse_chart(s: &str) -> Result<Vec<bool>, BoxError> {
    s.chars()
        .map(|c| match c {
            '1' => Ok(true),
            '0' => Ok(false),
            _ => Err(BoxError::Invalid(format!("chart {s:?}: not a 0/1 string"))),
        })
        .collect()
}

fn layers_value(l: &[[u32; 2]]) -> Value {
    Value::Array(l.iter().map(|&[d, r]| json!([d, r])).collect())
}

fn key_value(k: &[i32]) -> Value {
    Value::Array(k.iter().map(|&x| json!(x)).collect())
}

fn as_u32(v: &Value, what: &str) -> Result<u32, BoxError> {
    v.as_u64()
        .and_then(|x| u32::try_from(x).ok())
        .ok_or_else(|| {
            BoxError::Schema(format!("{what}: expected a non-negative integer, got {v}"))
        })
}

fn parse_layers(v: &Value, what: &str) -> Result<Vec<[u32; 2]>, BoxError> {
    let a = v
        .as_array()
        .ok_or_else(|| BoxError::Schema(format!("{what}: expected an array of [D, r_max]")))?;
    a.iter()
        .map(|p| {
            let p = p
                .as_array()
                .filter(|p| p.len() == 2)
                .ok_or_else(|| BoxError::Schema(format!("{what}: layer {p} is not [D, r_max]")))?;
            Ok([as_u32(&p[0], what)?, as_u32(&p[1], what)?])
        })
        .collect()
}

fn parse_key_value(v: &Value) -> Result<Key, BoxError> {
    v.as_array()
        .ok_or_else(|| BoxError::Schema(format!("key {v} is not an array")))?
        .iter()
        .map(|x| {
            x.as_i64()
                .and_then(|x| i32::try_from(x).ok())
                .ok_or_else(|| BoxError::Schema(format!("key entry {x} is not an i32")))
        })
        .collect()
}

fn opt_string(o: &Map<String, Value>, k: &str) -> Result<Option<String>, BoxError> {
    match o.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(v) => Err(BoxError::Schema(format!(
            "{k}: expected a string or null, got {v}"
        ))),
    }
}

fn req_string(o: &Map<String, Value>, k: &str) -> Result<String, BoxError> {
    opt_string(o, k)?.ok_or_else(|| BoxError::Schema(format!("missing string field {k:?}")))
}

/// Canonical JSON of plan 3.3.0 (the crate's one rule, [`crate::schema::canon`]): the bytes the digest hashes.
pub fn canonical_json(v: &Value) -> String {
    crate::schema::canon::canonical_json(v, false).expect("box documents hold integers only")
}

const FIELDS: [&str; 13] = [
    "schema",
    "id",
    "name",
    "measure",
    "chart",
    "layers",
    "sectors",
    "cap",
    "slot_caps",
    "keys",
    "origin",
    "derived_from",
    "digest",
];

impl BoxDef {
    /// Parse, validate and normalise a box document; a stated `digest` must equal the computed one.
    pub fn from_json(text: &str) -> Result<BoxDef, BoxError> {
        let v: Value = serde_json::from_str(text).map_err(|e| BoxError::Json(e.to_string()))?;
        BoxDef::from_value(&v)
    }

    /// As [`BoxDef::from_json`] on a parsed value.
    pub fn from_value(v: &Value) -> Result<BoxDef, BoxError> {
        let o = v
            .as_object()
            .ok_or_else(|| BoxError::Schema("a box is a JSON object".into()))?;
        for k in o.keys() {
            if !FIELDS.contains(&k.as_str()) {
                return Err(BoxError::Schema(format!("unknown field {k:?}")));
            }
        }
        let schema = req_string(o, "schema")?;
        if schema != BOX_SCHEMA {
            return Err(BoxError::Schema(format!(
                "schema {schema:?}, expected {BOX_SCHEMA:?}"
            )));
        }
        let measure = req_string(o, "measure")?;
        if measure != BOX_MEASURE {
            return Err(BoxError::Schema(format!(
                "measure {measure:?}, expected {BOX_MEASURE:?}"
            )));
        }
        let chart = parse_chart(&req_string(o, "chart")?)?;
        let layers = parse_layers(
            o.get("layers")
                .ok_or_else(|| BoxError::Schema("missing \"layers\"".into()))?,
            "layers",
        )?;
        let mut sectors = vec![];
        match o.get("sectors") {
            None | Some(Value::Null) => {}
            Some(Value::Array(a)) => {
                for s in a {
                    let so = s
                        .as_object()
                        .ok_or_else(|| BoxError::Schema("sector: expected an object".into()))?;
                    for k in so.keys() {
                        if k != "support" && k != "layers" {
                            return Err(BoxError::Schema(format!("sector: unknown field {k:?}")));
                        }
                    }
                    let support = so
                        .get("support")
                        .and_then(Value::as_array)
                        .ok_or_else(|| {
                            BoxError::Schema("sector: missing \"support\" array".into())
                        })?
                        .iter()
                        .map(|x| {
                            x.as_u64().map(|x| x as usize).ok_or_else(|| {
                                BoxError::Schema(format!("sector support entry {x}"))
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let sl = parse_layers(
                        so.get("layers")
                            .ok_or_else(|| BoxError::Schema("sector: missing \"layers\"".into()))?,
                        "sector layers",
                    )?;
                    sectors.push(SectorLayers {
                        support,
                        layers: sl,
                    });
                }
            }
            Some(v) => {
                return Err(BoxError::Schema(format!(
                    "sectors: expected an array, got {v}"
                )));
            }
        }
        let cap = match o.get("cap") {
            None | Some(Value::Null) => None,
            Some(c) => Some(as_u32(c, "cap")?),
        };
        let slot_caps = match o.get("slot_caps") {
            None | Some(Value::Null) => vec![],
            Some(Value::Array(a)) => a
                .iter()
                .map(|c| match c {
                    Value::Null => Ok(None),
                    c => as_u32(c, "slot_caps").map(Some),
                })
                .collect::<Result<_, _>>()?,
            Some(v) => {
                return Err(BoxError::Schema(format!(
                    "slot_caps: expected an array, got {v}"
                )));
            }
        };
        let keys = match o.get("keys") {
            None | Some(Value::Null) => vec![],
            Some(Value::Array(a)) => a.iter().map(parse_key_value).collect::<Result<_, _>>()?,
            Some(v) => {
                return Err(BoxError::Schema(format!(
                    "keys: expected an array, got {v}"
                )));
            }
        };
        let b = BoxDef {
            id: req_string(o, "id")?,
            name: req_string(o, "name")?,
            chart,
            layers,
            sectors,
            cap,
            slot_caps,
            keys,
            origin: opt_string(o, "origin")?,
            derived_from: opt_string(o, "derived_from")?,
        };
        b.validate()?;
        let b = b.normalised();
        if let Some(d) = opt_string(o, "digest")? {
            let c = b.digest();
            if d != c {
                return Err(BoxError::Digest {
                    expected: d,
                    computed: c,
                });
            }
        }
        Ok(b)
    }

    /// Structural checks (interface 7.1).
    pub fn validate(&self) -> Result<(), BoxError> {
        let n = self.chart.len();
        if n == 0 || n > MAX_BOX_ARITY {
            return Err(BoxError::Invalid(format!(
                "chart of length {n} (1..={MAX_BOX_ARITY})"
            )));
        }
        if !self.chart.iter().any(|&l| l) {
            return Err(BoxError::Invalid("chart has no line".into()));
        }
        if self.id.is_empty() {
            return Err(BoxError::Invalid("empty id".into()));
        }
        if self.cap == Some(0) {
            return Err(BoxError::Invalid(
                "cap 0 (a support line has power >= 1)".into(),
            ));
        }
        if !self.slot_caps.is_empty() && self.slot_caps.len() != n {
            return Err(BoxError::Invalid(format!(
                "slot_caps has {} entries for {n} slots",
                self.slot_caps.len()
            )));
        }
        for (i, c) in self.slot_caps.iter().enumerate() {
            if self.chart[i] && *c == Some(0) {
                return Err(BoxError::Invalid(format!(
                    "slot_caps[{i}] = 0 on a line slot"
                )));
            }
        }
        let n_lines = self.n_lines();
        let mut seen = std::collections::BTreeSet::new();
        for s in &self.sectors {
            let mut sup = s.support.clone();
            sup.sort_unstable();
            sup.dedup();
            if sup.len() != s.support.len() {
                return Err(BoxError::Invalid(format!(
                    "sector support {:?} repeats a slot",
                    s.support
                )));
            }
            if let Some(&x) = sup.iter().find(|&&x| x >= n || !self.chart[x]) {
                return Err(BoxError::Invalid(format!(
                    "sector support {:?}: slot {x} is not a line of the chart",
                    s.support
                )));
            }
            if sup.len() >= n_lines {
                return Err(BoxError::Invalid(format!(
                    "sector support {:?} is not a proper subsector (use \"layers\" for the top sector)",
                    s.support
                )));
            }
            if !seen.insert(sup) {
                return Err(BoxError::Invalid(format!(
                    "sector support {:?} listed twice",
                    s.support
                )));
            }
        }
        for k in &self.keys {
            if k.len() != n {
                return Err(BoxError::Arity {
                    expected: n,
                    got: k.len(),
                });
            }
        }
        Ok(())
    }

    /// The normal form: layers merged per D (max r_max) and sorted, sectors sorted, explicit keys sorted and unique.
    pub fn normalised(&self) -> BoxDef {
        let mut b = self.clone();
        b.layers = merge_layers(&b.layers);
        for s in &mut b.sectors {
            s.support.sort_unstable();
            s.layers = merge_layers(&s.layers);
        }
        b.sectors.sort();
        b.keys.sort();
        b.keys.dedup();
        if b.slot_caps.iter().all(Option::is_none) {
            b.slot_caps.clear();
        }
        b
    }

    /// Cap of a slot when it is a support line (largest power) or another slot (largest |value|).
    fn slot_cap(&self, slot: usize, support: bool) -> Option<u32> {
        let s = self.slot_caps.get(slot).copied().flatten();
        if support {
            match (self.cap, s) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            }
        } else {
            s
        }
    }

    fn value_without_digest(&self) -> Value {
        let b = self.normalised();
        let mut o = Map::new();
        o.insert("schema".into(), json!(BOX_SCHEMA));
        o.insert("id".into(), json!(b.id));
        o.insert("name".into(), json!(b.name));
        o.insert("measure".into(), json!(BOX_MEASURE));
        o.insert("chart".into(), json!(chart_string(&b.chart)));
        o.insert("layers".into(), layers_value(&b.layers));
        o.insert(
            "sectors".into(),
            Value::Array(
                b.sectors
                    .iter()
                    .map(|s| json!({"support": s.support, "layers": layers_value(&s.layers)}))
                    .collect(),
            ),
        );
        o.insert("cap".into(), b.cap.map_or(Value::Null, |c| json!(c)));
        o.insert(
            "slot_caps".into(),
            Value::Array(
                b.slot_caps
                    .iter()
                    .map(|c| c.map_or(Value::Null, |c| json!(c)))
                    .collect(),
            ),
        );
        o.insert(
            "keys".into(),
            Value::Array(b.keys.iter().map(|k| key_value(k)).collect()),
        );
        o.insert("origin".into(), b.origin.map_or(Value::Null, Value::String));
        o.insert(
            "derived_from".into(),
            b.derived_from.map_or(Value::Null, Value::String),
        );
        Value::Object(o)
    }

    /// Canonical JSON without the digest field (the hashed bytes).
    pub fn canonical_json(&self) -> String {
        canonical_json(&self.value_without_digest())
    }

    /// BLAKE3 (hex) of [`BoxDef::canonical_json`].
    pub fn digest(&self) -> String {
        blake3_hex(self.canonical_json().as_bytes())
    }

    /// The document with its digest.
    pub fn to_value(&self) -> Value {
        let mut v = self.value_without_digest();
        v.as_object_mut()
            .expect("object")
            .insert("digest".into(), json!(self.digest()));
        v
    }

    /// Canonical JSON with the digest.
    pub fn to_json(&self) -> String {
        canonical_json(&self.to_value())
    }

    pub fn arity(&self) -> usize {
        self.chart.len()
    }
    pub fn n_lines(&self) -> usize {
        self.chart.iter().filter(|&&l| l).count()
    }

    /// The box was written for this frame's line mask.
    pub fn check_frame(&self, frame: &NodeFrame) -> Result<(), BoxError> {
        // epsframe: the box predicates read line signs (an eps axis is forced and two-sided; BOXDEF B is not
        // implemented here)
        frame
            .require_integer_only_for("the box (boxdef: integer frames only)")
            .map_err(|e| BoxError::Invalid(e.to_string()))?;
        if frame.mask() != self.chart.as_slice() {
            return Err(BoxError::Invalid(format!(
                "box chart {} differs from the frame mask {}",
                chart_string(&self.chart),
                frame.mask_string()
            )));
        }
        Ok(())
    }

    fn sections(&self) -> Vec<Section> {
        let b = self.normalised();
        let n = b.chart.len();
        let mut out = vec![Section::new(
            &b,
            (0..n).filter(|&i| b.chart[i]).collect(),
            b.layers.clone(),
        )];
        for s in &b.sectors {
            out.push(Section::new(&b, s.support.clone(), s.layers.clone()));
        }
        out
    }

    /// The key lies in the layered part (top sector or a listed sector).
    fn in_layers(&self, key: &[i32]) -> bool {
        let n = self.chart.len();
        if key.len() != n {
            return false;
        }
        let top = (0..n).all(|i| !self.chart[i] || key[i] >= 1);
        let layers: &[[u32; 2]] = if top {
            &self.layers
        } else {
            let sup: Vec<usize> = (0..n).filter(|&i| self.chart[i] && key[i] >= 1).collect();
            match self.sectors.iter().find(|s| s.support == sup) {
                Some(s) => &s.layers,
                None => return false,
            }
        };
        let (mut d, mut r) = (0i64, 0i64);
        for (i, &x) in key.iter().enumerate() {
            if self.chart[i] && x >= 1 {
                if self
                    .slot_cap(i, true)
                    .is_some_and(|c| i64::from(x) > i64::from(c))
                {
                    return false;
                }
                d += i64::from(x) - 1;
            } else {
                if x > 0
                    || self
                        .slot_cap(i, false)
                        .is_some_and(|c| -i64::from(x) > i64::from(c))
                {
                    return false;
                }
                r -= i64::from(x);
            }
        }
        layers
            .iter()
            .any(|&[ld, lr]| i64::from(ld) == d && r <= i64::from(lr))
    }

    /// Membership (interface 7.2). False on a wrong-length key. Works on the box as stored (layers need not be
    /// normalised: union semantics; sector supports must be sorted, as [`BoxDef::normalised`] makes them).
    pub fn contains(&self, key: &[i32]) -> bool {
        if key.len() != self.chart.len() {
            return false;
        }
        if self.in_layers(key) {
            return true;
        }
        self.keys.iter().any(|k| k.as_slice() == key)
    }

    /// Streaming enumeration (interface 7.3).
    pub fn iter(&self) -> BoxKeys {
        let b = self.normalised();
        let layered_only = BoxDef {
            keys: vec![],
            ..b.clone()
        };
        let extra: Vec<Key> = b
            .keys
            .iter()
            .filter(|k| !layered_only.in_layers(k))
            .cloned()
            .collect();
        BoxKeys {
            sections: self.sections(),
            extra,
            sec: 0,
            layer: 0,
            r: 0,
            lines: None,
            others: None,
            extra_pos: 0,
            arity: b.chart.len(),
        }
    }

    /// Visit every key without allocating one vector per key.
    pub fn for_each_key(&self, mut f: impl FnMut(&[i32])) {
        let mut it = self.iter();
        let mut buf = vec![0i32; self.arity()];
        while it.next_into(&mut buf) {
            f(&buf);
        }
    }

    /// Per (section, D, r) counts, combinatorially (interface 7.4). Section 0 = top sector, then the sectors in
    /// normalised order; explicit keys outside the layered part are not included (see [`BoxDef::count`]).
    pub fn layer_counts(&self) -> Vec<LayerCount> {
        let mut out = vec![];
        for (si, s) in self.sections().iter().enumerate() {
            let dmax = s.layers.iter().map(|l| l[0]).max().unwrap_or(0);
            let rmax_all = s.layers.iter().map(|l| l[1]).max().unwrap_or(0);
            let nls = compositions_counts(u64::from(dmax), &s.line_caps);
            let nrs = compositions_counts(u64::from(rmax_all), &s.other_caps);
            for &[d, rmax] in &s.layers {
                let nl = nls[d as usize];
                for r in 0..=rmax {
                    let nr = nrs[r as usize];
                    out.push(LayerCount {
                        section: si,
                        dots: d,
                        rank: r,
                        count: nl.checked_mul(nr).expect("box count overflows u128"),
                    });
                }
            }
        }
        out
    }

    /// Exact number of keys (no enumeration).
    pub fn count(&self) -> u128 {
        let layered: u128 = self.layer_counts().iter().map(|c| c.count).sum();
        let b = self.normalised();
        let lo = BoxDef {
            keys: vec![],
            ..b.clone()
        };
        layered + b.keys.iter().filter(|k| !lo.in_layers(k)).count() as u128
    }

    /// Saturation (plan 2.4.8; interface 7.6): the smallest box of the same shape containing this box and every
    /// absorbable key.
    pub fn saturate<'a>(&self, keys: impl IntoIterator<Item = &'a [i32]>) -> Saturation {
        let mut b = self.normalised();
        let n = b.chart.len();
        let parent = self.digest();
        let mut top: BTreeMap<u32, u32> = b.layers.iter().map(|&[d, r]| (d, r)).collect();
        let mut secs: Vec<BTreeMap<u32, u32>> = b
            .sectors
            .iter()
            .map(|s| s.layers.iter().map(|&[d, r]| (d, r)).collect())
            .collect();
        let mut cap = b.cap;
        let mut not_absorbed = vec![];
        let (mut absorbed, mut inside) = (0u64, 0u64);
        for k in keys {
            if b.contains(k) {
                inside += 1;
                continue;
            }
            if k.len() != n {
                not_absorbed.push(k.to_vec());
                continue;
            }
            let sup: Vec<usize> = (0..n).filter(|&i| b.chart[i] && k[i] >= 1).collect();
            let map = if sup.len() == b.n_lines() {
                Some(&mut top)
            } else {
                b.sectors
                    .iter()
                    .position(|s| s.support == sup)
                    .map(|p| &mut secs[p])
            };
            let positive = (0..n).any(|i| !(b.chart[i] && k[i] >= 1) && k[i] > 0);
            let over_slot_cap = (0..n).any(|i| {
                let sup_i = b.chart[i] && k[i] >= 1;
                b.slot_caps.get(i).copied().flatten().is_some_and(|c| {
                    if sup_i {
                        k[i] as i64 > i64::from(c)
                    } else {
                        -(k[i] as i64) > i64::from(c)
                    }
                })
            });
            let Some(map) = map.filter(|_| !positive && !over_slot_cap) else {
                not_absorbed.push(k.to_vec());
                continue;
            };
            let d: i64 = sup.iter().map(|&i| i64::from(k[i]) - 1).sum();
            let r: i64 = (0..n)
                .filter(|&i| !(b.chart[i] && k[i] >= 1))
                .map(|i| -i64::from(k[i]))
                .sum();
            let (Ok(d), Ok(r)) = (u32::try_from(d), u32::try_from(r)) else {
                not_absorbed.push(k.to_vec());
                continue;
            };
            let e = map.entry(d).or_insert(r);
            *e = (*e).max(r);
            let pmax = sup.iter().map(|&i| k[i] as u32).max().unwrap_or(1);
            if let Some(c) = cap {
                cap = Some(c.max(pmax));
            }
            absorbed += 1;
        }
        let mut added = vec![];
        let old: BTreeMap<u32, u32> = b.layers.iter().map(|&[d, r]| (d, r)).collect();
        for (&d, &r) in &top {
            if old.get(&d) != Some(&r) {
                added.push(LayerChange {
                    section: 0,
                    dots: d,
                    old_r_max: old.get(&d).copied(),
                    new_r_max: r,
                });
            }
        }
        for (si, m) in secs.iter().enumerate() {
            let old: BTreeMap<u32, u32> =
                b.sectors[si].layers.iter().map(|&[d, r]| (d, r)).collect();
            for (&d, &r) in m {
                if old.get(&d) != Some(&r) {
                    added.push(LayerChange {
                        section: si + 1,
                        dots: d,
                        old_r_max: old.get(&d).copied(),
                        new_r_max: r,
                    });
                }
            }
        }
        let raised_cap = match (b.cap, cap) {
            (Some(a), Some(c)) if a != c => Some((a, c)),
            _ => None,
        };
        let changed = !added.is_empty() || raised_cap.is_some();
        if changed {
            b.layers = top.into_iter().map(|(d, r)| [d, r]).collect();
            for (s, m) in b.sectors.iter_mut().zip(secs) {
                s.layers = m.into_iter().map(|(d, r)| [d, r]).collect();
            }
            b.cap = cap;
            if !b.id.ends_with("+sat") {
                b.id = format!("{}+sat", b.id);
            }
            b.origin = Some(format!("saturate({parent})"));
            b.derived_from = Some(parent);
        }
        Saturation {
            boxdef: b,
            added,
            raised_cap,
            not_absorbed,
            absorbed,
            already_inside: inside,
        }
    }

    /// The `box` object of the engine's `boxed_node` header: `{name, measure, layers, cap}` (top sector only).
    pub fn to_boxed_header_box(&self) -> Result<Value, BoxError> {
        if !self.sectors.is_empty() {
            return Err(BoxError::NotHeaderable(
                "the box has subsector layers".into(),
            ));
        }
        if !self.keys.is_empty() {
            return Err(BoxError::NotHeaderable("the box has explicit keys".into()));
        }
        let b = self.normalised();
        Ok(json!({
            "name": b.id,
            "measure": BOX_MEASURE,
            "layers": layers_value(&b.layers),
            "cap": b.cap.map_or(Value::Null, |c| json!(c)),
        }))
    }

    /// A box from the engine's `boxed_node` header box and the header's chart; id and name = the header's `name`.
    pub fn from_boxed_header_box(bx: &Value, chart: &[bool]) -> Result<BoxDef, BoxError> {
        let o = bx
            .as_object()
            .ok_or_else(|| BoxError::Schema("boxed_node box: expected an object".into()))?;
        let measure = req_string(o, "measure")?;
        if measure != BOX_MEASURE {
            return Err(BoxError::Schema(format!(
                "boxed_node box measure {measure:?}"
            )));
        }
        let name = req_string(o, "name")?;
        let layers = parse_layers(
            o.get("layers")
                .ok_or_else(|| BoxError::Schema("boxed_node box: missing layers".into()))?,
            "boxed_node layers",
        )?;
        let cap = match o.get("cap") {
            None | Some(Value::Null) => None,
            Some(c) => {
                let c = c
                    .as_i64()
                    .ok_or_else(|| BoxError::Schema(format!("boxed_node cap {c}")))?;
                // the engine's cap is an i64 on every line power; a cap below 1 admits no key
                Some(
                    u32::try_from(c.max(0))
                        .map_err(|_| BoxError::Schema(format!("boxed_node cap {c}")))?,
                )
            }
        };
        let b = BoxDef {
            id: name.clone(),
            name,
            chart: chart.to_vec(),
            layers,
            sectors: vec![],
            cap,
            slot_caps: vec![],
            keys: vec![],
            origin: Some("boxed_node header".into()),
            derived_from: None,
        };
        if b.cap == Some(0) {
            return Err(BoxError::Invalid("boxed_node cap < 1 admits no key".into()));
        }
        b.validate()?;
        Ok(b.normalised())
    }

    /// A band box from clauses (interface 7.5).
    pub fn from_band(spec: &BandSpec, chart: &[bool]) -> Result<BoxDef, BoxError> {
        let e = chart.iter().filter(|&&l| l).count() as i64;
        let mut layers = vec![];
        for c in &spec.clauses {
            if c.d_lo > c.d_hi {
                return Err(BoxError::Invalid(format!(
                    "clause {}: D range [{}, {}]",
                    c.name, c.d_lo, c.d_hi
                )));
            }
            for d in c.d_lo..=c.d_hi {
                let r = c.r_e * e + c.r_d * i64::from(d) + c.r_const;
                if r >= 0 {
                    layers.push([
                        d,
                        u32::try_from(r).map_err(|_| BoxError::Invalid("r_max".into()))?,
                    ]);
                }
            }
        }
        let b = BoxDef {
            id: format!("{}-E{e}", spec.id),
            name: spec.name.clone(),
            chart: chart.to_vec(),
            layers,
            sectors: vec![],
            cap: spec.cap,
            slot_caps: vec![],
            keys: vec![],
            origin: Some(format!("band({}, E={e})", spec.id)),
            derived_from: None,
        };
        b.validate()?;
        Ok(b.normalised())
    }
}

/// One (section, D, r) count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerCount {
    pub section: usize,
    pub dots: u32,
    pub rank: u32,
    pub count: u128,
}

/// A layer added or widened by saturation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerChange {
    pub section: usize,
    pub dots: u32,
    pub old_r_max: Option<u32>,
    pub new_r_max: u32,
}

/// Result of [`BoxDef::saturate`].
#[derive(Clone, Debug)]
pub struct Saturation {
    /// B*: equal to the (normalised) input box when nothing changed.
    pub boxdef: BoxDef,
    pub added: Vec<LayerChange>,
    pub raised_cap: Option<(u32, u32)>,
    /// Keys that no section of the box can take (unlisted support, positive numerator, wrong length).
    pub not_absorbed: Vec<Key>,
    pub absorbed: u64,
    pub already_inside: u64,
}

impl Saturation {
    /// Every key was already inside the box.
    pub fn is_fixed_point(&self) -> bool {
        self.added.is_empty() && self.raised_cap.is_none() && self.not_absorbed.is_empty()
    }
}

/// One clause of a band (interface 7.5): for D in [d_lo, d_hi], r_max = r_e * E + r_d * D + r_const.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BandClause {
    pub name: String,
    pub d_lo: u32,
    pub d_hi: u32,
    pub r_e: i64,
    pub r_d: i64,
    pub r_const: i64,
}

// ===================================================================================================================
// EPS BOXES (lane epsframe): the box of a node of an eps-shifted (two-sided) family as DATA (BOXDEF section B,
// rule L15). The integer box above reads line signs and refuses eps frames; an eps box measures the eps axes by its
// own data-given rule. Nothing here knows a node: the constants of the bound, the carrier input and the eps-axis
// rule are fields of the clause document.
// ===================================================================================================================

/// Schema of an eps box document.
pub const EPS_BOX_SCHEMA: &str = "rustred.campaign.eps-box.v1";
/// Schema of an eps box CLAUSE (the generator's input: the bound as coefficients, the carrier term as an input).
pub const EPS_BOX_CLAUSE_SCHEMA: &str = "rustred.campaign.eps-box-clause.v1";
/// The measure of an eps box: D and r of the integer slots as `dots-rank-v1` on the top support, plus the eps-axis
/// contributions of [`EpsAxisRule`].
pub const EPS_BOX_MEASURE: &str = "dots-rank-eps-v1";

/// Where the dots of an eps axis of weight w start (data: `dots_floor`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EpsDotsFloor {
    /// D counted from 1 - w (`"one_minus_weight"`).
    OneMinusWeight,
    /// D counted from 1, as on an integer line (`"one"`).
    One,
}

/// How an eps axis (integer part n, weight w) enters the measure: D += max(0, n - floor), and, with
/// `rank_from_nonpositive`, r += max(0, -n).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EpsAxisRule {
    pub dots_floor: EpsDotsFloor,
    pub rank_from_nonpositive: bool,
}

impl EpsAxisRule {
    fn floor(&self, w: u32) -> i64 {
        match self.dots_floor {
            EpsDotsFloor::OneMinusWeight => 1 - i64::from(w),
            EpsDotsFloor::One => 1,
        }
    }
    fn contribution(&self, n: i32, w: u32) -> (i64, i64) {
        let n = i64::from(n);
        let d = (n - self.floor(w)).max(0);
        let r = if self.rank_from_nonpositive {
            (-n).max(0)
        } else {
            0
        };
        (d, r)
    }
    fn from_object(o: &Map<String, Value>) -> Result<EpsAxisRule, BoxError> {
        let dots_floor = match o.get("dots_floor").and_then(Value::as_str) {
            Some("one_minus_weight") => EpsDotsFloor::OneMinusWeight,
            Some("one") => EpsDotsFloor::One,
            other => {
                return Err(BoxError::Schema(format!(
                    "dots_floor {other:?}: expected \"one_minus_weight\" or \"one\""
                )));
            }
        };
        let rank_from_nonpositive = o
            .get("rank_from_nonpositive")
            .and_then(Value::as_bool)
            .ok_or_else(|| BoxError::Schema("rank_from_nonpositive: expected a boolean".into()))?;
        if !rank_from_nonpositive {
            // integer parts below the dots floor would then cost nothing: the box would be infinite
            return Err(BoxError::Invalid(
                "rank_from_nonpositive false: the eps box would be unbounded below the dots floor"
                    .into(),
            ));
        }
        Ok(EpsAxisRule {
            dots_floor,
            rank_from_nonpositive,
        })
    }
    fn to_fields(self, o: &mut Map<String, Value>) {
        o.insert(
            "dots_floor".into(),
            json!(match self.dots_floor {
                EpsDotsFloor::OneMinusWeight => "one_minus_weight",
                EpsDotsFloor::One => "one",
            }),
        );
        o.insert(
            "rank_from_nonpositive".into(),
            json!(self.rank_from_nonpositive),
        );
    }
}

/// The box of an eps node: its TOP support (every integer line >= 1, every numerator <= 0, eps axes any integer:
/// the eps lines never drop to a subsector), layers `[D, r_max]` (union semantics, exact D) under
/// [`EPS_BOX_MEASURE`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EpsBox {
    pub id: String,
    /// `chart[slot]` = the slot is a line (eps axes are lines).
    pub chart: Vec<bool>,
    /// (slot, weight w) of every eps axis, ascending by slot.
    pub eps: Vec<(usize, u32)>,
    pub rule: EpsAxisRule,
    pub layers: Vec<[u32; 2]>,
    pub origin: Option<String>,
}

/// The generator's input (BOXDEF section B as coefficients): for D = 0..=d_max, r_max(D) = d_coef*D + e_coef*E +
/// eps_coef*e + constant (a negative bound = an empty layer), d_max = d_max_constant + carriers; E = number of lines of
/// the top support (eps axes included), e = the sum of the eps weights; the weight of an eps axis is its kshift k.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EpsBoxClause {
    pub d_coef: i64,
    pub e_coef: i64,
    pub eps_coef: i64,
    pub constant: i64,
    pub d_max_constant: i64,
    /// The carrier term (an input from the node's graph; 0 on a clean support).
    pub carriers: i64,
    pub rule: EpsAxisRule,
    /// Where the clause comes from (e.g. "BOXDEF.v3 B, reading of 2026-11-15").
    pub origin: String,
}

impl EpsBoxClause {
    pub fn from_json(text: &str) -> Result<EpsBoxClause, BoxError> {
        let v: Value = serde_json::from_str(text).map_err(|e| BoxError::Json(e.to_string()))?;
        let o = v
            .as_object()
            .ok_or_else(|| BoxError::Schema("an eps box clause is a JSON object".into()))?;
        if o.get("schema").and_then(Value::as_str) != Some(EPS_BOX_CLAUSE_SCHEMA) {
            return Err(BoxError::Schema(format!(
                "schema: expected {EPS_BOX_CLAUSE_SCHEMA:?}"
            )));
        }
        let int = |k: &str| -> Result<i64, BoxError> {
            o.get(k)
                .and_then(Value::as_i64)
                .ok_or_else(|| BoxError::Schema(format!("{k}: expected an integer")))
        };
        Ok(EpsBoxClause {
            d_coef: int("d_coef")?,
            e_coef: int("e_coef")?,
            eps_coef: int("eps_coef")?,
            constant: int("constant")?,
            d_max_constant: int("d_max_constant")?,
            carriers: int("carriers")?,
            rule: EpsAxisRule::from_object(o)?,
            origin: req_string(o, "origin")?,
        })
    }
}

impl EpsBox {
    /// The box of `frame` under `clause` (the frame must have eps axes; weights = kshift, which must be positive).
    pub fn from_clause(
        frame: &NodeFrame,
        clause: &EpsBoxClause,
        id: &str,
    ) -> Result<EpsBox, BoxError> {
        if !frame.has_eps_axes() {
            return Err(BoxError::Invalid(
                "an eps box needs a frame with eps axes".into(),
            ));
        }
        let mut eps = vec![];
        for a in frame.eps_axes() {
            let k = frame.kshift()[a];
            let w = u32::try_from(k).map_err(|_| {
                BoxError::Invalid(format!(
                    "eps axis {a} with shift {k} eps: a negative weight is not defined by the clause"
                ))
            })?;
            eps.push((a, w));
        }
        let e_lines = frame.n_lines() as i64;
        let e_sum: i64 = eps.iter().map(|&(_, w)| i64::from(w)).sum();
        let d_max = clause.d_max_constant + clause.carriers;
        let mut layers = vec![];
        for d in 0..=d_max.max(-1) {
            let r = clause.d_coef * d
                + clause.e_coef * e_lines
                + clause.eps_coef * e_sum
                + clause.constant;
            if r >= 0 {
                layers.push([d as u32, r as u32]);
            }
        }
        Ok(EpsBox {
            id: id.to_string(),
            chart: frame.mask().to_vec(),
            eps,
            rule: clause.rule,
            layers,
            origin: Some(clause.origin.clone()),
        })
    }
    /// The box was written for this frame: same line mask, same eps axes, weights = kshift.
    pub fn check_frame(&self, frame: &NodeFrame) -> Result<(), BoxError> {
        let want: Vec<(usize, u32)> = frame
            .eps_axes()
            .into_iter()
            .map(|a| (a, u32::try_from(frame.kshift()[a]).unwrap_or(u32::MAX)))
            .collect();
        if frame.mask() != self.chart.as_slice() || want != self.eps {
            return Err(BoxError::Invalid(format!(
                "eps box (chart {}, eps {:?}) differs from the frame (mask {}, eps {:?})",
                chart_string(&self.chart),
                self.eps,
                frame.mask_string(),
                want
            )));
        }
        Ok(())
    }
    fn weight(&self, slot: usize) -> Option<u32> {
        self.eps.iter().find(|&&(a, _)| a == slot).map(|&(_, w)| w)
    }
    /// (D, r) of a top-support key; `None` for a wrong length, an integer line <= 0 or a numerator > 0.
    pub fn measure(&self, key: &[i32]) -> Option<(i64, i64)> {
        if key.len() != self.chart.len() {
            return None;
        }
        let (mut d, mut r) = (0i64, 0i64);
        for (i, &x) in key.iter().enumerate() {
            if let Some(w) = self.weight(i) {
                let (dd, rr) = self.rule.contribution(x, w);
                d += dd;
                r += rr;
            } else if self.chart[i] {
                if x < 1 {
                    return None;
                }
                d += i64::from(x) - 1;
            } else {
                if x > 0 {
                    return None;
                }
                r -= i64::from(x);
            }
        }
        Some((d, r))
    }
    /// Membership: the measure lands in a layer (exact D, r <= r_max).
    pub fn contains(&self, key: &[i32]) -> bool {
        self.measure(key).is_some_and(|(d, r)| {
            self.layers
                .iter()
                .any(|&[ld, lr]| i64::from(ld) == d && r <= i64::from(lr))
        })
    }
    /// Every key of the box, each once (layers merged per D), in no particular order.
    pub fn for_each_key(&self, mut f: impl FnMut(&[i32])) {
        let layers = merge_layers(&self.layers);
        let n = self.chart.len();
        let mut key = vec![0i32; n];
        for &[d, rmax] in &layers {
            self.fill(0, i64::from(d), i64::from(rmax), &mut key, &mut f);
        }
    }
    fn fill(
        &self,
        slot: usize,
        d_left: i64,
        r_left: i64,
        key: &mut Vec<i32>,
        f: &mut impl FnMut(&[i32]),
    ) {
        if slot == key.len() {
            if d_left == 0 {
                f(key);
            }
            return;
        }
        if let Some(w) = self.weight(slot) {
            // every integer part whose contributions fit: n from -(r budget) (or the floor) up to floor + D budget
            // (rank_from_nonpositive is always true: EpsAxisRule::from_object refuses false)
            let lo = -r_left;
            let hi = self.rule.floor(w) + d_left;
            for x in lo..=hi {
                let (dd, rr) = self.rule.contribution(x as i32, w);
                if dd <= d_left && rr <= r_left {
                    key[slot] = x as i32;
                    self.fill(slot + 1, d_left - dd, r_left - rr, key, f);
                }
            }
        } else if self.chart[slot] {
            for dd in 0..=d_left {
                key[slot] = 1 + dd as i32;
                self.fill(slot + 1, d_left - dd, r_left, key, f);
            }
        } else {
            for rr in 0..=r_left {
                key[slot] = -(rr as i32);
                self.fill(slot + 1, d_left, r_left - rr, key, f);
            }
        }
    }
    /// The number of keys.
    pub fn count(&self) -> u64 {
        let mut c = 0u64;
        self.for_each_key(|_| c += 1);
        c
    }
    /// The document (schema [`EPS_BOX_SCHEMA`]).
    pub fn to_value(&self) -> Value {
        let mut o = Map::new();
        o.insert("schema".into(), json!(EPS_BOX_SCHEMA));
        o.insert("id".into(), json!(self.id));
        o.insert("measure".into(), json!(EPS_BOX_MEASURE));
        o.insert("chart".into(), json!(chart_string(&self.chart)));
        o.insert(
            "eps".into(),
            Value::Array(self.eps.iter().map(|&(a, w)| json!([a, w])).collect()),
        );
        self.rule.to_fields(&mut o);
        o.insert("layers".into(), layers_value(&merge_layers(&self.layers)));
        o.insert(
            "origin".into(),
            self.origin.clone().map_or(Value::Null, Value::String),
        );
        Value::Object(o)
    }
}

/// A band preset (data, e.g. the R* box of the spec).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BandSpec {
    pub id: String,
    pub name: String,
    pub clauses: Vec<BandClause>,
    pub cap: Option<u32>,
}

impl BandSpec {
    /// `{"id", "name", "clauses": [{"name", "D": [lo, hi], "r_max": {"E", "D", "const"}}], "cap"}`.
    pub fn from_value(v: &Value) -> Result<BandSpec, BoxError> {
        let o = v
            .as_object()
            .ok_or_else(|| BoxError::Schema("band: expected an object".into()))?;
        let int = |v: Option<&Value>, what: &str| -> Result<i64, BoxError> {
            v.and_then(Value::as_i64)
                .ok_or_else(|| BoxError::Schema(format!("band clause: {what} must be an integer")))
        };
        let mut clauses = vec![];
        for c in o
            .get("clauses")
            .and_then(Value::as_array)
            .ok_or_else(|| BoxError::Schema("band: missing clauses".into()))?
        {
            let co = c
                .as_object()
                .ok_or_else(|| BoxError::Schema("band clause: expected an object".into()))?;
            let dr = co
                .get("D")
                .and_then(Value::as_array)
                .filter(|a| a.len() == 2)
                .ok_or_else(|| BoxError::Schema("band clause: D must be [lo, hi]".into()))?;
            let rm = co
                .get("r_max")
                .and_then(Value::as_object)
                .ok_or_else(|| BoxError::Schema("band clause: missing r_max".into()))?;
            clauses.push(BandClause {
                name: co
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                d_lo: as_u32(&dr[0], "D lo")?,
                d_hi: as_u32(&dr[1], "D hi")?,
                r_e: int(rm.get("E"), "r_max.E")?,
                r_d: int(rm.get("D"), "r_max.D")?,
                r_const: int(rm.get("const"), "r_max.const")?,
            });
        }
        Ok(BandSpec {
            id: req_string(o, "id")?,
            name: req_string(o, "name")?,
            clauses,
            cap: match o.get("cap") {
                None | Some(Value::Null) => None,
                Some(c) => Some(as_u32(c, "cap")?),
            },
        })
    }
}

// ------------------------------------------------------------------------------------------------ counting

/// C(n, k) in u128 (exact; panics on overflow).
fn binom(n: u64, k: u64) -> u128 {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    let mut r: u128 = 1;
    for i in 0..k {
        r = r
            .checked_mul(u128::from(n - i))
            .expect("binomial overflows u128")
            / u128::from(i + 1);
    }
    r
}

/// Number of compositions of `total` into `parts` parts, each at most `cap` (None: unbounded), by
/// inclusion-exclusion (cross-check of [`compositions_counts`]).
pub fn capped_compositions(total: u64, parts: u64, cap: Option<u64>) -> u128 {
    if parts == 0 {
        return u128::from(total == 0);
    }
    let Some(c) = cap else {
        return binom(total + parts - 1, parts - 1);
    };
    let mut s: i128 = 0;
    for j in 0..=parts {
        let Some(rest) = total.checked_sub(j * (c + 1)) else {
            break;
        };
        let t = (binom(parts, j) * binom(rest + parts - 1, parts - 1)) as i128;
        if j % 2 == 0 {
            s += t;
        } else {
            s -= t;
        }
    }
    s as u128
}

/// `out[t]` = number of vectors with entry j in [0, caps[j]] (None: unbounded) summing to t, for t = 0..=max_total
/// (dynamic programming over the parts; exact for any per-part caps).
pub fn compositions_counts(max_total: u64, caps: &[Option<u32>]) -> Vec<u128> {
    let n = max_total as usize + 1;
    let mut cur = vec![0u128; n];
    cur[0] = 1;
    for c in caps {
        let mut next = vec![0u128; n];
        match c {
            None => {
                // prefix sums: next[t] = sum_{u <= t} cur[u]
                let mut acc = 0u128;
                for t in 0..n {
                    acc = acc.checked_add(cur[t]).expect("box count overflows u128");
                    next[t] = acc;
                }
            }
            Some(c) => {
                let c = *c as usize;
                let mut acc = 0u128;
                for t in 0..n {
                    acc = acc.checked_add(cur[t]).expect("box count overflows u128");
                    if t > c {
                        acc -= cur[t - c - 1];
                    }
                    next[t] = acc;
                }
            }
        }
        cur = next;
    }
    cur
}

// ------------------------------------------------------------------------------------------------ enumeration

struct Section {
    support: Vec<usize>,
    others: Vec<usize>,
    layers: Vec<[u32; 2]>,
    /// Dot caps of the support lines (power cap - 1).
    line_caps: Vec<Option<u32>>,
    /// |value| caps of the other slots.
    other_caps: Vec<Option<u32>>,
}

impl Section {
    fn new(b: &BoxDef, support: Vec<usize>, layers: Vec<[u32; 2]>) -> Section {
        let n = b.chart.len();
        let others: Vec<usize> = (0..n).filter(|i| !support.contains(i)).collect();
        let line_caps = support
            .iter()
            .map(|&i| b.slot_cap(i, true).map(|c| c - 1))
            .collect();
        let other_caps = others.iter().map(|&i| b.slot_cap(i, false)).collect();
        Section {
            support,
            others,
            layers,
            line_caps,
            other_caps,
        }
    }
}

fn capacity(caps: &[Option<u32>]) -> Option<u64> {
    caps.iter()
        .try_fold(0u64, |a, c| c.map(|c| a + u64::from(c)))
}

fn fits(caps: &[Option<u32>], total: u32) -> bool {
    capacity(caps).is_none_or(|c| u64::from(total) <= c)
}

/// Compositions of `total` with per-part caps, ascending lexicographic order.
#[derive(Clone, Debug)]
struct AscComp {
    v: Vec<u32>,
    caps: Vec<Option<u32>>,
}

impl AscComp {
    fn first(total: u32, caps: &[Option<u32>]) -> Option<AscComp> {
        if !fits(caps, total) {
            return None;
        }
        let mut v = vec![0; caps.len()];
        if caps.is_empty() {
            return (total == 0).then(|| AscComp { v, caps: vec![] });
        }
        fill_right(&mut v, caps, total);
        Some(AscComp {
            v,
            caps: caps.to_vec(),
        })
    }
    fn advance(&mut self) -> bool {
        let m = self.v.len();
        if m < 2 {
            return false;
        }
        let mut suffix = self.v[m - 1];
        for i in (0..m - 1).rev() {
            if self.caps[i].is_none_or(|c| self.v[i] < c) && suffix >= 1 {
                self.v[i] += 1;
                fill_right(&mut self.v[i + 1..], &self.caps[i + 1..], suffix - 1);
                return true;
            }
            suffix += self.v[i];
        }
        false
    }
}

/// Lexicographically smallest arrangement: as much as possible at the right.
fn fill_right(v: &mut [u32], caps: &[Option<u32>], mut total: u32) {
    for (x, c) in v.iter_mut().zip(caps).rev() {
        let t = c.map_or(total, |c| total.min(c));
        *x = t;
        total -= t;
    }
    debug_assert_eq!(total, 0);
}

/// Lexicographically largest arrangement: as much as possible at the left.
fn fill_left(v: &mut [u32], caps: &[Option<u32>], mut total: u32) {
    for (x, c) in v.iter_mut().zip(caps) {
        let t = c.map_or(total, |c| total.min(c));
        *x = t;
        total -= t;
    }
    debug_assert_eq!(total, 0);
}

/// Compositions of `total` with per-part caps in DESCENDING lexicographic order (= ascending order of the signed
/// values -b).
#[derive(Clone, Debug)]
struct DescComp {
    v: Vec<u32>,
    caps: Vec<Option<u32>>,
}

impl DescComp {
    fn first(total: u32, caps: &[Option<u32>]) -> Option<DescComp> {
        if !fits(caps, total) {
            return None;
        }
        let mut v = vec![0; caps.len()];
        if caps.is_empty() {
            return (total == 0).then(|| DescComp { v, caps: vec![] });
        }
        fill_left(&mut v, caps, total);
        Some(DescComp {
            v,
            caps: caps.to_vec(),
        })
    }
    fn advance(&mut self) -> bool {
        let m = self.v.len();
        if m < 2 {
            return false;
        }
        let mut suffix = self.v[m - 1];
        for i in (0..m - 1).rev() {
            if self.v[i] > 0 && fits(&self.caps[i + 1..], suffix + 1) {
                self.v[i] -= 1;
                fill_left(&mut self.v[i + 1..], &self.caps[i + 1..], suffix + 1);
                return true;
            }
            suffix += self.v[i];
        }
        false
    }
}

/// Streaming enumeration of a box (interface 7.3). Constant memory apart from the explicit keys.
pub struct BoxKeys {
    sections: Vec<Section>,
    extra: Vec<Key>,
    sec: usize,
    layer: usize,
    r: u32,
    lines: Option<AscComp>,
    others: Option<DescComp>,
    extra_pos: usize,
    arity: usize,
}

impl BoxKeys {
    fn write(&self, out: &mut [i32]) {
        let s = &self.sections[self.sec];
        let l = self.lines.as_ref().expect("lines");
        let o = self.others.as_ref().expect("others");
        for (j, &slot) in s.support.iter().enumerate() {
            out[slot] = 1 + l.v[j] as i32;
        }
        for (j, &slot) in s.others.iter().enumerate() {
            out[slot] = -(o.v[j] as i32);
        }
    }

    /// Position the state on the first key at or after (sec, layer, r) with fresh compositions.
    fn settle(&mut self) -> bool {
        while self.sec < self.sections.len() {
            let s = &self.sections[self.sec];
            if self.layer >= s.layers.len() {
                self.sec += 1;
                self.layer = 0;
                self.r = 0;
                continue;
            }
            let [d, rmax] = s.layers[self.layer];
            if self.r > rmax {
                self.layer += 1;
                self.r = 0;
                continue;
            }
            match (
                AscComp::first(d, &s.line_caps),
                DescComp::first(self.r, &s.other_caps),
            ) {
                (Some(l), Some(o)) => {
                    self.lines = Some(l);
                    self.others = Some(o);
                    return true;
                }
                (None, _) => {
                    // no line composition at this D: skip the whole layer
                    self.layer += 1;
                    self.r = 0;
                }
                (Some(_), None) => self.r += 1,
            }
        }
        false
    }

    /// Write the next key into `out` (length = arity); false when the box is exhausted.
    pub fn next_into(&mut self, out: &mut [i32]) -> bool {
        assert_eq!(out.len(), self.arity, "BoxKeys::next_into: buffer length");
        if self.sec < self.sections.len() {
            if self.lines.is_none() {
                if self.settle() {
                    self.write(out);
                    return true;
                }
            } else {
                let advanced = self.others.as_mut().expect("others").advance() || {
                    let s = &self.sections[self.sec];
                    let ok = self.lines.as_mut().expect("lines").advance();
                    if ok {
                        self.others = DescComp::first(self.r, &s.other_caps);
                    }
                    ok
                };
                if advanced {
                    self.write(out);
                    return true;
                }
                self.r += 1;
                self.lines = None;
                self.others = None;
                if self.settle() {
                    self.write(out);
                    return true;
                }
            }
        }
        if self.extra_pos < self.extra.len() {
            out.copy_from_slice(&self.extra[self.extra_pos]);
            self.extra_pos += 1;
            return true;
        }
        false
    }
}

impl Iterator for BoxKeys {
    type Item = Key;
    fn next(&mut self) -> Option<Key> {
        let mut k = vec![0i32; self.arity];
        self.next_into(&mut k).then_some(k)
    }
}

// ------------------------------------------------------------------------------------------------ faces

/// A face (plan s.1): dotted lines (>= 2) x nonzero numerator slots (<= -1).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Face {
    pub dotted: Vec<usize>,
    pub numerators: Vec<usize>,
}

impl Face {
    /// The face of a chart-orthant key (every line >= 1, every numerator <= 0); None otherwise.
    pub fn of(chart: &[bool], key: &[i32]) -> Option<Face> {
        if key.len() != chart.len() {
            return None;
        }
        let mut f = Face {
            dotted: vec![],
            numerators: vec![],
        };
        for (i, (&l, &x)) in chart.iter().zip(key).enumerate() {
            if l {
                if x < 1 {
                    return None;
                }
                if x >= 2 {
                    f.dotted.push(i);
                }
            } else {
                if x > 0 {
                    return None;
                }
                if x <= -1 {
                    f.numerators.push(i);
                }
            }
        }
        Some(f)
    }
}

impl fmt::Display for Face {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let set = |v: &[usize]| {
            v.iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        write!(f, "{{{}}}", set(&self.dotted))?;
        if !self.numerators.is_empty() {
            write!(f, "x{{{}}}", set(&self.numerators))?;
        }
        Ok(())
    }
}

/// Keys grouped by face; keys outside the chart orthant in `other`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FaceGroups {
    pub faces: BTreeMap<Face, Vec<Key>>,
    pub other: Vec<Key>,
}

/// Group keys by face (input order kept inside a face).
pub fn group_by_face(chart: &[bool], keys: impl IntoIterator<Item = Key>) -> FaceGroups {
    let mut g = FaceGroups::default();
    for k in keys {
        match Face::of(chart, &k) {
            Some(f) => g.faces.entry(f).or_default().push(k),
            None => g.other.push(k),
        }
    }
    g
}

// ------------------------------------------------------------------------------------------------ BLAKE3

const B3_IV: [u32; 8] = [
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
];
const B3_PERM: [usize; 16] = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];
const CHUNK_START: u32 = 1;
const CHUNK_END: u32 = 2;
const PARENT: u32 = 4;
const ROOT: u32 = 8;
const CHUNK_LEN: usize = 1024;

#[allow(clippy::many_single_char_names)]
fn b3_g(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, x: u32, y: u32) {
    s[a] = s[a].wrapping_add(s[b]).wrapping_add(x);
    s[d] = (s[d] ^ s[a]).rotate_right(16);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_right(12);
    s[a] = s[a].wrapping_add(s[b]).wrapping_add(y);
    s[d] = (s[d] ^ s[a]).rotate_right(8);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_right(7);
}

fn b3_compress(cv: &[u32; 8], block: &[u32; 16], counter: u64, len: u32, flags: u32) -> [u32; 16] {
    let mut s = [
        cv[0],
        cv[1],
        cv[2],
        cv[3],
        cv[4],
        cv[5],
        cv[6],
        cv[7],
        B3_IV[0],
        B3_IV[1],
        B3_IV[2],
        B3_IV[3],
        counter as u32,
        (counter >> 32) as u32,
        len,
        flags,
    ];
    let mut m = *block;
    for round in 0..7 {
        b3_g(&mut s, 0, 4, 8, 12, m[0], m[1]);
        b3_g(&mut s, 1, 5, 9, 13, m[2], m[3]);
        b3_g(&mut s, 2, 6, 10, 14, m[4], m[5]);
        b3_g(&mut s, 3, 7, 11, 15, m[6], m[7]);
        b3_g(&mut s, 0, 5, 10, 15, m[8], m[9]);
        b3_g(&mut s, 1, 6, 11, 12, m[10], m[11]);
        b3_g(&mut s, 2, 7, 8, 13, m[12], m[13]);
        b3_g(&mut s, 3, 4, 9, 14, m[14], m[15]);
        if round < 6 {
            let mut p = [0u32; 16];
            for (i, &j) in B3_PERM.iter().enumerate() {
                p[i] = m[j];
            }
            m = p;
        }
    }
    for i in 0..8 {
        s[i] ^= s[i + 8];
        s[i + 8] ^= cv[i];
    }
    s
}

fn b3_words(bytes: &[u8]) -> [u32; 16] {
    let mut buf = [0u8; 64];
    buf[..bytes.len()].copy_from_slice(bytes);
    let mut w = [0u32; 16];
    for (i, c) in buf.as_chunks::<4>().0.iter().enumerate() {
        w[i] = u32::from_le_bytes(*c);
    }
    w
}

fn first8(s: &[u32; 16]) -> [u32; 8] {
    let mut o = [0u32; 8];
    o.copy_from_slice(&s[..8]);
    o
}

struct B3Output {
    cv: [u32; 8],
    block: [u32; 16],
    counter: u64,
    len: u32,
    flags: u32,
}

impl B3Output {
    fn chaining_value(&self) -> [u32; 8] {
        first8(&b3_compress(
            &self.cv,
            &self.block,
            self.counter,
            self.len,
            self.flags,
        ))
    }
    fn root_hash(&self) -> [u8; 32] {
        let s = b3_compress(&self.cv, &self.block, 0, self.len, self.flags | ROOT);
        let mut out = [0u8; 32];
        for (i, w) in s[..8].iter().enumerate() {
            out[4 * i..4 * i + 4].copy_from_slice(&w.to_le_bytes());
        }
        out
    }
}

/// Output of one chunk (at most 1,024 bytes) with the given counter.
fn b3_chunk(data: &[u8], counter: u64) -> B3Output {
    let mut cv = B3_IV;
    let mut blocks = data.chunks(64).peekable();
    let mut first = true;
    if data.is_empty() {
        return B3Output {
            cv,
            block: [0; 16],
            counter,
            len: 0,
            flags: CHUNK_START | CHUNK_END,
        };
    }
    loop {
        let b = blocks.next().expect("block");
        let start = if first { CHUNK_START } else { 0 };
        first = false;
        if blocks.peek().is_none() {
            return B3Output {
                cv,
                block: b3_words(b),
                counter,
                len: b.len() as u32,
                flags: start | CHUNK_END,
            };
        }
        cv = first8(&b3_compress(&cv, &b3_words(b), counter, 64, start));
    }
}

fn b3_parent(left: &[u32; 8], right: &[u32; 8]) -> B3Output {
    let mut block = [0u32; 16];
    block[..8].copy_from_slice(left);
    block[8..].copy_from_slice(right);
    B3Output {
        cv: B3_IV,
        block,
        counter: 0,
        len: 64,
        flags: PARENT,
    }
}

/// BLAKE3 hash (32 bytes, hex), std-only (the crate takes no hashing dependency); checked against `b3sum` vectors.
pub fn blake3_hex(data: &[u8]) -> String {
    let chunks: Vec<&[u8]> = if data.is_empty() {
        vec![data]
    } else {
        data.chunks(CHUNK_LEN).collect()
    };
    let n = chunks.len();
    let mut stack: Vec<[u32; 8]> = vec![];
    let mut out = None;
    for (i, c) in chunks.iter().enumerate() {
        let o = b3_chunk(c, i as u64);
        if i + 1 == n {
            out = Some(o);
            break;
        }
        let mut cv = o.chaining_value();
        let mut total = i as u64 + 1;
        while total & 1 == 0 {
            cv = b3_parent(&stack.pop().expect("stack"), &cv).chaining_value();
            total >>= 1;
        }
        stack.push(cv);
    }
    let mut out = out.expect("at least one chunk");
    while let Some(left) = stack.pop() {
        out = b3_parent(&left, &out.chaining_value());
    }
    out.root_hash().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn compositions_orders() {
        let mut c = AscComp::first(3, &[Some(2); 3]).unwrap();
        let mut seen = vec![c.v.clone()];
        while c.advance() {
            seen.push(c.v.clone());
        }
        let mut s = seen.clone();
        s.sort();
        assert_eq!(seen, s);
        assert_eq!(seen.len() as u128, capped_compositions(3, 3, Some(2)));
        let mut d = DescComp::first(3, &[None; 3]).unwrap();
        let mut seen = vec![d.v.clone()];
        while d.advance() {
            seen.push(d.v.clone());
        }
        let mut s = seen.clone();
        s.sort();
        s.reverse();
        assert_eq!(seen, s);
        assert_eq!(seen.len(), 10);
    }
}
