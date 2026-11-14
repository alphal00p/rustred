//! CANONICAL JSON of plan 3.3.0 (Decision D-37): UTF-8, object keys sorted by byte order, no whitespace, integers only
//! in identifying documents (floats refused unless allowed), strings escaped as serde_json does, arrays in their given
//! order, a final newline only in files. Same rule as `canonical_json` of `schemas/validate.py` (shared vectors in
//! `schemas/canon-vectors.json`).
//!
//! This is the DOCUMENT rule. The contract crate's `canon::canonical_line` is the RECORD rule of child-job 3.2.3
//! (volatile keys removed, tokens copied verbatim); the two agree on serde-written input (see the vectors file).

use serde_json::Value;

/// Canonical text of `v`. `allow_floats = false` refuses any non-integer number (identifying documents).
pub fn canonical_json(v: &Value, allow_floats: bool) -> Result<String, String> {
    let mut out = String::new();
    emit(v, allow_floats, &mut out)?;
    Ok(out)
}

/// Canonical text of a JSON document given as bytes (duplicate keys are refused by the strict parse of the caller;
/// serde_json keeps the last duplicate, so use [`has_duplicate_keys`] first when that matters).
pub fn canonical_bytes(text: &[u8], allow_floats: bool) -> Result<String, String> {
    let v: Value = serde_json::from_slice(text).map_err(|e| e.to_string())?;
    canonical_json(&v, allow_floats)
}

fn emit(v: &Value, allow_floats: bool, out: &mut String) -> Result<(), String> {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if !allow_floats && !(n.is_i64() || n.is_u64()) {
                return Err(format!("float {n} in a canonical (identifying) document"));
            }
            out.push_str(&n.to_string());
        }
        Value::String(s) => out.push_str(&serde_json::to_string(s).map_err(|e| e.to_string())?),
        Value::Array(a) => {
            out.push('[');
            for (i, e) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                emit(e, allow_floats, out)?;
            }
            out.push(']');
        }
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(k).map_err(|e| e.to_string())?);
                out.push(':');
                emit(&m[k], allow_floats, out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

/// True when some object of the JSON text repeats a key (plan 3.3: duplicate keys are refused).
pub fn has_duplicate_keys(text: &[u8]) -> Result<bool, String> {
    use serde::de::{DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
    struct Seed;
    struct V;
    impl<'de> DeserializeSeed<'de> for Seed {
        type Value = bool;
        fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<bool, D::Error> {
            d.deserialize_any(V)
        }
    }
    impl<'de> Visitor<'de> for V {
        type Value = bool;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("JSON")
        }
        fn visit_bool<E>(self, _: bool) -> Result<bool, E> {
            Ok(false)
        }
        fn visit_i64<E>(self, _: i64) -> Result<bool, E> {
            Ok(false)
        }
        fn visit_u64<E>(self, _: u64) -> Result<bool, E> {
            Ok(false)
        }
        fn visit_f64<E>(self, _: f64) -> Result<bool, E> {
            Ok(false)
        }
        fn visit_str<E>(self, _: &str) -> Result<bool, E> {
            Ok(false)
        }
        fn visit_unit<E>(self) -> Result<bool, E> {
            Ok(false)
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<bool, A::Error> {
            let mut dup = false;
            while let Some(d) = a.next_element_seed(Seed)? {
                dup |= d;
            }
            Ok(dup)
        }
        fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<bool, A::Error> {
            let mut seen = std::collections::BTreeSet::new();
            let mut dup = false;
            while let Some(k) = a.next_key::<String>()? {
                dup |= !seen.insert(k);
                dup |= a.next_value_seed(Seed)?;
            }
            Ok(dup)
        }
    }
    let mut de = serde_json::Deserializer::from_slice(text);
    let r = Seed.deserialize(&mut de).map_err(|e| e.to_string())?;
    de.end().map_err(|e| e.to_string())?;
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sorts_and_refuses_floats() {
        let v = json!({"b": [3, {"z": 1, "a": "é\n"}], "a": null, "B": true});
        assert_eq!(
            canonical_json(&v, false).unwrap(),
            r#"{"B":true,"a":null,"b":[3,{"a":"é\n","z":1}]}"#
        );
        assert!(canonical_json(&json!({"x": 1.5}), false).is_err());
        assert_eq!(
            canonical_json(&json!({"x": 1.5}), true).unwrap(),
            r#"{"x":1.5}"#
        );
        assert!(has_duplicate_keys(br#"{"a":1,"b":{"c":1,"c":2}}"#).unwrap());
        assert!(!has_duplicate_keys(br#"{"a":1,"b":{"c":1,"d":[{"c":2}]}}"#).unwrap());
    }
}
