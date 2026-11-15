//! Record kinds of the engine's JSONL files, read from the PARSED top-level `type` member (engine-line7 reader fix).
//!
//! A record's kind is never decided by a line prefix or a key order: an externally written record may carry its
//! keys in any order (T211 rule 2894 starts `{"case":...` and ends `"type":"rule"}`). The scanner below walks the
//! top-level members of one JSON object line (string and nesting aware, no tree, no allocation), so a multi-GB rule
//! line costs one pass. Unreadable lines, records without a string `type` and kinds outside the file's list are
//! typed errors, never skipped.

/// Record kinds of a rule table (`F.table.jsonl`, table_dump.v1 / rustred.node.v1).
pub const TABLE_KINDS: &[&str] = &[
    "header",
    "zero_sectors",
    "chart",
    "symmetry_group",
    "rule",
    // sym-gen symmetry records (driver.rs sym_record; checked by rr_node replay)
    "sym",
    "sym_zero",
    "residual",
    "chart_end",
    "chart_error",
    "end",
    // external composite assemblies (Astra) found in accepted ledger tables (scan of 618 ledger table paths)
    "assembly_provenance",
    "composite_assembly_complete",
    "assembly_end",
    "composite_terminal_ledger",
];
/// Record kinds of a certificate (`F.cert.jsonl`).
pub const CERT_KINDS: &[&str] = &[
    "cert_header",
    "source_row",
    "basis_row",
    "cert_rule",
    "cert_end",
];

fn skip_ws(l: &[u8], mut i: usize) -> usize {
    while i < l.len() && l[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn end_of_string(l: &[u8], mut i: usize) -> Option<usize> {
    i += 1;
    while i < l.len() {
        match l[i] {
            b'\\' => i += 2,
            b'"' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

fn end_of_value(l: &[u8], i: usize) -> Option<usize> {
    match *l.get(i)? {
        b'"' => end_of_string(l, i).map(|e| e + 1),
        b'{' | b'[' => {
            let mut depth = 0i64;
            let mut j = i;
            while j < l.len() {
                match l[j] {
                    b'"' => j = end_of_string(l, j)?,
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(j + 1);
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            None
        }
        _ => {
            let mut j = i;
            while j < l.len() && !matches!(l[j], b',' | b'}' | b']') && !l[j].is_ascii_whitespace()
            {
                j += 1;
            }
            (j > i).then_some(j)
        }
    }
}

/// Visit the top-level members of a JSON object line: `f(name, raw value text)` (a string keeps its quotes);
/// `f` returns false to stop early. Err for a line that is not a well-formed object up to the point visited.
pub fn top_members<'a>(
    l: &'a [u8],
    mut f: impl FnMut(&'a [u8], &'a [u8]) -> bool,
) -> Result<(), &'static str> {
    let mut i = skip_ws(l, 0);
    if l.get(i) != Some(&b'{') {
        return Err("not a JSON object");
    }
    i = skip_ws(l, i + 1);
    if l.get(i) == Some(&b'}') {
        return if skip_ws(l, i + 1) == l.len() {
            Ok(())
        } else {
            Err("text after the object")
        };
    }
    loop {
        if l.get(i) != Some(&b'"') {
            return Err("member name expected");
        }
        let ke = end_of_string(l, i).ok_or("unterminated member name")?;
        let name = &l[i + 1..ke];
        i = skip_ws(l, ke + 1);
        if l.get(i) != Some(&b':') {
            return Err("':' expected");
        }
        i = skip_ws(l, i + 1);
        let ve = end_of_value(l, i).ok_or("unterminated value")?;
        if !f(name, &l[i..ve]) {
            return Ok(());
        }
        i = skip_ws(l, ve);
        match l.get(i) {
            Some(b',') => i = skip_ws(l, i + 1),
            Some(b'}') => {
                return if skip_ws(l, i + 1) == l.len() {
                    Ok(())
                } else {
                    Err("text after the object")
                };
            }
            _ => return Err("',' or '}' expected"),
        }
    }
}

/// The kind of a record from the START of its line (a bounded prefix): `{`, optional whitespace, `"type"`, optional
/// whitespace, `:`, optional whitespace, a plain string. None when `type` is not the first member (the caller decides).
pub fn first_member_kind(head: &str) -> Option<&str> {
    let b = head.as_bytes();
    let mut i = skip_ws(b, 0);
    if b.get(i) != Some(&b'{') {
        return None;
    }
    i = skip_ws(b, i + 1);
    let rest = head.get(i..)?.strip_prefix("\"type\"")?;
    let j = skip_ws(rest.as_bytes(), 0);
    let rest = rest.get(j..)?.strip_prefix(':')?;
    let j = skip_ws(rest.as_bytes(), 0);
    let rest = rest.get(j..)?.strip_prefix('"')?;
    let end = rest.find('"')?;
    let k = &rest[..end];
    k.bytes()
        .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        .then_some(k)
}

/// Byte range of the RAW value of the top-level member `name` (a string keeps its quotes), None if absent.
pub fn top_member_span(l: &str, name: &str) -> Option<(usize, usize)> {
    let b = l.as_bytes();
    let mut out = None;
    top_members(b, |k, v| {
        if k == name.as_bytes() {
            let start = v.as_ptr() as usize - b.as_ptr() as usize;
            out = Some((start, start + v.len()));
            false
        } else {
            true
        }
    })
    .ok()?;
    out
}

/// The top-level integer member `name` of a record line (None if absent or not an integer).
pub fn top_int(l: &str, name: &str) -> Option<i64> {
    let mut out = None;
    top_members(l.as_bytes(), |k, v| {
        if k == name.as_bytes() {
            out = std::str::from_utf8(v).ok().and_then(|s| s.parse().ok());
            false
        } else {
            true
        }
    })
    .ok()?;
    out
}

/// The kind of one record line: `Ok(None)` for a blank line; Err (typed: `UNREADABLE_RECORD`,
/// `UNKNOWN_RECORD_KIND`) for an unreadable line, a record without a plain string `type`, or a kind outside `allowed`.
pub fn record_kind<'a>(
    l: &'a str,
    file: &str,
    line_no: usize,
    allowed: &[&str],
) -> Result<Option<&'a str>, String> {
    if l.trim().is_empty() {
        return Ok(None);
    }
    let b = l.as_bytes();
    let mut kind: Option<&[u8]> = None;
    // the WHOLE line is scanned (a line truncated after its type member is unreadable, never classified)
    top_members(b, |k, v| {
        if k == b"type" && kind.is_none() {
            kind = Some(v);
        }
        true
    })
    .map_err(|e| format!("UNREADABLE_RECORD: {file} line {line_no}: {e}"))?;
    let k = match kind {
        Some(v)
            if v.len() >= 3
                && v[0] == b'"'
                && v[v.len() - 1] == b'"'
                && v[1..v.len() - 1]
                    .iter()
                    .all(|c| c.is_ascii_alphanumeric() || *c == b'_') =>
        {
            // ASCII checked above
            std::str::from_utf8(&v[1..v.len() - 1]).expect("ascii kind")
        }
        _ => {
            return Err(format!(
                "UNREADABLE_RECORD: {file} line {line_no}: no string member \"type\""
            ));
        }
    };
    if !allowed.contains(&k) {
        return Err(format!(
            "UNKNOWN_RECORD_KIND: {file} line {line_no}: type {k:?}"
        ));
    }
    Ok(Some(k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_come_from_the_parsed_type_member() {
        let sorted = r#"{"case":"x","chart":"110","index":2894,"rhs":[["I(1)","(n0-1)/(d-2)"]],"target":"I(n0)","type":"rule"}"#;
        assert_eq!(record_kind(sorted, "t", 1, TABLE_KINDS), Ok(Some("rule")));
        assert_eq!(top_int(sorted, "index"), Some(2894));
        let nested = r#"{"a":{"type":"rule","index":1},"s":"\"type\":\"end\"","type":"residual"}"#;
        assert_eq!(
            record_kind(nested, "t", 2, TABLE_KINDS),
            Ok(Some("residual"))
        );
        assert_eq!(top_int(nested, "index"), None);
        assert_eq!(record_kind("   ", "t", 3, TABLE_KINDS), Ok(None));
        for (bad, want) in [
            (r#"{"type":"mystery"}"#, "UNKNOWN_RECORD_KIND"),
            (r#"{"type":"rule","index":"#, "UNREADABLE_RECORD"),
            (r#"{"index":5}"#, "UNREADABLE_RECORD"),
            (r#"{"type":7}"#, "UNREADABLE_RECORD"),
            ("not json", "UNREADABLE_RECORD"),
        ] {
            let e = record_kind(bad, "t", 9, TABLE_KINDS).unwrap_err();
            assert!(e.starts_with(want), "{bad}: {e}");
        }
        assert_eq!(
            record_kind(r#"{"type":"cert_rule","rule":3}"#, "c", 1, CERT_KINDS),
            Ok(Some("cert_rule"))
        );
        // Python-written tables: whitespace after ':' and ','
        let spaced = r#"{"type": "rule", "chart": "110", "index": 7, "fixed": [1, null]}"#;
        assert_eq!(record_kind(spaced, "t", 6, TABLE_KINDS), Ok(Some("rule")));
        assert_eq!(top_int(spaced, "index"), Some(7));
        assert_eq!(first_member_kind(spaced), Some("rule"));
        assert_eq!(first_member_kind(r#"{"type":"end","rules":2"#), Some("end"));
        assert_eq!(first_member_kind(r#"{"case":"x","type":"rule"}"#), None);
        // sym-gen tables carry symmetry records; they are known kinds, not refused
        assert_eq!(
            record_kind(
                r#"{"type":"sym","chart":"110","index":3}"#,
                "t",
                4,
                TABLE_KINDS
            ),
            Ok(Some("sym"))
        );
        assert_eq!(
            record_kind(
                r#"{"type":"sym_zero","chart":"110","index":4}"#,
                "t",
                5,
                TABLE_KINDS
            ),
            Ok(Some("sym_zero"))
        );
    }

    /// The cover reader (registry::read_rule_shapes) finds a key-reordered rule record and refuses unknown kinds.
    #[test]
    fn cover_reader_reads_a_key_reordered_rule_and_refuses_unknown_kinds() {
        let dir = std::env::temp_dir().join(format!("rr_records_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("F.table.jsonl");
        let body = concat!(
            "{\"type\":\"header\",\"schema\":\"table_dump.v1\"}\n",
            "{\"type\":\"rule\",\"index\":0,\"fixed\":[1,null],\"equations\":[],\"exceptions\":[]}\n",
            "{\"case\":\"x\",\"equations\":[\"n0-n1\"],\"exceptions\":[[\"n0\"]],\"fixed\":[null,null],\"index\":1,\"type\":\"rule\"}\n",
            "{\"chart\":\"11\",\"key\":\"I(1,1)\",\"type\":\"residual\"}\n",
            "{\"rules\":2,\"type\":\"end\"}\n"
        );
        std::fs::write(&p, body).unwrap();
        let (shapes, _) =
            super::super::registry::read_rule_shapes(p.to_str().unwrap(), 10).unwrap();
        assert_eq!(shapes.len(), 2);
        assert_eq!(shapes[1].index, Some(1));
        assert_eq!(shapes[1].equations, vec!["n0-n1".to_string()]);
        std::fs::write(&p, format!("{body}{{\"type\":\"mystery\"}}\n")).unwrap();
        let e = super::super::registry::read_rule_shapes(p.to_str().unwrap(), 10).unwrap_err();
        assert!(e.starts_with("UNKNOWN_RECORD_KIND"), "{e}");
        assert_eq!(
            top_member_span(r#"{"a":"index","index":42}"#, "index"),
            Some((21, 23))
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
