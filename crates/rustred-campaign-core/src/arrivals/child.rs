//! The CHILD-SIDE CHECK (definition D6; interfaces/arrivals.md 5): does the child's certified set cover what its
//! parents send?
//!
//! Inputs: the child's import spec (its tables; the start set of the spec is NOT used), the child's CERTIFIED SET (a
//! statement: one line per closure key, the key first in an array line or under `key` in an object line, as an array or
//! an `I(..)` text), and one arrival-set file per parent. Per parent, every arriving key (support level) is
//!   (a) IN the certified set;
//!   (b) outside it, and the child's keys-only (Tier K) closure from ALL of that parent's arriving keys serves it (its row
//!       is served or a declared terminal);
//!   (c) outside it and NOT served (its row in that closure is open: uncovered, blocked, outside the chart, ..).
//! The closure is run per parent from all its arriving keys (comparable with an exhaustive walk from the arrival set);
//! its size, the keys it adds beyond the certified set and the arrivals, and EVERY open key it reaches (first 50 listed,
//! all counted by face) are reported: "the parent's arrivals close in the child's tables" iff no open key is reached.
//! Exact-level keys (proved non-zero total coefficient) are classified the same way and counted apart.

use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;
use std::path::Path;

use serde_json::{Value, json};

use super::format::ArrivalSet;
use super::{dots_rank, face_of, parse_key};
use crate::closure::KeySet;
use crate::import::check::check_streamed_from;
use crate::import::{CheckOptions, Evidence, ImportedSet, RowStatus};

/// Read a certified set (statement rows). Returns the keys and the file's sha256 (hex).
pub fn read_certified(path: &Path) -> Result<(BTreeSet<Vec<i32>>, String), String> {
    let f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut h = crate::closure::sha256::Sha256::new();
    let mut out = BTreeSet::new();
    for (i, l) in std::io::BufReader::with_capacity(1 << 20, f)
        .lines()
        .enumerate()
    {
        let l = l.map_err(|e| format!("{}: {e}", path.display()))?;
        h.update(l.as_bytes());
        h.update(b"\n");
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(&l)
            .map_err(|e| format!("{} line {}: {e}", path.display(), i + 1))?;
        let k = match &v {
            Value::Array(a) if !a.is_empty() && !a[0].is_number() => parse_key(&a[0]),
            Value::Object(_) => parse_key(&v["key"]),
            x => parse_key(x),
        }
        .map_err(|e| format!("{} line {}: {e}", path.display(), i + 1))?;
        out.insert(k);
    }
    Ok((out, h.finish_hex()))
}

/// Read a key list: a NumPy `.npy` file (2-D, dtype int8 / int16 / int32, C order: one key per row) or text lines (a JSON
/// array, an object with `key`, or an `I(..)` text). Returns the keys and the file's sha256 (hex).
pub fn read_keys(path: &Path) -> Result<(Vec<Vec<i32>>, String), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut h = crate::closure::sha256::Sha256::new();
    h.update(&bytes);
    let sha = h.finish_hex();
    if bytes.starts_with(b"\x93NUMPY") {
        let (major, hl_at) = (bytes[6], 8usize);
        let (hlen, data_at) = if major == 1 {
            (
                u16::from_le_bytes([bytes[hl_at], bytes[hl_at + 1]]) as usize,
                hl_at + 2,
            )
        } else {
            (
                u32::from_le_bytes([
                    bytes[hl_at],
                    bytes[hl_at + 1],
                    bytes[hl_at + 2],
                    bytes[hl_at + 3],
                ]) as usize,
                hl_at + 4,
            )
        };
        let header = std::str::from_utf8(&bytes[data_at..data_at + hlen])
            .map_err(|e| format!("{}: npy header: {e}", path.display()))?;
        let field = |name: &str| -> Option<&str> {
            let i = header.find(&format!("'{name}'"))?;
            Some(header[i + name.len() + 2..].trim_start_matches([':', ' ']))
        };
        let descr = field("descr").ok_or("npy: no descr")?;
        let width = if descr.starts_with("'|i1'") || descr.starts_with("'<i1'") {
            1
        } else if descr.starts_with("'<i2'") {
            2
        } else if descr.starts_with("'<i4'") {
            4
        } else {
            return Err(format!(
                "{}: npy dtype {descr:.8} not supported (int8/16/32 little endian)",
                path.display()
            ));
        };
        if field("fortran_order").is_some_and(|f| f.starts_with("True")) {
            return Err(format!("{}: npy in Fortran order", path.display()));
        }
        let shape = field("shape").ok_or("npy: no shape")?;
        let dims: Vec<usize> = shape
            .trim_start_matches('(')
            .split(')')
            .next()
            .unwrap_or("")
            .split(',')
            .filter_map(|x| x.trim().parse().ok())
            .collect();
        if dims.len() != 2 {
            return Err(format!("{}: npy shape {dims:?} is not 2-D", path.display()));
        }
        let (n, ar) = (dims[0], dims[1]);
        let body = &bytes[data_at + hlen..];
        if body.len() != n * ar * width {
            return Err(format!(
                "{}: npy body {} bytes for shape {n}x{ar}",
                path.display(),
                body.len()
            ));
        }
        let val = |i: usize| -> i32 {
            match width {
                1 => body[i] as i8 as i32,
                2 => i16::from_le_bytes([body[2 * i], body[2 * i + 1]]) as i32,
                _ => i32::from_le_bytes([
                    body[4 * i],
                    body[4 * i + 1],
                    body[4 * i + 2],
                    body[4 * i + 3],
                ]),
            }
        };
        let keys = (0..n)
            .map(|r| (0..ar).map(|c| val(r * ar + c)).collect())
            .collect();
        return Ok((keys, sha));
    }
    let text = std::str::from_utf8(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut keys = vec![];
    for (i, l) in text.lines().enumerate() {
        let l = l.trim();
        if l.is_empty() {
            continue;
        }
        let k = match serde_json::from_str::<Value>(l) {
            Ok(Value::Object(o)) => parse_key(o.get("key").unwrap_or(&Value::Null)),
            Ok(v) => parse_key(&v),
            Err(_) => super::parse_key_text(l),
        }
        .map_err(|e| format!("{} line {}: {e}", path.display(), i + 1))?;
        keys.push(k);
    }
    Ok((keys, sha))
}

/// A named CLOSED key set of the child (a set closed under the child's service set with no open key, e.g. a coordinator
/// statement's closure): its keys are served by definition; bound by the file's sha256.
#[derive(Clone, Debug)]
pub struct ClosedSet {
    pub name: String,
    pub path: String,
    pub sha256: String,
    pub keys: KeySet,
}

impl ClosedSet {
    pub fn read(name: &str, path: &Path, set: &ImportedSet) -> Result<ClosedSet, String> {
        let (keys, sha256) = read_keys(path)?;
        let keys = KeySet::from_keys(set.codec, keys.iter().map(|k| k.as_slice()))
            .map_err(|e| format!("{}: keys do not fit the child's codec: {e}", path.display()))?;
        Ok(ClosedSet {
            name: name.to_string(),
            path: path.display().to_string(),
            sha256,
            keys,
        })
    }
}

/// The classification of one parent's arrivals.
#[derive(Clone, Debug, Default)]
pub struct ParentCheck {
    pub parent: String,
    pub arrivals: u64,
    pub a: u64,
    pub b: u64,
    /// Arrivals outside the certified set but in a named closed set (name -> count).
    pub b_closed: BTreeMap<String, u64>,
    pub c: Vec<Vec<i32>>,
    pub exact_arrivals: u64,
    pub exact_a: u64,
    pub exact_b: u64,
    pub exact_c: u64,
    pub closure_keys: u64,
    pub closure_extra: u64,
    pub open_reached: Vec<(Vec<i32>, String)>,
    pub open_by_face: BTreeMap<String, u64>,
    pub terminals_reached: Vec<Vec<i32>>,
    pub seconds: f64,
}

impl ParentCheck {
    pub fn json(&self, lines: &[bool]) -> Value {
        let dr = |k: &Vec<i32>| dots_rank(k, lines);
        let mut c_by_face: BTreeMap<String, u64> = BTreeMap::new();
        for k in &self.c {
            *c_by_face.entry(face_of(k, lines)).or_default() += 1;
        }
        json!({"parent": self.parent, "arrivals": self.arrivals,
            "a_in_certified_set": self.a, "b_served_outside": self.b, "b_in_closed_sets": self.b_closed, "c_not_served": self.c.len(),
            "c_first50": self.c.iter().take(50).map(|k| json!({"key": k, "face": face_of(k, lines),
                "dots_rank": dr(k)})).collect::<Vec<_>>(),
            "c_by_face": c_by_face,
            "exact": {"arrivals": self.exact_arrivals, "a": self.exact_a, "b": self.exact_b, "c": self.exact_c},
            "closure_from_arrivals": {"keys": self.closure_keys, "extra_beyond_certified_and_arrivals": self.closure_extra,
                "open_reached": self.open_reached.len(),
                "open_first50": self.open_reached.iter().take(50).map(|(k, s)| json!({"key": k, "status": s,
                    "face": face_of(k, lines)})).collect::<Vec<_>>(),
                "open_by_face": self.open_by_face, "terminals_reached": self.terminals_reached,
                "closes": self.open_reached.is_empty()},
            "covers": self.b == 0 && self.b_closed.values().sum::<u64>() == 0 && self.c.is_empty(), "served": self.c.is_empty() && self.open_reached.is_empty(),
            "seconds": self.seconds})
    }
}

/// Run the check of one parent's arrival set against the child's set, certified keys and closed sets.
#[allow(clippy::too_many_arguments)]
pub fn check_parent(
    set: &ImportedSet,
    ev: &Evidence,
    certified: &BTreeSet<Vec<i32>>,
    closed: &[ClosedSet],
    name: &str,
    arr: &ArrivalSet,
    rows_path: &Path,
    threads: usize,
) -> Result<ParentCheck, String> {
    let t0 = std::time::Instant::now();
    let keys: Vec<&Vec<i32>> = arr.keys().collect();
    let in_closed = |k: &[i32]| {
        closed
            .iter()
            .find(|c| c.keys.contains(k))
            .map(|c| c.name.clone())
    };
    // the walk starts from the arrivals that neither the certified set nor a closed set serves
    let todo: Vec<&Vec<i32>> = keys
        .iter()
        .copied()
        .filter(|k| !certified.contains(*k) && in_closed(k).is_none())
        .collect();
    let start = KeySet::from_keys(set.codec, todo.iter().map(|k| k.as_slice()))
        .map_err(|e| format!("{name}: arrival keys do not fit the child's codec: {e}"))?;
    let r = check_streamed_from(
        set,
        ev,
        &CheckOptions {
            threads,
            tier: Some(crate::closure::Tier::K),
            max_rounds: None,
        },
        rows_path,
        &start,
    )
    .map_err(|e| format!("{name}: child closure: {e}"))?;
    let ks = r
        .key_set
        .as_ref()
        .ok_or_else(|| format!("{name}: the streamed check returned no key set"))?;
    let mut open: BTreeMap<Vec<i32>, String> = BTreeMap::new();
    let mut terminals = vec![];
    for row in &r.rows {
        match row.status {
            RowStatus::Open => {
                open.insert(
                    row.key.clone(),
                    row.open_status.clone().unwrap_or_else(|| "open".into()),
                );
            }
            RowStatus::Terminal => terminals.push(row.key.clone()),
            RowStatus::Served => {}
        }
    }
    let lines = set.frame.mask();
    let mut pc = ParentCheck {
        parent: name.to_string(),
        ..Default::default()
    };
    let arrset: BTreeSet<&Vec<i32>> = keys.iter().copied().collect();
    for l in &arr.lines {
        let k = &l.key;
        let exact = l.exact_senders.is_some_and(|n| n > 0);
        pc.arrivals += 1;
        pc.exact_arrivals += exact as u64;
        if certified.contains(k) {
            pc.a += 1;
            pc.exact_a += exact as u64;
        } else if let Some(c) = in_closed(k) {
            *pc.b_closed.entry(c).or_default() += 1;
            pc.exact_b += exact as u64;
        } else if open.contains_key(k) {
            pc.c.push(k.clone());
            pc.exact_c += exact as u64;
        } else if ks.contains(k) {
            pc.b += 1;
            pc.exact_b += exact as u64;
        } else {
            return Err(format!(
                "{name}: arrival {k:?} is not in its own closure (internal error)"
            ));
        }
    }
    pc.closure_keys = ks.len() as u64;
    pc.closure_extra = ks
        .iter()
        .filter(|k| !certified.contains(k) && !arrset.contains(k))
        .count() as u64;
    for (k, s) in open {
        // an open row of the walk that the certified set or a closed set serves is not open for the child
        if certified.contains(&k) || in_closed(&k).is_some() {
            continue;
        }
        *pc.open_by_face.entry(face_of(&k, lines)).or_default() += 1;
        pc.open_reached.push((k, s));
    }
    pc.terminals_reached = terminals;
    pc.seconds = t0.elapsed().as_secs_f64();
    Ok(pc)
}
