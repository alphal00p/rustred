//! Streaming readers of rule records.
//!
//! Formats:
//! - **rr_node `table_dump.v1`** (`F.table.jsonl`): one JSON object per line with a `type` field (`header`, `rule`,
//!   `chart`, `residual`, `zero_sectors`, `end`, ...). Rule fields used here: `chart`, `index`, `fixed`, `equations`,
//!   `exceptions`, `dispatch`, `numerical`, `target`, `rhs` (keys only, never coefficients), `witness_loci`,
//!   `recenter`. Written by `rustred-core/src/node/driver.rs` `write_rule`.
//! - **case-domain export** (`cases/*.jsonl`): `{"header":{...}}`, `{"other":{...}}`, `{"summary":{...}}` lines and rule
//!   lines `{i, index, chart, fixed, equations, exceptions, dispatch, numerical}` (no `type`).
//! - **T162 bank predicates** (`RULE-PREDICATES2440.json`): one JSON array; see [`BankPredicate`].
//!
//! Robustness (DESIGN-COMMENTS B5): a line may be 123 MB (a rule's `rhs`). The line reader scans the top-level object
//! byte by byte, records the byte span of each value and parses ONLY the fields it needs; `rhs` is either skipped or
//! scanned for its keys with the coefficient strings skipped unparsed ([`RhsMode`]). `witness_loci` may contain the
//! literal `"elided"` (the engine elides witness poles longer than 4000 characters); it is kept verbatim here and turned
//! into an explicit `Elided` verdict by the domain module.
//!
//! Strictness (audit A m4; the engine reads every line with `serde_json` into a `Value`): a duplicate top-level key
//! and any byte after the closing `}` are format errors (serde_json would keep the LAST duplicate; refusing is the
//! fail-closed choice). A rule must carry `exceptions` as an array (the engine's load fails with "rule without
//! exceptions" otherwise; also required of case-domain exports). A `table_dump.v1` rule (`"type":"rule"`) must also
//! carry `target` (a string) and `rhs` (an array), as `Table::load` requires. `chart` lines are returned as
//! [`TableLine::Chart`] so that the loader can refuse a chart other than the node mask, as the engine does.

use std::fmt;
use std::io::BufRead;

use serde::Deserialize;
use serde_json::Value;

pub mod stream;

/// Errors of the readers.
#[derive(Debug)]
pub enum RecordError {
    Io(std::io::Error),
    /// Malformed JSON or an unexpected field type, with the 1-based line number.
    Format {
        line: usize,
        msg: String,
    },
    /// A part of a record that must be held in memory (the line without its `rhs` value, the key texts, one visited
    /// term) is larger than the per-record in-memory budget (lane bigline; [`stream::stream_line`]).
    TooLarge {
        line: usize,
        what: String,
        bytes: u64,
        budget: u64,
    },
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecordError::Io(e) => write!(f, "io: {e}"),
            RecordError::Format { line, msg } => write!(f, "line {line}: {msg}"),
            RecordError::TooLarge {
                line,
                what,
                bytes,
                budget,
            } => write!(
                f,
                "line {line}: record too large for in-memory evaluation: {what} of {bytes} bytes exceeds the per-record budget of {budget} bytes"
            ),
        }
    }
}
impl std::error::Error for RecordError {}
impl From<std::io::Error> for RecordError {
    fn from(e: std::io::Error) -> Self {
        RecordError::Io(e)
    }
}

/// What to do with a rule's `rhs` field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RhsMode {
    /// Skip it entirely (domain-only reads).
    #[default]
    Skip,
    /// Collect the RHS key strings (`I(...)` patterns); coefficient strings are skipped unparsed.
    KeysOnly,
}

/// Options of the line reader.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReadOptions {
    pub rhs: RhsMode,
}

/// Per-record memory limits of the table readers (lane bigline; spec `reader`, interfaces/import.md 19).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadLimits {
    /// The per-record in-memory budget (bytes): the most of ONE record a reader holds at once (a whole line read in
    /// memory, the compact line of a streamed line, the key texts of a record, one coefficient term).
    pub record_budget: u64,
    /// Lines up to this length (bytes, without the newline) are read whole (the in-memory path); longer lines are
    /// streamed ([`stream::stream_line`]). At most `record_budget`.
    pub whole_line_max: u64,
}

impl ReadLimits {
    /// Budget `b`, lines up to `b` read whole.
    pub fn new(record_budget: u64) -> Self {
        ReadLimits {
            record_budget,
            whole_line_max: record_budget,
        }
    }
    /// No limit (every line read whole, nothing refused for its size): the readers as they were before lane bigline.
    pub fn unlimited() -> Self {
        ReadLimits::new(u64::MAX)
    }
}

/// Read from `rd` into `buf` up to and including the next newline, but at most `limit` bytes. Returns (bytes read,
/// complete): complete = the newline was read or the input ended (0 bytes read = end of input).
pub fn read_until_limited(
    rd: &mut dyn BufRead,
    buf: &mut Vec<u8>,
    limit: u64,
) -> std::io::Result<(usize, bool)> {
    let mut n = 0usize;
    loop {
        let room = limit.saturating_sub(n as u64);
        if room == 0 {
            return Ok((n, false));
        }
        let b = rd.fill_buf()?;
        if b.is_empty() {
            return Ok((n, true));
        }
        let cap = b.len().min(usize::try_from(room).unwrap_or(usize::MAX));
        match b[..cap].iter().position(|&c| c == b'\n') {
            Some(i) => {
                buf.extend_from_slice(&b[..=i]);
                rd.consume(i + 1);
                return Ok((n + i + 1, true));
            }
            None => {
                buf.extend_from_slice(&b[..cap]);
                rd.consume(cap);
                n += cap;
            }
        }
    }
}

/// Parsed table header (both formats). `raw` keeps the whole header object.
#[derive(Clone, Debug)]
pub struct TableHeader {
    pub schema: Option<String>,
    pub family: Option<String>,
    /// `denominators` = arity.
    pub denominators: Option<usize>,
    pub mask: Option<String>,
    /// Verbatim, informational (non-strings rendered as JSON). The frame reads `raw.power_shifts` itself
    /// ([`crate::key::NodeFrame::from_header`]).
    pub power_shifts: Vec<String>,
    pub charts: Vec<String>,
    pub order_name: Option<String>,
    /// `node_order.coordinate_priority` (what the engine uses), else `order.coordinate_priority`. Informational.
    pub coordinate_priority: Option<Vec<usize>>,
    /// Per-slot flags of `order.two_sided`, read as the engine's `table_import` reads them (a boolean, or a number
    /// meaning `true` iff it is 1); empty when absent. Informational.
    pub two_sided: Vec<bool>,
    pub guard_version: Option<i64>,
    pub raw: Value,
}

impl TableHeader {
    /// Parse a header object (errors on a malformed `two_sided` or `coordinate_priority`).
    pub fn from_value(v: Value) -> Result<Self, String> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let strs = |x: Option<&Value>| -> Vec<String> {
            x.and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|e| {
                            e.as_str()
                                .map(str::to_string)
                                .unwrap_or_else(|| e.to_string())
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        let usizes = |x: Option<&Value>| -> Result<Option<Vec<usize>>, String> {
            match x {
                None | Some(Value::Null) => Ok(None),
                Some(Value::Array(a)) => a
                    .iter()
                    .map(|e| {
                        e.as_u64()
                            .and_then(|u| usize::try_from(u).ok())
                            .ok_or_else(|| format!("coordinate_priority entry {e}"))
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(Some),
                Some(o) => Err(format!("coordinate_priority {o}")),
            }
        };
        let two_sided: Vec<bool> = match v.get("order").and_then(|o| o.get("two_sided")) {
            None | Some(Value::Null) => vec![],
            Some(Value::Array(a)) => a
                .iter()
                .map(|f| match f {
                    Value::Bool(b) => Ok(*b),
                    Value::Number(n) => Ok(n.as_u64() == Some(1)),
                    o => Err(format!("order.two_sided flag {o}")),
                })
                .collect::<Result<_, _>>()?,
            Some(o) => return Err(format!("order.two_sided {o}")),
        };
        let coordinate_priority = match usizes(
            v.get("node_order")
                .and_then(|o| o.get("coordinate_priority")),
        )? {
            Some(p) => Some(p),
            None => usizes(v.get("order").and_then(|o| o.get("coordinate_priority")))?,
        };
        Ok(TableHeader {
            schema: s("schema"),
            family: s("family"),
            denominators: v
                .get("denominators")
                .and_then(Value::as_u64)
                .map(|x| x as usize),
            mask: s("mask"),
            power_shifts: strs(v.get("power_shifts")),
            charts: strs(v.get("charts")),
            order_name: s("order_name"),
            coordinate_priority,
            two_sided,
            guard_version: v.get("guard_version").and_then(Value::as_i64),
            raw: v,
        })
    }
}

/// Which JSONL format a rule record came from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RecordFormat {
    /// rr_node `table_dump.v1` (`"type":"rule"`): carries `target` and `rhs`.
    #[default]
    TableDump,
    /// Case-domain export (rule line without `type`): carries NO target.
    CaseExport,
}

/// One rule record (domain fields; RHS keys only on request).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuleRecord {
    /// 1-based line number in the file.
    pub line_no: usize,
    /// The format of the line.
    pub format: RecordFormat,
    /// Case-domain export: record number `i`.
    pub i: Option<i64>,
    /// `index` field (engine rule index within the chart).
    pub index: Option<i64>,
    pub chart: Option<String>,
    /// `null` = free slot.
    pub fixed: Vec<Option<i64>>,
    pub equations: Vec<String>,
    /// OR-list of AND-lists of polynomial strings.
    pub exceptions: Vec<Vec<String>>,
    pub dispatch: Option<String>,
    pub numerical: Option<bool>,
    pub target: Option<String>,
    /// RHS key strings ([`RhsMode::KeysOnly`]); `None` when skipped or absent.
    pub rhs_keys: Option<Vec<String>>,
    /// Verbatim; may contain the literal `"elided"`. `None` when the record has no such field.
    pub witness_loci: Option<Vec<String>>,
    pub recenter: Option<Vec<i64>>,
    /// `domain_bounds` as written (shape checked here; MEANING: [`crate::domain::bound_branches`], the one place;
    /// lane partfix4, audit-part4 B1: this reader dropped the field).
    pub domain_bounds: Option<DomainBounds>,
    /// Top-level members outside [`RULE_RECORD_MEMBERS`] (sorted). A domain is never built from such a record
    /// ([`crate::domain::DomainError::UnknownMember`]): a member this crate does not know may change what the engine
    /// does with the record (audit-part4: never drop a field silently).
    pub unknown_members: Vec<String>,
}

/// A rule record's `domain_bounds` as written: inclusive bounds on the rule's CASE VARIABLES (the parameters `n_i` of
/// the target pattern), `None` = no bound. Shape (the engine's, rustred-core node/bounds.rs): an object with only
/// `min` / `max`, each absent, `null` or an array of the record's arity of integers or nulls; anything else is a
/// format error of the reader (never ignored). What a bound DOES is [`crate::domain::bound_branches`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DomainBounds {
    pub min: Vec<Option<i64>>,
    pub max: Vec<Option<i64>>,
}

impl DomainBounds {
    /// Parse a `domain_bounds` value for a record of `arity` slots (`null` is handled by the caller: no bounds).
    pub fn from_value(v: &Value, arity: usize) -> Result<DomainBounds, String> {
        let o = v
            .as_object()
            .ok_or_else(|| format!("domain_bounds is not an object: {v}"))?;
        if let Some(k) = o.keys().find(|k| *k != "min" && *k != "max") {
            return Err(format!("domain_bounds: unknown field {k:?}"));
        }
        let side = |k: &str| -> Result<Vec<Option<i64>>, String> {
            match o.get(k) {
                None | Some(Value::Null) => Ok(vec![None; arity]),
                Some(Value::Array(a)) if a.len() == arity => a
                    .iter()
                    .map(|x| match x {
                        Value::Null => Ok(None),
                        // the engine's `as_i64`: an integer of i64 (a float, a string, a u64 beyond i64: refused)
                        x => x.as_i64().map(Some).ok_or_else(|| {
                            format!("domain_bounds.{k} entry {x}: integers or null")
                        }),
                    })
                    .collect(),
                Some(x) => Err(format!(
                    "domain_bounds.{k} {x}: an array of the record's arity {arity}"
                )),
            }
        };
        Ok(DomainBounds {
            min: side("min")?,
            max: side("max")?,
        })
    }
}

/// The CLOSED list of top-level members a rule record may carry (lane partfix4; survey of every table of the campaign
/// on 2026-11-15: 1,335 files, 252,607 rule records, plus the engine's rule writer (rustred-core node/driver.rs, the
/// `{"type":"rule",..}` format string; symface.rs), the campaign's writers (closeloop proofs / derived / termmin) and
/// the test fixtures). A member outside it
/// makes [`RuleRecord::unknown_members`] non-empty and the record unloadable as a domain. Two groups:
/// [`RULE_MEMBERS_READ`] (read by this crate) and [`RULE_MEMBERS_INFORMATIONAL`] (provenance, statistics and
/// certification aids: the engine's first match, rustred-core mastermap.rs `Table::load` / `find_rule`, reads none of
/// them; neither does this crate's dispatcher).
pub const RULE_RECORD_MEMBERS: [&[&str]; 2] = [RULE_MEMBERS_READ, RULE_MEMBERS_INFORMATIONAL];

/// Rule-record members this crate reads (`type` / `i` / `index` identify; the others make the domain or the body).
pub const RULE_MEMBERS_READ: &[&str] = &[
    "type",
    "i",
    "index",
    "chart",
    "fixed",
    "equations",
    "exceptions",
    "dispatch",
    "numerical",
    "target",
    "rhs",
    "witness_loci",
    "recenter",
    "domain_bounds",
];

/// Rule-record members known to carry no dispatch meaning (see [`RULE_RECORD_MEMBERS`]).
pub const RULE_MEMBERS_INFORMATIONAL: &[&str] = &[
    "case",
    "seeds",
    "nsources",
    "direct_hit",
    "seconds_search",
    "raw_pivot_encoded",
    "trace_rows",
    "lift_seconds",
    "source_order",
    "source_index",
    "repair_piece",
    "monomials",
    "oversized_fallback",
    "witness_poles",
    // d-only pole loci (engine driver.rs rule writer): never vanish identically in d at a key; find_rule reads none
    "witness_d_poles",
    "guard_version",
    "origin",
    "origin_index",
    // the registry join's provenance of a re-indexed record (rustred-core node/registry.rs)
    "join_source",
    // free-text note of a hand-assembled record (the T162 box symmetry rules: "symmetry rule")
    "case_note",
    // provenance indices of an external assembler's records (seen in an imported assembled table)
    "component_variant",
    "component_index",
    "generation_index",
    "seed_setting",
    "sym_rows",
    "symmetry_origin",
    "symmetry_element",
    "symmetry_elements",
    "symmetry_weights",
    "supplement",
    "composite_of",
    "kind",
    "alternatives",
    "descent",
    "derivation",
    "symface",
    "relation_digest",
    "relation_rule",
];

/// Is `name` a member of [`RULE_RECORD_MEMBERS`]?
pub fn is_known_rule_member(name: &str) -> bool {
    RULE_RECORD_MEMBERS.iter().any(|g| g.contains(&name))
}

/// One line of a table file.
#[derive(Clone, Debug)]
pub enum TableLine {
    Header(Box<TableHeader>),
    Rule(Box<RuleRecord>),
    /// A `chart` line (`table_dump.v1` `{"type":"chart",..}` or export `{"other":{"type":"chart",..}}`): the chart
    /// mask and its guards.
    Chart {
        chart: String,
        guards: Vec<String>,
    },
    /// Any other line, with its kind (`type` value, or `other` / `summary` in the case-domain export).
    Other {
        kind: String,
    },
}

/// Streaming reader: one [`TableLine`] per non-empty input line.
pub struct RecordReader<R: BufRead> {
    inner: R,
    buf: Vec<u8>,
    line_no: usize,
    opts: ReadOptions,
}

impl<R: BufRead> RecordReader<R> {
    pub fn new(inner: R, opts: ReadOptions) -> Self {
        RecordReader {
            inner,
            buf: Vec::with_capacity(1 << 16),
            line_no: 0,
            opts,
        }
    }
}

impl RecordReader<std::io::BufReader<std::fs::File>> {
    pub fn open(path: impl AsRef<std::path::Path>, opts: ReadOptions) -> Result<Self, RecordError> {
        let f = std::fs::File::open(path)?;
        Ok(RecordReader::new(
            std::io::BufReader::with_capacity(1 << 20, f),
            opts,
        ))
    }
}

impl<R: BufRead> Iterator for RecordReader<R> {
    type Item = Result<TableLine, RecordError>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            self.buf.clear();
            match self.inner.read_until(b'\n', &mut self.buf) {
                Err(e) => return Some(Err(e.into())),
                Ok(0) => return None,
                Ok(_) => {}
            }
            self.line_no += 1;
            if self.buf.iter().all(|c| c.is_ascii_whitespace()) {
                continue;
            }
            return Some(parse_line(&self.buf, self.line_no, self.opts));
        }
    }
}

/// A whole table file read into memory (domains only unless `opts` asks for RHS keys).
#[derive(Clone, Debug, Default)]
pub struct TableFile {
    pub header: Option<TableHeader>,
    pub rules: Vec<RuleRecord>,
    /// `chart` lines (mask, guards) in file order.
    pub charts: Vec<(String, Vec<String>)>,
    /// Count of other lines by kind.
    pub others: Vec<(String, usize)>,
}

/// Read a table file (either JSONL format).
///
/// The header must come before every rule and chart line (re-audit RB m2: the engine's `Table::load` reads the
/// arity from the header, so a rule line before it is parsed with arity 0 and the load fails); a header after a rule
/// or chart line, and a second header, are format errors.
pub fn read_table_file(
    path: impl AsRef<std::path::Path>,
    opts: ReadOptions,
) -> Result<TableFile, RecordError> {
    let mut t = TableFile::default();
    let mut rd = RecordReader::open(path, opts)?;
    while let Some(l) = rd.next() {
        match l? {
            TableLine::Header(h) => {
                if t.header.is_some() {
                    return Err(RecordError::Format {
                        line: rd.line_no,
                        msg: "second header line".into(),
                    });
                }
                if !t.rules.is_empty() || !t.charts.is_empty() {
                    return Err(RecordError::Format {
                        line: rd.line_no,
                        msg: "header after rule or chart lines (the engine reads the arity from the header first)"
                            .into(),
                    });
                }
                t.header = Some(*h)
            }
            TableLine::Rule(r) => t.rules.push(*r),
            TableLine::Chart { chart, guards } => t.charts.push((chart, guards)),
            TableLine::Other { kind } => match t.others.iter_mut().find(|(k, _)| *k == kind) {
                Some(e) => e.1 += 1,
                None => t.others.push((kind, 1)),
            },
        }
    }
    Ok(t)
}

// ------------------------------------------------------------------------------------------------------------------
// Byte scanner
// ------------------------------------------------------------------------------------------------------------------

/// Top-level object fields: (key, byte span of the unparsed value).
type FieldSpans = Vec<(String, (usize, usize))>;

struct Scan<'a> {
    s: &'a [u8],
    pos: usize,
    line: usize,
}

impl<'a> Scan<'a> {
    fn err(&self, msg: impl Into<String>) -> RecordError {
        RecordError::Format {
            line: self.line,
            msg: format!("{} (byte {})", msg.into(), self.pos),
        }
    }
    fn ws(&mut self) {
        while self.pos < self.s.len() && self.s[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }
    fn expect(&mut self, c: u8) -> Result<(), RecordError> {
        self.ws();
        if self.s.get(self.pos) == Some(&c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.err(format!("expected {:?}", c as char)))
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.pos).copied()
    }
    /// Span of a string including quotes.
    fn string_span(&mut self) -> Result<(usize, usize), RecordError> {
        self.ws();
        let start = self.pos;
        if self.s.get(self.pos) != Some(&b'"') {
            return Err(self.err("expected a string"));
        }
        self.pos += 1;
        while self.pos < self.s.len() {
            match self.s[self.pos] {
                b'\\' => self.pos += 2,
                b'"' => {
                    self.pos += 1;
                    return Ok((start, self.pos));
                }
                _ => self.pos += 1,
            }
        }
        Err(self.err("unterminated string"))
    }
    /// Skip any value; returns its span.
    fn skip_value(&mut self) -> Result<(usize, usize), RecordError> {
        self.ws();
        let start = self.pos;
        match self.s.get(self.pos) {
            None => Err(self.err("unexpected end of line")),
            Some(b'"') => self.string_span(),
            Some(b'{') | Some(b'[') => {
                let mut depth = 0usize;
                while self.pos < self.s.len() {
                    match self.s[self.pos] {
                        b'"' => {
                            self.string_span()?;
                            continue;
                        }
                        b'{' | b'[' => depth += 1,
                        b'}' | b']' => {
                            depth -= 1;
                            if depth == 0 {
                                self.pos += 1;
                                return Ok((start, self.pos));
                            }
                        }
                        _ => {}
                    }
                    self.pos += 1;
                }
                Err(self.err("unterminated object or array"))
            }
            Some(_) => {
                while self.pos < self.s.len() && !matches!(self.s[self.pos], b',' | b'}' | b']') {
                    self.pos += 1;
                }
                let mut end = self.pos;
                while end > start && self.s[end - 1].is_ascii_whitespace() {
                    end -= 1;
                }
                Ok((start, end))
            }
        }
    }
    /// Top-level object: (key, value span) pairs, values skipped (not parsed).
    fn object_fields(&mut self) -> Result<FieldSpans, RecordError> {
        self.expect(b'{')?;
        let mut out = vec![];
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(out);
        }
        loop {
            let (a, b) = self.string_span()?;
            let k: String =
                serde_json::from_slice(&self.s[a..b]).map_err(|e| self.err(e.to_string()))?;
            self.expect(b':')?;
            let span = self.skip_value()?;
            if out.iter().any(|(n, _)| *n == k) {
                return Err(self.err(format!("duplicate key {k:?}")));
            }
            out.push((k, span));
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(out);
                }
                _ => return Err(self.err("expected ',' or '}'")),
            }
        }
    }
}

/// Keys of an `rhs` array `[[key, coef], ...]`, coefficients skipped unparsed.
fn rhs_keys(s: &[u8], line: usize) -> Result<Vec<String>, RecordError> {
    let mut sc = Scan { s, pos: 0, line };
    let mut out = vec![];
    sc.expect(b'[')?;
    if sc.peek() == Some(b']') {
        return Ok(out);
    }
    loop {
        sc.expect(b'[')?;
        let (a, b) = sc.string_span()?;
        out.push(serde_json::from_slice(&s[a..b]).map_err(|e| sc.err(e.to_string()))?);
        while sc.peek() == Some(b',') {
            sc.pos += 1;
            sc.skip_value()?;
        }
        sc.expect(b']')?;
        match sc.peek() {
            Some(b',') => sc.pos += 1,
            Some(b']') => return Ok(out),
            _ => return Err(sc.err("expected ',' or ']' in rhs")),
        }
    }
}

fn parse_line(buf: &[u8], line: usize, opts: ReadOptions) -> Result<TableLine, RecordError> {
    let mut sc = Scan {
        s: buf,
        pos: 0,
        line,
    };
    let fields = sc.object_fields()?;
    sc.ws();
    if sc.pos != buf.len() {
        return Err(sc.err("trailing bytes after the object"));
    }
    let get = |k: &str| {
        fields
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, sp)| &buf[sp.0..sp.1])
    };
    let fmt = |m: String| RecordError::Format { line, msg: m };
    fn de<'a, T: Deserialize<'a>>(b: &'a [u8], field: &str, line: usize) -> Result<T, RecordError> {
        serde_json::from_slice(b).map_err(|e| RecordError::Format {
            line,
            msg: format!("field {field}: {e}"),
        })
    }
    let kind: Option<String> = match get("type") {
        Some(b) => Some(de(b, "type", line)?),
        None => None,
    };
    let chart_line = |v: &Value| -> Result<TableLine, RecordError> {
        let chart = v
            .get("chart")
            .and_then(Value::as_str)
            .ok_or_else(|| fmt("chart line without chart".into()))?
            .to_string();
        let guards = match v.get("guards") {
            None | Some(Value::Null) => vec![],
            Some(g) => serde_json::from_value::<Vec<String>>(g.clone())
                .map_err(|e| fmt(format!("chart guards: {e}")))?,
        };
        Ok(TableLine::Chart { chart, guards })
    };
    let format = if kind.is_some() {
        RecordFormat::TableDump
    } else {
        RecordFormat::CaseExport
    };
    match kind.as_deref() {
        Some("header") => {
            let v: Value = serde_json::from_slice(buf).map_err(|e| fmt(e.to_string()))?;
            return Ok(TableLine::Header(Box::new(
                TableHeader::from_value(v).map_err(fmt)?,
            )));
        }
        Some("chart") => {
            let v: Value = serde_json::from_slice(buf).map_err(|e| fmt(e.to_string()))?;
            return chart_line(&v);
        }
        Some("rule") => {}
        Some(k) => {
            return Ok(TableLine::Other {
                kind: k.to_string(),
            });
        }
        None => {
            if let Some(h) = get("header") {
                let v: Value = de(h, "header", line)?;
                return Ok(TableLine::Header(Box::new(
                    TableHeader::from_value(v).map_err(fmt)?,
                )));
            }
            if let Some(o) = get("other") {
                let v: Value = de(o, "other", line)?;
                if v.get("type").and_then(Value::as_str) == Some("chart") {
                    return chart_line(&v);
                }
            }
            // case-domain export: the only non-rule lines are the single-key wrappers `other` / `summary`;
            // anything else must be a rule and carry `fixed` (audit B M2: never drop a malformed rule silently)
            if fields.len() == 1 && matches!(fields[0].0.as_str(), "other" | "summary") {
                return Ok(TableLine::Other {
                    kind: fields[0].0.clone(),
                });
            }
            if get("fixed").is_none() {
                return Err(fmt(format!(
                    "line without type, not a header/other/summary wrapper and without fixed (keys {:?})",
                    fields.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>()
                )));
            }
        }
    }
    let mut r = RuleRecord {
        line_no: line,
        format,
        ..Default::default()
    };
    let opt_i64 = |k: &str| -> Result<Option<i64>, RecordError> {
        match get(k) {
            Some(b) => de::<Option<i64>>(b, k, line),
            None => Ok(None),
        }
    };
    r.i = opt_i64("i")?;
    r.index = opt_i64("index")?;
    if let Some(b) = get("chart") {
        r.chart = de(b, "chart", line)?;
    }
    r.fixed = de(
        get("fixed").ok_or_else(|| fmt("rule without fixed".into()))?,
        "fixed",
        line,
    )?;
    // stricter than the engine (which reads a missing `equations` as none): every record at hand carries it, and a
    // record without it is malformed (audit B M1)
    r.equations = de::<Vec<String>>(
        get("equations").ok_or_else(|| fmt("rule without equations".into()))?,
        "equations",
        line,
    )?;
    // the engine: `v["exceptions"].as_array().ok_or("rule without exceptions")`
    r.exceptions = de::<Vec<Vec<String>>>(
        get("exceptions").ok_or_else(|| fmt("rule without exceptions".into()))?,
        "exceptions",
        line,
    )?;
    if let Some(b) = get("dispatch") {
        r.dispatch = de(b, "dispatch", line)?;
    }
    if let Some(b) = get("numerical") {
        r.numerical = de(b, "numerical", line)?;
    }
    if let Some(b) = get("target") {
        r.target = de(b, "target", line)?;
    }
    if format == RecordFormat::TableDump {
        // `Table::load`: `parse_pat(v["target"])` and `v["rhs"].as_array().ok_or("rule without rhs")`
        if r.target.is_none() {
            return Err(fmt("table_dump.v1 rule without target".into()));
        }
        match get("rhs") {
            Some(b) if b.first() == Some(&b'[') => {}
            _ => return Err(fmt("table_dump.v1 rule without an rhs array".into())),
        }
    }
    if let Some(b) = get("witness_loci") {
        r.witness_loci = de(b, "witness_loci", line)?;
    }
    if let Some(b) = get("recenter") {
        r.recenter = de(b, "recenter", line)?;
    }
    // audit-part4 B1: the field is read here, by THE reader of rule lines (the import's streamed lines come through
    // this function too); null = absent, as in the engine
    if let Some(b) = get("domain_bounds") {
        let v: Value = de(b, "domain_bounds", line)?;
        if !v.is_null() {
            r.domain_bounds = Some(DomainBounds::from_value(&v, r.fixed.len()).map_err(fmt)?);
        }
    }
    r.unknown_members = fields
        .iter()
        .map(|(n, _)| n.as_str())
        .filter(|n| !is_known_rule_member(n))
        .map(str::to_string)
        .collect();
    r.unknown_members.sort();
    if opts.rhs == RhsMode::KeysOnly
        && let Some(b) = get("rhs")
    {
        r.rhs_keys = Some(rhs_keys(b, line)?);
    }
    Ok(TableLine::Rule(Box::new(r)))
}

// ------------------------------------------------------------------------------------------------------------------
// Line kinds of a `table_dump.v1` file; `sym` records
// ------------------------------------------------------------------------------------------------------------------

/// The CLOSED list of line kinds a rule table (`F.table.jsonl`) may contain (interfaces/coord-sym-records.md section
/// 6; interfaces/import.md 18.1). One list for the import and the assembly; any other kind is refused by both.
/// `sym_zero` is deliberately absent: zero by symmetry needs a sign flip, impossible with squared propagators (the
/// engine's replay refuses it); readers refuse a table that carries one ([`SYM_ZERO_KIND`]).
pub const TABLE_LINE_KINDS: &[&str] = &[
    "header",
    "rule",
    "sym",
    "zero_sectors",
    "chart",
    "chart_error",
    "symmetry_group",
    "residual",
    "chart_end",
    "end",
    "assembly_provenance",
    "composite_assembly_complete",
    "assembly_end",
    "composite_terminal_ledger",
];

/// The kind of a one-term symmetry rule record ([`SymRecord`]).
pub const SYM_KIND: &str = "sym";
/// The kind of a zero-by-symmetry record: never valid for squared propagators; a table carrying one is refused.
pub const SYM_ZERO_KIND: &str = "sym_zero";

/// A `"type":"sym"` record (engine `rr_node generate --symmetry`; interfaces/coord-sym-records.md section 1): the case
/// (`chart`, `fixed`, `equations`) is closed by `I(a) = s(g, a) * I(g a)`: ONE right-hand-side term `image`, unit
/// coefficient, no exceptions. Parsed strictly: a member `exceptions` or `rhs` is a format error (a sym record serves
/// exactly one term and has no exception); unknown other members are ignored (provenance), as readers of rule records do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymRecord {
    /// 1-based line number.
    pub line_no: usize,
    pub chart: String,
    /// Position in the table's ONE index sequence shared with rule records.
    pub index: i64,
    /// `null` = free slot.
    pub fixed: Vec<Option<i64>>,
    pub equations: Vec<String>,
    /// The key pattern `I(..)` of the case.
    pub target: String,
    /// The pattern of `g(target)`.
    pub image: String,
    /// Index into the table's `symmetry_group` elements (informational: the import verifies against its own group).
    pub element: Option<i64>,
    /// Per slot: `(target slot, flip)`, or `None` (the slot goes to a combination: fixed at 0 on the case).
    pub action: Vec<Option<(usize, bool)>>,
    /// The momentum map T, `(L+1)^2` integers row-major, row-vector convention `v -> v T` over `(k_1..k_L, q)`.
    pub matrix: Vec<i64>,
    /// `+`, `-` or `key`.
    pub sign: String,
    pub numerical: Option<bool>,
}

/// Parse one `sym` line (the caller has established that its `type` is [`SYM_KIND`]).
pub fn parse_sym_line(buf: &[u8], line: usize) -> Result<SymRecord, RecordError> {
    let fmt = |m: String| RecordError::Format { line, msg: m };
    let v: Value = serde_json::from_slice(buf).map_err(|e| fmt(e.to_string()))?;
    let o = v
        .as_object()
        .ok_or_else(|| fmt("sym record is not an object".into()))?;
    if o.get("type").and_then(Value::as_str) != Some(SYM_KIND) {
        return Err(fmt("not a sym record".into()));
    }
    for k in ["exceptions", "rhs"] {
        if o.contains_key(k) {
            return Err(fmt(format!(
                "sym record with a member {k:?} (a sym record has one term and no exception)"
            )));
        }
    }
    let s = |k: &str| -> Result<String, RecordError> {
        o.get(k)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| fmt(format!("sym record without a string {k}")))
    };
    let fixed: Vec<Option<i64>> = serde_json::from_value(
        o.get("fixed")
            .cloned()
            .ok_or_else(|| fmt("sym record without fixed".into()))?,
    )
    .map_err(|e| fmt(format!("field fixed: {e}")))?;
    let equations: Vec<String> = match o.get("equations") {
        None => return Err(fmt("sym record without equations".into())),
        Some(e) => {
            serde_json::from_value(e.clone()).map_err(|e| fmt(format!("field equations: {e}")))?
        }
    };
    let index = o
        .get("index")
        .and_then(Value::as_i64)
        .ok_or_else(|| fmt("sym record without an integer index".into()))?;
    let action = match o.get("action") {
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| match x {
                Value::Null => Ok(None),
                Value::Array(p) if p.len() == 2 => {
                    let t = p[0]
                        .as_u64()
                        .and_then(|t| usize::try_from(t).ok())
                        .ok_or_else(|| fmt(format!("action entry {x}")))?;
                    let flip = match &p[1] {
                        Value::Bool(b) => *b,
                        Value::Number(n) if n.as_u64() == Some(0) => false,
                        Value::Number(n) if n.as_u64() == Some(1) => true,
                        _ => return Err(fmt(format!("action entry {x}"))),
                    };
                    Ok(Some((t, flip)))
                }
                _ => Err(fmt(format!("action entry {x}"))),
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(fmt("sym record without an action array".into())),
    };
    let matrix: Vec<i64> = serde_json::from_value(
        o.get("matrix")
            .cloned()
            .ok_or_else(|| fmt("sym record without matrix".into()))?,
    )
    .map_err(|e| fmt(format!("field matrix: {e}")))?;
    let element = match o.get("element") {
        None | Some(Value::Null) => None,
        Some(e) => Some(
            e.as_i64()
                .ok_or_else(|| fmt(format!("field element {e}")))?,
        ),
    };
    let numerical = match o.get("numerical") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(b)) => Some(*b),
        Some(x) => return Err(fmt(format!("field numerical {x}"))),
    };
    let r = SymRecord {
        line_no: line,
        chart: s("chart")?,
        index,
        fixed,
        equations,
        target: s("target")?,
        image: s("image")?,
        element,
        action,
        matrix,
        sign: s("sign")?,
        numerical,
    };
    if r.action.len() != r.fixed.len() {
        return Err(fmt(format!(
            "sym record: action of length {} for {} slots",
            r.action.len(),
            r.fixed.len()
        )));
    }
    Ok(r)
}

impl SymRecord {
    /// The record as the rule record the dispatcher reads: same case, no exceptions, ONE body term `image`, complete
    /// (empty) witness loci (unit coefficient: no pole), dispatch `Partition`. `with_rhs`: the body key (else None).
    pub fn as_rule_record(&self, with_rhs: bool) -> RuleRecord {
        RuleRecord {
            line_no: self.line_no,
            format: RecordFormat::TableDump,
            i: None,
            index: Some(self.index),
            chart: Some(self.chart.clone()),
            fixed: self.fixed.clone(),
            equations: self.equations.clone(),
            exceptions: vec![],
            dispatch: Some("Partition".into()),
            numerical: self.numerical,
            target: Some(self.target.clone()),
            rhs_keys: with_rhs.then(|| vec![self.image.clone()]),
            witness_loci: Some(vec![]),
            recenter: None,
            domain_bounds: None,
            unknown_members: vec![],
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------
// T162 bank predicates
// ------------------------------------------------------------------------------------------------------------------

/// A field that must be present (it may be `null`): serde's default for a missing `Option` would be `None`.
fn required<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(d)
}

/// A field that must be present AND a list (`null` is an error; re-audit RB m3). Kept as `Option` (always `Some`)
/// so that the field types stay as they were.
fn required_list<'de, D, T>(d: D) -> Result<Option<Vec<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Vec::<T>::deserialize(d).map(Some)
}

/// One rule of `RULE-PREDICATES2440.json` (schema `t162-exact-written-domain-complement.v1`; fields as listed in the
/// helper's TOOLS-INVENTORY "partition / Inputs"). Every domain field must be PRESENT (audit B M3: a predicate without
/// `domain_atoms` used to load with no chart/recenter bounds) and a LIST: `null` is an error everywhere except in
/// `cached_domain_exception_or_pole_AND_loci`, the one field where the real bank writes `null` (2,415 of 2,440
/// records) and where it reads as an empty list (re-audit RB m3: a `null` exception list used to read as "no
/// exception", i.e. fail open). Unused fields are ignored.
#[derive(Clone, Debug, Deserialize)]
pub struct BankPredicate {
    pub global_ordinal0: i64,
    #[serde(default)]
    pub source_id: Option<String>,
    /// 20 values or null.
    pub raw_fixed20: Vec<Option<i64>>,
    #[serde(deserialize_with = "required_list", rename = "all_raw_Case_equations")]
    pub case_equations: Option<Vec<String>>,
    /// `[rel, text]` with rel `EQ` (= 0) or `GE` (>= 0): the chart/recenter bounds and fixed slots. REQUIRED, not null.
    pub domain_atoms: Vec<(String, String)>,
    #[serde(
        deserialize_with = "required_list",
        rename = "raw_written_exceptions_OR_of_AND"
    )]
    pub exceptions: Option<Vec<Vec<String>>>,
    /// The only field where `null` is accepted (= empty list), as the real bank writes it.
    #[serde(
        deserialize_with = "required",
        rename = "cached_domain_exception_or_pole_AND_loci"
    )]
    pub cached_loci: Option<Vec<Vec<String>>>,
    /// d-polynomials; forbidden where all d-coefficients vanish.
    #[serde(
        deserialize_with = "required_list",
        rename = "literal_written_denominators_generic_d_nonzero"
    )]
    pub denominators: Option<Vec<String>>,
    #[serde(
        deserialize_with = "required_list",
        rename = "raw_header_source_conditions_generic_d_nonzero"
    )]
    pub source_conditions: Option<Vec<String>>,
    #[serde(
        deserialize_with = "required_list",
        rename = "raw_rule_witness_loci_generic_d_nonzero"
    )]
    pub witness_loci: Option<Vec<String>>,
    /// Must be `true` for the domain loader (the bank's own statement that its slots are the canonical ones).
    #[serde(default)]
    pub no_recenter_or_permutation_applied: Option<bool>,
}

/// Read the T162 bank predicate file (one JSON array).
pub fn read_bank_predicates(
    path: impl AsRef<std::path::Path>,
) -> Result<Vec<BankPredicate>, RecordError> {
    let f = std::io::BufReader::new(std::fs::File::open(path)?);
    serde_json::from_reader(f).map_err(|e| RecordError::Format {
        line: e.line(),
        msg: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_rule_line_skips_rhs() {
        let line = br#"{"type":"rule","chart":"110","index":3,"fixed":[null,1,null],"equations":["n0=n2"],"target":"I(n0,1,n2)","rhs":[["I(n0-1,1,n2)","n0/(n2-1) \"q\""],["I(n0,1,n2-1)","[1]"]],"exceptions":[["-1+n0"],["n2","n0"]],"dispatch":"Partition","witness_loci":["elided"],"recenter":[0,0,1]}"#;
        let o = ReadOptions {
            rhs: RhsMode::KeysOnly,
        };
        let TableLine::Rule(r) = parse_line(line, 7, o).unwrap() else {
            panic!()
        };
        assert_eq!(r.index, Some(3));
        assert_eq!(r.fixed, vec![None, Some(1), None]);
        assert_eq!(
            r.exceptions,
            vec![vec!["-1+n0".to_string()], vec!["n2".into(), "n0".into()]]
        );
        assert_eq!(r.rhs_keys.as_ref().unwrap().len(), 2);
        assert_eq!(r.witness_loci, Some(vec!["elided".to_string()]));
        let TableLine::Rule(r2) = parse_line(line, 7, ReadOptions::default()).unwrap() else {
            panic!()
        };
        assert_eq!(r2.rhs_keys, None);
    }
}
