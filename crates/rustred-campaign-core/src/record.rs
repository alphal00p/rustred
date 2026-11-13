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

use std::fmt;
use std::io::BufRead;

use serde::Deserialize;
use serde_json::Value;

/// Errors of the readers.
#[derive(Debug)]
pub enum RecordError {
    Io(std::io::Error),
    /// Malformed JSON or an unexpected field type, with the 1-based line number.
    Format {
        line: usize,
        msg: String,
    },
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecordError::Io(e) => write!(f, "io: {e}"),
            RecordError::Format { line, msg } => write!(f, "line {line}: {msg}"),
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

/// Parsed table header (both formats). `raw` keeps the whole header object.
#[derive(Clone, Debug)]
pub struct TableHeader {
    pub schema: Option<String>,
    pub family: Option<String>,
    /// `denominators` = arity.
    pub denominators: Option<usize>,
    pub mask: Option<String>,
    pub power_shifts: Vec<String>,
    pub charts: Vec<String>,
    pub order_name: Option<String>,
    pub coordinate_priority: Option<Vec<usize>>,
    pub two_sided: Vec<usize>,
    pub guard_version: Option<i64>,
    pub raw: Value,
}

impl TableHeader {
    fn from_value(v: Value) -> Self {
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
        let usizes = |x: Option<&Value>| -> Option<Vec<usize>> {
            x.and_then(Value::as_array).map(|a| {
                a.iter()
                    .filter_map(|e| e.as_u64().map(|u| u as usize))
                    .collect()
            })
        };
        TableHeader {
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
            coordinate_priority: usizes(v.get("order").and_then(|o| o.get("coordinate_priority")))
                .or_else(|| {
                    usizes(
                        v.get("node_order")
                            .and_then(|o| o.get("coordinate_priority")),
                    )
                }),
            two_sided: usizes(v.get("order").and_then(|o| o.get("two_sided"))).unwrap_or_default(),
            guard_version: v.get("guard_version").and_then(Value::as_i64),
            raw: v,
        }
    }
}

/// One rule record (domain fields; RHS keys only on request).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuleRecord {
    /// 1-based line number in the file.
    pub line_no: usize,
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
}

/// One line of a table file.
#[derive(Clone, Debug)]
pub enum TableLine {
    Header(Box<TableHeader>),
    Rule(Box<RuleRecord>),
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
    /// Count of other lines by kind.
    pub others: Vec<(String, usize)>,
}

/// Read a table file (either JSONL format).
pub fn read_table_file(
    path: impl AsRef<std::path::Path>,
    opts: ReadOptions,
) -> Result<TableFile, RecordError> {
    let mut t = TableFile::default();
    for l in RecordReader::open(path, opts)? {
        match l? {
            TableLine::Header(h) => t.header = Some(*h),
            TableLine::Rule(r) => t.rules.push(*r),
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
    match kind.as_deref() {
        Some("header") => {
            let v: Value = serde_json::from_slice(buf).map_err(|e| fmt(e.to_string()))?;
            return Ok(TableLine::Header(Box::new(TableHeader::from_value(v))));
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
                return Ok(TableLine::Header(Box::new(TableHeader::from_value(v))));
            }
            if get("fixed").is_none() {
                let k = fields
                    .first()
                    .map_or("empty".to_string(), |(n, _)| n.clone());
                return Ok(TableLine::Other { kind: k });
            }
        }
    }
    let mut r = RuleRecord {
        line_no: line,
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
    if let Some(b) = get("equations") {
        r.equations = de::<Option<Vec<String>>>(b, "equations", line)?.unwrap_or_default();
    }
    if let Some(b) = get("exceptions") {
        r.exceptions = de::<Option<Vec<Vec<String>>>>(b, "exceptions", line)?.unwrap_or_default();
    }
    if let Some(b) = get("dispatch") {
        r.dispatch = de(b, "dispatch", line)?;
    }
    if let Some(b) = get("numerical") {
        r.numerical = de(b, "numerical", line)?;
    }
    if let Some(b) = get("target") {
        r.target = de(b, "target", line)?;
    }
    if let Some(b) = get("witness_loci") {
        r.witness_loci = de(b, "witness_loci", line)?;
    }
    if let Some(b) = get("recenter") {
        r.recenter = de(b, "recenter", line)?;
    }
    if opts.rhs == RhsMode::KeysOnly
        && let Some(b) = get("rhs")
    {
        r.rhs_keys = Some(rhs_keys(b, line)?);
    }
    Ok(TableLine::Rule(Box::new(r)))
}

// ------------------------------------------------------------------------------------------------------------------
// T162 bank predicates
// ------------------------------------------------------------------------------------------------------------------

/// One rule of `RULE-PREDICATES2440.json` (schema `t162-exact-written-domain-complement.v1`; fields as listed in the
/// helper's TOOLS-INVENTORY "partition / Inputs"). Unused fields are ignored.
#[derive(Clone, Debug, Deserialize)]
pub struct BankPredicate {
    pub global_ordinal0: i64,
    #[serde(default)]
    pub source_id: Option<String>,
    /// 20 values or null.
    pub raw_fixed20: Vec<Option<i64>>,
    #[serde(default, rename = "all_raw_Case_equations")]
    pub case_equations: Option<Vec<String>>,
    /// `[rel, text]` with rel `EQ` (= 0) or `GE` (>= 0): the chart/recenter bounds and fixed slots.
    #[serde(default)]
    pub domain_atoms: Vec<(String, String)>,
    #[serde(default, rename = "raw_written_exceptions_OR_of_AND")]
    pub exceptions: Option<Vec<Vec<String>>>,
    #[serde(default, rename = "cached_domain_exception_or_pole_AND_loci")]
    pub cached_loci: Option<Vec<Vec<String>>>,
    /// d-polynomials; forbidden where all d-coefficients vanish.
    #[serde(default, rename = "literal_written_denominators_generic_d_nonzero")]
    pub denominators: Option<Vec<String>>,
    #[serde(default, rename = "raw_header_source_conditions_generic_d_nonzero")]
    pub source_conditions: Option<Vec<String>>,
    #[serde(default, rename = "raw_rule_witness_loci_generic_d_nonzero")]
    pub witness_loci: Option<Vec<String>>,
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
