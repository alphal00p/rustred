//! Volatile keys and the CANONICAL RECORD FORM (child-job.md 3.2.3, ruling R-c).
//!
//! Canonical form of one JSONL record (a JSON object):
//! 1. the top-level keys that are volatile for the record's `type` are removed ([`is_volatile`]);
//! 2. every object (at any depth) has its members sorted bytewise by the key as written; duplicate keys are an error;
//! 3. all insignificant whitespace is removed;
//! 4. every other token is copied VERBATIM: strings keep their escapes as written, numbers keep their text.
//!
//! Point 4 refines the document's "parse and re-serialise" (changed by package contract, 2026-11-13): a generic JSON
//! value model would round integers above 2^64 through f64 and normalise string escapes in a library-version
//! dependent way; copying tokens verbatim makes the form exact and stable, and still identical for records written by
//! the same binary. The canonical digest of a stream is the BLAKE3 over the canonical lines, each followed by "\n",
//! in file order.
//!
//! The parser is a small validating scanner over the line's bytes: memory is the line itself plus one member list
//! per open object level (no value tree), so 100+ MB lines are fine.

use std::fmt;
use std::io::{self, BufRead, Write};

/// Version of the volatile-key list (bumped with the contract).
pub const VOLATILE_KEYS_VERSION: u32 = 1;

/// Volatile top-level keys per record type of the table and cert streams. Every record type NOT listed here (verdict
/// records of descent / replay / proof / laporta, and any other record) follows the verdict rule: a key is volatile
/// iff it equals `seconds` or `utc` or ends in `_seconds`.
pub const VOLATILE_BY_TYPE: [(&str, &[&str]); 9] = [
    ("header", &["zero_seconds"]),
    ("rule", &["seconds_search", "lift_seconds"]),
    ("chart_end", &["seconds", "vmhwm_kb"]),
    ("chart_partial", &["seconds", "vmhwm_kb"]),
    ("end", &["seconds", "vmhwm_kb"]),
    ("partial", &["seconds", "vmhwm_kb"]),
    ("chart_error", &["seconds", "vmhwm_kb"]),
    ("cert_end", &["seconds", "vmhwm_kb"]),
    ("cert_partial", &["seconds", "vmhwm_kb"]),
];

/// Whether top-level `key` of a record of type `record_type` is volatile. Keys are compared as written (a key
/// spelled with escapes is never volatile: fail closed, it then counts in the digest).
pub fn is_volatile(record_type: Option<&str>, key: &str) -> bool {
    match record_type.and_then(|t| VOLATILE_BY_TYPE.iter().find(|(n, _)| *n == t)) {
        Some((_, keys)) => keys.contains(&key),
        None => key == "seconds" || key == "utc" || key.ends_with("_seconds"),
    }
}

/// Why a line has no canonical form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonError {
    Utf8 {
        offset: usize,
    },
    Syntax {
        offset: usize,
        what: &'static str,
    },
    /// The line is valid JSON but not an object.
    NotObject,
    DuplicateKey(String),
    TooDeep,
    /// The top-level `type` member is not a string.
    TypeNotString,
    /// A stream ends with bytes after its last "\n".
    TornLine {
        line: u64,
    },
    Io(String),
}

impl fmt::Display for CanonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CanonError::Utf8 { offset } => write!(f, "invalid UTF-8 at byte {offset}"),
            CanonError::Syntax { offset, what } => {
                write!(f, "JSON syntax at byte {offset}: {what}")
            }
            CanonError::NotObject => write!(f, "record is not a JSON object"),
            CanonError::DuplicateKey(k) => write!(f, "duplicate key {k}"),
            CanonError::TooDeep => write!(f, "nesting deeper than {MAX_DEPTH}"),
            CanonError::TypeNotString => write!(f, "top-level `type` is not a string"),
            CanonError::TornLine { line } => {
                write!(f, "torn last line (line {line} has no newline)")
            }
            CanonError::Io(m) => write!(f, "I/O: {m}"),
        }
    }
}
impl std::error::Error for CanonError {}

impl From<io::Error> for CanonError {
    fn from(e: io::Error) -> Self {
        CanonError::Io(e.to_string())
    }
}

/// Maximum nesting depth accepted.
pub const MAX_DEPTH: usize = 128;

type R<T> = Result<T, CanonError>;

struct Scan<'a> {
    b: &'a [u8],
    i: usize,
}

/// A member of an object: raw key bytes (between the quotes), key span with quotes, value span.
struct Member {
    key: (usize, usize),
    val: (usize, usize),
}

impl<'a> Scan<'a> {
    fn err(&self, what: &'static str) -> CanonError {
        CanonError::Syntax {
            offset: self.i,
            what,
        }
    }
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    fn expect(&mut self, c: u8, what: &'static str) -> R<()> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(self.err(what))
        }
    }

    /// A string; returns its span including the quotes.
    fn string(&mut self) -> R<(usize, usize)> {
        let s = self.i;
        self.expect(b'"', "expected a string")?;
        loop {
            let Some(c) = self.peek() else {
                return Err(self.err("unterminated string"));
            };
            match c {
                b'"' => {
                    self.i += 1;
                    return Ok((s, self.i));
                }
                b'\\' => {
                    self.i += 1;
                    match self.peek() {
                        Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => self.i += 1,
                        Some(b'u') => {
                            self.i += 1;
                            for _ in 0..4 {
                                match self.peek() {
                                    Some(h) if h.is_ascii_hexdigit() => self.i += 1,
                                    _ => return Err(self.err("bad \\u escape")),
                                }
                            }
                        }
                        _ => return Err(self.err("bad escape")),
                    }
                }
                0..=0x1f => return Err(self.err("control character in string")),
                _ => self.i += 1,
            }
        }
    }

    /// A number; returns (span, is_integer).
    fn number(&mut self) -> R<((usize, usize), bool)> {
        let s = self.i;
        let digits = |sc: &mut Scan| {
            let st = sc.i;
            while sc.peek().is_some_and(|c| c.is_ascii_digit()) {
                sc.i += 1;
            }
            sc.i > st
        };
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        match self.peek() {
            Some(b'0') => self.i += 1,
            Some(b'1'..=b'9') => {
                digits(self);
            }
            _ => return Err(self.err("bad number")),
        }
        let mut integer = true;
        if self.peek() == Some(b'.') {
            self.i += 1;
            integer = false;
            if !digits(self) {
                return Err(self.err("bad fraction"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            integer = false;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if !digits(self) {
                return Err(self.err("bad exponent"));
            }
        }
        Ok(((s, self.i), integer))
    }

    fn literal(&mut self, lit: &'static [u8]) -> R<(usize, usize)> {
        let s = self.i;
        if self.b[self.i..].starts_with(lit) {
            self.i += lit.len();
            Ok((s, self.i))
        } else {
            Err(self.err("bad literal"))
        }
    }

    /// Validate one value and return its span.
    fn skip_value(&mut self, depth: usize) -> R<(usize, usize)> {
        if depth > MAX_DEPTH {
            return Err(CanonError::TooDeep);
        }
        let s = self.i;
        match self.peek() {
            Some(b'{') => {
                self.i += 1;
                self.ws();
                if self.peek() == Some(b'}') {
                    self.i += 1;
                    return Ok((s, self.i));
                }
                loop {
                    self.ws();
                    self.string()?;
                    self.ws();
                    self.expect(b':', "expected ':'")?;
                    self.ws();
                    self.skip_value(depth + 1)?;
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok((s, self.i));
                        }
                        _ => return Err(self.err("expected ',' or '}'")),
                    }
                }
            }
            Some(b'[') => {
                self.i += 1;
                self.ws();
                if self.peek() == Some(b']') {
                    self.i += 1;
                    return Ok((s, self.i));
                }
                loop {
                    self.ws();
                    self.skip_value(depth + 1)?;
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok((s, self.i));
                        }
                        _ => return Err(self.err("expected ',' or ']'")),
                    }
                }
            }
            Some(b'"') => self.string(),
            Some(b't') => self.literal(b"true"),
            Some(b'f') => self.literal(b"false"),
            Some(b'n') => self.literal(b"null"),
            Some(b'-' | b'0'..=b'9') => self.number().map(|(sp, _)| sp),
            _ => Err(self.err("expected a value")),
        }
    }

    /// Members of the object starting at `self.i` (validated), sorted by raw key, duplicates rejected.
    fn members(&mut self, depth: usize) -> R<Vec<Member>> {
        if depth > MAX_DEPTH {
            return Err(CanonError::TooDeep);
        }
        self.expect(b'{', "expected '{'")?;
        let mut m = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(m);
        }
        loop {
            self.ws();
            let key = self.string()?;
            self.ws();
            self.expect(b':', "expected ':'")?;
            self.ws();
            let val = self.skip_value(depth + 1)?;
            m.push(Member { key, val });
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(self.err("expected ',' or '}'")),
            }
        }
        let b = self.b;
        let raw = |mm: &Member| &b[mm.key.0 + 1..mm.key.1 - 1];
        m.sort_by(|x, y| raw(x).cmp(raw(y)));
        for w in m.windows(2) {
            if raw(&w[0]) == raw(&w[1]) {
                return Err(CanonError::DuplicateKey(
                    String::from_utf8_lossy(raw(&w[0])).into_owned(),
                ));
            }
        }
        Ok(m)
    }

    /// Write the canonical form of the (already validated) value at `self.i`.
    fn write_value<W: Write + ?Sized>(&mut self, out: &mut W, depth: usize) -> R<()> {
        match self.peek() {
            Some(b'{') => {
                let m = self.members(depth)?;
                self.write_members(out, &m, depth)
            }
            Some(b'[') => {
                self.i += 1;
                out.write_all(b"[")?;
                self.ws();
                if self.peek() == Some(b']') {
                    self.i += 1;
                    out.write_all(b"]")?;
                    return Ok(());
                }
                let mut first = true;
                loop {
                    self.ws();
                    if !first {
                        out.write_all(b",")?;
                    }
                    first = false;
                    if depth + 1 > MAX_DEPTH {
                        return Err(CanonError::TooDeep);
                    }
                    self.write_value(out, depth + 1)?;
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            out.write_all(b"]")?;
                            return Ok(());
                        }
                        _ => return Err(self.err("expected ',' or ']'")),
                    }
                }
            }
            _ => {
                let (s, e) = self.skip_value(depth)?;
                out.write_all(&self.b[s..e])?;
                Ok(())
            }
        }
    }

    fn write_members<W: Write + ?Sized>(&self, out: &mut W, m: &[Member], depth: usize) -> R<()> {
        out.write_all(b"{")?;
        for (k, mm) in m.iter().enumerate() {
            if k > 0 {
                out.write_all(b",")?;
            }
            out.write_all(&self.b[mm.key.0..mm.key.1])?;
            out.write_all(b":")?;
            let mut sub = Scan {
                b: self.b,
                i: mm.val.0,
            };
            sub.write_value(out, depth + 1)?;
        }
        out.write_all(b"}")?;
        Ok(())
    }
}

fn strip_eol(line: &[u8]) -> &[u8] {
    let l = line.strip_suffix(b"\n").unwrap_or(line);
    l.strip_suffix(b"\r").unwrap_or(l)
}

/// Parse a record line into its sorted top-level members and its `type` (raw string content).
fn top<'a>(line: &'a [u8]) -> R<(Scan<'a>, Vec<Member>, Option<&'a str>)> {
    let line = strip_eol(line);
    if let Err(e) = std::str::from_utf8(line) {
        return Err(CanonError::Utf8 {
            offset: e.valid_up_to(),
        });
    }
    let mut sc = Scan { b: line, i: 0 };
    sc.ws();
    if sc.peek() != Some(b'{') {
        sc.skip_value(0)?;
        sc.ws();
        if sc.i != line.len() {
            return Err(sc.err("trailing characters"));
        }
        return Err(CanonError::NotObject);
    }
    let m = sc.members(0)?;
    sc.ws();
    if sc.i != line.len() {
        return Err(sc.err("trailing characters"));
    }
    let mut ty = None;
    for mm in &m {
        if &line[mm.key.0 + 1..mm.key.1 - 1] == b"type" {
            let v = &line[mm.val.0..mm.val.1];
            if v.first() != Some(&b'"') {
                return Err(CanonError::TypeNotString);
            }
            // valid UTF-8 checked above
            ty = Some(std::str::from_utf8(&v[1..v.len() - 1]).expect("utf8"));
        }
    }
    Ok((sc, m, ty))
}

fn key_str<'a>(b: &'a [u8], m: &Member) -> &'a str {
    std::str::from_utf8(&b[m.key.0 + 1..m.key.1 - 1]).expect("utf8")
}

/// Write the canonical form of one record line (without a trailing newline) into `out`.
pub fn write_canonical<W: Write + ?Sized>(line: &[u8], out: &mut W) -> Result<(), CanonError> {
    let (sc, m, ty) = top(line)?;
    let kept: Vec<Member> = m
        .into_iter()
        .filter(|mm| !is_volatile(ty, key_str(sc.b, mm)))
        .collect();
    sc.write_members(out, &kept, 0)
}

/// The canonical bytes of one record line (no trailing newline; a trailing "\n" or "\r\n" of the input is ignored).
pub fn canonical_line(line: &[u8]) -> Result<Vec<u8>, CanonError> {
    let mut v = Vec::with_capacity(line.len());
    write_canonical(line, &mut v)?;
    Ok(v)
}

/// BLAKE3 hex of the canonical line followed by "\n" (the per-record digest, event `rule_committed.canon_blake3`).
pub fn canonical_line_blake3(line: &[u8]) -> Result<String, CanonError> {
    let mut h = blake3::Hasher::new();
    write_canonical(line, &mut h)?;
    h.update(b"\n");
    Ok(h.finalize().to_hex().to_string())
}

/// The top-level `type` of a record line (`None` when absent).
pub fn line_type(line: &[u8]) -> Result<Option<String>, CanonError> {
    let (_, _, ty) = top(line)?;
    Ok(ty.map(str::to_string))
}

/// Contract lint C-3 (restricted by package contract to TOP-LEVEL keys, because the table header nests the resolved
/// `NodeOptions`, which has f64 fields): the non-volatile top-level keys whose value is a non-integer number.
pub fn top_level_float_keys(line: &[u8]) -> Result<Vec<String>, CanonError> {
    let (sc, m, ty) = top(line)?;
    let mut out = Vec::new();
    for mm in &m {
        let k = key_str(sc.b, mm);
        if is_volatile(ty, k) {
            continue;
        }
        let v = &sc.b[mm.val.0..mm.val.1];
        if matches!(v.first(), Some(b'-' | b'0'..=b'9'))
            && v.iter().any(|c| matches!(c, b'.' | b'e' | b'E'))
        {
            out.push(k.to_string());
        }
    }
    Ok(out)
}

/// Running canonical digest of a stream of records.
#[derive(Default, Clone)]
pub struct CanonicalDigest {
    h: blake3::Hasher,
    lines: u64,
}

impl CanonicalDigest {
    pub fn new() -> Self {
        Self::default()
    }
    /// Add one record line (with or without its "\n").
    pub fn push_line(&mut self, line: &[u8]) -> Result<(), CanonError> {
        // canonicalise into a scratch buffer first so that an invalid line leaves the digest untouched
        let c = canonical_line(line)?;
        self.h.update(&c);
        self.h.update(b"\n");
        self.lines += 1;
        Ok(())
    }
    pub fn lines(&self) -> u64 {
        self.lines
    }
    pub fn hex(&self) -> String {
        self.h.finalize().to_hex().to_string()
    }
}

/// Canonical digest of a whole JSONL stream; memory is bounded by the longest line. A last line without "\n" is a
/// [`CanonError::TornLine`]. Returns (digest, number of records).
pub fn canonical_digest_reader<Rd: BufRead>(mut r: Rd) -> Result<(String, u64), CanonError> {
    let mut d = CanonicalDigest::new();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let n = r.read_until(b'\n', &mut buf)?;
        if n == 0 {
            break;
        }
        if buf.last() != Some(&b'\n') {
            return Err(CanonError::TornLine {
                line: d.lines() + 1,
            });
        }
        d.push_line(&buf)?;
    }
    Ok((d.hex(), d.lines()))
}

/// [`canonical_digest_reader`] over a file.
pub fn canonical_digest_file(path: &std::path::Path) -> Result<(String, u64), CanonError> {
    let f = std::fs::File::open(path)?;
    canonical_digest_reader(io::BufReader::with_capacity(1 << 20, f))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(s: &str) -> String {
        String::from_utf8(canonical_line(s.as_bytes()).unwrap()).unwrap()
    }

    #[test]
    fn sorts_strips_and_keeps_tokens() {
        assert_eq!(
            c(
                r#"{"type":"rule","index":3,"seconds_search":0.71,"rhs":[["I(1)","-1/2"]],"lift_seconds":2.0,"a":{"z":1,"b":[1,{"y":2,"x":1}]}}"#
            ),
            r#"{"a":{"b":[1,{"x":1,"y":2}],"z":1},"index":3,"rhs":[["I(1)","-1/2"]],"type":"rule"}"#
        );
        // header strips only zero_seconds; a nested `seconds` is not volatile
        assert_eq!(
            c(r#"{ "type" : "header", "zero_seconds": 1.5, "x": {"seconds": 2} }"#),
            r#"{"type":"header","x":{"seconds":2}}"#
        );
        // verbatim tokens: big integers and escapes survive
        assert_eq!(
            c(r#"{"n":123456789012345678901234567890,"s":"é\/"}"#),
            r#"{"n":123456789012345678901234567890,"s":"é\/"}"#
        );
        // verdict rule for other types
        assert_eq!(
            c(r#"{"type":"verdict","seconds":1,"exact_seconds":2.5,"utc":"x","ok":true}"#),
            r#"{"ok":true,"type":"verdict"}"#
        );
        assert_eq!(
            c("{\"type\":\"end\",\"seconds\":1.25,\"vmhwm_kb\":77,\"rules\":2}\r\n"),
            r#"{"rules":2,"type":"end"}"#
        );
    }

    #[test]
    fn rejects() {
        for bad in [
            r#"[1]"#,
            r#"{"a":1,"a":2}"#,
            r#"{"a":01}"#,
            r#"{"a":1} x"#,
            r#"{"a":"x"#,
            r#"{"type":3}"#,
            r#"{"a":1.}"#,
            "{\"a\":\"\t\"}",
        ] {
            assert!(canonical_line(bad.as_bytes()).is_err(), "{bad}");
        }
        let deep = format!("{{\"a\":{}{}}}", "[".repeat(200), "]".repeat(200));
        assert_eq!(canonical_line(deep.as_bytes()), Err(CanonError::TooDeep));
        assert!(matches!(
            canonical_line(b"{\"a\":\"\xff\"}"),
            Err(CanonError::Utf8 { .. })
        ));
    }

    #[test]
    fn floats_lint() {
        assert_eq!(
            top_level_float_keys(
                br#"{"type":"rule","seconds_search":0.5,"w":1e3,"n":2,"o":{"f":0.5}}"#
            )
            .unwrap(),
            vec!["w"]
        );
    }

    #[test]
    fn torn_stream() {
        let r = canonical_digest_reader(&b"{\"a\":1}\n{\"a\":2}"[..]);
        assert_eq!(r, Err(CanonError::TornLine { line: 2 }));
        let (d, n) = canonical_digest_reader(&b"{\"a\":1}\n{\"b\":2, \"a\":1}\n"[..]).unwrap();
        assert_eq!(n, 2);
        assert_eq!(d, crate::blake3_hex(b"{\"a\":1}\n{\"a\":1,\"b\":2}\n"));
    }
}
