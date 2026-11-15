//! STREAMING scanner of ONE table line of any length (lane bigline): memory bounded by the per-record budget,
//! independent of the line's length.
//!
//! A rule line of a five-loop table can be several GB, almost all of it the top-level `rhs` member (the coefficient
//! texts). The in-memory reader ([`super::RecordReader`] / `parse_line`) needs the whole line; this scanner reads the
//! line from a [`BufRead`] once, byte-exact, and keeps only:
//! - the COMPACT line: the line with the value of its top-level `rhs` member replaced by `[]` (whatever that value
//!   is; [`StreamedRhs::array`] says whether it was an array), every other byte verbatim (exactly the index payload of a rule line, `import::index`); it is the input of
//!   the in-memory reader, which then parses the domain fields as for any line;
//! - of the `rhs` array: its span in the line, the KEY texts (keys-only mode), the term count, the length of the
//!   largest coefficient value, its sha256 on request, or every term handed to a visitor ([`stream_line`]);
//! - the line's length and whether it ended with a newline. Every byte of the line goes through `tap` (hashing).
//!
//! GRAMMAR AND ERRORS: the scanner applies the in-memory scanner's (`Scan`) grammar with the SAME messages and the same
//! byte positions (`(byte N)` counted from the start of the line, from the start of the `rhs` value for a key-grammar
//! error), so that a malformed line gives the same typed error whatever its length. As in the in-memory reader, the
//! line-level grammar (object, keys, duplicate keys, balanced values, trailing bytes) is checked first; an error of the
//! `rhs` key grammar (keys-only mode) is DEFERRED ([`StreamedRhs::keys_error`]) and reported by the caller after the
//! fields of the compact line, where `parse_line` reports it. Whatever cannot be kept within the budget (the compact
//! line, a key list, a visited term) is a typed [`RecordError::TooLarge`] naming the member and its size.

use std::io::BufRead;

use super::{RecordError, RhsMode};
use crate::closure::sha256::Sha256;

/// The `rhs` member of a streamed line (present iff the line has a top-level `rhs` member).
#[derive(Debug)]
pub struct StreamedRhs {
    /// The value is an array (anything else is left out all the same; a reader of rules refuses it as the in-memory
    /// reader does: `table_dump.v1 rule without an rhs array`, or the key grammar's `expected '['`).
    pub array: bool,
    /// Byte span of the value (as `Scan::skip_value` spans it) in the ORIGINAL line.
    pub span: (u64, u64),
    /// Where the value sits in the compact line (the two bytes `[]`).
    pub compact_span: (usize, usize),
    /// The key texts in order ([`RhsMode::KeysOnly`] and visiting; `None` in [`RhsMode::Skip`]).
    pub keys: Option<Vec<String>>,
    /// A violation of the `rhs` key grammar (keys-only mode), reported after the compact line's own fields.
    pub keys_error: Option<RecordError>,
    /// sha256 hex of the value text (on request).
    pub sha256: Option<String>,
    /// Terms read (keys-only mode and visiting).
    pub terms: usize,
    /// The longest coefficient value (the second element of a term), in bytes as written (quotes included).
    pub max_coefficient: u64,
}

/// What [`stream_line`] keeps of one line.
#[derive(Debug, Default)]
pub struct StreamedLine {
    /// Only whitespace before the end of the line.
    pub blank: bool,
    /// The line with the value of its top-level `rhs` replaced by `[]`; every other byte verbatim.
    pub compact: Vec<u8>,
    /// Length of the line without its newline.
    pub bytes: u64,
    /// The line ended with a newline (consumed); false at the end of the input.
    pub newline: bool,
    pub rhs: Option<StreamedRhs>,
}

impl StreamedLine {
    /// The compact line IS the line (no `rhs` value was replaced).
    pub fn exact(&self) -> bool {
        self.rhs.is_none()
    }
}

/// Options of [`stream_line`].
#[derive(Clone, Copy, Debug)]
pub struct StreamOptions {
    pub rhs: RhsMode,
    /// Compute [`StreamedRhs::sha256`].
    pub rhs_digest: bool,
    /// The per-record in-memory budget (bytes) for the compact line, the key texts and one visited term.
    pub budget: u64,
    /// Line number used in the errors (the in-memory reader of one line numbers it 1).
    pub line: usize,
}

/// Visitor of the `rhs` terms: (term position, the term's raw JSON text `[key, coefficient, ..]`).
pub type TermVisitor<'v> = dyn FnMut(usize, &[u8]) -> Result<(), RecordError> + 'v;

/// Read ONE line (up to and including its newline, or to the end of the input) from `rd`. `tap` sees every byte of the
/// line except the newline, in order. `visit`: hand every `rhs` term (raw text, at most the budget) to it, in order,
/// instead of collecting keys; a key-grammar violation is then an immediate error.
pub fn stream_line(
    rd: &mut dyn BufRead,
    opts: &StreamOptions,
    tap: &mut dyn FnMut(&[u8]),
    visit: Option<&mut TermVisitor>,
) -> Result<StreamedLine, RecordError> {
    let mut s = Stream {
        rd,
        avail: 0,
        end: false,
        newline: false,
        pos: 0,
        line: opts.line,
        sinks: Sinks {
            tap,
            budget: opts.budget,
            compact: Vec::new(),
            compact_on: true,
            compact_over: false,
            field: Vec::new(),
            field_on: false,
            field_over: false,
            rhs_hash: None,
        },
    };
    let out = s.line(opts, visit)?;
    Ok(out)
}

struct Sinks<'t> {
    tap: &'t mut dyn FnMut(&[u8]),
    budget: u64,
    compact: Vec<u8>,
    compact_on: bool,
    compact_over: bool,
    field: Vec<u8>,
    field_on: bool,
    field_over: bool,
    rhs_hash: Option<Sha256>,
}

impl Sinks<'_> {
    fn feed(&mut self, seg: &[u8]) {
        (self.tap)(seg);
        if self.compact_on && !self.compact_over {
            if (self.compact.len() + seg.len()) as u64 > self.budget {
                self.compact_over = true;
                self.compact = Vec::new();
            } else {
                self.compact.extend_from_slice(seg);
            }
        }
        if self.field_on && !self.field_over {
            if (self.field.len() + seg.len()) as u64 > self.budget {
                self.field_over = true;
                self.field = Vec::new();
            } else {
                self.field.extend_from_slice(seg);
            }
        }
        if let Some(h) = &mut self.rhs_hash {
            h.update(seg);
        }
    }
}

struct Stream<'a, 't> {
    rd: &'a mut dyn BufRead,
    /// Usable bytes at the front of the reader's buffer (up to, not including, a newline).
    avail: usize,
    /// The line ended (newline consumed, or end of input).
    end: bool,
    newline: bool,
    /// Bytes of the line consumed (as `Scan::pos`: may exceed the length by one after a final backslash).
    pos: u64,
    line: usize,
    sinks: Sinks<'t>,
}

/// The state of the `rhs` key grammar when it was left on a violation: the bracket depth reached.
struct Deferred {
    depth: usize,
}

enum Str {
    Done,
    Cont,
    /// A backslash was the last usable byte: the next byte is escaped.
    Escape,
}

impl Stream<'_, '_> {
    fn err(&self, msg: impl Into<String>) -> RecordError {
        RecordError::Format {
            line: self.line,
            msg: format!("{} (byte {})", msg.into(), self.pos),
        }
    }

    fn refill(&mut self) -> Result<(), RecordError> {
        if self.end || self.avail > 0 {
            return Ok(());
        }
        let b = self.rd.fill_buf()?;
        if b.is_empty() {
            self.end = true;
            return Ok(());
        }
        match b.iter().position(|&c| c == b'\n') {
            Some(0) => {
                self.rd.consume(1);
                self.end = true;
                self.newline = true;
            }
            Some(i) => self.avail = i,
            None => self.avail = b.len(),
        }
        Ok(())
    }

    /// Hand the usable bytes to `f`, which returns how many it consumes and a result; `None` at the end of the line.
    fn step<T>(&mut self, f: impl FnOnce(&[u8]) -> (usize, T)) -> Result<Option<T>, RecordError> {
        self.refill()?;
        if self.avail == 0 {
            return Ok(None);
        }
        let buf = self.rd.fill_buf()?;
        let usable = &buf[..self.avail];
        let (n, t) = f(usable);
        self.sinks.feed(&usable[..n]);
        self.rd.consume(n);
        self.avail -= n;
        self.pos += n as u64;
        Ok(Some(t))
    }

    fn peek(&mut self) -> Result<Option<u8>, RecordError> {
        self.step(|b| (0, b[0]))
    }

    fn bump(&mut self) -> Result<(), RecordError> {
        self.step(|_| (1, ()))?;
        Ok(())
    }

    fn ws(&mut self) -> Result<(), RecordError> {
        while let Some(all) = self.step(|b| {
            let n = b.iter().take_while(|c| c.is_ascii_whitespace()).count();
            (n, n == b.len())
        })? {
            if !all {
                break;
            }
        }
        Ok(())
    }

    /// A string; the current byte is its opening quote. Errors as `Scan::string_span`.
    fn string(&mut self) -> Result<(), RecordError> {
        self.bump()?;
        let mut escape = false;
        loop {
            if escape {
                // the escaped byte (or, at the end of the line, a virtual one: `Scan` advances by two)
                match self.step(|_| (1, ()))? {
                    Some(()) => escape = false,
                    None => {
                        self.pos += 1;
                        return Err(self.err("unterminated string"));
                    }
                }
                continue;
            }
            let r = self.step(|b| match b.iter().position(|&c| c == b'"' || c == b'\\') {
                Some(i) if b[i] == b'"' => (i + 1, Str::Done),
                Some(i) if i + 1 < b.len() => (i + 2, Str::Cont),
                Some(i) => (i + 1, Str::Escape),
                None => (b.len(), Str::Cont),
            })?;
            match r {
                None => return Err(self.err("unterminated string")),
                Some(Str::Done) => return Ok(()),
                Some(Str::Cont) => {}
                Some(Str::Escape) => escape = true,
            }
        }
    }

    /// Continue a bracketed value at `depth` (> 0) to its end. Errors as `Scan::skip_value`.
    fn balanced(&mut self, mut depth: usize) -> Result<(), RecordError> {
        loop {
            // scan to a quote or to the bracket that closes the value
            let r = self.step(|b| {
                for (i, &c) in b.iter().enumerate() {
                    match c {
                        b'"' => return (i, Some(false)),
                        b'{' | b'[' => depth += 1,
                        b'}' | b']' => {
                            depth -= 1;
                            if depth == 0 {
                                return (i + 1, Some(true));
                            }
                        }
                        _ => {}
                    }
                }
                (b.len(), None)
            })?;
            match r {
                None => return Err(self.err("unterminated object or array")),
                Some(Some(true)) => return Ok(()),
                Some(Some(false)) => self.string()?,
                Some(None) => {}
            }
        }
    }

    /// Any value (whitespace before it already skipped). Errors as `Scan::skip_value`. Returns the length of the value
    /// as `Scan` spans it (a scalar without its trailing whitespace).
    fn value(&mut self) -> Result<u64, RecordError> {
        let start = self.pos;
        match self.peek()? {
            None => Err(self.err("unexpected end of line")),
            Some(b'"') => {
                self.string()?;
                Ok(self.pos - start)
            }
            Some(b'{') | Some(b'[') => {
                self.bump()?;
                self.balanced(1)?;
                Ok(self.pos - start)
            }
            Some(_) => {
                let mut trailing_ws = 0u64;
                while let Some(more) = self.step(|b| {
                    let n = b
                        .iter()
                        .position(|c| matches!(c, b',' | b'}' | b']'))
                        .unwrap_or(b.len());
                    let seg = &b[..n];
                    let ws = seg
                        .iter()
                        .rev()
                        .take_while(|c| c.is_ascii_whitespace())
                        .count();
                    (n, (n == b.len(), ws, n))
                })? {
                    let (all, ws, n) = more;
                    trailing_ws = if ws == n {
                        trailing_ws + ws as u64
                    } else {
                        ws as u64
                    };
                    if !all {
                        break;
                    }
                }
                Ok(self.pos - start - trailing_ws)
            }
        }
    }

    /// The `rhs` key grammar's peek: the end of the line inside the `rhs` value is the LINE-level error `Scan` gives
    /// for an unterminated array.
    fn kpeek(&mut self) -> Result<u8, RecordError> {
        self.ws()?;
        match self.peek()? {
            Some(c) => Ok(c),
            None => Err(self.err("unterminated object or array")),
        }
    }

    /// The `rhs` array (current byte `[`) under `rec::rhs_keys`'s grammar. Keys go to `keys`; a violation returns
    /// `Ok(Err(..))` with the depth reached (the caller finishes the value with [`Stream::balanced`]).
    #[allow(clippy::too_many_arguments)]
    fn rhs_terms(
        &mut self,
        base: u64,
        keys: &mut Vec<String>,
        keys_bytes: &mut u64,
        terms: &mut usize,
        max_coef: &mut u64,
        mut visit: Option<&mut TermVisitor>,
    ) -> Result<Result<(), (Deferred, RecordError)>, RecordError> {
        let kerr = |s: &Self, msg: String| RecordError::Format {
            line: s.line,
            msg: format!("{msg} (byte {})", s.pos - base),
        };
        self.bump()?;
        if self.kpeek()? == b']' {
            self.bump()?;
            return Ok(Ok(()));
        }
        loop {
            if self.kpeek()? != b'[' {
                return Ok(Err((
                    Deferred { depth: 1 },
                    kerr(self, "expected '['".into()),
                )));
            }
            let term_start = self.pos;
            if visit.is_some() {
                self.sinks.field.clear();
                self.sinks.field_over = false;
                self.sinks.field_on = true;
            }
            self.bump()?;
            if self.kpeek()? != b'"' {
                return Ok(Err((
                    Deferred { depth: 2 },
                    kerr(self, "expected a string".into()),
                )));
            }
            let key_start = self.pos;
            let was_on = self.sinks.field_on;
            if !was_on {
                self.sinks.field.clear();
                self.sinks.field_over = false;
                self.sinks.field_on = true;
            }
            self.string()?;
            if !was_on {
                self.sinks.field_on = false;
                if self.sinks.field_over {
                    return Err(self.too_large("an rhs key", self.pos - key_start));
                }
                match serde_json::from_slice::<String>(&self.sinks.field) {
                    Ok(k) => {
                        *keys_bytes += k.len() as u64;
                        if *keys_bytes > self.sinks.budget {
                            return Err(self.too_large("the rhs keys", *keys_bytes));
                        }
                        keys.push(k);
                    }
                    Err(e) => {
                        return Ok(Err((Deferred { depth: 2 }, kerr(self, e.to_string()))));
                    }
                }
            }
            let mut element = 1usize;
            loop {
                if self.kpeek()? != b',' {
                    break;
                }
                self.bump()?;
                self.ws()?;
                let len = match self.peek()? {
                    // `Scan::skip_value` would read a scalar of length zero at a closing bracket, never the end
                    None => return Err(self.err("unterminated object or array")),
                    Some(_) => self.value()?,
                };
                if element == 1 {
                    *max_coef = (*max_coef).max(len);
                }
                element += 1;
            }
            if self.kpeek()? != b']' {
                return Ok(Err((
                    Deferred { depth: 2 },
                    kerr(self, "expected ']'".into()),
                )));
            }
            self.bump()?;
            if let Some(v) = visit.as_deref_mut() {
                self.sinks.field_on = false;
                if self.sinks.field_over {
                    return Err(
                        self.too_large(&format!("rhs term {}", *terms), self.pos - term_start)
                    );
                }
                let t = std::mem::take(&mut self.sinks.field);
                v(*terms, &t)?;
            }
            *terms += 1;
            match self.kpeek()? {
                b',' => self.bump()?,
                b']' => {
                    self.bump()?;
                    return Ok(Ok(()));
                }
                _ => {
                    return Ok(Err((
                        Deferred { depth: 1 },
                        kerr(self, "expected ',' or ']' in rhs".into()),
                    )));
                }
            }
        }
    }

    fn too_large(&self, what: &str, bytes: u64) -> RecordError {
        RecordError::TooLarge {
            line: self.line,
            what: what.to_string(),
            bytes,
            budget: self.sinks.budget,
        }
    }

    /// Decode a top-level key captured in the field sink.
    fn take_key(&mut self, start: u64) -> Result<String, RecordError> {
        self.sinks.field_on = false;
        if self.sinks.field_over {
            return Err(self.too_large("a member name", self.pos - start));
        }
        serde_json::from_slice::<String>(&self.sinks.field).map_err(|e| self.err(e.to_string()))
    }

    fn line(
        &mut self,
        opts: &StreamOptions,
        mut visit: Option<&mut TermVisitor>,
    ) -> Result<StreamedLine, RecordError> {
        let mut out = StreamedLine::default();
        self.ws()?;
        if self.peek()?.is_none() {
            out.blank = true;
            out.bytes = self.pos;
            out.newline = self.newline;
            return Ok(out);
        }
        // `Scan::object_fields`
        if self.peek()? != Some(b'{') {
            return Err(self.err(format!("expected {:?}", '{')));
        }
        self.bump()?;
        let mut names: Vec<String> = vec![];
        self.ws()?;
        let empty = self.peek()? == Some(b'}');
        if empty {
            self.bump()?;
        }
        if !empty {
            loop {
                self.ws()?;
                if self.peek()? != Some(b'"') {
                    return Err(self.err("expected a string"));
                }
                let kstart = self.pos;
                self.sinks.field.clear();
                self.sinks.field_over = false;
                self.sinks.field_on = true;
                self.string()?;
                let k = self.take_key(kstart)?;
                self.ws()?;
                if self.peek()? != Some(b':') {
                    return Err(self.err(format!("expected {:?}", ':')));
                }
                self.bump()?;
                self.ws()?;
                let vstart = self.pos;
                if k == "rhs" && out.rhs.is_none() {
                    let array = self.peek()? == Some(b'[');
                    // the rhs value is NOT kept: `[]` in the compact line
                    self.sinks.compact_on = false;
                    // a value whose span ends at its last byte (array, string, object) is hashed as it passes; a scalar
                    // (whose span `Scan` trims of trailing blanks) is held, bounded, and hashed over its span
                    let first = self.peek()?;
                    let scalar = !matches!(first, Some(b'[' | b'"' | b'{'));
                    if opts.rhs_digest && !scalar {
                        self.sinks.rhs_hash = Some(Sha256::new());
                    }
                    let mut keys = vec![];
                    let (mut kb, mut terms, mut maxc) = (0u64, 0usize, 0u64);
                    let mut keys_error = None;
                    let mut end = None;
                    match opts.rhs {
                        _ if !array => {
                            self.sinks.field.clear();
                            self.sinks.field_over = false;
                            self.sinks.field_on = scalar && opts.rhs_digest;
                            let len = self.value()?;
                            self.sinks.field_on = false;
                            if scalar && opts.rhs_digest {
                                if self.sinks.field_over {
                                    return Err(self.too_large("the scalar rhs value", len));
                                }
                                let mut h = Sha256::new();
                                h.update(&self.sinks.field[..len as usize]);
                                self.sinks.rhs_hash = Some(h);
                            }
                            end = Some(vstart + len);
                            // `rhs_keys`' first `expect(b'[')` (the value starts at a non-blank byte)
                            if opts.rhs == RhsMode::KeysOnly && visit.is_none() {
                                keys_error = Some(RecordError::Format {
                                    line: self.line,
                                    msg: format!("expected {:?} (byte 0)", '['),
                                });
                            }
                        }
                        RhsMode::Skip if visit.is_none() => {
                            self.bump()?;
                            self.balanced(1)?;
                        }
                        _ => {
                            if let Err((d, e)) = self.rhs_terms(
                                vstart,
                                &mut keys,
                                &mut kb,
                                &mut terms,
                                &mut maxc,
                                visit.as_deref_mut(),
                            )? {
                                if visit.is_some() {
                                    return Err(e);
                                }
                                self.sinks.field_on = false;
                                self.balanced(d.depth)?;
                                keys_error = Some(e);
                            }
                        }
                    }
                    let sha256 = self.sinks.rhs_hash.take().map(Sha256::finish_hex);
                    self.sinks.compact_on = true;
                    let at = self.sinks.compact.len();
                    if !self.sinks.compact_over {
                        self.sinks.compact.extend_from_slice(b"[]");
                    }
                    out.rhs = Some(StreamedRhs {
                        array,
                        span: (vstart, end.unwrap_or(self.pos)),
                        compact_span: (at, at + 2),
                        keys: (opts.rhs == RhsMode::KeysOnly && visit.is_none()).then_some(keys),
                        keys_error,
                        sha256,
                        terms,
                        max_coefficient: maxc,
                    });
                } else {
                    let over_before = self.sinks.compact_over;
                    self.value()?;
                    if self.sinks.compact_over && !over_before {
                        return Err(self.too_large(&format!("member {k:?}"), self.pos - vstart));
                    }
                }
                if names.contains(&k) {
                    return Err(self.err(format!("duplicate key {k:?}")));
                }
                names.push(k);
                self.ws()?;
                match self.peek()? {
                    Some(b',') => self.bump()?,
                    Some(b'}') => {
                        self.bump()?;
                        break;
                    }
                    _ => return Err(self.err("expected ',' or '}'")),
                }
            }
        }
        // `parse_line`: nothing but whitespace after the object
        self.ws()?;
        if self.peek()?.is_some() {
            return Err(self.err("trailing bytes after the object"));
        }
        if self.sinks.compact_over {
            return Err(self.too_large("the line without its rhs value", self.pos));
        }
        out.bytes = self.pos;
        out.newline = self.newline;
        out.compact = std::mem::take(&mut self.sinks.compact);
        Ok(out)
    }
}
