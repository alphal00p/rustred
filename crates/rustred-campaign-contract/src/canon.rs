//! Volatile keys and the CANONICAL RECORD FORM (child-job.md 3.2.3, ruling R-c), version 2 (audit A C3, m8, m12;
//! coordinator ruling on `family_file`).
//!
//! Canonical form of one JSONL record (a JSON object):
//! 1. every string (keys and values) is DECODED to its Unicode value (escapes resolved, lone surrogates rejected) and
//!    re-encoded with ONE fixed escaping rule: `"` and `\` escaped, U+0008 / U+000C / U+000A / U+000D / U+0009 as
//!    `\b \f \n \r \t`, every other character below U+0020 as `\u00xx` (lower-case hex), everything else as raw
//!    UTF-8 (this is exactly serde_json's output, so serde-written strings are already canonical);
//! 2. every object (at any depth) has its members sorted bytewise by the DECODED key (its UTF-8 bytes); two members
//!    with the same decoded key are an error ([`CanonError::DuplicateKey`]);
//! 3. the top-level keys that are volatile for the record's `type` are removed ([`is_volatile_with`]);
//! 4. all insignificant whitespace is removed;
//! 5. numbers are DECODED to their value and re-encoded (plan 3.3.0, coordinator addendum 2): an integer token (no
//!    fraction, no exponent) is kept exactly whatever its size, `-0` becomes `0`; a token with a fraction or an
//!    exponent is its IEEE f64 value written as serde_json (ryu) writes it (`1e2` -> `100.0`, `1.0` -> `1.0`); a value
//!    outside the f64 range is [`CanonError::NumberOutOfRange`] (readers reject it). Two spellings of one value give
//!    one form; distinct forms are distinct values for every reader.
//!
//! This is ONE canonical-JSON rule for the whole campaign: the plan's 3.3.0 rule (`validate.py`, `schema::canon` of
//! rustred-campaign-core, floats allowed); the RECORD digest is that form with the volatile top-level keys removed.
//!
//! The canonical digest of a stream is the BLAKE3 over the canonical lines, each followed by "\n", in file order.
//! The scanner works over the line's bytes in one pass: memory is a small multiple of the line (each object level
//! assembles its sorted members in a buffer), no value tree; 100+ MB lines are fine.

use std::fmt;
use std::io::{self, BufRead, Write};
use std::ops::Range;

/// Version of the volatile-key list and of the canonical form (bumped with the contract). 2 = decoded strings and
/// keys, canonical numbers, verdict rule limited to verdict records, `family_file` rule (audit A, 2026-11-14).
pub const VOLATILE_KEYS_VERSION: u32 = 2;

/// Volatile top-level keys per record type of the table and cert streams.
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

/// Record types that follow the VERDICT rule (a key is volatile iff it equals `seconds` or `utc` or ends in
/// `_seconds`); records WITHOUT a `type` follow it too (checker result lines). Every other typed record that is not
/// in [`VOLATILE_BY_TYPE`] has NO volatile key (audit A m12: fail safe, a timing never hides a meaningful key).
pub const VERDICT_TYPES: [&str; 1] = ["verdict"];

/// The header key that names the family file (a path): volatile ONLY when the same header carries
/// [`FAMILY_DIGEST_KEY`] with a BLAKE3 hex value (the family CONTENT is then the identity, not the path).
pub const FAMILY_FILE_KEY: &str = "family_file";
/// The header key carrying the BLAKE3 of the family content (non-volatile; compared with the spec's
/// `inputs[role=family].blake3` by the classifier).
pub const FAMILY_DIGEST_KEY: &str = "family_blake3";

/// Whether top-level `key` (decoded) of a record of type `record_type` is volatile, for a record WITHOUT a family
/// content digest (see [`is_volatile_with`]).
pub fn is_volatile(record_type: Option<&str>, key: &str) -> bool {
    is_volatile_with(record_type, key, false)
}

/// Whether top-level `key` (decoded) is volatile; `header_has_family_digest` = the record is a header carrying
/// `family_blake3` as a BLAKE3 hex string, which makes its `family_file` volatile.
pub fn is_volatile_with(
    record_type: Option<&str>,
    key: &str,
    header_has_family_digest: bool,
) -> bool {
    if record_type == Some("header") && key == FAMILY_FILE_KEY {
        return header_has_family_digest;
    }
    match record_type {
        Some(t) => match VOLATILE_BY_TYPE.iter().find(|(n, _)| *n == t) {
            Some((_, keys)) => keys.contains(&key),
            None if VERDICT_TYPES.contains(&t) => verdict_volatile(key),
            None => false,
        },
        None => verdict_volatile(key),
    }
}

fn verdict_volatile(key: &str) -> bool {
    key == "seconds" || key == "utc" || key.ends_with("_seconds")
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
    /// Two members of one object have the same DECODED key.
    DuplicateKey(String),
    TooDeep,
    /// The top-level `type` member is not a string.
    TypeNotString,
    /// A `\uD800`-`\uDFFF` escape that is not part of a surrogate pair.
    LoneSurrogate {
        offset: usize,
    },
    /// A number whose value is outside the f64 range (readers reject it), in any position.
    NumberOutOfRange {
        offset: usize,
    },
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
            CanonError::DuplicateKey(k) => write!(f, "duplicate key {k:?}"),
            CanonError::TooDeep => write!(f, "nesting deeper than {MAX_DEPTH}"),
            CanonError::TypeNotString => write!(f, "top-level `type` is not a string"),
            CanonError::LoneSurrogate { offset } => {
                write!(f, "lone surrogate escape at byte {offset}")
            }
            CanonError::NumberOutOfRange { offset } => {
                write!(f, "number at byte {offset} is outside the f64 range")
            }
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

const NONE: usize = usize::MAX;

/// A piece of canonical output: bytes of the line, an owned re-encoded string, or punctuation.
#[derive(Clone, Copy)]
enum Piece {
    Line(usize, usize),
    Own(usize),
    Lit(&'static [u8]),
}

/// Canonical output as linked lists of pieces in one arena, so that moving a member's value up one object level and
/// reordering members is O(1) per member, never a copy of its bytes: the whole canonicalisation is linear in the
/// line size (audit B F11), whatever the nesting depth.
#[derive(Default)]
struct Arena {
    nodes: Vec<(Piece, usize)>,
    owned: Vec<Vec<u8>>,
}

#[derive(Clone, Copy)]
struct List {
    head: usize,
    tail: usize,
}

impl List {
    const EMPTY: List = List {
        head: NONE,
        tail: NONE,
    };
}

impl Arena {
    fn push(&mut self, l: &mut List, p: Piece) {
        let i = self.nodes.len();
        self.nodes.push((p, NONE));
        if l.head == NONE {
            l.head = i;
        } else {
            self.nodes[l.tail].1 = i;
        }
        l.tail = i;
    }
    fn cat(&mut self, l: &mut List, m: List) {
        if m.head == NONE {
            return;
        }
        if l.head == NONE {
            *l = m;
        } else {
            self.nodes[l.tail].1 = m.head;
            l.tail = m.tail;
        }
    }
    fn emit(&self, l: List, line: &[u8], out: &mut Vec<u8>) {
        let mut i = l.head;
        while i != NONE {
            let (p, next) = self.nodes[i];
            match p {
                Piece::Line(s, e) => out.extend_from_slice(&line[s..e]),
                Piece::Own(k) => out.extend_from_slice(&self.owned[k]),
                Piece::Lit(b) => out.extend_from_slice(b),
            }
            if i == l.tail {
                break;
            }
            i = next;
        }
    }
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
    ar: Arena,
}

/// A member of an object: decoded key, its encoded key, its canonical value.
struct Member {
    key: Vec<u8>,
    key_enc: List,
    val: List,
    /// Decoded value when it is a string and the caller asked for it.
    str_val: Option<String>,
}

fn encode_char(c: char, out: &mut Vec<u8>) {
    match c {
        '"' => out.extend_from_slice(b"\\\""),
        '\\' => out.extend_from_slice(b"\\\\"),
        '\u{8}' => out.extend_from_slice(b"\\b"),
        '\u{c}' => out.extend_from_slice(b"\\f"),
        '\n' => out.extend_from_slice(b"\\n"),
        '\r' => out.extend_from_slice(b"\\r"),
        '\t' => out.extend_from_slice(b"\\t"),
        c if (c as u32) < 0x20 => {
            const H: &[u8; 16] = b"0123456789abcdef";
            let v = c as u32;
            out.extend_from_slice(b"\\u00");
            out.push(H[(v >> 4) as usize]);
            out.push(H[(v & 15) as usize]);
        }
        c => {
            let mut buf = [0u8; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }
    }
}

impl<'a> P<'a> {
    fn new(b: &'a [u8]) -> P<'a> {
        P {
            b,
            i: 0,
            ar: Arena::default(),
        }
    }
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

    fn hex4(&mut self) -> R<u32> {
        let mut v = 0u32;
        for _ in 0..4 {
            let d = match self.peek() {
                Some(h @ b'0'..=b'9') => h - b'0',
                Some(h @ b'a'..=b'f') => h - b'a' + 10,
                Some(h @ b'A'..=b'F') => h - b'A' + 10,
                _ => return Err(self.err("bad \\u escape")),
            };
            v = v * 16 + d as u32;
            self.i += 1;
        }
        Ok(v)
    }

    /// A string: decoded (pushed to `dec` when given) and re-encoded with the one escaping rule. A string without
    /// any backslash is already canonical (raw control characters are rejected) and stays a slice of the line.
    fn string(&mut self, out: &mut List, mut dec: Option<&mut Vec<u8>>) -> R<()> {
        let start = self.i;
        self.expect(b'"', "expected a string")?;
        let mut owned: Option<Vec<u8>> = None;
        loop {
            let s = self.i;
            while self.i < self.b.len() {
                let c = self.b[self.i];
                if c == b'"' || c == b'\\' || c < 0x20 {
                    break;
                }
                self.i += 1;
            }
            if self.i > s {
                if let Some(o) = owned.as_mut() {
                    o.extend_from_slice(&self.b[s..self.i]);
                }
                if let Some(d) = dec.as_deref_mut() {
                    d.extend_from_slice(&self.b[s..self.i]);
                }
            }
            let Some(c) = self.peek() else {
                return Err(self.err("unterminated string"));
            };
            match c {
                b'"' => {
                    self.i += 1;
                    match owned {
                        None => self.ar.push(out, Piece::Line(start, self.i)),
                        Some(mut o) => {
                            o.push(b'"');
                            let k = self.ar.owned.len();
                            self.ar.owned.push(o);
                            self.ar.push(out, Piece::Own(k));
                        }
                    }
                    return Ok(());
                }
                b'\\' => {
                    if owned.is_none() {
                        owned = Some(self.b[start..self.i].to_vec());
                    }
                    let esc_at = self.i;
                    self.i += 1;
                    let ch = match self.peek() {
                        Some(b'"') => '"',
                        Some(b'\\') => '\\',
                        Some(b'/') => '/',
                        Some(b'b') => '\u{8}',
                        Some(b'f') => '\u{c}',
                        Some(b'n') => '\n',
                        Some(b'r') => '\r',
                        Some(b't') => '\t',
                        Some(b'u') => {
                            self.i += 1;
                            let u = self.hex4()?;
                            let lone = CanonError::LoneSurrogate { offset: esc_at };
                            let cp = if (0xD800..0xDC00).contains(&u) {
                                if !self.b[self.i..].starts_with(b"\\u") {
                                    return Err(lone);
                                }
                                self.i += 2;
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err(lone);
                                }
                                0x10000 + ((u - 0xD800) << 10) + (lo - 0xDC00)
                            } else if (0xDC00..0xE000).contains(&u) {
                                return Err(lone);
                            } else {
                                u
                            };
                            self.i -= 1; // the common `self.i += 1` below
                            char::from_u32(cp).ok_or(lone)?
                        }
                        _ => return Err(self.err("bad escape")),
                    };
                    self.i += 1;
                    encode_char(ch, owned.as_mut().expect("owned"));
                    if let Some(d) = dec.as_deref_mut() {
                        let mut buf = [0u8; 4];
                        d.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    }
                }
                _ => return Err(self.err("control character in string")),
            }
        }
    }

    /// A number, decoded and re-encoded (module docs, point 5). A number whose value is outside the f64 range (e.g.
    /// `1e999999`) is refused anywhere: readers (serde_json) reject it (audit B F9).
    fn number(&mut self, out: &mut List) -> R<()> {
        let s = self.i;
        let digits = |p: &mut P| {
            let st = p.i;
            while p.peek().is_some_and(|c| c.is_ascii_digit()) {
                p.i += 1;
            }
            p.i > st
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
        let mut plain = true;
        if self.peek() == Some(b'.') {
            self.i += 1;
            if !digits(self) {
                return Err(self.err("bad fraction"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            plain = false;
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if !digits(self) {
                return Err(self.err("bad exponent"));
            }
        }
        // the token is ASCII by construction
        let tok = std::str::from_utf8(&self.b[s..self.i]).unwrap_or("");
        if plain && !tok.contains('.') {
            // an integer: exact, any size; `-0` is `0`
            if tok == "-0" {
                let k = self.ar.owned.len();
                self.ar.owned.push(b"0".to_vec());
                self.ar.push(out, Piece::Own(k));
            } else {
                self.ar.push(out, Piece::Line(s, self.i));
            }
            return Ok(());
        }
        // a fraction or an exponent: its f64 value, written as serde_json (ryu) writes it
        let x: f64 = tok
            .parse()
            .map_err(|_| CanonError::NumberOutOfRange { offset: s })?;
        if !x.is_finite() {
            return Err(CanonError::NumberOutOfRange { offset: s });
        }
        let f =
            serde_json::to_string(&x).map_err(|_| CanonError::NumberOutOfRange { offset: s })?;
        let k = self.ar.owned.len();
        self.ar.owned.push(f.into_bytes());
        self.ar.push(out, Piece::Own(k));
        Ok(())
    }

    fn literal(&mut self, lit: &'static [u8], out: &mut List) -> R<()> {
        if self.b[self.i..].starts_with(lit) {
            self.ar.push(out, Piece::Line(self.i, self.i + lit.len()));
            self.i += lit.len();
            Ok(())
        } else {
            Err(self.err("bad literal"))
        }
    }

    /// One value, canonicalised into `out`.
    fn value(&mut self, out: &mut List, depth: usize) -> R<()> {
        if depth > MAX_DEPTH {
            return Err(CanonError::TooDeep);
        }
        match self.peek() {
            Some(b'{') => {
                let m = self.members(depth, &[])?;
                self.join_members(out, m);
                Ok(())
            }
            Some(b'[') => {
                self.i += 1;
                self.ar.push(out, Piece::Lit(b"["));
                self.ws();
                if self.peek() == Some(b']') {
                    self.i += 1;
                    self.ar.push(out, Piece::Lit(b"]"));
                    return Ok(());
                }
                let mut first = true;
                loop {
                    self.ws();
                    if !first {
                        self.ar.push(out, Piece::Lit(b","));
                    }
                    first = false;
                    self.value(out, depth + 1)?;
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            self.ar.push(out, Piece::Lit(b"]"));
                            return Ok(());
                        }
                        _ => return Err(self.err("expected ',' or ']'")),
                    }
                }
            }
            Some(b'"') => self.string(out, None),
            Some(b't') => self.literal(b"true", out),
            Some(b'f') => self.literal(b"false", out),
            Some(b'n') => self.literal(b"null", out),
            Some(b'-' | b'0'..=b'9') => self.number(out),
            _ => Err(self.err("expected a value")),
        }
    }

    /// The members of the object at `self.i`, canonicalised, sorted by decoded key, duplicates rejected. For keys in
    /// `want_str`, a string value is also returned decoded.
    fn members(&mut self, depth: usize, want_str: &[&str]) -> R<Vec<Member>> {
        if depth > MAX_DEPTH {
            return Err(CanonError::TooDeep);
        }
        self.expect(b'{', "expected '{'")?;
        let mut m: Vec<Member> = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(m);
        }
        loop {
            self.ws();
            let mut key = Vec::new();
            let mut key_enc = List::EMPTY;
            self.string(&mut key_enc, Some(&mut key))?;
            self.ws();
            self.expect(b':', "expected ':'")?;
            self.ws();
            let mut val = List::EMPTY;
            let mut str_val = None;
            if self.peek() == Some(b'"') && want_str.iter().any(|w| w.as_bytes() == key.as_slice())
            {
                let mut d = Vec::new();
                self.string(&mut val, Some(&mut d))?;
                str_val = Some(String::from_utf8(d).expect("decoded from UTF-8"));
            } else {
                self.value(&mut val, depth + 1)?;
            }
            m.push(Member {
                key,
                key_enc,
                val,
                str_val,
            });
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
        m.sort_by(|x, y| x.key.cmp(&y.key));
        for w in m.windows(2) {
            if w[0].key == w[1].key {
                return Err(CanonError::DuplicateKey(
                    String::from_utf8_lossy(&w[0].key).into_owned(),
                ));
            }
        }
        Ok(m)
    }

    /// `{` members `}` (already sorted) appended to `out`.
    fn join_members(&mut self, out: &mut List, m: Vec<Member>) {
        self.ar.push(out, Piece::Lit(b"{"));
        for (k, mm) in m.into_iter().enumerate() {
            if k > 0 {
                self.ar.push(out, Piece::Lit(b","));
            }
            self.ar.cat(out, mm.key_enc);
            self.ar.push(out, Piece::Lit(b":"));
            self.ar.cat(out, mm.val);
        }
        self.ar.push(out, Piece::Lit(b"}"));
    }
}

fn strip_eol(line: &[u8]) -> &[u8] {
    let l = line.strip_suffix(b"\n").unwrap_or(line);
    l.strip_suffix(b"\r").unwrap_or(l)
}

/// The canonical form of one record with what the verifier needs to read from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonRecord {
    /// The canonical line (no trailing newline).
    pub bytes: Vec<u8>,
    /// The decoded top-level `type`.
    pub record_type: Option<String>,
    /// Kept top-level members: decoded key and the span of its canonical value inside `bytes`.
    pub members: Vec<(String, Range<usize>)>,
}

impl CanonRecord {
    /// The canonical value bytes of a kept top-level member.
    pub fn member(&self, key: &str) -> Option<&[u8]> {
        self.members
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, r)| &self.bytes[r.clone()])
    }
}

/// Canonicalise one record line (a trailing "\n" or "\r\n" is ignored).
pub fn canonical_record(line: &[u8]) -> Result<CanonRecord, CanonError> {
    let line = strip_eol(line);
    if let Err(e) = std::str::from_utf8(line) {
        return Err(CanonError::Utf8 {
            offset: e.valid_up_to(),
        });
    }
    let mut p = P::new(line);
    p.ws();
    if p.peek() != Some(b'{') {
        let mut sink = List::EMPTY;
        p.value(&mut sink, 0)?;
        p.ws();
        if p.i != line.len() {
            return Err(p.err("trailing characters"));
        }
        return Err(CanonError::NotObject);
    }
    let m = p.members(0, &["type", FAMILY_DIGEST_KEY])?;
    p.ws();
    if p.i != line.len() {
        return Err(p.err("trailing characters"));
    }
    let mut ty = None;
    let mut family_digest = false;
    for mm in &m {
        if mm.key == b"type" {
            match &mm.str_val {
                Some(s) => ty = Some(s.clone()),
                None => return Err(CanonError::TypeNotString),
            }
        }
        if mm.key == FAMILY_DIGEST_KEY.as_bytes()
            && mm.str_val.as_deref().is_some_and(crate::is_blake3_hex)
        {
            family_digest = true;
        }
    }
    let family_digest = family_digest && ty.as_deref() == Some("header");
    let mut bytes = Vec::with_capacity(line.len());
    let mut members = Vec::new();
    bytes.push(b'{');
    let mut first = true;
    for mm in m {
        let key = String::from_utf8(mm.key).expect("decoded from UTF-8");
        if is_volatile_with(ty.as_deref(), &key, family_digest) {
            continue;
        }
        if !first {
            bytes.push(b',');
        }
        first = false;
        p.ar.emit(mm.key_enc, line, &mut bytes);
        bytes.push(b':');
        let start = bytes.len();
        p.ar.emit(mm.val, line, &mut bytes);
        members.push((key, start..bytes.len()));
    }
    bytes.push(b'}');
    Ok(CanonRecord {
        bytes,
        record_type: ty,
        members,
    })
}

/// Write the canonical form of one record line (without a trailing newline) into `out`.
pub fn write_canonical<W: Write + ?Sized>(line: &[u8], out: &mut W) -> Result<(), CanonError> {
    let c = canonical_record(line)?;
    out.write_all(&c.bytes)?;
    Ok(())
}

/// The canonical bytes of one record line (no trailing newline; a trailing "\n" or "\r\n" of the input is ignored).
pub fn canonical_line(line: &[u8]) -> Result<Vec<u8>, CanonError> {
    canonical_record(line).map(|c| c.bytes)
}

/// BLAKE3 hex of the canonical line followed by "\n" (the per-record digest, event `rule_committed.canon_blake3`).
pub fn canonical_line_blake3(line: &[u8]) -> Result<String, CanonError> {
    let mut h = blake3::Hasher::new();
    h.update(&canonical_line(line)?);
    h.update(b"\n");
    Ok(h.finalize().to_hex().to_string())
}

/// The decoded top-level `type` of a record line (`None` when absent).
pub fn line_type(line: &[u8]) -> Result<Option<String>, CanonError> {
    canonical_record(line).map(|c| c.record_type)
}

/// Decode a canonical JSON string token (with quotes) as written by this module.
pub fn decode_canonical_string(tok: &[u8]) -> Option<String> {
    let mut p = P::new(tok);
    let mut sink = List::EMPTY;
    let mut d = Vec::new();
    p.string(&mut sink, Some(&mut d)).ok()?;
    (p.i == tok.len()).then(|| String::from_utf8(d).ok())?
}

/// For a canonical JSON array (e.g. a record's `witness_loci`), the number of its elements that are strings equal to
/// `needle`; `None` when the bytes are not a JSON array.
pub fn count_string_elements(array: &[u8], needle: &str) -> Option<u64> {
    let mut p = P::new(array);
    p.expect(b'[', "expected '['").ok()?;
    let mut n = 0;
    p.ws();
    if p.peek() == Some(b']') {
        p.i += 1;
        return (p.i == array.len()).then_some(0);
    }
    loop {
        p.ws();
        let mut sink = List::EMPTY;
        if p.peek() == Some(b'"') {
            let mut d = Vec::new();
            p.string(&mut sink, Some(&mut d)).ok()?;
            if d == needle.as_bytes() {
                n += 1;
            }
        } else {
            p.value(&mut sink, 1).ok()?;
        }
        p.ws();
        match p.peek() {
            Some(b',') => p.i += 1,
            Some(b']') => {
                p.i += 1;
                return (p.i == array.len()).then_some(n);
            }
            _ => return None,
        }
    }
}

/// Contract lint C-3 (TOP-LEVEL keys only, because the table header nests the resolved `NodeOptions`, which has
/// f64 fields): the kept top-level keys whose value is a non-integer number.
pub fn top_level_float_keys(line: &[u8]) -> Result<Vec<String>, CanonError> {
    let c = canonical_record(line)?;
    Ok(c.members
        .iter()
        .filter(|(_, r)| {
            let v = &c.bytes[r.clone()];
            matches!(v.first(), Some(b'-' | b'0'..=b'9'))
                && v.iter().any(|b| matches!(b, b'.' | b'e'))
        })
        .map(|(k, _)| k.clone())
        .collect())
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
    /// Add one record line (with or without its "\n"). An invalid line leaves the digest untouched.
    pub fn push_line(&mut self, line: &[u8]) -> Result<(), CanonError> {
        let c = canonical_line(line)?;
        self.push_canonical(&c);
        Ok(())
    }
    /// Add one already canonical line (without "\n").
    pub fn push_canonical(&mut self, canonical: &[u8]) {
        self.h.update(canonical);
        self.h.update(b"\n");
        self.lines += 1;
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
        // big integers survive verbatim; escapes are decoded and re-encoded with the one rule
        assert_eq!(
            c(r#"{"n":123456789012345678901234567890,"s":"é\/\u0001"}"#),
            "{\"n\":123456789012345678901234567890,\"s\":\"\u{e9}/\\u0001\"}"
        );
        // verdict rule for verdict records and typeless records only
        assert_eq!(
            c(r#"{"type":"verdict","seconds":1,"exact_seconds":2.5,"utc":"x","ok":true}"#),
            r#"{"ok":true,"type":"verdict"}"#
        );
        assert_eq!(c(r#"{"seconds":1,"ok":true}"#), r#"{"ok":true}"#);
        // m12: another typed record keeps its *_seconds
        assert_eq!(
            c(r#"{"type":"residual","limit_seconds":5}"#),
            r#"{"limit_seconds":5,"type":"residual"}"#
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
            r#"{"a":1,"a":2}"#,
            r#"{"a":01}"#,
            r#"{"a":1} x"#,
            r#"{"a":"x"#,
            r#"{"type":3}"#,
            r#"{"a":1.}"#,
            r#"{"a":"\ud800"}"#,
            r#"{"a":"\udc00x"}"#,
            r#"{"type":"rule","seconds_search":1e999999}"#,
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
        // a pair is fine
        assert_eq!(c(r#"{"a":"😀"}"#), "{\"a\":\"\u{1f600}\"}");
    }

    #[test]
    fn numbers() {
        // plan 3.3.0: integers exact, -0 -> 0, fractions / exponents as serde_json writes the f64
        assert_eq!(
            c(r#"{"a":-0,"b":18446744073709551616}"#),
            r#"{"a":0,"b":18446744073709551616}"#
        );
        assert_eq!(
            c(r#"{"x":1e2,"y":1.0,"z":0.50,"w":1E-7}"#),
            r#"{"w":1e-7,"x":100.0,"y":1.0,"z":0.5}"#
        );
        assert_eq!(c(r#"{"x":1e2}"#), c(r#"{"x":100.0}"#));
        assert!(matches!(
            canonical_line(br#"{"x":1e999}"#),
            Err(CanonError::NumberOutOfRange { .. })
        ));
    }

    #[test]
    fn family_file_rule() {
        let d = "a".repeat(64);
        // without a family digest the path counts
        assert_eq!(
            c(r#"{"type":"header","family_file":"/x/T.sym"}"#),
            r#"{"family_file":"/x/T.sym","type":"header"}"#
        );
        // with it the path is volatile and the digest stays
        assert_eq!(
            c(&format!(
                r#"{{"type":"header","family_file":"/x/T.sym","family_blake3":"{d}"}}"#
            )),
            format!(r#"{{"family_blake3":"{d}","type":"header"}}"#)
        );
        // a malformed digest does not make the path volatile; neither does another record type
        assert!(c(r#"{"type":"header","family_file":"/x","family_blake3":"zz"}"#).contains("/x"));
        assert!(
            c(&format!(
                r#"{{"type":"rule","family_file":"/x","family_blake3":"{d}"}}"#
            ))
            .contains("/x")
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

    #[test]
    fn members_and_decode() {
        let r = canonical_record(br#"{"type":"rule","index":3,"target":"I(1)"}"#).unwrap();
        assert_eq!(r.member("index"), Some(&b"3"[..]));
        assert_eq!(r.member("target"), Some(&b"\"I(1)\""[..]));
        assert_eq!(
            decode_canonical_string(r.member("target").unwrap()).as_deref(),
            Some("I(1)")
        );
        assert_eq!(r.record_type.as_deref(), Some("rule"));
    }
}
