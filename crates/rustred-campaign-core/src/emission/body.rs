//! The BODY of a rule record and its identity hashes (emission contract section 1, implementer review 2026-11-18:
//! "the body hash must be stable under the things a shipped form changes legitimately").
//!
//! - `relation_sha256` = sha256 of `[target, rhs]` re-serialised compactly as Python's `json.dumps(v, separators=(',',
//!   ':'))` (the binding key `target_rhs_sha256` of the independent checker and of tools/final_cert);
//! - `domain_sha256`   = sha256 of `[fixed, equations, exceptions, domain_bounds]`, same serialisation, members in FILE
//!   order (the checker's `domain_sha256`);
//! - `body_sha256`     = sha256 of `chart \n case \n relation_sha256 \n domain_sha256` (case = the record's `case` text,
//!   `null` when absent).
//!
//! Invariant under: the `index` number (renumbering), whitespace, member order of the record, every member other than
//! chart / case / target / rhs / fixed / equations / exceptions / domain_bounds (origin, seeds, provenance, the
//! `derivation` member itself). Changed by: any change of a coefficient TEXT, of the target, of the domain members, of
//! chart or case. The coefficient text is the engine's canonical (Symbolica) printing, which the checker confirms
//! byte for byte at emission (`binding.byte_equal_terms`); a payload that re-encodes coefficients (shared form) must
//! decode to the identical text, or the record must be re-emitted.
use crate::closure::sha256::Sha256;
use std::ops::Range;

pub fn sha256_hex(b: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// One top-level member of a JSON object line.
#[derive(Clone, Debug)]
pub struct Member {
    pub key: String,
    /// byte offset of the opening quote of the key
    pub key_start: usize,
    /// byte range of the value
    pub value: Range<usize>,
}

/// The top-level members of a JSON object line: (key, byte range of the value), in file order.
pub fn top_level_members(line: &str) -> Result<Vec<(String, Range<usize>)>, String> {
    Ok(members(line)?
        .into_iter()
        .map(|m| (m.key, m.value))
        .collect())
}

/// The top-level members of a JSON object line with their key offsets, in file order.
pub fn members(line: &str) -> Result<Vec<Member>, String> {
    let b = line.as_bytes();
    let mut i = skip_ws(b, 0);
    if b.get(i) != Some(&b'{') {
        return Err("record is not a JSON object".into());
    }
    i += 1;
    let mut out = vec![];
    loop {
        i = skip_ws(b, i);
        match b.get(i) {
            Some(b'}') => return Ok(out),
            Some(b'"') => {}
            _ => return Err(format!("expected a member name at byte {i}")),
        }
        let key_start = i;
        let ke = string_end(b, i)?;
        let key = unescape(&line[i + 1..ke - 1]);
        i = skip_ws(b, ke);
        if b.get(i) != Some(&b':') {
            return Err(format!("expected ':' at byte {i}"));
        }
        i = skip_ws(b, i + 1);
        let ve = value_end(b, i)?;
        out.push(Member {
            key,
            key_start,
            value: i..ve,
        });
        i = skip_ws(b, ve);
        match b.get(i) {
            Some(b',') => i += 1,
            Some(b'}') => return Ok(out),
            _ => return Err(format!("expected ',' or '}}' at byte {i}")),
        }
    }
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | b'\r') {
        i += 1;
    }
    i
}

/// End (exclusive) of the string starting at `i` (b[i] == '"').
fn string_end(b: &[u8], i: usize) -> Result<usize, String> {
    let mut k = i + 1;
    while k < b.len() {
        match b[k] {
            b'\\' => k += 2,
            b'"' => return Ok(k + 1),
            _ => k += 1,
        }
    }
    Err("unterminated string".into())
}

/// End (exclusive) of the JSON value starting at `i`.
fn value_end(b: &[u8], i: usize) -> Result<usize, String> {
    match b.get(i) {
        Some(b'"') => string_end(b, i),
        Some(b'{') | Some(b'[') => {
            let mut depth = 0i64;
            let mut k = i;
            while k < b.len() {
                match b[k] {
                    b'"' => {
                        k = string_end(b, k)?;
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            return Ok(k + 1);
                        }
                    }
                    _ => {}
                }
                k += 1;
            }
            Err("unterminated container".into())
        }
        Some(_) => {
            let mut k = i;
            while k < b.len() && !matches!(b[k], b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r')
            {
                k += 1;
            }
            Ok(k)
        }
        None => Err("value expected".into()),
    }
}

fn unescape(raw: &str) -> String {
    serde_json::from_str::<String>(&format!("\"{raw}\"")).unwrap_or_else(|_| raw.to_string())
}

/// Python `json.dumps(s)` string form (ensure_ascii).
fn py_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{u:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Re-serialise the JSON value text `raw` compactly as Python's `json.dumps(v, separators=(',', ':'))` would, members
/// in their order, numbers as written.
pub fn py_compact(raw: &str) -> Result<String, String> {
    let b = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            b'"' => {
                let e = string_end(b, i)?;
                py_str(&unescape(&raw[i + 1..e - 1]), &mut out);
                i = e;
            }
            _ => {
                // ASCII structure characters and number / literal bytes are copied as they are
                let c = raw[i..].chars().next().ok_or("bad utf-8")?;
                out.push(c);
                i += c.len_utf8();
            }
        }
    }
    Ok(out)
}

/// The text of a top-level STRING member of a record line (None when absent or not a string): member order free.
pub fn string_member(line: &str, key: &str) -> Option<String> {
    let m = members(line).ok()?;
    let r = &m.iter().find(|x| x.key == key)?.value;
    let t = &line[r.clone()];
    t.starts_with('"').then(|| unescape(&t[1..t.len() - 1]))
}

/// The identity of a rule record's body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyIds {
    pub relation_sha256: String,
    pub domain_sha256: String,
    pub chart: String,
    pub case: String,
    pub body_sha256: String,
}

/// The identity hashes of a rule line (with or without a `derivation` member: it is never part of the body).
pub fn body_ids(line: &str) -> Result<BodyIds, String> {
    let m = top_level_members(line)?;
    let get = |k: &str| {
        m.iter()
            .find(|(n, _)| n == k)
            .map(|(_, r)| &line[r.clone()])
    };
    let list = |keys: &[&str]| -> Result<String, String> {
        let mut s = String::from("[");
        for (i, k) in keys.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&py_compact(get(k).unwrap_or("null"))?);
        }
        s.push(']');
        Ok(s)
    };
    let relation_sha256 = sha256_hex(list(&["target", "rhs"])?.as_bytes());
    let domain_sha256 =
        sha256_hex(list(&["fixed", "equations", "exceptions", "domain_bounds"])?.as_bytes());
    let text = |k: &str| -> String {
        match get(k) {
            Some(r) if r.starts_with('"') => unescape(&r[1..r.len() - 1]),
            Some(r) => r.to_string(),
            None => "null".to_string(),
        }
    };
    let chart = text("chart");
    let case = text("case");
    let body_sha256 =
        sha256_hex(format!("{chart}\n{case}\n{relation_sha256}\n{domain_sha256}").as_bytes());
    Ok(BodyIds {
        relation_sha256,
        domain_sha256,
        chart,
        case,
        body_sha256,
    })
}

/// The line without its top-level `derivation` member, and that member's raw JSON text (None when absent).
pub fn strip_derivation(line: &str) -> Result<(String, Option<String>), String> {
    let m = members(line)?;
    let Some(pos) = m.iter().position(|x| x.key == "derivation") else {
        return Ok((line.to_string(), None));
    };
    let raw = line[m[pos].value.clone()].to_string();
    let out = if pos > 0 {
        // from the end of the previous value (its comma included) to the end of this value
        format!(
            "{}{}",
            &line[..m[pos - 1].value.end],
            &line[m[pos].value.end..]
        )
    } else if pos + 1 < m.len() {
        // first member: up to the next member's key
        format!(
            "{}{}",
            &line[..m[pos].key_start],
            &line[m[pos + 1].key_start..]
        )
    } else {
        // the only member
        format!("{}{}", &line[..m[pos].key_start], &line[m[pos].value.end..])
    };
    Ok((out, Some(raw)))
}

/// `body` (no `derivation` member) with `derivation` appended as its LAST top-level member.
pub fn attach_derivation(body: &str, derivation: &serde_json::Value) -> Result<String, String> {
    let m = top_level_members(body)?;
    if m.iter().any(|(k, _)| k == "derivation") {
        return Err("the body already carries a derivation member".into());
    }
    let t = body.trim_end();
    let close = t.rfind('}').ok_or("record is not a JSON object")?;
    let sep = if m.is_empty() { "" } else { "," };
    Ok(format!(
        "{}{sep}\"derivation\":{}}}",
        &t[..close],
        serde_json::to_string(derivation).map_err(|e| e.to_string())?
    ))
}
