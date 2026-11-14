//! Raw scan of a `table_dump.v1` file: every line is classified (known kinds only), every rule record keeps its byte
//! offset, length, ordinal and `record_sha256` (plain sha256 hex of the raw line WITHOUT its newline: rr_proof's answer
//! echo, coord-ec2-instantiate.md s.2c). The domain fields are parsed by the core reader (`record::RecordReader`, one
//! line at a time); the few top-level fields the core reader does not keep (`domain_bounds`, `origin`, `origin_index`,
//! the span of `rhs`) are read here with a string-aware top-level scanner (no field of a nested object is ever taken).

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Cursor};
use std::path::Path;

use serde_json::Value;

use super::error::{ImportError, io_err};
use crate::closure::sha256::Sha256;
use crate::record::{ReadOptions, RecordReader, RhsMode, RuleRecord, TableHeader, TableLine};

/// Line kinds a table file may contain (assemble's list); anything else is a refusal.
pub const KNOWN_KINDS: &[&str] = &[
    "header",
    "rule",
    "zero_sectors",
    "chart",
    "chart_error",
    "symmetry_group",
    "residual",
    "chart_end",
    "end",
    "composite_terminal_ledger",
];

/// Inclusive selection bounds on the rule point (`domain_bounds` of the engine line, coord-bank-tables.md s.10).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainBounds {
    pub min: Vec<Option<i64>>,
    pub max: Vec<Option<i64>>,
}

/// One rule record of a scanned file.
#[derive(Clone, Debug)]
pub struct RawRecord {
    /// 0-based position among the RULE records of the file (file order).
    pub ordinal: usize,
    /// 1-based line number.
    pub line_no: usize,
    pub offset: u64,
    pub bytes: u64,
    /// sha256 hex of the raw line without its newline.
    pub sha256: String,
    pub rec: RuleRecord,
    pub domain_bounds: Option<DomainBounds>,
    pub origin: Option<String>,
    pub origin_index: Option<i64>,
    /// sha256 hex of the raw `rhs` value text (the coefficients as written), for supplied relations.
    pub rhs_sha256: Option<String>,
    /// Raw `kind` field of the record (e.g. "symmetry-one-term"), informational.
    pub record_kind: Option<String>,
}

/// A scanned table file.
#[derive(Clone, Debug)]
pub struct RawTable {
    pub header: TableHeader,
    pub charts: Vec<(String, Vec<String>)>,
    pub records: Vec<RawRecord>,
    /// Count of non-rule lines per kind.
    pub kinds: BTreeMap<String, usize>,
    /// sha256 hex of the whole file.
    pub file_sha256: String,
    pub file_bytes: u64,
}

/// Scan a table file. `want_rhs`: collect the rhs keys of the records (bodies). `keep(ordinal, label)` decides which
/// records are kept in memory (all are hashed and counted).
pub fn scan_table(
    source: &str,
    path: &Path,
    want_rhs: bool,
    keep: &dyn Fn(usize, Option<i64>) -> bool,
) -> Result<RawTable, ImportError> {
    let f = std::fs::File::open(path).map_err(|e| io_err(path, e))?;
    let mut rd = BufReader::with_capacity(1 << 20, f);
    let mut file_h = Sha256::new();
    let opts = ReadOptions {
        rhs: if want_rhs {
            RhsMode::KeysOnly
        } else {
            RhsMode::Skip
        },
    };
    let lerr = |line: usize, msg: String| ImportError::Line {
        source: source.to_string(),
        path: path.display().to_string(),
        line,
        msg,
    };
    let mut header: Option<TableHeader> = None;
    let mut charts = vec![];
    let mut records = vec![];
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut buf: Vec<u8> = Vec::with_capacity(1 << 16);
    let (mut offset, mut line_no, mut ordinal) = (0u64, 0usize, 0usize);
    loop {
        buf.clear();
        let n = rd
            .read_until(b'\n', &mut buf)
            .map_err(|e| io_err(path, e))?;
        if n == 0 {
            break;
        }
        file_h.update(&buf);
        line_no += 1;
        let start = offset;
        offset += n as u64;
        let body: &[u8] = if buf.last() == Some(&b'\n') {
            &buf[..buf.len() - 1]
        } else {
            &buf[..]
        };
        if body.iter().all(|c| c.is_ascii_whitespace()) {
            continue;
        }
        let parsed = RecordReader::new(Cursor::new(body), opts)
            .next()
            .ok_or_else(|| lerr(line_no, "empty line".into()))?
            .map_err(|e| lerr(line_no, e.to_string()))?;
        match parsed {
            TableLine::Header(h) => {
                if header.is_some() || !records.is_empty() || !charts.is_empty() {
                    return Err(lerr(
                        line_no,
                        "a second header or a header after records".into(),
                    ));
                }
                header = Some(*h);
            }
            TableLine::Chart { chart, guards } => {
                *kinds.entry("chart".into()).or_default() += 1;
                charts.push((chart, guards));
            }
            TableLine::Other { kind } => {
                if !KNOWN_KINDS.contains(&kind.as_str()) {
                    return Err(lerr(
                        line_no,
                        format!("line kind {kind:?} is not a table line"),
                    ));
                }
                *kinds.entry(kind).or_default() += 1;
            }
            TableLine::Rule(r) => {
                if header.is_none() {
                    return Err(lerr(line_no, "rule record before the header".into()));
                }
                let mut rec = *r;
                rec.line_no = line_no;
                let this = ordinal;
                ordinal += 1;
                let mut h = Sha256::new();
                h.update(body);
                let sha = h.finish_hex();
                if !keep(this, rec.index) {
                    continue;
                }
                let spans = top_level_spans(
                    body,
                    &["domain_bounds", "origin", "origin_index", "rhs", "kind"],
                )
                .map_err(|m| lerr(line_no, m))?;
                let val = |k: &str| -> Result<Option<Value>, ImportError> {
                    match spans.get(k) {
                        None => Ok(None),
                        Some(&(a, b)) => serde_json::from_slice(&body[a..b])
                            .map(Some)
                            .map_err(|e| lerr(line_no, format!("field {k}: {e}"))),
                    }
                };
                let domain_bounds = match val("domain_bounds")? {
                    None | Some(Value::Null) => None,
                    Some(v) => {
                        Some(parse_bounds(&v, rec.fixed.len()).map_err(|m| lerr(line_no, m))?)
                    }
                };
                let origin = val("origin")?.and_then(|v| match v {
                    Value::String(s) => Some(s),
                    Value::Null => None,
                    o => Some(o.to_string()),
                });
                let origin_index = val("origin_index")?.and_then(|v| v.as_i64());
                let record_kind = val("kind")?.and_then(|v| v.as_str().map(str::to_string));
                let rhs_sha256 = spans.get("rhs").map(|&(a, b)| {
                    let mut h = Sha256::new();
                    h.update(&body[a..b]);
                    h.finish_hex()
                });
                records.push(RawRecord {
                    ordinal: this,
                    line_no,
                    offset: start,
                    bytes: body.len() as u64,
                    sha256: sha,
                    rec,
                    domain_bounds,
                    origin,
                    origin_index,
                    rhs_sha256,
                    record_kind,
                });
            }
        }
    }
    let header = header.ok_or_else(|| lerr(0, "file without header".into()))?;
    Ok(RawTable {
        header,
        charts,
        records,
        kinds,
        file_sha256: file_h.finish_hex(),
        file_bytes: offset,
    })
}

fn parse_bounds(v: &Value, n: usize) -> Result<DomainBounds, String> {
    let side = |k: &str| -> Result<Vec<Option<i64>>, String> {
        match v.get(k) {
            None | Some(Value::Null) => Ok(vec![None; n]),
            Some(Value::Array(a)) => {
                if a.len() != n {
                    return Err(format!(
                        "domain_bounds.{k} of length {} for arity {n}",
                        a.len()
                    ));
                }
                a.iter()
                    .map(|x| match x {
                        Value::Null => Ok(None),
                        x => x
                            .as_i64()
                            .map(Some)
                            .ok_or_else(|| format!("domain_bounds.{k} entry {x}")),
                    })
                    .collect()
            }
            Some(o) => Err(format!("domain_bounds.{k} {o}")),
        }
    };
    if let Some(o) = v.as_object() {
        if let Some(k) = o.keys().find(|k| *k != "min" && *k != "max") {
            return Err(format!("domain_bounds field {k:?}"));
        }
    } else {
        return Err("domain_bounds is not an object".into());
    }
    Ok(DomainBounds {
        min: side("min")?,
        max: side("max")?,
    })
}

/// Byte spans of the requested TOP-LEVEL fields of one JSON object line (string-aware; nested objects skipped).
pub fn top_level_spans(
    s: &[u8],
    want: &[&str],
) -> Result<BTreeMap<String, (usize, usize)>, String> {
    let mut out = BTreeMap::new();
    let mut p = 0usize;
    let ws = |p: &mut usize| {
        while *p < s.len() && s[*p].is_ascii_whitespace() {
            *p += 1;
        }
    };
    let string_end = |mut p: usize| -> Result<usize, String> {
        // s[p] == '"'
        p += 1;
        while p < s.len() {
            match s[p] {
                b'\\' => p += 2,
                b'"' => return Ok(p + 1),
                _ => p += 1,
            }
        }
        Err("unterminated string".into())
    };
    let value_end = |p: usize| -> Result<usize, String> {
        match s.get(p) {
            None => Err("unexpected end".into()),
            Some(b'"') => string_end(p),
            Some(b'{') | Some(b'[') => {
                let mut depth = 0usize;
                let mut q = p;
                while q < s.len() {
                    match s[q] {
                        b'"' => {
                            q = string_end(q)?;
                            continue;
                        }
                        b'{' | b'[' => depth += 1,
                        b'}' | b']' => {
                            depth -= 1;
                            if depth == 0 {
                                return Ok(q + 1);
                            }
                        }
                        _ => {}
                    }
                    q += 1;
                }
                Err("unterminated value".into())
            }
            Some(_) => {
                let mut q = p;
                while q < s.len() && !matches!(s[q], b',' | b'}' | b']') {
                    q += 1;
                }
                let mut e = q;
                while e > p && s[e - 1].is_ascii_whitespace() {
                    e -= 1;
                }
                Ok(e)
            }
        }
    };
    ws(&mut p);
    if s.get(p) != Some(&b'{') {
        return Err("not an object".into());
    }
    p += 1;
    loop {
        ws(&mut p);
        match s.get(p) {
            Some(b'}') => return Ok(out),
            Some(b'"') => {}
            _ => return Err("expected a key".into()),
        }
        let ke = string_end(p)?;
        let key: String = serde_json::from_slice(&s[p..ke]).map_err(|e| e.to_string())?;
        p = ke;
        ws(&mut p);
        if s.get(p) != Some(&b':') {
            return Err("expected ':'".into());
        }
        p += 1;
        ws(&mut p);
        let ve = value_end(p)?;
        if want.contains(&key.as_str()) {
            out.insert(key, (p, ve));
        }
        p = ve;
        ws(&mut p);
        match s.get(p) {
            Some(b',') => p += 1,
            Some(b'}') => return Ok(out),
            _ => return Err("expected ',' or '}'".into()),
        }
    }
}

/// sha256 hex of a whole file.
pub fn file_sha256(path: &Path) -> Result<String, ImportError> {
    let f = std::fs::File::open(path).map_err(|e| io_err(path, e))?;
    let mut rd = BufReader::with_capacity(1 << 20, f);
    let mut h = Sha256::new();
    loop {
        let b = rd.fill_buf().map_err(|e| io_err(path, e))?;
        if b.is_empty() {
            break;
        }
        let n = b.len();
        h.update(b);
        rd.consume(n);
    }
    Ok(h.finish_hex())
}

/// Verify a file against an expected hex digest (if given); returns the computed digest.
pub fn verify_file(path: &Path, expected: Option<&str>) -> Result<String, ImportError> {
    let got = file_sha256(path)?;
    if let Some(e) = expected {
        let e = e.trim().trim_start_matches("sha256:");
        if !e.eq_ignore_ascii_case(&got) {
            return Err(ImportError::Digest {
                path: path.display().to_string(),
                expected: e.to_string(),
                got,
            });
        }
    }
    Ok(got)
}

/// Parse a SHA256SUMS file (`<hex>  <name>` lines) into name -> hex.
pub fn read_sha256sums(path: &Path) -> Result<BTreeMap<String, String>, ImportError> {
    let t = std::fs::read_to_string(path).map_err(|e| io_err(path, e))?;
    let mut m = BTreeMap::new();
    for (i, l) in t.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let (h, name) = l
            .split_once(char::is_whitespace)
            .ok_or_else(|| ImportError::Line {
                source: "sha256sums".into(),
                path: path.display().to_string(),
                line: i + 1,
                msg: "not '<hex>  <name>'".into(),
            })?;
        let name = name.trim_start().trim_start_matches('*').to_string();
        if h.len() != 64 || !h.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(ImportError::Line {
                source: "sha256sums".into(),
                path: path.display().to_string(),
                line: i + 1,
                msg: format!("digest {h:?}"),
            });
        }
        if m.insert(name.clone(), h.to_ascii_lowercase()).is_some() {
            return Err(ImportError::Line {
                source: "sha256sums".into(),
                path: path.display().to_string(),
                line: i + 1,
                msg: format!("{name} listed twice"),
            });
        }
    }
    Ok(m)
}

/// Parse an `I(a,b,..)` text into a key of the given arity.
pub fn parse_key_text(t: &str, arity: usize) -> Option<Vec<i32>> {
    let inner = t.trim().strip_prefix("I(")?.strip_suffix(')')?;
    let v: Option<Vec<i32>> = inner.split(',').map(|x| x.trim().parse().ok()).collect();
    v.filter(|v| v.len() == arity)
}

/// `I(a,b,..)` text of a key.
pub fn key_text(k: &[i32]) -> String {
    let v: Vec<String> = k.iter().map(|x| x.to_string()).collect();
    format!("I({})", v.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spans_are_top_level_only() {
        let l = br#"{"a":{"origin":"x"},"rhs":[["I(1)","n0/(\"q\")"]],"origin":"tb-1","origin_index":3}"#;
        let s = top_level_spans(l, &["origin", "origin_index", "rhs"]).unwrap();
        assert_eq!(&l[s["origin"].0..s["origin"].1], br#""tb-1""#);
        assert_eq!(&l[s["origin_index"].0..s["origin_index"].1], b"3");
        assert!(s.contains_key("rhs"));
    }
}
