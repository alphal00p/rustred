//! CERTIFIED ARRIVAL SETS per DAG edge parent -> child (lane arrivals; interfaces/arrivals.md; definitions D1-D7 in
//! WS/notes/implementer.arrivals.md). Symbolica-free part:
//!
//! - [`edge`]: the ROUTE of an edge (which parent sub-sector lands on the child's class, and how a parent key of that
//!   sector is written in the child's table frame: a line map plus exact linear forms), read from an edge file (DATA,
//!   pinned by sha256) and checked against the parent's / child's own table headers (frame binding);
//! - [`format`]: the arrival-set file `rustred.campaign.arrival-set.v1` (canonical JSON lines, bound to the parent's
//!   closure rows, its table set, the hand-down proof files and the edge file) and its reader;
//! - [`rows`]: the parent's closure rows (a check rows file) as (key, applied key, record) items;
//! - [`child`]: the CHILD-SIDE CHECK: every arriving key is (a) in the child's certified set, (b) outside it but served by
//!   the child's tables (a keys-only closure from them closes), or (c) not served.
//!
//! The exact decisions (structural proofs of hand-down terms, per-key evaluation) and the expansion of a parent key into
//! child keys use Symbolica and live in the algebra crate (`rustred-campaign-algebra/src/arrivals.rs`, bin
//! `rr_arrivals`). Nothing here names a node, a count, an arity or a frame (L8): all of that is data.

pub mod child;
pub mod edge;
pub mod format;
pub mod rows;

use serde_json::Value;

/// Schema of an arrival-set file (one header line, then one line per arriving child key, then a summary line).
pub const ARRIVAL_SET_SCHEMA: &str = "rustred.campaign.arrival-set.v1";
/// Schema of the child-side check report.
pub const CHILD_CHECK_SCHEMA: &str = "rustred.campaign.arrival-child-check.v1";
/// Version of the arrival semantics (bump on any change of a result).
pub const ARRIVAL_SEMANTICS: &str = "arrival-semantics.v1";

/// Canonical JSON text of a value: object keys sorted at every level, no whitespace (independent of serde_json's map
/// feature set).
pub fn canonical_json(v: &Value) -> String {
    let mut s = String::new();
    write_canonical(v, &mut s);
    s
}

fn write_canonical(v: &Value, out: &mut String) {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(k.clone()).to_string());
                out.push(':');
                write_canonical(&m[k], out);
            }
            out.push('}');
        }
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(x, out);
            }
            out.push(']');
        }
        x => out.push_str(&x.to_string()),
    }
}

/// Parse a key written as a JSON array of integers or as an `I(a,b,..)` text.
pub fn parse_key(v: &Value) -> Result<Vec<i32>, String> {
    match v {
        Value::Array(a) => a
            .iter()
            .map(|x| {
                x.as_i64()
                    .and_then(|i| i32::try_from(i).ok())
                    .ok_or_else(|| format!("key entry {x} is not an integer"))
            })
            .collect(),
        Value::String(s) => parse_key_text(s),
        x => Err(format!("{x} is not a key")),
    }
}

/// Parse `I(a,b,..)` (or a bare `a,b,..`).
pub fn parse_key_text(s: &str) -> Result<Vec<i32>, String> {
    let t = s.trim();
    let t = t
        .strip_prefix("I(")
        .and_then(|x| x.strip_suffix(')'))
        .unwrap_or(t);
    t.split(',')
        .map(|x| {
            x.trim()
                .parse::<i32>()
                .map_err(|e| format!("key text {s:?}: {e}"))
        })
        .collect()
}

/// The FACE of a top-sector key: (dotted line slots, non-zero numerator slots), as `{l..}/{n..}`.
pub fn face_of(key: &[i32], lines: &[bool]) -> String {
    let dots: Vec<String> = key
        .iter()
        .zip(lines)
        .enumerate()
        .filter(|(_, (v, l))| **l && **v > 1)
        .map(|(i, _)| i.to_string())
        .collect();
    let nums: Vec<String> = key
        .iter()
        .zip(lines)
        .enumerate()
        .filter(|(_, (v, l))| !**l && **v < 0)
        .map(|(i, _)| i.to_string())
        .collect();
    format!("{{{}}}/{{{}}}", dots.join(","), nums.join(","))
}

/// Dots D (sum over lines of power - 1) and rank r (sum of |numerator powers|) of a top-sector key.
pub fn dots_rank(key: &[i32], lines: &[bool]) -> (i64, i64) {
    let mut d = 0i64;
    let mut r = 0i64;
    for (v, l) in key.iter().zip(lines) {
        if *l {
            d += (*v as i64 - 1).max(0);
        } else {
            r += (*v as i64).abs();
        }
    }
    (d, r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_sorts_keys_and_keys_parse() {
        let v = json!({"b": 1, "a": {"y": [1, {"d": 2, "c": 3}], "x": null}});
        assert_eq!(
            canonical_json(&v),
            r#"{"a":{"x":null,"y":[1,{"c":3,"d":2}]},"b":1}"#
        );
        assert_eq!(parse_key(&json!("I(1,-2,0)")).unwrap(), vec![1, -2, 0]);
        assert_eq!(parse_key(&json!([3, 0])).unwrap(), vec![3, 0]);
        assert!(parse_key(&json!("I(1,x)")).is_err());
        assert_eq!(face_of(&[2, 1, -1, 0], &[true, true, false, false]), "{0}/{2}");
        assert_eq!(dots_rank(&[3, 1, -2, -1], &[true, true, false, false]), (2, 3));
    }
}
